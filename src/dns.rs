use std::io;

use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RecordType};

#[derive(Debug)]
#[allow(dead_code)]
pub enum DnsError {
    Timeout,
    ServFail,
    NxDomain,
    Refused,
    FormErr,
    NetworkError(io::Error),
    Other(String),
}

pub fn build_query_packet(name: &Name, record_type: RecordType) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(0);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(true);

    let mut query = Query::new();
    query.set_name(name.clone());
    query.set_query_type(record_type);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);

    msg.to_vec().expect("failed to serialize DNS query")
}

pub fn patch_query_id(buf: &mut [u8], id: u16) {
    let bytes = id.to_be_bytes();
    buf[0] = bytes[0];
    buf[1] = bytes[1];
}

pub fn parse_response(buf: &[u8]) -> Result<(), DnsError> {
    let msg = Message::from_vec(buf).map_err(|e| DnsError::Other(e.to_string()))?;
    match msg.response_code() {
        ResponseCode::NoError => Ok(()),
        ResponseCode::ServFail => Err(DnsError::ServFail),
        ResponseCode::NXDomain => Err(DnsError::NxDomain),
        ResponseCode::Refused => Err(DnsError::Refused),
        ResponseCode::FormErr => Err(DnsError::FormErr),
        other => Err(DnsError::Other(format!("{other}"))),
    }
}
