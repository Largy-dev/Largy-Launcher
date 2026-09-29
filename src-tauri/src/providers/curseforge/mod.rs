//! CurseForge modpacks via the official Core API (`api.curseforge.com`).
//! Release builds embed the launcher's own API key (see [`builtin_key`]); a
//! player can still use their own from Paramètres. Modpacks are distributed as a zip (`manifest.json` +
//! `overrides/`): `resolve_version` downloads and unpacks it, then looks up
//! every referenced file in two batched requests (files, then their
//! projects — for the target folder and the manual-download page).

mod api_types;
pub mod content;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use serde_json::json;

use super::archive;
use super::{
    InstallWarning, LoaderKind, ModpackDetails, ModpackFileRef, ModpackProvider, ModpackSummary,
    ModpackVersionSummary, ProviderError, PackCategory, ResolvedModpackVersion, SearchQuery, SearchSort,
};
use crate::download::{DownloadItem, DownloadManager};
pub use api_types::CfProject;
use api_types::{parse_loader_id, CfFile, CfManifest, CfMod, ItemResponse, ListResponse};

const BASE: &str = "https://api.curseforge.com/v1";
const MINECRAFT_GAME_ID: u32 = 432;
const MODPACK_CLASS_ID: u32 = 4471;
const MAX_VERSION_PAGES: u32 = 4;
const BATCH: usize = 500;

/// Largy Launcher's own CurseForge API key, injected at compile time from
/// the `CURSEFORGE_API_KEY` environment variable (a GitHub Actions secret
/// for release builds) — never committed. Local builds without it fall back
/// to the key the player enters in Paramètres.
pub fn builtin_key() -> Option<&'static str> {
    option_env!("CURSEFORGE_API_KEY").map(str::trim).filter(|k| !k.is_empty())
}

pub struct CurseForgeProvider {
    client: reqwest::Client,
    api_key: Arc<RwLock<String>>,
    cache_dir: PathBuf,
}

impl CurseForgeProvider {
    pub fn new(client: reqwest::Client, api_key: Arc<RwLock<String>>, cache_dir: PathBuf) -> Self {
        Self { client, api_key, cache_dir }
    }

    /// The player's own key when set, otherwise the one baked into release
    /// builds.
    fn key(&self) -> Result<String, ProviderError> {
        let key = self.api_key.read().trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
        builtin_key().map(str::to_string).ok_or_else(|| {
            ProviderError::Other(
                "Clé API CurseForge manquante. Ouvre Paramètres et renseigne ta clé depuis \
                 console.curseforge.com pour utiliser CurseForge."
                    .to_string(),
            )
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T, ProviderError> {
        let response = self
            .client
            .get(format!("{BASE}{path}"))
            .header("x-api-key", self.key()?)
            .query(query)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ProviderError::NotFound(path.to_string()));
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Other("Clé API CurseForge refusée — vérifie-la dans Paramètres.".to_string()));
        }
        Ok(response.error_for_status()?.json().await?)
    }

    async fn post<T: DeserializeOwned>(&self, path: &str, body: serde_json::Value) -> Result<T, ProviderError> {
        Ok(self
            .client
            .post(format!("{BASE}{path}"))
            .header("x-api-key", self.key()?)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn files_by_id(&self, ids: &[u32]) -> Result<HashMap<u32, CfFile>, ProviderError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(BATCH) {
            let response: ListResponse<CfFile> = self.post("/mods/files", json!({ "fileIds": chunk })).await?;
            out.extend(response.data.into_iter().map(|f| (f.id, f)));
        }
        Ok(out)
    }

    /// Whether a key (the player's or the built-in one) is available.
    pub fn has_key(&self) -> bool {
        self.key().is_ok()
    }

    /// CurseForge projects that exactly match these file fingerprints (see
    /// [`fingerprint`]), keyed by fingerprint.
    pub async fn identify(&self, fingerprints: &[u32]) -> Result<HashMap<u32, CfProject>, ProviderError> {
        let mut file_to_mod: HashMap<u32, u32> = HashMap::new();
        for chunk in fingerprints.chunks(BATCH) {
            let response: ItemResponse<api_types::CfFingerprintMatches> =
                self.post(&format!("/fingerprints/{MINECRAFT_GAME_ID}"), json!({ "fingerprints": chunk })).await?;
            for m in response.data.exact_matches {
                file_to_mod.insert(m.file.file_fingerprint, m.id);
            }
        }
        let mut mod_ids: Vec<u32> = file_to_mod.values().copied().collect();
        mod_ids.sort_unstable();
        mod_ids.dedup();
        let mods = self.mods_by_id(&mod_ids).await?;
        Ok(file_to_mod
            .into_iter()
            .filter_map(|(fp, mod_id)| mods.get(&mod_id).map(|m| (fp, m.to_project())))
            .collect())
    }

    async fn mods_by_id(&self, ids: &[u32]) -> Result<HashMap<u32, CfMod>, ProviderError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(BATCH) {
            let response: ListResponse<CfMod> = self.post("/mods", json!({ "modIds": chunk })).await?;
            out.extend(response.data.into_iter().map(|m| (m.id, m)));
        }
        Ok(out)
    }

