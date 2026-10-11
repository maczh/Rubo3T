//! 中央结果视图：Table / Tree / Text / Custom。

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;
use egui::{
    Color32, Frame, Galley, Margin, Painter, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2,
};

use rubo3t_core::Document;

use crate::app::{AppState, ViewMode};
use crate::icons;
use rubo3t_core::bson::Bson;

/// 脚本编辑器 / 文本视图的背景（#494C4E）与文字（浅色）。
const SCRIPT_BG: Color32 = Color32::from_rgb(0x49, 0x4C, 0x4E);
const SCRIPT_FG: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xE6);
/// 表头背景（浅灰）与列分隔线。
const HEADER_BG: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE8);
const SEP_COLOR: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);

/// 把单个 BSON 值渲染为简短字符串（表格单元格用）。
fn cell_text(value: &Bson) -> String {
    match value {
        Bson::Null => "null".to_string(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(f) => format!("{:.6}", f),
        Bson::String(s) => s.clone(),
        Bson::ObjectId(oid) => oid.to_hex(),
        Bson::DateTime(dt) => {
            let secs = dt.timestamp_millis() / 1000;
            let ms = dt.timestamp_millis() % 1000;
            format!("ISODate({}.{:03}Z)", secs, ms)
        }
        Bson::Array(a) => format!("[{} elements]", a.len()),
        Bson::Document(d) => format!("{{{} fields}}", d.len()),
        Bson::Binary(b) => format!("Binary({:?})", b.subtype),
        Bson::Timestamp(ts) => format!("{}/ {}", ts.time, ts.increment),
        Bson::Decimal128(d) => d.to_string(),
        Bson::RegularExpression(r) => format!("/ {}/ {}", r.pattern, r.options),
        Bson::DbPointer(_) => "DBPointer".to_string(),
        Bson::JavaScriptCode(c) => c.clone(),
        Bson::JavaScriptCodeWithScope(_) => "JavaScriptCodeWithScope".to_string(),
        Bson::Symbol(s) => s.clone(),
        Bson::Undefined => "undefined".to_string(),
        Bson::MinKey => "MinKey".to_string(),
        Bson::MaxKey => "MaxKey".to_string(),
    }
}

fn type_name(value: &Bson) -> &'static str {
    match value {
        Bson::Null => "Null",
        Bson::Boolean(_) => "Boolean",
        Bson::Int32(_) | Bson::Int64(_) | Bson::Double(_) | Bson::Decimal128(_) => "Number",
        Bson::String(_) => "String",
        Bson::ObjectId(_) => "ObjectId",
        Bson::DateTime(_) => "ISODate",
        Bson::Array(_) => "Array",
        Bson::Document(_) => "Object",
        Bson::Binary(_) => "Binary",
        Bson::Timestamp(_) => "Timestamp",
        Bson::RegularExpression(_) => "Regex",
        Bson::DbPointer(_) => "DBPointer",
        Bson::JavaScriptCode(_) | Bson::JavaScriptCodeWithScope(_) => "Code",
        Bson::Symbol(_) => "Symbol",
        Bson::Undefined => "Undefined",
        Bson::MinKey => "MinKey",
        Bson::MaxKey => "MaxKey",
    }
}

/// 从 documents 中提取所有顶层字段名，_id 放最前。
fn collect_columns(docs: &[Document]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for d in docs {
        for k in d.keys() {
            set.insert(k.to_string());
        }
    }
    let mut cols: Vec<String> = set.into_iter().collect();
    // _id 移到最前
    if let Some(pos) = cols.iter().position(|c| c == "_id") {
        let id = cols.remove(pos);
        cols.insert(0, id);
    }
    cols
}

/// 测量文本在 Body 样式下的像素宽度。
fn measure(ui: &Ui, text: &str) -> f32 {
    let style = ui.style();
    ui.fonts(|f| {
        f.layout_no_wrap(text.to_string(), egui::TextStyle::Body.resolve(&style), Color32::WHITE)
            .size()
            .x
    })
}

/// 绘制一个单元格：背景填充 + 左对齐文字（左内边距 4）+ 右侧 1px 分隔线。
fn draw_cell(painter: &Painter, rect: egui::Rect, galley: Arc<Galley>, bg: Color32, right_sep: bool) {
    painter.rect_filled(rect, Rounding::ZERO, bg);
    let pos = egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, Color32::PLACEHOLDER);
    if right_sep {
        painter.line_segment(
            [
                egui::pos2(rect.right(), rect.top()),
                egui::pos2(rect.right(), rect.bottom()),
            ],
            Stroke::new(1.0_f32, SEP_COLOR),
        );
    }
}

