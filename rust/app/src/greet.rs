use lab_core::Record;

const GREETING: &str = include_str!("../../shared-data/greeting.txt");

pub fn line(record: &Record) -> String {
    format!("{} | {}", GREETING.trim(), lab_lib::describe(record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_starts_with_the_shared_greeting() {
        assert!(line(&Record::new(1, "a")).starts_with("hello from shared data | "));
    }

    #[test]
    fn line_names_the_record() {
        assert!(line(&Record::new(9, "nine")).ends_with("lib:9:nine"));
    }
}
