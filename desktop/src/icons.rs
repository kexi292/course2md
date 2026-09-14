//! Embedded Material Icons, including the component library's standard icon paths.
use gpui::{AssetSource, SharedString, Styled};
use std::borrow::Cow;

pub struct Assets;
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/book-open.svg",
        include_bytes!("../assets/material/book-open.svg"),
    ),
    (
        "icons/plus.svg",
        include_bytes!("../assets/material/plus.svg"),
    ),
    (
        "icons/task.svg",
        include_bytes!("../assets/material/task.svg"),
    ),
    (
        "icons/refresh.svg",
        include_bytes!("../assets/material/refresh.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/material/settings.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/material/folder.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../assets/material/folder-open.svg"),
    ),
    (
        "icons/arrow-left.svg",
        include_bytes!("../assets/material/arrow-left.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../assets/material/chevron-up.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/material/chevron-down.svg"),
    ),
    (
        "icons/chevron-left.svg",
        include_bytes!("../assets/material/chevron-left.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../assets/material/chevron-right.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/material/close.svg"),
    ),
    (
        "icons/circle-x.svg",
        include_bytes!("../assets/material/circle-x.svg"),
    ),
    (
        "icons/file.svg",
        include_bytes!("../assets/material/file.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/material/check.svg"),
    ),
    (
        "icons/eye.svg",
        include_bytes!("../assets/material/eye.svg"),
    ),
    (
        "icons/eye-off.svg",
        include_bytes!("../assets/material/eye-off.svg"),
    ),
    (
        "icons/minus.svg",
        include_bytes!("../assets/material/minus.svg"),
    ),
    (
        "icons/external-link.svg",
        include_bytes!("../assets/material/external-link.svg"),
    ),
    (
        "icons/circle-check.svg",
        include_bytes!("../assets/material/circle-check.svg"),
    ),
    (
        "icons/info.svg",
        include_bytes!("../assets/material/info.svg"),
    ),
    (
        "icons/triangle-alert.svg",
        include_bytes!("../assets/material/triangle-alert.svg"),
    ),
    (
        "icons/window-close.svg",
        include_bytes!("../assets/material/window-close.svg"),
    ),
    (
        "icons/window-maximize.svg",
        include_bytes!("../assets/material/window-maximize.svg"),
    ),
    (
        "icons/window-minimize.svg",
        include_bytes!("../assets/material/window-minimize.svg"),
    ),
    (
        "icons/window-restore.svg",
        include_bytes!("../assets/material/window-restore.svg"),
    ),
    (
        "icons/loader.svg",
        include_bytes!("../assets/material/loader.svg"),
    ),
    (
        "icons/loader-circle.svg",
        include_bytes!("../assets/material/loader-circle.svg"),
    ),
    (
        "icons/link.svg",
        include_bytes!("../assets/material/link.svg"),
    ),
    (
        "icons/movie.svg",
        include_bytes!("../assets/material/movie.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/material/search.svg"),
    ),
    (
        "icons/more-horiz.svg",
        include_bytes!("../assets/material/more-horiz.svg"),
    ),
    (
        "icons/download.svg",
        include_bytes!("../assets/material/download.svg"),
    ),
    (
        "icons/play-arrow.svg",
        include_bytes!("../assets/material/play-arrow.svg"),
    ),
    (
        "icons/schedule.svg",
        include_bytes!("../assets/material/schedule.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../assets/material/image.svg"),
    ),
    (
        "icons/format-list-bulleted.svg",
        include_bytes!("../assets/material/format-list-bulleted.svg"),
    ),
    (
        "icons/arrow-forward.svg",
        include_bytes!("../assets/material/arrow-forward.svg"),
    ),
    (
        "icons/pause.svg",
        include_bytes!("../assets/material/pause.svg"),
    ),
    (
        "icons/file-upload.svg",
        include_bytes!("../assets/material/file-upload.svg"),
    ),
    (
        "icons/content-copy.svg",
        include_bytes!("../assets/material/content-copy.svg"),
    ),
    (
        "icons/history.svg",
        include_bytes!("../assets/material/history.svg"),
    ),
    (
        "icons/edit.svg",
        include_bytes!("../assets/material/edit.svg"),
    ),
    (
        "icons/create-new-folder.svg",
        include_bytes!("../assets/material/create-new-folder.svg"),
    ),
    (
        "icons/delete.svg",
        include_bytes!("../assets/material/delete.svg"),
    ),
    (
        "icons/zoom-in.svg",
        include_bytes!("../assets/material/zoom-in.svg"),
    ),
    (
        "icons/zoom-out.svg",
        include_bytes!("../assets/material/zoom-out.svg"),
    ),
    (
        "icons/fit-screen.svg",
        include_bytes!("../assets/material/fit-screen.svg"),
    ),
    (
        "icons/article.svg",
        include_bytes!("../assets/material/article.svg"),
    ),
    (
        "icons/subtitles.svg",
        include_bytes!("../assets/material/subtitles.svg"),
    ),
    (
        "icons/mic.svg",
        include_bytes!("../assets/material/mic.svg"),
    ),
    (
        "icons/computer.svg",
        include_bytes!("../assets/material/computer.svg"),
    ),
    (
        "icons/cloud.svg",
        include_bytes!("../assets/material/cloud.svg"),
    ),
    (
        "icons/auto-fix.svg",
        include_bytes!("../assets/material/auto-fix.svg"),
    ),
    (
        "icons/code.svg",
        include_bytes!("../assets/material/code.svg"),
    ),
    (
        "icons/web.svg",
        include_bytes!("../assets/material/web.svg"),
    ),
    (
        "icons/palette.svg",
        include_bytes!("../assets/material/palette.svg"),
    ),
    (
        "icons/storage.svg",
        include_bytes!("../assets/material/storage.svg"),
    ),
    (
        "icons/tune.svg",
        include_bytes!("../assets/material/tune.svg"),
    ),
    (
        "icons/save.svg",
        include_bytes!("../assets/material/save.svg"),
    ),
    (
        "icons/science.svg",
        include_bytes!("../assets/material/science.svg"),
    ),
    (
        "icons/login.svg",
        include_bytes!("../assets/material/login.svg"),
    ),
    (
        "icons/logout.svg",
        include_bytes!("../assets/material/logout.svg"),
    ),
    (
        "icons/qr-code.svg",
        include_bytes!("../assets/material/qr-code.svg"),
    ),
    (
        "icons/sun.svg",
        include_bytes!("../assets/material/sun.svg"),
    ),
    (
        "icons/contrast.svg",
        include_bytes!("../assets/material/contrast.svg"),
    ),
    (
        "icons/home.svg",
        include_bytes!("../assets/material/home.svg"),
    ),
    (
        "icons/arrow-up.svg",
        include_bytes!("../assets/material/arrow-up.svg"),
    ),
    (
        "icons/arrow-down.svg",
        include_bytes!("../assets/material/arrow-down.svg"),
    ),
    (
        "icons/ellipsis.svg",
        include_bytes!("../assets/material/ellipsis.svg"),
    ),
    (
        "icons/stop.svg",
        include_bytes!("../assets/material/stop.svg"),
    ),
    (
        "icons/checklist.svg",
        include_bytes!("../assets/material/checklist.svg"),
    ),
    (
        "icons/dashboard.svg",
        include_bytes!("../assets/material/dashboard.svg"),
    ),
    (
        "icons/shield.svg",
        include_bytes!("../assets/material/shield.svg"),
    ),
    (
        "icons/grid-view.svg",
        include_bytes!("../assets/material/grid-view.svg"),
    ),
    (
        "icons/summarize.svg",
        include_bytes!("../assets/material/summarize.svg"),
    ),
    (
        "icons/moon.svg",
        include_bytes!("../assets/material/moon.svg"),
    ),
    (
        "icons/restart.svg",
        include_bytes!("../assets/material/restart.svg"),
    ),
    (
        "brands/youtube.svg",
        include_bytes!("../assets/brands/youtube.svg"),
    ),
    (
        "brands/bilibili.svg",
        include_bytes!("../assets/brands/bilibili.svg"),
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "images/course2md.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/128x128@2x.png"
            ))));
        }
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = gpui_kit_assets::Assets.list(path)?;
        paths.extend(
            ICONS
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::from(*name)),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
macro_rules! icon_fn {
    ($name:ident, $path:literal) => {
        pub fn $name() -> gpui_component::Icon {
            gpui_component::Icon::default().path($path)
        }
    };
}
icon_fn!(book_open, "icons/book-open.svg");
icon_fn!(plus, "icons/plus.svg");
icon_fn!(task, "icons/task.svg");
icon_fn!(refresh, "icons/refresh.svg");
icon_fn!(settings, "icons/settings.svg");
icon_fn!(folder, "icons/folder.svg");
icon_fn!(folder_open, "icons/folder-open.svg");
icon_fn!(arrow_left, "icons/arrow-left.svg");
icon_fn!(chevron_up, "icons/chevron-up.svg");
icon_fn!(chevron_down, "icons/chevron-down.svg");
icon_fn!(chevron_left, "icons/chevron-left.svg");
icon_fn!(chevron_right, "icons/chevron-right.svg");
icon_fn!(close, "icons/close.svg");
icon_fn!(circle_x, "icons/circle-x.svg");
icon_fn!(file, "icons/file.svg");
icon_fn!(check, "icons/check.svg");
icon_fn!(eye, "icons/eye.svg");
icon_fn!(eye_off, "icons/eye-off.svg");
icon_fn!(minus, "icons/minus.svg");
icon_fn!(external_link, "icons/external-link.svg");
icon_fn!(circle_check, "icons/circle-check.svg");
icon_fn!(info, "icons/info.svg");
icon_fn!(triangle_alert, "icons/triangle-alert.svg");
icon_fn!(window_close, "icons/window-close.svg");
icon_fn!(window_maximize, "icons/window-maximize.svg");
icon_fn!(window_minimize, "icons/window-minimize.svg");
icon_fn!(window_restore, "icons/window-restore.svg");
icon_fn!(loader, "icons/loader.svg");
icon_fn!(loader_circle, "icons/loader-circle.svg");
icon_fn!(link, "icons/link.svg");
icon_fn!(movie, "icons/movie.svg");
icon_fn!(search, "icons/search.svg");
icon_fn!(more_horiz, "icons/more-horiz.svg");
icon_fn!(download, "icons/download.svg");
icon_fn!(play_arrow, "icons/play-arrow.svg");
icon_fn!(schedule, "icons/schedule.svg");
icon_fn!(image, "icons/image.svg");
icon_fn!(format_list_bulleted, "icons/format-list-bulleted.svg");
icon_fn!(arrow_forward, "icons/arrow-forward.svg");
icon_fn!(pause, "icons/pause.svg");
icon_fn!(file_upload, "icons/file-upload.svg");
icon_fn!(content_copy, "icons/content-copy.svg");
icon_fn!(history, "icons/history.svg");
icon_fn!(edit, "icons/edit.svg");
icon_fn!(create_new_folder, "icons/create-new-folder.svg");
icon_fn!(delete, "icons/delete.svg");
icon_fn!(zoom_in, "icons/zoom-in.svg");
icon_fn!(zoom_out, "icons/zoom-out.svg");
icon_fn!(fit_screen, "icons/fit-screen.svg");
icon_fn!(article, "icons/article.svg");
icon_fn!(subtitles, "icons/subtitles.svg");
icon_fn!(mic, "icons/mic.svg");
icon_fn!(computer, "icons/computer.svg");
icon_fn!(cloud, "icons/cloud.svg");
icon_fn!(auto_fix, "icons/auto-fix.svg");
icon_fn!(code, "icons/code.svg");
icon_fn!(web, "icons/web.svg");
icon_fn!(palette, "icons/palette.svg");
icon_fn!(storage, "icons/storage.svg");
icon_fn!(tune, "icons/tune.svg");
icon_fn!(save, "icons/save.svg");
icon_fn!(science, "icons/science.svg");
icon_fn!(login, "icons/login.svg");
icon_fn!(logout, "icons/logout.svg");
icon_fn!(qr_code, "icons/qr-code.svg");
icon_fn!(sun, "icons/sun.svg");
icon_fn!(contrast, "icons/contrast.svg");
icon_fn!(home, "icons/home.svg");
icon_fn!(arrow_up, "icons/arrow-up.svg");
icon_fn!(arrow_down, "icons/arrow-down.svg");
icon_fn!(ellipsis, "icons/ellipsis.svg");
icon_fn!(stop, "icons/stop.svg");
icon_fn!(checklist, "icons/checklist.svg");
icon_fn!(dashboard, "icons/dashboard.svg");
icon_fn!(shield, "icons/shield.svg");
icon_fn!(grid_view, "icons/grid-view.svg");
icon_fn!(summarize, "icons/summarize.svg");
icon_fn!(moon, "icons/moon.svg");
icon_fn!(restart, "icons/restart.svg");
icon_fn!(check_circle, "icons/circle-check.svg");
icon_fn!(error, "icons/circle-x.svg");
icon_fn!(warning, "icons/triangle-alert.svg");
icon_fn!(microphone, "icons/mic.svg");
icon_fn!(add, "icons/plus.svg");
icon_fn!(toc, "icons/format-list-bulleted.svg");
// Brand colors are intrinsic to the platform marks, independent of UI themes.
pub fn youtube() -> gpui_component::Icon {
    gpui_component::Icon::default()
        .path("brands/youtube.svg")
        .text_color(gpui::rgb(0xff0000))
}

pub fn bilibili() -> gpui_component::Icon {
    gpui_component::Icon::default()
        .path("brands/bilibili.svg")
        .text_color(gpui::rgb(0x00a1d6))
}
