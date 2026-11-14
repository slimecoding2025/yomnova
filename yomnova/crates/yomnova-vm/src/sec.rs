//! Natives for defensive analysis: hashing, encodings, byte search and entropy.
//! Everything here is a pure function of its arguments (no I/O, no clock, no randomness).

use crate::VmError;
use yomnova_bytecode::Value;

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// HMAC-SHA-256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = if key.len() > 64 {
        sha256(key).to_vec()
    } else {
        key.to_vec()
    };
    k.resize(64, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(msg);
    let inner_hash = sha256(&inner);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

pub fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    let t = text.trim();
    if t.len() % 2 == 1 {
        return Err("hex text must have an even number of digits".to_string());
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("'{}' is not a hex digit", c as char)),
    };
    t.as_bytes()
        .chunks(2)
        .map(|p| Ok(digit(p[0])? << 4 | digit(p[1])?))
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let t: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if t.len() & 3 != 0 {
        return Err("base64 text length must be a multiple of 4".to_string());
    }
    let mut out = Vec::new();
    for (i, c) in t.chunks(4).enumerate() {
        let last = (i + 1) * 4 == t.len();
        let pad = c.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return Err("misplaced '=' padding in base64 text".to_string());
        }
        let mut n = 0u32;
        for &b in &c[..4 - pad] {
            let v = B64
                .iter()
                .position(|&x| x == b)
                .ok_or_else(|| format!("'{}' is not a base64 character", b as char))?;
            n = n << 6 | v as u32;
        }
        n <<= 6 * pad as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// Shannon entropy in bits per byte (0.0 to 8.0); 0.0 for an empty buffer.
pub fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let n = data.len() as f64;
    let h: f64 = counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum();
    // A single repeated byte gives exactly -0.0 above; report a plain 0.
    if h <= 0.0 {
        0.0
    } else {
        h
    }
}

