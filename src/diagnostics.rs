//! Append-only, content-free evidence for one task invocation.
use anyhow::Result;
use serde_json::{Value, json};
use std::{io::Write, path::Path, sync::Mutex, time::Instant};

pub(crate) struct Journal {
    file: Mutex<Option<std::fs::File>>,
    started: Instant,
}

impl Journal {
    pub(crate) fn new(work: &Path) -> Result<Self> {
        let directory = work.join("diagnostics");
        std::fs::create_dir_all(&directory)?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        let (file, path) = tempfile::Builder::new()
            .prefix(&format!("round-{timestamp}-"))
            .suffix(".jsonl")
            .tempfile_in(directory)?
            .keep()?;
        let journal = Self {
            file: Mutex::new(Some(file)),
            started: Instant::now(),
        };
        journal.record(json!({"type":"round_start", "schema":1,
            "started_unix_ms":timestamp, "version":env!("CARGO_PKG_VERSION")}));
        tracing::info!("AI diagnostics: {}", path.display());
        Ok(journal)
    }

    // Callers supply only counts, generated IDs and fixed categories, never bodies or keys.
    pub(crate) fn record(&self, mut event: Value) {
        event["elapsed_ms"] = json!(self.started.elapsed().as_millis());
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(writer) = file.as_mut()
            && writeln!(writer, "{event}")
                .and_then(|_| writer.flush())
                .is_err()
        {
            *file = None;
            tracing::warn!("AI diagnostic log could not be written; task processing continues");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_are_separate_and_concurrent_records_remain_readable() {
        let root = tempfile::tempdir().unwrap();
        for _ in 0..2 {
            let journal = Journal::new(root.path()).unwrap();
            std::thread::scope(|scope| {
                for worker in 0..4 {
                    let journal = &journal;
                    scope.spawn(move || {
                        for index in 0..10 {
                            journal.record(json!({"type":"check", "worker":worker, "index":index}));
                        }
                    });
                }
            });
        }
        let files: Vec<_> = std::fs::read_dir(root.path().join("diagnostics"))
            .unwrap()
            .collect();
        assert_eq!(files.len(), 2);
        for file in files {
            let contents = std::fs::read_to_string(file.unwrap().path()).unwrap();
            let events: Vec<Value> = contents
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(events.len(), 41);
            assert_eq!(events[0]["type"], "round_start");
            assert!(events.iter().all(|event| event["elapsed_ms"].is_u64()));
        }
    }
}
