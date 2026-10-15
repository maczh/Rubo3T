//! 弹窗：连接管理、连接设置、文档查看/编辑/插入、关于、测试、文件选择、Toast。
//!
//! 全部弹窗遵循规格第二部分（§11–§15）：
//! - egui::Window 作为模态窗口，Esc 关闭（`.open(&mut bool)` 的默认行为）。
//! - 主按钮白底浅边框（`primary_button`），Cancel 为普通浅灰按钮。
//! - 所有新增文案进入 i18n（ZH/EN）。

use eframe::egui;
use egui::{
    Align, Align2, Button, Color32, ComboBox, Context, DragValue, FontId, Frame, Id, Layout,
    Margin, Rounding, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Ui, UiBuilder, Vec2, Window,
};

use crate::app::{AppState, ConnDlgMode, ExplorerAction, FindTarget};
use crate::form::{ConnectionForm, MechChoice};
use crate::i18n::{trf, Lang};
use crate::icons;
use crate::ui::find;
use crate::ui::syntax;
use rubo3t_core::bson_ext;
use rubo3t_core::settings::{validate, ConnectionMode, SshAuthMethod};
use rubo3t_core::version::FeatureGate;
use rubo3t_core::Document;

/// Robo3T 蓝链颜色（#106CD6）。
const LINK_BLUE: Color32 = Color32::from_rgb(0x10, 0x6C, 0xD6);
/// 主按钮底色（白）。
const PRIMARY_FILL: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// 交替行底色（斑马纹）。
const ZEBRA: Color32 = Color32::from_gray(0xF4);
/// 选中行底色（浅蓝高亮）。
const SELECTED_BG: Color32 = Color32::from_rgb(0xCC, 0xE6, 0xFF);
/// 表头底色。
const HEAD_FILL: Color32 = Color32::from_gray(0xE8);
/// 灰色说明文字。
const HINT_GRAY: Color32 = Color32::from_gray(0x8C);
/// 4 列表格列宽（与表头保持一致以对齐）。
const COL0_W: f32 = 180.0;
const COL1_W: f32 = 188.0;
const COL2_W: f32 = 98.0;
const CONNECTION_ROW_HEIGHT: f32 = 22.0;

/// 文本视图配色（与 `result_view::text_view` 保持一致）：
/// 普通文字 A1FCA2、字符串高亮 #1F9D5E、文本框背景 #494C4E、编辑器内底色 #3D3F41。
const TEXT_COLOR: Color32 = Color32::from_rgb(0xA1, 0xFC, 0xA2);
const TEXT_STR: Color32 = Color32::from_rgb(0x1F, 0x9D, 0x5E);
const TEXTBOX_BG: Color32 = Color32::from_rgb(0x49, 0x4C, 0x4E);
const SCRIPT_BG: Color32 = Color32::from_rgb(0x3D, 0x3F, 0x41);

/// 主按钮浅边框。
fn primary_stroke() -> Stroke {
    Stroke::new(1.0_f32, Color32::from_gray(0xB4))
}

/// 弹窗总入口：在每帧最后渲染全部弹窗（盖住下方 UI，制造"模态"观感）。
pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    if state.conn_dlg_open {
        match state.conn_dlg_mode {
            ConnDlgMode::List => render_connections_dialog(ctx, state),
            ConnDlgMode::Edit => render_connection_settings_dialog(ctx, state),
        }
    }
    if state.view_doc_open {
        render_view_document(ctx, state);
    }
    if state.edit_doc_open {
        render_edit_document(ctx, state);
    }
    if state.insert_doc_open {
        render_insert_document(ctx, state);
    }
    if state.export_open {
        render_export_dialog(ctx, state);
    }
    if state.import_open {
        render_import_dialog(ctx, state);
    }
    if state.indexes_dlg_open {
        render_indexes_dialog(ctx, state);
    }
    if state.users_dlg_open {
        render_users_dialog(ctx, state);
    }
    if state.user_edit_open {
        render_edit_user_dialog(ctx, state);
    }
    if state.index_edit_open {
        render_edit_index_dialog(ctx, state);
    }
    if state.about_open {
        render_about_dialog(ctx, state);
    }
    if state.version_caps_open {
        render_version_caps_dialog(ctx, state);
    }
    if state.server_status_open {
        render_server_status_dialog(ctx, state);
    }
    if state.test_open {
        render_test_dialog(ctx, state);
    }
    if state.browse_open {
        render_browse_dialog(ctx, state);
    }

    // 新建集合输入框（由右键「新建集合...」触发）。
    if let Some(db) = state.create_coll_db.clone() {
        let mut name = state.create_coll_name.clone();
        let mut ok = false;
        let mut cancel = false;
        Window::new(lang.tr("explorer.create_coll_title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(lang.tr("explorer.coll_name"));
                ui.text_edit_singleline(&mut name);
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("common.cancel")).clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, None, lang.tr("common.create")) {
                        ok = true;
                    }
                });
            });
        if cancel {
            state.create_coll_db = None;
            state.create_coll_name.clear();
        }
        if ok {
            let n = name.trim().to_string();
            if !n.is_empty() {
                state.create_coll_name = n.clone();
                state.create_collection(&db, &n);
            }
            state.create_coll_db = None;
            state.create_coll_name.clear();
        }
    }

    // 删除集合 / 删除数据库确认框。
    if let Some(action) = state.confirm_action.clone() {
        let msg = match &action {
            ExplorerAction::DropCollection(db, coll) => trf(
                lang,
                "explorer.confirm_drop_coll",
                &[&format!("{}.{}", db, coll)],
            ),
            ExplorerAction::DropDatabase(db) => trf(lang, "explorer.confirm_drop_db", &[db]),
        };
        let mut ok = false;
        let mut cancel = false;
        Window::new(lang.tr("common.delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(msg);
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("common.cancel")).clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, None, lang.tr("common.delete")) {
                        ok = true;
                    }
                });
            });
        if cancel {
            state.confirm_action = None;
        }
        if ok {
            state.perform_explorer_action(action);
            state.confirm_action = None;
        }
    }

    // 重命名集合输入模态
    if let Some((db, old)) = state.rename_coll.clone() {
        let mut new_name = state.rename_coll_new.clone();
        let mut ok = false;
        let mut cancel = false;
        Window::new(lang.tr("explorer.rename_title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(lang.tr("explorer.new_name"));
                ui.text_edit_singleline(&mut new_name);
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("common.cancel")).clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, None, lang.tr("common.ok")) {
                        ok = true;
                    }
                });
            });
        if cancel {
            state.rename_coll = None;
            state.rename_coll_new.clear();
        }
        if ok {
            let n = new_name.trim().to_string();
            if !n.is_empty() && n != old {
                state.rename_coll_new = n.clone();
                state.rename_collection(&db, &old, &n);
            }
            state.rename_coll = None;
            state.rename_coll_new.clear();
        }
    }

    render_toast(ctx, state);
}

/// 主按钮：白底浅边，可选左侧图标（如 Connect 的 server 图标）。
fn primary_button(ui: &mut Ui, icon: Option<&str>, label: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        if let Some(name) = icon {
            ui.add(icons::img(name).fit_to_original_size(1.0));
        }
        let b = Button::new(label)
            .fill(PRIMARY_FILL)
            .stroke(primary_stroke());
        if ui.add(b).clicked() {
            clicked = true;
        }
    });
    clicked
}

/// 密码/口令框 + 「显示/隐藏」切换按钮（对应原版密码行右侧的眼睛图标）。
fn pw_row(ui: &mut Ui, label: &str, text: &mut String, show: &mut bool, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(label);
        let edit = TextEdit::singleline(text)
            .password(!*show)
            .desired_width(200.0);
        ui.add(edit);
        let tgl = if *show {
            lang.tr("form.hide")
        } else {
            lang.tr("form.show")
        };
        if ui.button(tgl).clicked() {
            *show = !*show;
        }
    });
}

