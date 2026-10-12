//! 中央工作区：自绘标签栏、查询区（TopStatusBar + 深色脚本编辑器）、输出头（分页 + 视图切换）、结果视图。
//!
//! 布局严格对齐 Robo3T 1.4.4（MainWindow / QueryWidget / OutputItemHeaderWidget）：
//! 标签栏 → 查询区（白底 frame，内含顶部状态栏与深色脚本编辑器）→ 水平分隔线 →
//! 输出头（集合名 / 耗时 / 分页 / 视图按钮）→ 结果视图。中央区不再包含文档编辑 /
//! Shell 折叠面板（已改为弹窗）。

use eframe::egui;
use egui::{
    Align, Button, CentralPanel, Color32, Context, FontId, Frame, Id, ImageButton, Layout, Margin,
    Pos2, Rect, Rounding, ScrollArea, Sense, Stroke, TextEdit, TopBottomPanel, Ui, Vec2,
};

use crate::app::{AppState, FindTarget, QueryMode, QueryTab, ViewMode};
use crate::icons;
use crate::ui::find;
use crate::ui::result_view;
use crate::ui::syntax;

/// 标签栏背景色（与窗口浅灰一致，用于切角时填充）。
const TAB_BAR_BG: Color32 = Color32::from_rgb(0xF0, 0xF0, 0xF0);
/// 脚本编辑器背景（#3D3F41）与文字（白色）。
const SCRIPT_BG: Color32 = Color32::from_rgb(0x3D, 0x3F, 0x41);
const SCRIPT_FG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// 顶部状态栏文字灰（#A6A6A6）。
const STATUS_GRAY: Color32 = Color32::from_rgb(0xA6, 0xA6, 0xA6);

pub fn render(ctx: &Context, state: &mut AppState) {
    let _lang = state.lang;
    // 标签栏（自绘）
    TopBottomPanel::top("tabs").show(ctx, |ui| {
        render_tabs(ui, state);
    });

    // 中央：查询区 + 输出头 + 结果视图，或欢迎页
    CentralPanel::default().show(ctx, |ui| {
        let active = state.tabs.get(state.active_tab);
        let is_welcome = active.map(|t| t.db.is_empty() && t.coll.is_empty()).unwrap_or(true);
        if is_welcome {
            render_welcome(ui, state);
        } else {
            render_query_area(ui, state);
            render_output_header(ui, state);
            result_view::render(ui, state);
        }
    });
}

// ============================================================ 标签栏

