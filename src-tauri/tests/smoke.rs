//! End-to-end checks against the real services (Mojang, Fabric, Forge,
//! NeoForge, Modrinth, FTB). Network-heavy (~300 MB), so ignored by default:
//!
//! ```text
//! cargo test --test smoke -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Set `LARGY_SMOKE_DIR` to reuse a download cache between runs.

use std::path::{Path, PathBuf};

use largy_launcher_lib::download::DownloadManager;
use largy_launcher_lib::java::{self, JavaManager};
use largy_launcher_lib::minecraft::{libraries, manifest};
use largy_launcher_lib::modloaders::{LoaderContext, LoaderRegistry};
use largy_launcher_lib::paths::AppPaths;
use largy_launcher_lib::providers::ftb::FtbProvider;
use largy_launcher_lib::providers::modrinth::ModrinthProvider;
use largy_launcher_lib::providers::{LoaderKind, ModpackProvider, SearchQuery};
use largy_launcher_lib::state::build_http_client;
use largy_launcher_lib::util::http_cache::MetaCache;

struct Env {
    app: tauri::App,
    paths: AppPaths,
    meta: MetaCache,
    downloader: DownloadManager,
    java: JavaManager,
    client: reqwest::Client,
    _tmp: Option<tempfile::TempDir>,
}

fn env() -> Env {
    let app = tauri::Builder::default()
        .any_thread()
        .build(tauri::generate_context!())
        .expect("tauri app");
    let (root, tmp) = match std::env::var_os("LARGY_SMOKE_DIR") {
        Some(dir) => (PathBuf::from(dir), None),
        None => {
            let tmp = tempfile::tempdir().unwrap();
            (tmp.path().to_path_buf(), Some(tmp))
        }
    };
    let paths = AppPaths::from_root(root);
    let client = build_http_client();
    let meta = MetaCache::new(client.clone(), paths.meta_cache_dir());
    Env {
        app,
        downloader: DownloadManager::new(client.clone()),
        java: JavaManager::new(meta.clone()),
        meta,
        paths,
        client,
        _tmp: tmp,
    }
}

/// Libraries + client jar of a vanilla version (no assets); returns the
/// Java runtime component it asks for.
async fn vanilla_files(env: &Env, mc: &str) -> String {
    let entry = manifest::find_version_entry(&env.meta, mc).await.unwrap();
    let raw = manifest::fetch_version_json(&env.meta, &entry.url).await.unwrap();
    let resolved = libraries::resolve_libraries(&raw.libraries, &env.paths.libraries_dir());
    let mut items = resolved.download_items();
    let client = raw.downloads.client.as_ref().unwrap();
    items.push(largy_launcher_lib::download::DownloadItem {
        url: client.url.clone(),
        dest: env.paths.versions_dir().join(&raw.id).join(format!("{}.jar", raw.id)),
        sha1: Some(client.sha1.clone()),
        size: Some(client.size),
    });
    env.downloader.run_batch(env.app.handle(), "t", "t", items, 16).await.unwrap();
    java::resolve_component(raw.java_version.as_ref())
}

async fn install_loader(env: &Env, kind: LoaderKind, mc: &str, component: &str) -> (String, Vec<PathBuf>, String) {
    let registry = LoaderRegistry::with_defaults();
    let installer = registry.get(kind).unwrap();
    let versions = installer.list_versions(&env.meta, mc).await.unwrap();
    assert!(!versions.is_empty(), "{kind:?} {mc}: no versions");
    let version = versions[0].clone();
    let ctx = LoaderContext {
        app: env.app.handle(),
        paths: &env.paths,
        meta: &env.meta,
        downloader: &env.downloader,
        java: &env.java,
        java_component: component,
        force_reinstall: false,
    };
    let profile = installer.resolve(&ctx, mc, &version).await.unwrap_or_else(|e| panic!("{kind:?} {mc} {version}: {e}"));
    let paths: Vec<PathBuf> = profile.extra_libraries.iter().map(|l| l.path.clone()).collect();
    (version, paths, profile.main_class_override.unwrap_or_default())
}

