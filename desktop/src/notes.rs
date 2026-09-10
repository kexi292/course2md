//! Read published versions and isolate damaged library entries.
use crate::backend::Completed;
use anyhow::{Context, Result};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::SystemTime,
};
#[derive(Clone)]
pub struct Course {
    pub dir: PathBuf,
    pub title: String,
    pub modified: SystemTime,
    pub slides: usize,
    pub segments: usize,
    pub thumbnail: Option<PathBuf>,
    pub manifest: Option<course2md::artifact::Manifest>,
    pub warning: Option<String>,
}
impl Course {
    pub fn from_completed(done: &Completed) -> Self {
        Self {
            dir: done.out_dir.clone(),
            title: done.title.clone(),
            modified: SystemTime::now(),
            slides: done.slides,
            segments: done.segments,
            thumbnail: None,
            manifest: course2md::artifact::read_manifest(&done.out_dir.join("manifest.json")).ok(),
            warning: None,
        }
    }
    pub fn description(&self) -> String {
        let content = if self.slides > 0 {
            format!("含 {} 张截图", self.slides)
        } else {
            "文字笔记".to_owned()
        };
        if self.manifest.as_ref().is_some_and(has_incomplete_content) {
            format!("{content} · 部分内容待补全")
        } else {
            content
        }
    }
    pub fn storage_dir(&self) -> PathBuf {
        if self.manifest.is_some() {
            self.dir
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or(&self.dir)
                .to_path_buf()
        } else {
            self.dir.clone()
        }
    }
}
/// Optional file exports describe a task's output, not the completeness of its note.
/// Published manifests stay immutable when a later task successfully repairs exports.
pub(crate) fn has_incomplete_content(manifest: &course2md::artifact::Manifest) -> bool {
    !processing_issues(manifest).is_empty()
}
pub struct LibraryScan {
    pub courses: Vec<Course>,
    pub issues: Vec<String>,
}

fn version_course(dir: &Path) -> Result<Course> {
    let manifest = course2md::artifact::read_manifest(&dir.join("manifest.json"))?;
    let document: course2md::artifact::Document = serde_json::from_slice(&std::fs::read(
        course2md::artifact::safe_asset_path(dir, &manifest.document)?,
    )?)?;
    anyhow::ensure!(
        document.schema == 1 && course2md::artifact::has_readable_body(&document.sections),
        "笔记正文无法读取"
    );
    let warning = course2md::artifact::validate_version(dir, &manifest)
        .err()
        .map(|_| "部分笔记文件有变化或缺失，已显示当前可读内容。原文件已保留。".into());
    let thumbnail = manifest
        .frames
        .iter()
        .find_map(|f| course2md::artifact::safe_asset_path(dir, &f.image).ok());
    Ok(Course {
        dir: dir.to_path_buf(),
        title: manifest.title.clone(),
        modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(manifest.created_at_ms),
        slides: manifest
            .frames
            .iter()
            .filter(|f| course2md::artifact::safe_asset_path(dir, &f.image).is_ok())
            .count(),
        segments: document.sections.iter().map(|s| s.speech.len()).sum(),
        thumbnail,
        manifest: Some(manifest),
        warning,
    })
}

fn current_course(dir: &Path) -> Result<Course> {
    let load = || -> Result<Course> {
        let current: course2md::artifact::CurrentVersion =
            serde_json::from_slice(&std::fs::read(dir.join("current.json"))?)?;
        anyhow::ensure!(current.schema == 1, "笔记版本暂不受支持");
        let manifest_path = course2md::artifact::safe_asset_path(dir, &current.manifest)?;
        let course = version_course(manifest_path.parent().context("版本位置无效")?)?;
        let manifest = course.manifest.as_ref().unwrap();
        anyhow::ensure!(
            manifest.course_id == current.course_id && manifest.version_id == current.version_id,
            "笔记版本记录不一致"
        );
        Ok(course)
    };
    match load() {
        Ok(course) => Ok(course),
        Err(error) => {
            let mut previous = std::fs::read_dir(dir.join("versions"))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| version_course(&e.path()).ok())
                .filter(|c| c.warning.is_none())
                .collect::<Vec<_>>();
            previous.sort_by_key(|c| c.manifest.as_ref().map(|m| m.revision));
            if let Some(mut course) = previous.pop() {
                course.warning = Some(
                    "当前版本暂时无法读取；正在显示此前保存的完好版本，所有原文件已保留。".into(),
                );
                Ok(course)
            } else {
                Err(error)
            }
        }
    }
}