fn render_tabs(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    // 先收集每个标签的展示信息，避免在循环内同时借用 state 的可变/不可变引用
    let tabs_info: Vec<(usize, String, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(idx, t)| {
            (
                t.id,
                tab_title(t),
                t.db.is_empty() && t.coll.is_empty(), // 是否欢迎页（无关闭/右键菜单）
                idx == state.active_tab,
            )
        })
        .collect();

    let mut switch_idx: Option<usize> = None;
    let mut close_idx: Option<usize> = None;
    let mut new_shell = false;
    let mut reexec = false;
    let mut duplicate = false;
    let mut close_others: Option<usize> = None;
    let mut close_right: Option<usize> = None;

    ui.painter().rect_filled(
        Rect::from_min_size(ui.min_rect().min, ui.available_size()),
        Rounding::ZERO,
        TAB_BAR_BG,
    );

    ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            if !tabs_info.is_empty() {
                ui.add_space(4.0); // 第一个 tab margin-left: 4px
            }
            for (idx, (id, title, is_welcome, is_active)) in tabs_info.iter().enumerate() {
                let text_w = measure(ui, &title);
                let close_w = if *is_welcome { 0.0 } else { 16.0 };
                let pad_l = 8.0;
                let pad_r = 4.0;
                let content = text_w + pad_l + pad_r + close_w;
                let w = content.clamp(60.0, 200.0);
                // 超过 200 时要进一步按宽度截断标题
                let avail = w - pad_l - pad_r - close_w;
                let title = if measure(ui, &title) > avail {
                    truncate_to_width(ui, &title, avail)
                } else {
                    title.clone()
                };
                let bar_h = 22.0;
                let (tab_rect, tab_resp) =
                    ui.allocate_exact_size(Vec2::new(w, bar_h), Sense::click());

                // 选中态顶部齐平，未选中态顶部下沉 2px、矮 2px
                let paint_rect = if *is_active {
                    tab_rect
                } else {
                    Rect::from_min_max(
                        Pos2::new(tab_rect.left(), tab_rect.top() + 2.0),
                        tab_rect.right_bottom(),
                    )
                };

                // 关闭按钮命中区（欢迎页无）
                let close_rect = if *is_welcome {
                    None
                } else {
                    Some(Rect::from_center_size(
                        Pos2::new(tab_rect.right() - 12.0, tab_rect.center().y),
                        Vec2::new(15.0, 15.0),
                    ))
                };
                let close_resp = close_rect
                    .map(|r| ui.interact(r, Id::new(("tabclose", *id)), Sense::click()));

                // 绘制标签外观
                paint_tab(ui, paint_rect, *is_active, tab_resp.hovered(), TAB_BAR_BG);

                // 标题文字
                let style = ui.style();
                let galley = ui.fonts(|f| {
                    f.layout_no_wrap(
                        title.clone(),
                        egui::TextStyle::Body.resolve(&style),
                        Color32::from_rgb(0x33, 0x33, 0x33),
                    )
                });
                let pos = Pos2::new(
                    paint_rect.left() + pad_l,
                    paint_rect.center().y - galley.size().y / 2.0,
                );
                ui.painter().galley(pos, galley, Color32::PLACEHOLDER);

                // 关闭图标（悬停切换大小）
                if let Some(cr) = close_rect {
                    let hover = ui
                        .ctx()
                        .pointer_hover_pos()
                        .map(|p| cr.contains(p))
                        .unwrap_or(false);
                    let size = if hover { 15.0 } else { 10.0 };
                    let img = if hover {
                        icons::img_size("close_hover_16x16", size)
                    } else {
                        icons::img_size("close_2_16x16", size)
                    };
                    ui.put(cr, img);
                }

                // 事件收集
                let mut do_close = false;
                let mut do_switch = false;
                if let Some(cr) = &close_resp {
                    if cr.clicked() {
                        do_close = true;
                    }
                }
                if !do_close {
                    if tab_resp.clicked() {
                        do_switch = true;
                    }
                    if tab_resp.middle_clicked() {
                        do_close = true;
                    }
                }

                if do_close {
                    close_idx = Some(idx);
                } else if do_switch {
                    switch_idx = Some(idx);
                }

                // 右键菜单（非欢迎页）
                if !*is_welcome {
                    let idx_for_menu = idx;
                    tab_resp.context_menu(|ui| {
                        if ui.button(lang.tr("tab.new_shell")).clicked() {
                            new_shell = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(lang.tr("tab.reexecute")).clicked() {
                            reexec = true;
                            ui.close_menu();
                        }
                        if ui.button(lang.tr("tab.duplicate")).clicked() {
                            duplicate = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(lang.tr("tab.close_shell")).clicked() {
                            close_idx = Some(idx_for_menu);
                            ui.close_menu();
                        }
                        if ui.button(lang.tr("tab.close_others")).clicked() {
                            close_others = Some(idx_for_menu);
                            ui.close_menu();
                        }
                        if ui.button(lang.tr("tab.close_right")).clicked() {
                            close_right = Some(idx_for_menu);
                            ui.close_menu();
                        }
                    });
                }

                ui.add_space(1.0); // 相邻 tab 间距 1px
            }
        });
    });

    if let Some(idx) = switch_idx {
        state.sync_to_active_tab();
        state.active_tab = idx;
        state.sync_from_active_tab();
    }
    if let Some(idx) = close_idx {
        state.close_tab(idx);
    }
    if let Some(idx) = close_others {
        state.close_other_tabs(idx);
    }
    if let Some(idx) = close_right {
        state.close_tabs_to_right(idx);
    }
    if new_shell {
        let db = state
            .tabs
            .get(state.active_tab)
            .and_then(|t| {
                if t.db.is_empty() {
                    None
                } else {
                    Some(t.db.clone())
                }
            })
            .or_else(|| state.databases.first().map(|d| d.name.clone()))
            .unwrap_or_default();
        let id = state.next_tab_id;
        state.next_tab_id += 1;
        let mut tab = QueryTab::new(id, db, "");
        tab.script = String::new();
        tab.dirty = false;
        state.tabs.push(tab);
        state.active_tab = state.tabs.len() - 1;
        state.sync_from_active_tab();
    }
    if reexec {
        state.run_query();
    }
    if duplicate {
        if let Some(tab) = state.tabs.get(state.active_tab).cloned() {
            let id = state.next_tab_id;
            state.next_tab_id += 1;
            let mut new_tab = tab.clone();
            new_tab.id = id;
            new_tab.dirty = true;
            state.tabs.push(new_tab);
            state.active_tab = state.tabs.len() - 1;
            state.sync_from_active_tab();
        }
    }
}

