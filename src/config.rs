use std::net::SocketAddr;
use std::time::Duration;

use hickory_proto::rr::{Name, RecordType};

#[derive(Debug, Clone)]
pub enum Protocol {
    Udp,
    Tcp,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Tcp => write!(f, "TCP"),
        }
    }
}

impl std::str::FromStr for Protocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "udp" => Ok(Protocol::Udp),
            "tcp" => Ok(Protocol::Tcp),
            _ => Err(format!("unknown protocol: {s}")),
        }
    }
}

pub struct BenchConfig {
    pub nameserver: SocketAddr,
    pub host: Name,
    pub record_type: RecordType,
    pub protocol: Protocol,
    pub duration: Duration,
    pub timeout: Duration,
    pub workers: usize,
    pub warmup: Duration,
    pub json: bool,
}
