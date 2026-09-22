//! Unpacking a downloaded CurseForge modpack zip: pulling out `manifest.json`
//! and copying its `overrides/` folder to a cache location for the instance
//! installer to copy wholesale.

use super::api_types::CfManifest;

pub(super) fn extract_manifest_and_overrides(
    zip_path: &std::path::Path,
    extract_dir: &std::path::Path,
) -> std::io::Result<CfManifest> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| std::io::Error::other(e.to_string()))?;

    let manifest: CfManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text)?;
        serde_json::from_str(&text).map_err(|e| std::io::Error::other(e.to_string()))?
    };

    if !extract_dir.exists() {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| std::io::Error::other(e.to_string()))?;
            let name = entry.name().to_string();
            if !name.starts_with("overrides/") || name.ends_with('/') {
                continue;
            }
            let dest = extract_dir.join(name.trim_start_matches("overrides/"));
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }

    Ok(manifest)
}