/// 绘制单个标签（Robo3T 梯形圆角风格）。
fn paint_tab(ui: &Ui, rect: Rect, selected: bool, hovered: bool, bg: Color32) {
    let rounding = Rounding {
        nw: 6.0,
        ne: 6.0,
        sw: 0.0,
        se: 0.0,
    };
    if selected || hovered {
        ui.painter().rect_filled(rect, rounding, Color32::WHITE);
    } else {
        // 垂直渐变带
        let steps = 24;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let y0 = rect.top() + rect.height() * t0;
            let y1 = rect.top() + rect.height() * t1;
            let band = Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1));
            ui.painter().rect_filled(band, Rounding::ZERO, gradient_color(t0));
        }
        // 用背景色切掉顶部两个圆角
        let c = 6.0_f32;
        ui.painter()
            .rect_filled(Rect::from_min_size(rect.min, Vec2::new(c, c)), Rounding::ZERO, bg);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(rect.right() - c, rect.top()), Vec2::new(c, c)),
            Rounding::ZERO,
            bg,
        );
    }
    // 边框
    let border = if selected {
        Color32::from_rgb(0x9B, 0x9B, 0x9B)
    } else {
        Color32::from_rgb(0xC4, 0xC4, 0xC3)
    };
    ui.painter()
        .rect_stroke(rect, rounding, Stroke::new(1.0_f32, border));
}

