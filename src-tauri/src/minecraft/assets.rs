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

    let mut items = Vec::with_capacity(index.objects.len());
    for object in index.objects.values() {
        let prefix = &object.hash[0..2];
        let dest = assets_dir.join("objects").join(prefix).join(&object.hash);
        items.push(DownloadItem {
            url: format!("https://resources.download.minecraft.net/{prefix}/{}", object.hash),
            dest,
            sha1: Some(object.hash.clone()),
            size: Some(object.size),
        });
    }

    downloader
        .run_batch(app, "assets", "Ressources du jeu", items, 16)
        .await?;

    if asset_index.id == "legacy" || asset_index.id == "pre-1.6" {
        let virtual_dir = assets_dir.join("virtual").join(&asset_index.id);
        for (virtual_path, object) in &index.objects {
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

    Ok(())
}
