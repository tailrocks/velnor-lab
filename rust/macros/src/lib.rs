use proc_macro::TokenStream;

fn doubled_literal(text: &str) -> Option<String> {
    let value: u32 = text.trim().parse().ok()?;
    Some(format!("{}u32", value.checked_mul(2)?))
}

#[proc_macro]
pub fn double(input: TokenStream) -> TokenStream {
    let source = doubled_literal(&input.to_string())
        .unwrap_or_else(|| "compile_error!(\"double! takes one u32 literal\")".to_owned());
    source.parse().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::doubled_literal;

    #[test]
    fn doubles_a_literal() {
        assert_eq!(doubled_literal(" 21 "), Some("42u32".to_owned()));
    }

    #[test]
    fn rejects_text_and_overflow() {
        assert_eq!(doubled_literal("x"), None);
        assert_eq!(doubled_literal("4294967295"), None);
    }
}
