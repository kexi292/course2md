//! Model evidence is loaded in the background; preparation uses the existing model job.
use super::{settings_detail_group, settings_detail_row, settings_value};
use crate::theme::*;
use crate::*;
use course2md::{
    config::{AsrProvider, model_dir_from},
    models::status::{CacheState, LocalModelStatus},
};
use gpui_component::button::*;
use std::path::Path;

#[derive(Clone)]
struct Request {
    provider: AsrProvider,
    model: String,
    root: PathBuf,
}
impl Request {
    fn new(provider: AsrProvider, model: Option<&str>, root: &Path) -> Self {
        Self {
            provider,
            model: model
                .filter(|model| !model.trim().is_empty())
                .unwrap_or("qwen3-1.7b")
                .into(),
            root: root.into(),
        }
    }
    fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.provider.as_str(),
            self.model,
            self.root.display()
        )
    }
}
struct Entry {
    request: Request,
    generation: u64,
    checking: bool,
    result: Option<Result<LocalModelStatus, String>>,
    cache_directories: std::collections::BTreeSet<PathBuf>,
}
#[derive(Default)]
pub(super) struct State {
    entries: BTreeMap<String, Entry>,
    preparing: Option<Request>,
    result: Option<(String, bool)>,
    cancelled: bool,
    details: bool,
    cache_details: std::collections::BTreeSet<String>,
}

/// A read-only view for setup. Checks and downloads keep the same ownership as Settings.
pub(crate) struct ModelSetupSnapshot {
    pub(crate) checking: bool,
    pub(crate) result: Option<Result<LocalModelStatus, String>>,
    pub(crate) device_issue: Option<String>,
    pub(crate) preparing: bool,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) cancelled: bool,
}
fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", value as f64 / (1024_f64.powi(3)))
    } else if value >= 1024 * 1024 {
        format!("{:.1} MB", value as f64 / (1024_f64.powi(2)))
    } else {
        format!("{value} 字节")
    }
}
fn provider_name(provider: AsrProvider) -> &'static str {
    match provider {
        AsrProvider::Coreml => "Apple 原生",
        AsrProvider::Gpu => "GPU",
        AsrProvider::Cpu => "CPU",
        AsrProvider::Npu => "Intel NPU",
        AsrProvider::Api => "语音服务",
    }
}
impl Desktop {
    pub(crate) fn setup_model_snapshot(
        &self,
        provider: AsrProvider,
        model: Option<&str>,
        root: &Path,
    ) -> ModelSetupSnapshot {
        let request = Request::new(provider, model, root);
        let state = &self.settings_ui.model_diagnostics;
        let entry = state.entries.get(&request.key());
        let matching = state
            .preparing
            .as_ref()
            .is_some_and(|active| active.key() == request.key());
        ModelSetupSnapshot {
            checking: entry.is_none_or(|entry| entry.checking),
            result: entry.and_then(|entry| entry.result.clone()),
            device_issue: self.model_device_issue(provider),
            preparing: matching && self.kind == Kind::Models && self.job.is_some(),
            notice: matching.then(|| state.result.clone()).flatten(),
            cancelled: matching && state.cancelled,
        }
    }

    pub(crate) fn prepare_setup_model(
        &mut self,
        provider: AsrProvider,
        model: Option<&str>,
        root: &Path,
        cx: &mut Context<Self>,
    ) {
        self.begin_model_preparation(Request::new(provider, model, root), cx);
    }

