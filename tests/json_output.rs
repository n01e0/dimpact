#![allow(dead_code)]

pub fn parse_payload(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("valid json output")
}

pub fn parse_payload_slice(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("valid json output")
}