fn assert_all_exist(paths: &[PathBuf], what: &str) {
    let missing: Vec<&Path> = paths.iter().map(PathBuf::as_path).filter(|p| !p.exists()).collect();
    assert!(missing.is_empty(), "{what}: missing libraries {missing:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn modern_loaders_install_and_java_runs() {
    let env = env();
    let component = vanilla_files(&env, "1.20.1").await;

    let runtime = env.java.ensure_runtime(env.app.handle(), &env.paths, &env.downloader, &component).await.unwrap();
    let (version, major) = java::probe(&runtime.path).await.expect("managed java must run");
    println!("java {version} ({major}) at {}", runtime.path.display());
    assert!(major >= 17);

    let (v, libs, main) = install_loader(&env, LoaderKind::Fabric, "1.20.1", &component).await;
    println!("fabric {v}: {} libs, main {main}", libs.len());
    assert_all_exist(&libs, "fabric");
    assert!(libs.iter().any(|p| p.to_string_lossy().contains("fabric-loader")));

    let (v, libs, main) = install_loader(&env, LoaderKind::Quilt, "1.20.1", &component).await;
    println!("quilt {v}: {} libs, main {main}", libs.len());
    assert_all_exist(&libs, "quilt");

    let (v, libs, main) = install_loader(&env, LoaderKind::Forge, "1.20.1", &component).await;
    println!("forge {v}: {} libs, main {main}", libs.len());
    assert_all_exist(&libs, "forge 1.20.1");
    assert_eq!(main, "cpw.mods.bootstraplauncher.BootstrapLauncher");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn neoforge_installs_for_1_21_1_and_1_20_1() {
    let env = env();
    let component = vanilla_files(&env, "1.21.1").await;
    let (v, libs, main) = install_loader(&env, LoaderKind::NeoForge, "1.21.1", &component).await;
    println!("neoforge {v}: {} libs, main {main}", libs.len());
    assert_all_exist(&libs, "neoforge 1.21.1");

    let component = vanilla_files(&env, "1.20.1").await;
    let (v, libs, _) = install_loader(&env, LoaderKind::NeoForge, "1.20.1", &component).await;
    println!("neoforge (1.20.1) {v}: {} libs", libs.len());
    assert!(v.starts_with("1.20.1-"));
    assert_all_exist(&libs, "neoforge 1.20.1");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn legacy_forge_1_12_2_installs() {
    let env = env();
    let component = vanilla_files(&env, "1.12.2").await;
    let (v, libs, main) = install_loader(&env, LoaderKind::Forge, "1.12.2", &component).await;
    println!("forge {v}: {} libs, main {main}", libs.len());
    assert_eq!(main, "net.minecraft.launchwrapper.Launch");
    assert_all_exist(&libs, "forge 1.12.2");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn modpack_providers_resolve_real_packs() {
    let env = env();

    let modrinth = ModrinthProvider::new(env.client.clone(), env.paths.cache_dir().join("modrinth"));
    let hits = modrinth
        .search(SearchQuery { text: "fabulously optimized".to_string(), offset: 0 })
        .await
        .unwrap();
    let pack = hits.first().expect("modrinth hit");
    let versions = modrinth.get_versions(&pack.id).await.unwrap();
    let resolved = modrinth.resolve_version(&pack.id, &versions[0].id).await.unwrap();
    println!(
        "modrinth {}: mc {} {:?} {} — {} files, {} warnings",
        pack.name,
        resolved.minecraft_version,
        resolved.loader,
        resolved.loader_version,
        resolved.files.len(),
        resolved.warnings.len()
    );
    assert!(!resolved.files.is_empty());
    assert!(!resolved.loader_version.is_empty());

    let ftb = FtbProvider::new(env.client.clone());
    let packs = ftb.search(SearchQuery::default()).await.unwrap();
    assert!(!packs.is_empty());
    let versions = ftb.get_versions(&packs[0].id).await.unwrap();
    let resolved = ftb.resolve_version(&packs[0].id, &versions[0].id).await.unwrap();
    let without_url = resolved.files.iter().filter(|f| f.direct_url.is_none()).count();
    println!(
        "ftb {}: mc {} {:?} — {} files ({without_url} without URL)",
        packs[0].name,
        resolved.minecraft_version,
        resolved.loader,
        resolved.files.len()
    );
    assert!(!resolved.files.is_empty());
    assert_eq!(without_url, 0);
}