pub fn scan_library(root: &Path) -> Result<LibraryScan> {
    let mut scan = LibraryScan {
        courses: Vec::new(),
        issues: Vec::new(),
    };
    if !root.exists() {
        return Ok(scan);
    }
    let mut queue = VecDeque::from([(root.to_path_buf(), 0)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if dir.join("current.json").is_file() {
            match current_course(&dir) {
                Ok(course) => scan.courses.push(course),
                Err(error) => scan.issues.push(format!("{}：{error:#}", dir.display())),
            }
            continue;
        }
        if depth < 8 {
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                if entry.file_name().to_string_lossy().starts_with('.')
                                    || entry.file_name() == "versions"
                                {
                                    continue;
                                }
                                match entry.file_type() {
                                    Ok(kind) if kind.is_dir() => {
                                        queue.push_back((entry.path(), depth + 1))
                                    }
                                    Ok(_) => {}
                                    Err(error) => scan
                                        .issues
                                        .push(format!("{}：{error}", entry.path().display())),
                                }
                            }
                            Err(error) => scan.issues.push(format!("{}：{error}", dir.display())),
                        }
                    }
                }
                Err(error) => scan.issues.push(format!("{}：{error}", dir.display())),
            }
        }
    }
    scan.courses.sort_by_key(|c| std::cmp::Reverse(c.modified));
    Ok(scan)
}

pub fn default_output() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    PathBuf::from(home.unwrap_or_else(|| ".".into())).join("Documents/course2md")
}

#[derive(Clone)]
pub enum PreviewBlock {
    Image(PathBuf),
    Heading {
        text: String,
        anchor: String,
        seconds: Option<f64>,
    },
    Paragraph {
        text: String,
        anchor: String,
    },
}
#[derive(Clone)]
pub enum ProcessingStage {
    Transcript,
    Screenshots,
    Proofreading,
    Summary,
}
impl ProcessingStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Transcript => "部分文字",
            Self::Screenshots => "截图",
            Self::Proofreading => "AI 校对",
            Self::Summary => "摘要",
        }
    }
}
#[derive(Clone)]
pub struct ProcessingIssue {
    pub stage: ProcessingStage,
    pub outcome: course2md::artifact::Outcome,
}
fn processing_issues(manifest: &course2md::artifact::Manifest) -> Vec<ProcessingIssue> {
    [
        (ProcessingStage::Transcript, &manifest.outcomes.transcript),
        (ProcessingStage::Screenshots, &manifest.outcomes.screenshots),
        (
            ProcessingStage::Proofreading,
            &manifest.outcomes.proofreading,
        ),
        (ProcessingStage::Summary, &manifest.outcomes.summary),
    ]
    .into_iter()
    .filter(|(_, outcome)| {
        matches!(
            outcome.status,
            course2md::artifact::Status::Failed | course2md::artifact::Status::Partial
        )
    })
    .map(|(stage, outcome)| ProcessingIssue {
        stage,
        outcome: outcome.clone(),
    })
    .collect()
}
#[derive(Clone)]
pub struct Preview {
    pub course: Course,
    pub blocks: Vec<PreviewBlock>,
    pub frames: Vec<PathBuf>,
    pub has_markdown: bool,
    pub outputs: Vec<String>,
    pub plain_text: String,
    pub markdown_text: String,
    pub document: Option<course2md::artifact::Document>,
    pub metadata: Vec<(String, String)>,
    pub issues: Vec<String>,
    /// Incomplete processing is recoverable through its task, never by re-reading files.
    pub processing_issues: Vec<ProcessingIssue>,
}

