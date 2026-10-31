//! Network types, sockets, protocol validation, and safe buffer abstractions for YOMNOVA.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener as StdTcpListener, TcpStream as StdTcpStream};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NetworkError {
    #[error("Invalid IPv4 address: '{0}'")]
    InvalidIpv4(String),
    #[error("Invalid IPv6 address: '{0}'")]
    InvalidIpv6(String),
    #[error("Invalid port '{0}': port must be between 1 and 65535")]
    InvalidPort(i64),
    #[error("Network I/O error: {0}")]
    IoError(String),
    #[error("Buffer index out of bounds: offset {offset}, length {length}, capacity {capacity}")]
    BufferOverflow {
        offset: usize,
        length: usize,
        capacity: usize,
    },
    #[error("Connection closed")]
    ConnectionClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Address([u8; 4]);

impl Ipv4Address {
    pub fn parse(s: &str) -> Result<Self, NetworkError> {
        match s.parse::<Ipv4Addr>() {
            Ok(addr) => Ok(Self(addr.octets())),
            Err(_) => Err(NetworkError::InvalidIpv4(s.to_string())),
        }
    }

    pub fn octets(&self) -> [u8; 4] {
        self.0
    }
}

impl std::fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(u16);

impl Port {
    pub fn new(num: i64) -> Result<Self, NetworkError> {
        if (1..=65535).contains(&num) {
            Ok(Self(num as u16))
        } else {
            Err(NetworkError::InvalidPort(num))
        }
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub ip: Ipv4Address,
    pub port: Port,
}

impl Endpoint {
    pub fn new(ip: Ipv4Address, port: Port) -> Self {
        Self { ip, port }
    }

    pub fn to_socket_addr(&self) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(self.ip.octets()),
            self.port.value(),
        ))
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkBuffer {
    data: Vec<u8>,
    capacity_limit: usize,
}

impl NetworkBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity_limit: capacity,
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Self {
        Self {
            data: bytes.to_vec(),
            capacity_limit: bytes.len() * 2,
        }
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<(), NetworkError> {
        if self.data.len() + bytes.len() > self.capacity_limit {
            return Err(NetworkError::BufferOverflow {
                offset: self.data.len(),
                length: bytes.len(),
                capacity: self.capacity_limit,
            });
        }
        self.data.extend_from_slice(bytes);
        Ok(())
    }

    pub fn slice(&self, start: usize, len: usize) -> Result<&[u8], NetworkError> {
        if start + len > self.data.len() {
            return Err(NetworkError::BufferOverflow {
                offset: start,
                length: len,
                capacity: self.data.len(),
            });
        }
        Ok(&self.data[start..start + len])
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

pub struct TcpClient {
    stream: StdTcpStream,
}

impl TcpClient {
    pub fn connect(endpoint: &Endpoint, timeout: Duration) -> Result<Self, NetworkError> {
        let addr = endpoint.to_socket_addr();
        let stream = StdTcpStream::connect_timeout(&addr, timeout)
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        Ok(Self { stream })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<usize, NetworkError> {
        self.stream
            .write_all(data)
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        self.stream
            .flush()
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        Ok(data.len())
    }

    pub fn receive(&mut self, max_bytes: usize) -> Result<Vec<u8>, NetworkError> {
        let mut buf = vec![0u8; max_bytes];
        let bytes_read = self
            .stream
            .read(&mut buf)
            .map_err(|e| NetworkError::IoError(e.to_string()))?;
        if bytes_read == 0 {
            return Err(NetworkError::ConnectionClosed);
        }
        buf.truncate(bytes_read);
        Ok(buf)
    }
}

pub struct TcpServer {
    listener: StdTcpListener,
}

impl TcpServer {
    pub fn bind(endpoint: &Endpoint) -> Result<Self, NetworkError> {
        let addr = endpoint.to_socket_addr();
        let listener = StdTcpListener::bind(addr).map_err(|e| NetworkError::IoError(e.to_string()))?;
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, NetworkError> {
        self.listener
            .local_addr()
            .map_err(|e| NetworkError::IoError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ipv4_and_port() {
        let ip = Ipv4Address::parse("127.0.0.1").unwrap();
        assert_eq!(ip.octets(), [127, 0, 0, 1]);

        let port = Port::new(8080).unwrap();
        assert_eq!(port.value(), 8080);

        let endpoint = Endpoint::new(ip, port);
        assert_eq!(endpoint.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn test_invalid_port() {
        assert!(Port::new(0).is_err());
        assert!(Port::new(70000).is_err());
    }

    #[test]
    fn test_network_buffer_bounds() {
        let mut buf = NetworkBuffer::with_capacity(10);
        assert!(buf.append(b"hello").is_ok());
        assert_eq!(buf.len(), 5);
        assert!(buf.append(b"world!").is_err()); // 5 + 6 > 10
    }
}
