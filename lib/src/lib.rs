use lab_core::Record;

pub const LIMIT: u32 = lab_macros::double!(21);

pub const BUILD: &str = env!("LAB_LIB_BUILD");

pub fn describe(record: &Record) -> String {
    format!("{}:{}:{}", BUILD, record.id, record.name)
}

pub fn digest(record: &Record) -> String {
    hex::encode(record.to_json())
}

pub fn failure() -> Box<dyn std::error::Error> {
    Box::new(hex::FromHexError::OddLength)
}

pub fn within_limit(record: &Record) -> bool {
    record.id < LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_doubles_at_compile_time() {
        assert_eq!(LIMIT, 42);
    }

    #[test]
    fn describe_names_the_build_script_value() {
        assert_eq!(describe(&Record::new(1, "one")), "lib:1:one");
    }

    #[test]
    fn digest_is_hex_of_the_json() {
        let record = Record::new(1, "x");
        assert_eq!(digest(&record), hex::encode(record.to_json()));
        assert!(within_limit(&record));
    }
}
// lab/specimen-fails-alone-1794576678
