// Tests for playback history request validation and deserialization
// Run with: cargo test --test test_playback_validation -- --nocapture

use atmos_video_backend::models::playback::PlaybackHistoryRequest;

// ── Deserialization tests ──

#[test]
fn test_playback_request_deserialize_valid() {
    let json = r#"{"video_id": 42, "position_ms": 5000, "duration_ms": 120000}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, 42);
    assert_eq!(req.position_ms, 5000);
    assert_eq!(req.duration_ms, 120000);
}

#[test]
fn test_playback_request_deserialize_zero_values() {
    let json = r#"{"video_id": 1, "position_ms": 0, "duration_ms": 0}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, 1);
    assert_eq!(req.position_ms, 0);
    assert_eq!(req.duration_ms, 0);
}

#[test]
fn test_playback_request_deserialize_negative_values() {
    // Deserialization allows negative values — validation happens in the handler
    let json = r#"{"video_id": 1, "position_ms": -100, "duration_ms": -50}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.position_ms, -100);
    assert_eq!(req.duration_ms, -50);
}

#[test]
fn test_playback_request_deserialize_very_large_values() {
    let json = r#"{"video_id": 1, "position_ms": 999999999999, "duration_ms": 999999999999}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.position_ms, 999999999999);
    assert_eq!(req.duration_ms, 999999999999);
}

#[test]
fn test_playback_request_deserialize_missing_fields() {
    // Missing required fields should fail deserialization
    let json = r#"{"video_id": 1}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(
        result.is_err(),
        "missing position_ms and duration_ms should fail"
    );

    let json = r#"{"video_id": 1, "position_ms": 0}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(result.is_err(), "missing duration_ms should fail");

    let json = r#"{"position_ms": 0, "duration_ms": 0}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(result.is_err(), "missing video_id should fail");
}

#[test]
fn test_playback_request_deserialize_extra_fields() {
    // Extra fields should be ignored (serde default behavior)
    let json =
        r#"{"video_id": 1, "position_ms": 100, "duration_ms": 200, "extra_field": "ignored"}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, 1);
    assert_eq!(req.position_ms, 100);
    assert_eq!(req.duration_ms, 200);
}

#[test]
fn test_playback_request_deserialize_wrong_types() {
    // String values for integer fields should fail
    let json = r#"{"video_id": "not_a_number", "position_ms": 0, "duration_ms": 0}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(
        result.is_err(),
        "string video_id should fail deserialization"
    );
}

#[test]
fn test_playback_request_deserialize_null_values() {
    // Null for non-optional fields should fail
    let json = r#"{"video_id": null, "position_ms": 0, "duration_ms": 0}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(result.is_err(), "null video_id should fail deserialization");
}

#[test]
fn test_playback_request_deserialize_empty_json() {
    let json = r#"{}"#;
    let result = serde_json::from_str::<PlaybackHistoryRequest>(json);
    assert!(result.is_err(), "empty JSON should fail deserialization");
}

#[test]
fn test_playback_request_deserialize_empty_string() {
    let result = serde_json::from_str::<PlaybackHistoryRequest>("");
    assert!(result.is_err(), "empty string should fail deserialization");
}

#[test]
fn test_playback_request_deserialize_negative_video_id() {
    // Deserialization accepts any i64 — handler/session logic must not
    // assume a positive video_id arrives.
    let json = r#"{"video_id": -7, "position_ms": 100, "duration_ms": 1000}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, -7);
    assert_eq!(req.position_ms, 100);
}

#[test]
fn test_playback_request_deserialize_zero_video_id() {
    let json = r#"{"video_id": 0, "position_ms": 100, "duration_ms": 1000}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, 0);
}

#[test]
fn test_playback_request_deserialize_i64_extremes() {
    let json = r#"{"video_id": 9223372036854775807, "position_ms": -9223372036854775808, "duration_ms": 9223372036854775807}"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, i64::MAX);
    assert_eq!(req.position_ms, i64::MIN);
    assert_eq!(req.duration_ms, i64::MAX);
}

#[test]
fn test_playback_request_deserialize_overflow_beyond_i64() {
    // One past i64::MAX must fail deserialization
    let json = r#"{"video_id": 9223372036854775808, "position_ms": 0, "duration_ms": 0}"#;
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>(json).is_err(),
        "video_id beyond i64::MAX should fail"
    );
}

#[test]
fn test_playback_request_deserialize_float_values() {
    let json = r#"{"video_id": 1, "position_ms": 1.5, "duration_ms": 1000}"#;
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>(json).is_err(),
        "float position_ms should fail deserialization"
    );
}

#[test]
fn test_playback_request_deserialize_trailing_comma() {
    let json = r#"{"video_id": 1, "position_ms": 0, "duration_ms": 0,}"#;
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>(json).is_err(),
        "trailing comma should fail deserialization"
    );
}

#[test]
fn test_playback_request_deserialize_duplicate_keys() {
    let json = r#"{"video_id": 1, "video_id": 2, "position_ms": 0, "duration_ms": 0}"#;
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>(json).is_err(),
        "duplicate keys should fail deserialization"
    );
}

#[test]
fn test_playback_request_deserialize_array_positional() {
    // serde's derived impl accepts a JSON array and maps fields positionally
    // (video_id, position_ms, duration_ms) — a permissive but consistent
    // behavior worth pinning down.
    let json = r#"[7, 100, 1000]"#;
    let req: PlaybackHistoryRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.video_id, 7);
    assert_eq!(req.position_ms, 100);
    assert_eq!(req.duration_ms, 1000);
}

