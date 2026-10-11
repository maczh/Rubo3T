//! 弹窗：连接管理、连接设置、文档查看、关于、Toast。

use eframe::egui;
use egui::{Align2, CollapsingHeader, Color32, ComboBox, Context, DragValue, ScrollArea, TextEdit, Vec2, Window};

use crate::app::{AppState, ConnDlgMode};
use crate::form::{ConnectionForm, MechChoice};
use crate::i18n::Lang;
use crate::icons;
use rubo3t_core::settings::{validate, ConnectionMode, SshAuthMethod};

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
    render_toast(ctx, state);
}

// ---------------------------------------------------------------- MongoDB Connections

fn render_connections_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut selected = state.conn_dlg_selected;
    let mut act_create = false;
    let mut act_edit = false;
    let mut act_remove = false;
    let mut act_clone = false;
    let mut act_connect = false;
    let mut act_cancel = false;
    let mut new_sel: Option<usize> = None;

    Window::new(lang.tr("dialog.conn_title"))
        .open(&mut state.conn_dlg_open)
        .resizable(true)
        .default_width(700.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(lang.tr("dialog.conn_create")).clicked() { act_create = true; }
                ui.label("/");
                if ui.button(lang.tr("dialog.conn_edit")).clicked() { act_edit = true; }
                ui.label("/");
                if ui.button(lang.tr("dialog.conn_remove")).clicked() { act_remove = true; }
                ui.label("/");
                if ui.button(lang.tr("dialog.conn_clone")).clicked() { act_clone = true; }
                ui.label(lang.tr("dialog.conn_reorder"));
            });
            ui.separator();
            // 表头
            ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                ui.columns(4, |cols| {
                    cols[0].label(lang.tr("dialog.conn_name"));
                    cols[1].label(lang.tr("dialog.conn_address"));
                    cols[2].label(lang.tr("dialog.conn_attr"));
                    cols[3].label(lang.tr("dialog.conn_auth"));
                });
            });
            ui.separator();
            ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                for (i, conn) in state.connections.iter().enumerate() {
                    let is_selected = selected == Some(i);
                    // 每行前加 MongoDB 图标
                    let response = ui
                        .horizontal(|ui| {
                            ui.add(icons::img("mongodb_16x16").fit_to_original_size(1.0));
                            ui.selectable_label(is_selected, conn.name.clone())
                        })
                        .inner;
                    if response.clicked() {
                        new_sel = Some(i);
                    }
                    response.context_menu(|ui| {
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
            ui.horizontal(|ui| {
                if ui.button(lang.tr("dialog.conn_create")).clicked() { act_create = true; }
                if selected.is_some() {
                    if ui.button(lang.tr("dialog.conn_edit")).clicked() { act_edit = true; }
                    if ui.button(lang.tr("dialog.conn_remove")).clicked() { act_remove = true; }
                    if ui.button(lang.tr("dialog.conn_clone")).clicked() { act_clone = true; }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(lang.tr("dialog.conn_cancel")).clicked() {
                        act_cancel = true;
                    }
                    if ui.button(lang.tr("dialog.conn_connect")).clicked() {
                        act_connect = true;
                    }
                });
            });
        });

    if let Some(i) = new_sel {
        selected = Some(i);
        state.conn_dlg_selected = selected;
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
            cloned.name = format!("{} {}", lang.tr("dialog.conn_clone"), cloned.name);
            state.conn_dlg = ConnectionForm::from_settings(&cloned);
            state.conn_dlg_selected = None;
            state.conn_dlg_mode = ConnDlgMode::Edit;
        }
    }
    if act_remove {
        if let Some(i) = selected {
            let name = state.connections[i].name.clone();
            state.connections.remove(i);
            state.save_connections();
            selected = None;
            state.toast(&format!("{}: {}", lang.tr("common.delete"), name), false);
        }
    }
    if act_connect {
        if let Some(i) = selected {
            let s = state.connections[i].clone();
            state.connect(s);
            state.conn_dlg_open = false;
        }
    }
    if act_cancel {
        state.conn_dlg_open = false;
    }

    state.conn_dlg_selected = selected;
}

// ---------------------------------------------------------------- Connection Settings

fn render_connection_settings_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut back_to_list = false;
    let mut act_save = false;
    let mut act_test = false;

    Window::new(lang.tr("settings.title"))
        .open(&mut state.conn_dlg_open)
        .resizable(true)
        .default_width(640.0)
        .show(ctx, |ui| {
            let mut dlg = state.conn_dlg.clone();
            ui.horizontal(|ui| {
                if ui.button(format!("< {}", lang.tr("dialog.conn_title"))).clicked() {
                    back_to_list = true;
                }
            });
            ui.separator();
            ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                render_form_tabs(&mut dlg, ui, lang);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(lang.tr("settings.test")).clicked() {
                    act_test = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(lang.tr("common.cancel")).clicked() {
                        back_to_list = true;
                    }
                    if ui.button(lang.tr("common.save")).clicked() {
                        act_save = true;
                    }
                });
            });
            state.conn_dlg = dlg;
        });

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
            state.toast(&format!("{} ({})", lang.tr("msg.save_failed"), lang.tr("msg.check_name_host")), true);
        }
    }
    if act_test {
        let s = state.conn_dlg.to_settings();
        state.connect(s);
    }
}

