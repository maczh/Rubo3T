//! 简易语法高亮（对齐 Robo3T 1.4.4 深色脚本编辑器 / JSON 文本视图）。
//!
//! 规则：普通文字颜色与字符串颜色均由调用方通过 `highlight_with_fg` 指定，单引号或
//! 双引号括起的字符串（含 `\` 转义识别，跨行不跨段——每帧全文重新扫描，状态不残留）。
//! 通过 `TextEdit::layouter` 接入，供脚本编辑器与结果 JSON 文本框共用。

use std::sync::Arc;

use eframe::egui;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, FontId, Galley, TextFormat, Ui};

/// 字符串绿色。
pub const STR: Color32 = Color32::from_rgb(0x00, 0xC0, 0x00);

/// 生成高亮排版结果，并允许调用方分别指定普通文字颜色与字符串颜色。
pub fn highlight_with_fg(
    ui: &Ui,
    text: &str,
    wrap_width: f32,
    fg: Color32,
    str_color: Color32,
) -> Arc<Galley> {
    let font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| FontId::monospace(12.0));
    let default_fmt = TextFormat::simple(font.clone(), fg);
    let string_fmt = TextFormat::simple(font, str_color);

    let mut job = LayoutJob::default();
    job.wrap = TextWrapping::wrap_at_width(wrap_width);

    let mut plain = String::new();
    let mut s = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for c in text.chars() {
        if let Some(q) = quote {
            s.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                // 引号闭合：整段按绿色着色
                job.append(&s, 0.0, string_fmt.clone());
                s.clear();
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            // 引号开启：先落盘前面的普通文本
            job.append(&plain, 0.0, default_fmt.clone());
            plain.clear();
            quote = Some(c);
            s.clear();
            s.push(c);
        } else {
            plain.push(c);
        }
    }
    // 收尾：未闭合字符串与剩余普通文本
    if !plain.is_empty() {
        job.append(&plain, 0.0, default_fmt);
    }
    if !s.is_empty() {
        job.append(&s, 0.0, string_fmt);
    }
    ui.fonts(|f| f.layout_job(job))
}
