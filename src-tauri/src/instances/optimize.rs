//! "Optimiser": the well-known client performance mods for the instance's
//! loader and version, installed from Modrinth in one go. Each one is only
//! offered when nothing already covers it (Embeddium or OptiFine already do
//! Sodium's job, Canary Lithium's…) and when a compatible version exists.

use serde::Serialize;

use super::content::{self, ContentKind};
use super::installed::InstalledItem;
use super::Instance;
use crate::download::DownloadManager;
use crate::error::AppResult;
use crate::providers::modrinth::ModrinthApi;
use crate::providers::LoaderKind;

struct Candidate {
    /// Modrinth slugs to try, in order (the first compatible one wins).
    slugs: &'static [&'static str],
    reason: &'static str,
    /// Mod ids meaning "this job is already done".
    covered_by: &'static [&'static str],
}

const CANDIDATES: &[Candidate] = &[
    Candidate {
        slugs: &["sodium", "embeddium"],
        reason: "Rendu beaucoup plus rapide : souvent 2 à 3 fois plus d'images par seconde.",
        covered_by: &["sodium", "embeddium", "rubidium", "magnesium", "optifine", "xenon", "celeritas"],
    },
    Candidate {
        slugs: &["lithium"],
        reason: "Logique du jeu (IA, physique, blocs) optimisée, sans rien changer au gameplay.",
        covered_by: &["lithium", "canary", "radium", "roadrunner"],
    },
    Candidate {
        slugs: &["ferrite-core"],
        reason: "Réduit fortement la mémoire utilisée, surtout avec beaucoup de mods.",
        covered_by: &["ferritecore"],
    },
    Candidate {
        slugs: &["modernfix"],
        reason: "Démarrage plus rapide et moins de mémoire, pensé pour les gros packs.",
        covered_by: &["modernfix"],
    },
    Candidate {
        slugs: &["entityculling"],
        reason: "N'affiche pas les entités et coffres cachés derrière les murs.",
        covered_by: &["entityculling"],
    },
    Candidate {
        slugs: &["immediatelyfast"],
        reason: "Accélère l'affichage de l'interface, du texte et des cartes.",
        covered_by: &["immediatelyfast"],
    },
    Candidate {
        slugs: &["dynamic-fps"],
        reason: "Réduit les images par seconde quand le jeu est en arrière-plan : PC plus frais.",
        covered_by: &["dynamic_fps", "dynamicfps"],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OptimizeStatus {
    /// Will be installed.
    Install,
    /// Already installed, or another mod does the same job.
    Present,
    /// No version for this Minecraft version / loader.
    Unavailable,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct OptimizeItem {
    /// Modrinth slug to install (the first candidate when none fits).
    pub slug: String,
    pub title: String,
    pub reason: String,
    pub icon_url: Option<String>,
    pub status: OptimizeStatus,
    /// The installed mod that already covers it, for `Present`.
    pub covered_by: Option<String>,
}

fn covered(installed: &[InstalledItem], ids: &[&str]) -> Option<String> {
    installed.iter().filter(|i| i.enabled).find_map(|i| {
        i.provides
            .iter()
            .chain(i.mod_id.as_ref())
            .any(|id| ids.contains(&id.as_str()))
            .then(|| i.remote.as_ref().map(|r| r.title.clone()).or_else(|| i.name.clone()).unwrap_or_else(|| i.file_name.clone()))
    })
}

/// What "Optimiser" would do for this instance.
pub async fn plan(api: &ModrinthApi, instance: &Instance, installed: &[InstalledItem]) -> AppResult<Vec<OptimizeItem>> {
    if instance.loader == LoaderKind::Vanilla {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for candidate in CANDIDATES {
        if let Some(by) = covered(installed, candidate.covered_by) {
            items.push(OptimizeItem {
                slug: candidate.slugs[0].to_string(),
                title: String::new(),
                reason: candidate.reason.to_string(),
                icon_url: None,
                status: OptimizeStatus::Present,
                covered_by: Some(by),
            });
            continue;
        }
        let mut chosen = None;
        for slug in candidate.slugs {
            if content::compatible_version(api, instance, ContentKind::Mod, slug).await.is_ok() {
                chosen = Some(*slug);
                break;
            }
        }
        items.push(OptimizeItem {
            slug: chosen.unwrap_or(candidate.slugs[0]).to_string(),
            title: String::new(),
            reason: candidate.reason.to_string(),
            icon_url: None,
            status: if chosen.is_some() { OptimizeStatus::Install } else { OptimizeStatus::Unavailable },
            covered_by: None,
        });
    }

    let slugs: Vec<String> = items.iter().map(|i| i.slug.clone()).collect();
    if let Ok(projects) = api.projects(&slugs).await {
        for item in &mut items {
            if let Some(p) = projects.iter().find(|p| p.slug == item.slug || p.id == item.slug) {
                item.title = p.title.clone();
                item.icon_url = p.icon_url.clone();
            }
        }
    }
    for item in &mut items {
        if item.title.is_empty() {
            item.title = item.slug.clone();
        }
    }
    Ok(items)
}

/// Installs the chosen slugs (with their dependencies); returns the files
/// written and the failures as `"slug : error"`.
pub async fn apply(
    api: &ModrinthApi,
    downloader: &DownloadManager,
    instance: &Instance,
    slugs: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut written = Vec::new();
    let mut failed = Vec::new();
    for slug in slugs {
        match content::install(api, downloader, instance, slug, ContentKind::Mod).await {
            Ok(files) => written.extend(files),
            Err(e) => failed.push(format!("{slug} : {e}")),
        }
    }
    (written, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(mod_id: &str, provides: &[&str], enabled: bool) -> InstalledItem {
        serde_json::from_value(serde_json::json!({
            "file_name": format!("{mod_id}.jar"), "enabled": enabled, "is_dir": false, "size": 1, "modified": 0,
            "sha1": null, "fingerprint": null, "mod_id": mod_id, "name": mod_id.to_uppercase(), "version": null,
            "description": null, "authors": [], "loaders": [], "provides": provides, "depends": [], "breaks": [],
            "icon_path": null, "remote": null
        }))
        .unwrap()
    }

    #[test]
    fn a_mod_doing_the_same_job_counts_as_present() {
        let installed = vec![item("embeddium", &["embeddium"], true), item("canary", &["canary"], false)];
        assert_eq!(covered(&installed, CANDIDATES[0].covered_by).as_deref(), Some("EMBEDDIUM"));
        // Disabled mods don't do any job.
        assert_eq!(covered(&installed, CANDIDATES[1].covered_by), None);
    }

    #[test]
    fn every_candidate_covers_itself() {
        for c in CANDIDATES {
            assert!(!c.slugs.is_empty() && !c.covered_by.is_empty());
        }
    }
}
