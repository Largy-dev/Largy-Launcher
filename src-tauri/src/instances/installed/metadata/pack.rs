//! Resource packs describe themselves in `pack.mcmeta`, whose description
//! is a Minecraft text component with `§` formatting codes.

use serde_json::Value as Json;

use super::{non_empty, set_if_none, LocalMeta};

/// Plain text of a JSON text component: a string, `{"text": .., "extra": [..]}` or an array of them.
fn component_text(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        Json::Array(items) => items.iter().map(component_text).collect(),
        Json::Object(o) => {
            let mut out = o.get("text").and_then(Json::as_str).unwrap_or_default().to_string();
            if let Some(extra) = o.get("extra") {
                out.push_str(&component_text(extra));
            }
            out
        }
        _ => String::new(),
    }
}

/// Drops `§x` formatting codes.
pub fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

pub fn apply_pack_mcmeta(meta: &mut LocalMeta, text: &str) {
    let Ok(json) = serde_json::from_str::<Json>(text) else { return };
    if let Some(description) = json.get("pack").and_then(|p| p.get("description")) {
        let text = strip_formatting(&component_text(description));
        set_if_none(&mut meta.description, non_empty(Some(&text)));
    }
}
