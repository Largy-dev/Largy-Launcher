//! Turns the HTML some providers use for changelogs into plain text the UI
//! can show as-is (never injected as markup).

/// Block-level tags end a line; `<li>` becomes a bullet; every other tag is
/// dropped. Common entities are decoded, runs of blank lines collapsed.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(len) = rest[start..].find('>') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + len].trim().to_ascii_lowercase();
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        match name.as_str() {
            "br" | "p" | "div" | "ul" | "ol" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "hr" => out.push('\n'),
            "li" if !tag.starts_with('/') => out.push_str("\n• "),
            _ => {}
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);

    let decoded = decode_entities(&out);
    let mut lines: Vec<&str> = Vec::new();
    for line in decoded.lines().map(str::trim_end) {
        if line.trim().is_empty() && lines.last().is_none_or(|l| l.trim().is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let decoded = tail.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &tail[1..end];
            let ch = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_structure_and_drops_tags() {
        let html = "<h2>v1.2</h2><p>Fixed <b>crash</b> &amp; lag</p><ul><li>Added X</li><li>Removed Y</li></ul>";
        assert_eq!(html_to_text(html), "v1.2\n\nFixed crash & lag\n\n• Added X\n• Removed Y");
    }

    #[test]
    fn decodes_numeric_entities_and_leaves_stray_ampersands() {
        assert_eq!(html_to_text("caf&#233; &#x2764; R&D"), "café ❤ R&D");
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(html_to_text("- Updated mods\n- Fixed quests"), "- Updated mods\n- Fixed quests");
    }
}
