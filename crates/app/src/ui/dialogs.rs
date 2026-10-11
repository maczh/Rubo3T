//! 弹窗：连接管理、连接设置、文档查看/编辑/插入、关于、测试、文件选择、Toast。
//!
//! 全部弹窗遵循规格第二部分（§11–§15）：
//! - egui::Window 作为模态窗口，Esc 关闭（`.open(&mut bool)` 的默认行为）。
//! - 主按钮白底浅边框（`primary_button`），Cancel 为普通浅灰按钮。
//! - 所有新增文案进入 i18n（ZH/EN）。

use eframe::egui;
use egui::{
    Align, Align2, Button, Color32, ComboBox, Context, DragValue, Frame, Layout, Margin, Rounding,
    ScrollArea, Stroke, TextEdit, Ui, Vec2, Window,
};

use crate::app::{AppState, ConnDlgMode};
use crate::form::{ConnectionForm, MechChoice};
use crate::i18n::{trf, Lang};
use crate::icons;
use rubo3t_core::bson_ext;
use rubo3t_core::settings::{validate, ConnectionMode, SshAuthMethod};
use rubo3t_core::version::FeatureGate;

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
const COL0_W: f32 = 220.0;
const COL1_W: f32 = 170.0;
const COL2_W: f32 = 150.0;

/// 主按钮浅边框。
fn primary_stroke() -> Stroke {
    Stroke::new(1.0_f32, Color32::from_gray(0xB4))
}

/// 弹窗总入口：在每帧最后渲染全部弹窗（盖住下方 UI，制造"模态"观感）。
pub fn render(ctx: &Context, state: &mut AppState) {
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
    if state.indexes_dlg_open {
        render_indexes_dialog(ctx, state);
    }
    if state.users_dlg_open {
        render_users_dialog(ctx, state);
    }
    if state.about_open {
        render_about_dialog(ctx, state);
    }
    if state.version_caps_open {
        render_version_caps_dialog(ctx, state);
    }
    if state.test_open {
        render_test_dialog(ctx, state);
    }
    if state.browse_open {
        render_browse_dialog(ctx, state);
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
        let b = Button::new(label).fill(PRIMARY_FILL).stroke(primary_stroke());
        if ui.add(b).clicked() {
            clicked = true;
        }
    });
    clicked
}

// ---------------------------------------------------------------- §11 MongoDB Connections

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

    Window::new(lang.tr("dialog.conn_title"))
        .open(&mut state.conn_dlg_open)
        .resizable(true)
        .default_size(Vec2::new(630.0, 290.0))
        .min_size(Vec2::new(630.0, 290.0))
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
            ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
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

                    let resp = Frame::none().fill(bg).show(ui, |ui| {
                        ui.columns(4, |cols| {
                            cols[0].set_min_width(COL0_W);
                            cols[1].set_min_width(COL1_W);
                            cols[2].set_min_width(COL2_W);
                            cols[0].horizontal(|ui| {
                                ui.add(icons::img(icon).fit_to_original_size(1.0));
                                ui.label(&conn.name);
                            });
                            cols[1].label(&addr);
                            cols[2].label(attrs.join(", "));
                            if conn.credential.enabled {
                                cols[3].horizontal(|ui| {
                                    ui.label(format!(
                                        "{} / {}",
                                        conn.credential.auth_source, conn.credential.user_name
                                    ));
                                    ui.add(icons::img("key_16x16").fit_to_original_size(1.0));
                                });
                            }
                        });
                    })
                    .response;

                    if resp.clicked() {
                        new_sel = Some(i);
                    }
                    if resp.double_clicked() {
                        act_connect_idx = Some(i);
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
    state.conn_dlg_selected = selected;

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
                if state.conn_dlg_selected == Some(idx) {
                    state.conn_dlg_selected = None;
                }
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
fn render_connection_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
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
                ui.selectable_value(
                    &mut mode,
                    ConnectionMode::ShardCluster,
                    lang.tr("form.mode.shard"),
                );
            });
        dlg.mode = mode;
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.name"));
        ui.add(TextEdit::singleline(&mut dlg.name).desired_width(240.0));
    });
    ui.colored_label(HINT_GRAY, lang.tr("form.hint.name"));
    if dlg.mode == ConnectionMode::Standalone {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.address"));
            ui.add(TextEdit::singleline(&mut dlg.host_input).desired_width(180.0));
            ui.label(":");
            ui.add(TextEdit::singleline(&mut dlg.port_input).desired_width(60.0));
        });
        ui.colored_label(HINT_GRAY, lang.tr("form.hint.address"));
    } else {
        ui.label(lang.tr("form.hosts"));
        ui.add(
            TextEdit::multiline(&mut dlg.hosts_text)
                .code_editor()
                .desired_rows(3)
                .desired_width(300.0),
        );
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.replica_set"));
            ui.add(TextEdit::singleline(&mut dlg.replica_set).desired_width(200.0));
        });
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut dlg.use_srv, lang.tr("form.use_srv"));
        ui.checkbox(&mut dlg.direct, lang.tr("form.direct"));
    });
    None
}

