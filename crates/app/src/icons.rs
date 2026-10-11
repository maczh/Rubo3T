//! Robo3T 图标资源接入层。
//!
//! 把从 Robo3T 1.4.4 源码提取的 PNG 图标在编译期用 `include_bytes!` 编入二进制，
//! 运行时解码成 RGBA 纹理并注册到 egui 上下文（URI 形如 `icon://<name>`，可在
//! `egui::Image` / `egui::ImageButton` 中直接引用）。同时提供窗口图标。

use eframe::egui;
use egui::{ColorImage, Context, Id, Image, TextureOptions};

/// 所有图标：(URI 名称, 编译期嵌入的 PNG 字节)。
///
/// URI 名称即文件名去掉 `.png` 后缀，例如 `connect_24x24`、`bson_object_16x16`。
pub const ICONS: &[(&str, &'static [u8])] = &[
    ("connect_24x24", include_bytes!("../resources/icons/connect_24x24.png")),
    ("execute_24x24", include_bytes!("../resources/icons/execute_24x24.png")),
    ("stop_24x24", include_bytes!("../resources/icons/stop_24x24.png")),
    ("rotate_16x16", include_bytes!("../resources/icons/rotate_16x16.png")),
    ("text_highlighted_16x16", include_bytes!("../resources/icons/text_highlighted_16x16.png")),
    ("tree_highlighted_16x16", include_bytes!("../resources/icons/tree_highlighted_16x16.png")),
    ("table_highlighted_16x16", include_bytes!("../resources/icons/table_highlighted_16x16.png")),
    ("custom_highlighted_16x16", include_bytes!("../resources/icons/custom_highlighted_16x16.png")),
    ("server_16x16", include_bytes!("../resources/icons/server_16x16.png")),
    ("database_16x16", include_bytes!("../resources/icons/database_16x16.png")),
    ("collection_16x16", include_bytes!("../resources/icons/collection_16x16.png")),
    ("replica_set_16x16", include_bytes!("../resources/icons/replica_set_16x16.png")),
    ("function_16x16", include_bytes!("../resources/icons/function_16x16.png")),
    ("user_16x16", include_bytes!("../resources/icons/user_16x16.png")),
    ("index_16x16", include_bytes!("../resources/icons/index_16x16.png")),
    ("key_16x16", include_bytes!("../resources/icons/key_16x16.png")),
    ("mongodb_16x16", include_bytes!("../resources/icons/mongodb_16x16.png")),
    ("welcome_tab_icon", include_bytes!("../resources/icons/welcome_tab_icon.png")),
    ("close_16x16", include_bytes!("../resources/icons/close_16x16.png")),
    ("left_16x16", include_bytes!("../resources/icons/left_16x16.png")),
    ("right_16x16", include_bytes!("../resources/icons/right_16x16.png")),
    ("logo", include_bytes!("../resources/icons/logo.png")),
    ("logo-256x256", include_bytes!("../resources/icons/logo-256x256.png")),
    ("bson_object_16x16", include_bytes!("../resources/icons/bson_object_16x16.png")),
    ("bson_array_16x16", include_bytes!("../resources/icons/bson_array_16x16.png")),
    ("bson_string_16x16", include_bytes!("../resources/icons/bson_string_16x16.png")),
    ("bson_integer_16x16", include_bytes!("../resources/icons/bson_integer_16x16.png")),
    ("bson_double_16x16", include_bytes!("../resources/icons/bson_double_16x16.png")),
    ("bson_bool_16x16", include_bytes!("../resources/icons/bson_bool_16x16.png")),
    ("bson_datetime_16x16", include_bytes!("../resources/icons/bson_datetime_16x16.png")),
    ("bson_null_16x16", include_bytes!("../resources/icons/bson_null_16x16.png")),
    ("bson_binary_16x16", include_bytes!("../resources/icons/bson_binary_16x16.png")),
    ("bson_decimal128_16x16", include_bytes!("../resources/icons/bson_decimal128_16x16.png")),
    ("bson_unsupported_16x16", include_bytes!("../resources/icons/bson_unsupported_16x16.png")),
    // 新增：工作区标签关闭按钮 + 结果视图按钮（未选中态）+ 耗时图标
    ("close_2_16x16", include_bytes!("../resources/icons/close_2_16x16.png")),
    ("close_hover_16x16", include_bytes!("../resources/icons/close_hover_16x16.png")),
    ("table_16x16", include_bytes!("../resources/icons/table_16x16.png")),
    ("tree_16x16", include_bytes!("../resources/icons/tree_16x16.png")),
    ("text_16x16", include_bytes!("../resources/icons/text_16x16.png")),
    ("custom_16x16", include_bytes!("../resources/icons/custom_16x16.png")),
    ("time_16x16", include_bytes!("../resources/icons/time_16x16.png")),
];

/// 程序化绘制的额外图标名称（Open 文件夹 / Save 软盘，浅灰线条）。
const PROGRAMMATIC_ICONS: &[(&str, IconKind)] = &[
    ("open_16x16", IconKind::Folder),
    ("save_16x16", IconKind::Floppy),
];

/// 程序化图标的种类。
#[derive(Clone, Copy)]
enum IconKind {
    Folder,
    Floppy,
}

/// 用逐像素回调生成一个 16×16 的 RGBA `ColorImage`。
///
/// 回调返回 `Some([r,g,b])` 表示该像素为不透明线条/填充色，返回 `None` 表示透明。
fn build_icon<F: Fn(usize, usize) -> Option<[u8; 3]>>(size: usize, f: F) -> ColorImage {
    let mut pixels: Vec<u8> = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            if let Some([r, g, b]) = f(x, y) {
                pixels.extend_from_slice(&[r, g, b, 255]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    ColorImage::from_rgba_unmultiplied([size, size], &pixels)
}

/// 浅灰线条文件夹图标（近似 Qt 的 SP_DialogOpenButton）。
fn folder_icon() -> ColorImage {
    let line = [0x5f, 0x5f, 0x5f];
    build_icon(16, |x, y| {
        let on = (y == 2 || y == 3) && x >= 1 && x <= 6
            || (y == 4 && x >= 1 && x <= 14)
            || (y == 13 && x >= 1 && x <= 14)
            || (x == 1 && y >= 4 && y <= 13)
            || (x == 14 && y >= 4 && y <= 13);
        if on {
            Some(line)
        } else {
            None
        }
    })
}

/// 浅灰线条软盘图标（近似 Qt 的 SP_DialogSaveButton）。
fn floppy_icon() -> ColorImage {
    let line = [0x5f, 0x5f, 0x5f];
    build_icon(16, |x, y| {
        let outline = ((y == 1 || y == 14) && x >= 1 && x <= 14)
            || ((x == 1 || x == 14) && y >= 1 && y <= 14);
        let shutter = y >= 1 && y <= 5 && x >= 9 && x <= 13;
        let label = y >= 9 && y <= 13 && x >= 3 && x <= 12;
        let hole = y >= 2 && y <= 4 && x >= 10 && x <= 12;
        let label_line = y >= 11 && y <= 12 && x >= 4 && x <= 11;
        if (outline || shutter || label) && !hole && !label_line {
            Some(line)
        } else {
            None
        }
    })
}

/// 标记：图标是否已全部注册（存于 egui 上下文，保证进程内只注册一次）。
const REGISTERED_MARKER: &str = "robo3t_icons_registered";

/// 把全部图标解码并注册为 egui 纹理。
///
/// 已注册的图标会跳过，因此可安全重复调用（配合 [`AppState::icon_registered`] 在
/// 每帧更新开头调用）。
pub fn register(ctx: &Context) {
    let marker = Id::new(REGISTERED_MARKER);
    if ctx.data(|d| d.get_temp::<bool>(marker) == Some(true)) {
        return;
    }
    for (name, bytes) in ICONS {
        let per_icon = Id::new(("robo3t_icon", *name));
        if ctx.data(|d| d.get_temp::<bool>(per_icon) == Some(true)) {
            continue;
        }
        match image::load_from_memory(bytes) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                let pixels = rgba.into_raw();
                let color_image = ColorImage::from_rgba_unmultiplied([w, h], &pixels);
                let _ = ctx.load_texture(
                    format!("icon://{}", name),
                    color_image,
                    TextureOptions::default(),
                );
                ctx.data_mut(|d| d.insert_temp(per_icon, true));
            }
            Err(e) => {
                eprintln!("[icons] 无法解码图标 {}: {}", name, e);
            }
        }
    }
    // 程序化图标（Open 文件夹 / Save 软盘）
    for (name, kind) in PROGRAMMATIC_ICONS {
        let per_icon = Id::new(("robo3t_icon", *name));
        if ctx.data(|d| d.get_temp::<bool>(per_icon) == Some(true)) {
            continue;
        }
        let color_image = match kind {
            IconKind::Folder => folder_icon(),
            IconKind::Floppy => floppy_icon(),
        };
        let _ = ctx.load_texture(
            format!("icon://{}", name),
            color_image,
            TextureOptions::default(),
        );
        ctx.data_mut(|d| d.insert_temp(per_icon, true));
    }
    ctx.data_mut(|d| d.insert_temp(marker, true));
}

/// 构造引用某个已注册图标的 [`Image`]。
///
/// 调用处一般配合 `.fit_to_original_size(1.0)` 控制尺寸（工具栏 24×24、树 16×16 自然呈现）。
pub fn img(name: &str) -> Image<'static> {
    Image::new(format!("icon://{}", name))
}

/// 构造一个固定边长（正方形）的图标 [`Image`]，等比缩放后不超过 `size` 像素。
///
/// 用于状态栏/输出头/分页等需要精确像素尺寸的地方（如 16×16、24×24）。
pub fn img_size(name: &str, size: f32) -> Image<'static> {
    Image::new(format!("icon://{}", name)).max_size(egui::vec2(size, size))
}

/// 返回窗口图标（解码 `logo-256x256.png`）。
pub fn window_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../resources/icons/logo-256x256.png"))
        .ok()
}
