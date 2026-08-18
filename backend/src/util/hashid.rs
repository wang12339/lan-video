use harsh::Harsh;
use std::sync::OnceLock;

fn hasher() -> &'static Harsh {
    static HASHER: OnceLock<Harsh> = OnceLock::new();
    HASHER.get_or_init(|| {
        // The OnceLock makes this run exactly once per process, so the
        // warning fires once at startup rather than on every hashid use.
        let salt = std::env::var("HASHID_SALT").unwrap_or_else(|_| {
            tracing::warn!(
                "HASHID_SALT not set, using built-in default salt. Production must set HASHID_SALT: the default is public in source and lets anyone decode hashids (e.g. video ID enumeration)."
            );
            "atmos-video-default-salt".to_string()
        });
        Harsh::builder()
            .salt(salt.as_bytes().to_vec())
            .length(8)
            .build()
            .expect("Harsh builder should succeed")
    })
}

pub fn encode_id(id: i64) -> String {
    hasher().encode(&[id as u64])
}

pub fn decode_id(hash: &str) -> Option<i64> {
    match hasher().decode(hash) {
        Ok(v) if !v.is_empty() => {
            // Reject values outside the i64 range instead of silently wrapping.
            let id = i64::try_from(v[0]).ok()?;
            // Only accept canonical encodings: a padded/ambiguous string that
            // merely decodes to a number must not shadow a legacy numeric ID
            // (decode_id_or_numeric would otherwise resolve it differently).
            if hasher().encode(&[v[0]]) == hash {
                Some(id)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Decode a hashid, falling back to plain numeric parse.
/// This allows both hashid and legacy numeric IDs in URLs during migration.
pub fn decode_id_or_numeric(s: &str) -> Option<i64> {
    decode_id(s).or_else(|| s.parse::<i64>().ok())
}
