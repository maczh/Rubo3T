//! Ctrl/Cmd+F 文本查找弹窗。
//!
//! 在脚本/聚合编辑器、文档编辑/查看、结果文本（JSON）视图中提供快速查找：
//! 弹出查找框，输入关键字后用「下一个 / 上一个」、Enter 或 Ctrl/Cmd+G / Shift+G
//! 在文本区内跳转。当前命中以「白底红字」高亮，其余命中弱高亮；跳转时把当前
//! 命中滚动到可视区域（零宽光标 + 手动滚动，不抢焦点，Enter 不会误编辑文档）。
use eframe::egui;
use egui::epaint::text::cursor::Cursor;
use egui::text::{CCursor, CursorRange};
use egui::text_selection::text_cursor_state::cursor_rect;
use egui::widgets::text_edit::{TextEditOutput, TextEditState};
use egui::{Align2, Context, FontId, Id, Key, Modifiers, Response, TextEdit, Ui};

use crate::app::{AppState, FindTarget};

/// 各目标文本区对应的 `TextEdit` 稳定 Id，用于程序化设置光标并滚动到命中位置。
pub const EDIT_ID_SCRIPT: &str = "rubo3t_find_script";
pub const EDIT_ID_AGG: &str = "rubo3t_find_agg";
pub const EDIT_ID_EDITDOC: &str = "rubo3t_find_editdoc";
pub const EDIT_ID_VIEWDOC: &str = "rubo3t_find_viewdoc";
pub const EDIT_ID_RESULT: &str = "rubo3t_find_result";
const FIND_INPUT_ID: &str = "rubo3t_find_input";

fn edit_id(target: FindTarget) -> Id {
    Id::new(match target {
        FindTarget::Script => EDIT_ID_SCRIPT,
        FindTarget::Agg => EDIT_ID_AGG,
        FindTarget::EditDoc => EDIT_ID_EDITDOC,
        FindTarget::ViewDoc => EDIT_ID_VIEWDOC,
        FindTarget::ResultText => EDIT_ID_RESULT,
    })
}

/// 取得目标文本区的文本内容（用于计算命中区间）。
///
/// 必须与对应 `TextEdit` 实际展示的文本一致，否则命中字节偏移会对不齐。
/// `ResultText` 与 `result_view::text_view` 一致：无结果时展示 `selected_doc_text`。
fn buffer_text(state: &AppState, target: FindTarget) -> String {
    match target {
        FindTarget::Script => state.script_text.clone(),
        FindTarget::Agg => state.agg_text.clone(),
        FindTarget::EditDoc => state.selected_doc_text.clone(),
        FindTarget::ViewDoc => state.view_doc_text.clone(),
        FindTarget::ResultText => {
            if state.documents.is_empty() {
                state.selected_doc_text.clone()
            } else {
                crate::ui::result_view::all_docs_text(&state.documents)
            }
        }
    }
}

/// 取目标文本区当前应在编辑器里高亮的命中区间（含当前命中索引）。
///
/// 仅当查找弹窗打开且作用在该目标时返回真实命中，否则返回空（不高亮）。
/// 供各 `TextEdit` 的 layouter 调用，把命中区段着色（白底红字 / 其余弱高亮）。
pub fn highlight_ranges(state: &AppState, target: FindTarget) -> (Vec<(usize, usize)>, Option<usize>) {
    if state.find.open && state.find.active == target && !state.find.matches.is_empty() {
        (state.find.matches.clone(), Some(state.find.current))
    } else {
        (Vec::new(), None)
    }
}

/// 把光标定位到 `start`（零宽，仅用于让 TextEdit 输出光标矩形），并请求下一帧
/// 由该文本区把自己滚动到命中位置。
///
/// 采用「零宽光标 + 手动滚动」而非「选区 + 抢焦点」：
/// - 零宽光标不会绘制选区底色，避免覆盖查找高亮（白底红字）；
/// - 不抢焦点，查找输入框可持续聚焦，Enter 始终用于“下一个”，不会误编辑文档。
fn goto(ctx: &Context, state: &mut AppState, target: FindTarget, start: usize) {
    let id = edit_id(target);
    let cursor = Cursor {
        ccursor: CCursor {
            index: start,
            prefer_next_row: false,
        },
        ..Default::default()
    };
    let mut st = TextEditState::load(ctx, id).unwrap_or_default();
    st.cursor.set_range(Some(CursorRange::one(cursor)));
    st.store(ctx, id);
    state.find.pending_scroll = Some(target);
}