/// 渲染表格视图；返回被选中的行索引（如果有）。
pub fn table_view(ui: &mut Ui, state: &mut AppState) -> Option<usize> {
    let lang = state.lang;
    let docs = state.documents.clone();
    let cols = collect_columns(&docs);
    let row_height = 22.0;
    let header_h = ui.text_style_height(&egui::TextStyle::Body) + 10.0;
    let mut clicked: Option<usize> = None;

    if cols.is_empty() {
        ui.label(lang.tr("explorer.no_collections"));
        return None;
    }

    // 列宽：clamp(60,250)，以表头 + 首行测量（简化一次）
    let mut col_w: Vec<f32> = cols.iter().map(|_| 80.0).collect();
    for (i, c) in cols.iter().enumerate() {
        col_w[i] = col_w[i].max(measure(ui, c) + 16.0);
    }
    if let Some(doc) = docs.first() {
        for (i, c) in cols.iter().enumerate() {
            let v = doc.get(c).unwrap_or(&Bson::Null);
            col_w[i] = col_w[i].max(measure(ui, &cell_text(v)) + 16.0);
        }
    }
    for w in col_w.iter_mut() {
        *w = w.clamp(60.0, 250.0);
    }
    let row_num_w = 40.0;
    let avail = ui.available_width();

    // 表头
    let (hrect, _) = ui.allocate_exact_size(Vec2::new(avail, header_h), Sense::hover());
    let hpainter = ui.painter();
    let style = ui.style();
    let mut x = hrect.left();
    {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap("#".to_string(), egui::TextStyle::Body.resolve(&style), ui.visuals().strong_text_color())
        });
        draw_cell(&hpainter, egui::Rect::from_min_size(egui::pos2(x, hrect.top()), Vec2::new(row_num_w, header_h)), galley, HEADER_BG, true);
    }
    x += row_num_w;
    for (i, c) in cols.iter().enumerate() {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(c.to_string(), egui::TextStyle::Body.resolve(&style), ui.visuals().strong_text_color())
        });
        draw_cell(&hpainter, egui::Rect::from_min_size(egui::pos2(x, hrect.top()), Vec2::new(col_w[i], header_h)), galley, HEADER_BG, true);
        x += col_w[i];
    }
    ui.separator();

    // 数据行
    ScrollArea::vertical().show_rows(ui, row_height, docs.len(), |ui, row_range| {
        let font_id = {
            let style = ui.style();
            egui::TextStyle::Body.resolve(&style)
        };
        for i in row_range {
            if i >= docs.len() {
                break;
            }
            let doc = &docs[i];
            let selected = state.selected_doc == Some(i);
            let (rect, response) = ui.allocate_at_least(Vec2::new(avail, row_height), Sense::click());
            if response.clicked() {
                clicked = Some(i);
            }
            let bg = if selected {
                ui.visuals().selection.bg_fill
            } else if i % 2 == 1 {
                ui.visuals().faint_bg_color
            } else {
                ui.visuals().panel_fill
            };
            let fg = if selected { Color32::WHITE } else { ui.visuals().text_color() };
            let painter = ui.painter_at(rect);
            let mut x = rect.left();
            {
                let galley = ui.fonts(|f| f.layout_no_wrap(format!("{}", i + 1), font_id.clone(), fg));
                draw_cell(&painter, egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(row_num_w, row_height)), galley, bg, true);
            }
            x += row_num_w;
            for (ci, c) in cols.iter().enumerate() {
                let v = doc.get(c).unwrap_or(&Bson::Null);
                let galley = ui.fonts(|f| f.layout_no_wrap(cell_text(v), font_id.clone(), fg));
                draw_cell(&painter, egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(col_w[ci], row_height)), galley, bg, true);
                x += col_w[ci];
            }
            let idx = i;
            response.context_menu(|ui| {
                doc_context_menu(ui, state, idx);
            });
        }
    });

    clicked
}

/// 文档右键菜单（View/Edit/Insert/Copy Value/Name/Path/Delete）。
fn doc_context_menu(ui: &mut Ui, state: &mut AppState, idx: usize) {
    let lang = state.lang;
    if ui.button(lang.tr("doc.view")).clicked() {
        state.open_view_document(idx);
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.edit")).clicked() {
        state.select_document(idx);
        let coll = state
            .tabs
            .get(state.active_tab)
            .map(|t| t.coll.clone())
            .unwrap_or_default();
        state.edit_doc_title = format!("\"{}\" - {}", coll, idx + 1);
        state.edit_doc_open = true;
        ui.close_menu();
    }
    ui.separator();
    if ui.button(lang.tr("doc.copy_json")).clicked() {
        if let Some(doc) = state.documents.get(idx) {
            if let Ok(t) = rubo3t_core::bson_ext::document_to_text(doc) {
                ui.output_mut(|o| o.copied_text = t);
            }
        }
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.copy_value")).clicked() {
        if let Some(doc) = state.documents.get(idx) {
            if let Some((_, v)) = doc.iter().next() {
                ui.output_mut(|o| o.copied_text = cell_text(v));
            }
        }
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.copy_name")).clicked() {
        if let Some(tab) = state.tabs.get(state.active_tab) {
            ui.output_mut(|o| o.copied_text = tab.coll.clone());
        }
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.copy_path")).clicked() {
        if let Some(tab) = state.tabs.get(state.active_tab) {
            if let Some(doc) = state.documents.get(idx) {
                let path = format!(
                    "{}.{}.{}",
                    tab.db,
                    tab.coll,
                    doc.get("_id").map(cell_text).unwrap_or_default()
                );
                ui.output_mut(|o| o.copied_text = path);
            }
        }
        ui.close_menu();
    }
    ui.separator();
    if ui.button(lang.tr("doc.insert_doc")).clicked() {
        state.selected_doc = None;
        state.selected_doc_text = "{\n  \"_id\": {}\n}".to_string();
        state.insert_doc_open = true;
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.delete_doc")).clicked() {
        state.delete_document(Some(idx));
        ui.close_menu();
    }
}

