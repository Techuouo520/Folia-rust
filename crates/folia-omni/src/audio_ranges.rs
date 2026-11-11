// Validated priority byte ranges feed decoder seeks while the sequential cache download continues.

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::OmniError;
use folia_core::{Download, ReadRequest};
use reqwest::header::{ACCEPT_ENCODING, CONTENT_RANGE, ETAG, IF_RANGE, RANGE};

const RANGE_BYTES: u64 = 256 * 1024;

pub(crate) fn start(
    http: reqwest::Client,
    response: &reqwest::Response,
    path: PathBuf,
    download: Download,
) -> Option<tokio::task::JoinHandle<()>> {
    let total = response.content_length()?;
    let validator = response.headers().get(ETAG)?.clone();
    let tag = validator.to_str().ok()?;
    if !tag.starts_with('"') || !tag.ends_with('"') {
        return None;
    }
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    download.request_ranges(sender);
    let source = Source { http, url: response.url().clone(), validator, total, path, download };
    Some(tokio::spawn(source.run(receiver)))
}

struct Source {
    http: reqwest::Client,
    url: reqwest::Url,
    validator: reqwest::header::HeaderValue,
    total: u64,
    path: PathBuf,
    download: Download,
}

impl Source {
    async fn run(self, requests: Receiver<ReadRequest>) {
        loop {
            let request = match requests.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => {
                    if self.download.progress().1 {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
            };
            if request.offset >= self.total || request.cancelled.load(Ordering::Acquire) {
                continue;
            }
            let result = tokio::select! {
                result = self.fetch(request.offset) => result,
                () = cancelled(&request) => continue,
            };
            match result {
                Ok(true) => {}
                Ok(false) => {
                    self.download.disable_ranges();
                    tracing::debug!("audio source does not provide matching ranges; keeping sequential download");
                    return;
                }
                Err(err) => {
                    self.download.disable_ranges();
                    tracing::warn!(
                        error = &err as &dyn std::error::Error,
                        "audio range unavailable; keeping sequential download"
                    );
                    return;
                }
            }
        }
    }

    async fn fetch(&self, start: u64) -> Result<bool, OmniError> {
        let began = Instant::now();
        let end = start.saturating_add(RANGE_BYTES).min(self.total) - 1;
        let mut response = self
            .http
            .get(self.url.clone())
            .header(ACCEPT_ENCODING, "identity")
            .header(RANGE, format!("bytes={start}-{end}"))
            .header(IF_RANGE, self.validator.clone())
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|err| OmniError::Http(err.without_url()))?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            || response.headers().get(ETAG) != Some(&self.validator)
        {
            return Ok(false);
        }
        let range = response.headers().get(CONTENT_RANGE).and_then(|value| value.to_str().ok()).and_then(content_range);
        let Some((from, to, total)) =
            range.filter(|&(from, to, total)| from == start && to >= from && to <= end && total == self.total)
        else {
            return Ok(false);
        };
        let count = (to - from + 1) as usize;
        let mut data = Vec::with_capacity(count);
        while let Some(chunk) = response.chunk().await.map_err(|err| OmniError::Http(err.without_url()))? {
            if chunk.len() > count - data.len() {
                return Ok(false);
            }
            data.extend_from_slice(&chunk);
        }
        if data.len() != count {
            return Ok(false);
        }
        let io_err = |source| OmniError::Io { path: self.path.clone(), source };
        let mut file = OpenOptions::new().write(true).open(&self.path).map_err(io_err)?;
        file.seek(SeekFrom::Start(start)).map_err(io_err)?;
        file.write_all(&data).map_err(io_err)?;
        self.download.publish_range(start, to + 1);
        tracing::debug!(
            offset = start,
            bytes = count,
            total,
            elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
            "audio seek range ready"
        );
        Ok(true)
    }
}

async fn cancelled(request: &ReadRequest) {
    while !request.cancelled.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn content_range(value: &str) -> Option<(u64, u64, u64)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
}
