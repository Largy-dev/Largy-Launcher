//! The game's `screenshots/` folder, listed for the in-launcher gallery.
//! Images themselves are served to the webview by the asset protocol.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::util::fs::validate_file_name;

const DIR: &str = "screenshots";

#[derive(Debug, Clone, Serialize)]
pub struct Screenshot {
    pub file_name: String,
    /// Absolute path, turned into an asset URL by the frontend.
    pub path: String,
    pub size: u64,
    /// Unix seconds (file modification time).
    pub taken_at: i64,
}

fn is_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg"].iter().any(|ext| lower.ends_with(ext))
}

/// Newest first.
pub fn list(instance_dir: &Path) -> AppResult<Vec<Screenshot>> {
    let Ok(entries) = std::fs::read_dir(instance_dir.join(DIR)) else {
        return Ok(Vec::new());
    };
    let mut shots: Vec<Screenshot> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().ok()?;
            (meta.is_file() && is_image(&file_name)).then(|| Screenshot {
                path: entry.path().display().to_string(),
                size: meta.len(),
                taken_at: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs() as i64),
                file_name,
            })
        })
        .collect();
    shots.sort_by(|a, b| b.taken_at.cmp(&a.taken_at).then_with(|| b.file_name.cmp(&a.file_name)));
    Ok(shots)
}

pub fn delete(instance_dir: &Path, file_name: &str) -> AppResult<()> {
    validate_file_name(file_name)?;
    if !is_image(file_name) {
        return Err(AppError::Instance(format!("{file_name} n'est pas une capture d'écran")));
    }
    std::fs::remove_file(instance_dir.join(DIR).join(file_name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_images_only_and_deletes_by_name() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).unwrap().is_empty());

        let shots = dir.path().join(DIR);
        std::fs::create_dir_all(&shots).unwrap();
        std::fs::write(shots.join("2026-01-01_10.00.00.png"), b"png").unwrap();
        std::fs::write(shots.join("notes.txt"), b"x").unwrap();
        std::fs::write(shots.join("b.JPG"), b"jpg").unwrap();

        let names: Vec<_> = list(dir.path()).unwrap().into_iter().map(|s| s.file_name).collect();
        assert_eq!(names.len(), 2);
        assert!(!names.contains(&"notes.txt".to_string()));

        delete(dir.path(), "b.JPG").unwrap();
        assert_eq!(list(dir.path()).unwrap().len(), 1);
        assert!(delete(dir.path(), "notes.txt").is_err());
        assert!(delete(dir.path(), "../instance.json").is_err());
    }
}
