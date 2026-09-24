//! Skins saved locally (`skins/<id>.png` + `skins/library.json`) so the
//! player can switch between them in one click.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{data_url, validate_skin, SkinVariant};
use crate::error::{AppError, AppResult};
use crate::util::fs::write_atomic;

const INDEX: &str = "library.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibrarySkin {
    pub id: String,
    pub name: String,
    pub variant: SkinVariant,
    pub added_at: i64,
}

/// A library entry with its texture, for the UI.
#[derive(Debug, Clone, Serialize)]
pub struct LibrarySkinView {
    #[serde(flatten)]
    pub skin: LibrarySkin,
    pub texture: String,
}

fn load(dir: &Path) -> Vec<LibrarySkin> {
    std::fs::read(dir.join(INDEX)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(dir: &Path, skins: &[LibrarySkin]) -> AppResult<()> {
    write_atomic(&dir.join(INDEX), serde_json::to_string_pretty(skins)?.as_bytes())?;
    Ok(())
}

fn png_path(dir: &Path, id: &str) -> AppResult<std::path::PathBuf> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(AppError::Other("skin inconnu".to_string()));
    }
    Ok(dir.join(format!("{id}.png")))
}

/// Newest first; entries whose image went missing are skipped.
pub fn list(dir: &Path) -> Vec<LibrarySkinView> {
    let mut skins = load(dir);
    skins.sort_by_key(|s| std::cmp::Reverse(s.added_at));
    skins
        .into_iter()
        .filter_map(|skin| {
            let bytes = std::fs::read(png_path(dir, &skin.id).ok()?).ok()?;
            Some(LibrarySkinView { texture: data_url(&bytes), skin })
        })
        .collect()
}

pub fn add(dir: &Path, name: &str, variant: SkinVariant, png: &[u8], now: i64) -> AppResult<LibrarySkin> {
    validate_skin(png)?;
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(40).collect();
    let skin = LibrarySkin {
        id: uuid::Uuid::new_v4().to_string(),
        name: if name.is_empty() { "Skin".to_string() } else { name },
        variant,
        added_at: now,
    };
    write_atomic(&png_path(dir, &skin.id)?, png)?;
    let mut skins = load(dir);
    skins.push(skin.clone());
    save(dir, &skins)?;
    Ok(skin)
}

pub fn get(dir: &Path, id: &str) -> AppResult<(LibrarySkin, Vec<u8>)> {
    let skin = load(dir).into_iter().find(|s| s.id == id).ok_or_else(|| AppError::Other("skin inconnu".to_string()))?;
    let bytes = std::fs::read(png_path(dir, id)?)?;
    Ok((skin, bytes))
}

pub fn remove(dir: &Path, id: &str) -> AppResult<()> {
    let path = png_path(dir, id)?;
    let mut skins = load(dir);
    skins.retain(|s| s.id != id);
    save(dir, &skins)?;
    let _ = std::fs::remove_file(path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skins::tests::fake_png;

    #[test]
    fn add_list_get_remove() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).is_empty());

        let a = add(dir.path(), " Knight ", SkinVariant::Classic, &fake_png(64, 64), 10).unwrap();
        let b = add(dir.path(), "", SkinVariant::Slim, &fake_png(64, 64), 20).unwrap();
        assert_eq!(a.name, "Knight");
        assert_eq!(b.name, "Skin");

        let listed = list(dir.path());
        assert_eq!(listed.iter().map(|s| s.skin.id.clone()).collect::<Vec<_>>(), vec![b.id.clone(), a.id.clone()]);
        assert!(listed[0].texture.starts_with("data:image/png;base64,"));

        let (skin, bytes) = get(dir.path(), &a.id).unwrap();
        assert_eq!(skin.variant, SkinVariant::Classic);
        assert_eq!(bytes, fake_png(64, 64));

        remove(dir.path(), &a.id).unwrap();
        assert_eq!(list(dir.path()).len(), 1);
        assert!(get(dir.path(), "../settings").is_err());
    }

    #[test]
    fn refuses_files_that_are_not_skins() {
        let dir = tempfile::tempdir().unwrap();
        assert!(add(dir.path(), "x", SkinVariant::Classic, &fake_png(32, 32), 0).is_err());
        assert!(list(dir.path()).is_empty());
    }
}
