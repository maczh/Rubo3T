//! 简易语法高亮（对齐 Robo3T 1.4.4 深色脚本编辑器 / JSON 文本视图）。
//!
//! 规则：普通文字颜色与字符串颜色均由调用方传入 `highlight_with_find`，单引号或
//! 双引号括起的字符串（含 `\` 转义识别，跨行不跨段——每帧全文重新扫描，状态不残留）。
//! 通过 `TextEdit::layouter` 接入，供脚本编辑器与结果 JSON 文本框共用。
//!
//! `highlight_with_find` 在语法高亮之上叠加「查找命中」高亮：当前命中白底红字，
//! 其余命中加一层弱高亮底色（不影响原文字色）。

use std::sync::Arc;

use eframe::egui;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, FontId, Galley, TextFormat, Ui};

/// 字符串绿色。
pub const STR: Color32 = Color32::from_rgb(0x00, 0xC0, 0x00);

/// 查找命中——当前项背景（白底）。
pub const MATCH_CURRENT_BG: Color32 = Color32::WHITE;
/// 查找命中——当前项文字（红字）。
pub const MATCH_CURRENT_FG: Color32 = Color32::RED;

/// 查找命中——其余项背景（暗底上的半透明琥珀色，弱高亮）。
/// `from_rgba_unmultiplied` 非 const，故用函数而非常量暴露。
pub fn match_other_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(0xFF, 0xC1, 0x07, 0x5B)
}

/// 在语法高亮（引号字符串）基础上叠加查找命中高亮。
///
/// - 当前命中：白底 + 红字（`MATCH_CURRENT_BG` / `MATCH_CURRENT_FG`）。
/// - 其余命中：仅加弱高亮底色（`match_other_bg()`），文字色保持原语法色。
///
/// `matches` 为按起点升序、互不重叠的字节区间；`current` 为当前命中在其中的索引。
/// 传入空 `matches` 时即纯语法高亮（无查找高亮）。
pub fn highlight_with_find(
    ui: &Ui,
    text: &str,
    wrap_width: f32,
    fg: Color32,
    str_color: Color32,
    matches: &[(usize, usize)],
    current: Option<usize>,
) -> Arc<Galley> {
    let font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| FontId::monospace(12.0));

    // kind: 0 = 非命中, 1 = 其余命中, 2 = 当前命中
    let fmt_for = |is_string: bool, kind: u8| -> TextFormat {
        let base = if is_string { str_color } else { fg };
        let (color, background) = match kind {
            2 => (MATCH_CURRENT_FG, MATCH_CURRENT_BG),
            1 => (base, match_other_bg()),
            _ => (base, Color32::TRANSPARENT),
        };
        TextFormat {
            font_id: font.clone(),
            color,
            background,
            ..Default::default()
        }
    };

    let mut job = LayoutJob {
        wrap: TextWrapping::wrap_at_width(wrap_width),
        ..Default::default()
    };

    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut byte: usize = 0;
    let mut seg_start: usize = 0;
    let mut mi: usize = 0;
    // 当前区段分类 (is_string, kind)；None 表示尚未开始任何区段
    let mut run: Option<(bool, u8)> = None;

    for c in text.chars() {
        let clen = c.len_utf8();

        // 该字符是否属于字符串（含起止引号本身）
        let is_string = quote.is_some() || c == '"' || c == '\'';

        // 推进命中指针并判定当前字节落在哪个命中内
        while mi < matches.len() && matches[mi].1 <= byte {
            mi += 1;
        }
        let kind = if mi < matches.len() && matches[mi].0 <= byte && byte < matches[mi].1 {
            if Some(mi) == current {
                2
            } else {
                1
            }
        } else {
            0
        };

        // 分类变化时落盘上一个区段
        if run != Some((is_string, kind)) {
            if byte > seg_start {
                let (s, k) = run.unwrap_or((false, 0));
                job.append(&text[seg_start..byte], 0.0, fmt_for(s, k));
            }
            seg_start = byte;
            run = Some((is_string, kind));
        }

        // 更新引号 / 转义状态
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        }

        byte += clen;
    }

    // 收尾：剩余普通/字符串文本
    if byte > seg_start {
        let (s, k) = run.unwrap_or((false, 0));
        job.append(&text[seg_start..byte], 0.0, fmt_for(s, k));
    }

    ui.fonts(|f| f.layout_job(job))
}
