//! Apple 原生后端（仅 `apple_native` 构建生效）：
//! Silero VAD（CoreML/ANE）+ Qwen3-ASR（CoreML，来自 speech-swift）。
//! 模型首次使用时自动下载到 ~/Library/Caches/qwen3-speech/（HF_ENDPOINT 可换镜像）。

#![cfg(apple_native)]

use crate::models::normalize_apple_model as normalize;
use crate::timeline::TranscriptEvent;
use anyhow::{Context, Result};
use std::ffi::{CStr, CString};
use std::path::Path;
use std::time::Instant;

mod ffi {
    use std::os::raw::{c_char, c_double, c_int};

    unsafe extern "C" {
        pub fn c2m_vad_detect(
            wav_path: *const c_char,
            min_speech: c_double,
            min_silence: c_double,
            out_starts: *mut *mut c_double,
            out_ends: *mut *mut c_double,
            out_n: *mut c_int,
        ) -> c_int;
        pub fn c2m_free_doubles(p: *mut c_double);
        pub fn c2m_asr_create(
            model: *const c_char,
            err: *mut c_char,
            err_len: usize,
        ) -> *mut std::ffi::c_void;
        pub fn c2m_asr_prepare_cache(
            model: *const c_char,
            err: *mut c_char,
            err_len: usize,
        ) -> c_int;
        pub fn c2m_asr_transcribe(
            handle: *mut std::ffi::c_void,
            wav_path: *const c_char,
            out_text: *mut c_char,
            out_len: usize,
        ) -> c_int;
        pub fn c2m_asr_destroy(handle: *mut std::ffi::c_void);
        pub fn c2m_last_error() -> *const c_char;
    }
}

// Swift downloads report through the same serialized NDJSON stream as Rust.
#[unsafe(no_mangle)]
extern "C" fn c2m_model_progress(fraction: f64, message: *const std::ffi::c_char) {
    if message.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    if crate::progress::is_json() {
        crate::progress::emit(serde_json::json!({"type":"progress", "stage":"model/apple",
            "current": (fraction.clamp(0.0, 1.0) * 10000.0) as u64, "total":10000,
            "message":message}));
    } else if !crate::progress::is_quiet() {
        eprintln!("模型 / Model {:.0}% · {message}", fraction * 100.0);
    }
}

