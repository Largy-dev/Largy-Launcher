//! Game stdout/stderr → frontend. Readers never stop on bad bytes (a stopped
//! reader leaves the pipe full and eventually freezes the game), lines are
//! batched into one `instance-log` event every [`FLUSH_INTERVAL`], and the
//! last lines are kept for crash analysis.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const MAX_BATCH: usize = 1000;
/// Enough to catch a crash trace without holding a long session's log.
pub const LOG_BUFFER_CAPACITY: usize = 1000;
/// Guards against a mod printing a multi-megabyte "line".
const MAX_LINE_BYTES: usize = 16 * 1024;

pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;

pub fn new_log_buffer() -> LogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(LOG_BUFFER_CAPACITY)))
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub line: String,
    pub stream: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceLogBatch {
    pub instance_id: String,
    pub lines: Vec<LogLine>,
}

/// Starts one reader per stream plus a batching emitter. The returned handle
/// completes once both streams hit EOF and the last batch was sent — await
/// it before reporting the exit so no log line arrives after the exit event.
pub fn stream_output(
    app: &AppHandle,
    instance_id: &str,
    child: &mut Child,
    buffer: LogBuffer,
    xml_logs: bool,
    secrets: Vec<String>,
) -> JoinHandle<()> {
    let (tx, rx) = mpsc::unbounded_channel::<LogLine>();
    let secrets = Arc::new(secrets.into_iter().filter(|s| s.len() >= 8).collect::<Vec<_>>());
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, "stdout", tx.clone(), xml_logs, secrets.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, "stderr", tx, false, secrets);
    }
    tokio::spawn(emit_batches(app.clone(), instance_id.to_string(), rx, buffer))
}

fn spawn_reader<R>(reader: R, stream: &'static str, tx: mpsc::UnboundedSender<LogLine>, xml: bool, secrets: Arc<Vec<String>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::with_capacity(64 * 1024, reader);
        let mut raw = Vec::with_capacity(512);
        let mut decoder = xml.then(Log4jXmlDecoder::default);
        loop {
            raw.clear();
            match reader.read_until(b'\n', &mut raw).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("log reader ({stream}) stopped: {e}");
                    break;
                }
            }
            if raw.len() > MAX_LINE_BYTES {
                raw.truncate(MAX_LINE_BYTES);
            }
            let text = String::from_utf8_lossy(&raw);
            let text = text.trim_end_matches(['\n', '\r']);
            let lines = match decoder.as_mut() {
                Some(d) => d.feed(text),
                None => vec![text.to_string()],
            };
            for mut line in lines {
                for secret in secrets.iter() {
                    if line.contains(secret.as_str()) {
                        line = line.replace(secret.as_str(), "********");
                    }
                }
                if tx.send(LogLine { line, stream }).is_err() {
                    return;
                }
            }
        }
    });
}

async fn emit_batches(app: AppHandle, instance_id: String, mut rx: mpsc::UnboundedReceiver<LogLine>, buffer: LogBuffer) {
    let mut batch: Vec<LogLine> = Vec::new();
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let flush = |batch: &mut Vec<LogLine>| {
        if batch.is_empty() {
            return;
        }
        {
            let mut ring = buffer.lock();
            for l in batch.iter() {
                if ring.len() >= LOG_BUFFER_CAPACITY {
                    ring.pop_front();
                }
                ring.push_back(l.line.clone());
            }
        }
        let _ = app.emit("instance-log", InstanceLogBatch { instance_id: instance_id.clone(), lines: std::mem::take(batch) });
    };
    loop {
        tokio::select! {
            line = rx.recv() => match line {
                Some(line) => {
                    batch.push(line);
                    if batch.len() >= MAX_BATCH {
                        flush(&mut batch);
                    }
                }
                None => break,
            },
            _ = ticker.tick() => flush(&mut batch),
        }
    }
    flush(&mut batch);
}

/// Turns log4j `<log4j:Event>` XML (what Mojang's hardened logging config
/// prints for old log4j versions) back into vanilla-style text lines:
/// `[HH:MM:SS] [thread/LEVEL]: message`.
#[derive(Default)]
pub struct Log4jXmlDecoder {
    event: Option<String>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn cdata_of<'a>(event: &'a str, element: &str) -> Option<&'a str> {
    let open = event.find(&format!("<log4j:{element}>"))?;
    let rest = &event[open..];
    let start = rest.find("<![CDATA[")? + "<![CDATA[".len();
    let end = rest[start..].find("]]>")? + start;
    Some(&rest[start..end])
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

impl Log4jXmlDecoder {
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        let trimmed = line.trim_start();
        match &mut self.event {
            None if trimmed.starts_with("<log4j:Event") => {
                self.event = Some(line.to_string());
            }
            None => return vec![line.to_string()],
            Some(event) => {
                event.push('\n');
                event.push_str(line);
            }
        }
        let complete = self.event.as_ref().is_some_and(|e| e.contains("</log4j:Event>"));
        if !complete {
            return Vec::new();
        }
        let event = self.event.take().unwrap_or_default();
        Self::render(&event)
    }

    fn render(event: &str) -> Vec<String> {
        let tag_end = event.find('>').unwrap_or(event.len());
        let tag = &event[..tag_end];
        let level = attr(tag, "level").unwrap_or("INFO");
        let thread = unescape(attr(tag, "thread").unwrap_or("main"));
        let time = attr(tag, "timestamp")
            .and_then(|t| t.parse::<i64>().ok())
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "00:00:00".to_string());
        let message = cdata_of(event, "Message").unwrap_or("");

        let mut lines: Vec<String> = Vec::new();
        let mut message_lines = message.lines();
        lines.push(format!("[{time}] [{thread}/{level}]: {}", message_lines.next().unwrap_or("")));
        lines.extend(message_lines.map(String::from));
        if let Some(throwable) = cdata_of(event, "Throwable") {
            lines.extend(throwable.lines().filter(|l| !l.is_empty()).map(String::from));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_events_are_rendered_as_vanilla_lines() {
        let mut d = Log4jXmlDecoder::default();
        assert!(d.feed(r#"<log4j:Event logger="bib" timestamp="1700000000000" level="WARN" thread="Client thread">"#).is_empty());
        assert!(d.feed("  <log4j:Message><![CDATA[Skipping bad option: lastServer:]]></log4j:Message>").is_empty());
        let out = d.feed("</log4j:Event>");
        assert_eq!(out.len(), 1);
        assert!(out[0].ends_with("[Client thread/WARN]: Skipping bad option: lastServer:"), "{}", out[0]);
    }

    #[test]
    fn multiline_messages_and_throwables_keep_every_line() {
        let mut d = Log4jXmlDecoder::default();
        d.feed(r#"<log4j:Event logger="x" timestamp="1700000000000" level="ERROR" thread="main">"#);
        d.feed("<log4j:Message><![CDATA[first");
        d.feed("second]]></log4j:Message>");
        d.feed("<log4j:Throwable><![CDATA[java.lang.RuntimeException: boom");
        d.feed("\tat a.b.C(C.java:1)");
        d.feed("]]></log4j:Throwable>");
        let out = d.feed("</log4j:Event>");
        assert_eq!(out.len(), 4);
        assert!(out[0].ends_with("[main/ERROR]: first"));
        assert_eq!(out[1], "second");
        assert_eq!(out[2], "java.lang.RuntimeException: boom");
    }

    #[test]
    fn plain_lines_outside_events_pass_through() {
        let mut d = Log4jXmlDecoder::default();
        assert_eq!(d.feed("Exception in thread main"), vec!["Exception in thread main".to_string()]);
    }
}
