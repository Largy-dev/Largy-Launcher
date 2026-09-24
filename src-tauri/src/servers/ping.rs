//! Server List Ping (the status request the multiplayer screen sends):
//! handshake → status JSON → ping/pong for the latency. Addresses without a
//! port go through the `_minecraft._tcp` SRV record first, like the game.

use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{AppError, AppResult};

const DEFAULT_PORT: u16 = 25565;
const TIMEOUT: Duration = Duration::from_secs(5);
/// Any recent protocol works for a status request.
const PROTOCOL_VERSION: i32 = 767;
/// Status responses carry a base64 favicon; anything past this is not a server.
const MAX_PACKET: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub online: u32,
    pub max: u32,
    pub version: String,
    /// `§`-coded description, see [`super::motd`].
    pub motd: String,
    /// `data:image/png;base64,...` as the server sends it.
    pub favicon: Option<String>,
    pub latency_ms: u32,
    /// Sample of connected player names (servers may hide or fake it).
    pub players: Vec<String>,
}

#[derive(Deserialize)]
struct StatusJson {
    #[serde(default)]
    version: Option<VersionJson>,
    #[serde(default)]
    players: Option<PlayersJson>,
    #[serde(default)]
    description: serde_json::Value,
    #[serde(default)]
    favicon: Option<String>,
}

#[derive(Deserialize)]
struct VersionJson {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct PlayersJson {
    #[serde(default)]
    online: u32,
    #[serde(default)]
    max: u32,
    #[serde(default)]
    sample: Vec<PlayerJson>,
}

#[derive(Deserialize)]
struct PlayerJson {
    #[serde(default)]
    name: String,
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            buf.push(value as u8);
            return;
        }
        buf.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
}

fn read_varint_from(bytes: &[u8]) -> Option<(i32, usize)> {
    let mut value: u32 = 0;
    for (i, byte) in bytes.iter().take(5).enumerate() {
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value as i32, i + 1));
        }
    }
    None
}

async fn read_varint(stream: &mut TcpStream) -> AppResult<i32> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let byte = stream.read_u8().await?;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(AppError::Other("réponse du serveur invalide".to_string()))
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(body.len() + 5);
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::with_capacity(inner.len() + 5);
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

fn handshake(host: &str, port: u16) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(&mut body, PROTOCOL_VERSION);
    write_varint(&mut body, host.len() as i32);
    body.extend_from_slice(host.as_bytes());
    body.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut body, 1);
    packet(0x00, &body)
}

/// Reads one packet, returning its id and payload.
async fn read_packet(stream: &mut TcpStream) -> AppResult<(i32, Vec<u8>)> {
    let len = read_varint(stream).await?;
    let len = usize::try_from(len).ok().filter(|&l| l > 0 && l <= MAX_PACKET);
    let Some(len) = len else {
        return Err(AppError::Other("réponse du serveur invalide".to_string()));
    };
    let mut data = vec![0; len];
    stream.read_exact(&mut data).await?;
    let (id, used) = read_varint_from(&data).ok_or_else(|| AppError::Other("paquet invalide".to_string()))?;
    Ok((id, data.split_off(used)))
}

fn parse_status(payload: &[u8]) -> AppResult<StatusJson> {
    let invalid = || AppError::Other("statut du serveur illisible".to_string());
    let (len, used) = read_varint_from(payload).ok_or_else(invalid)?;
    let json = payload.get(used..used + usize::try_from(len).map_err(|_| invalid())?).ok_or_else(invalid)?;
    serde_json::from_slice(json).map_err(|_| invalid())
}

fn resolver() -> Option<&'static hickory_resolver::TokioAsyncResolver> {
    static RESOLVER: OnceLock<Option<hickory_resolver::TokioAsyncResolver>> = OnceLock::new();
    RESOLVER
        .get_or_init(|| hickory_resolver::TokioAsyncResolver::tokio_from_system_conf().ok())
        .as_ref()
}

/// `_minecraft._tcp.<host>` target, when the domain publishes one.
async fn srv_target(host: &str) -> Option<(String, u16)> {
    let lookup = resolver()?.srv_lookup(format!("_minecraft._tcp.{host}.")).await.ok()?;
    let record = lookup.iter().min_by_key(|r| r.priority())?;
    let target = record.target().to_utf8();
    Some((target.trim_end_matches('.').to_string(), record.port()))
}

