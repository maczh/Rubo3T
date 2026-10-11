//! 中央工作区：标签页、查询输入、结果视图、分页、右键菜单。

use eframe::egui;
use egui::{CentralPanel, DragValue, ScrollArea, TextEdit, TopBottomPanel, Ui};

use crate::app::AppState;
use crate::icons;
use crate::ui::result_view;

pub fn render(ctx: &egui::Context, state: &mut AppState) {
    let _lang = state.lang;
    // 标签页
    TopBottomPanel::top("tabs").show(ctx, |ui| {
        render_tabs(ui, state);
    });

    // 查询编辑区
    TopBottomPanel::top("query_editor").show(ctx, |ui| {
        render_query_editor(ui, state);
    });

    // 中央：结果视图 + 文档编辑 + Shell
    CentralPanel::default().show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.add_space(4.0);
            if state.current_collection().is_some() {
                result_view::render(ui, state);
            } else if !state.tabs.is_empty() {
                render_welcome(ui, state);
            }
            ui.separator();
            egui::CollapsingHeader::new(_lang.tr("doc.edit"))
                .default_open(true)
                .show(ui, |ui| {
                    render_doc_editor(ui, state);
                });
            egui::CollapsingHeader::new(_lang.tr("shell.label"))
                .default_open(true)
                .show(ui, |ui| {
                    render_shell(ui, state);
                });
        });
    });
}

fn render_tabs(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    let tab_ids: Vec<usize> = state.tabs.iter().map(|t| t.id).collect();
    let mut close_idx: Option<usize> = None;
    let mut switch_idx: Option<usize> = None;

    ui.horizontal(|ui| {
        for (idx, _id) in tab_ids.iter().enumerate() {
            let title = state.tabs[idx].title.clone();
            let is_active = state.active_tab == idx;
            let response = ui.selectable_label(is_active, &title);
            if response.clicked() {
                switch_idx = Some(idx);
            }
            if response.middle_clicked() {
                close_idx = Some(idx);
            }
            if is_active {
                response.context_menu(|ui| {
                    if ui.button(lang.tr("tab.close")).clicked() {
                        close_idx = Some(idx);
                        ui.close_menu();
                    }
                });
            }
        }
    });

    if let Some(idx) = switch_idx {
        state.sync_to_active_tab();
        state.active_tab = idx;
        state.sync_from_active_tab();
    }
    if let Some(idx) = close_idx {
        state.close_tab(idx);
    }
}

fn render_query_editor(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    let mut filter = state.filter_text.clone();
    let mut sort = state.sort_text.clone();
    let mut limit = state.limit;
    let mut skip = state.skip;
    let mut act_run = false;
    let mut act_new = false;

    ui.horizontal(|ui| {
        ui.label(lang.tr("query.filter"));
        ui.add(TextEdit::singleline(&mut filter).desired_width(220.0));
        ui.label(lang.tr("query.sort"));
        ui.add(TextEdit::singleline(&mut sort).desired_width(160.0));
        ui.label(lang.tr("query.limit"));
        ui.add(DragValue::new(&mut limit).range(1..=1000));
        ui.label(lang.tr("query.paging"));
        if ui.button(lang.tr("query.prev")).clicked() && skip >= limit {
            skip -= limit;
            act_run = true;
        }
        if ui.button(lang.tr("query.next")).clicked() {
            skip += limit;
            act_run = true;
        }
        if ui.button(lang.tr("query.run")).clicked() {
            act_run = true;
        }
        if ui.button(lang.tr("query.new_doc")).clicked() {
            act_new = true;
        }
    });

    state.filter_text = filter;
    state.sort_text = sort;
    state.limit = limit;
    state.skip = skip;

    if act_run {
        state.run_query();
    }
    if act_new {
        state.selected_doc = None;
        state.selected_doc_text = "{\n  \"_id\": {}\n}".to_string();
    }
}

fn render_welcome(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.add(icons::img("welcome_tab_icon").fit_to_original_size(1.0));
        ui.add_space(8.0);
        ui.heading(lang.tr("welcome.heading"));
        ui.label(lang.tr("welcome.sub"));
        ui.add_space(10.0);
        if ui.button(lang.tr("welcome.open_connect")).clicked() {
            state.conn_dlg = crate::form::ConnectionForm::default();
            state.conn_dlg_selected = None;
            state.conn_dlg_mode = crate::app::ConnDlgMode::List;
            state.conn_dlg_open = true;
        }
        ui.add_space(10.0);
        ui.label(lang.tr("welcome.tip"));
    });
}

fn render_shell(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    let mut run = false;
    ui.horizontal(|ui| {
        ui.label(lang.tr("shell.db"));
        ui.add(TextEdit::singleline(&mut state.shell_db).desired_width(120.0));
        if ui.button(lang.tr("shell.run")).clicked() {
            run = true;
        }
    });
    let mut input = state.shell_input.clone();
    ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
        ui.add(TextEdit::multiline(&mut input).code_editor().desired_rows(4));
    });
    state.shell_input = input;
    ui.label(lang.tr("shell.tip"));
    ui.separator();
    let out = state.shell_output.clone();
    ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
        ui.monospace(&out);
    });
    if run {
        state.run_shell();
    }
}

fn render_doc_editor(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    let mut act_save = false;
    let mut act_delete = false;
    let mut act_insert = false;
    let mut act_index = false;
    let mut act_user = false;

    ui.horizontal(|ui| {
        if ui.button(lang.tr("doc.save")).clicked() {
            act_save = true;
        }
        if ui.button(lang.tr("doc.delete")).clicked() {
            act_delete = true;
        }
        if ui.button(lang.tr("doc.insert")).clicked() {
            act_insert = true;
        }
        if ui.button(lang.tr("index.title")).clicked() {
            act_index = true;
        }
        if ui.button(lang.tr("user.title")).clicked() {
            act_user = true;
        }
    });

    let mut doc_text = state.selected_doc_text.clone();
    ScrollArea::vertical().show(ui, |ui| {
        ui.add(TextEdit::multiline(&mut doc_text).code_editor().desired_rows(18));
    });
    state.selected_doc_text = doc_text;

    if !state.indexes.is_empty() {
        ui.collapsing(format!("{} ({})", lang.tr("index.title"), state.indexes.len()), |ui| {
            for idx in &state.indexes {
                ui.label(format!(
                    "• {}  keys={} {}",
                    idx.name,
                    serde_json::to_string(&idx.keys).unwrap_or_default(),
                    if idx.unique { "[唯一]" } else { "" }
                ));
            }
        });
    }
    if !state.users.is_empty() {
        ui.collapsing(format!("{} ({})", lang.tr("user.title"), state.users.len()), |ui| {
            for u in &state.users {
                ui.label(format!("• {} {} {}", u.name, lang.tr("user.roles"), u.roles.join(", ")));
            }
        });
    }

    if act_save {
        state.save_document();
    }
    if act_delete {
        state.delete_document(None);
    }
    if act_insert {
        state.insert_document();
    }
    if act_index {
        state.load_indexes();
    }
    if act_user {
        state.load_users();
    }
}