#[test]
fn test_playback_request_deserialize_array_wrong_length() {
    // A sequence that doesn't line up with the 3 fields must fail
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>("[1, 0]").is_err(),
        "2-element array should fail"
    );
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>("[1, 0, 0, 4]").is_err(),
        "4-element array should fail"
    );
}

#[test]
fn test_playback_request_deserialize_nested_object_for_field() {
    // A JSON object where an i64 is expected must fail
    let json = r#"{"video_id": {"id": 1}, "position_ms": 0, "duration_ms": 0}"#;
    assert!(serde_json::from_str::<PlaybackHistoryRequest>(json).is_err());
}

#[test]
fn test_playback_request_deserialize_whitespace_json() {
    assert!(
        serde_json::from_str::<PlaybackHistoryRequest>("   ").is_err(),
        "whitespace-only input should fail"
    );
}

// ── Validation logic tests (matching handler validation rules) ──

/// Simulates the validation logic from update_playback_history handler
fn validate_playback_request(req: &PlaybackHistoryRequest) -> Result<(), &'static str> {
    if req.position_ms < 0 || req.duration_ms < 0 {
        return Err("position and duration must not be negative");
    }
    if req.duration_ms > 86_400_000 * 7 {
        return Err("duration exceeds maximum (7 days)");
    }
    if req.position_ms > req.duration_ms + 1000 {
        return Err("position exceeds duration + 1s tolerance");
    }
    Ok(())
}

#[test]
fn test_validation_negative_position() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: -100,
        duration_ms: 1000,
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_negative_duration() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 100,
        duration_ms: -1000,
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_both_negative() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: -1,
        duration_ms: -1,
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_duration_too_large() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 0,
        duration_ms: 86_400_000 * 7 + 1, // 7 days + 1ms
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_duration_exactly_7_days() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 0,
        duration_ms: 86_400_000 * 7, // exactly 7 days
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_position_exceeds_duration() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 2000,
        duration_ms: 1000,
    };
    // position_ms (2000) > duration_ms (1000) + 1000 tolerance = 2000
    // 2000 > 2000 is false, so this should be OK
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_position_exceeds_duration_plus_tolerance() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 2001,
        duration_ms: 1000,
    };
    // position_ms (2001) > duration_ms (1000) + 1000 tolerance = 2000
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_position_equals_duration() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 5000,
        duration_ms: 5000,
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_zero_position_zero_duration() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 0,
        duration_ms: 0,
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_normal_playback() {
    let req = PlaybackHistoryRequest {
        video_id: 42,
        position_ms: 30000,
        duration_ms: 3600000, // 1 hour
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_position_near_end() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 3599000,
        duration_ms: 3600000,
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_video_id_not_validated() {
    // NOTE: the handler only validates position/duration. A zero or negative
    // video_id passes validation and only fails later at the DB lookup.
    for video_id in [-1, 0, i64::MIN] {
        let req = PlaybackHistoryRequest {
            video_id,
            position_ms: 100,
            duration_ms: 1000,
        };
        assert!(
            validate_playback_request(&req).is_ok(),
            "video_id {} should pass position/duration validation",
            video_id
        );
    }
}

#[test]
fn test_validation_position_exactly_at_tolerance_boundary() {
    // position_ms == duration_ms + 1000 is allowed (inclusive bound)
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 1000,
        duration_ms: 0,
    };
    assert!(validate_playback_request(&req).is_ok());

    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 1001,
        duration_ms: 0,
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_position_at_seven_days_plus_tolerance() {
    let max = 86_400_000 * 7;
    // duration == 7 days exactly, position within +1s tolerance
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: max + 1000,
        duration_ms: max,
    };
    assert!(validate_playback_request(&req).is_ok());

    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: max + 1001,
        duration_ms: max,
    };
    assert!(validate_playback_request(&req).is_err());
}

#[test]
fn test_validation_zero_position_with_max_duration() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: 0,
        duration_ms: 86_400_000 * 7,
    };
    assert!(validate_playback_request(&req).is_ok());
}

#[test]
fn test_validation_i64_max_values() {
    // i64::MAX duration fails the 7-day check *before* any arithmetic on
    // duration_ms + 1000, so no overflow is reachable in the handler.
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: i64::MAX,
        duration_ms: i64::MAX,
    };
    assert!(
        validate_playback_request(&req).is_err(),
        "i64::MAX duration must be rejected by the 7-day cap"
    );
}

#[test]
fn test_validation_duration_negative_with_max_position() {
    let req = PlaybackHistoryRequest {
        video_id: 1,
        position_ms: i64::MAX,
        duration_ms: -1,
    };
    assert!(
        validate_playback_request(&req).is_err(),
        "negative duration must fail before the position check"
    );
}

#[test]
fn test_validation_position_equals_duration_plus_tolerance_exact_max() {
    // Largest valid duration combined with the exact +1000 tolerance
    let max = 86_400_000 * 7;
    let req = PlaybackHistoryRequest {
        video_id: i64::MAX,
        position_ms: max + 1000,
        duration_ms: max,
    };
    assert!(validate_playback_request(&req).is_ok());
}
