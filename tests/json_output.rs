#![allow(dead_code)]

pub fn parse_envelope(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("valid json output")
}

pub fn parse_payload(text: &str) -> serde_json::Value {
    let value = parse_envelope(text);
    value.get("data").cloned().unwrap_or(value)
}

pub fn parse_payload_slice(bytes: &[u8]) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(bytes).expect("valid json output");
    value.get("data").cloned().unwrap_or(value)
}

pub fn schema_id(text: &str) -> Option<String> {
    parse_envelope(text)
        .pointer("/_schema/id")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

pub fn schema_path(text: &str) -> Option<String> {
    parse_envelope(text)
        .get("json_schema")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}