// ---------------------------------------------------------------- §11 MongoDB Connections

/// 将文本按可用宽度截断，超出部分以 "..." 结尾（连接列表地址列超长时使用）。
fn truncate_with_dots(ui: &mut Ui, text: &str, max_width: f32) -> String {
    const ELLIPSIS: &str = "...";
    if max_width <= 1.0 {
        return ELLIPSIS.to_string();
    }
    let font = ui
        .style()
        .text_styles
        .get(&TextStyle::Body)
        .cloned()
        .unwrap_or_else(FontId::default);
    let width_of = |s: &str| -> f32 {
        ui.fonts(|f| {
            f.layout(s.to_string(), font.clone(), Color32::WHITE, f32::INFINITY)
                .size()
                .x
        })
    };
    if width_of(text) <= max_width {
        return text.to_string();
    }
    let ell_w = width_of(ELLIPSIS);
    if ell_w >= max_width {
        return ELLIPSIS.to_string();
    }
    let n = text.chars().count();
    let mut lo = 0usize;
    let mut hi = n;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = text.chars().take(mid).collect();
        if width_of(&cand) + ell_w <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = text.chars().take(lo).collect();
    out.push_str(ELLIPSIS);
    out
}

fn render_connections_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut selected = state.conn_dlg_selected;
    let mut new_sel: Option<usize> = None;
    let mut act_create = false;
    let mut act_edit = false;
    let mut act_clone = false;
    let mut act_remove = false;
    let mut act_connect_idx: Option<usize> = None;
    let mut act_cancel = false;
    let mut drag_release: Option<usize> = None;
    let mut row_rects = Vec::new();

    Window::new(lang.tr("dialog.conn_title"))
        .open(&mut state.conn_dlg_open)
        .resizable(false)
        .fixed_size(Vec2::new(654.0, 397.0))
        .show(ctx, |ui| {
            // 顶部蓝色链接说明行：Create / edit / remove / clone 为可点击链接。
            ui.style_mut().visuals.hyperlink_color = LINK_BLUE;
            ui.horizontal_wrapped(|ui| {
                if ui.link(lang.tr("dialog.conn_link_create")).clicked() {
                    act_create = true;
                }
                ui.label(", ");
                if ui.link(lang.tr("dialog.conn_link_edit")).clicked() {
                    act_edit = true;
                }
                ui.label(", ");
                if ui.link(lang.tr("dialog.conn_link_remove")).clicked() {
                    act_remove = true;
                }
                ui.label(", ");
                if ui.link(lang.tr("dialog.conn_link_clone")).clicked() {
                    act_clone = true;
                }
                ui.label(lang.tr("dialog.conn_link_tail"));
            });
            ui.separator();

            // 4 列表头
            Frame::none().fill(HEAD_FILL).show(ui, |ui| {
                ui.columns(4, |cols| {
                    cols[0].set_min_width(COL0_W);
                    cols[1].set_min_width(COL1_W);
                    cols[2].set_min_width(COL2_W);
                    cols[0].strong(lang.tr("dialog.conn_name"));
                    cols[1].strong(lang.tr("dialog.conn_address"));
                    cols[2].strong(lang.tr("dialog.conn_attr"));
                    cols[3].strong(lang.tr("dialog.conn_auth"));
                });
            });
            ui.separator();

            // 表格体
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(280.0)
                .show(ui, |ui| {
                    for (i, conn) in state.connections.iter().enumerate() {
                        let is_selected = selected == Some(i);
                        let zebra = i % 2 == 1;
                        let bg = if is_selected {
                            SELECTED_BG
                        } else if zebra {
                            ZEBRA
                        } else {
                            Color32::TRANSPARENT
                        };

                        let icon = match conn.mode {
                            ConnectionMode::ReplicaSet => "replica_set_16x16",
                            _ => "server_16x16",
                        };
                        let addr = if conn.mode == ConnectionMode::ReplicaSet {
                            let first = conn
                                .hosts
                                .first()
                                .map(|h| h.to_string())
                                .unwrap_or_default();
                            format!("{} node(s) ({})", conn.hosts.len(), first)
                        } else {
                            conn.hosts
                                .first()
                                .map(|h| h.to_string())
                                .unwrap_or_default()
                        };
                        let mut attrs: Vec<&str> = Vec::new();
                        if conn.mode == ConnectionMode::ReplicaSet {
                            attrs.push(lang.tr("attr.replica"));
                        }
                        if conn.ssl.enabled {
                            attrs.push(lang.tr("attr.tls"));
                        }
                        if conn.ssh.enabled {
                            attrs.push(lang.tr("attr.ssh"));
                        }

                        let (row_rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), CONNECTION_ROW_HEIGHT),
                            Sense::hover(),
                        );
                        row_rects.push(row_rect);
                        ui.painter().rect_filled(row_rect, 0.0, bg);
                        let mut row_ui = ui.new_child(
                            UiBuilder::new()
                                .max_rect(row_rect)
                                .layout(Layout::left_to_right(Align::Center)),
                        );
                        row_ui.columns(4, |cols| {
                            cols[0].set_min_width(COL0_W);
                            cols[1].set_min_width(COL1_W);
                            cols[2].set_min_width(COL2_W);
                            cols[0].horizontal(|ui| {
                                ui.add(icons::img(icon).fit_to_original_size(1.0));
                                ui.label(&conn.name);
                            });
                            let addr_max = cols[1].available_width();
                            let addr_disp = truncate_with_dots(&mut cols[1], &addr, addr_max);
                            cols[1].label(&addr_disp);
                            cols[2].label(attrs.join(", "));
                            if conn.credential.enabled {
                                cols[3].horizontal(|ui| {
                                    ui.add(icons::img("key_16x16").fit_to_original_size(1.0));
                                    ui.label(format!(
                                        "{} / {}",
                                        conn.credential.auth_source, conn.credential.user_name
                                    ));
                                });
                            }
                        });
                        let resp = ui.interact(
                            row_rect,
                            ui.id().with(("connection_row", i)),
                            Sense::click_and_drag(),
                        );

                        if resp.clicked() {
                            new_sel = Some(i);
                        }
                        if resp.double_clicked() {
                            act_connect_idx = Some(i);
                        }
                        if resp.drag_stopped() {
                            drag_release = Some(i);
                        }
                        resp.context_menu(|ui| {
                            if ui.button(lang.tr("dialog.conn_create")).clicked() {
                                act_create = true;
                                ui.close_menu();
                            }
                            if ui.button(lang.tr("dialog.conn_edit")).clicked() {
                                act_edit = true;
                                new_sel = Some(i);
                                ui.close_menu();
                            }
                            if ui.button(lang.tr("dialog.conn_clone")).clicked() {
                                act_clone = true;
                                new_sel = Some(i);
                                ui.close_menu();
                            }
                            if ui.button(lang.tr("dialog.conn_remove")).clicked() {
                                act_remove = true;
                                new_sel = Some(i);
                                ui.close_menu();
                            }
                        });
                    }
                });
            ui.separator();

            // 底部按钮盒：右侧 [Connect(主按钮)][Cancel]
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lang.tr("dialog.conn_cancel")).clicked() {
                        act_cancel = true;
                    }
                    if primary_button(ui, Some("server_16x16"), lang.tr("dialog.conn_connect")) {
                        act_connect_idx = act_connect_idx.or(selected);
                    }
                });
            });
        });

    if let Some(from) = drag_release {
        if let Some(pointer) = ctx.input(|input| input.pointer.interact_pos()) {
            if let Some(to) = row_rects.iter().position(|rect| rect.contains(pointer)) {
                if from != to && from < state.connections.len() && to < state.connections.len() {
                    let connection = state.connections.remove(from);
                    state.connections.insert(to, connection);
                    selected = selected.map(|index| {
                        if index == from {
                            to
                        } else if from < to && (from + 1..=to).contains(&index) {
                            index - 1
                        } else if to < from && (to..from).contains(&index) {
                            index + 1
                        } else {
                            index
                        }
                    });
                    state.save_connections();
                }
            }
        }
    }
    if let Some(i) = new_sel {
        selected = Some(i);
    }
    if act_create {
        state.conn_dlg = ConnectionForm::default();
        state.conn_dlg_selected = None;
        state.conn_dlg_mode = ConnDlgMode::Edit;
    }
    if act_edit {
        if let Some(i) = selected {
            state.conn_dlg = ConnectionForm::from_settings(&state.connections[i]);
            state.conn_dlg_mode = ConnDlgMode::Edit;
        }
    }
    if act_clone {
        if let Some(i) = selected {
            let mut cloned = state.connections[i].clone();
            cloned.name = format!("{}{}", lang.tr("dialog.conn_clone_prefix"), cloned.name);
            state.conn_dlg = ConnectionForm::from_settings(&cloned);
            state.conn_dlg_selected = None;
            state.conn_dlg_mode = ConnDlgMode::Edit;
        }
    }
    if act_remove {
        if let Some(i) = selected {
            state.confirm_remove_idx = Some(i);
        }
    }
    if let Some(i) = act_connect_idx {
        if i < state.connections.len() {
            let s = state.connections[i].clone();
            state.connect(s);
            state.conn_dlg_open = false;
        }
    }
    if act_cancel {
        state.conn_dlg_open = false;
    }
    // 新建/克隆连接时 conn_dlg_selected 应为 None（见 act_create / act_clone 块），
    // 此处不可回写为 selected，否则保存时会覆盖原连接。
    if !(act_create || act_clone) {
        state.conn_dlg_selected = selected;
    }

    // Remove 确认框
    if let Some(idx) = state.confirm_remove_idx {
        if idx < state.connections.len() {
            let name = state.connections[idx].name.clone();
            let mut ok = false;
            let mut cancel = false;
            Window::new(lang.tr("dialog.confirm_remove_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(trf(lang, "dialog.conn_confirm_remove", &[&name]));
                    ui.horizontal(|ui| {
                        if ui.button(lang.tr("common.cancel")).clicked() {
                            cancel = true;
                        }
                        if primary_button(ui, None, lang.tr("common.ok")) {
                            ok = true;
                        }
                    });
                });
            if ok {
                state.connections.remove(idx);
                state.save_connections();
                state.conn_dlg_selected = state.conn_dlg_selected.and_then(|selected| {
                    if selected == idx {
                        None
                    } else if selected > idx {
                        Some(selected - 1)
                    } else {
                        Some(selected)
                    }
                });
                state.toast(&format!("{}: {}", lang.tr("common.delete"), name), false);
                state.confirm_remove_idx = None;
            }
            if cancel {
                state.confirm_remove_idx = None;
            }
        } else {
            state.confirm_remove_idx = None;
        }
    }
}

