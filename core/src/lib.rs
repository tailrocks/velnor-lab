use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub id: u32,
    pub name: String,
    pub tags: Vec<String>,
}

impl Record {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            tags: Vec::new(),
        }
    }

    pub fn tagged(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_owned());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }
}

pub fn checksum(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |acc, byte| acc.rotate_left(5) ^ u32::from(*byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_keeps_every_field() {
        let record = Record::new(7, "seven").tagged("a").tagged("b");
        assert_eq!(Record::from_json(&record.to_json()), Some(record));
    }

    #[test]
    fn invalid_json_gives_none() {
        assert_eq!(Record::from_json("{"), None);
    }

    #[test]
    fn checksum_depends_on_order() {
        assert_ne!(checksum(b"ab"), checksum(b"ba"));
        assert_eq!(checksum(b""), 0);
    }
}
// lab hub 1794559533-7
