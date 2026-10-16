//! ffmpeg 子进程封装：音频抽取（16k 单声道 PCM wav）与媒体探测。

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

async fn probe_output(cmd: &mut Command, timeout: Duration) -> Option<std::process::Output> {
    let output = cmd
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .output();
    tokio::time::timeout(timeout, output).await.ok()?.ok()
}

/// 跑子进程并检查退出码；失败时返回带 stderr 尾部摘要的错误（供各模块复用）。
pub(crate) async fn run_cmd(cmd: &mut Command, what: &str) -> Result<std::process::Output> {
    let out = cmd
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| format!("无法启动 {what} / Could not start {what}"))?;
    if !out.status.success() {
        return Err(crate::error::cmd_error(
            what,
            out.status.code(),
            &String::from_utf8_lossy(&out.stderr),
        ));
    }
    Ok(out)
}

/// 抽取并归一化为 16kHz 单声道 s16 wav，成功后原子替换目标。
pub async fn extract_audio(media: &Path, dest: &Path) -> Result<()> {
    if let Some(p) = dest.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    let temporary = tempfile::Builder::new()
        .suffix(".wav")
        .tempfile_in(dest.parent().unwrap_or(Path::new(".")))?;
    run_cmd(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .arg("-i")
            .arg(media)
            .args([
                "-vn",
                "-af",
                "loudnorm=I=-16:TP=-1.5:LRA=11",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(temporary.path()),
        "ffmpeg",
    )
    .await?;
    temporary.persist(dest)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
}

/// ffprobe `-of json` 输出中我们关心的字段子集。
#[derive(serde::Deserialize)]
struct ProbeJson {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(serde::Deserialize)]
struct ProbeStream {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(serde::Deserialize)]
struct ProbeFormat {
    /// ffprobe 的 duration 是字符串（如 "95.040000"）
    duration: Option<String>,
}

/// 视频宽高与时长。用 `-of json` 结构化输出，取代按逗号/空白切分的文本解析。
pub async fn probe_video(media: &Path) -> Option<VideoInfo> {
    let out = probe_output(
        Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height:format=duration",
                "-of",
                "json",
            ])
            .arg(media),
        PROBE_TIMEOUT,
    )
    .await?;
    if !out.status.success() {
        return None;
    }
    let probe: ProbeJson = serde_json::from_slice(&out.stdout).ok()?;
    let stream = probe.streams.first()?;
    let duration = probe
        .format
        .and_then(|f| f.duration)
        .and_then(|d| parse_duration(&d))?;
    Some(VideoInfo {
        width: stream.width?,
        height: stream.height?,
        duration,
    })
}

/// `probe_duration` 两个入口共享的 ffprobe 参数（只输出时长数字）。
fn duration_args() -> [&'static str; 6] {
    [
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
    ]
}

/// 从 ffprobe 输出解析时长（秒）；进程失败或输出非法均为 None（非致命）。
fn parse_duration(stdout: &str) -> Option<f64> {
    stdout
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration >= 0.0)
}

/// 用 ffprobe 拿时长（秒）。失败时返回 None（非致命）。
pub async fn probe_duration(media: &Path) -> Option<f64> {
    let out = probe_output(
        Command::new("ffprobe").args(duration_args()).arg(media),
        PROBE_TIMEOUT,
    )
    .await?;
    if !out.status.success() {
        return None;
    }
    parse_duration(&String::from_utf8_lossy(&out.stdout))
}

/// [`probe_duration`] 的同步版（供阻塞线程使用）。
pub fn probe_duration_blocking(media: &Path) -> Option<f64> {
    let mut cmd = std::process::Command::new("ffprobe");
    cmd.args(duration_args()).arg(media);
    let out = crate::runtime::run_bounded("ffprobe", &mut cmd, PROBE_TIMEOUT).ok()?;
    if !out.status.success() {
        return None;
    }
    parse_duration(&out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_rejects_invalid_values() {
        for value in ["NaN", "inf", "-1", "N/A", ""] {
            assert_eq!(parse_duration(value), None, "{value}");
        }
        assert_eq!(parse_duration(" 95.04\n"), Some(95.04));
    }

    #[tokio::test]
    async fn probe_timeout_returns_without_waiting_for_process_exit() {
        #[cfg(windows)]
        let mut cmd = {
            let mut cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 5"]);
            cmd
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut cmd = Command::new("sleep");
            cmd.arg("5");
            cmd
        };
        let start = std::time::Instant::now();
        assert!(
            probe_output(&mut cmd, Duration::from_millis(100))
                .await
                .is_none()
        );
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn extraction_replaces_unverified_audio_and_preserves_it_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let dest = dir.path().join("audio.wav");
        run_cmd(
            Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "anullsrc=r=16000:cl=mono",
                    "-t",
                    "1",
                ])
                .arg(&source),
            "ffmpeg",
        )
        .await
        .unwrap();
        std::fs::write(&dest, b"truncated").unwrap();
        extract_audio(&source, &dest).await.unwrap();
        assert_eq!(probe_duration(&dest).await, Some(1.0));
        let valid = std::fs::read(&dest).unwrap();
        assert!(
            extract_audio(&dir.path().join("missing.wav"), &dest)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&dest).unwrap(), valid);
    }
}
