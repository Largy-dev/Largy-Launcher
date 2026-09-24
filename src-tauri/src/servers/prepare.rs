//! "Préparer et jouer": a Fabric instance made for one server — right
//! Minecraft version, the chosen client mods and shaders, the server in its
//! multiplayer list and joined straight from the launch.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::featured::{is_minecraft_version, is_slug, MAX_PROJECTS};
use crate::error::{AppError, AppResult};
use crate::instances::content::{self, ContentKind};
use crate::instances::{self, CreateInstanceInput, Instance};
use crate::providers::modrinth::ModrinthApi;
use crate::providers::LoaderKind;
use crate::state::AppState;

/// Server icons arrive as data URLs from the status ping.
const MAX_ICON_LEN: usize = 96 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct PrepareSpec {
    /// Catalog id, or `None` for a server the player typed in.
    #[serde(default)]
    pub featured_id: Option<String>,
    pub name: String,
    pub address: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub mods: Vec<String>,
    #[serde(default)]
    pub shaders: Vec<String>,
    /// `data:image/png;base64,...` favicon from the server's ping.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrepareResult {
    pub instance: Instance,
    /// Mods or shaders that couldn't be installed (the instance still works).
    pub warnings: Vec<String>,
}

impl PrepareSpec {
    fn validate(mut self) -> AppResult<Self> {
        super::validate_address(&self.address)?;
        self.address = self.address.trim().to_string();
        if !is_minecraft_version(&self.minecraft_version) {
            return Err(AppError::Other(format!("version de Minecraft invalide : {}", self.minecraft_version)));
        }
        if self.featured_id.as_deref().is_some_and(|id| !is_slug(id)) {
            return Err(AppError::Other("serveur inconnu".to_string()));
        }
        self.mods.sort();
        self.mods.dedup();
        self.shaders.dedup();
        if self.mods.len() + self.shaders.len() > MAX_PROJECTS {
            return Err(AppError::Other("trop de mods demandés".to_string()));
        }
        if let Some(bad) = self.mods.iter().chain(&self.shaders).find(|s| !is_slug(s)) {
            return Err(AppError::Other(format!("mod invalide : {bad}")));
        }
        self.icon = self
            .icon
            .filter(|icon| icon.starts_with("data:image/png;base64,") && icon.len() <= MAX_ICON_LEN);
        Ok(self)
    }
}

/// Tells Iris to load `pack` with shaders on, so they work on first launch.
pub fn enable_shader_pack(instance_dir: &Path, pack: &str) -> AppResult<()> {
    let config = format!("enableShaders=true\nshaderPack={pack}\n");
    crate::util::fs::write_atomic(&instance_dir.join("config").join("iris.properties"), config.as_bytes())?;
    Ok(())
}

pub async fn prepare(state: &AppState, spec: PrepareSpec) -> AppResult<PrepareResult> {
    let spec = spec.validate()?;
    let name = instances::validate_name(&spec.name)?;

    let fabric = state
        .loaders
        .get(LoaderKind::Fabric)
        .ok_or_else(|| AppError::Loader("Fabric indisponible".to_string()))?;
    let loader_version = fabric
        .list_versions(&state.meta, &spec.minecraft_version)
        .await
        .map_err(AppError::from)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Loader(format!("Fabric ne supporte pas encore Minecraft {}", spec.minecraft_version)))?;

    let mut instance = instances::create(
        &state.paths,
        CreateInstanceInput {
            name,
            minecraft_version: spec.minecraft_version.clone(),
            loader: LoaderKind::Fabric,
            loader_version: Some(loader_version),
            modpack: None,
            icon_url: spec.icon.clone(),
        },
    )?;
    instance.auto_join_server = Some(spec.address.clone());
    instance.featured_server = spec.featured_id.clone();
    instances::save(&instance)?;
    super::add(&instance.directory, &instance.name, &spec.address)?;

    let api = ModrinthApi::new(state.client.clone());
    let mut warnings = Vec::new();
    for slug in &spec.mods {
        if let Err(e) = content::install(&api, &state.downloader, &instance, slug, ContentKind::Mod).await {
            warnings.push(format!("{slug} : {e}"));
        }
    }
    let mut first_shader = None;
    for slug in &spec.shaders {
        match content::install(&api, &state.downloader, &instance, slug, ContentKind::Shader).await {
            Ok(files) => {
                if first_shader.is_none() {
                    first_shader = files.into_iter().next();
                }
            }
            Err(e) => warnings.push(format!("{slug} : {e}")),
        }
    }
    if let Some(pack) = first_shader {
        if let Err(e) = enable_shader_pack(&instance.directory, &pack) {
            warnings.push(format!("activation des shaders : {e}"));
        }
    }

    Ok(PrepareResult { instance, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> PrepareSpec {
        PrepareSpec {
            featured_id: Some("hypixel".into()),
            name: "Hypixel".into(),
            address: " mc.hypixel.net ".into(),
            minecraft_version: "26.3".into(),
            mods: vec!["sodium".into(), "iris".into(), "sodium".into()],
            shaders: vec!["complementary-reimagined".into()],
            icon: Some("data:image/png;base64,iVBOR".into()),
        }
    }

    #[test]
    fn validation_normalises_a_good_spec() {
        let spec = spec().validate().unwrap();
        assert_eq!(spec.address, "mc.hypixel.net");
        assert_eq!(spec.mods, vec!["iris", "sodium"]);
        assert!(spec.icon.is_some());
    }

    #[test]
    fn validation_rejects_bad_input() {
        assert!(PrepareSpec { address: "a b".into(), ..spec() }.validate().is_err());
        assert!(PrepareSpec { minecraft_version: "latest".into(), ..spec() }.validate().is_err());
        assert!(PrepareSpec { mods: vec!["../x".into()], ..spec() }.validate().is_err());
        assert!(PrepareSpec { featured_id: Some("A/B".into()), ..spec() }.validate().is_err());
        let too_many = (0..=MAX_PROJECTS).map(|i| format!("mod-{i}")).collect();
        assert!(PrepareSpec { mods: too_many, ..spec() }.validate().is_err());
    }

    #[test]
    fn non_png_icons_are_dropped() {
        let spec = PrepareSpec { icon: Some("https://evil/x.png".into()), ..spec() }.validate().unwrap();
        assert!(spec.icon.is_none());
    }

    #[test]
    fn shader_pack_is_switched_on_for_iris() {
        let dir = tempfile::tempdir().unwrap();
        enable_shader_pack(dir.path(), "ComplementaryReimagined_r5.zip").unwrap();
        let config = std::fs::read_to_string(dir.path().join("config/iris.properties")).unwrap();
        assert!(config.contains("enableShaders=true"));
        assert!(config.contains("shaderPack=ComplementaryReimagined_r5.zip"));
    }
}