async fn status(host: &str, port: u16, connect_to: (&str, u16)) -> AppResult<ServerStatus> {
    let addrs: Vec<_> = tokio::net::lookup_host(connect_to)
        .await
        .map_err(|_| AppError::Other("adresse introuvable".to_string()))?
        .collect();
    let mut stream = TcpStream::connect(addrs.as_slice())
        .await
        .map_err(|_| AppError::Other("le serveur ne répond pas".to_string()))?;
    stream.set_nodelay(true)?;

    let started = Instant::now();
    stream.write_all(&handshake(host, port)).await?;
    stream.write_all(&packet(0x00, &[])).await?;
    let (id, payload) = read_packet(&mut stream).await?;
    if id != 0x00 {
        return Err(AppError::Other("réponse du serveur inattendue".to_string()));
    }
    let mut latency = started.elapsed();
    let json = parse_status(&payload)?;

    // Ping/pong measures the round trip alone; fall back to the status time.
    let ping_started = Instant::now();
    let pong = async {
        stream.write_all(&packet(0x01, &42i64.to_be_bytes())).await?;
        read_packet(&mut stream).await
    };
    if let Ok(Ok((0x01, _))) = tokio::time::timeout(Duration::from_secs(2), pong).await {
        latency = ping_started.elapsed();
    }

    let players = json.players.unwrap_or(PlayersJson { online: 0, max: 0, sample: Vec::new() });
    Ok(ServerStatus {
        online: players.online,
        max: players.max,
        version: json.version.map(|v| super::motd::to_legacy(&serde_json::Value::String(v.name))).unwrap_or_default(),
        motd: super::motd::to_legacy(&json.description),
        favicon: json.favicon.filter(|f| f.starts_with("data:image/png;base64,")),
        latency_ms: latency.as_millis().min(u128::from(u32::MAX)) as u32,
        players: players.sample.into_iter().map(|p| p.name).filter(|n| !n.is_empty()).take(12).collect(),
    })
}

/// Pings `address` (`host[:port]`), failing after a few seconds.
pub async fn ping(address: &str) -> AppResult<ServerStatus> {
    super::validate_address(address)?;
    let (host, port) = super::parse_address(address)
        .ok_or_else(|| AppError::Other(format!("adresse de serveur invalide : {address}")))?;
    let run = async {
        let (target, target_port) = match port {
            Some(port) => (host.clone(), port),
            None if host.parse::<IpAddr>().is_ok() => (host.clone(), DEFAULT_PORT),
            None => srv_target(&host).await.unwrap_or_else(|| (host.clone(), DEFAULT_PORT)),
        };
        status(&target, target_port, (target.as_str(), target_port)).await
    };
    match tokio::time::timeout(TIMEOUT, run).await {
        Ok(Err(AppError::Io(_))) => Err(AppError::Other("la connexion a été coupée".to_string())),
        Ok(result) => result,
        Err(_) => Err(AppError::Other("le serveur ne répond pas".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for value in [0, 1, 127, 128, 255, 25565, 2_097_151, i32::MAX, -1] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            assert_eq!(read_varint_from(&buf), Some((value, buf.len())), "{value}");
        }
    }

    #[test]
    fn handshake_matches_the_protocol_layout() {
        let bytes = handshake("a.b", 25565);
        // length, id 0, protocol 767 (2-byte varint), "a.b", port, next state 1
        assert_eq!(bytes, vec![10, 0x00, 0xff, 0x05, 3, b'a', b'.', b'b', 0x63, 0xdd, 1]);
    }

    #[test]
    fn parses_a_status_payload() {
        let json = r#"{"version":{"name":"Paper 1.21"},"players":{"online":3,"max":20,"sample":[{"name":"Steve","id":"x"}]},"description":{"text":"Hi"}}"#;
        let mut payload = Vec::new();
        write_varint(&mut payload, json.len() as i32);
        payload.extend_from_slice(json.as_bytes());
        let status = parse_status(&payload).unwrap();
        assert_eq!(status.players.as_ref().unwrap().online, 3);
        assert_eq!(status.players.unwrap().sample[0].name, "Steve");
        assert_eq!(super::super::motd::to_legacy(&status.description), "Hi");
    }

    #[tokio::test]
    async fn pings_a_fake_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_packet(&mut socket).await.unwrap(); // handshake
            read_packet(&mut socket).await.unwrap(); // status request
            let json = r#"{"version":{"name":"1.21"},"players":{"online":1,"max":10},"description":"Test"}"#;
            let mut body = Vec::new();
            write_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json.as_bytes());
            socket.write_all(&packet(0x00, &body)).await.unwrap();
            let (_, ping) = read_packet(&mut socket).await.unwrap();
            socket.write_all(&packet(0x01, &ping)).await.unwrap();
        });

        let status = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert_eq!((status.online, status.max), (1, 10));
        assert_eq!(status.motd, "Test");
        assert_eq!(status.version, "1.21");
    }
}
