pub fn append_line(content: &str, line: &str) -> String {
    let mut out = content.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::append_line;

    #[test]
    fn adds_the_line_after_an_unterminated_file() {
        assert_eq!(append_line("a", "b"), "a\nb\n");
    }

    #[test]
    fn keeps_existing_lines_and_starts_empty_files() {
        assert_eq!(append_line("a\n", "b"), "a\nb\n");
        assert_eq!(append_line("", "b"), "b\n");
    }
}
// 1702 delta timing sample 1
// 1702 delta timing sample A
// 1702 delta timing sample B
// 1702 delta timing sample C
