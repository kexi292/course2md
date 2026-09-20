//! Exports that retain their contents when moved away from the course library.

use crate::{
    artifact::{self, Document},
    config::OutputFormat,
    execution, render,
};
use anyhow::{Context, Result};
use base64::Engine as _;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

pub fn file_name(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Md => "course.zip",
        OutputFormat::Html => "course.html",
        OutputFormat::Json => "structured.json",
    }
}

/// Exact destination, with no overwrite. UI may explicitly confirm replacing a separate
/// exported file, but must never mutate the immutable note version itself.
pub fn export(version_dir: &Path, format: OutputFormat, destination: &Path) -> Result<PathBuf> {
    let document = read_document(version_dir)?;
    write_document(version_dir, &document, format, destination)
}

/// Write one published note as Markdown beside a shared `assets` directory.
/// Existing assets with identical content are reused; no user file is overwritten.
pub fn export_markdown(version_dir: &Path, destination: &Path) -> Result<PathBuf> {
    let document = read_document(version_dir)?;
    write_markdown_document(version_dir, &document, destination)
}

fn read_document(version_dir: &Path) -> Result<Document> {
    let manifest = artifact::read_manifest(&version_dir.join("manifest.json"))?;
    artifact::validate_version(version_dir, &manifest)?;
    serde_json::from_slice(&std::fs::read(artifact::safe_asset_path(
        version_dir,
        &manifest.document,
    )?)?)
    .context("笔记正文无法读取")
}

struct ImageAsset {
    id: String,
    path: String,
    mime: &'static str,
    bytes: Vec<u8>,
    sha256: String,
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

fn images(
    root: &Path,
    document: &Document,
    directory: &str,
) -> Result<BTreeMap<String, ImageAsset>> {
    let mut images = BTreeMap::new();
    for section in document.sections.iter().filter(|s| !s.image.is_empty()) {
        if images.contains_key(&section.image) {
            continue;
        }
        let bytes = std::fs::read(artifact::safe_asset_path(root, &section.image)?)?;
        let sha256 = execution::digest(&bytes);
        let id = format!("image-{}", &sha256[..24]);
        let mime =
            image_mime(&bytes).context("截图不是可识别的图片，原文件已保留 / Screenshot is not a recognized image; original file kept")?;
        let ext = match mime {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg",
        };
        images.insert(
            section.image.clone(),
            ImageAsset {
                path: format!("{directory}/{id}.{ext}"),
                id,
                mime,
                bytes,
                sha256,
            },
        );
    }
    Ok(images)
}

pub(crate) fn write_document(
    root: &Path,
    document: &Document,
    format: OutputFormat,
    destination: &Path,
) -> Result<PathBuf> {
    anyhow::ensure!(
        !destination.exists(),
        "导出位置已有文件，未覆盖：{} / Export destination already exists",
        destination.display()
    );
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    let images = images(root, document, "images")?;
    let mut portable = document.clone();
    let local_file = if portable.meta.extractor == "local" {
        let name = Path::new(&portable.meta.webpage_url)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned());
        portable.meta.webpage_url.clear();
        name
    } else {
        None
    };
    for section in &mut portable.sections {
        if let Some(image) = images.get(&section.image) {
            section.image = image.path.clone();
        }
    }
    match format {
        OutputFormat::Md => {
            let mut text = render::render_markdown(&portable.meta, &portable.sections);
            if let Some(summary) = &portable.summary {
                text = crate::summarize::insert_into_md(&text, summary);
            }
            if let Some(name) = &local_file {
                text.push_str(&format!("\n源文件：{}\n", name.replace(['\r', '\n'], " ")));
            }
            let mut zip = zip::ZipWriter::new(file.as_file_mut());
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("course.md", options)?;
            zip.write_all(text.as_bytes())?;
            let mut written = std::collections::HashSet::new();
            for image in images.values() {
                if written.insert(&image.path) {
                    zip.start_file(&image.path, options)?;
                    zip.write_all(&image.bytes)?;
                }
            }
            zip.finish()?;
        }
        OutputFormat::Html => {
            let mut text = render::render_html(&portable.meta, &portable.sections);
            if let Some(summary) = &portable.summary {
                text = crate::summarize::insert_into_html(&text, summary);
            }
            for image in images.values() {
                text = text.replace(
                    &format!("src=\"{}\"", render::esc(&image.path)),
                    &format!(
                        "src=\"data:{};base64,{}\"",
                        image.mime,
                        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
                    ),
                );
            }
            file.write_all(text.as_bytes())?;
        }
        OutputFormat::Json => {
            let sections = document.sections.iter().map(|section| serde_json::json!({"t":section.t,"end":section.end,"image_id":images.get(&section.image).map(|image|&image.id),"speech":section.speech})).collect::<Vec<_>>();
            let image_data = images.values().map(|image| serde_json::json!({"id":image.id,"mime_type":image.mime,"sha256":image.sha256})).collect::<Vec<_>>();
            file.write_all(&serde_json::to_vec_pretty(&serde_json::json!({"schema":1,"title":document.meta.title,"author":document.meta.uploader,"duration":if document.meta.duration > 0. { Some(document.meta.duration) } else { None },"source":if let Some(name) = local_file {serde_json::json!({"kind":"local","file_name":name})} else if document.meta.webpage_url.is_empty() {serde_json::json!({"kind":"unknown"})}else{serde_json::json!({"kind":"online","url":document.meta.webpage_url})},"summary":document.summary,"sections":sections,"images":image_data}))?)?;
        }
    }
    file.as_file().sync_all()?;
    file.persist_noclobber(destination).with_context(|| {
        format!(
            "无法保存导出文件；原文件未覆盖 / Could not save export: {}",
            destination.display()
        )
    })?;
    artifact::sync_dir(parent)?;
    Ok(destination.to_path_buf())
}

