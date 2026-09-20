use super::*;
use crate::theme::*;
use anyhow::{Context as _, Result};
use gpui_component::button::Button;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FolderKey {
    library_id: String,
    root: PathBuf,
    folder_id: u64,
}

#[derive(Clone, Debug)]
struct Pending {
    generation: u64,
    key: FolderKey,
    folder_name: String,
    completed: usize,
    total: Option<usize>,
}

#[derive(Clone, Debug)]
struct Failure {
    title: String,
    reason: String,
}

#[derive(Clone, Debug)]
struct Report {
    output: Option<PathBuf>,
    total: usize,
    failures: Vec<Failure>,
}

impl Report {
    fn succeeded(&self) -> usize {
        self.total.saturating_sub(self.failures.len())
    }
}

#[derive(Clone, Debug)]
struct Feedback {
    folder_name: String,
    result: std::result::Result<Report, String>,
}

#[derive(Default)]
pub(crate) struct State {
    generation: u64,
    pending: Option<Pending>,
    feedback: BTreeMap<FolderKey, Feedback>,
    details_open: BTreeSet<FolderKey>,
}

impl State {
    fn begin(&mut self, key: FolderKey, folder_name: String) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.feedback.remove(&key);
        self.details_open.remove(&key);
        self.pending = Some(Pending {
            generation: self.generation,
            key,
            folder_name,
            completed: 0,
            total: None,
        });
        Some(self.generation)
    }

    fn matches(&self, generation: u64, key: &FolderKey) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.generation == generation && pending.key == *key)
    }

    fn cancel(&mut self, generation: u64, key: &FolderKey) -> bool {
        if !self.matches(generation, key) {
            return false;
        }
        self.pending = None;
        true
    }

    fn start_progress(
        &mut self,
        generation: u64,
        key: &FolderKey,
        folder_name: String,
        total: usize,
    ) -> bool {
        let Some(pending) = self
            .pending
            .as_mut()
            .filter(|pending| pending.generation == generation && pending.key == *key)
        else {
            return false;
        };
        pending.folder_name = folder_name;
        pending.total = Some(total);
        true
    }

    fn advance(&mut self, generation: u64, key: &FolderKey, completed: usize) -> bool {
        let Some(pending) = self
            .pending
            .as_mut()
            .filter(|pending| pending.generation == generation && pending.key == *key)
        else {
            return false;
        };
        pending.completed = completed;
        true
    }

    fn finish(&mut self, generation: u64, key: &FolderKey, feedback: Feedback) -> bool {
        if !self.cancel(generation, key) {
            return false;
        }
        self.feedback.insert(key.clone(), feedback);
        true
    }
}

#[derive(Clone, Debug)]
struct SnapshotNote {
    title: String,
    version: std::result::Result<PathBuf, String>,
}

#[derive(Clone, Debug)]
struct Snapshot {
    root: PathBuf,
    folder_name: String,
    notes: Vec<SnapshotNote>,
}

fn safe_title(value: &str) -> String {
    let value = value.replace(['\r', '\n'], " ");
    let value = value.trim();
    if value.is_empty() {
        "未命名笔记".into()
    } else {
        value.into()
    }
}

fn snapshot(key: &FolderKey) -> Result<Snapshot> {
    let root = key.root.canonicalize().context("课程库暂时无法访问")?;
    let library = organize::Library::load(&root)?;
    let folder_name = library
        .folders
        .get(&key.folder_id)
        .cloned()
        .context("文件夹已不存在")?;
    let aliases = organize::title_aliases(&root).unwrap_or_default();
    let notes = library
        .courses
        .iter()
        .filter(|(_, folder)| **folder == key.folder_id)
        .map(|(relative, _)| {
            let fallback = relative
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let alias = aliases.get(relative).cloned();
            let valid = !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)));
            if !valid {
                return SnapshotNote {
                    title: safe_title(alias.as_deref().unwrap_or(&fallback)),
                    version: Err("笔记位置无效".into()),
                };
            }
            match notes::current_course(&root.join(relative)) {
                Ok(course) => SnapshotNote {
                    title: safe_title(alias.as_deref().unwrap_or(&course.title)),
                    version: Ok(course.dir),
                },
                Err(_) => SnapshotNote {
                    title: safe_title(alias.as_deref().unwrap_or(&fallback)),
                    version: Err("笔记正文无法读取".into()),
                },
            }
        })
        .collect();
    Ok(Snapshot {
        root,
        folder_name,
        notes,
    })
}

