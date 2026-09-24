//! Tauri-managed application state, shared across all command handlers.
//! Built inside the `setup` hook (not before `.manage()`), since it needs an
//! `AppHandle` to resolve the app data directory.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use tauri::AppHandle;
use tokio::sync::Notify;

use crate::auth::AccountSession;
use crate::discord::DiscordPresence;
use crate::download::DownloadManager;
use crate::error::{AppError, AppResult};
use crate::java::JavaManager;
use crate::launch::RunningChild;
use crate::modloaders::LoaderRegistry;
use crate::paths::{self, AppPaths};
use crate::providers::curseforge::CurseForgeProvider;
use crate::providers::ftb::FtbProvider;
use crate::providers::modrinth::ModrinthProvider;
use crate::providers::ProviderRegistry;
use crate::settings::GlobalSettings;
use crate::util::http_cache::MetaCache;

pub struct AppState {
    pub paths: AppPaths,
    pub client: reqwest::Client,
    pub meta: MetaCache,
    pub downloader: DownloadManager,
    pub java: JavaManager,
    pub providers: ProviderRegistry,
    pub loaders: LoaderRegistry,
    pub settings: RwLock<GlobalSettings>,
    pub active_account: RwLock<Option<AccountSession>>,
    pub curseforge_api_key: Arc<RwLock<String>>,
    pub running: Arc<Mutex<HashMap<String, RunningChild>>>,
    /// Kept across calls so per-process CPU usage has a previous sample to
    /// diff against (sysinfo computes it between two refreshes).
    pub system: Mutex<sysinfo::System>,
    /// Signalled to abort the in-flight Microsoft device-code poll.
    pub login_cancel: Mutex<Option<Arc<Notify>>>,
    /// Modpack installs/updates in flight, keyed by instance id (or pack
    /// install token), each cancellable from the UI.
    pub installs: Mutex<HashMap<String, Arc<Notify>>>,
    /// Instance a desktop shortcut asked to launch at startup, until the UI
    /// picks it up.
    pub pending_launch: Mutex<Option<String>>,
    pub discord: Arc<DiscordPresence>,
}

/// Shared HTTP client. Connect/read timeouts (not a total timeout — a
/// 200 MB modpack legitimately takes minutes) so a stalled connection
/// fails and gets retried instead of hanging a launch forever.
pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!(
            "Largy-dev/Largy-Launcher/",
            env!("CARGO_PKG_VERSION"),
            " (github.com/Largy-dev/Largy-Launcher)"
        ))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(45))
        .pool_idle_timeout(Duration::from_secs(60))
        .build()
        .expect("building the shared HTTP client cannot fail with this config")
}

impl AppState {
    pub fn new(app: &AppHandle) -> AppResult<Self> {
        let app_paths = AppPaths::new(app);
        paths::ensure_dir(app_paths.root())?;
        paths::ensure_dir(&app_paths.instances_dir())?;
        paths::ensure_dir(&app_paths.cache_dir())?;

        let client = build_http_client();
        let meta = MetaCache::new(client.clone(), app_paths.meta_cache_dir());
        let downloader = DownloadManager::new(client.clone());
        let java = JavaManager::new(meta.clone());
        let settings = GlobalSettings::load(&app_paths)?;
        let curseforge_api_key = Arc::new(RwLock::new(settings.curseforge_api_key.clone()));
        let discord = Arc::new(DiscordPresence::new(settings.discord_rich_presence));

        let mut providers = ProviderRegistry::new();
        providers.register(Box::new(ModrinthProvider::new(client.clone(), app_paths.cache_dir().join("modrinth"))));
        providers.register(Box::new(FtbProvider::new(client.clone())));
        providers.register(Box::new(CurseForgeProvider::new(
            client.clone(),
            curseforge_api_key.clone(),
            app_paths.cache_dir().join("curseforge"),
        )));

        Ok(Self {
            paths: app_paths,
            client,
            meta,
            downloader,
            java,
            providers,
            loaders: LoaderRegistry::with_defaults(),
            settings: RwLock::new(settings),
            active_account: RwLock::new(None),
            curseforge_api_key,
            running: Arc::new(Mutex::new(HashMap::new())),
            system: Mutex::new(sysinfo::System::new()),
            login_cancel: Mutex::new(None),
            installs: Mutex::new(HashMap::new()),
            pending_launch: Mutex::new(None),
            discord,
        })
    }

    /// Registers a cancellable install under `key`; fails if one is already
    /// running for it (double-clicked install/update).
    pub fn begin_install(&self, key: &str) -> AppResult<InstallGuard<'_>> {
        let mut installs = self.installs.lock();
        if installs.contains_key(key) {
            return Err(AppError::Instance("une installation est déjà en cours pour cette instance".to_string()));
        }
        let cancel = Arc::new(Notify::new());
        installs.insert(key.to_string(), cancel.clone());
        Ok(InstallGuard { state: self, key: key.to_string(), cancel })
    }
}

pub struct InstallGuard<'a> {
    state: &'a AppState,
    key: String,
    pub cancel: Arc<Notify>,
}

impl Drop for InstallGuard<'_> {
    fn drop(&mut self) {
        self.state.installs.lock().remove(&self.key);
    }
}

/// Runs `fut` unless `cancel` fires first, in which case the future is
/// dropped (aborting every download task it owns) and `Cancelled` returned.
pub async fn cancellable<T>(cancel: &Notify, fut: impl std::future::Future<Output = AppResult<T>>) -> AppResult<T> {
    tokio::select! {
        result = fut => result,
        _ = cancel.notified() => Err(AppError::Cancelled),
    }
}
