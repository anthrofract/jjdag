use regex::Regex;

pub fn strip_ansi(pretty_str: &str) -> String {
    let ansi_regex = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    ansi_regex.replace_all(pretty_str, "").to_string()
}

pub fn strip_non_style_ansi(str: &str) -> String {
    let non_style_ansi_regex =
        Regex::new(r"\x1b(\[[0-9;?]*[ -/]*([@-l]|[n-~])|\].*?(\x07|\x1b\\)|P.*?\x1b\\)").unwrap();
    non_style_ansi_regex.replace_all(str, "").to_string()
}