fn render_auth_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang, gate: Option<FeatureGate>) -> Option<(u8, String)> {
    ui.checkbox(&mut dlg.auth_enabled, lang.tr("form.auth.perform"));
    if !dlg.auth_enabled {
        return None;
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.auth_db"));
        ui.add(TextEdit::singleline(&mut dlg.auth_source).desired_width(120.0));
        ui.label(lang.tr("form.user"));
        ui.add(TextEdit::singleline(&mut dlg.user).desired_width(120.0));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.password"));
        ui.add(TextEdit::singleline(&mut dlg.password).desired_width(160.0));
        ui.label(lang.tr("form.mechanism"));
        let mut mech = dlg.mechanism;
        ComboBox::from_label("")
            .selected_text(mech.tr(lang))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mech, MechChoice::Auto, MechChoice::Auto.tr(lang));
                ui.selectable_value(
                    &mut mech,
                    MechChoice::MongoDbCr,
                    MechChoice::MongoDbCr.tr(lang),
                );
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
                ui.selectable_value(&mut mech, MechChoice::Plain, MechChoice::Plain.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::X509, MechChoice::X509.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::GssApi, MechChoice::GssApi.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::AwsIam, MechChoice::AwsIam.tr(lang));
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
    None
}

fn render_ssh_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    ui.checkbox(&mut dlg.ssh_enabled, lang.tr("form.ssh.use_tunnel"));
    if !dlg.ssh_enabled {
        return None;
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.address"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_host).desired_width(180.0));
        ui.label(":");
        ui.add(DragValue::new(&mut dlg.ssh_port).range(1..=65535));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.user"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_user).desired_width(140.0));
        let mut auth = dlg.ssh_auth;
        ComboBox::from_label(lang.tr("form.ssh.auth"))
            .selected_text(match auth {
                SshAuthMethod::Password => lang.tr("ssh.password"),
                SshAuthMethod::PublicKey => lang.tr("ssh.key"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut auth, SshAuthMethod::PublicKey, lang.tr("ssh.key"));
                ui.selectable_value(&mut auth, SshAuthMethod::Password, lang.tr("ssh.password"));
            });
        dlg.ssh_auth = auth;
    });
    let mut req: Option<(u8, String)> = None;
    match dlg.ssh_auth {
        SshAuthMethod::PublicKey => {
            let key = dlg.ssh_key.clone();
            ui.horizontal(|ui| {
                ui.label(lang.tr("form.ssh.private_key"));
                ui.add(TextEdit::singleline(&mut dlg.ssh_key).desired_width(220.0));
                if ui.button(lang.tr("common.browse")).clicked() {
                    req = Some((3, key));
                }
            });
        }
        SshAuthMethod::Password => {
            ui.horizontal(|ui| {
                ui.label(lang.tr("form.password"));
                ui.add(TextEdit::singleline(&mut dlg.ssh_password).desired_width(220.0));
            });
        }
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.passphrase"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_passphrase).desired_width(220.0));
    });
    req
}

fn render_tls_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    ui.checkbox(&mut dlg.ssl_enabled, lang.tr("form.tls.use"));
    if !dlg.ssl_enabled {
        return None;
    }
    let mut validate_cert = !dlg.ssl_allow_invalid;
    ui.checkbox(&mut validate_cert, lang.tr("form.tls.validate"));
    dlg.ssl_allow_invalid = !validate_cert;

    let mut req: Option<(u8, String)> = None;
    let ca = dlg.ssl_ca_file.clone();
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.tls.ca_file"));
        ui.add(TextEdit::singleline(&mut dlg.ssl_ca_file).desired_width(220.0));
        if ui.button(lang.tr("common.browse")).clicked() {
            req = Some((0, ca));
        }
    });
    let cert = dlg.ssl_cert_file.clone();
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.tls.cert_file"));
        ui.add(TextEdit::singleline(&mut dlg.ssl_cert_file).desired_width(220.0));
        if ui.button(lang.tr("common.browse")).clicked() {
            req = Some((1, cert));
        }
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.tls.key_file"));
        ui.add(TextEdit::singleline(&mut dlg.ssl_key_file).desired_width(220.0));
    });
    req
}

fn render_advanced_tab(ui: &mut Ui, dlg: &mut ConnectionForm, lang: Lang) -> Option<(u8, String)> {
    ui.horizontal(|ui| {
        ui.label(lang.tr("settings.default_db"));
        ui.add(TextEdit::singleline(&mut dlg.default_db).desired_width(200.0));
    });
    ui.horizontal(|ui| {
        if ui.button(lang.tr("settings.from_uri")).clicked() {
            // URI 导入占位（保持现有字段结构，不新增数据模型）。
        }
        ui.label(lang.tr("settings.uri_placeholder"));
    });
    None
}

