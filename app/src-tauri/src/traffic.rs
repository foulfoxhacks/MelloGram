//! Traffic log: every network exchange MelloGram makes, recorded without its
//! content. Entries carry a direction, a channel, a redacted endpoint (host
//! or API method only: no paths, queries, tokens, phone numbers or message
//! text) and a byte count. Kept in memory only, newest 1000 entries.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;

const MAX_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize)]
pub struct TrafficEntry {
    pub at: i64,
    /// "out" or "in".
    pub dir: &'static str,
    /// "telegram" (MTProto), "https" or "local".
    pub channel: &'static str,
    pub endpoint: String,
    pub bytes: u64,
    pub note: &'static str,
}

#[derive(Default)]
pub struct TrafficLog {
    entries: Mutex<VecDeque<TrafficEntry>>,
}

#[derive(Debug, Serialize, Default)]
pub struct TrafficTotals {
    pub sent_bytes: u64,
    pub received_bytes: u64,
    pub entries: usize,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Host only, lowercased; anything that is not a plain hostname is replaced.
pub fn redact_url(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host); // drop user:pass@
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    if !host.is_empty() && host.len() <= 253 && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
        host
    } else {
        "(redacted)".into()
    }
}

impl TrafficLog {
    pub fn record(&self, dir: &'static str, channel: &'static str, endpoint: impl Into<String>, bytes: u64, note: &'static str) {
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(TrafficEntry { at: now(), dir, channel, endpoint: endpoint.into(), bytes, note });
    }

    pub fn snapshot(&self) -> (Vec<TrafficEntry>, TrafficTotals) {
        let entries = self.entries.lock().unwrap();
        let mut totals = TrafficTotals { entries: entries.len(), ..Default::default() };
        for e in entries.iter() {
            if e.dir == "out" { totals.sent_bytes += e.bytes } else { totals.received_bytes += e.bytes }
        }
        (entries.iter().rev().cloned().collect(), totals)
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_keeps_only_the_host() {
        assert_eq!(redact_url("https://user:pw@API.wakatime.com:443/api/v1/x?token=abc"), "api.wakatime.com");
        assert_eq!(redact_url("https://example.com/theme.css?k=1"), "example.com");
        assert_eq!(redact_url("https://bad host/"), "(redacted)");
    }

    #[test]
    fn ring_buffer_and_totals() {
        let log = TrafficLog::default();
        for _ in 0..(MAX_ENTRIES + 5) {
            log.record("out", "telegram", "messages.sendMessage", 10, "");
        }
        log.record("in", "https", "example.com", 7, "");
        let (entries, totals) = log.snapshot();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].dir, "in");
        assert_eq!(totals.received_bytes, 7);
    }
}
