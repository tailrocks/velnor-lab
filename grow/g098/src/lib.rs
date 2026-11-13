pub const ID: u64 = 98;

pub fn mix(value: u64) -> u64 {
    let base = value.wrapping_mul(ID + 1);
    base ^ (base >> 3)
}

pub fn fold(values: &[u64]) -> u64 {
    values.iter().fold(ID, |acc, value| mix(acc ^ value))
}

pub fn label(value: u64) -> String {
    format!("g98:{}", mix(value))
}

pub fn series(count: u64) -> Vec<u64> {
    (0..count).map(mix).collect()
}

pub fn total(value: u64) -> u64 {
    mix(value)
        .wrapping_add(grow_097::total(value))
        .wrapping_add(grow_000::total(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_is_deterministic() {
        assert_eq!(mix(3), mix(3));
    }

    #[test]
    fn fold_depends_on_the_values() {
        assert_ne!(fold(&[1]), fold(&[2]));
    }

    #[test]
    fn series_has_the_requested_length() {
        assert_eq!(series(4).len(), 4);
    }

    #[test]
    fn total_and_label_are_stable() {
        assert_eq!(total(5), total(5));
        assert!(label(1).starts_with("g98:"));
    }
}
