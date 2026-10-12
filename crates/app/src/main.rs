#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Rubo3T —— 用 Rust 重写的 MongoDB 管理工具（MVP）。
//!
//! 目标：在保持原 Robo3T 1.4.4 使用习惯的前提下，用官方 `mongodb` Rust 驱动
//! 覆盖 MongoDB 3.x → 8.x、副本集 / 分片集群 / Atlas(SRV) / RDS(DocumentDB)，
//! 并支持各版本认证机制与协议差异。
//!
//! GUI 采用 egui 0.29，通过 tokio 运行时同步调用异步驱动。

mod app;
mod font;
mod form;
mod i18n;
mod icons;
mod ui;

use eframe::egui;
use egui::{Context, TopBottomPanel, Vec2, ViewportBuilder};

use crate::app::AppState;

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 把 SSH 隧道调试日志抽到主日志框（隧道在后台线程/运行时并发写入共享缓冲）。
        self.drain_ssh_logs();

        // 同步当前 tab 的状态到 app state 渲染字段
        if !self.tabs.is_empty() {
            self.sync_to_active_tab();
        }

        // 首次有效帧：注册 Robo3T 图标并应用浅色主题（保证每帧只注册一次）
        if !self.icon_registered {
            crate::icons::register(ctx);
            crate::font::apply_robo_theme(ctx);
            self.icon_registered = true;
        }

        ui::menu::render(ctx, self);
        ui::toolbar::render(ctx, self);
        ui::explorer::render(ctx, self);

        // 日志面板（底部，受状态栏 Logs 按钮控制显隐）
        if self.show_logs {
            TopBottomPanel::bottom("logs")
                .resizable(true)
                .default_height(120.0)
                .show(ctx, |ui| {
                    ui.strong(self.lang.tr("logs.title"));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for line in self.logs.iter().rev() {
                                ui.add(egui::Label::new(line).wrap());
                            }
                        });
                });
        }

        // 底部状态栏（紧贴窗口底部，位于日志面板之下）
        ui::workspace::render_status_bar(ctx, self);

        ui::workspace::render(ctx, self);
        ui::dialogs::render(ctx, self);
        // 文本查找弹窗（Ctrl/Cmd+F 触发），置于最上层渲染
        ui::find::render(ctx, self);
    }
}

fn main() -> eframe::Result<()> {
    let icon = crate::icons::window_icon();
    let mut viewport = ViewportBuilder::default()
        .with_title("Rubo 3T - 1.4")
        .with_inner_size(Vec2::new(1280.0, 820.0));
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Rubo 3T - 1.4",
        options,
        Box::new(|cc| Ok(Box::new(AppState::new(cc)))),
    )
}