/// 标签渐变取色：#F0F0F0(0) → #DEDEDE(0.4) → #E6E6E6(0.5) → #E1E1E1(1)。
fn gradient_color(t: f32) -> Color32 {
    let stops: [(f32, [u8; 3]); 4] = [
        (0.0, [0xF0, 0xF0, 0xF0]),
        (0.4, [0xDE, 0xDE, 0xDE]),
        (0.5, [0xE6, 0xE6, 0xE6]),
        (1.0, [0xE1, 0xE1, 0xE1]),
    ];
    let mut c = stops[0].1;
    for i in 0..stops.len() - 1 {
        let (t0, c0) = stops[i];
        let (t1, c1) = stops[i + 1];
        if t >= t0 && t <= t1 {
            let f = if (t1 - t0).abs() < 1e-6 {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            c = [
                (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * f) as u8,
                (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * f) as u8,
                (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * f) as u8,
            ];
            break;
        }
    }
    Color32::from_rgb(c[0], c[1], c[2])
}

/// 计算标签展示标题：脚本前 41 字符（\n\r\t→空格），dirty 前缀 "* "，空则 "New Shell"。
fn tab_title(tab: &QueryTab) -> String {
    let raw = if tab.script.is_empty() {
        "New Shell".to_string()
    } else {
        let mut s = tab.script.replace(['\n', '\r', '\t'], " ");
        if s.chars().count() > 41 {
            s = s.chars().take(41).collect();
        }
        s
    };
    if tab.dirty {
        format!("* {}", raw)
    } else {
        raw
    }
}

// ============================================================ 查询区

fn render_query_area(ui: &mut Ui, state: &mut AppState) {
    let frame = Frame::none()
        .fill(Color32::WHITE)
        .inner_margin(Margin {
            top: 5.0,
            right: 1.0,
            bottom: 5.0,
            left: 5.0,
        });
    frame.show(ui, |ui| {
        render_top_status_bar(ui, state);
        render_query_mode_toggle(ui, state);
        render_script_editor(ui, state);
    });
    ui.separator();
}

/// 顶部状态栏：connect/server/database 三图标 + 文字，灰 #A6A6A6，间距 0。
fn render_top_status_bar(ui: &mut Ui, state: &mut AppState) {
    let _lang = state.lang;
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        ui.style_mut().spacing.item_spacing.x = 0.0;
        // connect 图标 + 连接名
        ui.add(icons::img_size("connect_24x24", 16.0));
        ui.add_space(2.0);
        ui.colored_label(STATUS_GRAY, state.active_name.clone());
        ui.add_space(10.0);
        // server 图标 + 服务器地址
        ui.add(icons::img_size("server_16x16", 16.0));
        ui.add_space(2.0);
        let host = state
            .server_info
            .as_ref()
            .map(|i| i.host.clone())
            .unwrap_or_default();
        ui.colored_label(STATUS_GRAY, host);
        ui.add_space(10.0);
        // database 图标 + 库名（无效红色）
        ui.add(icons::img_size("database_16x16", 16.0));
        ui.add_space(2.0);
        let db = state
            .current_collection()
            .map(|(d, _)| d)
            .unwrap_or_default();
        let db_valid = !db.is_empty();
        ui.colored_label(
            if db_valid { STATUS_GRAY } else { Color32::RED },
            db,
        );
    });
    ui.add_space(3.0);
}

/// 查询模式切换：Find / Aggregation（对齐 Robo3T 查询标签栏）。
fn render_query_mode_toggle(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        let entries = [
            (QueryMode::Find, "query.find"),
            (QueryMode::Aggregate, "agg.title"),
        ];
        for (mode, key) in entries {
            let selected = state.query_mode == mode;
            if ui.selectable_label(selected, lang.tr(key)).clicked() {
                state.query_mode = mode;
            }
        }
    });
    ui.add_space(2.0);
}

