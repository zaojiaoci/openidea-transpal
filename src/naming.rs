//! 标识符拆词与命名风格转换（camelCase / snake_case / kebab-case / SCREAMING）。

/// 把标识符拆成英文单词：处理 snake_case、kebab-case、camelCase、PascalCase、
/// SCREAMING_SNAKE 与数字边界（v2User → v2, User）。
pub fn split_identifier(s: &str) -> Vec<String> {
    let normalized: String = s
        .chars()
        .map(|c| if c == '_' || c == '-' || c == '.' { ' ' } else { c })
        .collect();

    let mut words = Vec::new();
    for chunk in normalized.split_whitespace() {
        camel_words(chunk, &mut words);
    }
    words
}

fn camel_words(chunk: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = chunk.chars().collect();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        let next = chars.get(i + 1).copied();
        let starts_new_word = c.is_uppercase()
            && !cur.is_empty()
            && (prev.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit())
                || next.is_some_and(|n| n.is_lowercase()));
        if starts_new_word && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if c.is_alphanumeric() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
}

pub fn camel_to_snake(s: &str) -> String {
    split_identifier(s)
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_all_common_styles() {
        assert_eq!(split_identifier("userAccountBalance"), ["user", "Account", "Balance"]);
        assert_eq!(split_identifier("user_account_balance"), ["user", "account", "balance"]);
        assert_eq!(split_identifier("USER_ACCOUNT"), ["USER", "ACCOUNT"]);
        assert_eq!(split_identifier("parse-HTTPResponse"), ["parse", "HTTP", "Response"]);
        assert_eq!(split_identifier("v2User"), ["v2", "User"]);
    }

    #[test]
    fn camel_to_snake_roundtrip() {
        assert_eq!(camel_to_snake("userAccountBalance"), "user_account_balance");
        assert_eq!(camel_to_snake("HTTPResponse"), "http_response");
    }
}