// ---------------------------------------------------------------- §12 Connection Settings

fn render_connection_settings_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut back_to_list = false;
    let mut act_save = false;
    let mut act_test = false;
    let mut dlg = state.conn_dlg.clone();
    let mut pending_browse: Option<(u8, String)> = None;

    Window::new(lang.tr("settings.title"))
        .open(&mut state.conn_dlg_open)
        .resizable(true)
        .default_width(560.0)
        .min_width(500.0)
        .show(ctx, |ui| {
            // 自绘 5-tab 条（顺序固定：Connection/Authentication/SSH/TLS/Advanced）
            ui.horizontal(|ui| {
                let tabs = [
                    lang.tr("settings.tab_connection"),
                    lang.tr("settings.tab_auth"),
                    lang.tr("settings.tab_ssh"),
                    lang.tr("settings.tab_tls"),
                    lang.tr("settings.tab_advanced"),
                ];
                for (ti, label) in tabs.iter().enumerate() {
                    let active = dlg.tab_index == ti;
                    if ui.selectable_label(active, *label).clicked() {
                        dlg.tab_index = ti;
                    }
                }
            });
            ui.separator();

            ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                let auth_gate = state.service.as_ref().and_then(|s| s.feature_gate());
                pending_browse = match dlg.tab_index {
                    0 => render_connection_tab(ui, &mut dlg, lang),
                    1 => render_auth_tab(ui, &mut dlg, lang, auth_gate),
                    2 => render_ssh_tab(ui, &mut dlg, lang),
                    3 => render_tls_tab(ui, &mut dlg, lang),
                    _ => render_advanced_tab(ui, &mut dlg, lang),
                };
            });
            ui.separator();

            // 底部：左 Test，右 [Save][Cancel]
            ui.horizontal(|ui| {
                if ui.button(lang.tr("settings.test")).clicked() {
                    act_test = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lang.tr("common.cancel")).clicked() {
                        back_to_list = true;
                    }
                    if primary_button(ui, None, lang.tr("common.save")) {
                        act_save = true;
                    }
                });
            });
        });

    if let Some((t, p)) = pending_browse {
        state.browse_open = true;
        state.browse_target = t;
        state.browse_path = p;
    }
    state.conn_dlg = dlg;

    if back_to_list {
        state.conn_dlg_mode = ConnDlgMode::List;
    }
    if act_save {
        let s = state.conn_dlg.to_settings();
        if validate(&s).is_ok() {
            if let Some(i) = state.conn_dlg_selected {
                state.connections[i] = s;
            } else {
                state.connections.push(s);
            }
            state.save_connections();
            state.toast(lang.tr("msg.save_new_ok"), false);
            state.conn_dlg_mode = ConnDlgMode::List;
        } else {
            state.toast(
                &format!(
                    "{} ({})",
                    lang.tr("msg.save_failed"),
                    lang.tr("msg.check_name_host")
                ),
                true,
            );
        }
    }
    if act_test {
        let s = state.conn_dlg.to_settings();
        state.test_connection(s);
    }
}

/// 返回 `Some((browse_target, 当前路径))` 表示用户点击了某个 Browse 按钮。
fn render_connection_tab(
    ui: &mut Ui,
    dlg: &mut ConnectionForm,
    lang: Lang,
) -> Option<(u8, String)> {
    // 各组件上下行间隔不小于 8px
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(12.0);
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.type"));
        let mut mode = dlg.mode;
        ComboBox::from_label("")
            .selected_text(match mode {
                ConnectionMode::Standalone => lang.tr("form.mode.standalone"),
                ConnectionMode::ReplicaSet => lang.tr("form.mode.replica"),
                ConnectionMode::ShardCluster => lang.tr("form.mode.shard"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut mode,
                    ConnectionMode::Standalone,
                    lang.tr("form.mode.standalone"),
                );
                ui.selectable_value(
                    &mut mode,
                    ConnectionMode::ReplicaSet,
                    lang.tr("form.mode.replica"),
                );
            });
        dlg.mode = mode;
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.name"));
        ui.add(TextEdit::singleline(&mut dlg.name).desired_width(240.0));
    });
    if dlg.mode == ConnectionMode::ReplicaSet {
        ui.label(lang.tr("form.members"));
        let mut lines: Vec<String> = dlg.hosts_text.lines().map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push("127.0.0.1:27017".to_string());
        }
        let mut remove_idx: Option<usize> = None;
        for (i, line) in lines.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(line).desired_width(220.0));
                if ui.button("−").clicked() {
                    remove_idx = Some(i);
                }
            });
        }
        if ui.button("＋").clicked() {
            lines.push("127.0.0.1:27017".to_string());
        }
        if let Some(i) = remove_idx {
            lines.remove(i);
        }
        dlg.hosts_text = lines.join("\n");
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.replica_set"));
            ui.add(TextEdit::singleline(&mut dlg.replica_set).desired_width(200.0));
        });
    } else {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.address"));
            ui.add(TextEdit::singleline(&mut dlg.host_input).desired_width(180.0));
            ui.label(":");
            ui.add(TextEdit::singleline(&mut dlg.port_input).desired_width(60.0));
        });
        ui.colored_label(HINT_GRAY, lang.tr("form.hint.address"));
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(lang.tr("settings.from_uri")).clicked() {
            let uri = dlg.uri_text.clone();
            dlg.apply_uri(&uri);
        }
        ui.add(TextEdit::singleline(&mut dlg.uri_text).desired_width(320.0));
        ui.label(lang.tr("settings.uri_placeholder"));
    });
    None
}

