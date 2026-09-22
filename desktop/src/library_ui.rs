use super::*;
use crate::theme::*;
use anyhow::Context as _;
use gpui_component::{
    button::*,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct FolderOrigin {
    pub root: PathBuf,
    pub draft_id: Option<String>,
}

/// Resolved picker state shared by the full picker and the compact chip.
#[derive(Clone)]
pub(crate) struct FolderContext {
    pub(crate) storage: Option<PathBuf>,
    pub(crate) origin: FolderOrigin,
    pub(crate) folder: Option<u64>,
    pub(crate) load_error: Option<String>,
    pub(crate) label: String,
    pub(crate) folders: BTreeMap<u64, String>,
}

struct FolderDialog {
    desktop: Entity<Desktop>,
    title: &'static str,
    _observation: Subscription,
}

impl Render for FolderDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("folder-dialog-content")
            .role(Role::Dialog)
            .aria_label(self.title)
            .child(
                self.desktop
                    .update(cx, |desktop, cx| desktop.folder_editor_view(cx)),
            )
    }
}

impl Desktop {
    pub fn begin_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_video_from_action(window, cx);
        if self.online && self.value(Field::Source, cx).is_empty() {
            self.inputs[&Field::Source].update(cx, |state, cx| state.focus(window, cx));
        }
    }

    pub fn invalidate_source(&mut self) {
        self.pending_conversion = None;
        if let Some(cancel) = self.preview_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(cancel) = self.subtitle_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.subtitle_generation = self.subtitle_generation.wrapping_add(1);
        self.source_preview = None;
        self.source_editor_open = false;
        self.generation_options_open = false;
        self.source_candidates.clear();
        self.source_collection_title = None;
        self.subtitle_loading = false;
        self.subtitle_error = None;
        self.preview_error = None;
        self.source_validation = None;
        self.show_preview_details = false;
        self.expanded_subtitle_issue = None;
    }

    pub fn inspect_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut input = self.value(Field::Source, cx);
        if !self.prepare_next_import(&input, window, cx) {
            return;
        }
        if self.preview_cancel.is_some() && self.last_source_input == input {
            return;
        }
        let previous_source = self.source_preview.clone();
        self.completed_source = None;
        self.invalidate_source();
        if input.is_empty() {
            self.source_validation = Some(
                if self.online {
                    "先粘贴视频链接"
                } else {
                    "先选择一个视频"
                }
                .into(),
            );
            cx.notify();
            return;
        }
        if self.online {
            let links = source::video_links(&input);
            if links.len() > 1 {
                self.source_candidates = links
                    .into_iter()
                    .map(|input| source::SourceCandidate {
                        title: input.clone(),
                        input,
                        identity: None,
                        duration: None,
                    })
                    .collect();
                self.source_collection_title = Some("分享内容中有多个链接，请选择一个视频".into());
                self.save_current_draft(cx);
                cx.notify();
                return;
            }
            if let Some(link) = links.into_iter().next() {
                if input != link {
                    self.inputs[&Field::Source].update(cx, |state, cx| {
                        state.set_value(link.clone(), window, cx);
                    });
                    input = link;
                }
            }
            if let Err(error) = source::validate_url(&input) {
                self.source_validation = Some(error.to_string());
                cx.notify();
                return;
            }
        }
        self.last_source_input = input.clone();
        if !self.save_current_draft(cx) {
            return;
        }
        let token = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.state.draft())
            .map(|draft| (draft.id.clone(), draft.revision));
        let generation = self.preview_generation;
        let request_input = input.clone();
        self.preview_workers += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.preview_cancel = Some(cancel.clone());
        let online = self.online;
        // 用发起探测的窗口而不是“第一个窗口”：回调只可能落回正确的窗口
        let handle = Some(window.window_handle());
        // probe 是同步网络/子进程工作；见 crate::spawn_blocking_io 的说明
        let task = crate::spawn_blocking_io(move || source::probe(input, online, cancel));
        cx.spawn(async move |this, cx| {
            let result = task
                .recv()
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("识别工作线程意外结束")));
            // worker 计数无论窗口存亡都必须归还（request_close 等待它归零）
            let _ = this.update(cx, |this, _| {
                this.preview_workers = this.preview_workers.saturating_sub(1);
            });
            if let Some(handle) = handle {
                let _ = cx.update_window(handle, |_, window, cx| this.update(cx, |this, cx| {
                    if this.preview_generation != generation || this.value(Field::Source, cx) != request_input { return; }
                    if let Some((id, revision)) = &token {
                        if !this.workspace.as_ref().is_some_and(|workspace| workspace.state.matches_input(id, *revision)) { return; }
                    }
                    this.preview_cancel = None;
                    match result {
                        Ok(source::SourceProbe::Single(mut source)) => {
                            if let course2md::subtitle::SubtitleEvidence::Found { tracks, .. } = &mut source.subtitles {
                                course2md::subtitle::sort_tracks(tracks, &this.preferences.generation().preferred_subtitle_languages, "zh-Hans", source.original_language.as_deref());
                            }
                            let title = this.workspace.as_ref().and_then(|workspace| workspace.state.draft())
                                .filter(|draft| draft.custom_title).map(|draft| draft.title.clone()).unwrap_or_else(|| source.title.clone());
                            this.draft_loading = true;
                            this.inputs[&Field::Title].update(cx, |state, cx| state.set_value(title, window, cx));
                            this.draft_loading = false;
                            let previous_source = previous_source.as_ref().filter(|previous| previous.identity == source.identity);
                            let wanted = previous_source.and_then(|previous| previous.subtitle_request.as_ref().map(|track| track.id.clone()).or_else(|| previous.selected_subtitle.as_ref().map(|subtitle| subtitle.track_id.clone())));
                            let default_track = match &wanted {
                                Some(id) => source.subtitles.tracks().iter().find(|track| &track.id == id).cloned().or_else(|| previous_source.and_then(|previous| previous.subtitles.tracks().iter().find(|track| &track.id == id && matches!(track.origin, course2md::subtitle::SubtitleOrigin::File { .. })).cloned())),
                                None => source.subtitles.tracks().first().cloned(),
                            };
                            if let Some(previous) = previous_source { source.selected_subtitle = previous.selected_subtitle.clone(); }
                            if wanted.is_some() && default_track.is_none() {
                                let message = "原来选择的字幕暂时无法读取。已确认的正文仍保留，请明确选择要使用的文字来源。".to_owned();
                                this.subtitle_error = Some(message.clone());
                                source.subtitle_read_error = Some(course2md::subtitle::SubtitleReadError::Failed { message });
                            }
                            this.source_preview = Some(source);
                            this.save_current_draft(cx);
                            if this.task_options.source_mode != 2 && let Some(track) = default_track {
                                this.confirm_subtitle(track, false, cx);
                            }
                        }
                        Ok(source::SourceProbe::Collection { title, candidates, unavailable_entries }) => {
                            this.source_collection_title = Some(if title.is_empty() { "请选择本次处理的视频".into() } else { title });
                            this.source_candidates = candidates;
                            if this.source_candidates.is_empty() {
                                this.preview_error = Some("还无法确定要处理哪个视频。请复制具体视频的链接。".into());
                            } else if unavailable_entries > 0 {
                                this.preview_error = Some(format!("另有 {unavailable_entries} 个条目暂时无法确认。可选择下列视频，或复制具体视频的链接。"));
                            }
                        }
                        Ok(source::SourceProbe::Unresolved { message }) => this.preview_error = Some(message),
                        Err(error) => this.preview_error = Some(format!("{error:#}")),
                    }
                    this.advance_conversion(window, cx);
                    cx.notify();
                }));
            }
        }).detach();
        cx.notify();
    }
    fn folder_name(&self, id: Option<u64>) -> String {
        match id.filter(|id| *id != 0) {
            None => "未分类".into(),
            Some(id) => self
                .library
                .folders
                .get(&id)
                .cloned()
                .unwrap_or_else(|| "已删除的文件夹".into()),
        }
    }

    fn current_folder_origin(&self) -> FolderOrigin {
        if self.page == Page::New {
            if let Some(workspace) = &self.workspace {
                if let Some(draft) = workspace.state.draft() {
                    if let Some(library) = workspace.state.library(&draft.library_id) {
                        return FolderOrigin {
                            root: library.root.clone(),
                            draft_id: Some(draft.id.clone()),
                        };
                    }
                }
            }
        }
        FolderOrigin {
            root: self.library_root.clone(),
            draft_id: None,
        }
    }

    pub fn begin_folder(&mut self, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_saving {
            return;
        }
        if !matches!(self.page, Page::Library | Page::New) {
            self.navigate(Page::Library, cx);
        }
        let mut origin = self.current_folder_origin();
        self.folder_targets = self
            .workspace
            .as_ref()
            .map(|w| {
                w.state
                    .libraries
                    .iter()
                    .filter(|library| {
                        (self.page == Page::Library && id.is_none() && self.folder_filter.is_none())
                            || library.root == origin.root
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if let Some(target) = self.folder_targets.first() {
            origin.root = target.root.clone();
        }
        let name = id
            .and_then(|id| self.library_indexes.get(&origin.root)?.folders.get(&id))
            .cloned()
            .unwrap_or_default();
        self.folder_origin = Some(origin);
        self.folder_editor = Some(id);
        self.folder_error = None;
        self.delete_folder = None;
        self.inputs[&Field::FolderName].update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        let desktop = cx.entity();
        let title = if id.is_some() {
            "重命名文件夹"
        } else {
            "新建文件夹"
        };
        let content = cx.new(|cx| FolderDialog {
            _observation: cx.observe(&desktop, |_, _, cx| cx.notify()),
            desktop,
            title,
        });
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title(title)
                .w(px(420.))
                .overlay_closable(false)
                .close_button(false)
                .keyboard(true)
                .child(content.clone())
                .on_close(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this
                            .batch_import
                            .as_ref()
                            .is_some_and(|batch| batch.folder.is_none())
                        {
                            this.batch_import = None;
                            this.message = Some(
                                "批量处理已取消；必须新建一个笔记文件夹才能继续".into(),
                            );
                        }
                        this.folder_editor = None;
                        this.folder_origin = None;
                        this.folder_error = None;
                        cx.notify();
                    });
                })
        });
        let input = self.inputs[&Field::FolderName].clone();
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx))
        });
        cx.notify();
    }

    pub fn save_folder(&mut self, cx: &mut Context<Self>) {
        if self.folder_saving {
            return;
        }
        let Some(id) = self.folder_editor else {
            return;
        };
        let Some(origin) = self.folder_origin.clone() else {
            return;
        };
        let Some(handle) = cx.windows().first().copied() else {
            return;
        };
        let name = self.value(Field::FolderName, cx);
        self.folder_saving = true;
        self.folder_error = None;
        self.preview_workers += 1;
        let root = origin.root.clone();
        let task = crate::spawn_blocking_io(move || {
            let mut saved = None;
            let library = organize::Library::edit(&root, |library| {
                saved = Some(library.rename(id, &name)?);
                Ok(())
            })?;
            Ok::<_, anyhow::Error>((library, saved))
        });
        cx.spawn(async move |this, cx| {
            let result = task
                .recv()
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("文件夹保存线程意外结束")));
            let _ = this.update(cx, |this, _| {
                this.preview_workers = this.preview_workers.saturating_sub(1);
                this.folder_saving = false;
            });
            let _ = cx.update_window(handle, |_, window, cx| {
                this.update(cx, |this, cx| {
                    this.finish_folder_save(origin, result, window, cx);
                })
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_folder_save(
        &mut self,
        origin: FolderOrigin,
        result: anyhow::Result<(organize::Library, Option<u64>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((library, saved)) => {
                if self.library_root == origin.root {
                    self.library = library.clone();
                }
                self.library_indexes
                    .insert(origin.root.clone(), library.clone());
                self.refresh_library(cx);
                if let Some(draft_id) = &origin.draft_id {
                    if let Some(workspace) = &mut self.workspace {
                        if let Err(error) = workspace.transaction(|state| {
                            let draft = state
                                .drafts
                                .iter_mut()
                                .find(|draft| &draft.id == draft_id)
                                .context("原输入已改变，请在当前输入中重新选择文件夹")?;
                            anyhow::ensure!(
                                draft.submitted_task.is_none(),
                                "原输入已提交，请在新输入中选择文件夹"
                            );
                            let location = state
                                .libraries
                                .iter()
                                .find(|library| library.id == draft.library_id)
                                .context("原课程库已不存在")?;
                            anyhow::ensure!(
                                location.root == origin.root,
                                "输入的课程库已改变，请重新选择文件夹"
                            );
                            draft.folder = saved;
                            Ok(())
                        }) {
                            self.folder_error =
                                Some(format!("文件夹已创建，保存位置尚未更新：{error:#}"));
                            self.folder_editor = Some(saved);
                            cx.notify();
                            return;
                        }
                        if workspace.state.current_draft == *draft_id {
                            self.target_folder = saved;
                        }
                    }
                } else {
                    self.library_root = origin.root;
                    self.library = library;
                    self.folder_filter = saved;
                    self.inputs[&Field::Search]
                        .update(cx, |state, cx| state.set_value("", window, cx));
                    self.library_list.scroll_to(ListOffset {
                        item_ix: 0,
                        offset_in_item: px(0.),
                    });
                    self.page = Page::Library;
                }
                self.folder_editor = None;
                self.folder_origin = None;
                self.folder_error = None;
                if let Some(folder) = saved
                    && self.batch_import.is_some()
                {
                    self.start_batch_import(folder, cx);
                }
                window.close_dialog(cx);
            }
            Err(error) => self.folder_error = Some(format!("{error:#}")),
        }
        cx.notify();
    }

    pub fn folder_editor_view(&self, cx: &mut Context<Self>) -> Div {
        let mut view = v_flex().gap_3();
        if let Some(id) = self.folder_editor {
            if let Some(origin) = &self.folder_origin {
                let label = self
                    .folder_targets
                    .iter()
                    .find(|library| library.root == origin.root)
                    .map(|library| format!("{} · {}", library.name, library.root.display()))
                    .unwrap_or_else(|| origin.root.display().to_string());
                view = view.child(
                    accessible_text("folder-library-label", "所属课程库")
                        .font_weight(FontWeight::MEDIUM),
                );
                if self.folder_targets.len() > 1 && id.is_none() {
                    let targets = self.folder_targets.clone();
                    let root = origin.root.clone();
                    let entity = cx.entity().downgrade();
                    view =
                        view.child(
                            control("folder-library")
                                .icon(IconName::FolderOpen)
                                .w_full()
                                .min_w_0()
                                .disabled(self.folder_saving)
                                .tooltip(label.clone())
                                .child(div().flex_1().min_w_0().text_ellipsis().child(label))
                                .child(Icon::new(IconName::ChevronDown).size_4())
                                .dropdown_menu(move |menu, _, _| {
                                    targets.iter().fold(menu, |menu, target| {
                                        let target_root = target.root.clone();
                                        let entity = entity.clone();
                                        menu.item(
                                            PopupMenuItem::new(format!(
                                                "{} · {}",
                                                target.name,
                                                target.root.display()
                                            ))
                                            .checked(target.root == root)
                                            .on_click(move |_, _, cx| {
                                                let _ = entity.update(cx, |this, cx| {
                                                    if !this.folder_saving
                                                        && let Some(origin) =
                                                            &mut this.folder_origin
                                                    {
                                                        origin.root = target_root.clone();
                                                        this.folder_error = None;
                                                        cx.notify();
                                                    }
                                                });
                                            }),
                                        )
                                    })
                                }),
                        );
                } else {
                    view = view.child(accessible_text("folder-library-value", label).text_color(color(MUTED)));
                }
            }
            view = view
                .child(
                    accessible_text("folder-name-label", "文件夹名称")
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text_input(&self.inputs[&Field::FolderName])
                        .disabled(self.folder_saving)
                        .aria_label("文件夹名称")
                        .when(self.folder_error.is_some(), |v| {
                            v.border_color(color(DANGER))
                        }),
                )
                .when_some(self.folder_error.clone(), |v, error| {
                    v.child(
                        accessible_text("folder-editor-error", error)
                            .text_sm()
                            .text_color(color(DANGER)),
                    )
                })
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            control("cancel-folder")
                                .icon(IconName::Close)
                                .h_auto()
                                .min_h(rems(2.6))
                                .ghost()
                                .label("取消")
                                .disabled(self.folder_saving)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this
                                        .batch_import
                                        .as_ref()
                                        .is_some_and(|batch| batch.folder.is_none())
                                    {
                                        this.batch_import = None;
                                        this.message = Some(
                                            "批量处理已取消；必须新建一个笔记文件夹才能继续"
                                                .into(),
                                        );
                                    }
                                    this.folder_editor = None;
                                    this.folder_origin = None;
                                    this.folder_error = None;
                                    window.close_dialog(cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            control("save-folder")
                                .icon(if id.is_some() {
                                    icons::edit()
                                } else {
                                    icons::create_new_folder()
                                })
                                .h_auto()
                                .min_h(rems(2.6))
                                .primary()
                                .disabled(self.folder_saving)
                                .loading(self.folder_saving)
                                .label(if id.is_some() {
                                    "保存名称"
                                } else {
                                    "创建文件夹"
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.save_folder(cx))),
                        ),
                );
        }
        if let Some(id) = self.delete_folder {
            let origin = self
                .folder_origin
                .clone()
                .unwrap_or_else(|| self.current_folder_origin());
            let name = self
                .library_indexes
                .get(&origin.root)
                .and_then(|library| library.folders.get(&id))
                .cloned()
                .unwrap_or_else(|| self.folder_name(Some(id)));
            view =
                view.p_4()
                    .rounded_lg()
                    .bg(color(SURFACE))
                    .border_1()
                    .border_color(color(LINE))
                    .child(accessible_text(
                        "folder-delete-title",
                        format!("删除「{name}」文件夹？"),
                    ))
                    .child(
                        accessible_text(
                            "folder-delete-description",
                            "其中的课程会回到未分类，笔记和原视频都会保留。",
                        )
                        .text_color(color(MUTED)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(control("keep-folder").label("保留文件夹").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.delete_folder = None;
                                    this.folder_origin = None;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                control("delete-folder")
                                    .label("删除文件夹，保留课程")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        match organize::Library::edit(&origin.root, |library| {
                                            library.remove(id);
                                            Ok(())
                                        }) {
                                            Ok(library) => {
                                                if this.library_root == origin.root {
                                                    this.library = library.clone();
                                                }
                                                this.library_indexes
                                                    .insert(origin.root.clone(), library);
                                                this.delete_folder = None;
                                                this.folder_origin = None;
                                                if this.library_root == origin.root
                                                    && this.folder_filter == Some(id)
                                                {
                                                    this.folder_filter = Some(0);
                                                }
                                                // Drafts keep the deleted folder reference so the
                                                // next submission asks for a deliberate new destination.
                                            }
                                            Err(e) => {
                                                this.message =
                                                    Some(format!("无法删除文件夹：{e:#}"))
                                            }
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );
        }
        view
    }

    pub fn begin_delete_folder(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let origin = self.current_folder_origin();
        let name = self
            .library_indexes
            .get(&origin.root)
            .and_then(|library| library.folders.get(&id))
            .cloned()
            .unwrap_or_else(|| self.folder_name(Some(id)));
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("删除「{name}」文件夹？"),
            Some("其中的笔记会回到未分类，笔记正文和原视频都会保留。"),
            &["删除文件夹", "保留文件夹"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await.ok() != Some(0) {
                return;
            }
            let _ = this.update_in(cx, |this, _, cx| {
                match organize::Library::edit(&origin.root, |library| {
                    library.remove(id);
                    Ok(())
                }) {
                    Ok(library) => {
                        if this.library_root == origin.root {
                            this.library = library.clone();
                            if this.folder_filter == Some(id) {
                                this.folder_filter = Some(0);
                            }
                        }
                        this.library_indexes.insert(origin.root, library);
                    }
                    Err(error) => this.message = Some(format!("无法删除文件夹：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn folder_context(&self, course: Option<PathBuf>) -> FolderContext {
        let storage = course.as_ref().map(|path| {
            self.courses
                .iter()
                .find(|course| &course.dir == path || &course.storage_dir() == path)
                .map(Course::storage_dir)
                .unwrap_or_else(|| path.clone())
        });
        let origin = if let Some(path) = &storage {
            let root = self
                .library_view_cache
                .locations
                .get(path)
                .map(|location| location.root.clone())
                .unwrap_or_else(|| self.library_root.clone());
            FolderOrigin {
                root,
                draft_id: None,
            }
        } else {
            self.current_folder_origin()
        };
        let loaded = self.library_indexes.get(&origin.root);
        let checking = loaded.is_none() && self.loading;
        let load_error =
            (loaded.is_none() && !checking).then(|| "文件夹暂不可用，请刷新课程库。".to_owned());
        let folder = storage
            .as_ref()
            .and_then(|path| self.library_view_cache.locations.get(path))
            .filter(|location| location.root == origin.root)
            .and_then(|location| loaded?.folder_key(&location.relative))
            .or_else(|| {
                if storage.is_none() {
                    self.target_folder
                } else {
                    None
                }
            });
        let label = if checking {
            "正在读取文件夹…".to_owned()
        } else if load_error.is_some() {
            "文件夹暂不可用".to_owned()
        } else if let Some(id) = folder {
            loaded
                .and_then(|organization| organization.folders.get(&id))
                .cloned()
                .unwrap_or_else(|| "文件夹已删除，请重新选择".into())
        } else {
            "未分类".into()
        };
        FolderContext {
            storage,
            origin,
            folder,
            load_error,
            label,
            folders: loaded
                .map(|organization| organization.folders.clone())
                .unwrap_or_default(),
        }
    }

    pub(crate) fn folder_menu(
        entity: WeakEntity<Desktop>,
        origin: FolderOrigin,
        storage: Option<PathBuf>,
        folders: BTreeMap<u64, String>,
        current: Option<u64>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        move |menu, _, _| {
            let mut entries = vec![(None, "未分类".to_owned())];
            entries.extend(folders.iter().map(|(id, name)| (Some(*id), name.clone())));
            entries.into_iter().fold(menu, |menu, (id, name)| {
                let entity = entity.clone();
                let storage = storage.clone();
                let origin = origin.clone();
                menu.item(
                    PopupMenuItem::new(name)
                        .icon(IconName::Folder)
                        .checked(id == current)
                        .on_click(move |_, _, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                if let Some(path) = &storage {
                                    match organize::Library::edit(&origin.root, |library| {
                                        library.assign(&origin.root, path, id)
                                    }) {
                                        Ok(library) => {
                                            if this.library_root == origin.root {
                                                this.library = library.clone();
                                            }
                                            this.library_indexes
                                                .insert(origin.root.clone(), library);
                                        }
                                        Err(error) => {
                                            this.message = Some(format!("无法移动笔记：{error:#}"))
                                        }
                                    }
                                } else if this
                                    .workspace
                                    .as_ref()
                                    .and_then(|workspace| workspace.state.draft())
                                    .is_some_and(|draft| {
                                        Some(&draft.id) == origin.draft_id.as_ref()
                                    })
                                {
                                    this.target_folder = id;
                                    this.save_current_draft(cx);
                                }
                                cx.notify();
                            });
                        }),
                )
            })
        }
    }

    pub fn folder_picker(
        &self,
        course: Option<PathBuf>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_course = course.is_some();
        let context = self.folder_context(course);
        let load_error = context.load_error.clone();
        let label = context.label.clone();
        let picker = control(("folder-picker", index))
            .w_full()
            .min_w_0()
            .when(has_course, |button| button.ghost())
            .h_auto()
            .min_h(rems(2.6))
            .py_2()
            .disabled(self.loading || load_error.is_some())
            .icon(IconName::Folder)
            .accessibility_label(format!("保存到文件夹：{label}"))
            .tooltip(label.clone())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .whitespace_normal()
                    .text_ellipsis()
                    .line_clamp(2)
                    .child(label),
            )
            .child(Icon::new(IconName::ChevronDown).size_4().flex_shrink_0())
            .dropdown_menu(Self::folder_menu(
                cx.entity().downgrade(),
                context.origin,
                context.storage,
                context.folders,
                context.folder,
            ));
        v_flex()
            .gap_1()
            .child(picker)
            .when_some(load_error, |view, error| {
                view.child(
                    accessible_text(("folder-picker-error", index), error)
                        .text_sm()
                        .text_color(color(DANGER)),
                )
            })
    }
}
