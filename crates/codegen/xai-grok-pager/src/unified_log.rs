//! Unified log forwarding for the pager.
//!
//! Buffers log entries in memory and flushes them to the shell via
//! `x.ai/log` ACP notifications. Call [`init`] once at startup with
//! the ACP sender, then use [`info`], [`warn`], [`error`], [`debug`]
//! from anywhere.

use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;
use tokio::runtime::Handle;
use xai_acp_lib::AcpAgentTx;
use xai_grok_telemetry::unified_log::{
    ClientLogEntry, LOG_METHOD, LogLevel, LogNotificationParams, LogSource,
};

static ACP_TX: Mutex<Option<AcpAgentTx>> = Mutex::new(None);
static BUFFER: Mutex<Vec<ClientLogEntry>> = Mutex::new(Vec::new());
static FLUSH_LOOP_INIT: OnceLock<()> = OnceLock::new();

/// Initialize the unified log forwarder with the ACP sender.
///
/// Call this after the ACP connection is established and again after
/// reconnects to rebind the sender.
/// Spawns a background task that flushes buffered entries every few
/// seconds so events are delivered promptly without manual flush calls.
/// Entries buffered before this call will be picked up on the first tick.
pub fn init(tx: AcpAgentTx) {
    set_sender(tx);
    FLUSH_LOOP_INIT.get_or_init(|| {
        tokio::spawn(async {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                flush();
            }
        });
    });
}

/// Replace the ACP sender used for future log flushes.
pub fn set_sender(tx: AcpAgentTx) {
    if let Ok(mut current) = ACP_TX.lock() {
        *current = Some(tx);
    }
}

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn push_entry(lvl: LogLevel, msg: &str, sid: Option<&str>, ctx: Option<serde_json::Value>) {
    let entry = ClientLogEntry {
        ts: now_ts(),
        pid: Some(std::process::id()),
        ver: Some(xai_grok_version::VERSION.to_owned()),
        lvl,
        sid: sid.map(Into::into),
        msg: msg.into(),
        ctx,
    };
    if let Ok(mut buf) = BUFFER.lock() {
        buf.push(entry);
        // Auto-flush when we have a reasonable batch, but only if the ACP
        // sender is ready -- otherwise keep buffering until an explicit flush().
        if buf.len() >= 16 && current_sender().is_some() {
            let entries: Vec<ClientLogEntry> = buf.drain(..).collect();
            drop(buf);
            send_entries(entries);
        }
    }
}

fn current_sender() -> Option<AcpAgentTx> {
    ACP_TX.lock().ok()?.as_ref().cloned()
}

fn build_notification(entries: Vec<ClientLogEntry>) -> Option<acp::ExtNotification> {
    if entries.is_empty() {
        return None;
    }
    let params = LogNotificationParams {
        src: LogSource::GrokPager,
        entries,
    };
    let raw = serde_json::value::to_raw_value(&params).ok()?;
    Some(acp::ExtNotification::new(LOG_METHOD, raw.into()))
}

fn send_entries(entries: Vec<ClientLogEntry>) {
    let Some(tx) = current_sender() else { return };
    let Some(notification) = build_notification(entries) else {
        return;
    };
    // Guard against panic if called from a non-tokio thread (e.g., a
    // tracing::Layer callback on a blocking thread).
    let Ok(handle) = Handle::try_current() else {
        return;
    };
    let tx = tx.clone();
    handle.spawn(async move {
        let _ = xai_acp_lib::acp_send(notification, &tx).await;
    });
}

/// Flush any buffered entries to the shell (fire-and-forget).
pub fn flush() {
    let entries = {
        let Ok(mut buf) = BUFFER.lock() else { return };
        if buf.is_empty() {
            return;
        }
        buf.drain(..).collect::<Vec<_>>()
    };
    send_entries(entries);
}

/// Flush buffered entries and await delivery.
///
/// Use this before process exit to ensure entries are delivered
/// before the agent shuts down.
pub async fn flush_blocking() {
    let entries = {
        let Ok(mut buf) = BUFFER.lock() else { return };
        if buf.is_empty() {
            return;
        }
        buf.drain(..).collect::<Vec<_>>()
    };
    let Some(tx) = current_sender() else { return };
    let Some(notification) = build_notification(entries) else {
        return;
    };
    let _ = xai_acp_lib::acp_send(notification, &tx).await;
}

/// Log an info-level entry.
pub fn info(msg: &str, sid: Option<&str>, ctx: Option<serde_json::Value>) {
    push_entry(LogLevel::Info, msg, sid, ctx);
}

/// Log a warn-level entry.
pub fn warn(msg: &str, sid: Option<&str>, ctx: Option<serde_json::Value>) {
    push_entry(LogLevel::Warn, msg, sid, ctx);
}

/// Log an error-level entry.
pub fn error(msg: &str, sid: Option<&str>, ctx: Option<serde_json::Value>) {
    push_entry(LogLevel::Error, msg, sid, ctx);
}

/// Log a debug-level entry.
pub fn debug(msg: &str, sid: Option<&str>, ctx: Option<serde_json::Value>) {
    push_entry(LogLevel::Debug, msg, sid, ctx);
}

#[cfg(test)]
fn reset_for_tests() {
    if let Ok(mut current) = ACP_TX.lock() {
        *current = None;
    }
    if let Ok(mut buf) = BUFFER.lock() {
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use xai_acp_lib::AcpAgentMessage;

    #[tokio::test]
    #[serial_test::serial(UNIFIED_LOG)]
    async fn set_sender_rebinds_future_flushes() {
        reset_for_tests();
        let (tx1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
        let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();
        init(tx1);
        set_sender(tx2);

        let responder = tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(1), rx2.recv())
                .await
                .expect("rebinding should deliver a log message")
                .expect("second receiver should stay open")
            {
                AcpAgentMessage::ExtNotification(args) => {
                    args.response_tx
                        .send(Ok(()))
                        .expect("log notification response should be accepted");
                }
                other => panic!("expected ext notification, got {other:?}"),
            }
        });

        info("rebound log entry", None, None);
        flush_blocking().await;
        responder.await.expect("receiver task should complete");
        match tokio::time::timeout(Duration::from_millis(50), rx1.recv()).await {
            Ok(Some(message)) => {
                panic!("old sender should not receive log flushes after rebinding: {message:?}")
            }
            Ok(None) | Err(_) => {}
        }
        reset_for_tests();
    }
}
