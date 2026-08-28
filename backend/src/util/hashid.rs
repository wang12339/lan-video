use harsh::Harsh;
use std::sync::OnceLock;

#[allow(clippy::expect_used)]
fn hasher() -> &'static Harsh {
    static HASHER: OnceLock<Harsh> = OnceLock::new();
    HASHER.get_or_init(|| {
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

#[inline]
pub fn encode_id(id: i64) -> String {
    hasher().encode(&[id as u64])
}

pub fn decode_id(hash: &str) -> Option<i64> {
    match hasher().decode(hash) {
        Ok(v) if !v.is_empty() => {
            let id = i64::try_from(v[0]).ok()?;
            if hasher().encode(&[v[0]]) == hash {
                Some(id)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[inline]
pub fn decode_id_or_numeric(s: &str) -> Option<i64> {
    decode_id(s).or_else(|| s.parse::<i64>().ok())
}
