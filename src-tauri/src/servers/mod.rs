//! An instance's multiplayer server list: the game's own `servers.dat`
//! (uncompressed NBT), read and edited in place so the launcher and the
//! in-game list always agree, plus a status ping ([`ping`]).

mod motd;
pub mod ping;

use std::collections::HashMap;
use std::path::Path;

use fastnbt::Value;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::util::fs::write_atomic;

const SERVERS_FILE: &str = "servers.dat";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    /// Base64 PNG the game cached from the server's last ping.
    pub icon: Option<String>,
}

/// `host`, `host:port` or `[ipv6]:port`, as typed in the game.
pub fn validate_address(address: &str) -> AppResult<()> {
    let address = address.trim();
    let valid = !address.is_empty()
        && address.len() <= 255
        && !address.chars().any(|c| c.is_whitespace() || c.is_control() || c == '/')
        && parse_address(address).is_some();
    if valid {
        Ok(())
    } else {
        Err(AppError::Other(format!("adresse de serveur invalide : {address}")))
    }
}

/// Splits `address` into host and explicit port (none = default / SRV).
pub fn parse_address(address: &str) -> Option<(String, Option<u16>)> {
    let address = address.trim();
    if let Some(rest) = address.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p.parse().ok()?),
            None if tail.is_empty() => None,
            None => return None,
        };
        return (!host.is_empty()).then(|| (host.to_string(), port));
    }
    match address.rsplit_once(':') {
        // More than one colon without brackets: a bare IPv6 address.
        Some((host, _)) if host.contains(':') => Some((address.to_string(), None)),
        Some((host, port)) => {
            let port = port.parse::<u16>().ok()?;
            (!host.is_empty()).then(|| (host.to_string(), Some(port)))
        }
        None => (!address.is_empty()).then(|| (address.to_string(), None)),
    }
}

fn read_root(dir: &Path) -> AppResult<HashMap<String, Value>> {
    let path = dir.join(SERVERS_FILE);
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(HashMap::new());
    };
    match fastnbt::from_bytes::<Value>(&bytes) {
        Ok(Value::Compound(root)) => Ok(root),
        _ => Err(AppError::Other("servers.dat est illisible".to_string())),
    }
}

fn write_root(dir: &Path, root: HashMap<String, Value>) -> AppResult<()> {
    let bytes = fastnbt::to_bytes(&Value::Compound(root))
        .map_err(|e| AppError::Other(format!("écriture de servers.dat impossible : {e}")))?;
    write_atomic(&dir.join(SERVERS_FILE), &bytes)?;
    Ok(())
}

fn server_list(root: &mut HashMap<String, Value>) -> &mut Vec<Value> {
    let entry = root.entry("servers".to_string()).or_insert_with(|| Value::List(Vec::new()));
    if !matches!(entry, Value::List(_)) {
        *entry = Value::List(Vec::new());
    }
    match entry {
        Value::List(list) => list,
        _ => unreachable!("just replaced with a list"),
    }
}