    /// Resolves an already-downloaded modpack zip — shared by provider
    /// installs and "import a zip" from disk.
    pub async fn resolve_zip(&self, zip_path: &Path, extract_dir: &Path) -> Result<ResolvedModpackVersion, ProviderError> {
        let manifest_text = archive::read_text(zip_path, "manifest.json")?
            .ok_or_else(|| ProviderError::Other("ce zip n'est pas un modpack CurseForge (manifest.json absent)".to_string()))?;
        let manifest: CfManifest =
            serde_json::from_str(&manifest_text).map_err(|e| ProviderError::Other(format!("manifest.json invalide: {e}")))?;

        let overrides_prefix = format!("{}/", manifest.overrides.trim_matches('/'));
        let overrides_dirs = {
            let (zip, dest) = (zip_path.to_path_buf(), extract_dir.to_path_buf());
            tokio::task::spawn_blocking(move || archive::extract_prefixes(&zip, &dest, &[overrides_prefix.as_str()]))
                .await
                .map_err(|e| ProviderError::Other(format!("tâche de fond interrompue: {e}")))??
        };

        let (loader, loader_version) = manifest
            .minecraft
            .mod_loaders
            .iter()
            .find(|l| l.primary)
            .or_else(|| manifest.minecraft.mod_loaders.first())
            .and_then(|l| parse_loader_id(&l.id))
            .unwrap_or((LoaderKind::Vanilla, String::new()));

        let wanted: Vec<_> = manifest.files.iter().filter(|f| f.required).cloned().collect();
        let (files, warnings) = if wanted.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let file_ids: Vec<u32> = wanted.iter().map(|f| f.file_id).collect();
            let found = self.files_by_id(&file_ids).await?;
            let mut mod_ids: Vec<u32> = wanted.iter().map(|f| f.project_id).collect();
            mod_ids.sort_unstable();
            mod_ids.dedup();
            let projects = self.mods_by_id(&mod_ids).await?;
            build_file_refs(&wanted, &found, &projects)
        };

        Ok(ResolvedModpackVersion {
            minecraft_version: manifest.minecraft.version,
            loader,
            loader_version,
            files,
            overrides_dirs,
            warnings,
            pack_name: manifest.name,
        })
    }
}

/// French names of CurseForge's modpack categories (unknown ones kept as is).
fn category_label(name: &str) -> String {
    let label = match name {
        "Adventure and RPG" => "Aventure et RPG",
        "Combat / PvP" => "Combat / PvP",
        "Exploration" => "Exploration",
        "Extra Large" => "Très gros",
        "Hardcore" => "Hardcore",
        "Magic" => "Magie",
        "Map Based" => "Basé sur une map",
        "Mini Game" => "Mini-jeux",
        "Multiplayer" => "Multijoueur",
        "Quests" => "Quêtes",
        "Sci-Fi" => "Science-fiction",
        "Skyblock" => "Skyblock",
        "Small / Light" => "Petit / léger",
        "Tech" => "Technologie",
        "Vanilla+" => "Vanilla+",
        "Horror" => "Horreur",
        "Exploration & Adventure" => "Exploration et aventure",
        other => return other.to_string(),
    };
    label.to_string()
}

