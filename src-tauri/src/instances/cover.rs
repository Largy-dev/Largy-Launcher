//! The picture shown on an instance's card and banner: by default its
//! newest screenshot, or one the player picked (a screenshot, or any image
//! copied into the instance folder), or none.

use serde::Deserialize;

use super::Instance;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

/// Custom covers can be this big at most.
const MAX_COVER_BYTES: u64 = 15 * 1024 * 1024;
const NO_COVER: &str = "none";

/// What the player picked as an instance's picture.
#[derive(Debug, Clone, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CoverChoice {
    /// The newest screenshot, whatever it is.
    Auto,
    None,
    /// An image file: a screenshot of the instance is used in place, any
    /// other picture is copied into the instance folder.
    File(String),
}

pub(super) fn resolve(instance: &mut Instance) {
    instance.cover_path = match instance.cover.as_deref() {
        Some(NO_COVER) => None,
        Some(rel) => crate::util::fs::safe_join(&instance.directory, rel)
            .filter(|p| p.is_file())
            .or_else(|| super::screenshots::newest(&instance.directory))
            .map(|p| p.display().to_string()),
        None => super::screenshots::newest(&instance.directory).map(|p| p.display().to_string()),
    };
}

fn is_picture(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["png", "jpg", "jpeg", "webp"].contains(&e.to_ascii_lowercase().as_str()))
}

pub fn set_cover(paths: &AppPaths, id: &str, choice: CoverChoice) -> AppResult<Instance> {
    let current = super::get(paths, id)?;
    let cover = match choice {
        CoverChoice::Auto => None,
        CoverChoice::None => Some(NO_COVER.to_string()),
        CoverChoice::File(file) => {
            let source = std::path::PathBuf::from(&file);
            if !is_picture(&source) || !source.is_file() {
                return Err(AppError::Instance("choisis une image PNG, JPG ou WebP".to_string()));
            }
            let inside = std::fs::canonicalize(&source)
                .ok()
                .zip(std::fs::canonicalize(&current.directory).ok())
                .and_then(|(s, root)| s.strip_prefix(&root).ok().map(|rel| rel.to_path_buf()))
                // Only screenshots are used in place: the asset protocol
                // serves nothing else from inside an instance.
                .filter(|rel| rel.starts_with("screenshots"));
            match inside {
                Some(rel) => Some(rel.to_string_lossy().replace('\\', "/")),
                None => {
                    if std::fs::metadata(&source)?.len() > MAX_COVER_BYTES {
                        return Err(AppError::Instance("image trop lourde (15 Mo au maximum)".to_string()));
                    }
                    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                    for old in ["png", "jpg", "jpeg", "webp"] {
                        let _ = std::fs::remove_file(current.directory.join(format!("cover.{old}")));
                    }
                    let name = format!("cover.{ext}");
                    std::fs::copy(&source, current.directory.join(&name))?;
                    Some(name)
                }
            }
        }
    };
    super::update(paths, id, |i| {
        i.cover = cover;
        Ok(())
    })
}