pub fn read_preview(mut course: Course) -> Result<Preview> {
    if course.dir.join("manifest.json").is_file() {
        let manifest = course2md::artifact::read_manifest(&course.dir.join("manifest.json"))?;
        let path = course2md::artifact::safe_asset_path(&course.dir, &manifest.document)?;
        let document: course2md::artifact::Document =
            serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(
            course2md::artifact::has_readable_body(&document.sections),
            "笔记正文无法读取，原文件已保留"
        );
        let markdown = course2md::artifact::safe_asset_path(&course.dir, &manifest.markdown)
            .and_then(|path| Ok(std::fs::read_to_string(path)?))
            .unwrap_or_else(|_| {
                course2md::render::render_markdown(&document.meta, &document.sections)
            });
        let mut plain_text = format!("{}\n\n", manifest.title);
        let mut blocks = Vec::new();
        if let Some(summary) = &document.summary {
            blocks.push(PreviewBlock::Heading {
                text: "摘要".into(),
                anchor: "summary".into(),
                seconds: None,
            });
            blocks.push(PreviewBlock::Paragraph {
                text: summary.tldr.clone(),
                anchor: "summary-tldr".into(),
            });
            plain_text.push_str(&summary.tldr);
            plain_text.push_str("\n\n");
            for (i, point) in summary.key_points.iter().enumerate() {
                blocks.push(PreviewBlock::Paragraph {
                    text: format!("• {point}"),
                    anchor: format!("key-point-{i}"),
                });
                plain_text.push_str(point);
                plain_text.push('\n');
            }
        }
        let mut issues = course.warning.clone().into_iter().collect::<Vec<_>>();
        let processing_issues = processing_issues(&manifest);
        let mut frames = Vec::new();
        for frame in &manifest.frames {
            match course2md::artifact::safe_asset_path(&course.dir, &frame.image) {
                Ok(path) => frames.push(path),
                Err(_) => issues.push(format!(
                    "{} 的截图暂时无法读取",
                    course2md::render::fmt_ts(frame.t)
                )),
            }
        }
        for (section_index, section) in document.sections.iter().enumerate() {
            blocks.push(PreviewBlock::Heading {
                text: course2md::render::fmt_ts(section.t),
                anchor: format!("section-{section_index}"),
                seconds: Some(section.t),
            });
            if !section.image.is_empty()
                && manifest.frames.iter().any(|f| f.image == section.image)
                && let Ok(path) = course2md::artifact::safe_asset_path(&course.dir, &section.image)
            {
                blocks.push(PreviewBlock::Image(path));
            }
            for (paragraph_index, paragraph) in section.speech.iter().enumerate() {
                if !paragraph.text.trim().is_empty() {
                    let anchor = format!("paragraph-{section_index}-{paragraph_index}");
                    blocks.push(PreviewBlock::Paragraph {
                        text: paragraph.text.clone(),
                        anchor,
                    });
                    plain_text.push_str(&paragraph.text);
                    plain_text.push_str("\n\n");
                }
            }
        }
        let mut metadata = Vec::new();
        if !document.meta.uploader.is_empty() {
            metadata.push(("作者".into(), document.meta.uploader.clone()));
        }
        if document.meta.duration > 0. {
            metadata.push((
                "时长".into(),
                course2md::render::fmt_ts(document.meta.duration),
            ));
        }
        if !document.meta.webpage_url.is_empty() {
            metadata.push(("来源".into(), document.meta.webpage_url.clone()));
        }
        course.title = manifest.title.clone();
        course.slides = manifest.frames.len();
        course.segments = document.sections.iter().map(|s| s.speech.len()).sum();
        course.manifest = Some(manifest.clone());
        let markdown_text = without_image_references(&markdown);
        return Ok(Preview {
            course,
            blocks,
            frames,
            has_markdown: true,
            outputs: manifest.outputs,
            plain_text,
            markdown_text,
            document: Some(document),
            metadata,
            issues,
            processing_issues,
        });
    }
    anyhow::bail!("笔记正文无法读取，原文件已保留")
}

