//! 加载系统 CJK 字体并注入 egui，解决中文显示成 tofu 方块的问题。
//!
//! 优先查找 Windows 自带的微软雅黑 / SimHei，回退到 Noto Sans CJK SC / PingFang SC /
//! Hiragino Sans GB / WenQuanYi Micro Hei 等系统字体。找到后插入到 Proportional 和
//! Monospace 字体族的 fallback 链中。

use egui::{Color32, Stroke, Visuals};

pub fn load_cjk_fonts(ctx: &egui::Context) {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[
            fontdb::Family::Name("Microsoft YaHei"),
            fontdb::Family::Name("Microsoft YaHei UI"),
            fontdb::Family::Name("SimHei"),
            fontdb::Family::Name("Noto Sans CJK SC"),
            fontdb::Family::Name("PingFang SC"),
            fontdb::Family::Name("Source Han Sans SC"),
            fontdb::Family::Name("Hiragino Sans GB"),
            fontdb::Family::Name("WenQuanYi Micro Hei"),
            fontdb::Family::SansSerif,
        ],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let id = match db.query(&query) {
        Some(id) => id,
        None => return,
    };
    let face = match db.face(id) {
        Some(f) => f,
        None => return,
    };
    let bytes: Vec<u8> = match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).unwrap_or_default(),
        fontdb::Source::Binary(blob) => blob.as_ref().as_ref().to_vec(),
        #[allow(unreachable_patterns)]
        _ => return,
    };
    if bytes.is_empty() {
        return;
    }
    let font_name = face
        .families
        .first()
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| "cjk".to_string());
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(font_name.clone(), egui::FontData::from_owned(bytes));
    if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop.insert(0, font_name.clone());
    }
    if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        mono.push(font_name);
    }
    ctx.set_fonts(fonts);
}

/// 应用 Robo3T 1.4 风格的浅色主题。
///
/// 在 CJK 字体加载之后、首次 `update` 时调用（仅影响 `Visuals`，不碰 `set_fonts`
/// 已注入的字体），使界面整体对齐 Robo3T 1.4.4 的浅灰观感：浅灰窗体、白色中央结果
/// 区、偏蓝灰的侧栏/工具栏、蓝色选中高亮 + 白色选中文字、浅蓝灰悬停。
///
/// 注意：egui 0.29 的 `Selection` 没有独立的 `text_color` 字段，选中态文字颜色取自
/// `selection.stroke.color`，故这里把 `selection.stroke` 设为白色以得到白字蓝底的
/// Robo3T 风格选中效果。
pub fn apply_robo_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    // 整体背景：浅灰窗体，中央结果区偏白，侧栏/工具栏更深一点的蓝灰
    visuals.window_fill = Color32::from_rgb(240, 240, 240);
    visuals.panel_fill = Color32::from_rgb(255, 255, 255);
    visuals.extreme_bg_color = Color32::from_rgb(225, 228, 232);
    // 交替行底色
    visuals.faint_bg_color = Color32::from_rgb(244, 247, 250);
    // 链接/强调色
    visuals.hyperlink_color = Color32::from_rgb(0, 120, 215);

    // 选中高亮（树/表格行/列表）：蓝色背景
    visuals.selection.bg_fill = Color32::from_rgb(30, 104, 160);
    // egui 0.29 用 selection.stroke 作为选中态文字颜色 → 设为白色得到白字蓝底
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

    // 控件配色（基于浅色主题再微调，贴近 Robo3T）
    let mut widgets = visuals.widgets.clone();
    // 不可交互控件（窗口/分隔背景）
    widgets.noninteractive.bg_fill = Color32::from_rgb(225, 228, 232);
    widgets.noninteractive.weak_bg_fill = Color32::from_rgb(225, 228, 232);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(208, 211, 216));
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40));
    // 静止交互控件（按钮等）
    widgets.inactive.bg_fill = Color32::from_rgb(240, 240, 240);
    widgets.inactive.weak_bg_fill = Color32::from_rgb(240, 240, 240);
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(200, 200, 200));
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40));
    // 悬停：浅蓝灰
    widgets.hovered.bg_fill = Color32::from_rgb(210, 225, 240);
    widgets.hovered.weak_bg_fill = Color32::from_rgb(210, 225, 240);
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(120, 170, 210));
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0, 0, 0));
    // 按下/激活：更蓝
    widgets.active.bg_fill = Color32::from_rgb(186, 212, 236);
    widgets.active.weak_bg_fill = Color32::from_rgb(186, 212, 236);
    widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(40, 120, 170));
    widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(30, 45, 60));
    // 打开的菜单（combo 等）沿用悬停配色
    widgets.open = widgets.hovered;

    visuals.widgets = widgets;

    ctx.set_visuals(visuals);
}

#[cfg(test)]
mod tests {
    #[test]
    fn emphasis_text_is_readable_on_light_headers() {
        let ctx = egui::Context::default();
        super::apply_robo_theme(&ctx);
        let color = ctx.style().visuals.strong_text_color();
        assert!(color.r() < 80 && color.g() < 80 && color.b() < 80);
    }
}