struct NameAllocator {
    used: BTreeSet<String>,
    case_sensitive: bool,
}

impl NameAllocator {
    fn new(case_sensitive: bool) -> Self {
        let mut this = Self {
            used: BTreeSet::new(),
            case_sensitive,
        };
        this.reserve("assets");
        this.reserve("_导出未完成");
        this
    }

    fn key(&self, name: &str) -> String {
        if self.case_sensitive {
            name.into()
        } else {
            name.to_lowercase()
        }
    }

    fn reserve(&mut self, name: &str) {
        self.used.insert(self.key(name));
    }

    fn allocate(&mut self, title: &str) -> String {
        let stem = course2md::portable::export_component(title, "未命名笔记");
        for index in 1.. {
            let candidate = if index == 1 {
                stem.clone()
            } else {
                format!("{stem} ({index})")
            };
            if self.used.insert(self.key(&candidate)) {
                return format!("{candidate}.md");
            }
        }
        unreachable!()
    }
}

fn case_sensitive(directory: &Path) -> Result<bool> {
    let lower = directory.join(".course2md-case-probe-a");
    let upper = directory.join(".course2md-case-probe-A");
    std::fs::write(&lower, b"")?;
    let sensitive = !upper.exists();
    std::fs::remove_file(lower)?;
    Ok(sensitive)
}

fn available_directory(parent: &Path, name: &str) -> PathBuf {
    let stem = course2md::portable::export_component(name, "未命名文件夹");
    for index in 1.. {
        let candidate = if index == 1 {
            stem.clone()
        } else {
            format!("{stem} ({index})")
        };
        let path = parent.join(candidate);
        if !path.exists() {
            return path;
        }
    }
    unreachable!()
}

fn incomplete_report(report: &Report) -> String {
    let mut text = format!(
        "# 导出未完成\n\n已导出 {}/{} 篇笔记。\n",
        report.succeeded(),
        report.total
    );
    for failure in &report.failures {
        text.push_str(&format!("\n- 《{}》：{}\n", failure.title, failure.reason));
    }
    text
}

fn export_snapshot(
    parent: &Path,
    snapshot: &Snapshot,
    mut progress: impl FnMut(usize),
) -> Result<Report> {
    let parent = parent.canonicalize().context("所选位置暂时无法访问")?;
    anyhow::ensure!(
        !parent.starts_with(&snapshot.root),
        "请选择课程库以外的位置"
    );
    let destination = available_directory(&parent, &snapshot.folder_name);
    let staging = tempfile::Builder::new()
        .prefix(".course2md-export-")
        .tempdir_in(&parent)?;
    let mut names = NameAllocator::new(case_sensitive(staging.path())?);
    let filenames = snapshot
        .notes
        .iter()
        .map(|note| names.allocate(&note.title))
        .collect::<Vec<_>>();
    let mut failures = Vec::new();
    for (index, (note, filename)) in snapshot.notes.iter().zip(filenames).enumerate() {
        let result = note
            .version
            .as_ref()
            .map_err(|reason| reason.clone())
            .and_then(|version| {
                course2md::portable::export_markdown(version, &staging.path().join(filename))
                    .map(|_| ())
                    .map_err(|_| "笔记正文或图片暂时无法读取".into())
            });
        if let Err(reason) = result {
            failures.push(Failure {
                title: note.title.clone(),
                reason,
            });
        }
        progress(index + 1);
    }
    let report = Report {
        output: None,
        total: snapshot.notes.len(),
        failures,
    };
    if report.succeeded() == 0 {
        return Ok(report);
    }
    if !report.failures.is_empty() {
        std::fs::write(
            staging.path().join("_导出未完成.md"),
            incomplete_report(&report),
        )?;
    }
    course2md::artifact::sync_dir(staging.path())?;
    std::fs::rename(staging.path(), &destination)?;
    let _ = staging.keep();
    course2md::artifact::sync_dir(&parent)?;
    Ok(Report {
        output: Some(destination),
        ..report
    })
}