/// 文本区渲染后调用：若上一帧对该目标请求了滚动，则把当前命中位置滚动到可见区域。
///
/// 复用 TextEdit 已同步好的 `output.cursor_range`（零宽光标即命中起点），不依赖焦点。
pub fn after_show_scroll(ui: &Ui, state: &mut AppState, target: FindTarget, output: &TextEditOutput) {
    if state.find.pending_scroll != Some(target) {
        return;
    }
    state.find.pending_scroll = None;
    let Some(cr) = output.cursor_range else {
        return;
    };
    let font_id = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| FontId::monospace(12.0));
    let row_height = ui.fonts(|f| f.row_height(&font_id));
    let rect = cursor_rect(output.galley_pos, &output.galley, &cr.primary, row_height);
    ui.scroll_to_rect(rect, None);
}

/// 重新计算命中区间（基于当前 query 与 active 目标）。
fn recompute(state: &mut AppState) {
    state.find.matches.clear();
    let q = state.find.query.trim().to_string();
    if !q.is_empty() {
        let text = buffer_text(state, state.find.active);
        for (start, _) in text.match_indices(&q) {
            state.find.matches.push((start, start + q.len()));
        }
    }
    state.find.current = 0;
}

/// 跳转到指定方向的下一个命中（并滚动/高亮）。
fn navigate(state: &mut AppState, ctx: &Context, forward: bool) {
    let n = state.find.matches.len();
    if n == 0 {
        return;
    }
    let idx = if forward {
        (state.find.current + 1) % n
    } else {
        (state.find.current + n - 1) % n
    };
    state.find.current = idx;
    let active = state.find.active;
    let start = match state.find.matches.get(idx) {
        Some(&(s, _)) => s,
        None => return,
    };
    goto(ctx, state, active, start);
}

/// 由各个文本区在获得焦点时调用：捕获 Ctrl/Cmd+F 打开查找弹窗。
pub fn handle_hotkey(ctx: &Context, state: &mut AppState, target: FindTarget, resp: &Response) {
    if resp.has_focus()
        && ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::F))
    {
        let switched = !state.find.open || state.find.active != target;
        state.find.open = true;
        state.find.active = target;
        state.find.just_opened = true;
        if switched {
            state.find.recompute_now = true;
        }
        ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::COMMAND, Key::F));
    }
}

/// 渲染查找弹窗（每帧在 `update` 末尾调用）。
pub fn render(ctx: &Context, state: &mut AppState) {
    if !state.find.open {
        return;
    }

    let query_changed = state.find.query != state.find.cached_query;
    let active_changed = state.find.active != state.find.cached_active;
    if state.find.recompute_now || query_changed || active_changed {
        recompute(state);
        state.find.cached_query = state.find.query.clone();
        state.find.cached_active = state.find.active;
        state.find.recompute_now = false;
        // 切换目标时自动跳到第一个命中（避免打字时抢焦点）
        if active_changed {
            let first = state.find.matches.first().map(|&(s, _)| s);
            if let Some(s) = first {
                let active = state.find.active;
                goto(ctx, state, active, s);
            }
        }
    }

    // 全局快捷键
    if ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::G)) {
        let shift = ctx.input(|i| i.modifiers.shift);
        navigate(state, ctx, !shift);
        ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::COMMAND, Key::G));
    }
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.find.open = false;
        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    }
    if ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::F)) {
        state.find.just_opened = true;
        ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::COMMAND, Key::F));
    }

    let mut close = false;
    let mut next = false;
    let mut prev = false;
    egui::Window::new("查找")
        .anchor(Align2::CENTER_TOP, [0.0, 8.0])
        .resizable(false)
        .collapsible(false)
        .open(&mut state.find.open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let r = ui.add(
                    TextEdit::singleline(&mut state.find.query)
                        .hint_text("查找内容 (Enter / Ctrl+G 下一个, Shift+G 上一个)")
                        .desired_width(320.0)
                        .id(Id::new(FIND_INPUT_ID))
                        .lock_focus(true),
                );
                if r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    next = true;
                }
                if ui.button("上一个").clicked() {
                    prev = true;
                }
                if ui.button("下一个").clicked() {
                    next = true;
                }
                if ui.button("关闭").clicked() {
                    close = true;
                }
            });
            let total = state.find.matches.len();
            let label = if total == 0 {
                if state.find.query.trim().is_empty() {
                    "输入关键字后回车查找".to_string()
                } else {
                    format!("无匹配: {}", state.find.query)
                }
            } else {
                format!("{}/{}  \"{}\"", state.find.current + 1, total, state.find.query)
            };
            ui.label(label);
        });

    if close {
        state.find.open = false;
    }
    if next {
        navigate(state, ctx, true);
    }
    if prev {
        navigate(state, ctx, false);
    }

    // 刚打开 / 重新聚焦时，把焦点交给查找输入框
    if state.find.just_opened {
        ctx.memory_mut(|m| m.request_focus(Id::new(FIND_INPUT_ID)));
        state.find.just_opened = false;
    }
}