pub(crate) fn write_markdown_document(
    root: &Path,
    document: &Document,
    destination: &Path,
) -> Result<PathBuf> {
    anyhow::ensure!(
        !destination.exists(),
        "导出位置已有文件，未覆盖：{} / Export destination already exists",
        destination.display()
    );
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let images = images(root, document, "assets")?;
    let mut portable = document.clone();
    let local_file = if portable.meta.extractor == "local" {
        let name = Path::new(&portable.meta.webpage_url)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        portable.meta.webpage_url.clear();
        name
    } else {
        None
    };
    for section in &mut portable.sections {
        if let Some(image) = images.get(&section.image) {
            section.image = image.path.clone();
        }
    }
    let mut text = render::render_markdown(&portable.meta, &portable.sections);
    if let Some(summary) = &portable.summary {
        text = crate::summarize::insert_into_md(&text, summary);
    }
    if let Some(name) = local_file {
        text.push_str(&format!("\n源文件：{}\n", name.replace(['\r', '\n'], " ")));
    }

    if !images.is_empty() {
        let assets = parent.join("assets");
        std::fs::create_dir_all(&assets)?;
        for image in images.values() {
            let target = parent.join(&image.path);
            if target.exists() {
                anyhow::ensure!(
                    std::fs::read(&target)? == image.bytes,
                    "共享图片摘要冲突，未覆盖已有文件"
                );
                continue;
            }
            let mut file = tempfile::NamedTempFile::new_in(&assets)?;
            file.write_all(&image.bytes)?;
            file.as_file().sync_all()?;
            file.persist_noclobber(&target)
                .with_context(|| "无法写入共享图片，未覆盖已有文件")?;
        }
        artifact::sync_dir(&assets)?;
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist_noclobber(destination)
        .with_context(|| "无法保存 Markdown，未覆盖已有文件")?;
    artifact::sync_dir(parent)?;
    Ok(destination.to_path_buf())
}

/// Export-only component handling. Storage identities intentionally keep using
/// the existing shared sanitizer without Windows device-name rewriting.
pub fn export_component(name: &str, fallback: &str) -> String {
    let name = crate::config::sanitize_filename_with_fallback(name, fallback);
    let device = name.split('.').next().unwrap_or_default();
    let reserved = matches!(
        device.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved { format!("_{name}") } else { name }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_exports_survive_without_the_original_library_and_never_overwrite() {
        use crate::{
            fetch::VideoMeta,
            timeline::{Section, TranscriptEvent},
        };
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir_all(source.join("frames")).unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([20, 80, 180]))
            .save(source.join("frames/a.png"))
            .unwrap();
        let document = Document {
            schema: 1,
            meta: VideoMeta {
                title: "title".into(),
                uploader: String::new(),
                duration: 0.,
                webpage_url: "/private/machine/movie.mp4".into(),
                extractor: "local".into(),
                id: "source".into(),
            },
            sections: vec![Section {
                t: 0.,
                end: 1.,
                image: "frames/a.png".into(),
                speech: vec![TranscriptEvent {
                    start: 0.,
                    end: 1.,
                    text: "Actual note".into(),
                    raw: None,
                    translation: None,
                }],
            }],
            summary: None,
        };
        let destination = root.path().join("moved");
        for format in [OutputFormat::Md, OutputFormat::Html, OutputFormat::Json] {
            write_document(
                &source,
                &document,
                format,
                &destination.join(file_name(format)),
            )
            .unwrap();
        }
        std::fs::remove_dir_all(source).unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(destination.join("course.zip")).unwrap())
                .unwrap();
        assert_eq!(zip.len(), 2);
        assert!(zip.by_name("course.md").is_ok());
        let html = std::fs::read_to_string(destination.join("course.html")).unwrap();
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.contains("src=\"images/"));
        assert!(!html.contains("时长 00:00"));
        let json = std::fs::read_to_string(destination.join("structured.json")).unwrap();
        assert!(json.contains("image_id"));
        assert!(!json.contains("/private/machine"));
        assert!(!json.contains("frames/a.png"));
        let existing = destination.join("keep.html");
        std::fs::write(&existing, "user content").unwrap();
        assert!(write_document(root.path(), &document, OutputFormat::Html, &existing).is_err());
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "user content");
    }

    #[test]
    fn markdown_files_share_hashed_assets_and_export_names_avoid_devices() {
        use crate::{
            fetch::VideoMeta,
            timeline::{Section, TranscriptEvent},
        };
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir_all(source.join("frames")).unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([20, 80, 180]))
            .save(source.join("frames/a.png"))
            .unwrap();
        let document = Document {
            schema: 1,
            meta: VideoMeta {
                title: "title".into(),
                uploader: String::new(),
                duration: 1.,
                webpage_url: String::new(),
                extractor: "web".into(),
                id: "source".into(),
            },
            sections: vec![Section {
                t: 0.,
                end: 1.,
                image: "frames/a.png".into(),
                speech: vec![TranscriptEvent {
                    start: 0.,
                    end: 1.,
                    text: "Actual note".into(),
                    raw: None,
                    translation: None,
                }],
            }],
            summary: None,
        };
        let output = root.path().join("output");
        write_markdown_document(&source, &document, &output.join("One.md")).unwrap();
        write_markdown_document(&source, &document, &output.join("Two.md")).unwrap();
        assert_eq!(std::fs::read_dir(output.join("assets")).unwrap().count(), 1);
        let markdown = std::fs::read_to_string(output.join("One.md")).unwrap();
        assert!(markdown.contains("assets/image-"));
        assert_eq!(export_component("CON", "未命名笔记"), "_CON");
        assert_eq!(export_component("<> ", "未命名笔记"), "未命名笔记");
        assert!(write_markdown_document(&source, &document, &output.join("One.md")).is_err());
    }
}