fn render_auth_tab(
    ui: &mut Ui,
    dlg: &mut ConnectionForm,
    lang: Lang,
    gate: Option<FeatureGate>,
) -> Option<(u8, String)> {
    // 各组件上下行间隔不小于 8px
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(12.0);
    ui.checkbox(&mut dlg.auth_enabled, lang.tr("form.auth.perform"));
    if !dlg.auth_enabled {
        return None;
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.auth_db"));
        ui.add(TextEdit::singleline(&mut dlg.auth_source).desired_width(160.0));
    });
    ui.colored_label(HINT_GRAY, lang.tr("form.auth.db_hint"));
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.user"));
        ui.add(TextEdit::singleline(&mut dlg.user).desired_width(200.0));
    });
    pw_row(
        ui,
        lang.tr("form.password"),
        &mut dlg.password,
        &mut dlg.show_auth_password,
        lang,
    );
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.mechanism"));
        let mut mech = dlg.mechanism;
        ComboBox::from_label("")
            .selected_text(mech.tr(lang))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut mech,
                    MechChoice::ScramSha1,
                    MechChoice::ScramSha1.tr(lang),
                );
                ui.selectable_value(
                    &mut mech,
                    MechChoice::ScramSha256,
                    MechChoice::ScramSha256.tr(lang),
                );
                ui.selectable_value(
                    &mut mech,
                    MechChoice::MongoDbCr,
                    MechChoice::MongoDbCr.tr(lang),
                );
            });
        dlg.mechanism = mech;
    });
    ui.label(lang.tr("form.mechanism.tip"));

    // 版本感知警告：依据已连接服务器的 FeatureGate 提示该认证机制是否可用。
    let warn: Option<&'static str> = match dlg.mechanism {
        MechChoice::MongoDbCr => {
            if let Some(g) = gate {
                if !g.supports_mongodb_cr() {
                    Some(lang.tr("auth.warn.cr_removed"))
                } else {
                    None
                }
            } else {
                None
            }
        }
        MechChoice::ScramSha256 => {
            if let Some(g) = gate {
                if !g.supports_scram_sha256() {
                    Some(lang.tr("auth.warn.sha256_min"))
                } else {
                    None
                }
            } else {
                None
            }
        }
        MechChoice::ScramSha1 => {
            if let Some(g) = gate {
                if !g.supports_sessions() {
                    Some(lang.tr("auth.warn.sha1_min"))
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(warn_text) = warn {
        ui.colored_label(Color32::from_rgb(0xB0, 0x6A, 0x00), warn_text);
    }

    ui.separator();
    ui.checkbox(
        &mut dlg.auth_visible_dbs_enabled,
        lang.tr("form.auth.visible_dbs"),
    );
    if dlg.auth_visible_dbs_enabled {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.auth.visible_dbs_label"));
            ui.add(
                TextEdit::singleline(&mut dlg.manually_visible_dbs)
                    .desired_width(240.0)
                    .hint_text(lang.tr("form.auth.visible_dbs_placeholder")),
            );
        });
        ui.colored_label(HINT_GRAY, lang.tr("form.auth.visible_dbs_hint"));
    }
    None
}

fn render_ssh_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    // 各组件上下行间隔不小于 8px
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(12.0);
    ui.checkbox(&mut dlg.ssh_enabled, lang.tr("form.ssh.use_tunnel"));
    if !dlg.ssh_enabled {
        return None;
    }
    if dlg.mode == ConnectionMode::ReplicaSet {
        ui.colored_label(HINT_GRAY, lang.tr("form.ssh.disabled_hint"));
        ui.disable();
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.address"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_host).desired_width(180.0));
        ui.label(":");
        ui.add(DragValue::new(&mut dlg.ssh_port).range(1..=65535));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.user_name"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_user).desired_width(160.0));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.auth_method"));
        let mut auth = dlg.ssh_auth;
        ComboBox::from_label("")
            .selected_text(match auth {
                SshAuthMethod::Password => lang.tr("ssh.password"),
                SshAuthMethod::PublicKey => lang.tr("ssh.key"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut auth, SshAuthMethod::Password, lang.tr("ssh.password"));
                ui.selectable_value(&mut auth, SshAuthMethod::PublicKey, lang.tr("ssh.key"));
            });
        dlg.ssh_auth = auth;
    });
    let mut req: Option<(u8, String)> = None;
    match dlg.ssh_auth {
        SshAuthMethod::Password => {
            pw_row(
                ui,
                lang.tr("form.ssh.user_password"),
                &mut dlg.ssh_password,
                &mut dlg.show_ssh_password,
                lang,
            );
        }
        SshAuthMethod::PublicKey => {
            let key = dlg.ssh_key.clone();
            ui.horizontal(|ui| {
                ui.label(lang.tr("form.ssh.private_key"));
                ui.add(TextEdit::singleline(&mut dlg.ssh_key).desired_width(220.0));
                if ui.button(lang.tr("common.browse")).clicked() {
                    req = Some((3, key));
                }
            });
            ui.colored_label(HINT_GRAY, lang.tr("form.ssh.private_key_placeholder"));
        }
    }
    pw_row(
        ui,
        lang.tr("form.ssh.passphrase"),
        &mut dlg.ssh_passphrase,
        &mut dlg.show_ssh_passphrase,
        lang,
    );
    let ask_label = match dlg.ssh_auth {
        SshAuthMethod::Password => lang.tr("form.ssh.ask_password"),
        SshAuthMethod::PublicKey => lang.tr("form.ssh.ask_passphrase"),
    };
    ui.checkbox(&mut dlg.ssh_ask_password_each_time, ask_label);
    req
}

