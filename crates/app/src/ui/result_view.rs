//! 中央结果视图：Table / Tree / Text / Custom。

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use eframe::egui;
use egui::{
    Color32, Frame, Galley, Id, Margin, Painter, Pos2, Rect, Rounding, ScrollArea, Sense, Stroke,
    TextEdit, Ui, Vec2,
};

use rubo3t_core::Document;

use crate::app::{AppState, FindTarget, ViewMode};
use crate::icons;
use crate::ui::find;
use crate::ui::syntax;
use rubo3t_core::bson::Bson;

/// 脚本编辑器 / 文本视图的深色背景（#3D3F41）；文字颜色由 [`syntax`] 高亮提供。
const SCRIPT_BG: Color32 = Color32::from_rgb(0x3D, 0x3F, 0x41);
const TEXT_COLOR: Color32 = Color32::from_rgb(0xA1, 0xFC, 0xA2);
/// 文本视图中字符串（引号内）的绿色高亮，由用户指定为 #1F9D5E。
const TEXT_STR: Color32 = Color32::from_rgb(0x1F, 0x9D, 0x5E);
const TEXTBOX_BG: Color32 = Color32::from_rgb(0x49, 0x4C, 0x4E);
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
        Bson::DateTime(dt) => local_datetime_str(dt.timestamp_millis()),
        Bson::Array(a) => format!("[{} elements]", a.len()),
        Bson::Document(d) => format!("{{{} fields}}", d.len()),
        Bson::Binary(b) => format!("Binary({:?})", b.subtype),
        Bson::Timestamp(ts) => local_datetime_str((ts.time as i64) * 1000),
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
        Bson::Int32(_) => "Int32",
        Bson::Int64(_) => "Int64",
        Bson::Double(_) => "Double",
        Bson::Decimal128(_) => "Decimal128",
        Bson::String(_) => "String",
        Bson::ObjectId(_) => "ObjectId",
        Bson::DateTime(_) => "Date",
        Bson::Array(_) => "Array",
        Bson::Document(_) => "Object",
        Bson::Binary(_) => "Binary",
        Bson::Timestamp(_) => "Timestamp",
        Bson::RegularExpression(_) => "Regular Expression",
        Bson::DbPointer(_) => "DBRef",
        Bson::JavaScriptCode(_) => "Code",
        Bson::JavaScriptCodeWithScope(_) => "CodeWScope",
        Bson::Symbol(_) => "Symbol",
        Bson::Undefined => "Undefined",
        Bson::MinKey => "MinKey",
        Bson::MaxKey => "MaxKey",
    }
}

/// 把 UTC 毫秒（bson::DateTime / BSON Timestamp 内部存储）格式化为**本地时区**
/// `YYYY-MM-DD HH:mm:ss.sss` 字符串。毫秒不足补零，时区按运行环境本地时区解释。
///
/// 与 `bson_ext::document_to_text` 中日期展示共用同一格式（单一事实来源）。
fn local_datetime_str(utc_millis: i64) -> String {
    rubo3t_core::bson_ext::millis_to_local_str(utc_millis)
}

/// 树视图 Value 列文本（Robo3T `BsonUtils::buildJsonString` 规则：字符串不带引号、
/// ObjectId 显示为 `ObjectId("...")`、整型 double 补 `.0`、日期显示为 ISODate(...) 等）。
fn value_text(value: &Bson) -> String {
    match value {
        Bson::Null => "null".to_string(),
        Bson::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(f) => reformat_double(*f),
        Bson::Decimal128(d) => d.to_string(),
        Bson::String(s) => s.clone(),
        Bson::ObjectId(oid) => format!("ObjectId(\"{}\")", oid.to_hex()),
        Bson::DateTime(dt) => local_datetime_str(dt.timestamp_millis()),
        Bson::Binary(_) => "<binary>".to_string(),
        Bson::Timestamp(ts) => local_datetime_str((ts.time as i64) * 1000),
        Bson::RegularExpression(r) => format!("/{}/{}", r.pattern, r.options),
        Bson::Symbol(s) => s.clone(),
        Bson::Undefined => "undefined".to_string(),
        Bson::MinKey => "MinKey".to_string(),
        Bson::MaxKey => "MaxKey".to_string(),
        Bson::JavaScriptCode(c) => c.clone(),
        Bson::JavaScriptCodeWithScope(c) => c.code.clone(),
        Bson::DbPointer(_) => "<dbpointer>".to_string(),
        Bson::Array(a) => count_text('[', a.len(), "element", "elements"),
        Bson::Document(d) => count_text('{', d.len(), "field", "fields"),
    }
}