/// 深色脚本编辑器（背景 #121212、白色文字、字符串绿色、边框 #C7C5C4、圆角 4、等宽、自动高度）。
fn render_script_editor(ui: &mut Ui, state: &mut AppState) {
    let is_agg = state.query_mode == QueryMode::Aggregate;
    let mut buf = if is_agg {
        state.agg_text.clone()
    } else {
        state.script_text.clone()
    };
    let lines = buf.lines().count().max(1);
    let rows = (lines as u32).clamp(1, 18) as usize;
    let frame = Frame::none()
        .fill(SCRIPT_BG)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xC7, 0xC5, 0xC4)))
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::same(2.0));
    let mut submit_query = false;
    frame.show(ui, |ui| {
        // TextEdit 使用 extreme_bg_color 绘制自身底色，而非外层 Frame 的填充色。
        ui.style_mut().visuals.extreme_bg_color = SCRIPT_BG;
        let cursor_style = &mut ui.style_mut().visuals.text_cursor;
        cursor_style.stroke = Stroke::new(2.0_f32, Color32::from_rgb(0x00, 0xFF, 0x00));
        cursor_style.blink = true;
        let target = if is_agg {
            FindTarget::Agg
        } else {
            FindTarget::Script
        };
        let (fm_ranges, fm_current) = find::highlight_ranges(state, target);
        // 语法高亮：默认近白、字符串（单/双引号）绿色（Robo3T 深色编辑器风格）
        let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
            syntax::highlight_with_find(
                ui,
                text,
                wrap_width,
                SCRIPT_FG,
                syntax::STR,
                &fm_ranges,
                fm_current,
            )
        };
        let mut edit = TextEdit::multiline(&mut buf)
            .code_editor()
            .desired_rows(rows)
            .desired_width(ui.available_width())
            .text_color(SCRIPT_FG)
            .layouter(&mut layouter)
            .lock_focus(true)
            .id(Id::new(if is_agg {
                find::EDIT_ID_AGG
            } else {
                find::EDIT_ID_SCRIPT
            }));
        if is_agg {
            edit = edit.hint_text(state.lang.tr("agg.pipeline"));
        }
        // 用 ScrollArea 包裹，使长文本可滚动：
        // 查找“下一个 / 上一个”时 `find::after_show_scroll` 才能把命中行滚动到可视区域居中。
        let font_id = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Monospace)
            .cloned()
            .unwrap_or_else(|| FontId::monospace(13.0));
        let row_height = ui.fonts(|f| f.row_height(&font_id));
        let max_h = (rows as f32) * row_height + 8.0;
        let edit_response = ScrollArea::vertical()
            .max_height(max_h)
            .show(ui, |ui| {
                let output = edit.show(ui);
                find::after_show_scroll(ui, state, target, &output);
                output.response
            })
            .inner;
        if edit_response.has_focus()
            && ui.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Enter))
        {
            ui.input_mut(|input| {
                let modifiers = input.modifiers;
                input.consume_key(modifiers, egui::Key::Enter);
            });
            submit_query = true;
        }
        // Ctrl/Cmd+F 打开文本查找弹窗
        let ctx = ui.ctx().clone();
        find::handle_hotkey(&ctx, state, target, &edit_response);
    });
    if is_agg {
        state.agg_text = buf;
    } else {
        state.script_text = buf;
    }
    if submit_query {
        state.run_current_query();
    }
}

// ============================================================ 输出头

fn render_output_header(ui: &mut Ui, state: &mut AppState) {
    let _lang = state.lang;
    let coll = state
        .current_collection()
        .map(|(_, c)| c)
        .unwrap_or_default();
    let qtime = state
        .tabs
        .get(state.active_tab)
        .map(|t| t.query_time_ms)
        .unwrap_or(0);
    let secs = qtime as f64 / 1000.0;

    ui.horizontal(|ui| {
        // 左侧：集合名 + 耗时
        if !coll.is_empty() {
            ui.add(icons::img_size("collection_16x16", 16.0));
            ui.add_space(2.0);
            ui.label(coll.clone());
        }
        ui.add_space(8.0);
        ui.add(icons::img_size("time_16x16", 16.0));
        ui.add_space(2.0);
        ui.label(format!("{:.3} sec.", secs));

        // 右侧：分页 + 竖线 + 视图按钮
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            render_view_buttons(ui, state);
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            render_paging(ui, state);
        });
    });
    ui.separator();
}

/// 分页控件：◀ [skip] [batch] ▶。
fn render_paging(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    // ▶（右侧先加，因 right_to_left 布局）
    if ui
        .add(ImageButton::new(icons::img_size("right_16x16", 16.0)).frame(false))
        .on_hover_text(lang.tr("query.next"))
        .clicked()
    {
        state.skip += state.limit;
        state.skip_input = state.skip.to_string();
        state.run_query_paged();
    }
    // batch 输入框（批量大小）
    let batch_resp = ui.add(
        TextEdit::singleline(&mut state.batch_input)
            .desired_width(64.0)
            .horizontal_align(Align::Center),
    );
    if batch_resp.lost_focus() {
        if let Ok(v) = state.batch_input.trim().parse::<u64>() {
            state.limit = v.max(1);
            state.run_query_paged();
        }
        state.batch_input = state.limit.to_string();
    }
    batch_resp.on_hover_text(lang.tr("paging.batch"));
    // skip 输入框
    let skip_resp = ui.add(
        TextEdit::singleline(&mut state.skip_input)
            .desired_width(64.0)
            .horizontal_align(Align::Center),
    );
    if skip_resp.lost_focus() {
        if let Ok(v) = state.skip_input.trim().parse::<u64>() {
            state.skip = v;
            state.run_query_paged();
        }
        state.skip_input = state.skip.to_string();
    }
    skip_resp.on_hover_text(lang.tr("paging.skip"));
    // ◀
    if ui
        .add(ImageButton::new(icons::img_size("left_16x16", 16.0)).frame(false))
        .on_hover_text(lang.tr("query.prev"))
        .clicked()
    {
        state.skip = state.skip.saturating_sub(state.limit);
        state.skip_input = state.skip.to_string();
        state.run_query_paged();
    }
}

