//! 顶部工具栏：Connect / Open / Save | Execute / Stop / Rotate。
//!
//! 顺序与 Robo3T 1.4.4 一致；移除 Refresh 文字按钮、视图模式切换与连接状态文字
//! （状态文字已移至底部状态栏）。Open/Save 图标为程序化绘制的浅灰文件夹/软盘纹理。

use std::path::PathBuf;

use eframe::egui;
use egui::popup;
use egui::{Context, ImageButton, TopBottomPanel};

use crate::app::AppState;
use crate::icons;

pub fn render(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    let mut act_manage = false;
    let mut act_quick_connect: Option<usize> = None;
    let mut act_open = false;
    let mut act_save = false;
    let mut act_run = false;
    let mut act_stop = false;
    let mut act_rotate = false;

    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // ---- 连接：分裂按钮 ----
            // 主图标：点击打开「连接管理」对话框（行为与改造前一致）。
            if ui
                .add(toolbar_icon("connect_24x24"))
                .on_hover_text(lang.tr("toolbar.connect"))
                .clicked()
            {
                act_manage = true;
            }
            // 下拉箭头触发区：点击弹出已保存连接的快速连接菜单。
            let connect_id = egui::Id::new("toolbar_connect_menu");
            let arrow_resp =
                ui.allocate_response(egui::vec2(12.0, 20.0), egui::Sense::click());
            // 画向下小三角（hover 或菜单已开时先画浅灰高亮底）。
            let painter = ui.painter();
            if arrow_resp.hovered()
                || ui.ctx().memory(|m| m.is_popup_open(connect_id))
            {
                painter.rect_filled(
                    arrow_resp.rect,
                    egui::Rounding::ZERO,
                    egui::Color32::from_rgb(0xE0, 0xE0, 0xE0),
                );
            }
            if arrow_resp.clicked() {
                ui.ctx().memory_mut(|mem| mem.toggle_popup(connect_id));
            }
            let arrow_rect = arrow_resp.rect;
            let top_y = arrow_rect.top() + arrow_rect.height() * 0.35;
            let bot_y = arrow_rect.top() + arrow_rect.height() * 0.65;
            let tri_left = egui::pos2(arrow_rect.left() + arrow_rect.width() * 0.2, top_y);
            let tri_right = egui::pos2(arrow_rect.right() - arrow_rect.width() * 0.2, top_y);
            let tri_mid = egui::pos2(arrow_rect.center().x, bot_y);
            painter.add(egui::Shape::convex_polygon(
                vec![tri_left, tri_right, tri_mid],
                ui.visuals().text_color(),
                egui::Stroke::NONE,
            ));

            // 下拉菜单：列出已保存连接 + 「管理连接…」。
            // 先克隆连接列表，菜单闭包内只读 conns（不可变），选中项写入
            // act_quick_connect，避免闭包内再次可变借用 state 触发借用检查错误。
            let conns = state.connections.clone();
            popup::popup_below_widget(
                ui,
                connect_id,
                &arrow_resp,
                popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(220.0);
                    for (i, conn) in conns.iter().enumerate() {
                        // 可读名称：连接名非空用名字，否则 "host:port"。
                        let name = if conn.name.trim().is_empty() {
                            conn.hosts
                                .first()
                                .map(|h| format!("{}:{}", h.host, h.port))
                                .unwrap_or_else(|| "?".to_string())
                        } else {
                            conn.name.clone()
                        };
                        // 前 9 条在行右侧显示弱灰 "Alt+N" 快捷键提示。
                        let r = ui.horizontal(|ui| {
                            let rr = ui.selectable_label(false, name.as_str());
                            if i < 9 {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("Alt+{}", i + 1)).weak(),
                                        );
                                    },
                                );
                            }
                            rr
                        });
                        if r.inner.clicked() {
                            act_quick_connect = Some(i);
                            ui.ctx().memory_mut(|m| m.close_popup());
                        }
                    }
                    if !conns.is_empty() {
                        ui.separator();
                    }
                    // 末项「管理连接…」：打开连接管理对话框（即使无已保存连接也显示）。
                    let mr = ui.horizontal(|ui| {
                        ui.add(icons::img_size("connect_24x24", 16.0));
                        ui.selectable_label(false, lang.tr("conn.manage"))
                    });
                    if mr.inner.clicked() {
                        act_manage = true;
                        ui.ctx().memory_mut(|m| m.close_popup());
                    }
                },
            );
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

    // ---- 快速连接：Alt+1..9 全局快捷键 ----
    // 与下拉菜单选中的处理合并到同一处（统一调用 state.connect）。
    if ctx.input(|i| i.modifiers.alt) {
        let keys = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
        ];
        for (idx, k) in keys.iter().enumerate() {
            if ctx.input(|i| i.key_pressed(*k)) && idx < state.connections.len() {
                act_quick_connect = Some(idx);
                break;
            }
        }
    }
    if let Some(i) = act_quick_connect {
        if let Some(s) = state.connections.get(i).cloned() {
            state.connect(s);
        }
    }

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
        state.run_current_query();
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
