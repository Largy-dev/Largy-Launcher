//! Reads what a content file says about itself: the loader metadata inside
//! a mod jar (`fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`,
//! `META-INF/neoforge.mods.toml`, legacy `mcmod.info`) or a resource pack's
//! `pack.mcmeta` — name, version, authors, icon and, per loader, the mod ids
//! it provides, requires and refuses to run with. A jar built for several
//! loaders carries one metadata file each, whose dependencies only hold on
//! that loader ([`LocalMeta::deps_for`] picks the right one). Everything here
//! is best effort: a jar with no or broken metadata yields an empty
//! [`LocalMeta`].

use std::io::{Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::providers::LoaderKind;

/// Largest metadata file or icon read out of an archive.
const MAX_ENTRY: u64 = 1024 * 1024;
/// Nested jars (jar-in-jar libraries) are only opened to learn the mod ids
/// they provide; past this size they're skipped.
const MAX_NESTED_JAR: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalMeta {
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    /// Loaders the file declares metadata for (`fabric`, `quilt`, `forge`, `neoforge`).
    pub loaders: Vec<String>,
    /// Dependency information, one entry per loader in [`Self::loaders`].
    pub per_loader: Vec<LoaderDeps>,
    /// Path of the icon inside the archive, if any.
    #[serde(skip)]
    pub icon_entry: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LoaderDeps {
    pub loader: String,
    /// Every mod id the file makes available on this loader: its own,
    /// `provides` aliases and those of the jars it bundles.
    pub provides: Vec<String>,
    /// Mod ids it can't run without (loader and game ids left out).
    pub depends: Vec<String>,
    /// Mods it declares itself incompatible with.
    pub breaks: Vec<BreakRule>,
}

/// "Doesn't work with these versions of `id`".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct BreakRule {
    pub id: String,
    /// Versions concerned: a Maven range (Forge / NeoForge, e.g. `(,1.6.2)`)
    /// or npm-style predicates joined by `||` (Fabric / Quilt, e.g. `<0.5`).
    /// `None` = every version.
    pub versions: Option<String>,
    /// Whether [`Self::versions`] is a Maven range.
    pub maven: bool,
}

impl BreakRule {
    fn any(id: String) -> Self {
        BreakRule { id, versions: None, maven: false }
    }
}

/// `"*"`, `"[0,)"`… — ranges that mean "every version".
fn is_unbounded(range: &str) -> bool {
    let r = range.trim();
    r.is_empty() || r == "*" || r == "[0,)" || r == "(0,)" || r == "[0.0.0,)" || r == "[0.0.1,)"
}

impl LocalMeta {
    fn deps_mut(&mut self, loader: &str) -> &mut LoaderDeps {
        if !self.loaders.iter().any(|l| l == loader) {
            self.loaders.push(loader.to_string());
        }
        match self.per_loader.iter().position(|d| d.loader == loader) {
            Some(i) => &mut self.per_loader[i],
            None => {
                self.per_loader.push(LoaderDeps { loader: loader.to_string(), ..Default::default() });
                self.per_loader.last_mut().expect("just pushed")
            }
        }
    }

    /// The metadata the instance's loader actually reads: Quilt also loads
    /// Fabric mods, NeoForge also loads (older) Forge ones.
    pub fn deps_for(&self, loader: LoaderKind) -> Option<&LoaderDeps> {
        let order: &[&str] = match loader {
            LoaderKind::Vanilla => &[],
            LoaderKind::Fabric => &["fabric"],
            LoaderKind::Quilt => &["quilt", "fabric"],
            LoaderKind::Forge => &["forge"],
            LoaderKind::NeoForge => &["neoforge", "forge"],
        };
        order.iter().find_map(|l| self.per_loader.iter().find(|d| d.loader == *l))
    }
}

/// Ids every loader provides itself — never reported as missing.
const BUILTIN_IDS: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "forge",
    "neoforge",
    "fml",
    "javafml",
    "lowcodefml",
    "mixinextras",
];

pub fn is_builtin(id: &str) -> bool {
    BUILTIN_IDS.contains(&id)
}

/// Metadata of a mod jar or resource/shader pack zip.
pub fn read_archive(path: &Path) -> LocalMeta {
    let Ok(file) = std::fs::File::open(path) else {
        return LocalMeta::default();
    };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) else {
        return LocalMeta::default();
    };
    read_archive_from(&mut zip)
}