fn last_error() -> String {
    unsafe {
        let p = ffi::c2m_last_error();
        if p.is_null() {
            String::new()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// Silero VAD（CoreML）。返回 (start, end) 语音段（秒）。
pub fn vad(wav: &Path, min_speech: f64, min_silence: f64) -> Result<Vec<(f64, f64)>> {
    let path = CString::new(wav.to_string_lossy().as_bytes())?;
    let mut starts: *mut f64 = std::ptr::null_mut();
    let mut ends: *mut f64 = std::ptr::null_mut();
    let mut n: i32 = 0;
    let rc = unsafe {
        ffi::c2m_vad_detect(
            path.as_ptr(),
            min_speech,
            min_silence,
            &mut starts,
            &mut ends,
            &mut n,
        )
    };
    if rc != 0 {
        anyhow::bail!("语音检测失败 / Speech detection failed: {}", last_error());
    }
    let mut out = Vec::with_capacity(n as usize);
    if n > 0 {
        unsafe {
            for i in 0..n as usize {
                out.push((starts.add(i).read(), ends.add(i).read()));
            }
            ffi::c2m_free_doubles(starts);
            ffi::c2m_free_doubles(ends);
        }
    }
    Ok(out)
}

pub struct CoremlAsr {
    handle: *mut std::ffi::c_void,
}

/// MLX 搜索 metallib 的顺序（mlx-swift load_default_library）：
/// 可执行文件同目录的 mlx.metallib → exe/Resources/mlx.metallib → ……
/// → CWD 下的 default.metallib（METAL_PATH 编译期常量，相对当前工作目录）。
/// Tauri sidecar 场景：codesign 把 Contents/MacOS/ 下所有文件都当代码签名，
/// 数据文件 metallib 只能放 Contents/Resources/（资源密封区）——
/// 由 GUI 后端在 spawn sidecar 时把 CWD 设为该目录，命中最后一条兜底路径。
fn ensure_metallib() -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    let resources = dir.join("Resources");
    let bundle_resources = dir.join("../Resources");
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    for base in [dir, &resources, &bundle_resources, &cwd] {
        for name in ["mlx.metallib", "default.metallib"] {
            if base.join(name).is_file() {
                return Ok(());
            }
        }
    }
    anyhow::bail!(
        "缺少 MLX Metal 库（{0}），CoreML 推理不可用。\n\
         从源码构建：把 native/apple-asr/.build/out/Products/Release/mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib \
         复制为二进制同目录的 mlx.metallib；预编译安装：重跑 install.sh。 / Missing the MLX Metal library ({0}); CoreML inference is unavailable. \
         From source: copy native/apple-asr/.build/out/Products/Release/mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib \
         to mlx.metallib next to the binary; for prebuilt installs, rerun install.sh.",
        dir.join("mlx.metallib").display()
    )
}

/// 解析 coreml 后端用的模型：显式参数（调用方已合并 CLI 与 config.toml
/// defaults.asr_model）> （交互式终端则询问并写 config.toml）> qwen3-1.7b。
pub fn resolve_model(explicit: Option<&str>) -> Result<String> {
    if let Some(m) = explicit {
        return normalize(m);
    }
    let chosen = prompt_model_choice()?;
    use std::io::IsTerminal as _;
    if !crate::progress::is_json()
        && !crate::progress::is_quiet()
        && std::io::stdin().is_terminal()
        && std::io::stderr().is_terminal()
    {
        persist_model_choice(&chosen);
    }
    Ok(chosen)
}

/// 把交互选择的模型写入 config.toml defaults.asr_model（不再写 marker 双轨）。
/// 失败只告警不阻断：本次已拿到选择，下次仍会再询问。
fn persist_model_choice(model: &str) {
    if let Err(e) = (|| -> Result<()> {
        let mut cfg = crate::settings::load()?;
        cfg.defaults.asr_model = Some(model.to_string());
        crate::settings::save(&cfg)?;
        Ok(())
    })() {
        tracing::warn!(
            "无法保存模型选择，下次仍会询问 / Could not save model choice; you will be asked again next time: {e:#}"
        );
    }
}

/// 首次使用：让用户选择下载哪个模型（非交互环境默认 qwen3-1.7b）。
/// dialoguer 提供标准行编辑与方向键选择（裸 read_line 不处理转义序列，issue #3）。
fn prompt_model_choice() -> Result<String> {
    use std::io::IsTerminal as _;
    if crate::progress::is_json()
        || crate::progress::is_quiet()
        || !std::io::stdin().is_terminal()
        || !std::io::stderr().is_terminal()
    {
        tracing::info!(
            "非交互环境，默认使用 Qwen3-ASR 1.7B 模型（--asr-model qwen3-0.6b/whisper 可切换） / Non-interactive environment; defaulting to the Qwen3-ASR 1.7B model (switch with --asr-model qwen3-0.6b/whisper)"
        );
        return Ok("qwen3-1.7b".into());
    }
    let choice = dialoguer::Select::new()
        .with_prompt("选择识别模型 / Select ASR Model")
        .items([
            "qwen3-1.7b — Qwen3-ASR 1.7B MLX（推荐，下载约 2.3GB / Recommended, ~2.3GB download）",
            "qwen3-0.6b — Qwen3-ASR 0.6B CoreML（低功耗，约 1GB / Low power, ~1GB download）",
            "whisper — Whisper Large-v3 Turbo（多语种 / Multilingual）",
        ])
        .default(0)
        .interact_opt()?;
    match choice {
        Some(1) => Ok("qwen3-0.6b".into()),
        Some(2) => Ok("whisper".into()),
        Some(_) => Ok("qwen3-1.7b".into()),
        None => anyhow::bail!(
            "已取消模型选择，未开始下载 / Model selection cancelled; download not started"
        ),
    }
}

impl CoremlAsr {
    /// 只加载已准备的缓存；缺失文件由模型准备阶段下载。
    pub fn load(model: &str) -> Result<Self> {
        let name = CString::new(model)?.into_raw();
        let mut err = vec![0u8; 1024];
        let handle = unsafe { ffi::c2m_asr_create(name, err.as_mut_ptr() as *mut _, err.len()) };
        unsafe { std::mem::drop(CString::from_raw(name)) };
        if handle.is_null() {
            let msg = CStr::from_bytes_until_nul(&err)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            anyhow::bail!("{msg}");
        }
        Ok(Self { handle })
    }

    /// 转写 16k 单声道 wav。Ok(None) = 无语音内容。
    pub fn transcribe(&self, wav: &Path) -> Result<Option<String>> {
        let path = CString::new(wav.to_string_lossy().as_bytes())?;
        let mut out = vec![0u8; 16 * 1024];
        let rc = unsafe {
            ffi::c2m_asr_transcribe(
                self.handle,
                path.as_ptr(),
                out.as_mut_ptr() as *mut _,
                out.len(),
            )
        };
        match rc {
            0 | 2 => {
                let s = CStr::from_bytes_until_nul(&out)
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if rc == 2 {
                    // shim 侧缓冲（16KB）不够，文本被截断：至少留痕
                    tracing::warn!(
                        "Apple 识别结果过长，部分文字被截断 / Apple transcript exceeded the size limit; some text was truncated"
                    );
                }
                Ok(Some(s))
            }
            1 => Ok(None),
            _ => anyhow::bail!(
                "Apple 语音识别失败 / Apple transcription failed: {}",
                last_error()
            ),
        }
    }
}

/// Verify prepared files using the same offline loaders as transcription, with no user media.
pub fn prepare_model(model: &str) -> Result<()> {
    ensure_metallib()?;
    let _asr = CoremlAsr::load(model)?;
    // VAD is a separate required model. Load it using a local second of silence so a
    // successful ASR download cannot misleadingly imply offline transcription readiness.
    let temporary = crate::runtime::TempWorkDir::new("model-check")?;
    let wav = temporary.path().join("silence.wav");
    let bytes = 32_000_u32;
    let mut data = Vec::with_capacity(44 + bytes as usize);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + bytes).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16_u32.to_le_bytes());
    data.extend_from_slice(&1_u16.to_le_bytes());
    data.extend_from_slice(&1_u16.to_le_bytes());
    data.extend_from_slice(&16_000_u32.to_le_bytes());
    data.extend_from_slice(&32_000_u32.to_le_bytes());
    data.extend_from_slice(&2_u16.to_le_bytes());
    data.extend_from_slice(&16_u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&bytes.to_le_bytes());
    data.resize(44 + bytes as usize, 0);
    crate::checkpoint::atomic_write(&wav, &data)?;
    vad(&wav, 0.25, 0.35)?;
    Ok(())
}