fn string_field(compound: &HashMap<String, Value>, key: &str) -> Option<String> {
    match compound.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Servers saved in the instance, in the in-game order.
pub fn list(dir: &Path) -> AppResult<Vec<ServerEntry>> {
    let mut root = read_root(dir)?;
    Ok(server_list(&mut root)
        .iter()
        .filter_map(|value| match value {
            Value::Compound(c) => Some(ServerEntry {
                name: string_field(c, "name").unwrap_or_else(|| "Serveur Minecraft".to_string()),
                address: string_field(c, "ip")?,
                icon: string_field(c, "icon").filter(|i| !i.is_empty()),
            }),
            _ => None,
        })
        .collect())
}

fn clean_name(name: &str) -> String {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(64).collect();
    if name.is_empty() {
        "Serveur Minecraft".to_string()
    } else {
        name
    }
}

/// Appends a server (at the bottom, like the game does).
pub fn add(dir: &Path, name: &str, address: &str) -> AppResult<()> {
    validate_address(address)?;
    let mut root = read_root(dir)?;
    let entry = HashMap::from([
        ("name".to_string(), Value::String(clean_name(name))),
        ("ip".to_string(), Value::String(address.trim().to_string())),
    ]);
    server_list(&mut root).push(Value::Compound(entry));
    write_root(dir, root)
}

/// Renames / re-addresses the server at `index`, keeping its other fields
/// (icon, resource-pack choice); the icon is dropped when the address changes.
pub fn update(dir: &Path, index: usize, name: &str, address: &str) -> AppResult<()> {
    validate_address(address)?;
    let mut root = read_root(dir)?;
    let Some(Value::Compound(entry)) = server_list(&mut root).get_mut(index) else {
        return Err(AppError::Other("ce serveur n'existe plus".to_string()));
    };
    let address = address.trim().to_string();
    if string_field(entry, "ip").as_deref() != Some(address.as_str()) {
        entry.remove("icon");
    }
    entry.insert("name".to_string(), Value::String(clean_name(name)));
    entry.insert("ip".to_string(), Value::String(address));
    write_root(dir, root)
}

pub fn remove(dir: &Path, index: usize) -> AppResult<()> {
    let mut root = read_root(dir)?;
    let list = server_list(&mut root);
    if index >= list.len() {
        return Err(AppError::Other("ce serveur n'existe plus".to_string()));
    }
    list.remove(index);
    write_root(dir, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_address_form() {
        assert_eq!(parse_address("play.example.net"), Some(("play.example.net".into(), None)));
        assert_eq!(parse_address("play.example.net:25570"), Some(("play.example.net".into(), Some(25570))));
        assert_eq!(parse_address("[::1]:25565"), Some(("::1".into(), Some(25565))));
        assert_eq!(parse_address("2001:db8::1"), Some(("2001:db8::1".into(), None)));
        assert_eq!(parse_address("host:notaport"), None);
        assert_eq!(parse_address(":25565"), None);
    }

    #[test]
    fn rejects_bad_addresses() {
        assert!(validate_address("play.example.net").is_ok());
        for bad in ["", "   ", "has space.net", "a/b", "host:99999"] {
            assert!(validate_address(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn add_update_remove_round_trip_through_nbt() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).unwrap().is_empty());

        add(dir.path(), "Hypixel", "mc.hypixel.net").unwrap();
        add(dir.path(), "  ", "localhost:25566").unwrap();
        let servers = list(dir.path()).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0], ServerEntry { name: "Hypixel".into(), address: "mc.hypixel.net".into(), icon: None });
        assert_eq!(servers[1].name, "Serveur Minecraft");

        update(dir.path(), 1, "Local", "localhost:25567").unwrap();
        assert_eq!(list(dir.path()).unwrap()[1].address, "localhost:25567");

        remove(dir.path(), 0).unwrap();
        let servers = list(dir.path()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Local");
        assert!(remove(dir.path(), 5).is_err());
    }

    #[test]
    fn keeps_unknown_fields_and_drops_the_icon_on_address_change() {
        let dir = tempfile::tempdir().unwrap();
        let entry = HashMap::from([
            ("name".to_string(), Value::String("A".into())),
            ("ip".to_string(), Value::String("a.net".into())),
            ("icon".to_string(), Value::String("iVBOR".into())),
            ("acceptTextures".to_string(), Value::Byte(1)),
        ]);
        let root = HashMap::from([("servers".to_string(), Value::List(vec![Value::Compound(entry)]))]);
        write_root(dir.path(), root).unwrap();
        assert_eq!(list(dir.path()).unwrap()[0].icon.as_deref(), Some("iVBOR"));

        update(dir.path(), 0, "A", "b.net").unwrap();
        let mut root = read_root(dir.path()).unwrap();
        let Value::Compound(entry) = &server_list(&mut root)[0] else { panic!() };
        assert!(entry.get("icon").is_none());
        assert_eq!(entry.get("acceptTextures"), Some(&Value::Byte(1)));
    }
}
