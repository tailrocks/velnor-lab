pub fn quadruple(value: u32) -> u32 {
    alpha::double(alpha::double(value))
}

#[cfg(test)]
mod tests {
    use super::quadruple;

    #[test]
    fn quadruples() {
        assert_eq!(quadruple(2), 8);
    }
}