/// Metadata of an already opened mod jar or pack zip.
pub fn read_archive_from<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> LocalMeta {
    read_zip(zip, true)
}

/// Metadata of a resource pack unpacked as a folder.
pub fn read_folder_pack(dir: &Path) -> LocalMeta {
    let mut meta = LocalMeta::default();
    if let Ok(text) = std::fs::read_to_string(dir.join("pack.mcmeta")) {
        apply_pack_mcmeta(&mut meta, &text);
    }
    if dir.join("pack.png").is_file() {
        meta.icon_entry = Some("pack.png".to_string());
    }
    meta
}

pub fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_ENTRY {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_ENTRY).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn read_text<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    read_entry(zip, name).map(|b| String::from_utf8_lossy(&b).trim_start_matches('\u{feff}').to_string())
}

fn read_zip<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, top_level: bool) -> LocalMeta {
    let mut meta = LocalMeta::default();
    if let Some(text) = read_text(zip, "fabric.mod.json") {
        apply_fabric(&mut meta, &text);
    }
    if let Some(text) = read_text(zip, "quilt.mod.json") {
        apply_quilt(&mut meta, &text);
    }
    for (file, loader) in [("META-INF/neoforge.mods.toml", "neoforge"), ("META-INF/mods.toml", "forge")] {
        if let Some(text) = read_text(zip, file) {
            let jar_version = read_text(zip, "META-INF/MANIFEST.MF").and_then(|m| manifest_version(&m));
            apply_mods_toml(&mut meta, &text, loader, jar_version.as_deref());
        }
    }
    if meta.loaders.is_empty() {
        if let Some(text) = read_text(zip, "mcmod.info") {
            apply_mcmod_info(&mut meta, &text);
        }
    }
    if meta.loaders.is_empty() {
        if let Some(text) = read_text(zip, "pack.mcmeta") {
            apply_pack_mcmeta(&mut meta, &text);
            if zip.by_name("pack.png").is_ok() {
                meta.icon_entry = Some("pack.png".to_string());
            }
        }
    }
    if top_level {
        for nested in nested_jars(zip) {
            let Some(bytes) = read_nested(zip, &nested) else { continue };
            if let Ok(mut inner) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
                let library = meta.per_loader.is_empty();
                for inner_deps in read_zip(&mut inner, false).per_loader {
                    // Bundled jars count for the loaders the outer jar targets
                    // — or for their own loader when the outer jar is a bare
                    // library (e.g. Kotlin for Forge, whose mod lives inside).
                    if library {
                        meta.deps_mut(&inner_deps.loader).provides.extend(inner_deps.provides);
                    } else if let Some(outer) = meta.per_loader.iter_mut().find(|d| d.loader == inner_deps.loader) {
                        outer.provides.extend(inner_deps.provides);
                    }
                }
            }
        }
    }
    for deps in &mut meta.per_loader {
        deps.provides.sort();
        deps.provides.dedup();
        let own = deps.provides.clone();
        deps.depends.retain(|d| !is_builtin(d) && !own.contains(d));
        deps.depends.sort();
        deps.depends.dedup();
        deps.breaks.retain(|b| !is_builtin(&b.id) && !own.contains(&b.id));
        deps.breaks.sort_by(|a, b| a.id.cmp(&b.id));
        deps.breaks.dedup();
    }
    meta.authors.dedup();
    meta
}

fn read_nested<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_NESTED_JAR {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_NESTED_JAR).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// Bundled jars: Fabric/Quilt's `META-INF/jars/` and Forge's `META-INF/jarjar/`.
fn nested_jars<R: Read + Seek>(zip: &zip::ZipArchive<R>) -> Vec<String> {
    zip.file_names()
        .filter(|n| (n.starts_with("META-INF/jars/") || n.starts_with("META-INF/jarjar/")) && n.ends_with(".jar"))
        .map(str::to_string)
        .collect()
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string)
}

fn set_if_none(slot: &mut Option<String>, value: Option<String>) {
    if slot.is_none() {
        *slot = value;
    }
}

