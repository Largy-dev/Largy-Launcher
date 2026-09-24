//! Server descriptions come either as a legacy `§`-coded string or as a JSON
//! text component. Both are normalised to `§`-coded text (with `§#rrggbb`
//! for hex colours) that the UI turns into coloured spans.

use serde_json::Value;

fn color_code(name: &str) -> Option<String> {
    let code = match name {
        "black" => '0',
        "dark_blue" => '1',
        "dark_green" => '2',
        "dark_aqua" => '3',
        "dark_red" => '4',
        "dark_purple" => '5',
        "gold" => '6',
        "gray" => '7',
        "dark_gray" => '8',
        "blue" => '9',
        "green" => 'a',
        "aqua" => 'b',
        "red" => 'c',
        "light_purple" => 'd',
        "yellow" => 'e',
        "white" => 'f',
        hex if hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|c| c.is_ascii_hexdigit()) => {
            return Some(format!("§{}", hex.to_ascii_lowercase()));
        }
        _ => return None,
    };
    Some(format!("§{code}"))
}

#[derive(Clone, Default)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

impl Style {
    fn inherit(&self, component: &serde_json::Map<String, Value>) -> Style {
        let flag = |key: &str, parent: bool| component.get(key).and_then(Value::as_bool).unwrap_or(parent);
        Style {
            color: component.get("color").and_then(Value::as_str).and_then(color_code).or_else(|| self.color.clone()),
            bold: flag("bold", self.bold),
            italic: flag("italic", self.italic),
            underlined: flag("underlined", self.underlined),
            strikethrough: flag("strikethrough", self.strikethrough),
            obfuscated: flag("obfuscated", self.obfuscated),
        }
    }

    fn codes(&self) -> String {
        let mut out = String::from("§r");
        if let Some(color) = &self.color {
            out.push_str(color);
        }
        for (on, code) in [
            (self.bold, 'l'),
            (self.italic, 'o'),
            (self.underlined, 'n'),
            (self.strikethrough, 'm'),
            (self.obfuscated, 'k'),
        ] {
            if on {
                out.push('§');
                out.push(code);
            }
        }
        out
    }
}

fn walk(value: &Value, style: &Style, out: &mut String) {
    match value {
        Value::String(text) => {
            out.push_str(&style.codes());
            out.push_str(text);
        }
        Value::Array(parts) => parts.iter().for_each(|part| walk(part, style, out)),
        Value::Object(component) => {
            let style = style.inherit(component);
            if let Some(text) = component.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                out.push_str(&style.codes());
                out.push_str(text);
            }
            if let Some(extra) = component.get("extra") {
                walk(extra, &style, out);
            }
        }
        _ => {}
    }
}

/// `description` from a status response as `§`-coded text.
pub fn to_legacy(description: &Value) -> String {
    let mut out = String::new();
    walk(description, &Style::default(), &mut out);
    // Plain descriptions need no leading reset.
    match out.strip_prefix("§r") {
        Some(rest) if !rest.contains('§') => rest.to_string(),
        _ => out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_and_legacy_strings_pass_through() {
        assert_eq!(to_legacy(&json!("A Minecraft Server")), "A Minecraft Server");
        assert_eq!(to_legacy(&json!("§aGreen §lbold")), "§r§aGreen §lbold");
    }

    #[test]
    fn components_inherit_their_parent_style() {
        let description = json!({
            "text": "",
            "extra": [
                { "text": "Hy", "color": "gold", "bold": true },
                { "text": "pixel", "color": "#FF00aa", "extra": [{ "text": "!" }] }
            ]
        });
        assert_eq!(to_legacy(&description), "§r§6§lHy§r§#ff00aapixel§r§#ff00aa!");
    }

    #[test]
    fn unknown_colors_are_ignored() {
        assert_eq!(to_legacy(&json!({ "text": "x", "color": "rainbow" })), "x");
    }
}