/// 整型 double 补 `.0`，避免科学计数法（对齐 Robo3T `reformatDoubleString`）。
fn reformat_double(f: f64) -> String {
    let s = f.to_string();
    if s.contains('e') || s.contains('E') {
        return format!("{:.6}", f);
    }
    if f.fract() == 0.0 {
        return format!("{:.1}", f);
    }
    s
}

/// 计数占位文本：`{ 1 field }` / `[ 2 elements ]`（对齐 C++ `objectValue`/`arrayValue`）。
fn count_text(brace: char, n: usize, singular: &str, plural: &str) -> String {
    let unit = if n == 1 { singular } else { plural };
    format!("{} {} {} ]", brace, n, unit)
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

    // 列宽默认以表头 + 首行测量；用户调整值按查询标签页和列名暂存。
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
    let tab_id = state
        .tabs
        .get(state.active_tab)
        .map(|tab| tab.id)
        .unwrap_or_default();
    let widths_id = Id::new(("result_table_column_widths", tab_id));
    let mut saved_widths = ui
        .ctx()
        .data(|data| data.get_temp::<HashMap<String, f32>>(widths_id))
        .unwrap_or_default();
    for w in col_w.iter_mut() {
        *w = w.clamp(60.0, 250.0);
    }
    for (i, c) in cols.iter().enumerate() {
        if let Some(width) = saved_widths.get(c) {
            col_w[i] = width.clamp(60.0, 250.0);
        }
    }
    let row_num_w = 40.0;
    let avail = ui.available_width();

    // 表头
    let (hrect, _) = ui.allocate_exact_size(Vec2::new(avail, header_h), Sense::hover());
    let mut resize_deltas = Vec::new();
    let mut edge_x = hrect.left() + row_num_w;
    for (i, width) in col_w.iter().enumerate() {
        edge_x += *width;
        let resize_rect = Rect::from_min_max(
            Pos2::new(edge_x - 4.0, hrect.top()),
            Pos2::new(edge_x + 4.0, hrect.bottom()),
        );
        let response = ui.interact(
            resize_rect,
            Id::new(("result_table_column_resize", tab_id, &cols[i])),
            Sense::drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if response.dragged() {
            let delta = ui.input(|input| input.pointer.delta().x);
            if delta != 0.0 {
                resize_deltas.push((i, delta));
            }
        }
    }
    for (i, delta) in resize_deltas {
        col_w[i] = (col_w[i] + delta).clamp(60.0, 250.0);
        saved_widths.insert(cols[i].clone(), col_w[i]);
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(widths_id, saved_widths));

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
    if ui.button(lang.tr("doc.export")).clicked() {
        state.export_open = true;
        state.export_doc_index = Some(idx);
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.export_all")).clicked() {
        state.export_open = true;
        state.export_doc_index = None;
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.import")).clicked() {
        state.import_open = true;
        ui.close_menu();
    }
    if ui.button(lang.tr("doc.delete_doc")).clicked() {
        state.delete_document(Some(idx));
        ui.close_menu();
    }
}

/// 树形视图渲染（1:1 复刻 Robo3T BsonTreeWidget：Key/Value/Type 三列）。
///
/// 每个文档是一个根节点，根 Key 形如 `(序号) <_id值>`，Value 为 `{ N fields }` /
/// `[ N elements ]`，Type 为 `Object` / `Array`。展开后递归渲染字段；嵌套的
/// 对象/数组同样可展开。节点展开态以 `id_path` 为键持久化在 egui 上下文中，
/// 因此不再调用 `ui.collapsing(path)`，从根本上消除了 widget ID 冲突（红字报错）。
pub fn tree_view(ui: &mut Ui, state: &mut AppState) {
    let docs = state.documents.clone();
    draw_tree_header(ui);
    let mut root_clicked: Option<usize> = None;
    ScrollArea::vertical().show(ui, |ui| {
        for (i, doc) in docs.iter().enumerate() {
            let id_val = doc
                .get("_id")
                .map(value_text)
                .unwrap_or_default();
            let root_key = format!("({}) {}", i + 1, id_val);
            let root_path = format!("d{}", i);
            let clicked = draw_tree_node(
                ui,
                i,
                &root_path,
                &root_key,
                &Bson::Document(doc.clone()),
                0,
                true,
                state,
            );
            if clicked {
                root_clicked = Some(i);
            }
        }
    });
    if let Some(i) = root_clicked {
        state.select_document(i);
    }
}

/// 绘制表头：Key / Value / Type（浅灰背景，与 Robo3T 一致）。
fn draw_tree_header(ui: &mut Ui) {
    let row_h = 22.0;
    let avail = ui.available_width();
    let key_w = (avail * 0.45).max(80.0);
    let value_w = (avail * 0.40).max(60.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail, row_h), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::ZERO, HEADER_BG);
    let strong = ui.visuals().strong_text_color();
    tree_text(ui, &painter, rect.left() + 6.0, rect.center().y, "Key", strong);
    tree_text(ui, &painter, rect.left() + key_w + 6.0, rect.center().y, "Value", strong);
    tree_text(
        ui,
        &painter,
        rect.left() + key_w + value_w + 6.0,
        rect.center().y,
        "Type",
        strong,
    );
}