/// Index of the first occurrence of `needle` at or after `from`, or -1. An empty needle is never found.
pub fn find(hay: &[u8], needle: &[u8], from: usize) -> i64 {
    if needle.is_empty() || from > hay.len() || needle.len() > hay.len() - from {
        return -1;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map_or(-1, |p| (p + from) as i64)
}

/// Length of the longest run of one repeated byte value.
pub fn longest_run(data: &[u8], value: u8) -> usize {
    let (mut best, mut cur) = (0, 0);
    for &b in data {
        if b == value {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

/// Parses a byte pattern written as hex digit pairs where `??` matches any byte; spaces are ignored.
pub fn parse_wild(text: &str) -> Result<Vec<Option<u8>>, String> {
    let t: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if t.is_empty() {
        return Err("the pattern is empty".to_string());
    }
    if t.len() % 2 == 1 {
        return Err("the pattern must be whole bytes (two digits each)".to_string());
    }
    if t.len() / 2 > 4096 {
        return Err("the pattern is longer than 4096 bytes".to_string());
    }
    t.chunks(2)
        .map(|p| {
            if p == b"??" {
                Ok(None)
            } else {
                let s = std::str::from_utf8(p).map_err(|_| "not hex".to_string())?;
                hex_decode(s).map(|v| Some(v[0]))
            }
        })
        .collect()
}

/// Like `find`, but `None` entries of the pattern match any byte.
pub fn find_wild(hay: &[u8], pat: &[Option<u8>], from: usize) -> i64 {
    if pat.is_empty() || from > hay.len() || pat.len() > hay.len() - from {
        return -1;
    }
    hay[from..]
        .windows(pat.len())
        .position(|w| w.iter().zip(pat).all(|(b, p)| p.is_none_or(|x| x == *b)))
        .map_or(-1, |p| (p + from) as i64)
}

/// Dispatch for the analysis natives.
pub fn sec_native(name: &str, args: &[Value], max_buffer: usize) -> Result<Value, VmError> {
    let buf = |i: usize| match &args[i] {
        Value::Buffer(b) => Ok(b),
        other => Err(VmError::TypeError(format!(
            "{name} expects a NetworkBuffer, got {other:?}"
        ))),
    };
    let text = |i: usize| match &args[i] {
        Value::Text(t) => Ok(t.as_str()),
        other => Err(VmError::TypeError(format!(
            "{name} expects Text, got {other:?}"
        ))),
    };
    let proto = |e: String| VmError::ProtocolError(format!("{name}: {e}"));
    Ok(match name {
        "sha256" => Value::Text(hex_encode(&sha256(buf(0)?))),
        "hmac_sha256" => Value::Text(hex_encode(&hmac_sha256(buf(0)?, buf(1)?))),
        "hex_encode" => Value::Text(hex_encode(buf(0)?)),
        "base64_encode" => Value::Text(base64_encode(buf(0)?)),
        "hex_decode" | "base64_decode" => {
            let bytes = if name == "hex_decode" {
                hex_decode(text(0)?)
            } else {
                base64_decode(text(0)?)
            }
            .map_err(proto)?;
            if bytes.len() > max_buffer {
                return Err(VmError::ResourceLimit(format!(
                    "{name}: result larger than {max_buffer} bytes"
                )));
            }
            Value::Buffer(bytes)
        }
        "buf_entropy" => Value::Float(entropy(buf(0)?)),
        "buf_find" => {
            let from = match &args[2] {
                Value::Int(n) if *n >= 0 => *n as usize,
                Value::Int(n) => return Err(proto(format!("negative start {n}"))),
                other => {
                    return Err(VmError::TypeError(format!(
                        "{name} expects an Int start, got {other:?}"
                    )))
                }
            };
            Value::Int(find(buf(0)?, buf(1)?, from))
        }
        "buf_longest_run" => {
            let v = match &args[1] {
                Value::Int(n) if (0..=255).contains(n) => *n as u8,
                Value::Int(n) => return Err(proto(format!("{n} is not a byte value (0 to 255)"))),
                other => {
                    return Err(VmError::TypeError(format!(
                        "{name} expects an Int byte value, got {other:?}"
                    )))
                }
            };
            Value::Int(longest_run(buf(0)?, v) as i64)
        }
        "buf_find_wild" => {
            let from = match &args[2] {
                Value::Int(n) if *n >= 0 => *n as usize,
                Value::Int(n) => return Err(proto(format!("negative start {n}"))),
                other => {
                    return Err(VmError::TypeError(format!(
                        "{name} expects an Int start, got {other:?}"
                    )))
                }
            };
            let pat = parse_wild(text(1)?).map_err(proto)?;
            Value::Int(find_wild(buf(0)?, &pat, from))
        }
        other => {
            return Err(VmError::TypeError(format!(
                "Unknown native function '{other}'"
            )))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_nist_vectors() {
        assert_eq!(
            hex_encode(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex_encode(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_encode(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        assert_eq!(
            hex_encode(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn encodings_round_trip_and_reject_bad_input() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        for s in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            assert_eq!(base64_decode(&base64_encode(s)).unwrap(), s);
        }
        assert!(base64_decode("TQ=").is_err());
        assert!(base64_decode("T!==").is_err());
        assert_eq!(hex_decode("00ff10").unwrap(), vec![0, 255, 16]);
        assert!(hex_decode("abc").is_err());
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn wildcard_patterns_match_any_byte_where_marked() {
        let p = |s: &str| parse_wild(s).unwrap();
        assert_eq!(find_wild(b"\x90\x41\x90\x42", &p("90 ?? 90"), 0), 0);
        assert_eq!(find_wild(b"\x90\x41\x91\x42", &p("90 ?? 90"), 0), -1);
        assert_eq!(find_wild(b"abcdef", &p("63??65"), 0), 2);
        assert_eq!(find_wild(b"abcdef", &p("63??65"), 3), -1);
        assert_eq!(find_wild(b"ab", &p("??"), 1), 1);
        assert_eq!(find_wild(b"ab", &p("616263"), 0), -1);
        assert!(parse_wild("").is_err());
        assert!(parse_wild("9").is_err());
        assert!(parse_wild("zz").is_err());
        assert!(parse_wild("?9").is_err());
        assert!(parse_wild(&"00".repeat(4097)).is_err());
    }

    #[test]
    fn longest_run_counts_the_best_streak() {
        assert_eq!(longest_run(&[], 0), 0);
        assert_eq!(longest_run(&[1, 0, 0, 0, 1, 0, 0], 0), 3);
        assert_eq!(longest_run(&[5, 5], 6), 0);
    }

    #[test]
    fn entropy_and_find_behave() {
        assert_eq!(entropy(&[]), 0.0);
        assert!(entropy(&[7; 100]).is_sign_positive());
        assert_eq!(entropy(&[7; 100]), 0.0);
        let all: Vec<u8> = (0..=255).collect();
        assert!((entropy(&all) - 8.0).abs() < 1e-9);
        assert_eq!(find(b"hello world", b"o w", 0), 4);
        assert_eq!(find(b"hello world", b"o", 5), 7);
        assert_eq!(find(b"hello", b"z", 0), -1);
        assert_eq!(find(b"hello", b"", 0), -1);
        assert_eq!(find(b"hi", b"hello", 0), -1);
        assert_eq!(find(b"hi", b"h", 9), -1);
    }
}