/// Cache-only preparation before downloading user media; inference loads weights once later.
pub fn prepare_cache(model: &str) -> Result<()> {
    ensure_metallib()?;
    let name = CString::new(model)?;
    let mut error = vec![0_u8; 4096];
    let status = unsafe {
        ffi::c2m_asr_prepare_cache(name.as_ptr(), error.as_mut_ptr() as *mut _, error.len())
    };
    if status != 0 {
        anyhow::bail!(
            "{}",
            CStr::from_bytes_until_nul(&error)
                .map(|error| error.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "Apple 模型缓存准备失败 / Apple model cache preparation failed".into())
        );
    }
    Ok(())
}

impl Drop for CoremlAsr {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ffi::c2m_asr_destroy(self.handle) };
        }
    }
}

/// CoreML 全流程：Silero VAD 分段 → 逐段转写。
pub fn run_coreml(
    wav: &Path,
    max_speech: f64,
    model: &str,
    tmp_dir: &Path,
    cp: &mut crate::checkpoint::Checkpoint,
) -> Result<Vec<TranscriptEvent>> {
    let t0 = Instant::now();
    crate::progress::stage("model/apple", "start");
    let raw = vad(wav, 0.25, 0.35)?;
    crate::progress::stage("model/apple", "done");
    // 时长只探测一次（与 ffmpeg_vad 同样约定：normalize_segments 不再自行 ffprobe）
    let dur = crate::media::probe_duration_blocking(wav).unwrap_or(0.0);
    let segs = crate::asr::normalize_segments(raw, max_speech, wav, dur)?;
    tracing::info!(segs = segs.len(), engine = "silero-coreml", "vad");
    if segs.is_empty() {
        tracing::warn!(
            "未检测到语音，将仅保留截图 / No speech detected; keeping slides without a transcript"
        );
        return Ok(vec![]);
    }

    ensure_metallib()?;
    tracing::info!(model, "loading cached Apple native ASR");
    crate::progress::stage("model/apple", "start");
    let asr = CoremlAsr::load(model).context("CoreML 模型加载失败 / CoreML model loading failed")?;
    crate::progress::stage("model/apple", "done");
    tracing::info!(
        secs = format_args!("{:.1}", t0.elapsed().as_secs_f64()),
        "coreml ready"
    );

    let r = crate::asr::run_chunks(wav, &segs, cp, tmp_dir, "asr", |_i, _seg, chunk| {
        asr.transcribe(chunk).map(|t| {
            let t = t.map(|s| crate::asr::sanitize_qwen_text(&s));
            t.filter(|s| !s.is_empty())
        })
    })?;
    tracing::info!(
        n = r.len(),
        secs = format_args!("{:.1}", t0.elapsed().as_secs_f64()),
        "asr done"
    );
    Ok(r)
}
