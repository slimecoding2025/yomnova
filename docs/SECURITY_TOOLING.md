# Security tooling in YOMNOVA

YOMNOVA aims at defensive security work: analysing and detecting, not attacking. It ships no exploits, no malware and no threat-intelligence data.
The only built-in signature is the harmless EICAR antivirus test string.

## Natives

 `sha256(buf)` and `hmac_sha256(key, msg)` return lower-case hex `Text`; `hex_encode/base64_encode(buf)` return `Text`, `hex_decode/base64_decode(text)`
  return a `NetworkBuffer` (malformed input is a runtime error); `buf_find(buf, needle, from)` returns the index or -1, `buf_find_wild(buf, "41 ?? 44", from)` the same with `??` matching any byte; `buf_entropy(buf)` returns a `Float` from 0 to 8, `buf_longest_run(buf, byte)` the longest run of one byte value
  bits per byte; `file_read_bytes(path)` reads a file as a `NetworkBuffer` (needs `--allow-fs`, same confinement as `file_read`, at most 1 MiB).

## More protocols in `std.net`

 VLAN: `net.vlan_tagged vlan_id vlan_priority vlan_inner_type vlan_payload` and `net.make_vlan(dst, src, id, priority, inner_type, payload)`.
  ARP over Ethernet/IPv4 (on the ARP packet): `net.arp_valid arp_operation arp_sender_mac arp_sender_ip arp_target_mac arp_target_ip arp_is_gratuitous` and
  `net.make_arp(op, sender_mac, sender_ip, target_mac, target_ip)`. ICMPv4 (on the ICMP message): `net.icmp_type icmp_code icmp_echo_id icmp_echo_seq icmp_payload
  icmp_checksum_compute icmp_checksum_ok` and `net.make_icmp_echo(type, id, seq, payload)`.

## The library `import std.sec`

 Defensive helpers: `sec.scan(&buf)` returns findings (lines starting `high:`, `medium:` or `low:`) for the EICAR test string, Windows PE and
  little-endian ELF structure problems (bad header offset, writable+executable sections, very high entropy sections), NOP sleds, `/bin/sh`/`cmd.exe` text and high entropy; `sec.verdict(&findings)`;
  `sec.file_kind pe_findings is_eicar find_hex longest_run entropy`; indicator lists `sec.load_list(path)`, `sec.in_list(value, &list)`, `sec.hash_file(path)`;
  network checks `sec.port_scan distinct_count syn_flood arp_conflict icmp_tunnel_suspect`; logs `sec.repeat_offenders(&lines, needle, marker, threshold)`. These are indicators,
  not proof: signatures miss new threats, heuristics raise false alarms, and "no indicators found" never means "safe".

## Packet captures (`import std.pcap`)

Classic libpcap files (little/big endian, microsecond/nanosecond): `pcap.valid(&f) linktype snaplen count truncated`, `pcap.offsets(&f)` (one offset per complete record),
`pcap.time_at(&f, off)`, `pcap.orig_length_at(&f, off)`, `pcap.packet_at(&f, off)` and the writer `pcap.make_header()` / `pcap.append(file, seconds, frame)`.
Minimal pcapng reading: `pcap.ng_valid(&f)`, `pcap.ng_offsets(&f)` (Enhanced Packet Blocks of the first section, either byte order), `pcap.ng_count(&f)`, `pcap.ng_packet_at(&f, off)`; no pcapng timestamps, interface metadata or writer. No flow reassembly, whole-second timestamps only; a file read with `file_read_bytes` is limited to 1 MiB.

## Detection rules (`import std.rules`)

One rule per line: `severity | name | kind | pattern`. Severity is `high`, `medium` or `low`; kind is `text` (the pattern occurs as text), `hex` (it occurs as bytes; `??` matches any byte and spaces are ignored, e.g. `41 ?? ?? 44`) or
`at` (`OFFSET: hex`, e.g. `0: 4d5a`: the bytes must start exactly at that offset; `??` allowed) or
`sha256` (the SHA-256 of the whole data equals it) or `group` (a condition over other rules: `2 of a, b, c`, `all of a, b`, `any of a, b`, `none of a, b`; no group of groups). `rules.match(&rs, &data)` returns `severity: name` findings (feed them to `sec.verdict`), `rules.check(&rs)` lists
every unusable rule, and `rules.load(path)` reads a rule file with `--allow-fs`. Blank lines and lines starting with `#` are comments. This is not YARA: only whole-byte `??` wildcards and simple `N of` groups, no regular expressions, no `not` operator (use `none of`) and offsets only through the `at` kind; invalid rules never match.

## Cookbook

- Hash a file: `let h = sec.hash_file("a.bin")` (needs `--allow-fs`), compare with a known list via `sec.in_list`.
- Scan a file and judge it: `let f = sec.scan(&data)`, then `sec.verdict(&f)`.
- Rules plus scan together: `rules.match(&rs, &data)` and `sec.scan(&data)` both return `severity: text` lines.
- Network capture review: loop `pcap.offsets(&cap)`, take `pcap.packet_at`, then `sec.port_scan`, `sec.syn_flood`, `sec.dns_suspect` or `sec.beaconing`.
- Every function is defensive analysis of data you supply; nothing here sends traffic or runs the data.

## Scan planning and nmap output (`import std.scan`)

nmap-style helpers that **send no packets and open no sockets**: `scan.hosts(&target)` (an address or a CIDR block from /22 to /32), `scan.ports(&spec)` / `scan.spec_ok(&spec)` (`22,80,8000-8100`, at most 10000 ports),
`scan.top_ports()`, `scan.ip_to_int` / `scan.int_to_ip`, `scan.service(port)`, `scan.risk(port)` and `scan.risk_note(port)`, `scan.open_ports(&text)` and `scan.exposure(&text)` for `nmap -oG` output,
`scan.diff(&before, &after)` (opened/closed ports between two scans) and `scan.banner_product` / `scan.banner_findings` for service banners.
The service table has about 30 TCP ports; banner checks cover only SSH protocol 1, vsFTPd 2.3.4 and version-announcing web servers. It does not scan: run nmap yourself on systems you are allowed to test.

## Examples

`examples/security_scan_plan.yom`, `examples/security_dns.yom`, `examples/security_strings.yom`, `examples/security_rules.yom`, `examples/security_pcap.yom`, `examples/security_scan.yom`, `security_network.yom`, `security_files.yom` (needs `--allow-fs`) and `security_logs.yom`.

## Honest limits

Simple indicators only. Signatures miss new threats; entropy and section heuristics raise false alarms (compressed archives look random). PE and ELF checks cover the
section/program-header tables only (no imports, symbols or Mach-O). No ICMPv6, IPv6 extension headers or active scanning yet.