fn render_tls_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    // 各组件上下行间隔不小于 8px
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(12.0);
    ui.checkbox(&mut dlg.ssl_enabled, lang.tr("form.tls.use"));
    if !dlg.ssl_enabled {
        return None;
    }
    let mut req: Option<(u8, String)> = None;

    ui.horizontal(|ui| {
        ui.label(lang.tr("form.tls.auth_method"));
        let mut self_signed = dlg.ssl_allow_invalid;
        ComboBox::from_label("")
            .selected_text(if self_signed {
                lang.tr("form.tls.auth_self")
            } else {
                lang.tr("form.tls.auth_ca")
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self_signed, true, lang.tr("form.tls.auth_self"));
                ui.selectable_value(&mut self_signed, false, lang.tr("form.tls.auth_ca"));
            });
        dlg.ssl_allow_invalid = self_signed;
    });
    if dlg.ssl_allow_invalid {
        ui.colored_label(HINT_GRAY, lang.tr("form.tls.self_signed_info"));
    } else {
        let ca = dlg.ssl_ca_file.clone();
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.tls.ca_file"));
            ui.add(TextEdit::singleline(&mut dlg.ssl_ca_file).desired_width(220.0));
            if ui.button(lang.tr("common.browse")).clicked() {
                req = Some((0, ca));
            }
        });
    }

    ui.checkbox(&mut dlg.ssl_use_pem, lang.tr("form.tls.use_pem"));
    if !dlg.ssl_use_pem {
        ui.colored_label(HINT_GRAY, lang.tr("form.tls.pem_info"));
    } else {
        let cert = dlg.ssl_cert_file.clone();
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.tls.pem_file"));
            ui.add(TextEdit::singleline(&mut dlg.ssl_cert_file).desired_width(220.0));
            if ui.button(lang.tr("common.browse")).clicked() {
                req = Some((1, cert));
            }
        });
        pw_row(
            ui,
            lang.tr("form.tls.pem_passphrase"),
            &mut dlg.ssl_pem_passphrase,
            &mut dlg.show_pem_passphrase,
            lang,
        );
        ui.checkbox(
            &mut dlg.ssl_ask_pem_passphrase,
            lang.tr("form.tls.ask_pem_pass"),
        );
    }

    ui.checkbox(&mut dlg.ssl_advanced_options, lang.tr("form.tls.advanced"));
    if dlg.ssl_advanced_options {
        let crl = dlg.ssl_crl_file.clone();
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.tls.crl"));
            ui.add(TextEdit::singleline(&mut dlg.ssl_crl_file).desired_width(220.0));
            if ui.button(lang.tr("common.browse")).clicked() {
                req = Some((2, crl));
            }
        });
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.tls.invalid_hostnames"));
            let mut allowed = dlg.ssl_allow_invalid_hostnames;
            ComboBox::from_label("")
                .selected_text(if allowed {
                    lang.tr("form.tls.invalid_allowed")
                } else {
                    lang.tr("form.tls.invalid_not_allowed")
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut allowed,
                        false,
                        lang.tr("form.tls.invalid_not_allowed"),
                    );
                    ui.selectable_value(&mut allowed, true, lang.tr("form.tls.invalid_allowed"));
                });
            dlg.ssl_allow_invalid_hostnames = allowed;
        });
    }
    req
}

fn render_advanced_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    // 各组件上下行间隔不小于 8px
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(12.0);
    ui.horizontal(|ui| {
        ui.label(lang.tr("settings.default_db"));
        ui.add(TextEdit::singleline(&mut dlg.default_db).desired_width(200.0));
    });
    ui.colored_label(HINT_GRAY, lang.tr("settings.default_db_hint"));
    None
}

// ---------------------------------------------------------------- §13 文档编辑 / 插入

/// 深色多行编辑器：配色与 `result_view::text_view` 完全一致（#494C4E 背景、
/// 普通文字 A1FCA2、字符串高亮 #1F9D5E），ScrollArea 自适应宽度。
/// 编辑光标为 #00C000 并以约 2Hz 频率闪烁（聚焦时持续重绘驱动）。
fn dark_editor(ui: &mut Ui, state: &mut AppState) {
    let avail = ui.available_width();
    let ctx = ui.ctx().clone();
    // 每 0.5s 在 #00C000 与透明之间切换，形成闪烁
    let blink_on = (ctx.input(|i| i.time) * 2.0).floor() % 2.0 == 0.0;
    let cursor_color = if blink_on {
        Color32::from_rgb(0x00, 0xC0, 0x00)
    } else {
        Color32::TRANSPARENT
    };
    Frame::none()
        .fill(TEXTBOX_BG)
        .rounding(Rounding::same(2.0))
        .inner_margin(Margin::same(4.0))
        .show(ui, |ui| {
            // 编辑器内底色与 text_view 一致
            ui.style_mut().visuals.extreme_bg_color = SCRIPT_BG;
            // 编辑光标设为 #00C000（闪烁由下方 request_repaint 驱动）
            ui.style_mut().visuals.text_cursor.stroke = Stroke::new(2.0_f32, cursor_color);
            // 普通文字 A1FCA2，字符串高亮 #1F9D5E（与 text_view 相同）
            let (fm_ranges, fm_current) = find::highlight_ranges(state, FindTarget::EditDoc);
            let mut layouter = |ui: &Ui, t: &str, w: f32| {
                syntax::highlight_with_find(ui, t, w, TEXT_COLOR, TEXT_STR, &fm_ranges, fm_current)
            };
            let resp = ScrollArea::vertical().show(ui, |ui| {
                let edit = TextEdit::multiline(&mut state.selected_doc_text)
                    .code_editor()
                    .text_color(TEXT_COLOR)
                    .desired_width((avail - 12.0).max(100.0))
                    .layouter(&mut layouter)
                    .id(Id::new(find::EDIT_ID_EDITDOC));
                let output = edit.show(ui);
                find::after_show_scroll(ui, state, FindTarget::EditDoc, &output);
                // Ctrl/Cmd+F 打开文本查找弹窗
                let fctx = ui.ctx().clone();
                find::handle_hotkey(&fctx, state, FindTarget::EditDoc, &output.response);
                output.response
            });
            // 聚焦时持续重绘，保持光标闪烁
            if resp.inner.has_focus() {
                ctx.request_repaint();
            }
        });
}

/// 校验当前编辑器文本是否为合法 JSON/BSON 文档；非法则弹 Error。
fn validate_document(state: &mut AppState, text: &str) {
    let lang = state.lang;
    match bson_ext::parse_document_text(text) {
        Ok(_) => state.toast(lang.tr("doc.validate_ok"), false),
        Err(e) => state.toast(&format!("{}: {}", lang.tr("common.error"), e), true),
    }
}

/// 编辑文档弹窗：深色编辑器 + Validate + [Save][Cancel]（Save 走 save_document()）。
fn render_edit_document(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let title = state.edit_doc_title.clone();
    let mut save = false;
    let mut cancel = false;
    let mut validate = false;
    let w = ctx.screen_rect().width() * 0.7;
    let h = ctx.screen_rect().height() * 0.7;
    let mut edit_open = state.edit_doc_open;
    Window::new(title)
        .open(&mut edit_open)
        .resizable(true)
        .default_size([w, h])
        .show(ctx, |ui| {
            dark_editor(ui, state);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("doc.validate")).clicked() {
                    validate = true;
                }
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.save")) {
                    save = true;
                }
            });
        });
    state.edit_doc_open = edit_open;
    if validate {
        let txt = state.selected_doc_text.clone();
        validate_document(state, &txt);
    }
    if save {
        let txt = state.selected_doc_text.clone();
        match bson_ext::parse_document_text(&txt) {
            Ok(_) => {
                state.save_document();
                state.edit_doc_open = false;
            }
            Err(e) => {
                state.toast(&format!("{}: {}", lang.tr("common.error"), e), true);
            }
        }
    }
    if cancel {
        state.edit_doc_open = false;
    }
}

/// 插入文档弹窗：深色编辑器 + Validate + [Save][Cancel]（Save 走 insert_document()）。
fn render_insert_document(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut save = false;
    let mut cancel = false;
    let mut validate = false;
    let w = ctx.screen_rect().width() * 0.7;
    let h = ctx.screen_rect().height() * 0.7;
    let mut insert_open = state.insert_doc_open;
    Window::new(lang.tr("doc.insert_title"))
        .open(&mut insert_open)
        .resizable(true)
        .default_size([w, h])
        .show(ctx, |ui| {
            dark_editor(ui, state);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("doc.validate")).clicked() {
                    validate = true;
                }
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.save")) {
                    save = true;
                }
            });
        });
    state.insert_doc_open = insert_open;
    if validate {
        let txt = state.selected_doc_text.clone();
        validate_document(state, &txt);
    }
    if save {
        let txt = state.selected_doc_text.clone();
        match bson_ext::parse_document_text(&txt) {
            Ok(_) => {
                state.insert_document();
                state.insert_doc_open = false;
            }
            Err(e) => {
                state.toast(&format!("{}: {}", lang.tr("common.error"), e), true);
            }
        }
    }
    if cancel {
        state.insert_doc_open = false;
    }
}

