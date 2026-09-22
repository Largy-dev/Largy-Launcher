//! A single `{key}`-replacement loop, parameterized by delimiter, shared by
//! the two places in the codebase that substitute placeholders into strings:
//! Mojang's version-json arguments (`${key}`) and Forge/NeoForge's install
//! profile processor args (`{key}`).

use std::collections::HashMap;

fn substitute(text: &str, placeholders: &HashMap<String, String>, open: &str, close: &str) -> String {
    let mut result = text.to_string();
    for (key, value) in placeholders {
        result = result.replace(&format!("{open}{key}{close}"), value);
    }
    result
}

/// Replaces every `${key}` occurrence, as used by Mojang's version-json
/// `arguments` (jvm/game).
pub fn substitute_dollar_braces(text: &str, placeholders: &HashMap<String, String>) -> String {
    substitute(text, placeholders, "${", "}")
}

/// Replaces every `{key}` occurrence, as used by Forge/NeoForge's install
/// profile processor arguments.
pub fn substitute_braces(text: &str, placeholders: &HashMap<String, String>) -> String {
    substitute(text, placeholders, "{", "}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_dollar_braces_replaces_all_occurrences() {
        let mut placeholders = HashMap::new();
        placeholders.insert("auth_player_name".to_string(), "Steve".to_string());
        let result = substitute_dollar_braces("--username ${auth_player_name}", &placeholders);
        assert_eq!(result, "--username Steve");
    }

    #[test]
    fn substitute_braces_replaces_all_occurrences() {
        let mut placeholders = HashMap::new();
        placeholders.insert("SIDE".to_string(), "client".to_string());
        placeholders.insert("MINECRAFT_VERSION".to_string(), "1.20.1".to_string());
        let result = substitute_braces("--side {SIDE} --version {MINECRAFT_VERSION} {SIDE}", &placeholders);
        assert_eq!(result, "--side client --version 1.20.1 client");
    }

    #[test]
    fn substitute_braces_leaves_unknown_tokens_untouched() {
        let placeholders = HashMap::new();
        let result = substitute_braces("{UNKNOWN}", &placeholders);
        assert_eq!(result, "{UNKNOWN}");
    }

    #[test]
    fn substitute_dollar_braces_leaves_unknown_tokens_untouched() {
        let placeholders = HashMap::new();
        let result = substitute_dollar_braces("${UNKNOWN}", &placeholders);
        assert_eq!(result, "${UNKNOWN}");
    }
}