/// 视图模式按钮：Custom / Tree / Table / Text（选中用 *_highlighted）。
fn render_view_buttons(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang;
    let modes: [(ViewMode, &str, &str, &str); 4] = [
        (
            ViewMode::Custom,
            "custom_16x16",
            "custom_highlighted_16x16",
            "view.custom.tip",
        ),
        (
            ViewMode::Tree,
            "tree_16x16",
            "tree_highlighted_16x16",
            "view.tree.tip",
        ),
        (
            ViewMode::Table,
            "table_16x16",
            "table_highlighted_16x16",
            "view.table.tip",
        ),
        (
            ViewMode::Text,
            "text_16x16",
            "text_highlighted_16x16",
            "view.text.tip",
        ),
    ];
    for (mode, icon, icon_hl, tip) in modes {
        let selected = state.current_view_mode == mode;
        let img = if selected {
            icons::img_size(icon_hl, 16.0)
        } else {
            icons::img_size(icon, 16.0)
        };
        if ui
            .add(ImageButton::new(img).frame(false).selected(selected))
            .on_hover_text(lang.tr(tip))
            .clicked()
        {
            state.current_view_mode = mode;
            state.save_settings();
        }
    }
}

// ============================================================ 欢迎页

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

// ============================================================ 底部状态栏

/// 底部状态栏：左侧 Logs 切换按钮（圆角 4，浅灰，按下加深），右侧连接状态文字。
pub fn render_status_bar(ctx: &Context, state: &mut AppState) {
    let lang = state.lang;
    TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let on = state.show_logs;
            let btn = Button::new(lang.tr("logs.toggle"))
                .rounding(4.0)
                .fill(if on {
                    Color32::from_rgb(0xCF, 0xCF, 0xCF)
                } else {
                    Color32::from_rgb(0xE8, 0xE8, 0xE8)
                })
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xC4, 0xC4, 0xC3)));
            if ui.add(btn).clicked() {
                state.show_logs = !state.show_logs;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = if state.service.is_some() {
                    let mech = state
                        .service
                        .as_ref()
                        .and_then(|s| s.feature_gate())
                        .map(|g| g.default_auth_mechanism().to_string());
                    if let Some(info) = &state.server_info {
                        format!(
                            "{} {} {}{}{}",
                            lang.tr("status.connected"),
                            state.active_name,
                            info.version,
                            if info.is_cluster {
                                format!(" {}", lang.tr("status.cluster"))
                            } else {
                                String::new()
                            },
                            mech.map(|m| format!(" · {}", m)).unwrap_or_default()
                        )
                    } else {
                        format!("{} {}", lang.tr("status.connected"), state.active_name)
                    }
                } else {
                    lang.tr("status.not_connected").to_string()
                };
                ui.label(text);
            });
        });
    });
}

// ============================================================ 工具函数

/// 测量文本在当前 Body 样式下的像素宽度。
fn measure(ui: &Ui, text: &str) -> f32 {
    let style = ui.style();
    ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_string(),
            egui::TextStyle::Body.resolve(&style),
            Color32::WHITE,
        )
        .size()
        .x
    })
}

/// 将标题按宽度截断（在末尾以 "…" 表示，简化用直接截断）。
fn truncate_to_width(ui: &Ui, text: &str, max_w: f32) -> String {
    if measure(ui, text) <= max_w {
        return text.to_string();
    }
    let mut s = text.to_string();
    while !s.is_empty() && measure(ui, &s) > max_w {
        s.pop();
    }
    s
}