/// 导出文档对话框：文件路径输入 + 保存（单个或全部结果）。
fn render_export_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut save = false;
    let mut cancel = false;
    Window::new(lang.tr("doc.export_title"))
        .open(&mut state.export_open)
        .resizable(true)
        .default_size([480.0, 180.0])
        .show(ctx, |ui| {
            let count = match state.export_doc_index {
                Some(i) => state.documents.get(i).map(|_| 1).unwrap_or(0),
                None => state.documents.len(),
            };
            ui.label(format!("{}: {}", lang.tr("doc.export_count"), count));
            ui.label(lang.tr("doc.export_path"));
            ui.text_edit_singleline(&mut state.export_path);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.save")) {
                    save = true;
                }
            });
        });
    if cancel {
        state.export_open = false;
    }
    if save {
        let path = state.export_path.clone();
        let docs: Vec<Document> = match state.export_doc_index {
            Some(i) => state.documents.get(i).cloned().into_iter().collect(),
            None => state.documents.clone(),
        };
        state.export_documents(&path, &docs);
        state.export_open = false;
    }
}

/// 导入文档对话框：文件路径输入 + 导入（数组或单个文档对象）。
fn render_import_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut import = false;
    let mut cancel = false;
    Window::new(lang.tr("doc.import_title"))
        .open(&mut state.import_open)
        .resizable(true)
        .default_size([480.0, 180.0])
        .show(ctx, |ui| {
            ui.label(lang.tr("doc.import_path"));
            ui.text_edit_singleline(&mut state.import_path);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.import")) {
                    import = true;
                }
            });
        });
    if cancel {
        state.import_open = false;
    }
    if import {
        let path = state.import_path.clone();
        state.import_documents(&path);
        state.import_open = false;
    }
}

/// 查看文档弹窗（只读）：配色与 `result_view::text_view` 一致（#494C4E 背景、
/// 普通文字 A1FCA2、字符串高亮 #1F9D5E），ScrollArea 自适应宽度，对话框默认主窗体 70%。
fn render_view_document(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let title = state.view_doc_title.clone();
    let mut close = false;
    let w = ctx.screen_rect().width() * 0.7;
    let h = ctx.screen_rect().height() * 0.7;
    let mut view_open = state.view_doc_open;
    Window::new(title)
        .open(&mut view_open)
        .resizable(true)
        .default_size([w, h])
        .show(ctx, |ui| {
            let avail = ui.available_width();
            let mut text = state.view_doc_text.clone();
            Frame::none()
                .fill(TEXTBOX_BG)
                .rounding(Rounding::same(2.0))
                .inner_margin(Margin::same(4.0))
                .show(ui, |ui| {
                    ui.style_mut().visuals.extreme_bg_color = SCRIPT_BG;
                    let (fm_ranges, fm_current) =
                        find::highlight_ranges(state, FindTarget::ViewDoc);
                    let mut layouter = |ui: &Ui, t: &str, w: f32| {
                        syntax::highlight_with_find(
                            ui, t, w, TEXT_COLOR, TEXT_STR, &fm_ranges, fm_current,
                        )
                    };
                    ScrollArea::vertical().show(ui, |ui| {
                        let edit = TextEdit::multiline(&mut text)
                            .code_editor()
                            .text_color(TEXT_COLOR)
                            .desired_width((avail - 12.0).max(100.0))
                            .layouter(&mut layouter)
                            .id(Id::new(find::EDIT_ID_VIEWDOC));
                        let output = edit.show(ui);
                        find::after_show_scroll(ui, state, FindTarget::ViewDoc, &output);
                        // Ctrl/Cmd+F 打开文本查找弹窗
                        let fctx = ui.ctx().clone();
                        find::handle_hotkey(&fctx, state, FindTarget::ViewDoc, &output.response);
                    });
                });
            state.view_doc_text = text;
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.close")).clicked() {
                    close = true;
                }
            });
        });
    state.view_doc_open = view_open;
    if close {
        state.view_doc_open = false;
    }
}

// ---------------------------------------------------------------- §14 About

fn render_about_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut ok = false;
    Window::new(lang.tr("about.title"))
        .open(&mut state.about_open)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add(icons::img("logo").max_size(Vec2::new(96.0, 96.0)));
                ui.heading("Rubo3T");
                ui.label("1.4.4");
                ui.label(lang.tr("about.copyright"));
            });
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.ok")).clicked() {
                    ok = true;
                }
            });
        });
    if ok {
        state.about_open = false;
    }
}

// ---------------------------------------------------------------- 版本能力自适应 UI

