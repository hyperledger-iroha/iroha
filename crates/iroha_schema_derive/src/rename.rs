use core::mem;

use darling::FromMeta;

#[derive(Debug, Clone, Copy)]
pub enum RenameRule {
    Lowercase,
    Uppercase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
    CamelCase,
    PascalCase,
}

impl RenameRule {
    #[must_use]
    pub fn apply(self, ident: &str) -> String {
        match self {
            Self::Lowercase => ident.to_ascii_lowercase(),
            Self::Uppercase => ident.to_ascii_uppercase(),
            Self::SnakeCase => join_with(words(ident), '_', |word| word),
            Self::ScreamingSnakeCase => join_with(words(ident), '_', ascii_uppercase),
            Self::KebabCase => join_with(words(ident), '-', |word| word),
            Self::ScreamingKebabCase => join_with(words(ident), '-', ascii_uppercase),
            Self::CamelCase => camel_case(words(ident)),
            Self::PascalCase => pascal_case(words(ident)),
        }
    }
}

impl FromMeta for RenameRule {
    fn from_string(value: &str) -> darling::Result<Self> {
        match value {
            "lowercase" => Ok(Self::Lowercase),
            "UPPERCASE" => Ok(Self::Uppercase),
            "snake_case" => Ok(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnakeCase),
            "kebab-case" => Ok(Self::KebabCase),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebabCase),
            "camelCase" => Ok(Self::CamelCase),
            "PascalCase" => Ok(Self::PascalCase),
            other => Err(darling::Error::unknown_value(other)),
        }
    }
}

fn camel_case(words: Vec<String>) -> String {
    let mut iter = words.into_iter();
    let mut result = iter.next().unwrap_or_default();
    for word in iter {
        result.push_str(&capitalize(&word));
    }
    result
}

fn pascal_case(words: Vec<String>) -> String {
    words.into_iter().map(|word| capitalize(&word)).collect()
}

fn join_with<F>(words: Vec<String>, separator: char, map: F) -> String
where
    F: FnMut(String) -> String,
{
    let mut iter = words.into_iter().map(map);
    let mut result = iter.next().unwrap_or_default();
    for word in iter {
        result.push(separator);
        result.push_str(&word);
    }
    result
}

fn ascii_uppercase(mut word: String) -> String {
    word.make_ascii_uppercase();
    word
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut result = String::new();
    result.extend(first.to_uppercase());
    result.push_str(chars.as_str());
    result
}

fn words(ident: &str) -> Vec<String> {
    let mut chars = ident.chars().peekable();
    let mut current = String::new();
    let mut result = Vec::new();
    let mut prev_is_lower = false;
    let mut prev_is_upper = false;
    let mut prev_is_digit = false;

    while let Some(ch) = chars.next() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                result.push(mem::take(&mut current));
            }
            prev_is_lower = false;
            prev_is_upper = false;
            prev_is_digit = false;
            continue;
        }

        let is_upper = ch.is_uppercase();
        let is_lower = ch.is_lowercase();
        let is_digit = ch.is_ascii_digit();
        let starts_new_word = is_upper
            && !current.is_empty()
            && (prev_is_lower
                || prev_is_digit
                || (prev_is_upper && chars.peek().is_some_and(|next| next.is_lowercase())));
        if starts_new_word {
            result.push(mem::take(&mut current));
        }

        if is_upper {
            current.extend(ch.to_lowercase());
        } else {
            current.push(ch);
        }

        prev_is_lower = is_lower;
        prev_is_upper = is_upper;
        prev_is_digit = is_digit;
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}