// ---------------------------------------------------------------- §13 文档编辑 / 插入

/// 深色多行编辑器（与脚本编辑器一致：#494C4E 背景 + 浅色文字）。
fn dark_editor(ui: &mut Ui, text: &mut String) {
    let bg = Color32::from_rgb(0x49, 0x4C, 0x4E);
    let fg = Color32::from_rgb(0xE6, 0xE6, 0xE6);
    Frame::none()
        .fill(bg)
        .rounding(Rounding::same(2.0))
        .inner_margin(Margin::same(4.0))
        .show(ui, |ui| {
            ui.style_mut().visuals.override_text_color = Some(fg);
            ScrollArea::vertical().show(ui, |ui| {
                ui.add(TextEdit::multiline(text).code_editor().desired_rows(18));
            });
        });
}

/// 校验当前编辑器文本是否为合法 JSON/BSON 文档；非法则弹 Error。
fn validate_document(state: &mut AppState, text: &str) {
    let lang = state.lang;
    match bson_ext::parse_document_text(text) {
        Ok(_) => state.toast(lang.tr("doc.validate_ok"), false),
        Err(e) => state.toast(
            &format!("{}: {}", lang.tr("common.error"), e.to_string()),
            true,
        ),
    }
}

/// 编辑文档弹窗：深色编辑器 + Validate + [Save][Cancel]（Save 走 save_document()）。
fn render_edit_document(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let title = state.edit_doc_title.clone();
    let mut save = false;
    let mut cancel = false;
    let mut validate = false;
    Window::new(title)
        .open(&mut state.edit_doc_open)
        .resizable(true)
        .default_size([600.0, 500.0])
        .show(ctx, |ui| {
            dark_editor(ui, &mut state.selected_doc_text);
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
                state.toast(
                    &format!("{}: {}", lang.tr("common.error"), e.to_string()),
                    true,
                );
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
    Window::new(lang.tr("doc.insert_title"))
        .open(&mut state.insert_doc_open)
        .resizable(true)
        .default_size([600.0, 500.0])
        .show(ctx, |ui| {
            dark_editor(ui, &mut state.selected_doc_text);
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
                state.toast(
                    &format!("{}: {}", lang.tr("common.error"), e.to_string()),
                    true,
                );
            }
        }
    }
    if cancel {
        state.insert_doc_open = false;
    }
}

/// 查看文档弹窗（只读）。
fn render_view_document(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let title = state.view_doc_title.clone();
    let mut close = false;
    Window::new(title)
        .open(&mut state.view_doc_open)
        .resizable(true)
        .default_size([600.0, 500.0])
        .show(ctx, |ui| {
            let mut text = state.view_doc_text.clone();
            ScrollArea::vertical().show(ui, |ui| {
                ui.add(TextEdit::multiline(&mut text).code_editor().desired_rows(24));
            });
            state.view_doc_text = text;
            ui.horizontal(|ui| {
                if ui.button(lang.tr("common.close")).clicked() {
                    close = true;
                }
            });
        });
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
    let info = state.service.as_ref().and_then(|s| s.server_info()).cloned();
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
                        cols[1].label(format!(
                            "{}: {}",
                            lang.tr("caps.since_label"),
                            cap.since
                        ));
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

/// 索引只读弹窗。
fn render_indexes_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.indexes_dlg_db.clone();
    let mut open = state.indexes_dlg_open;
    let mut close_requested = false;
    Window::new(format!("{} - {}", lang.tr("indexes.caption"), db))
        .open(&mut open)
        .resizable(true)
        .default_size([480.0, 360.0])
        .show(ctx, |ui| {
            if state.indexes.is_empty() {
                ui.label(lang.tr("index.empty"));
            } else {
                for idx in &state.indexes {
                    ui.label(format!(
                        "• {}  keys={} {}",
                        idx.name,
                        serde_json::to_string(&idx.keys).unwrap_or_default(),
                        if idx.unique { "[唯一]" } else { "" }
                    ));
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

/// 用户只读弹窗。
fn render_users_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let db = state.users_dlg_db.clone();
    let mut open = state.users_dlg_open;
    let mut close_requested = false;
    Window::new(format!("{} - {}", lang.tr("users.caption"), db))
        .open(&mut open)
        .resizable(true)
        .default_size([480.0, 360.0])
        .show(ctx, |ui| {
            if state.users.is_empty() {
                ui.label(lang.tr("user.empty"));
            } else {
                for u in &state.users {
                    ui.label(format!(
                        "• {} {} {}",
                        u.name,
                        lang.tr("user.roles"),
                        u.roles.join(", ")
                    ));
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
                ui.colored_label(
                    if is_err { Color32::RED } else { Color32::GREEN },
                    &msg,
                );
                ui.button(lang.tr("common.ok")).clicked()
            });
        if cleared.and_then(|r| r.inner).unwrap_or(false) {
            state.toast_msg = None;
        }
    }
}
