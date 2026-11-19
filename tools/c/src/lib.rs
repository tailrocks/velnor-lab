use fnv::FnvHasher;
use serde::Serialize;
use std::hash::Hasher;

#[derive(Serialize)]
pub struct Digest {
    pub hex: String,
}

/// FNV-1a digest of `input`, hex encoded (tool c).
pub fn digest(input: &str) -> Digest {
    let mut hasher = FnvHasher::default();
    hasher.write(input.as_bytes());
    Digest {
        hex: hex::encode(hasher.finish().to_be_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::digest;

    #[test]
    fn digest_is_stable() {
        assert_eq!(digest("").hex, "cbf29ce484222325");
    }
}
