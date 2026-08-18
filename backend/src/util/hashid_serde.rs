//! Serde helpers that transparently encode/decode `i64` IDs as HashID strings.
//!
//! Use `#[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]`
//! on response structs to emit HashID strings instead of raw numbers.
//!
//! Use `#[serde(deserialize_with = "crate::util::hashid_serde::deserialize_id")]`
//! on request structs to accept HashID strings (falling back to plain numbers).

use serde::{Deserialize, Deserializer, Serializer};

/// Serialize an `i64` as a HashID string.
pub fn serialize_id<S: Serializer>(id: &i64, s: S) -> Result<S::Ok, S::Error> {
    let hash = super::hashid::encode_id(*id);
    s.serialize_str(&hash)
}

/// Deserialize an `i64` from a HashID string or a plain JSON number.
pub fn deserialize_id<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let val = serde_json::Value::deserialize(d)?;
    match &val {
        serde_json::Value::String(s) => super::hashid::decode_id_or_numeric(s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid hashid: {}", s))),
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("number out of i64 range")),
        _ => Err(serde::de::Error::custom("expected string or number for id")),
    }
}

/// Serialize an `Option<i64>` as a HashID string or null.
pub fn serialize_option_id<S: Serializer>(id: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
    match id {
        Some(v) => serialize_id(v, s),
        None => s.serialize_none(),
    }
}

/// Deserialize an `Option<i64>` from a HashID string, number, or null.
pub fn deserialize_option_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let val = serde_json::Value::deserialize(d)?;
    match val {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => super::hashid::decode_id_or_numeric(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid hashid: {}", s))),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("number out of i64 range")),
        _ => Err(serde::de::Error::custom(
            "expected string, number, or null for id",
        )),
    }
}
