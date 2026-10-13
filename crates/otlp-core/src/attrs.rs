//! Attribute values and lookups.

use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value::Value};
use serde_json::{Map, Value as Json};

/// Returns the value of `key`, if present.
pub fn find<'a>(attrs: &'a [KeyValue], key: &str) -> Option<&'a AnyValue> {
    attrs.iter().find(|kv| kv.key == key)?.value.as_ref()
}

/// Converts an attribute value to plain JSON (`{"stringValue": "x"}` becomes `"x"`).
pub fn to_json(v: &AnyValue) -> Json {
    match &v.value {
        None => Json::Null,
        Some(Value::StringValue(s)) => Json::String(s.clone()),
        Some(Value::BoolValue(b)) => Json::Bool(*b),
        Some(Value::IntValue(i)) => Json::from(*i),
        Some(Value::DoubleValue(d)) => serde_json::Number::from_f64(*d).map_or(Json::Null, Json::Number),
        Some(Value::ArrayValue(a)) => Json::Array(a.values.iter().map(to_json).collect()),
        Some(Value::KvlistValue(kv)) => Json::Object(to_json_map(&kv.values)),
        Some(Value::BytesValue(b)) => Json::String(hex(b)),
        Some(Value::StringValueStrindex(i)) => Json::from(*i),
    }
}

/// Converts a list of attributes to a JSON object of key to plain value.
pub fn to_json_map(attrs: &[KeyValue]) -> Map<String, Json> {
    attrs
        .iter()
        .map(|kv| (kv.key.clone(), kv.value.as_ref().map_or(Json::Null, to_json)))
        .collect()
}

/// Formats a value for display and comparison: strings as-is, everything else as JSON.
pub fn to_string(v: &AnyValue) -> String {
    match &v.value {
        Some(Value::StringValue(s)) => s.clone(),
        _ => to_json(v).to_string(),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