enum WorkerEvent {
    Started {
        folder_name: String,
        total: usize,
    },
    Advanced(usize),
    Finished {
        folder_name: String,
        result: std::result::Result<Report, String>,
    },
}

fn worker(
    parent: PathBuf,
    key: FolderKey,
    initial_name: String,
    tx: smol::channel::Sender<WorkerEvent>,
) {
    let snapshot = match snapshot(&key) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            let _ = tx.send_blocking(WorkerEvent::Finished {
                folder_name: initial_name,
                result: Err("文件夹内容暂时无法读取，请刷新课程库后重试。".into()),
            });
            return;
        }
    };
    let _ = tx.send_blocking(WorkerEvent::Started {
        folder_name: snapshot.folder_name.clone(),
        total: snapshot.notes.len(),
    });
    let folder_name = snapshot.folder_name.clone();
    if snapshot.notes.is_empty() {
        let _ = tx.send_blocking(WorkerEvent::Finished {
            folder_name,
            result: Err("文件夹中没有可导出的笔记。".into()),
        });
        return;
    }
    let result = export_snapshot(&parent, &snapshot, |completed| {
        let _ = tx.send_blocking(WorkerEvent::Advanced(completed));
    })
    .map_err(|error| {
        if error.to_string().contains("课程库以外") {
            "请选择课程库以外的位置后重试。".into()
        } else {
            "无法在所选位置完成导出，请检查访问权限后重试。".into()
        }
    });
    let _ = tx.send_blocking(WorkerEvent::Finished {
        folder_name,
        result,
    });
}

impl Desktop {
    fn selected_folder_key(&self, folder_id: u64) -> FolderKey {
        let library_id = self
            .workspace
            .as_ref()
            .and_then(|workspace| {
                workspace
                    .state
                    .libraries
                    .iter()
                    .find(|library| library.root == self.library_root)
            })
            .map(|library| library.id.clone())
            .unwrap_or_else(|| self.library_root.display().to_string());
        FolderKey {
            library_id,
            root: self.library_root.clone(),
            folder_id,
        }
    }

    fn folder_note_count(&self, folder_id: u64) -> usize {
        self.library_indexes
            .get(&self.library_root)
            .unwrap_or(&self.library)
            .courses
            .values()
            .filter(|folder| **folder == folder_id)
            .count()
    }

    fn selected_library_online(&self) -> bool {
        self.cached_library_access()
            .is_some_and(|access| access.available.contains(&self.library_root))
    }