/// Copy-friendly markdown: strips image spans and their reference definitions while
/// leaving authored prose, inline code, lists, tables and links untouched.
pub fn without_image_references(markdown: &str) -> String {
    use pulldown_cmark::{Event, Tag};
    let mut ranges = Vec::new();
    let mut definitions = Vec::new();
    for (event, range) in
        pulldown_cmark::Parser::new_ext(markdown, pulldown_cmark::Options::all())
            .into_offset_iter()
    {
        if let Event::Start(Tag::Image { id, .. }) = event {
            ranges.push(range);
            if !id.is_empty() {
                definitions.push(id.into_string());
            }
        }
    }
    let mut result = markdown.to_owned();
    for range in ranges.into_iter().rev() {
        result.replace_range(range, "");
    }
    result
        .lines()
        .filter(|line| {
            !definitions
                .iter()
                .any(|id| line.trim_start().starts_with(&format!("[{id}]:")))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A published current-format note: one frame, one paragraph, no file exports.
    fn fixture_note(root: &Path) -> Course {
        use course2md::{
            artifact,
            fetch::VideoMeta,
            timeline::{Section, TranscriptEvent},
        };
        let work = root.join("work");
        std::fs::create_dir_all(work.join("frames")).unwrap();
        image::RgbImage::new(4, 3)
            .save(work.join("frames/frame-0.png"))
            .unwrap();
        let sections = vec![Section {
            t: 10.,
            end: 20.,
            image: "frames/frame-0.png".into(),
            speech: vec![TranscriptEvent {
                start: 10.,
                end: 20.,
                text: "真实正文".into(),
                raw: None,
            }],
        }];
        let target = artifact::Target {
            task_id: "task-1".into(),
            course_id: "course-1".into(),
            source_id: "source-1".into(),
            version_id: "v1".into(),
            course_dir: root.join("note"),
        };
        let meta = VideoMeta {
            title: "测试课程".into(),
            uploader: String::new(),
            duration: 0.,
            webpage_url: String::new(),
            extractor: "local".into(),
            id: "source".into(),
        };
        let manifest = smol::block_on(artifact::publish(
            &target,
            &work,
            &meta,
            &sections,
            None,
            &[],
            Default::default(),
        ))
        .unwrap();
        Course {
            dir: target.version_dir(),
            title: meta.title,
            modified: SystemTime::now(),
            slides: 1,
            segments: 1,
            thumbnail: None,
            manifest: Some(manifest),
            warning: None,
        }
    }

    #[test]
    fn optional_export_failure_does_not_describe_readable_content_as_incomplete() {
        let root = tempfile::tempdir().unwrap();
        let note = fixture_note(root.path());
        let mut course = scan_library(root.path()).unwrap().courses.remove(0);
        assert_eq!(course.dir, note.dir);
        let manifest = course.manifest.as_mut().unwrap();
        manifest.partial = true;
        manifest.outcomes.transcript = course2md::artifact::Outcome::succeeded();
        manifest.outcomes.screenshots = course2md::artifact::Outcome::succeeded();
        manifest.outcomes.proofreading = course2md::artifact::Outcome::succeeded();
        manifest.outcomes.summary = course2md::artifact::Outcome::succeeded();
        manifest.outcomes.exports.insert(
            "html".into(),
            course2md::artifact::Outcome::failed("destination unavailable"),
        );
        assert!(!has_incomplete_content(manifest));
        assert!(!course.description().contains("待补全"));
        course.manifest.as_mut().unwrap().outcomes.summary =
            course2md::artifact::Outcome::failed("service unavailable");
        assert!(course.description().contains("待补全"));
    }

    #[test]
    fn library_discovers_published_notes_and_ignores_task_leftovers() {
        let root = tempfile::tempdir().unwrap();
        let note = fixture_note(root.path());
        let leftover = root.path().join("failed");
        std::fs::create_dir_all(&leftover).unwrap();
        std::fs::write(leftover.join("run.json"), r#"{"success":false}"#).unwrap();
        let first = scan_library(root.path()).unwrap();
        assert_eq!(first.courses.len(), 1);
        assert!(first.issues.is_empty());
        assert_eq!(
            first.courses[0].storage_dir().canonicalize().unwrap(),
            root.path().join("note").canonicalize().unwrap()
        );
        let preview = read_preview(first.courses[0].clone()).unwrap();
        assert!(preview.plain_text.contains("真实正文"));
        assert!(preview.document.is_some());
        assert!(
            preview
                .blocks
                .iter()
                .any(|b| matches!(b,PreviewBlock::Heading{seconds:Some(t),..} if *t==10.))
        );
        let second = scan_library(root.path()).unwrap();
        assert_eq!(second.courses[0].dir, note.dir);
    }

    #[test]
    fn copy_markdown_removes_real_images_but_keeps_inline_code_and_surrounding_words() {
        let value = "before ![inline](frames/a.png) after\n\n![ref][pic]\n\n[pic]: frames/a.png\n\n`![example](literal)`";
        let copied = without_image_references(value);
        assert!(copied.contains("before  after"));
        assert!(!copied.contains("frames/a.png"));
        assert!(copied.contains("`![example](literal)`"));
    }
}
