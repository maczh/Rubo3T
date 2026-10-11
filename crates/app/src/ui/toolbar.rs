//! 顶部工具栏：Connect / Refresh / Execute / Stop / Orientation / View mode toggle。

use eframe::egui;
use egui::{Context, ImageButton, TopBottomPanel};

use crate::app::{AppState, ViewMode};
use crate::icons;

pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut act_manage = false;
    let mut act_refresh = false;
    let mut act_run = false;
    let mut act_stop = false;
    let mut act_rotate = false;
    let connected = state.service.is_some();

    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // 连接管理（图标按钮）
            if ui
                .add(toolbar_icon("connect_24x24"))
                .on_hover_text(lang.tr("toolbar.connect"))
                .clicked()
            {
                act_manage = true;
            }
            // 刷新（暂无对应图标，保留文字按钮）
            if ui.button(lang.tr("menu.refresh")).clicked() {
                act_refresh = true;
            }
            ui.separator();
            // 执行（图标按钮）
            if ui
                .add(toolbar_icon("execute_24x24"))
                .on_hover_text(lang.tr("toolbar.execute"))
                .clicked()
            {
                act_run = true;
            }
            // 停止（图标按钮）
            if ui
                .add(toolbar_icon("stop_24x24"))
                .on_hover_text(lang.tr("toolbar.stop"))
                .clicked()
            {
                act_stop = true;
            }
            // 旋转结果方向（图标按钮）
            if ui
                .add(toolbar_icon("rotate_16x16"))
                .on_hover_text(lang.tr("toolbar.rotate"))
                .clicked()
            {
                act_rotate = true;
            }
            ui.separator();
            // 视图模式切换（图标按钮，保留选中态高亮）
            let modes: [(ViewMode, &str, &str); 4] = [
                (ViewMode::Table, "table_highlighted_16x16", "view.table"),
                (ViewMode::Tree, "tree_highlighted_16x16", "view.tree"),
                (ViewMode::Text, "text_highlighted_16x16", "view.text"),
                (ViewMode::Custom, "custom_highlighted_16x16", "view.custom"),
            ];
            for (mode, icon, key) in modes {
                let selected = state.current_view_mode == mode;
                let resp = ui
                    .add(
                        ImageButton::new(icons::img(icon).fit_to_original_size(1.0))
                            .frame(false)
                            .selected(selected),
                    )
                    .on_hover_text(lang.tr(key));
                if resp.clicked() {
                    state.current_view_mode = mode;
                    state.save_settings();
                }
            }
            ui.separator();
            if connected {
                if let Some(info) = &state.server_info {
                    ui.label(format!(
                        "{} {} {}{}",
                        lang.tr("status.connected"),
                        state.active_name,
                        info.version,
                        if info.is_cluster {
                            format!(" {}", lang.tr("status.cluster"))
                        } else {
                            String::new()
                        }
                    ));
                } else {
                    ui.label(format!("{} {}", lang.tr("status.connected"), state.active_name));
                }
            } else {
                ui.label(lang.tr("status.not_connected"));
            }
        });
    });

    if act_manage {
        state.conn_dlg = crate::form::ConnectionForm::default();
        state.conn_dlg_selected = None;
        state.conn_dlg_mode = crate::app::ConnDlgMode::List;
        state.conn_dlg_open = true;
    }
    if act_refresh {
        state.load_databases();
    }
    if act_run {
        state.run_query();
    }
    if act_stop {
        // stop not implemented yet
    }
    if act_rotate {
        // orientation not implemented yet
    }
}

/// 构造一个无边框的工具栏图标按钮（仅图片，悬停/按下高亮由 egui 默认处理）。
fn toolbar_icon(icon: &str) -> ImageButton<'static> {
    ImageButton::new(icons::img(icon).fit_to_original_size(1.0)).frame(false)
}