/// `"icon": "path"` or `"icon": {"16": "a.png", "128": "b.png"}` — the largest.
fn icon_path(value: Option<&Json>) -> Option<String> {
    match value? {
        Json::String(s) => non_empty(Some(s)),
        Json::Object(sizes) => sizes
            .iter()
            .filter_map(|(size, path)| Some((size.parse::<u32>().unwrap_or(0), path.as_str()?)))
            .max_by_key(|(size, _)| *size)
            .and_then(|(_, path)| non_empty(Some(path))),
        _ => None,
    }
}

fn person_names(value: Option<&Json>) -> Vec<String> {
    let names = |v: &Json| match v {
        Json::String(s) => non_empty(Some(s)),
        Json::Object(o) => non_empty(o.get("name").and_then(Json::as_str)),
        _ => None,
    };
    match value {
        Some(Json::Array(items)) => items.iter().filter_map(names).collect(),
        // Quilt's `contributors` is a name → role map.
        Some(Json::Object(map)) => map.keys().filter_map(|k| non_empty(Some(k))).collect(),
        _ => Vec::new(),
    }
}

pub fn apply_fabric(meta: &mut LocalMeta, text: &str) {
    let Ok(json) = serde_json::from_str::<Json>(text) else { return };
    let id = non_empty(json.get("id").and_then(Json::as_str));
    set_if_none(&mut meta.mod_id, id.clone());
    set_if_none(&mut meta.name, non_empty(json.get("name").and_then(Json::as_str)));
    set_if_none(&mut meta.version, non_empty(json.get("version").and_then(Json::as_str)));
    set_if_none(&mut meta.description, non_empty(json.get("description").and_then(Json::as_str)));
    if meta.authors.is_empty() {
        meta.authors = person_names(json.get("authors"));
    }
    set_if_none(&mut meta.icon_entry, icon_path(json.get("icon")));
    let deps = meta.deps_mut("fabric");
    deps.provides.extend(id);
    if let Some(Json::Array(aliases)) = json.get("provides") {
        deps.provides.extend(aliases.iter().filter_map(|a| non_empty(a.as_str())));
    }
    if let Some(Json::Object(required)) = json.get("depends") {
        deps.depends.extend(required.keys().cloned());
    }
    if let Some(Json::Object(breaks)) = json.get("breaks") {
        deps.breaks.extend(breaks.iter().map(|(id, versions)| npm_rule(id.clone(), Some(versions))));
    }
}

