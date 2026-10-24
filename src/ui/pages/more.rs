//! The More page: appearance preferences on the left, updates on the right, and
//! the donation codes underneath when they are switched on.
//!
//! Ported from `SeexApp::more_page` in `src/main.rs`. The language switch that
//! used to live in the top bar is not here either: it moved to the Font page,
//! next to the font pickers.

use gpui_kit::*;

use crate::ui::app::{SeexUi, UpdateState, version_is_newer};
use crate::ui::{components, locale, theme};

const MORE_PANEL_HEIGHT: f32 = 300.0;
const BODY: f32 = 20.0;

/// The twelve presets, in the order the egui grid listed them.
const PRESETS: [(&str, &str, &str); 12] = [
    ("ocean", "Ocean", "海洋"),
    ("graphite", "Graphite", "石墨"),
    ("forest", "Forest", "森林"),
    ("amber", "Amber", "琥珀"),
    ("rose", "Rose", "玫瑰"),
    ("violet", "Violet", "紫罗兰"),
    ("teal", "Teal", "青色"),
    ("cobalt", "Cobalt", "钴蓝"),
    ("sunset", "Sunset", "日落"),
    ("mint", "Mint", "薄荷"),
    ("indigo", "Indigo", "靛蓝"),
    ("coral", "Coral", "珊瑚"),
];

impl SeexUi {
    pub(crate) fn more_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let columns = div()
            .flex()
            .flex_row()
            .w_full()
            .gap(px(18.0))
            .child(div().flex_1().child(self.preferences_panel(cx)))
            .child(div().flex_1().child(self.updates_panel(cx)));

        let mut page = div().flex().flex_col().w_full().child(columns);

        if self.show_donate {
            page = page.child(div().h(px(12.0))).child(self.donation_row());
        }

        page.into_any_element()
    }

    fn preferences_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut grid = div().flex().flex_col().w_full().gap(px(12.0));
        for row in PRESETS.chunks(4) {
            let mut line = div().flex().flex_row().w_full().gap(px(12.0));
            for (id, en, zh) in row {
                let id = *id;
                line = line.child(components::theme_button(
                    format!("theme-{id}"),
                    locale::t(en, zh),
                    self.theme_preset == id,
                    cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                        this.theme_preset = id.to_string();
                        crate::ui::theme::set_theme_index(crate::ui::theme::theme_index(id));
                        this.mark_config_dirty();
                        cx.notify();
                    }),
                ));
            }
            grid = grid.child(line);
        }

        components::section_with_min_height(
            MORE_PANEL_HEIGHT,
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(10.0))
                .child(components::section_header(
                    locale::t("Preferences", "偏好设置"),
                    locale::t("Choose what SeEx displays.", "选择显示内容。"),
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .w_full()
                        .gap(px(12.0))
                        .child(
                            div()
                                .w(px(72.0))
                                .h(px(38.0))
                                .flex()
                                .items_center()
                                .text_size(px(BODY))
                                .text_color(theme::muted())
                                .child(locale::t("Theme:", "主题:")),
                        )
                        .child(div().flex_1().child(grid)),
                )
                .child(components::selectable(
                    "toggle-donate",
                    locale::t("Show the donation code", "显示赞助二维码"),
                    self.show_donate,
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.show_donate = !this.show_donate;
                        this.mark_config_dirty();
                        cx.notify();
                    }),
                )),
        )
        .into_any_element()
    }

    fn updates_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let update = self
            .update_state
            .lock()
            .map(|state| UpdateState {
                checking: state.checking,
                checked: state.checked,
                latest_version: state.latest_version.clone(),
                release_url: state.release_url.clone(),
                error: state.error.clone(),
            })
            .unwrap_or_default();

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(10.0))
            .child(components::section_header(
                locale::t("Updates", "更新"),
                locale::t("Keep SeEx up to date", "检查 SeEx 是否有新版本"),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_wrap()
                    .gap(px(12.0))
                    .text_size(px(BODY))
                    .text_color(theme::text_color())
                    .child(format!(
                        "{}: {}",
                        locale::t("Current version", "当前版本"),
                        env!("CARGO_PKG_VERSION")
                    ))
                    .child(if update.checking {
                        div()
                            .text_color(theme::muted())
                            .child(locale::t("Checking...", "检查中..."))
                            .into_any_element()
                    } else {
                        components::button(
                            "check-updates",
                            locale::t("Check for updates", "检查更新"),
                            true,
                            cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.start_update_check();
                                cx.notify();
                            }),
                        )
                        .into_any_element()
                    }),
            );

        if let Some(error) = update.error {
            body = body.child(
                div()
                    .text_size(px(BODY))
                    .text_color(theme::danger())
                    .child(error),
            );
        } else if let Some(latest) = update.latest_version {
            if version_is_newer(&latest, env!("CARGO_PKG_VERSION")) {
                let mut line = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_wrap()
                    .gap(px(12.0))
                    .text_size(px(BODY))
                    .text_color(theme::success())
                    .child(format!(
                        "{}: {}",
                        locale::t("Update available", "发现新版本"),
                        latest
                    ));
                if let Some(url) = update.release_url {
                    line = line.child(
                        div()
                            .id("download-update")
                            .cursor_pointer()
                            .text_color(theme::primary())
                            .underline()
                            .child(locale::t("Download update", "下载更新"))
                            .on_click(cx.listener(
                                move |_this, _event: &ClickEvent, _window, cx| {
                                    cx.open_url(&url);
                                },
                            )),
                    );
                }
                body = body.child(line);
            } else if update.checked {
                body = body.child(
                    div()
                        .text_size(px(BODY))
                        .text_color(theme::success())
                        .child(locale::t("You are up to date.", "当前已是最新版本。")),
                );
            }
        }

        components::section_with_min_height(MORE_PANEL_HEIGHT, body).into_any_element()
    }

    /// The donation code, centred under the panels at a fixed height. One image
    /// carries both payment apps, so there is nothing to lay out side by side.
    fn donation_row(&self) -> AnyElement {
        const HEIGHT: f32 = 320.0;
        div()
            .flex()
            .flex_row()
            .items_start()
            .justify_center()
            .w_full()
            .child(
                img(self.donation_code.clone())
                    .h(px(HEIGHT))
                    .w(px(HEIGHT * self.donation_aspect)),
            )
            .into_any_element()
    }
}