/// CurseForge's file fingerprint: 32-bit MurmurHash2 (seed 1) of the file
/// with every whitespace byte (tab, LF, CR, space) removed.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let skip = |b: &u8| matches!(b, 9 | 10 | 13 | 32);
    // Same as `murmur2` on the filtered bytes, without copying the file.
    let len = bytes.iter().filter(|b| !skip(b)).count() as u32;
    let mut h = 1 ^ len;
    let (mut word, mut filled) = (0u32, 0u32);
    for &b in bytes.iter().filter(|b| !skip(b)) {
        word |= u32::from(b) << (8 * filled);
        filled += 1;
        if filled == 4 {
            let mut k = word.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            h = h.wrapping_mul(M) ^ k;
            (word, filled) = (0, 0);
        }
    }
    if filled > 0 {
        h ^= word;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

/// Reference MurmurHash2 — [`fingerprint`] streams the same computation.
#[cfg(test)]
fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let (chunks, tail) = data.as_chunks::<4>();
    for chunk in chunks {
        let mut k = u32::from_le_bytes(*chunk);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if !tail.is_empty() {
        for (i, byte) in tail.iter().enumerate() {
            h ^= u32::from(*byte) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

fn build_file_refs(
    wanted: &[api_types::CfManifestFile],
    found: &HashMap<u32, CfFile>,
    projects: &HashMap<u32, CfMod>,
) -> (Vec<ModpackFileRef>, Vec<InstallWarning>) {
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for entry in wanted {
        let project = projects.get(&entry.project_id);
        let Some(file) = found.get(&entry.file_id) else {
            warnings.push(InstallWarning {
                file_name: format!("projet {} / fichier {}", entry.project_id, entry.file_id),
                message: "Fichier introuvable sur CurseForge (supprimé par son auteur ?).".to_string(),
                browser_url: project.and_then(|p| p.links.website_url.clone()),
                ..Default::default()
            });
            continue;
        };
        if !crate::util::fs::is_plain_file_name(&file.file_name) {
            continue;
        }
        let folder = project.map(CfMod::target_folder).unwrap_or("mods");
        let browser_url = project
            .and_then(|p| p.links.website_url.clone())
            .map(|site| format!("{}/files/{}", site.trim_end_matches('/'), file.id));
        files.push(ModpackFileRef {
            project_id: entry.project_id.to_string(),
            file_id: entry.file_id.to_string(),
            path: PathBuf::from(folder).join(&file.file_name),
            sha1: file.sha1(),
            size: file.file_length,
            direct_url: file.download_url.clone(),
            browser_url: browser_url.or_else(|| {
                Some(format!("https://www.curseforge.com/minecraft/mc-mods/search?search={}", entry.project_id))
            }),
        });
    }
    (files, warnings)
}

#[async_trait]
impl ModpackProvider for CurseForgeProvider {
    fn id(&self) -> &'static str {
        "curseforge"
    }

    fn display_name(&self) -> &'static str {
        "CurseForge"
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ModpackSummary>, ProviderError> {
        // CurseForge's ModsSearchSortField: 2 popularity, 3 last updated,
        // 6 total downloads, 11 release date.
        let sort_field = match query.sort {
            SearchSort::Relevance => 2,
            SearchSort::Downloads => 6,
            SearchSort::Updated => 3,
            SearchSort::Newest => 11,
        };
        let mut params = vec![
            ("gameId", MINECRAFT_GAME_ID.to_string()),
            ("classId", MODPACK_CLASS_ID.to_string()),
            ("searchFilter", query.text.clone()),
            ("pageSize", "25".to_string()),
            ("index", query.offset.to_string()),
            ("sortField", sort_field.to_string()),
            ("sortOrder", "desc".to_string()),
        ];
        if let Some(version) = &query.game_version {
            params.push(("gameVersion", version.clone()));
        }
        if let Some(loader) = query.loader.and_then(content::loader_type) {
            params.push(("modLoaderType", loader.to_string()));
        }
        if let Some(category) = &query.category {
            params.push(("categoryId", category.clone()));
        }
        let response: ListResponse<CfMod> = self.get("/mods/search", &params).await?;
        Ok(response.data.iter().map(CfMod::to_summary).collect())
    }

    async fn categories(&self) -> Result<Vec<PackCategory>, ProviderError> {
        let response: ListResponse<api_types::CfCategory> = self
            .get(
                "/categories",
                &[("gameId", MINECRAFT_GAME_ID.to_string()), ("classId", MODPACK_CLASS_ID.to_string())],
            )
            .await?;
        let mut categories: Vec<PackCategory> = response
            .data
            .into_iter()
            .filter(|c| c.is_class != Some(true))
            .map(|c| PackCategory { id: c.id.to_string(), label: category_label(&c.name) })
            .collect();
        categories.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(categories)
    }

    async fn get_modpack(&self, pack_id: &str) -> Result<ModpackDetails, ProviderError> {
        let response: ItemResponse<CfMod> = self.get(&format!("/mods/{pack_id}"), &[]).await?;
        Ok(ModpackDetails { summary: response.data.to_summary(), description: response.data.summary.clone() })
    }

    async fn get_versions(&self, pack_id: &str) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        let mut all: Vec<CfFile> = Vec::new();
        for page in 0..MAX_VERSION_PAGES {
            let response: ListResponse<CfFile> = self
                .get(
                    &format!("/mods/{pack_id}/files"),
                    &[("pageSize", "50".to_string()), ("index", (page * 50).to_string())],
                )
                .await?;
            let total = response.pagination.as_ref().map(|p| p.total_count).unwrap_or(0);
            let got = response.data.len();
            all.extend(response.data);
            if got < 50 || all.len() as u32 >= total {
                break;
            }
        }
        all.retain(|f| f.is_server_pack != Some(true));
        all.sort_by_key(|f| std::cmp::Reverse(f.id));

        Ok(all
            .into_iter()
            .map(|f| {
                let (loader, loader_version) = f.loader().unwrap_or((LoaderKind::Vanilla, String::new()));
                ModpackVersionSummary {
                    id: f.id.to_string(),
                    name: f.display_name.clone(),
                    minecraft_version: f.minecraft_version().unwrap_or_default(),
                    loader,
                    loader_version,
                }
            })
            .collect())
    }

    async fn get_changelog(&self, pack_id: &str, version_id: &str) -> Result<Option<String>, ProviderError> {
        let response: ItemResponse<String> =
            match self.get(&format!("/mods/{pack_id}/files/{version_id}/changelog"), &[]).await {
                Err(ProviderError::NotFound(_)) => return Ok(None),
                other => other?,
            };
        let text = crate::util::html::html_to_text(&response.data);
        Ok((!text.trim().is_empty()).then_some(text))
    }

    async fn resolve_version(&self, pack_id: &str, version_id: &str) -> Result<ResolvedModpackVersion, ProviderError> {
        let file: ItemResponse<CfFile> = self.get(&format!("/mods/{pack_id}/files/{version_id}"), &[]).await?;
        let file = file.data;
        let download_url = file.download_url.clone().ok_or_else(|| {
            ProviderError::Other(
                "Cet auteur désactive le téléchargement direct de ce modpack : télécharge-le depuis \
                 curseforge.com puis importe le zip dans le launcher."
                    .to_string(),
            )
        })?;

        let pack_zip_path = self.cache_dir.join(format!("{pack_id}-{version_id}.zip"));
        DownloadManager::new(self.client.clone())
            .ensure_file(&DownloadItem {
                url: download_url,
                dest: pack_zip_path.clone(),
                sha1: file.sha1(),
                size: (file.file_length > 0).then_some(file.file_length),
            })
            .await?;

        let extract_dir = self.cache_dir.join(format!("{pack_id}-{version_id}-extracted"));
        self.resolve_zip(&pack_zip_path, &extract_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_types::CfManifestFile;

    fn cf_mod(id: u32, class_id: u32) -> CfMod {
        serde_json::from_value(json!({
            "id": id, "name": "M", "summary": "", "classId": class_id,
            "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/x/m{id}")}
        }))
        .unwrap()
    }

    fn cf_file(id: u32, name: &str, url: Option<&str>) -> CfFile {
        serde_json::from_value(json!({
            "id": id, "modId": 1, "displayName": name, "fileName": name, "downloadUrl": url,
            "hashes": [{"value": "AA", "algo": 1}], "fileLength": 42
        }))
        .unwrap()
    }

    #[test]
    fn murmur2_matches_the_reference_implementation() {
        // Values from the `murmurhash2` reference package.
        assert_eq!(murmur2(b"", 1), 1_540_447_798);
        assert_eq!(murmur2(b"a", 0x9747_b28c), 2_731_586_172);
        assert_eq!(murmur2(b"Hello, world!", 0x9747_b28c), 3_199_900_434);
        assert_eq!(murmur2(b"abcdefg", 1), 184_182_053);
    }

    #[test]
    fn streaming_fingerprint_matches_murmur2_on_the_filtered_bytes() {
        for sample in [&b""[..], b"a", b"ab c", b"abc d\te\r\nfgh", b"PK\x03\x04 some jar bytes\n"] {
            let filtered: Vec<u8> = sample.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
            assert_eq!(fingerprint(sample), murmur2(&filtered, 1), "{sample:?}");
        }
    }

    #[test]
    fn fingerprint_ignores_whitespace() {
        assert_eq!(fingerprint(b"a b
	c"), fingerprint(b"abc"));
    }

    #[test]
    fn file_refs_go_to_the_right_folder_and_report_missing_files() {
        let wanted = vec![
            CfManifestFile { project_id: 1, file_id: 10, required: true },
            CfManifestFile { project_id: 2, file_id: 20, required: true },
            CfManifestFile { project_id: 3, file_id: 30, required: true },
        ];
        let found = HashMap::from([
            (10, cf_file(10, "a.jar", Some("https://edge/a.jar"))),
            (20, cf_file(20, "pack.zip", None)),
        ]);
        let projects = HashMap::from([(1, cf_mod(1, 6)), (2, cf_mod(2, 12)), (3, cf_mod(3, 6))]);

        let (files, warnings) = build_file_refs(&wanted, &found, &projects);

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, PathBuf::from("mods/a.jar"));
        assert_eq!(files[0].sha1.as_deref(), Some("aa"));
        assert_eq!(files[0].size, 42);
        assert_eq!(files[1].path, PathBuf::from("resourcepacks/pack.zip"));
        assert_eq!(files[1].direct_url, None);
        assert_eq!(files[1].browser_url.as_deref(), Some("https://www.curseforge.com/minecraft/x/m2/files/20"));
        assert_eq!(warnings.len(), 1);
    }
}
