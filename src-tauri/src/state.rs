//! Tauri-managed application state, shared across all command handlers.
//! Built inside the `setup` hook (not before `.manage()`), since it needs an
//! `AppHandle` to resolve the app data directory.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tauri::AppHandle;

use crate::auth::AccountSession;
use crate::download::DownloadManager;
use crate::error::AppResult;
use crate::java::JavaManager;
use crate::launch::RunningChild;
use crate::modloaders::fabric::FabricInstaller;
use crate::modloaders::forge::ForgeInstaller;
use crate::modloaders::neoforge::NeoForgeInstaller;
use crate::modloaders::quilt::QuiltInstaller;
use crate::modloaders::LoaderRegistry;
use crate::paths::{self, AppPaths};
use crate::providers::curseforge::CurseForgeProvider;
use crate::providers::ftb::FtbProvider;
use crate::providers::ProviderRegistry;
use crate::settings::GlobalSettings;

pub struct AppState {
    pub paths: AppPaths,
    pub client: reqwest::Client,
    pub downloader: DownloadManager,
    pub java: JavaManager,
    pub providers: ProviderRegistry,
    pub loaders: LoaderRegistry,
    pub settings: RwLock<GlobalSettings>,
    pub active_account: RwLock<Option<AccountSession>>,
    pub curseforge_api_key: Arc<RwLock<String>>,
    pub running: Arc<Mutex<HashMap<String, RunningChild>>>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> AppResult<Self> {
        let app_paths = AppPaths::new(app);
        paths::ensure_dir(app_paths.root())?;
        paths::ensure_dir(&app_paths.instances_dir())?;
        paths::ensure_dir(&app_paths.cache_dir())?;

        let client = reqwest::Client::builder()
            .user_agent(concat!("LargyLauncher/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("building the shared HTTP client cannot fail with this config");

        let downloader = DownloadManager::new(client.clone());
        let java = JavaManager::new(client.clone());
        let settings = GlobalSettings::load(&app_paths)?;
        let curseforge_api_key = Arc::new(RwLock::new(settings.curseforge_api_key.clone()));

        let mut providers = ProviderRegistry::new();
        providers.register(Box::new(FtbProvider::new(client.clone())));
        providers.register(Box::new(CurseForgeProvider::new(
            client.clone(),
            curseforge_api_key.clone(),
            app_paths.cache_dir().join("curseforge"),
        )));

        let mut loaders = LoaderRegistry::new();
        loaders.register(Box::new(FabricInstaller::new(client.clone(), app_paths.libraries_dir())));
        loaders.register(Box::new(QuiltInstaller::new(client.clone(), app_paths.libraries_dir())));
        loaders.register(Box::new(ForgeInstaller::new(client.clone(), java.clone(), app_paths.clone())));
        loaders.register(Box::new(NeoForgeInstaller::new(client.clone(), java.clone(), app_paths.clone())));

        Ok(Self {
            paths: app_paths,
            client,
            downloader,
            java,
            providers,
            loaders,
            settings: RwLock::new(settings),
            active_account: RwLock::new(None),
            curseforge_api_key,
            running: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
