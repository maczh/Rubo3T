//! 顶部工具栏：Connect / Open / Save | Execute / Stop / Rotate。
//!
//! 顺序与 Robo3T 1.4.4 一致；移除 Refresh 文字按钮、视图模式切换与连接状态文字
//! （状态文字已移至底部状态栏）。Open/Save 图标为程序化绘制的浅灰文件夹/软盘纹理。

use std::path::PathBuf;

use eframe::egui;
use egui::{Context, ImageButton, TopBottomPanel};

use crate::app::AppState;
use crate::icons;

pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut act_manage = false;
    let mut act_open = false;
    let mut act_save = false;
    let mut act_run = false;
    let mut act_stop = false;
    let mut act_rotate = false;

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
            // 打开脚本（程序化文件夹图标）
            if ui
                .add(
                    ImageButton::new(icons::img_size("open_16x16", 16.0))
                        .frame(false),
                )
                .on_hover_text(lang.tr("toolbar.open"))
                .clicked()
            {
                act_open = true;
            }
            // 保存脚本（程序化软盘图标）
            if ui
                .add(
                    ImageButton::new(icons::img_size("save_16x16", 16.0))
                        .frame(false),
                )
                .on_hover_text(lang.tr("toolbar.save"))
                .clicked()
            {
                act_save = true;
            }
            // 分隔线仅在 Open/Save 与 Execute 组之间一条
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
        });
    });

    if act_manage {
        state.conn_dlg = crate::form::ConnectionForm::default();
        state.conn_dlg_selected = None;
        state.conn_dlg_mode = crate::app::ConnDlgMode::List;
        state.conn_dlg_open = true;
    }
    if act_open {
        let path = script_path(state);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                state.script_text = text;
                state.sync_to_active_tab();
                state.toast(&format!("{}: {}", lang.tr("toolbar.open"), path.display()), false);
            }
            Err(_) => state.toast(lang.tr("query.script_empty"), true),
        }
    }
    if act_save {
        let path = script_path(state);
        if let Err(e) = std::fs::write(&path, &state.script_text) {
            state.toast(&format!("{}: {}", lang.tr("toolbar.save"), e), true);
        } else {
            state.toast(&format!("{}: {}", lang.tr("toolbar.save"), path.display()), false);
        }
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

/// 脚本存取路径（位于配置目录下固定文件名）。
fn script_path(state: &AppState) -> PathBuf {
    state
        .connections_path
        .parent()
        .map(|p| p.join("rubo3t_script.js"))
        .unwrap_or_else(|| PathBuf::from("rubo3t_script.js"))
}

/// 构造一个无边框的工具栏图标按钮（仅图片，悬停/按下高亮由 egui 默认处理）。
fn toolbar_icon(icon: &str) -> ImageButton<'static> {
    ImageButton::new(icons::img(icon).fit_to_original_size(1.0)).frame(false)
}