fn render_form_tabs(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    // 使用 collapsing headers 模拟标签页
    CollapsingHeader::new(lang.tr("settings.tab_connection")).default_open(true).show(ui, |ui| {
        render_connection_tab(dlg, ui, lang);
    });
    CollapsingHeader::new(lang.tr("settings.tab_auth")).show(ui, |ui| {
        render_auth_tab(dlg, ui, lang);
    });
    CollapsingHeader::new(lang.tr("settings.tab_ssh")).show(ui, |ui| {
        render_ssh_tab(dlg, ui, lang);
    });
    CollapsingHeader::new(lang.tr("settings.tab_tls")).show(ui, |ui| {
        render_tls_tab(dlg, ui, lang);
    });
    CollapsingHeader::new(lang.tr("settings.tab_advanced")).show(ui, |ui| {
        render_advanced_tab(dlg, ui, lang);
    });
}

fn render_connection_tab(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
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
                ui.selectable_value(&mut mode, ConnectionMode::Standalone, lang.tr("form.mode.standalone"));
                ui.selectable_value(&mut mode, ConnectionMode::ReplicaSet, lang.tr("form.mode.replica"));
                ui.selectable_value(&mut mode, ConnectionMode::ShardCluster, lang.tr("form.mode.shard"));
            });
        dlg.mode = mode;
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.name"));
        ui.add(TextEdit::singleline(&mut dlg.name).desired_width(240.0));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.hosts"));
        ui.add(TextEdit::multiline(&mut dlg.hosts_text).code_editor().desired_rows(3).desired_width(300.0));
    });
    if dlg.mode == ConnectionMode::ReplicaSet {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.replica_set"));
            ui.add(TextEdit::singleline(&mut dlg.replica_set));
        });
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut dlg.use_srv, lang.tr("form.use_srv"));
        ui.checkbox(&mut dlg.direct, lang.tr("form.direct"));
    });
}

fn render_auth_tab(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    ui.checkbox(&mut dlg.auth_enabled, lang.tr("form.auth.enable"));
    if !dlg.auth_enabled {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.user"));
        ui.add(TextEdit::singleline(&mut dlg.user));
        ui.label(lang.tr("form.password"));
        ui.add(TextEdit::singleline(&mut dlg.password));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.auth_db"));
        ui.add(TextEdit::singleline(&mut dlg.auth_source).desired_width(120.0));
        ui.label(lang.tr("form.mechanism"));
        let mut mech = dlg.mechanism;
        ComboBox::from_label("")
            .selected_text(mech.tr(lang))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mech, MechChoice::Auto, MechChoice::Auto.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::MongoDbCr, MechChoice::MongoDbCr.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::ScramSha1, MechChoice::ScramSha1.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::ScramSha256, MechChoice::ScramSha256.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::Plain, MechChoice::Plain.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::X509, MechChoice::X509.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::GssApi, MechChoice::GssApi.tr(lang));
                ui.selectable_value(&mut mech, MechChoice::AwsIam, MechChoice::AwsIam.tr(lang));
            });
        dlg.mechanism = mech;
    });
    ui.label(lang.tr("form.mechanism.tip"));
}

fn render_ssh_tab(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    ui.checkbox(&mut dlg.ssh_enabled, lang.tr("form.ssh.enable"));
    if !dlg.ssh_enabled {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.bastion"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_host));
        ui.label(lang.tr("common.port"));
        ui.add(DragValue::new(&mut dlg.ssh_port).range(1..=65535));
    });
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.user"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_user));
        let mut auth = dlg.ssh_auth;
        ComboBox::from_label(lang.tr("form.ssh.auth"))
            .selected_text(match auth {
                SshAuthMethod::Password => lang.tr("form.ssh.password_label"),
                SshAuthMethod::PublicKey => lang.tr("form.ssh.key"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut auth, SshAuthMethod::PublicKey, lang.tr("form.ssh.key"));
                ui.selectable_value(&mut auth, SshAuthMethod::Password, lang.tr("form.ssh.password_label"));
            });
        dlg.ssh_auth = auth;
    });
    match dlg.ssh_auth {
        SshAuthMethod::PublicKey => {
            ui.horizontal(|ui| {
                ui.label(lang.tr("form.ssh.private_key"));
                ui.add(TextEdit::singleline(&mut dlg.ssh_key).desired_width(300.0));
            });
        }
        SshAuthMethod::Password => {
            ui.horizontal(|ui| {
                ui.label(lang.tr("form.password"));
                ui.add(TextEdit::singleline(&mut dlg.ssh_password));
            });
        }
    }
    ui.horizontal(|ui| {
        ui.label(lang.tr("form.ssh.local_port"));
        ui.add(DragValue::new(&mut dlg.ssh_local_port).range(1..=65535));
        ui.label(lang.tr("form.ssh.remote_host"));
        ui.add(TextEdit::singleline(&mut dlg.ssh_remote_host));
        ui.label(lang.tr("form.ssh.remote_port"));
        ui.add(DragValue::new(&mut dlg.ssh_remote_port).range(1..=65535));
    });
}

fn render_tls_tab(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    ui.checkbox(&mut dlg.ssl_enabled, lang.tr("form.ssl.enable"));
    if !dlg.ssl_enabled {
        return;
    }
    ui.checkbox(&mut dlg.ssl_allow_invalid, lang.tr("form.ssl.allow_invalid"));
}

fn render_advanced_tab(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(lang.tr("settings.default_db"));
        ui.add(TextEdit::singleline(&mut dlg.default_db));
    });
    ui.horizontal(|ui| {
        if ui.button(lang.tr("settings.from_uri")).clicked() {
            // URI import placeholder
        }
        ui.label(lang.tr("settings.uri_placeholder"));
    });
}

// ---------------------------------------------------------------- View Document

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