pub fn apply_quilt(meta: &mut LocalMeta, text: &str) {
    let Ok(json) = serde_json::from_str::<Json>(text) else { return };
    let Some(loader) = json.get("quilt_loader") else { return };
    let id = non_empty(loader.get("id").and_then(Json::as_str));
    set_if_none(&mut meta.mod_id, id.clone());
    set_if_none(&mut meta.version, non_empty(loader.get("version").and_then(Json::as_str)));
    if let Some(info) = loader.get("metadata") {
        set_if_none(&mut meta.name, non_empty(info.get("name").and_then(Json::as_str)));
        set_if_none(&mut meta.description, non_empty(info.get("description").and_then(Json::as_str)));
        if meta.authors.is_empty() {
            meta.authors = person_names(info.get("contributors"));
        }
        set_if_none(&mut meta.icon_entry, icon_path(info.get("icon")));
    }
    let ids_of = |value: Option<&Json>, skip_optional: bool| -> Vec<String> {
        let Some(Json::Array(items)) = value else { return Vec::new() };
        items
            .iter()
            .filter_map(|item| match item {
                Json::String(s) => non_empty(Some(s)),
                Json::Object(o) if !(skip_optional && o.get("optional").and_then(Json::as_bool).unwrap_or(false)) => {
                    non_empty(o.get("id").and_then(Json::as_str))
                }
                _ => None,
            })
            .collect()
    };
    let provides = ids_of(loader.get("provides"), false);
    let depends = ids_of(loader.get("depends"), true);
    let breaks: Vec<BreakRule> = match loader.get("breaks") {
        Some(Json::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Json::String(s) => non_empty(Some(s)).map(BreakRule::any),
                Json::Object(o) => {
                    non_empty(o.get("id").and_then(Json::as_str)).map(|id| npm_rule(id, o.get("versions")))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let deps = meta.deps_mut("quilt");
    deps.provides.extend(id);
    deps.provides.extend(provides);
    deps.depends.extend(depends);
    deps.breaks.extend(breaks);
}

/// A Fabric / Quilt version predicate (string or array of alternatives).
fn npm_rule(id: String, versions: Option<&Json>) -> BreakRule {
    let joined = match versions {
        Some(Json::String(v)) => Some(v.clone()),
        Some(Json::Array(items)) => {
            Some(items.iter().filter_map(Json::as_str).collect::<Vec<_>>().join(" || "))
        }
        _ => None,
    };
    match joined.filter(|v| !is_unbounded(v)) {
        Some(v) => BreakRule { id, versions: Some(v), maven: false },
        None => BreakRule::any(id),
    }
}

fn manifest_version(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find_map(|l| l.strip_prefix("Implementation-Version:"))
        .and_then(|v| non_empty(Some(v)))
}

pub fn apply_mods_toml(meta: &mut LocalMeta, text: &str, loader: &str, jar_version: Option<&str>) {
    let Ok(doc) = text.parse::<toml::Table>() else { return };
    meta.deps_mut(loader);
    let str_of = |t: &toml::Table, key: &str| non_empty(t.get(key).and_then(toml::Value::as_str));
    let mods: Vec<&toml::Table> = doc
        .get("mods")
        .and_then(toml::Value::as_array)
        .map(|a| a.iter().filter_map(toml::Value::as_table).collect())
        .unwrap_or_default();

    for (index, entry) in mods.iter().enumerate() {
        let Some(id) = str_of(entry, "modId") else { continue };
        meta.deps_mut(loader).provides.push(id.clone());
        if index > 0 {
            continue;
        }
        set_if_none(&mut meta.mod_id, Some(id));
        set_if_none(&mut meta.name, str_of(entry, "displayName"));
        let version = str_of(entry, "version").map(|v| {
            if v.contains("${file.jarVersion}") {
                jar_version.map(|j| v.replace("${file.jarVersion}", j)).unwrap_or_default()
            } else {
                v
            }
        });
        set_if_none(&mut meta.version, version.filter(|v| !v.is_empty()));
        set_if_none(&mut meta.description, str_of(entry, "description"));
        if meta.authors.is_empty() {
            meta.authors = str_of(entry, "authors")
                .map(|a| a.split([',', '&']).filter_map(|n| non_empty(Some(n))).collect())
                .unwrap_or_default();
        }
        set_if_none(&mut meta.icon_entry, str_of(entry, "logoFile").or_else(|| str_of(&doc, "logoFile")));
    }

    if let Some(deps) = doc.get("dependencies").and_then(toml::Value::as_table) {
        for list in deps.values().filter_map(toml::Value::as_array) {
            for dep in list.iter().filter_map(toml::Value::as_table) {
                let kind = dep.get("type").and_then(toml::Value::as_str).map(str::to_ascii_lowercase);
                let required = match kind.as_deref() {
                    Some(kind) => kind == "required",
                    None => dep.get("mandatory").and_then(toml::Value::as_bool).unwrap_or(false),
                };
                let server_only = dep.get("side").and_then(toml::Value::as_str).is_some_and(|s| s == "SERVER");
                if server_only {
                    continue;
                }
                let deps = meta.deps_mut(loader);
                if required {
                    deps.depends.extend(str_of(dep, "modId"));
                } else if kind.as_deref() == Some("incompatible") {
                    if let Some(id) = str_of(dep, "modId") {
                        let range = str_of(dep, "versionRange").filter(|r| !is_unbounded(r));
                        deps.breaks.push(BreakRule { id, maven: range.is_some(), versions: range });
                    }
                }
            }
        }
    }
}

pub fn apply_mcmod_info(meta: &mut LocalMeta, text: &str) {
    let Ok(json) = serde_json::from_str::<Json>(text) else { return };
    let list = match &json {
        Json::Array(items) => items.clone(),
        Json::Object(o) => o.get("modList").and_then(Json::as_array).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    let Some(first) = list.first() else { return };
    let id = non_empty(first.get("modid").and_then(Json::as_str));
    set_if_none(&mut meta.mod_id, id);
    set_if_none(&mut meta.name, non_empty(first.get("name").and_then(Json::as_str)));
    set_if_none(&mut meta.version, non_empty(first.get("version").and_then(Json::as_str)));
    set_if_none(&mut meta.description, non_empty(first.get("description").and_then(Json::as_str)));
    if meta.authors.is_empty() {
        meta.authors = person_names(first.get("authorList").or_else(|| first.get("authors")));
    }
    set_if_none(&mut meta.icon_entry, non_empty(first.get("logoFile").and_then(Json::as_str)));
    let ids: Vec<String> = list.iter().filter_map(|m| non_empty(m.get("modid").and_then(Json::as_str))).collect();
    meta.deps_mut("forge").provides.extend(ids);
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn read(bytes: Vec<u8>) -> LocalMeta {
        read_zip(&mut zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap(), true)
    }

    #[test]
    fn fabric_mod_json_with_nested_jars() {
        let inner = jar(&[("fabric.mod.json", br#"{"id":"fabric-api-base","version":"1"}"#)]);
        let meta = read(jar(&[
            (
                "fabric.mod.json",
                br#"{"id":"sodium","name":"Sodium","version":"0.5.8","description":"Fast",
                    "authors":["JellySquid", {"name":"IMS"}],"icon":{"16":"s.png","128":"assets/sodium/icon.png"},
                    "provides":["embeddium-compat"],
                    "depends":{"minecraft":"1.20.1","fabricloader":">=0.15","fabric-api-base":"*","indium":"*"}}"#,
            ),
            ("META-INF/jars/base.jar", &inner),
        ]));
        assert_eq!(meta.mod_id.as_deref(), Some("sodium"));
        assert_eq!(meta.name.as_deref(), Some("Sodium"));
        assert_eq!(meta.version.as_deref(), Some("0.5.8"));
        assert_eq!(meta.authors, vec!["JellySquid", "IMS"]);
        assert_eq!(meta.icon_entry.as_deref(), Some("assets/sodium/icon.png"));
        assert_eq!(meta.loaders, vec!["fabric"]);
        let deps = meta.deps_for(LoaderKind::Fabric).unwrap();
        assert_eq!(deps.provides, vec!["embeddium-compat", "fabric-api-base", "sodium"]);
        // Loader ids and ids the jar bundles itself are never "missing".
        assert_eq!(deps.depends, vec!["indium"]);
    }

    #[test]
    fn quilt_mod_json() {
        let mut meta = LocalMeta::default();
        apply_quilt(
            &mut meta,
            r#"{"quilt_loader":{"id":"qsl","version":"7","metadata":{"name":"QSL","contributors":{"Ennui":"Owner"},
               "icon":"q.png"},"depends":["quilt_loader",{"id":"opt","optional":true},{"id":"needed"}]}}"#,
        );
        assert_eq!(meta.name.as_deref(), Some("QSL"));
        assert_eq!(meta.authors, vec!["Ennui"]);
        assert_eq!(meta.deps_for(LoaderKind::Quilt).unwrap().depends, vec!["quilt_loader", "needed"]);
    }

    #[test]
    fn forge_mods_toml_resolves_the_jar_version_and_required_client_deps() {
        let toml = r#"
            modLoader="javafml"
            logoFile="logo.png"
            [[mods]]
            modId="create"
            version="${file.jarVersion}"
            displayName="Create"
            authors="simibubi, Zelo"
            description='''Building tools'''
            [[dependencies.create]]
            modId="forge"
            mandatory=true
            [[dependencies.create]]
            modId="flywheel"
            mandatory=true
            side="CLIENT"
            [[dependencies.create]]
            modId="jei"
            mandatory=false
            [[dependencies.create]]
            modId="serverthing"
            mandatory=true
            side="SERVER"
        "#;
        let meta = read(jar(&[
            ("META-INF/mods.toml", toml.as_bytes()),
            ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\nImplementation-Version: 0.5.1f\r\n"),
        ]));
        assert_eq!(meta.version.as_deref(), Some("0.5.1f"));
        assert_eq!(meta.authors, vec!["simibubi", "Zelo"]);
        assert_eq!(meta.icon_entry.as_deref(), Some("logo.png"));
        assert_eq!(meta.loaders, vec!["forge"]);
        assert_eq!(meta.deps_for(LoaderKind::Forge).unwrap().depends, vec!["flywheel"]);
    }

    #[test]
    fn neoforge_mods_toml_uses_dependency_types() {
        let mut meta = LocalMeta::default();
        apply_mods_toml(
            &mut meta,
            r#"[[mods]]
               modId="ae2"
               version="19.0"
               [[dependencies.ae2]]
               modId="guideme"
               type="required"
               [[dependencies.ae2]]
               modId="jei"
               type="optional"
               [[dependencies.ae2]]
               modId="embeddium"
               type="incompatible"
               versionRange="[0.0.1,)"
               [[dependencies.ae2]]
               modId="oldlib"
               type="incompatible"
               versionRange="(,1.6.2)""#,
            "neoforge",
            None,
        );
        assert_eq!(meta.version.as_deref(), Some("19.0"));
        let deps = meta.deps_for(LoaderKind::NeoForge).unwrap();
        assert_eq!(deps.depends, vec!["guideme"]);
        assert_eq!(
            deps.breaks,
            vec![
                BreakRule { id: "embeddium".into(), versions: None, maven: false },
                BreakRule { id: "oldlib".into(), versions: Some("(,1.6.2)".into()), maven: true },
            ]
        );
    }

    #[test]
    fn multi_loader_jars_only_count_the_instance_loaders_dependencies() {
        let meta = read(jar(&[
            ("fabric.mod.json", br#"{"id":"collective","depends":{"fabric":"*"},"breaks":{"optifine":"*"}}"#),
            (
                "META-INF/neoforge.mods.toml",
                br#"[[mods]]
                    modId="collective"
                    [[dependencies.collective]]
                    modId="neoforge"
                    type="required""#,
            ),
        ]));
        assert_eq!(meta.loaders, vec!["fabric", "neoforge"]);
        let neo = meta.deps_for(LoaderKind::NeoForge).unwrap();
        assert!(neo.depends.is_empty() && neo.breaks.is_empty());
        assert_eq!(meta.deps_for(LoaderKind::Quilt).unwrap().depends, vec!["fabric"]);
        assert_eq!(meta.deps_for(LoaderKind::Fabric).unwrap().breaks, vec![BreakRule::any("optifine".into())]);
        assert!(meta.deps_for(LoaderKind::Forge).is_none());
        assert!(meta.deps_for(LoaderKind::Vanilla).is_none());
    }

    #[test]
    fn fabric_break_predicates_are_kept() {
        let mut meta = LocalMeta::default();
        apply_fabric(&mut meta, r#"{"id":"a","breaks":{"b":"<0.5","c":["1.0","2.0"],"d":"*"}}"#);
        let breaks = &meta.deps_for(LoaderKind::Fabric).unwrap().breaks;
        assert_eq!(breaks[0].versions.as_deref(), Some("<0.5"));
        assert_eq!(breaks[1].versions.as_deref(), Some("1.0 || 2.0"));
        assert_eq!(breaks[2], BreakRule::any("d".into()));
    }

    #[test]
    fn a_bare_library_jar_provides_the_mods_it_bundles() {
        let inner = jar(&[("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"kotlinforforge\"\n")]);
        let meta = read(jar(&[
            ("META-INF/MANIFEST.MF", b"FMLModType: LIBRARY\r\n"),
            ("META-INF/jarjar/kffmod.jar", &inner),
        ]));
        assert_eq!(meta.deps_for(LoaderKind::NeoForge).unwrap().provides, vec!["kotlinforforge"]);
    }

    #[test]
    fn legacy_mcmod_info() {
        let meta = read(jar(&[(
            "mcmod.info",
            br#"[{"modid":"jei","name":"Just Enough Items","version":"4.16","authorList":["mezz"]}]"#,
        )]));
        assert_eq!(meta.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(meta.authors, vec!["mezz"]);
        assert_eq!(meta.loaders, vec!["forge"]);
    }

    #[test]
    fn resource_pack_description_is_flattened_and_unformatted() {
        let meta = read(jar(&[
            ("pack.mcmeta", r#"{"pack":{"pack_format":15,"description":[{"text":"§6Faithful "},"32x"]}}"#.as_bytes()),
            ("pack.png", b"png"),
        ]));
        assert_eq!(meta.description.as_deref(), Some("Faithful 32x"));
        assert_eq!(meta.icon_entry.as_deref(), Some("pack.png"));
        assert!(meta.loaders.is_empty());
    }

    #[test]
    fn broken_files_give_empty_metadata() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.jar"), b"not a zip").unwrap();
        assert_eq!(read_archive(&dir.path().join("x.jar")), LocalMeta::default());
        assert_eq!(read(jar(&[("fabric.mod.json", b"{oops")])).loaders, Vec::<String>::new());
    }
}