    fn default_model_request(&self) -> Request {
        let defaults = &self.preferences.generation().options;
        Request::new(
            defaults
                .provider
                .unwrap_or_else(|| self.recommended_local_provider()),
            defaults.asr_model.as_deref(),
            &model_dir_from(defaults.model_dir.as_deref()),
        )
    }
    pub fn ensure_model_diagnostic(
        &mut self,
        provider: AsrProvider,
        model: Option<&str>,
        root: &Path,
        cx: &mut Context<Self>,
    ) {
        self.check_model_request(Request::new(provider, model, root), false, cx);
    }
    fn check_model_request(&mut self, request: Request, force: bool, cx: &mut Context<Self>) {
        if request.provider == AsrProvider::Api {
            return;
        }
        let key = request.key();
        let state = &mut self.settings_ui.model_diagnostics;
        if !force && state.entries.contains_key(&key) {
            return;
        }
        let generation = state
            .entries
            .get(&key)
            .map_or(1, |entry| entry.generation + 1);
        state.entries.insert(
            key.clone(),
            Entry {
                request: request.clone(),
                generation,
                checking: true,
                result: None,
                cache_directories: Default::default(),
            },
        );
        cx.spawn(async move |this, cx| {
            let (result, cache_directories) = cx
                .background_executor()
                .spawn(async move {
                    let result = course2md::models::status::inspect(
                        request.provider,
                        &request.model,
                        &request.root,
                    )
                    .map_err(|error| format!("{error:#}"));
                    let directories = result
                        .as_ref()
                        .ok()
                        .into_iter()
                        .flat_map(|status| &status.parts)
                        .filter(|part| part.path.is_dir())
                        .map(|part| part.path.clone())
                        .collect();
                    (result, directories)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(entry) = this.settings_ui.model_diagnostics.entries.get_mut(&key)
                    && entry.generation == generation
                {
                    entry.checking = false;
                    entry.result = Some(result);
                    entry.cache_directories = cache_directories;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn refresh_model_diagnostics(&mut self, cx: &mut Context<Self>) {
        let mut requests = self
            .settings_ui
            .model_diagnostics
            .entries
            .values()
            .map(|entry| (entry.request.key(), entry.request.clone()))
            .collect::<BTreeMap<_, _>>();
        let default = self.default_model_request();
        requests.insert(default.key(), default);
        if let Some(request) = self.settings_ui.model_diagnostics.preparing.clone() {
            requests.insert(request.key(), request);
        }
        for request in requests.into_values() {
            self.check_model_request(request, true, cx);
        }
    }
    pub(super) fn ensure_settings_model_diagnostic(&mut self, cx: &mut Context<Self>) {
        self.check_model_request(self.default_model_request(), false, cx);
    }
    pub fn model_preparation_args(&self) -> Vec<String> {
        let request = self
            .settings_ui
            .model_diagnostics
            .preparing
            .clone()
            .unwrap_or_else(|| self.default_model_request());
        vec![
            "models".into(),
            "prepare".into(),
            "--provider".into(),
            request.provider.as_str().into(),
            "--model".into(),
            request.model,
            "--dir".into(),
            request.root.display().to_string(),
            "--json".into(),
        ]
    }
    fn begin_model_preparation(&mut self, request: Request, cx: &mut Context<Self>) {
        if self.job.is_some() {
            self.message = Some("当前处理结束后可以准备模型。已下载的文件会保留。".into());
            cx.notify();
            return;
        }
        self.settings_ui.model_diagnostics.preparing = Some(request);
        self.settings_ui.model_diagnostics.result = None;
        self.settings_ui.model_diagnostics.cancelled = false;
        self.settings_ui.model_diagnostics.details = false;
        self.start(Kind::Models, cx);
    }
    pub fn model_preparation_finished(
        &mut self,
        success: bool,
        cancelled: bool,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.model_diagnostics.cancelled = cancelled && !success;
        let request = self
            .settings_ui
            .model_diagnostics
            .preparing
            .clone()
            .unwrap_or_else(|| self.default_model_request());
        self.settings_ui.model_diagnostics.result = Some((
            if success {
                format!(
                    "{} · {} 的准备已完成，正在重新检查缓存。",
                    provider_name(request.provider),
                    request.model
                )
            } else if cancelled {
                format!(
                    "{} 的准备已停止。已下载完成的文件保留，可以继续准备。",
                    request.model
                )
            } else {
                format!(
                    "{} 的准备未完成：{}。已下载完成的文件保留。",
                    request.model,
                    self.task_error
                        .as_deref()
                        .unwrap_or("转换程序没有返回成功结果")
                )
            },
            !success && !cancelled,
        ));
        self.refresh_model_diagnostics(cx);
        cx.notify();
    }
    fn model_device_issue(&self, provider: AsrProvider) -> Option<String> {
        let environment = self.environment.as_ref()?;
        if !environment.engine {
            return Some("转换程序无法运行，请先恢复应用内的转换程序。".into());
        }
        match provider {
            AsrProvider::Coreml if !environment.apple && !cfg!(all(target_os = "macos", target_arch = "aarch64")) => Some("已选择 Apple 原生，但当前系统无法使用这套识别方式。选择仍保留，请选择本机可用的识别方式。".into()),
            AsrProvider::Coreml if !environment.apple => Some("已选择 Apple 原生，但本机没有检测到完整的 Apple 识别运行时。选择仍保留；安装完整应用可以恢复组件，也可以明确选择其他可用方式。".into()),
            AsrProvider::Gpu if !environment.llama => Some("已选择 GPU，但未检测到 llama-server 识别运行时。选择仍保留；安装运行时后重新检查。".into()),
            AsrProvider::Gpu if environment.gpu.is_none() => Some("已选择 GPU，但识别运行时没有报告可用的 GPU。选择仍保留；可以明确改用 CPU。".into()),
            AsrProvider::Cpu if !environment.llama => Some("已选择 CPU，但未检测到 llama-server 识别运行时。安装完成后可以使用已缓存的模型。".into()),
            AsrProvider::Npu if !environment.npu_device => Some("已选择 Intel NPU，但本机未检测到可用的 Intel NPU 设备。选择仍保留，请选择本机可用的识别方式。".into()),
            AsrProvider::Npu if !environment.npu_runtime => Some("已选择 Intel NPU，已检测到设备，但缺少 uv 或 Python 启动器。安装完成后重新检查；选择仍保留。".into()),
            _ => None,
        }
    }
    fn model_runtime_repair(&self, provider: AsrProvider, key: &str) -> Div {
        let mut view = v_flex().w_full().min_w_0().gap_2();
        let Some(environment) = &self.environment else {
            return view;
        };
        if !environment.engine
            || (provider == AsrProvider::Coreml
                && !environment.apple
                && cfg!(all(target_os = "macos", target_arch = "aarch64")))
        {
            return view
                .child(
                    settings_value(
                        SharedString::from(format!("model-reinstall-help-{key}")),
                        "重新安装完整应用会恢复应用内组件，已保存的笔记和设置保留。",
                    )
                    .text_size(TEXT_AUX),
                )
                .child(
                    control(SharedString::from(format!("model-reinstall-{key}")))
                        .icon(icons::download())
                        .label("下载完整应用")
                        .self_start()
                        .on_click(|_, _, cx| {
                            cx.open_url("https://github.com/mizorewww/course2md/releases")
                        }),
                );
        }
        if matches!(provider, AsrProvider::Cpu | AsrProvider::Gpu) && !environment.llama {
            view = view.child(
                control(SharedString::from(format!(
                    "model-llama-install-help-{key}"
                )))
                .icon(icons::external_link())
                .label("查看 llama.cpp 安装说明")
                .self_start()
                .on_click(|_, _, cx| {
                    cx.open_url("https://github.com/ggml-org/llama.cpp#quick-start")
                }),
            );
            if cfg!(target_os = "macos") {
                view = view
                    .child(settings_value(
                        SharedString::from(format!("model-llama-install-command-{key}")),
                        "已安装 Homebrew 时，可在终端运行 brew install llama.cpp。完成后重新打开应用并检查本机能力。",
                    ).text_sm())
                    .child(control(SharedString::from(format!("model-copy-llama-install-{key}")))
                        .icon(icons::content_copy()).label("复制识别程序安装命令")
                        .self_start()
                        .on_click(|_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string("brew install llama.cpp".into()))));
            } else {
                view = view.child(settings_value(
                    SharedString::from(format!("model-llama-path-help-{key}")),
                    "安装与当前系统及设备匹配的 llama-server，并将它所在的目录加入 PATH。完成后重新打开应用并检查本机能力。",
                ).text_sm());
            }
        }
        if provider == AsrProvider::Npu && environment.npu_device && !environment.npu_runtime {
            view = view.child(
                control(SharedString::from(format!(
                    "model-python-install-help-{key}"
                )))
                .icon(icons::external_link())
                .label("查看 uv 安装说明")
                .self_start()
                .on_click(|_, _, cx| {
                    cx.open_url("https://docs.astral.sh/uv/getting-started/installation/")
                }),
            );
        }
        view
    }
    pub fn model_readiness_panel(
        &self,
        provider: AsrProvider,
        model: Option<&str>,
        root: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let request = Request::new(provider, model, root);
        let key = request.key();
        let entry = self.settings_ui.model_diagnostics.entries.get(&key);
        let device_issue = self.model_device_issue(provider);
        let mut cache_details = None;
        let mut view = v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(settings_detail_row(
                SharedString::from(format!("model-provider-label-{key}")),
                "识别方式",
                settings_value(
                    SharedString::from(format!("model-provider-{key}")),
                    provider_name(provider),
                ),
            ))
            .child(settings_detail_row(
                SharedString::from(format!("model-name-label-{key}")),
                "模型",
                settings_value(
                    SharedString::from(format!("model-name-{key}")),
                    match request.model.as_str() {
                        "qwen3-1.7b" => "Qwen3-ASR 1.7B".to_owned(),
                        "qwen3-0.6b" => "Qwen3-ASR 0.6B".to_owned(),
                        _ => request.model.clone(),
                    },
                ),
            ));
        if let Some(issue) = &device_issue {
            view = view
                .child(settings_detail_row(
                    SharedString::from(format!("model-device-label-{key}")),
                    "设备状态",
                    settings_value(
                        SharedString::from(format!("model-device-problem-{key}")),
                        issue.clone(),
                    )
                    .text_color(color(DANGER)),
                ))
                .child(self.model_runtime_repair(provider, &key));
        }
        let checking = entry.is_none_or(|entry| entry.checking);
        let status = entry
            .and_then(|entry| entry.result.as_ref())
            .and_then(|result| result.as_ref().ok());
        if checking {
            view = view.child(settings_detail_row(
                SharedString::from(format!("model-checking-label-{key}")),
                "模型状态",
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .child(crate::motion::spinner(
                        SharedString::from(format!("model-check-spinner-{key}")),
                        cx,
                    ))
                    .child(
                        settings_value(
                            SharedString::from(format!("model-checking-{key}")),
                            "正在检查模型…",
                        )
                        .role(Role::Status),
                    ),
            ));
        }
        if let Some(Err(error)) = entry.and_then(|entry| entry.result.as_ref()) {
            view = view.child(settings_detail_row(
                SharedString::from(format!("model-check-error-label-{key}")),
                "检查结果",
                settings_value(
                    SharedString::from(format!("model-check-error-{key}")),
                    format!("模型缓存检查未完成：{error}"),
                )
                .text_color(color(DANGER)),
            ));
        }
        if let Some(status) = status {
            let (kind, label, description) = match status.state {
                CacheState::Missing => (BadgeKind::Neutral, "待下载", "首次识别时自动准备。"),
                CacheState::Partial => (
                    BadgeKind::Warning,
                    "待准备",
                    "尚未下载完整，继续准备会复用已有文件。",
                ),
                CacheState::Cached => (BadgeKind::Neutral, "已下载", "尚未验证加载。"),
                CacheState::Loaded => {
                    (BadgeKind::Success, "已验证可加载", "验证后模型文件未改变。")
                }
                CacheState::Unsupported => (
                    BadgeKind::Warning,
                    "不支持",
                    "这种识别方式不支持当前模型。原选择保留，请明确选择支持的模型。",
                ),
            };
            view = view.child(settings_detail_row(
                SharedString::from(format!("model-state-label-{key}")),
                "模型状态",
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .flex_wrap()
                    .child(badge(kind).child(label))
                    .child(settings_value(
                        SharedString::from(format!("model-state-{key}")),
                        description,
                    )),
            ));
            let detail_key = key.clone();
            let cache_open = self
                .settings_ui
                .model_diagnostics
                .cache_details
                .contains(&key);
            let mut cache = v_flex().w_full().min_w_0().gap_3();
            for (index, part) in status.parts.iter().enumerate() {
                let path = part.path.clone();
                let mut part_details = v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .child(
                        settings_value(
                            SharedString::from(format!("model-cache-path-{key}-{index}")),
                            part.path.display().to_string(),
                        )
                        .text_size(TEXT_AUX)
                        .text_color(color(MUTED)),
                    )
                    .when(
                        entry.is_some_and(|entry| entry.cache_directories.contains(&path)),
                        |view| {
                            view.child(
                                quiet(SharedString::from(format!(
                                    "open-model-cache-{key}-{index}"
                                )))
                                .icon(icons::folder_open())
                                .label("打开缓存位置")
                                .accessibility_label(format!(
                                    "打开 {} 的缓存位置：{}",
                                    part.name,
                                    part.path.display()
                                ))
                                .self_end()
                                .on_click(move |_, _, cx| cx.open_with_system(&path)),
                            )
                        },
                    );
                if !part.missing.is_empty() {
                    part_details = part_details.child(settings_value(
                        SharedString::from(format!("model-missing-files-{key}-{index}")),
                        format!("缺少或尚未验证：{}", part.missing.join("、")),
                    ));
                }
                cache = cache.child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .p_4()
                        .bg(color(SURFACE))
                        .border_1()
                        .border_color(color(CARD_LINE))
                        .rounded(RADIUS_CARD)
                        .child(
                            semantic_label(
                                SharedString::from(format!("model-cache-part-label-{key}-{index}")),
                                part.name.clone(),
                                icons::storage(),
                            )
                            .w_full()
                            .min_w_0(),
                        )
                        .child(part_details),
                );
            }
            cache_details = Some(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_3()
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap_3()
                            .flex_wrap()
                            .child(
                                quiet(SharedString::from(format!("model-cache-details-{key}")))
                                    .icon(icons::folder_open())
                                    .label(if cache_open {
                                        "收起缓存详情"
                                    } else {
                                        "查看缓存详情"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this
                                            .settings_ui
                                            .model_diagnostics
                                            .cache_details
                                            .remove(&detail_key)
                                        {
                                            this.settings_ui
                                                .model_diagnostics
                                                .cache_details
                                                .insert(detail_key.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .when(status.bytes > 0, |row| {
                                row.child(
                                    settings_value(
                                        SharedString::from(format!("model-cached-size-{key}")),
                                        format!("缓存占用 {}", bytes(status.bytes)),
                                    )
                                    .text_size(TEXT_AUX)
                                    .text_color(color(MUTED)),
                                )
                            }),
                    )
                    .child(crate::motion::disclosure(
                        SharedString::from(format!("model-cache-content-{key}")),
                        cache_open,
                        cache,
                        window,
                        cx,
                    )),
            );
            if let Some(error) = &status.last_error {
                view = view.child(settings_detail_row(
                    SharedString::from(format!("model-last-error-label-{key}")),
                    "上次准备",
                    settings_value(
                        SharedString::from(format!("model-last-error-{key}")),
                        format!("未完成：{error}"),
                    )
                    .text_color(color(DANGER)),
                ));
            }
        }
        let active = self.kind == Kind::Models
            && self.job.is_some()
            && self
                .settings_ui
                .model_diagnostics
                .preparing
                .as_ref()
                .is_some_and(|active| active.key() == key);
        let request_for_check = request.clone();
        let mut actions = h_flex().w_full().min_w_0().gap_2().flex_wrap().child(
            control(SharedString::from(format!("recheck-model-{key}")))
                .icon(icons::refresh())
                .label("重新检查模型")
                .disabled(checking || active)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.check_model_request(request_for_check.clone(), true, cx)
                })),
        );
        let prepare_allowed = self.environment.is_some()
            && device_issue.is_none()
            && status.is_some_and(|status| status.can_prepare);
        if active {
            view = view.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(crate::motion::spinner("model-prepare-spinner", cx))
                    .child(
                        settings_value(
                            "active-model-prepare",
                            format!("正在准备 {}", request.model),
                        )
                        .role(Role::Status),
                    ),
            );
            for (index, (stage, progress)) in self
                .progress
                .iter()
                .filter(|(stage, progress)| {
                    (stage.starts_with("model") || stage.contains("download"))
                        && progress.has_samples()
                        && !progress.done
                })
                .enumerate()
            {
                let label = progress.detail(stage, true);
                view = view
                    .child(
                        settings_value(
                            ("model-download-progress", index),
                            format!("{} · {label}", activity::title(stage)),
                        )
                        .text_sm(),
                    )
                    .when_some(progress.fraction(), |view, fraction| {
                        view.child(crate::motion::progress(
                            ("model-download-bar", index),
                            fraction,
                            window,
                            cx,
                        ))
                    });
            }
            actions = actions.child(
                control("stop-model-preparation")
                    .icon(icons::pause())
                    .label(if self.cancelling {
                        "正在停止…"
                    } else {
                        "暂停准备"
                    })
                    .disabled(self.cancelling)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(job) = &this.job {
                            job.cancel();
                            this.cancelling = true;
                            cx.notify();
                        }
                    })),
            );
        } else if prepare_allowed {
            let loaded = status.is_some_and(|status| status.state == CacheState::Loaded);
            let label = match status.map(|status| &status.state) {
                Some(CacheState::Missing) => "下载并准备模型",
                Some(CacheState::Loaded) => "重新验证加载",
                Some(CacheState::Cached)
                    if matches!(provider, AsrProvider::Cpu | AsrProvider::Gpu) =>
                {
                    "检查模型文件"
                }
                Some(CacheState::Cached) => "验证模型加载",
                _ => "继续准备模型",
            };
            actions = actions.child(
                control(SharedString::from(format!("prepare-model-{key}")))
                    .icon(if loaded {
                        icons::refresh()
                    } else {
                        icons::download()
                    })
                    .when(loaded, |button| button.ghost())
                    .when(!loaded, |button| button.primary())
                    .label(label)
                    .accessibility_label(format!(
                        "{label}：{} · {}",
                        provider_name(provider),
                        request.model
                    ))
                    .disabled(self.job.is_some())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.begin_model_preparation(request.clone(), cx)
                    })),
            );
            if self.job.is_some() && !loaded {
                view = view.child(
                    settings_value(
                        SharedString::from(format!("model-waits-for-job-{key}")),
                        "当前处理结束后可准备模型；本次已保存的任务不受影响。",
                    )
                    .text_sm(),
                );
            }
            if !loaded {
                view = view.child(
                    settings_value(
                        SharedString::from(format!("model-network-scope-{key}")),
                        "准备时可能下载模型，课程内容不会上传。",
                    )
                    .text_sm()
                    .text_color(color(MUTED)),
                );
            }
        }
        view.child(actions)
            .when_some(cache_details, |view, details| view.child(details))
    }
    pub(super) fn model_diagnostics_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let request = self.default_model_request();
        let mut current = settings_detail_group("model-diagnostic-default", "默认识别模型");
        if request.provider == AsrProvider::Api {
            current = current.child(crate::settings_ui::setting_surface().child(
                settings_detail_row(
                    "default-asr-provider-label",
                    "识别方式",
                    settings_value("default-asr-is-service", "语音服务，本机模型不参与。"),
                ),
            ));
        } else {
            current = current.child(crate::settings_ui::setting_surface().child(
                self.model_readiness_panel(
                    request.provider,
                    Some(&request.model),
                    &request.root,
                    window,
                    cx,
                ),
            ));
        }
        let mut view = v_flex().w_full().min_w_0().gap_6().child(current);
        if self.kind == Kind::Models
            && self.job.is_some()
            && let Some(active) = &self.settings_ui.model_diagnostics.preparing
            && active.key() != request.key()
        {
            view = view.child(
                settings_detail_group("other-active-model", "正在准备的模型").child(
                    self.model_readiness_panel(
                        active.provider,
                        Some(&active.model),
                        &active.root,
                        window,
                        cx,
                    ),
                ),
            );
        }
        if let Some((message, error)) = &self.settings_ui.model_diagnostics.result {
            view = view.child(
                settings_value("model-prepare-result", message.clone())
                    .role(Role::Status)
                    .text_color(color(if *error { DANGER } else { INK })),
            );
        }
        if self.settings_ui.model_diagnostics.preparing.is_some()
            && !self.logs.is_empty()
            && self.kind == Kind::Models
        {
            view = view.child(
                control("model-preparation-details")
                    .icon(icons::info())
                    .ghost()
                    .label(if self.settings_ui.model_diagnostics.details {
                        "收起准备详情"
                    } else {
                        "查看准备详情"
                    })
                    .self_start()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_ui.model_diagnostics.details =
                            !this.settings_ui.model_diagnostics.details;
                        cx.notify();
                    })),
            );
            if self.settings_ui.model_diagnostics.details {
                view = view.child(
                    settings_detail_group("model-preparation-log-heading", "准备日志").child(
                        settings_value(
                            "model-preparation-log",
                            self.logs.iter().cloned().collect::<Vec<_>>().join("\n"),
                        )
                        .text_size(TEXT_AUX),
                    ),
                );
            }
        }
        view
    }
    pub(super) fn model_hardware_details(&self, cx: &mut Context<Self>) -> Div {
        let request = self.default_model_request();
        let Some(environment) = &self.environment else {
            return v_flex();
        };
        let mut hardware = settings_detail_group("model-hardware-heading", "设备与运行时");
        for (id, label, value) in [
            ("cpu-device", "CPU 架构", std::env::consts::ARCH),
            (
                "apple-device",
                "Apple 平台",
                if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
                    "原生运行于 Apple 芯片"
                } else {
                    "当前平台不适用"
                },
            ),
            (
                "gpu-device",
                "GPU",
                environment.gpu.as_deref().unwrap_or(if environment.llama {
                    "识别运行时未报告可用设备"
                } else {
                    "缺少 llama-server，尚无法检查"
                }),
            ),
            (
                "npu-device",
                "Intel NPU",
                if environment.npu_device {
                    "已检测到设备"
                } else {
                    "未检测到设备"
                },
            ),
            (
                "npu-runtime",
                "NPU 启动器",
                if environment.npu_runtime {
                    "已找到；首次准备时验证 OpenVINO 及设备编译"
                } else {
                    "未找到 uv 或 Python"
                },
            ),
            (
                "apple-runtime",
                "Apple 识别程序",
                if environment.apple {
                    "已检测到原生运行时与 Metal 资源"
                } else {
                    "未检测到完整运行时与 Metal 资源"
                },
            ),
        ]
        .into_iter()
        .filter(|(id, _, _)| match *id {
            "apple-device" | "apple-runtime" => cfg!(target_os = "macos"),
            "npu-device" | "npu-runtime" => {
                !cfg!(target_os = "macos") || request.provider == AsrProvider::Npu
            }
            _ => true,
        }) {
            hardware = hardware.child(settings_detail_row(
                SharedString::from(format!("{id}-label")),
                label,
                settings_value(id, value.to_owned()),
            ));
        }
        let alternatives = [
            (AsrProvider::Coreml, environment.apple),
            (
                AsrProvider::Gpu,
                environment.gpu.is_some() && environment.llama,
            ),
            (AsrProvider::Cpu, environment.llama),
            (AsrProvider::Npu, environment.npu),
        ];
        let available = alternatives
            .into_iter()
            .filter(|(_, available)| *available)
            .map(|(provider, _)| provider_name(provider))
            .collect::<Vec<_>>();
        if !available.is_empty() {
            hardware = hardware.child(settings_detail_row(
                "available-local-model-providers-label",
                "可用识别方式",
                settings_value("available-local-model-providers", available.join("、")),
            ));
        }
        if self.model_device_issue(request.provider).is_some() {
            hardware = hardware.child(
                h_flex().gap_2().flex_wrap().children(
                    alternatives
                        .into_iter()
                        .filter(|(provider, available)| *available && *provider != request.provider)
                        .map(|(provider, _)| {
                            control(SharedString::from(format!(
                                "choose-available-model-{}",
                                provider.as_str()
                            )))
                            .icon(icons::microphone())
                            .label(format!("默认改用 {}", provider_name(provider)))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let mut value = this.generation_edit_base();
                                    value.select_provider(Some(provider));
                                    value.options.asr_model = Some("qwen3-1.7b".into());
                                    if this.commit_generation(value, cx) {
                                        this.refresh_model_diagnostics(cx);
                                    }
                                },
                            ))
                        }),
                ),
            );
        }
        hardware
    }
    pub(super) fn default_model_readiness_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let request = self.default_model_request();
        self.model_readiness_panel(
            request.provider,
            Some(&request.model),
            &request.root,
            window,
            cx,
        )
    }

    /// One-line readiness conclusion for the generation group main flow; the full
    /// panel stays behind 技术详情.
    pub(super) fn default_model_conclusion(&self) -> (bool, String) {
        let request = self.default_model_request();
        if request.provider == AsrProvider::Api {
            return (true, "当前使用语音服务识别，本机模型不参与。".into());
        }
        if self.model_device_issue(request.provider).is_some() {
            return (
                false,
                "所选识别方式暂不可用，可在模型管理中查看原因。".into(),
            );
        }
        let entry = self
            .settings_ui
            .model_diagnostics
            .entries
            .get(&request.key());
        if entry.is_none_or(|entry| entry.checking) {
            return (true, "正在检查识别模型…".into());
        }
        if entry
            .and_then(|entry| entry.result.as_ref())
            .is_some_and(Result::is_err)
        {
            return (
                false,
                "模型检查未完成，可在模型管理中查看原因并重试。".into(),
            );
        }
        match entry
            .and_then(|entry| entry.result.as_ref())
            .and_then(|result| result.as_ref().ok())
            .map(|status| &status.state)
        {
            Some(CacheState::Loaded) => (true, "识别模型已就绪。".into()),
            Some(CacheState::Cached) => (true, "模型已下载，尚未验证加载。".into()),
            Some(CacheState::Missing | CacheState::Partial) => {
                (true, "首次识别时会自动下载所选模型。".into())
            }
            Some(CacheState::Unsupported) => {
                (false, "当前识别方式不支持所选模型，请选择其他模型。".into())
            }
            None => (true, "尚未检查识别模型。".into()),
        }
    }
}
