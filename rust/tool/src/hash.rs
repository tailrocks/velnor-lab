use fnv::FnvHasher;
use std::hash::Hasher;

pub fn fingerprint(bytes: &[u8]) -> String {
    let mut hasher = FnvHasher::default();
    hasher.write(bytes);
    hex::encode(hasher.finish().to_be_bytes())
}

pub fn parse(text: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(hex::decode(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_sixteen_hex_digits() {
        assert_eq!(fingerprint(b"lab").len(), 16);
    }

    #[test]
    fn fingerprint_differs_for_different_input() {
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
    }

    #[test]
    fn parse_round_trips_and_rejects_odd_length() {
        assert_eq!(parse("6c6162").unwrap(), b"lab");
        assert!(parse("abc").is_err());
    }
}