/// 树形视图渲染。
pub fn tree_view(ui: &mut Ui, state: &mut AppState) {
    let docs = state.documents.clone();
    let mut clicked: Option<usize> = None;
    ScrollArea::vertical().show(ui, |ui| {
        for (i, doc) in docs.iter().enumerate() {
            let header = format!("({}) {}", i + 1, doc.get("_id").map(cell_text).unwrap_or_default());
            let selected = state.selected_doc == Some(i);
            let resp = ui.selectable_label(selected, header);
            if resp.clicked() {
                clicked = Some(i);
            }
            let idx = i;
            resp.context_menu(|ui| {
                doc_context_menu(ui, state, idx);
            });
            tree_node(ui, &Bson::Document(doc.clone()), "", i == docs.len() - 1, 0);
        }
    });
    if let Some(i) = clicked {
        state.select_document(i);
    }
}

fn tree_node(ui: &mut Ui, value: &Bson, path: &str, _last: bool, depth: usize) {
    let indent = depth as f32 * 16.0;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        // 按 BSON 类型前缀对应图标（Robo3T 风格）
        ui.add(icons::img(bson_icon_name(value)).fit_to_original_size(1.0));
        match value {
            Bson::Document(doc) => {
                if doc.is_empty() {
                    ui.label(format!("{} {{}}", path));
                } else {
                    ui.collapsing(path, |ui| {
                        let keys: Vec<_> = doc.keys().map(|s| s.to_string()).collect();
                        for (idx, key) in keys.iter().enumerate() {
                            let child_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{}: {}", path, key)
                            };
                            let child = doc.get(key).unwrap_or(&Bson::Null);
                            tree_node(ui, child, &child_path, idx == keys.len() - 1, depth + 1);
                        }
                    });
                }
            }
            Bson::Array(arr) => {
                if arr.is_empty() {
                    ui.label(format!("{} []", path));
                } else {
                    ui.collapsing(path, |ui| {
                        for (idx, item) in arr.iter().enumerate() {
                            let child_path = format!("{}[{}]", path, idx);
                            tree_node(ui, item, &child_path, idx == arr.len() - 1, depth + 1);
                        }
                    });
                }
            }
            _ => {
                ui.label(format!("{} = {}  ({})", path, cell_text(value), type_name(value)));
            }
        }
    });
}

/// 根据 BSON 值类型返回对应的图标 URI 名称。
fn bson_icon_name(value: &Bson) -> &'static str {
    match value {
        Bson::Document(_) => "bson_object_16x16",
        Bson::Array(_) => "bson_array_16x16",
        Bson::String(_) => "bson_string_16x16",
        Bson::Int32(_) | Bson::Int64(_) => "bson_integer_16x16",
        Bson::Double(_) => "bson_double_16x16",
        Bson::Boolean(_) => "bson_bool_16x16",
        Bson::DateTime(_) => "bson_datetime_16x16",
        Bson::Null => "bson_null_16x16",
        Bson::Binary(_) => "bson_binary_16x16",
        Bson::Decimal128(_) => "bson_decimal128_16x16",
        _ => "bson_unsupported_16x16",
    }
}

/// 文本/JSON 视图：深色背景、浅色等宽文字、只读。
pub fn text_view(ui: &mut Ui, state: &mut AppState) {
    let text = state.selected_doc_text.clone();
    Frame::none()
        .fill(SCRIPT_BG)
        .rounding(Rounding::same(2.0))
        .inner_margin(Margin::same(4.0))
        .show(ui, |ui| {
            ui.style_mut().visuals.override_text_color = Some(SCRIPT_FG);
            ScrollArea::vertical().show(ui, |ui| {
                ui.monospace(&text);
            });
        });
}

/// 主入口：根据当前 view_mode 渲染，并收集动作。
pub fn render(ui: &mut Ui, state: &mut AppState) {
    match state.current_view_mode {
        ViewMode::Table => {
            if let Some(i) = table_view(ui, state) {
                state.select_document(i);
            }
        }
        ViewMode::Tree => tree_view(ui, state),
        ViewMode::Text => text_view(ui, state),
        ViewMode::Custom => {
            ui.label(state.lang.tr("view.custom"));
            text_view(ui, state);
        }
    }
}
