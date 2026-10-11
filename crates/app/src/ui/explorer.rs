//! 左侧 Database Explorer：服务器 → 数据库 → 集合。

use eframe::egui;
use egui::{CollapsingHeader, Context, ScrollArea, SidePanel};

use crate::app::AppState;
use crate::icons;

pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let connected = state.service.is_some();
    let databases = state.databases.clone();
    let collections = state.collections.clone();
    let mut clicked: Option<(String, String)> = None;
    let mut expand_db = String::new();

    SidePanel::left("explorer")
        .resizable(true)
        .default_width(240.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(lang.tr("explorer.title"));
            });
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if !connected {
                    ui.label(lang.tr("explorer.not_connected"));
                    return;
                }
                if !state.active_name.is_empty() {
                    // 服务器节点（前加 server 图标）
                    ui.horizontal(|ui| {
                        ui.add(icons::img("server_16x16").fit_to_original_size(1.0));
                        CollapsingHeader::new(format!("{} ({})", state.active_name, state.databases.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                for db in &databases {
                                    let is_expanded =
                                        *state.expanded_dbs.get(&db.name).unwrap_or(&false);
                                    let header_text = if is_expanded {
                                        format!("▾ {}", db.name)
                                    } else {
                                        format!("▸ {}", db.name)
                                    };
                                    // 数据库节点（前加 database 图标）
                                    let db_response = ui
                                        .horizontal(|ui| {
                                            ui.add(
                                                icons::img("database_16x16")
                                                    .fit_to_original_size(1.0),
                                            );
                                            CollapsingHeader::new(&header_text)
                                                .id_salt(format!("db_{}", db.name))
                                                .default_open(is_expanded)
                                                .show(ui, |ui| {
                                                    if let Some(cols) =
                                                        collections.get(&db.name)
                                                    {
                                                        if cols.is_empty() {
                                                            ui.label(lang.tr(
                                                                "explorer.no_collections",
                                                            ));
                                                        }
                                                        for c in cols {
                                                            // 集合节点（前加 collection 图标，保留原有点击行为）
                                                            let label = c.name.clone();
                                                            let r = ui
                                                                .horizontal(|ui| {
                                                                    ui.add(
                                                                        icons::img(
                                                                            "collection_16x16",
                                                                        )
                                                                        .fit_to_original_size(1.0),
                                                                    );
                                                                    ui.selectable_label(false, label)
                                                                })
                                                                .inner;
                                                            if r.clicked() {
                                                                clicked = Some((
                                                                    db.name.clone(),
                                                                    c.name.clone(),
                                                                ));
                                                            }
                                                        }
                                                    } else {
                                                        ui.label(lang.tr(
                                                            "explorer.no_collections",
                                                        ));
                                                    }
                                                })
                                        })
                                        .inner;
                                    if db_response.header_response.clicked() {
                                        expand_db = db.name.clone();
                                    }
                                    // 数据库节点右键菜单：索引 / 用户（只读弹窗）
                                    db_response.header_response.context_menu(|ui| {
                                        if ui.button(lang.tr("dialog.indexes")).clicked() {
                                            let coll = collections
                                                .get(&db.name)
                                                .and_then(|c| c.first())
                                                .map(|c| c.name.clone())
                                                .unwrap_or_default();
                                            state.open_indexes_dialog(&db.name, &coll);
                                            ui.close_menu();
                                        }
                                        if ui.button(lang.tr("dialog.users")).clicked() {
                                            state.open_users_dialog(&db.name);
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                    });
                }
            });
        });

    if !expand_db.is_empty() {
        let open = !state.expanded_dbs.get(&expand_db).unwrap_or(&false);
        state.expanded_dbs.insert(expand_db.clone(), open);
        if open {
            state.load_collections(&expand_db);
        }
    }
    if let Some((d, c)) = clicked {
        state.select_collection(&d, &c);
    }
}
