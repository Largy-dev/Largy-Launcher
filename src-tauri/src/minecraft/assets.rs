//! Downloads a version's asset index and every object it references into
//! the shared `assets/` cache (content-addressed by hash, like Mojang's own
//! launcher, so every instance shares one copy of each sound/texture).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tauri::AppHandle;

use crate::download::{DownloadItem, DownloadManager};
use crate::error::AppResult;

use super::manifest::AssetIndexRef;

#[derive(Debug, Deserialize)]
struct AssetIndexJson {
    objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize, Clone)]
struct AssetObject {
    hash: String,
    size: u64,
}

/// Builds the content-addressed download item for every asset object. Split
/// out from [`prepare_assets`] so the URL/destination construction is
/// testable without a network round-trip.
fn asset_download_items(assets_dir: &Path, objects: &HashMap<String, AssetObject>) -> Vec<DownloadItem> {
    objects
        .values()
        .map(|object| {
            let prefix = &object.hash[0..2];
            DownloadItem {
                url: format!("https://resources.download.minecraft.net/{prefix}/{}", object.hash),
                dest: assets_dir.join("objects").join(prefix).join(&object.hash),
                sha1: Some(object.hash.clone()),
                size: Some(object.size),
            }
        })
        .collect()
}

/// Pre-1.6 clients expect assets laid out by their original relative path
/// (`virtual/<index-id>/<path>`) rather than content-addressed — copies
/// already-downloaded objects into that legacy layout, skipping anything
/// already there or not yet downloaded. Split out from [`prepare_assets`] so
/// the copy/skip decisions are testable with real temp-directory fixtures.
async fn populate_virtual_assets(assets_dir: &Path, asset_index_id: &str, objects: &HashMap<String, AssetObject>) {
    let virtual_dir = assets_dir.join("virtual").join(asset_index_id);
    for (virtual_path, object) in objects {
        let prefix = &object.hash[0..2];
        let src = assets_dir.join("objects").join(prefix).join(&object.hash);
        let dest = virtual_dir.join(virtual_path);
        if dest.exists() || !src.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::copy(&src, &dest).await.ok();
    }
}

pub async fn prepare_assets(
    app: &AppHandle,
    downloader: &DownloadManager,
    assets_dir: &Path,
    asset_index: &AssetIndexRef,
) -> AppResult<()> {
    let index_path = assets_dir.join("indexes").join(format!("{}.json", asset_index.id));
    if let Some(parent) = index_path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    let index_item = DownloadItem {
        url: asset_index.url.clone(),
        dest: index_path.clone(),
        sha1: Some(asset_index.sha1.clone()),
        size: Some(asset_index.size),
    };
    downloader.ensure_file(&index_item).await?;

    let bytes = tokio::fs::read(&index_path).await?;
    let index: AssetIndexJson = serde_json::from_slice(&bytes)?;

    let items = asset_download_items(assets_dir, &index.objects);

    downloader
        .run_batch(app, "assets", "Ressources du jeu", items, 16)
        .await?;

    if asset_index.id == "legacy" || asset_index.id == "pre-1.6" {
        populate_virtual_assets(assets_dir, &asset_index.id, &index.objects).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(hash: &str, size: u64) -> AssetObject {
        AssetObject { hash: hash.to_string(), size }
    }

    #[test]
    fn asset_download_items_builds_content_addressed_url_and_dest() {
        let mut objects = HashMap::new();
        objects.insert("sound/click.ogg".to_string(), object("abcdef1234567890", 42));

        let items = asset_download_items(Path::new("/assets"), &objects);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://resources.download.minecraft.net/ab/abcdef1234567890");
        assert!(items[0].dest.ends_with("objects/ab/abcdef1234567890"));
        assert_eq!(items[0].sha1, Some("abcdef1234567890".to_string()));
        assert_eq!(items[0].size, Some(42));
    }

    #[tokio::test]
    async fn populate_virtual_assets_copies_downloaded_objects_into_the_legacy_layout() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path();
        let hash = "abcdef1234567890";
        let object_path = assets_dir.join("objects").join("ab").join(hash);
        tokio::fs::create_dir_all(object_path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&object_path, b"sound data").await.unwrap();

        let mut objects = HashMap::new();
        objects.insert("sound/click.ogg".to_string(), object(hash, 10));

        populate_virtual_assets(assets_dir, "legacy", &objects).await;

        let dest = assets_dir.join("virtual").join("legacy").join("sound/click.ogg");
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), b"sound data");
    }

    #[tokio::test]
    async fn populate_virtual_assets_skips_objects_that_were_never_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path();

        let mut objects = HashMap::new();
        objects.insert("sound/click.ogg".to_string(), object("abcdef1234567890", 10));

        populate_virtual_assets(assets_dir, "legacy", &objects).await;

        let dest = assets_dir.join("virtual").join("legacy").join("sound/click.ogg");
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn populate_virtual_assets_does_not_overwrite_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path();
        let hash = "abcdef1234567890";
        let object_path = assets_dir.join("objects").join("ab").join(hash);
        tokio::fs::create_dir_all(object_path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&object_path, b"new data").await.unwrap();

        let dest = assets_dir.join("virtual").join("legacy").join("sound/click.ogg");
        tokio::fs::create_dir_all(dest.parent().unwrap()).await.unwrap();
        tokio::fs::write(&dest, b"existing data").await.unwrap();

        let mut objects = HashMap::new();
        objects.insert("sound/click.ogg".to_string(), object(hash, 10));

        populate_virtual_assets(assets_dir, "legacy", &objects).await;

        assert_eq!(tokio::fs::read(&dest).await.unwrap(), b"existing data");
    }
}