    pub(super) fn folder_export_button(&self, folder_id: u64, cx: &mut Context<Self>) -> Button {
        let key = self.selected_folder_key(folder_id);
        let count = self.folder_note_count(folder_id);
        let pending = self.folder_export.pending.as_ref();
        let current = pending.filter(|pending| pending.key == key);
        let label = current.map_or_else(
            || "导出文件夹".to_owned(),
            |pending| match pending.total {
                Some(total) => format!("正在导出 {}/{total}", pending.completed),
                None => "正在选择位置…".into(),
            },
        );
        let offline = !self.selected_library_online();
        let tooltip = if offline {
            "课程库暂时无法访问"
        } else if count == 0 {
            "文件夹中没有可导出的笔记"
        } else if pending.is_some() && current.is_none() {
            "当前正在导出另一个文件夹"
        } else {
            "导出文件夹中的全部笔记"
        };
        outline_pill("library-export-folder")
            .icon(icons::download())
            .label(label)
            .min_w(rems(15.))
            .loading(current.is_some())
            .tooltip(tooltip)
            .disabled(offline || count == 0 || pending.is_some())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.begin_folder_export(folder_id, window, cx)
            }))
    }

    fn begin_folder_export(&mut self, folder_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_note_count(folder_id) == 0 || !self.selected_library_online() {
            return;
        }
        let key = self.selected_folder_key(folder_id);
        let folder_name = self
            .library_indexes
            .get(&self.library_root)
            .unwrap_or(&self.library)
            .folders
            .get(&folder_id)
            .cloned()
            .unwrap_or_else(|| "文件夹".into());
        let Some(generation) = self.folder_export.begin(key.clone(), folder_name.clone()) else {
            return;
        };
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择导出位置".into()),
        });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let parent = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.folder_export.cancel(generation, &key) {
                            cx.notify();
                        }
                    });
                    return;
                }
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        let folder_name = this
                            .folder_export
                            .pending
                            .as_ref()
                            .map(|pending| pending.folder_name.clone())
                            .unwrap_or_else(|| "文件夹".into());
                        if this.folder_export.finish(
                            generation,
                            &key,
                            Feedback {
                                folder_name,
                                result: Err("无法打开目录选择器，请重试。".into()),
                            },
                        ) {
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let Some(parent) = parent else {
                return;
            };
            let (tx, rx) = smol::channel::unbounded();
            let worker_key = key.clone();
            std::thread::spawn(move || worker(parent, worker_key, folder_name, tx));
            while let Ok(event) = rx.recv().await {
                let finished = matches!(event, WorkerEvent::Finished { .. });
                let _ = this.update(cx, |this, cx| {
                    let changed = match event {
                        WorkerEvent::Started { folder_name, total } => this
                            .folder_export
                            .start_progress(generation, &key, folder_name, total),
                        WorkerEvent::Advanced(completed) => {
                            this.folder_export.advance(generation, &key, completed)
                        }
                        WorkerEvent::Finished {
                            folder_name,
                            result,
                        } => this.folder_export.finish(
                            generation,
                            &key,
                            Feedback {
                                folder_name,
                                result,
                            },
                        ),
                    };
                    if changed {
                        cx.notify();
                    }
                });
                if finished {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn folder_export_feedback(&self, cx: &mut Context<Self>) -> Option<Div> {
        let folder_id = self.folder_filter.filter(|id| *id != 0)?;
        let key = self.selected_folder_key(folder_id);
        let feedback = self.folder_export.feedback.get(&key)?.clone();
        let (icon, color_role, label, report) = match &feedback.result {
            Ok(report) if report.output.is_some() && report.failures.is_empty() => (
                icons::check(),
                SUCCESS,
                format!("已导出 {} 篇笔记", report.succeeded()),
                Some(report.clone()),
            ),
            Ok(report) if report.output.is_some() => (
                icons::triangle_alert(),
                WARNING,
                if report.failures.len() == 1 {
                    format!(
                        "已导出 {}/{} 篇；《{}》未完成",
                        report.succeeded(),
                        report.total,
                        report.failures[0].title
                    )
                } else {
                    format!(
                        "已导出 {}/{} 篇；{} 篇未完成",
                        report.succeeded(),
                        report.total,
                        report.failures.len()
                    )
                },
                Some(report.clone()),
            ),
            Ok(report) => (
                icons::circle_x(),
                DANGER,
                report
                    .failures
                    .first()
                    .map(|failure| {
                        format!("未能导出“{}”：{}", feedback.folder_name, failure.reason)
                    })
                    .unwrap_or_else(|| format!("未能导出“{}”", feedback.folder_name)),
                Some(report.clone()),
            ),
            Err(error) => (
                icons::circle_x(),
                DANGER,
                format!("未能导出“{}”：{error}", feedback.folder_name),
                None,
            ),
        };
        let id = format!("folder-export:{}:{}", key.library_id, key.folder_id);
        let dismiss_key = key.clone();
        let dismiss = quiet(SharedString::from(format!("{id}:dismiss")))
            .icon(icons::close())
            .w(CONTROL_HEIGHT)
            .px_0()
            .tooltip("关闭提示")
            .accessibility_label("关闭文件夹导出提示")
            .on_click(cx.listener(move |this, _, _, cx| {
                this.folder_export.feedback.remove(&dismiss_key);
                this.folder_export.details_open.remove(&dismiss_key);
                cx.notify();
            }));
        let mut actions = h_flex().min_w_0().flex_wrap().gap_2().items_center();
        if let Some(path) = report.as_ref().and_then(|report| report.output.clone()) {
            actions = actions.child(
                quiet(SharedString::from(format!("{id}:open")))
                    .icon(icons::folder_open())
                    .label("打开文件夹")
                    .on_click(move |_, _, cx| cx.open_with_system(&path)),
            );
        }
        let retry = feedback.result.as_ref().is_err()
            || report
                .as_ref()
                .is_some_and(|report| !report.failures.is_empty());
        if retry {
            actions = actions.child(
                outline_pill(SharedString::from(format!("{id}:retry")))
                    .icon(icons::refresh())
                    .label("重新导出")
                    .disabled(self.folder_export.pending.is_some())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_folder_export(folder_id, window, cx)
                    })),
            );
        }
        if report
            .as_ref()
            .is_some_and(|report| !report.failures.is_empty())
        {
            let details_key = key.clone();
            let open = self.folder_export.details_open.contains(&key);
            actions = actions.child(
                quiet(SharedString::from(format!("{id}:details")))
                    .label(if open { "收起详情" } else { "查看详情" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.folder_export.details_open.remove(&details_key) {
                            this.folder_export.details_open.insert(details_key.clone());
                        }
                        cx.notify();
                    })),
            );
        }
        actions = actions.child(dismiss);
        let mut view = v_flex().w_full().min_w_0().gap_1().child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .gap_2()
                .items_center()
                .child(
                    icon.size(rems(18. / 14.))
                        .flex_shrink_0()
                        .text_color(color(color_role)),
                )
                .child(
                    accessible_text(SharedString::from(id), label)
                        .flex_1()
                        .min_w(rems(12.))
                        .whitespace_normal()
                        .text_size(TEXT_AUX)
                        .font_weight(FontWeight::NORMAL)
                        .text_color(color(GRAY)),
                )
                .child(actions),
        );
        if self.folder_export.details_open.contains(&key)
            && let Some(report) = report
        {
            view =
                view.child(
                    v_flex()
                        .ml_6()
                        .gap_1()
                        .children(report.failures.into_iter().enumerate().map(
                            |(index, failure)| {
                                accessible_text(
                                    SharedString::from(format!(
                                        "folder-export-failure:{}:{index}",
                                        key.folder_id
                                    )),
                                    format!("《{}》：{}", failure.title, failure.reason),
                                )
                                .text_size(TEXT_AUX)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(color(MUTED))
                            },
                        )),
                );
        }
        Some(view)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Feedback, FolderKey, NameAllocator, Snapshot, SnapshotNote, State, export_snapshot,
    };
    use std::path::PathBuf;

    fn published_note(root: &std::path::Path, id: &str) -> PathBuf {
        use course2md::{
            artifact::{self, Outcomes, Target},
            fetch::VideoMeta,
            timeline::{Section, TranscriptEvent},
        };
        let work = root.join(format!("work-{id}"));
        std::fs::create_dir_all(work.join("frames")).unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([20, 80, 180]))
            .save(work.join("frames/a.png"))
            .unwrap();
        let target = Target {
            task_id: format!("task-{id}"),
            course_id: format!("course-{id}"),
            source_id: format!("source-{id}"),
            version_id: "v1".into(),
            course_dir: root.join(format!("note-{id}")),
        };
        artifact::publish_blocking(
            &target,
            &work,
            &VideoMeta {
                title: id.into(),
                uploader: String::new(),
                duration: 1.,
                webpage_url: String::new(),
                extractor: "web".into(),
                id: id.into(),
            },
            &[Section {
                t: 0.,
                end: 1.,
                image: "frames/a.png".into(),
                speech: vec![TranscriptEvent {
                    start: 0.,
                    end: 1.,
                    text: format!("body {id}"),
                    raw: None,
                    translation: None,
                }],
            }],
            None,
            &[],
            Outcomes::default(),
        )
        .unwrap();
        target.version_dir()
    }

    #[test]
    fn names_cover_reserved_case_conflicts_and_completion_generations() {
        let mut names = NameAllocator::new(false);
        assert_eq!(names.allocate("Lesson"), "Lesson.md");
        assert_eq!(names.allocate("lesson"), "lesson (2).md");
        assert_eq!(names.allocate("assets"), "assets (2).md");
        assert_eq!(names.allocate("CON"), "_CON.md");

        let key = FolderKey {
            library_id: "library".into(),
            root: PathBuf::from("library"),
            folder_id: 7,
        };
        let other = FolderKey {
            folder_id: 8,
            ..key.clone()
        };
        let mut state = State::default();
        let old = state.begin(key.clone(), "One".into()).unwrap();
        assert!(state.cancel(old, &key));
        let current = state.begin(other.clone(), "Two".into()).unwrap();
        assert!(!state.finish(
            old,
            &key,
            Feedback {
                folder_name: "One".into(),
                result: Err("late".into()),
            }
        ));
        assert!(state.matches(current, &other));
    }

    #[test]
    fn batch_publish_keeps_existing_directory_and_reports_partial_failure() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        let output = temp.path().join("output");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(output.join("课程")).unwrap();
        std::fs::write(output.join("课程/keep.txt"), "keep").unwrap();
        let version = published_note(&library, "one");
        std::fs::write(version.join("course.md"), "damaged cached markdown").unwrap();
        let snapshot = Snapshot {
            root: library.canonicalize().unwrap(),
            folder_name: "课程".into(),
            notes: vec![
                SnapshotNote {
                    title: "Lesson".into(),
                    version: Ok(version.clone()),
                },
                SnapshotNote {
                    title: "lesson".into(),
                    version: Ok(version),
                },
                SnapshotNote {
                    title: "坏笔记".into(),
                    version: Err("笔记正文无法读取".into()),
                },
            ],
        };
        let mut progress = Vec::new();
        let report = export_snapshot(&output, &snapshot, |done| progress.push(done)).unwrap();
        let published = report.output.clone().unwrap();
        assert_eq!(published.file_name().unwrap(), "课程 (2)");
        assert_eq!(
            std::fs::read_to_string(output.join("课程/keep.txt")).unwrap(),
            "keep"
        );
        assert_eq!(progress, [1, 2, 3]);
        assert_eq!(report.succeeded(), 2);
        assert_eq!(
            std::fs::read_dir(published.join("assets")).unwrap().count(),
            1
        );
        assert!(published.join("_导出未完成.md").is_file());
        assert_eq!(
            std::fs::read_dir(&published)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "md"))
                .count(),
            3
        );

        let failed = Snapshot {
            notes: vec![SnapshotNote {
                title: "坏笔记".into(),
                version: Err("笔记正文无法读取".into()),
            }],
            ..snapshot
        };
        let failed = export_snapshot(&output, &failed, |_| {}).unwrap();
        assert!(failed.output.is_none());
        assert!(
            !std::fs::read_dir(&output)
                .unwrap()
                .any(|entry| entry.ok().is_some_and(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".course2md-export-")))
        );
    }
}