/// 递归绘制一个树节点（一行），并在展开时递归绘制其子节点。
/// 返回该节点是否为文档根行被点击（用于选中文档并在文本视图中展示）。
fn draw_tree_node(
    ui: &mut Ui,
    doc_idx: usize,
    id_path: &str,
    key: &str,
    value: &Bson,
    depth: usize,
    is_root: bool,
    state: &mut AppState,
) -> bool {
    let row_h = 20.0;
    let avail = ui.available_width();
    let key_w = (avail * 0.45).max(80.0);
    let value_w = (avail * 0.40).max(60.0);

    let expandable = matches!(value, Bson::Document(_) | Bson::Array(_));
    let open_id = Id::new(("tree_open", id_path));
    let is_open = if expandable {
        ui.ctx()
            .data(|d| d.get_temp::<bool>(open_id))
            .unwrap_or(false)
    } else {
        false
    };

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, row_h), Sense::click());
    let painter = ui.painter_at(rect);

    let selected = is_root && state.selected_doc == Some(doc_idx);
    if selected {
        painter.rect_filled(rect, Rounding::ZERO, ui.visuals().selection.bg_fill);
    } else if resp.hovered() {
        painter.rect_filled(rect, Rounding::ZERO, ui.visuals().faint_bg_color);
    }

    let indent = 6.0 + depth as f32 * 16.0;
    let arrow_w = 16.0;
    let icon_w = 16.0;

    // 展开箭头（仅可展开节点）
    if expandable {
        let tri = if is_open { "▼" } else { "▶" };
        tree_text(
            ui,
            &painter,
            rect.left() + indent,
            rect.center().y,
            tri,
            ui.visuals().text_color(),
        );
    }

    // 类型图标（Key 列）
    let icon_x = rect.left() + indent + arrow_w + 2.0;
    let icon_rect = Rect::from_min_size(
        Pos2::new(icon_x, rect.top() + (row_h - icon_w) / 2.0),
        Vec2::new(icon_w, icon_w),
    );
    ui.put(icon_rect, icons::img(bson_icon_name(value)));

    // Key 文本
    let key_x = icon_x + icon_w + 4.0;
    let key_color = if selected {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    tree_text(ui, &painter, key_x, rect.center().y, key, key_color);

    // Value 文本（固定列）
    let value_x = rect.left() + key_w;
    let val_color = if selected {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    tree_text(
        ui,
        &painter,
        value_x + 6.0,
        rect.center().y,
        &value_text(value),
        val_color,
    );

    // Type 文本（灰色，固定列）
    let type_x = rect.left() + key_w + value_w;
    let type_color = if selected { Color32::WHITE } else { Color32::GRAY };
    tree_text(
        ui,
        &painter,
        type_x + 6.0,
        rect.center().y,
        type_name(value),
        type_color,
    );

    // 交互：可展开节点点击折叠/展开；根节点点击选中文档
    let mut root_clicked = false;
    if resp.clicked() {
        if expandable {
            let cur = ui
                .ctx()
                .data(|d| d.get_temp::<bool>(open_id))
                .unwrap_or(false);
            ui.ctx().data_mut(|d| d.insert_temp(open_id, !cur));
        }
        if is_root {
            root_clicked = true;
        }
    }

    // 右键菜单（仅根节点）
    if is_root {
        resp.context_menu(|ui| {
            doc_context_menu(ui, state, doc_idx);
        });
    }

    // 递归展开子节点
    if expandable && is_open {
        match value {
            Bson::Document(d) => {
                for (k, v) in d.iter() {
                    let child_path = format!("{}/{}", id_path, k);
                    draw_tree_node(ui, doc_idx, &child_path, k, v, depth + 1, false, state);
                }
            }
            Bson::Array(a) => {
                for (idx, v) in a.iter().enumerate() {
                    let child_path = format!("{}/{}", id_path, idx);
                    draw_tree_node(
                        ui,
                        doc_idx,
                        &child_path,
                        &format!("[{}]", idx),
                        v,
                        depth + 1,
                        false,
                        state,
                    );
                }
            }
            _ => {}
        }
    }

    root_clicked
}

/// 左中对齐绘制一行文本（用于在手动布局的树行上绘制 Key/Value/Type）。
fn tree_text(ui: &Ui, painter: &Painter, x: f32, center_y: f32, text: &str, color: Color32) {
    let style = ui.style();
    let fid = egui::TextStyle::Body.resolve(&style);
    let galley = ui
        .fonts(|f| f.layout_no_wrap(text.to_string(), fid, color));
    let pos = Pos2::new(x, center_y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}

/// 根据 BSON 值类型返回对应的图标 URI 名称（与 Robo3T `BsonTreeModel::getIcon` 一致：
/// ObjectId / RegEx / DBRef / Code 等统一复用 unsupported 图标）。
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

/// 文本/JSON 视图：黑灰底、浅色等宽文字（字符串绿色）、只读。
///
/// 自适应窗体宽度，**直接显示全部查询结果 JSON**（文档间逗号分隔），
/// 无需先在表格中选中某条记录再切换 JSON 模式（对齐 Robo3T Text 视图）。
pub fn text_view(ui: &mut Ui, state: &mut AppState) {
    let docs = state.documents.clone();
    let mut text = if docs.is_empty() {
        state.selected_doc_text.clone()
    } else {
        all_docs_text(&docs)
    };
    let avail = ui.available_width();
    Frame::none()
        .fill(TEXTBOX_BG)
        .rounding(Rounding::same(2.0))
        .inner_margin(Margin::same(4.0))
        .show(ui, |ui| {
            // TextEdit 自身内底色与深色 frame 保持一致
            ui.style_mut().visuals.extreme_bg_color = SCRIPT_BG;
            // 文本视图默认字体色 A1FCA2（#A1FCA2）；字符串高亮用 #1F9D5E
            let (fm_ranges, fm_current) = find::highlight_ranges(state, FindTarget::ResultText);
            let mut layouter = |ui: &Ui, t: &str, w: f32| {
                syntax::highlight_with_find(ui, t, w, TEXT_COLOR, TEXT_STR, &fm_ranges, fm_current)
            };
            ScrollArea::vertical().show(ui, |ui| {
                let edit = TextEdit::multiline(&mut text)
                    .code_editor()
                    .text_color(TEXT_COLOR)
                    .desired_width((avail - 12.0).max(100.0))
                    .layouter(&mut layouter)
                    .id(Id::new(find::EDIT_ID_RESULT));
                let output = edit.show(ui);
                find::after_show_scroll(ui, state, FindTarget::ResultText, &output);
                let ctx = ui.ctx().clone();
                find::handle_hotkey(&ctx, state, FindTarget::ResultText, &output.response);
            });
        });
}

/// 把全部查询结果文档拼为多行 JSON 文本（文档间以逗号分隔，含尾换行）。
pub fn all_docs_text(docs: &[Document]) -> String {
    let mut out = String::new();
    for d in docs {
        let t = rubo3t_core::bson_ext::document_to_text(d).unwrap_or_else(|_| "{}".to_string());
        out.push_str(&t);
        out.push_str(",\n");
    }
    // 去掉末尾多余的 ",\n"，保留一个换行
    if out.ends_with(",\n") {
        out.truncate(out.len() - 2);
        out.push('\n');
    }
    out
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
