//! The History page: what the clipboard monitor has seen, newest first.
//!
//! Ported from `SeexApp::history_page` in `src/main.rs`: the same panel and the
//! same per-row Copy/Delete pair. The list used to be pinned to 495px whatever
//! the window was, which showed nine entries in a page with room for twice that;
//! it now takes the height the page has, so a taller window simply shows more.

use gpui_kit::*;

use crate::ui::app::{SeexUi, copy_to_clipboard, first_line};
use crate::ui::{components, locale, theme};

const BODY: f32 = 20.0;

impl SeexUi {
    pub(crate) fn history_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let history = self
            .state
            .lock()
            .map(|state| state.history.clone())
            .unwrap_or_default();

        let header = components::section_header_inline(
            locale::t("History", "历史"),
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(components::count_chip(history.len(), theme::primary()))
                .child(components::button(
                    "clear-history",
                    locale::t("Delete all", "全部删除"),
                    !history.is_empty(),
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        if let Ok(mut state) = this.state.lock() {
                            state.clear_history();
                        }
                        this.show_toast_info(locale::t("History cleared", "历史已清空"));
                        cx.notify();
                    }),
                )),
        );

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            // Every link in the chain has to be allowed to shrink, or the one
            // that is not pushes the panel past the bottom of the window.
            .min_h(px(0.0))
            .gap(px(10.0))
            .child(header);

        // The entries live in a box of their own, and the box is always drawn:
        // the list scrolls inside it, so however short the window is the box's
        // outline — including its bottom edge — stays on screen instead of the
        // last row being cut off by the window frame.
        let mut list = div()
            .id("history-list")
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .p(px(8.0))
            .rounded(px(theme::RADIUS_MD))
            .border(px(1.0))
            .border_color(theme::subtle_border())
            .overflow_y_scroll();

        if history.is_empty() {
            list = list.child(components::empty_state(locale::t(
                "No history yet",
                "暂无历史记录",
            )));
        } else {
            for (index, (time, content)) in history.iter().enumerate() {
                let preview = first_line(content, 110);
                let mono = self.mono_family.clone();

                list = list.child(
                    div().mb(px(6.0)).child(components::row_frame(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .font_family(mono)
                                    .text_size(px(BODY))
                                    .text_color(theme::muted())
                                    .child(time.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_size(px(BODY))
                                    .text_color(theme::text_color())
                                    .child(preview),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(components::small_button(
                                        format!("copy-history-{index}"),
                                        locale::t("Copy", "复制"),
                                        {
                                            let copied = content.clone();
                                            cx.listener(
                                                move |this, _event: &ClickEvent, _window, cx| {
                                                    copy_to_clipboard(&copied);
                                                    this.show_toast_success(locale::t(
                                                        "Copied",
                                                        "已复制",
                                                    ));
                                                    cx.notify();
                                                },
                                            )
                                        },
                                    ))
                                    .child(components::small_button(
                                        format!("delete-history-{index}"),
                                        locale::t("Delete", "删除"),
                                        cx.listener(
                                            move |this, _event: &ClickEvent, _window, cx| {
                                                if let Ok(mut state) = this.state.lock() {
                                                    state.delete_history(index);
                                                }
                                                cx.notify();
                                            },
                                        ),
                                    )),
                            ),
                    )),
                );
            }
        }
        body = body.child(list);

        // The panel takes the height the page has, so the list below the header
        // is as tall as the window allows rather than a fixed 495px.
        components::section_fill(body).into_any_element()
    }
}
