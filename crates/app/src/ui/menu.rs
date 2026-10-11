//! 顶部菜单栏：File / View / Options / Window / Help。

use eframe::egui;
use egui::{menu, Context, TopBottomPanel};

use crate::app::{AppState, ViewMode};
use crate::i18n::Lang;

pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut act_manage = false;
    let mut act_disconnect = false;
    let mut act_about = false;
    let mut act_exit = false;
    let mut act_fullscreen = false;
    let mut act_next_tab = false;
    let mut act_prev_tab = false;
    let mut act_reexec = false;
    let mut act_duplicate = false;

    TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        menu::bar(ui, |ui| {
            ui.menu_button(lang.tr("menu.file"), |ui| {
                if ui.button(lang.tr("toolbar.connect")).clicked() {
                    act_manage = true;
                    ui.close_menu();
                }
                if state.service.is_some() && ui.button(lang.tr("menu.disconnect")).clicked() {
                    act_disconnect = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(lang.tr("menu.open")).clicked() {
                    ui.close_menu();
                }
                if ui.button(lang.tr("menu.save")).clicked() {
                    ui.close_menu();
                }
                if ui.button(lang.tr("menu.save_as")).clicked() {
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(lang.tr("menu.exit")).clicked() {
                    act_exit = true;
                    ui.close_menu();
                }
            });

            ui.menu_button(lang.tr("menu.view"), |ui| {
                if ui.checkbox(&mut state.show_logs, lang.tr("logs.toggle")).clicked() {
                    ui.close_menu();
                }
                if ui.checkbox(&mut state.show_caps, lang.tr("menu.version_caps")).clicked() {
                    ui.close_menu();
                }
            });

            ui.menu_button("语言 / Language", |ui| {
                if ui.button("中文").clicked() {
                    state.lang = Lang::Zh;
                    state.save_settings();
                    ui.close_menu();
                }
                if ui.button("English").clicked() {
                    state.lang = Lang::En;
                    state.save_settings();
                    ui.close_menu();
                }
            });

            ui.menu_button(lang.tr("menu.options"), |ui| {
                ui.menu_button(lang.tr("menu.view_mode"), |ui| {
                    if ui.radio_value(&mut state.current_view_mode, ViewMode::Custom, lang.tr("view.custom")).clicked() {
                        state.save_settings();
                        ui.close_menu();
                    }
                    if ui.radio_value(&mut state.current_view_mode, ViewMode::Tree, lang.tr("view.tree")).clicked() {
                        state.save_settings();
                        ui.close_menu();
                    }
                    if ui.radio_value(&mut state.current_view_mode, ViewMode::Table, lang.tr("view.table")).clicked() {
                        state.save_settings();
                        ui.close_menu();
                    }
                    if ui.radio_value(&mut state.current_view_mode, ViewMode::Text, lang.tr("view.text")).clicked() {
                        state.save_settings();
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui.menu_button(lang.tr("menu.display_dates"), |ui| {
                    if ui.button(lang.tr("menu.utc")).clicked() {
                        ui.close_menu();
                    }
                    if ui.button(lang.tr("menu.local_time")).clicked() {
                        ui.close_menu();
                    }
                });
                ui.menu_button(lang.tr("menu.uuid"), |ui| {
                    if ui.button(lang.tr("menu.uuid_default")).clicked() { ui.close_menu(); }
                    if ui.button(lang.tr("menu.uuid_java")).clicked() { ui.close_menu(); }
                    if ui.button(lang.tr("menu.uuid_dotnet")).clicked() { ui.close_menu(); }
                    if ui.button(lang.tr("menu.uuid_python")).clicked() { ui.close_menu(); }
                });
                ui.menu_button(lang.tr("menu.autocomplete"), |ui| {
                    if ui.button(lang.tr("menu.autocomplete_all")).clicked() { ui.close_menu(); }
                    if ui.button(lang.tr("menu.autocomplete_no_coll")).clicked() { ui.close_menu(); }
                    if ui.button(lang.tr("menu.autocomplete_none")).clicked() { ui.close_menu(); }
                });
                ui.separator();
                if ui.button(lang.tr("menu.load_mongorc")).clicked() { ui.close_menu(); }
                if ui.checkbox(&mut state.app_settings.auto_expand, lang.tr("menu.auto_expand")).clicked() { ui.close_menu(); }
                if ui.checkbox(&mut state.app_settings.line_numbers, lang.tr("menu.line_numbers")).clicked() { ui.close_menu(); }
                if ui.checkbox(&mut state.app_settings.auto_exec, lang.tr("menu.auto_exec")).clicked() { ui.close_menu(); }
                if ui.checkbox(&mut state.app_settings.disable_conn_shortcuts, lang.tr("menu.disable_conn_shortcuts")).clicked() { ui.close_menu(); }
            });

            ui.menu_button(lang.tr("menu.window"), |ui| {
                if ui.button(lang.tr("menu.full_screen")).clicked() {
                    act_fullscreen = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(lang.tr("menu.next_tab")).clicked() {
                    act_next_tab = true;
                    ui.close_menu();
                }
                if ui.button(lang.tr("menu.prev_tab")).clicked() {
                    act_prev_tab = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(lang.tr("menu.reexec")).clicked() {
                    act_reexec = true;
                    ui.close_menu();
                }
                if ui.button(lang.tr("menu.duplicate_tab")).clicked() {
                    act_duplicate = true;
                    ui.close_menu();
                }
            });

            ui.menu_button(lang.tr("menu.help"), |ui| {
                if ui.button(lang.tr("menu.about")).clicked() {
                    act_about = true;
                    ui.close_menu();
                }
            });
        });
    });

    if act_manage {
        state.conn_dlg = crate::form::ConnectionForm::default();
        state.conn_dlg_selected = None;
        state.conn_dlg_mode = crate::app::ConnDlgMode::List;
        state.conn_dlg_open = true;
    }
    if act_disconnect {
        state.disconnect();
    }
    if act_about {
        // About dialog handled separately
    }
    if act_exit {
        std::process::exit(0);
    }
    if act_fullscreen {
        // eframe fullscreen toggle handled in main
    }
    if act_next_tab && !state.tabs.is_empty() {
        state.active_tab = (state.active_tab + 1) % state.tabs.len();
        state.sync_from_active_tab();
    }
    if act_prev_tab && !state.tabs.is_empty() {
        state.active_tab = if state.active_tab == 0 { state.tabs.len() - 1 } else { state.active_tab - 1 };
        state.sync_from_active_tab();
    }
    if act_reexec {
        state.run_query();
    }
    if act_duplicate {
        if let Some(tab) = state.tabs.get(state.active_tab).cloned() {
            let id = state.next_tab_id;
            state.next_tab_id += 1;
            let mut new_tab = tab.clone();
            new_tab.id = id;
            new_tab.title = format!("{} ({})", tab.title, state.lang.tr("query.tab_title"));
            state.tabs.push(new_tab);
            state.active_tab = state.tabs.len() - 1;
            state.sync_from_active_tab();
        }
    }
}