/// 版本能力弹窗：展示当前服务器的版本能力矩阵与默认认证机制（差异化增值项）。
///
/// 复用 `FeatureGate` 能力矩阵，把 MongoDB 3.2→最新版本的自动适配在界面上呈现。
fn render_version_caps_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    // 先取出 owned 数据，规避 `state` 借用冲突。
    let info = state
        .service
        .as_ref()
        .and_then(|s| s.server_info())
        .cloned();
    if let Some(info) = info {
        let version = info.version;
        let edition = info.edition.clone();
        let host = info.host.clone();
        let is_cluster = info.is_cluster;
        let gate = FeatureGate::new(version);
        let summary = gate.summary();
        let default_auth = gate.default_auth_mechanism().to_string();
        let active_name = state.active_name.clone();

        let mut close = false;
        Window::new(lang.tr("caps.title"))
            .open(&mut state.version_caps_open)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.columns(2, |cols| {
                    cols[0].strong(lang.tr("caps.server_label"));
                    cols[1].label(&active_name);
                    cols[0].strong(lang.tr("caps.version_label"));
                    cols[1].label(version.to_string());
                    cols[0].strong(lang.tr("caps.edition_label"));
                    cols[1].label(if edition.is_empty() {
                        "(unknown)".to_string()
                    } else {
                        edition.clone()
                    });
                    cols[0].strong(lang.tr("caps.host_label"));
                    cols[1].label(&host);
                    cols[0].strong(lang.tr("caps.topology_label"));
                    cols[1].label(if is_cluster {
                        lang.tr("caps.cluster").to_string()
                    } else {
                        lang.tr("caps.standalone").to_string()
                    });
                    cols[0].strong(lang.tr("caps.default_auth_label"));
                    cols[1].strong(default_auth.clone());
                });

                ui.separator();
                ui.heading(lang.tr("caps.matrix_label"));
                ui.columns(3, |cols| {
                    for cap in &summary {
                        cols[0].label(cap.name);
                        cols[1].label(format!("{}: {}", lang.tr("caps.since_label"), cap.since));
                        if cap.supported {
                            cols[2].colored_label(
                                Color32::from_rgb(0x2e, 0x7d, 0x32),
                                format!("✓ {}", lang.tr("caps.supported")),
                            );
                        } else {
                            cols[2].colored_label(
                                Color32::from_rgb(0x9e, 0x9e, 0x9e),
                                format!("✗ {}", lang.tr("caps.unsupported")),
                            );
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("common.ok")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            state.version_caps_open = false;
        }
    } else {
        let mut close = false;
        Window::new(lang.tr("caps.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(lang.tr("caps.not_connected"));
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("common.ok")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            state.version_caps_open = false;
        }
    }
}

/// 服务器状态对话框：概览 / 连接 / 内存 / 操作计数 / 断言 / 主机 / 副本集拓扑 / 诊断。
fn render_server_status_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut close = false;
    let gate = state.service.as_ref().and_then(|s| s.feature_gate());
    Window::new(lang.tr("server_status.title"))
        .open(&mut state.server_status_open)
        .resizable(true)
        .default_size([680.0, 560.0])
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                let ss = state.server_status_doc.clone();
                if ss.is_none() {
                    ui.label(lang.tr("server_status.loading"));
                }
                if let Some(ss) = &ss {
                    // ---- 概览 ----
                    ui.heading(lang.tr("server_status.overview"));
                    if let Ok(v) = ss.get_str("version") {
                        ui.label(format!("{}: {}", lang.tr("server_status.version"), v));
                    }
                    let uptime = ss.get_f64("uptime").unwrap_or(0.0);
                    ui.label(format!(
                        "{}: {:.0}",
                        lang.tr("server_status.uptime"),
                        uptime
                    ));
                    ui.separator();

                    // ---- 连接 ----
                    ui.heading(lang.tr("server_status.connections"));
                    if let Ok(c) = ss.get_document("connections") {
                        let cur = c.get_i64("current").unwrap_or(0);
                        let avail = c.get_i64("available").unwrap_or(0);
                        ui.label(format!("{} / {}", cur, avail));
                    }
                    ui.separator();

                    // ---- 内存 ----
                    ui.heading(lang.tr("server_status.memory"));
                    if let Ok(m) = ss.get_document("mem") {
                        let resident = m.get_i64("resident").unwrap_or(0);
                        let virt = m.get_i64("virtual").unwrap_or(0);
                        ui.label(format!(
                            "{}: {} MB  /  {} MB",
                            resident,
                            virt,
                            lang.tr("server_status.memory")
                        ));
                    }
                    ui.separator();

                    // ---- 操作计数 ----
                    ui.heading(lang.tr("server_status.opcounters"));
                    if let Ok(o) = ss.get_document("opcounters") {
                        for k in ["insert", "query", "update", "delete", "getmore", "command"] {
                            let v = o.get_i64(k).unwrap_or(0);
                            ui.label(format!("{}: {}", k, v));
                        }
                    }
                    ui.separator();

                    // ---- 断言 ----
                    ui.heading(lang.tr("server_status.asserts"));
                    if let Ok(a) = ss.get_document("asserts") {
                        for k in ["regular", "warning", "msg", "user", "rollovers"] {
                            let v = a.get_i64(k).unwrap_or(0);
                            ui.label(format!("{}: {}", k, v));
                        }
                    }
                    ui.separator();
                }

                // ---- 主机信息 ----
                if let Some(hi) = &state.host_info_doc {
                    ui.heading(lang.tr("server_status.host"));
                    if let Ok(sys) = hi.get_document("system") {
                        if let Ok(v) = sys.get_i64("numCores") {
                            ui.label(format!("numCores: {}", v));
                        }
                        if let Ok(v) = sys.get_i64("memSizeMB") {
                            ui.label(format!("memSizeMB: {}", v));
                        }
                    }
                    ui.separator();
                }

                // ---- 副本集拓扑 ----
                ui.heading(lang.tr("server_status.replset"));
                match &state.repl_status_doc {
                    Some(rs) => {
                        if let Ok(members) = rs.get_array("members") {
                            for m in members {
                                if let Some(doc) = m.as_document() {
                                    let name = doc.get_str("name").unwrap_or_default().to_string();
                                    let state_str =
                                        doc.get_str("stateStr").unwrap_or_default().to_string();
                                    let health = doc.get_f64("health").unwrap_or(0.0);
                                    ui.label(format!(
                                        "{}  [{}]  health={}",
                                        name, state_str, health
                                    ));
                                }
                            }
                        } else if let Ok(set) = rs.get_str("set") {
                            ui.label(format!("replica set: {} (无 members 字段)", set));
                        } else {
                            ui.label(lang.tr("server_status.standalone_or_err"));
                        }
                    }
                    None => {
                        ui.label(lang.tr("server_status.standalone_or_err"));
                    }
                }
                ui.separator();

                // ---- 诊断 / 版本能力 ----
                ui.heading(lang.tr("server_status.diagnostics"));
                if let Some(g) = gate {
                    for cap in g.summary() {
                        let mark = if cap.supported {
                            format!("✓ {}", lang.tr("caps.supported"))
                        } else {
                            format!("✗ {}", lang.tr("caps.unsupported"))
                        };
                        ui.label(format!(
                            "{}  ({} {})  {}",
                            cap.name,
                            lang.tr("caps.since_label"),
                            cap.since,
                            mark
                        ));
                    }
                } else {
                    ui.label(lang.tr("caps.not_connected"));
                }
            });
            if ui.button(lang.tr("common.close")).clicked() {
                close = true;
            }
        });
    if close {
        state.server_status_open = false;
    }
}

// ---------------------------------------------------------------- §15 测试诊断 / 文件选择 / Toast

/// 连接测试诊断弹窗（§12 Test 按钮触发）。
fn render_test_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut ok = false;
    Window::new(lang.tr("test.title"))
        .open(&mut state.test_open)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(&state.test_result);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.ok")).clicked() {
                    ok = true;
                }
            });
        });
    if ok {
        state.test_open = false;
    }
}

/// 文件选择弹窗：因未引入文件对话框 crate，用自定义路径输入窗口代替（Browse 按钮触发）。
fn render_browse_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut ok = false;
    let mut cancel = false;
    Window::new(lang.tr("common.browse"))
        .open(&mut state.browse_open)
        .resizable(true)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(lang.tr("common.path"));
            ui.text_edit_singleline(&mut state.browse_path);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.ok")) {
                    ok = true;
                }
            });
        });
    if cancel {
        state.browse_open = false;
    }
    if ok {
        let p = state.browse_path.clone();
        match state.browse_target {
            0 => state.conn_dlg.ssl_ca_file = p,
            1 => state.conn_dlg.ssl_cert_file = p,
            2 => state.conn_dlg.ssl_key_file = p,
            3 => state.conn_dlg.ssh_key = p,
            _ => {}
        }
        state.browse_open = false;
    }
}

// ---------------------------------------------------------------- 索引 / 用户

