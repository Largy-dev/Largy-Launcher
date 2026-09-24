//! Discord Rich Presence: "Joue à <instance>" with the session timer while a
//! game runs. The IPC client isn't `Send`, so it lives on its own thread fed
//! by a channel; Discord not running is a normal state, retried on the next
//! update rather than reported.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::instances::Instance;
use crate::providers::LoaderKind;

/// Application id of "Largy Launcher" on the Discord developer portal. Its
/// name is what Discord shows after "Joue à"; the `logo` art asset is the
/// fallback image.
const CLIENT_ID: &str = "1552588225929941104";
const LOGO_ASSET: &str = "logo";

#[derive(Debug, Clone, PartialEq)]
pub struct GameActivity {
    pub details: String,
    pub state: String,
    pub started_at: i64,
    pub image: Option<String>,
}

impl GameActivity {
    pub fn for_instance(instance: &Instance, started_at: i64) -> Self {
        let loader = match instance.loader {
            LoaderKind::Vanilla => None,
            LoaderKind::Forge => Some("Forge"),
            LoaderKind::NeoForge => Some("NeoForge"),
            LoaderKind::Fabric => Some("Fabric"),
            LoaderKind::Quilt => Some("Quilt"),
        };
        let state = match loader {
            Some(loader) => format!("Minecraft {} · {loader}", instance.minecraft_version),
            None => format!("Minecraft {}", instance.minecraft_version),
        };
        GameActivity {
            details: instance.name.chars().take(128).collect(),
            state,
            started_at,
            image: instance.icon_url.clone().filter(|url| url.starts_with("https://") && url.len() <= 256),
        }
    }
}

enum Command {
    Enable(bool),
    Started(String, GameActivity),
    Stopped(String),
}

pub struct DiscordPresence {
    tx: Option<Sender<Command>>,
}

impl DiscordPresence {
    pub fn new(enabled: bool) -> Self {
        if CLIENT_ID.is_empty() {
            return Self { tx: None };
        }
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new().name("discord-presence".into()).spawn(move || run(rx, enabled));
        if let Err(e) = spawned {
            tracing::warn!("Discord presence unavailable: {e}");
            return Self { tx: None };
        }
        Self { tx: Some(tx) }
    }

    fn send(&self, command: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(command);
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.send(Command::Enable(enabled));
    }

    pub fn game_started(&self, instance_id: &str, activity: GameActivity) {
        self.send(Command::Started(instance_id.to_string(), activity));
    }

    pub fn game_stopped(&self, instance_id: &str) {
        self.send(Command::Stopped(instance_id.to_string()));
    }
}

/// The activity to show: the most recently started game.
fn current(games: &HashMap<String, GameActivity>) -> Option<&GameActivity> {
    games.values().max_by_key(|g| g.started_at)
}

struct Connection {
    client: Option<DiscordIpcClient>,
}

impl Connection {
    fn client(&mut self) -> Option<&mut DiscordIpcClient> {
        if self.client.is_none() {
            let mut client = DiscordIpcClient::new(CLIENT_ID).ok()?;
            client.connect().ok()?;
            self.client = Some(client);
        }
        self.client.as_mut()
    }

    fn drop_client(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
        }
    }

    /// Shows `activity` (or clears it), reconnecting once if Discord was
    /// restarted since the last update.
    fn show(&mut self, activity: Option<&GameActivity>) {
        for _ in 0..2 {
            let Some(client) = self.client() else {
                return;
            };
            let result = match activity {
                Some(game) => {
                    let assets = Assets::new()
                        .large_image(game.image.as_deref().unwrap_or(LOGO_ASSET))
                        .large_text(&game.details);
                    let payload = Activity::new()
                        .details(&game.details)
                        .state(&game.state)
                        .timestamps(Timestamps::new().start(game.started_at))
                        .assets(assets);
                    client.set_activity(payload)
                }
                None => client.clear_activity(),
            };
            if result.is_ok() {
                return;
            }
            self.drop_client();
        }
    }
}

fn run(rx: Receiver<Command>, mut enabled: bool) {
    let mut games: HashMap<String, GameActivity> = HashMap::new();
    let mut connection = Connection { client: None };
    while let Ok(command) = rx.recv() {
        match command {
            Command::Enable(on) => enabled = on,
            Command::Started(id, activity) => {
                games.insert(id, activity);
            }
            Command::Stopped(id) => {
                games.remove(&id);
            }
        }
        match (enabled, current(&games)) {
            (true, Some(game)) => {
                let game = game.clone();
                connection.show(Some(&game));
            }
            // Nothing to show: disconnect so Discord drops the presence entirely.
            _ if connection.client.is_some() => {
                connection.show(None);
                connection.drop_client();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(loader: LoaderKind, icon: Option<&str>) -> Instance {
        let mut instance: Instance = serde_json::from_value(serde_json::json!({
            "id": "a", "name": "All the Mods 9", "minecraft_version": "1.20.1",
            "loader": "vanilla", "loader_version": null, "directory": ""
        }))
        .unwrap();
        instance.loader = loader;
        instance.icon_url = icon.map(str::to_string);
        instance
    }

    #[test]
    fn describes_the_instance() {
        let activity = GameActivity::for_instance(&instance(LoaderKind::NeoForge, Some("https://cdn/x.png")), 5);
        assert_eq!(activity.details, "All the Mods 9");
        assert_eq!(activity.state, "Minecraft 1.20.1 · NeoForge");
        assert_eq!(activity.image.as_deref(), Some("https://cdn/x.png"));
        assert_eq!(GameActivity::for_instance(&instance(LoaderKind::Vanilla, None), 5).state, "Minecraft 1.20.1");
    }

    #[test]
    fn only_https_icons_are_used() {
        assert_eq!(GameActivity::for_instance(&instance(LoaderKind::Fabric, Some("file:///x.png")), 0).image, None);
    }

    #[test]
    fn shows_the_latest_game() {
        let mut games = HashMap::new();
        let game = |details: &str, started_at| GameActivity {
            details: details.to_string(),
            state: String::new(),
            started_at,
            image: None,
        };
        assert!(current(&games).is_none());
        games.insert("a".to_string(), game("A", 10));
        games.insert("b".to_string(), game("B", 20));
        assert_eq!(current(&games).unwrap().details, "B");
    }
}