/// 索引管理弹窗：列出已有索引，并支持新建 / 删除（可写）。
fn render_indexes_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.indexes_dlg_db.clone();
    let coll = state.indexes_dlg_coll.clone();
    let mut open = state.indexes_dlg_open;
    let mut close_requested = false;
    Window::new(format!("{} - {}.{}", lang.tr("indexes.caption"), db, coll))
        .open(&mut open)
        .resizable(true)
        .default_size([480.0, 380.0])
        .show(ctx, |ui| {
            if let Some(info) = &state.server_info {
                ui.label(format!(
                    "{} {}  {}",
                    lang.tr("caps.version_label"),
                    info.version,
                    info.edition
                ));
            }
            ui.horizontal(|ui| {
                ui.heading(lang.tr("index.existing"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lang.tr("index.create")).clicked() {
                        state.index_create_open = !state.index_create_open;
                    }
                });
            });
            if state.index_create_open {
                Frame::none().fill(ZEBRA).show(ui, |ui| {
                    ui.label(lang.tr("index.field"));
                    ui.text_edit_singleline(&mut state.index_create_field);
                    ui.label(lang.tr("index.field_hint"));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.index_create_unique, lang.tr("index.unique"));
                        ui.label(lang.tr("index.name"));
                        ui.text_edit_singleline(&mut state.index_create_name);
                    });
                    ui.horizontal(|ui| {
                        if ui.button(lang.tr("common.cancel")).clicked() {
                            state.index_create_open = false;
                            state.index_create_field.clear();
                            state.index_create_name.clear();
                            state.index_create_unique = false;
                        }
                        if primary_button(ui, None, lang.tr("common.create")) {
                            let f = state.index_create_field.clone();
                            let n = state.index_create_name.clone();
                            let uq = state.index_create_unique;
                            state.create_index(&db, &coll, &f, &n, uq);
                            state.index_create_open = false;
                            state.index_create_field.clear();
                            state.index_create_name.clear();
                            state.index_create_unique = false;
                        }
                    });
                });
            }
            ui.separator();
            let indexes = state.indexes.clone();
            let mut drop_name: Option<String> = None;
            let mut edit_name: Option<String> = None;
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                if indexes.is_empty() {
                    ui.label(lang.tr("index.empty"));
                } else {
                    for idx in &indexes {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{}  keys={} {}",
                                idx.name,
                                serde_json::to_string(&idx.keys).unwrap_or_default(),
                                if idx.unique { "[唯一]" } else { "" }
                            ));
                            if ui.button(lang.tr("index.edit")).clicked() {
                                edit_name = Some(idx.name.clone());
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(lang.tr("common.delete")).clicked() {
                                    drop_name = Some(idx.name.clone());
                                }
                            });
                        });
                    }
                }
            });
            if let Some(name) = drop_name {
                state.drop_index(&db, &coll, &name);
            }
            if let Some(old) = edit_name.take() {
                if let Some(idx) = state.indexes.iter().find(|i| i.name == old) {
                    state.index_edit_open = true;
                    state.index_edit_old_name = idx.name.clone();
                    state.index_edit_field = serde_json::to_string(&idx.keys).unwrap_or_default();
                    state.index_edit_name = idx.name.clone();
                    state.index_edit_unique = idx.unique;
                }
            }
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.close")).clicked() {
                    close_requested = true;
                }
            });
        });
    if close_requested {
        open = false;
    }
    state.indexes_dlg_open = open;
}

/// 用户管理弹窗：列出已有用户，并支持新建 / 删除（可写）。
fn render_users_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.users_dlg_db.clone();
    let mut open = state.users_dlg_open;
    let mut close_requested = false;
    Window::new(format!("{} - {}", lang.tr("users.caption"), db))
        .open(&mut open)
        .resizable(true)
        .default_size([480.0, 380.0])
        .show(ctx, |ui| {
            if let Some(info) = &state.server_info {
                ui.label(format!(
                    "{} {}  {}",
                    lang.tr("caps.version_label"),
                    info.version,
                    info.edition
                ));
            }
            ui.horizontal(|ui| {
                ui.heading(lang.tr("users.caption"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lang.tr("user.create")).clicked() {
                        state.user_create_open = !state.user_create_open;
                    }
                });
            });
            if state.user_create_open {
                Frame::none().fill(ZEBRA).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(lang.tr("user.name_label"));
                        ui.text_edit_singleline(&mut state.user_create_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(lang.tr("user.pwd"));
                        ui.text_edit_singleline(&mut state.user_create_pwd);
                    });
                    ui.label(lang.tr("user.roles_hint"));
                    ui.text_edit_singleline(&mut state.user_create_roles);
                    ui.horizontal(|ui| {
                        if ui.button(lang.tr("common.cancel")).clicked() {
                            state.user_create_open = false;
                            state.user_create_name.clear();
                            state.user_create_pwd.clear();
                            state.user_create_roles.clear();
                        }
                        if primary_button(ui, None, lang.tr("common.create")) {
                            let n = state.user_create_name.clone();
                            let p = state.user_create_pwd.clone();
                            let r = state.user_create_roles.clone();
                            state.create_user(&db, &n, &p, &r);
                            state.user_create_open = false;
                            state.user_create_name.clear();
                            state.user_create_pwd.clear();
                            state.user_create_roles.clear();
                        }
                    });
                });
            }
            ui.separator();
            let users = state.users.clone();
            let mut drop_name: Option<String> = None;
            let mut edit_name: Option<String> = None;
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                if users.is_empty() {
                    ui.label(lang.tr("user.empty"));
                } else {
                    for u in &users {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{}  {} {}",
                                u.name,
                                lang.tr("user.roles"),
                                u.roles.join(", ")
                            ));
                            if ui.button(lang.tr("user.edit")).clicked() {
                                edit_name = Some(u.name.clone());
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(lang.tr("common.delete")).clicked() {
                                    drop_name = Some(u.name.clone());
                                }
                            });
                        });
                    }
                }
            });
            if let Some(name) = drop_name {
                state.drop_user(&db, &name);
            }
            if let Some(name) = edit_name.take() {
                if let Some(u) = state.users.iter().find(|x| x.name == name) {
                    state.user_edit_open = true;
                    state.user_edit_name = u.name.clone();
                    state.user_edit_roles = u.roles.join(", ");
                    state.user_edit_pwd.clear();
                }
            }
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.close")).clicked() {
                    close_requested = true;
                }
            });
        });
    if close_requested {
        open = false;
    }
    state.users_dlg_open = open;
}

/// 编辑用户弹窗：名称只读，角色可改，密码可选（留空不改）。
fn render_edit_user_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.users_dlg_db.clone();
    let mut save = false;
    let mut cancel = false;
    Window::new(lang.tr("user.edit_title"))
        .open(&mut state.user_edit_open)
        .resizable(true)
        .default_size([420.0, 260.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(lang.tr("user.name_label"));
                ui.label(state.user_edit_name.clone());
            });
            ui.horizontal(|ui| {
                ui.label(lang.tr("user.pwd_optional"));
                ui.text_edit_singleline(&mut state.user_edit_pwd);
            });
            ui.label(lang.tr("user.roles_hint"));
            ui.text_edit_singleline(&mut state.user_edit_roles);
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.save")) {
                    save = true;
                }
            });
        });
    if cancel {
        state.user_edit_open = false;
    }
    if save {
        let n = state.user_edit_name.clone();
        let p = state.user_edit_pwd.clone();
        let r = state.user_edit_roles.clone();
        state.edit_user(&db, &n, &p, &r);
        state.user_edit_open = false;
    }
}

/// 编辑索引弹窗：keys 以 JSON 编辑；保存走「删旧 + 建新」。
fn render_edit_index_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.indexes_dlg_db.clone();
    let coll = state.indexes_dlg_coll.clone();
    let mut save = false;
    let mut cancel = false;
    Window::new(lang.tr("index.edit_title"))
        .open(&mut state.index_edit_open)
        .resizable(true)
        .default_size([480.0, 320.0])
        .show(ctx, |ui| {
            ui.label(lang.tr("index.field"));
            ui.text_edit_singleline(&mut state.index_edit_field);
            ui.label(lang.tr("index.field_hint"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.index_edit_unique, lang.tr("index.unique"));
                ui.label(lang.tr("index.name"));
                ui.text_edit_singleline(&mut state.index_edit_name);
            });
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, None, lang.tr("common.save")) {
                    save = true;
                }
            });
        });
    if cancel {
        state.index_edit_open = false;
    }
    if save {
        let old = state.index_edit_old_name.clone();
        let f = state.index_edit_field.clone();
        let n = state.index_edit_name.clone();
        let uq = state.index_edit_unique;
        state.edit_index(&db, &coll, &old, &f, &n, uq);
        state.index_edit_open = false;
    }
}

// ---------------------------------------------------------------- Toast

fn render_toast(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let toast = state.toast_msg.clone();
    if let Some((msg, is_err)) = toast {
        let cleared = Window::new(lang.tr("toast.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.colored_label(if is_err { Color32::RED } else { Color32::GREEN }, &msg);
                ui.button(lang.tr("common.ok")).clicked()
            });
        if cleared.and_then(|r| r.inner).unwrap_or(false) {
            state.toast_msg = None;
        }
    }
}
