//! The Font page: the language switch and the two installed-font pickers.
//!
//! This page is why SeEx moved to GPUI: egui could only use the faces it was
//! given at start-up, while GPUI can enumerate everything the system has
//! installed. The shape follows CAPS — a slot for the Roman-script face, a slot
//! for the Han face that becomes a fallback chain, and one search box over the
//! list.

use gpui_kit::component::input::Input;
use gpui_kit::*;

use crate::ui::app::{InputSlot, SeexUi};
use crate::ui::fonts::{self, FontSlot};
use crate::ui::locale::{self, Lang};
use crate::ui::{components, theme};

const BODY: f32 = 20.0;
const PICKER_HEIGHT: f32 = 260.0;

impl SeexUi {
    pub(crate) fn font_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.language_panel(window, cx))
            .child(div().h(px(10.0)))
            .child(self.fonts_panel(window, cx))
            .into_any_element()
    }

    /// The language switch, which used to sit in the top bar.
    fn language_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let current = locale::lang();
        components::section(
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(10.0))
                .child(components::section_header(
                    locale::t("Language", "语言"),
                    locale::t(
                        "The interface language. Hover text follows it too.",
                        "界面语言，悬停提示同样跟随。",
                    ),
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .child(components::fixed_label(locale::t("Language:", "语言:")))
                        .child(components::selectable(
                            "lang-en",
                            "English",
                            current == Lang::En,
                            cx.listener(move |this, _event: &ClickEvent, window, cx| {
                                locale::set_lang(Lang::En);
                                this.refresh_placeholders(window, cx);
                                this.mark_config_dirty();
                                cx.notify();
                            }),
                        ))
                        .child(components::selectable(
                            "lang-zh",
                            "中文",
                            current == Lang::Zh,
                            cx.listener(move |this, _event: &ClickEvent, window, cx| {
                                locale::set_lang(Lang::Zh);
                                this.refresh_placeholders(window, cx);
                                this.mark_config_dirty();
                                cx.notify();
                            }),
                        )),
                ),
        )
        .into_any_element()
    }

    fn fonts_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let search = self.input(InputSlot::FontSearch, window, cx);
        let query = search.read(cx).value().to_string();

        let latin_current = self.font_latin.to_string();
        let han_current = self
            .font_han_choice
            .clone()
            .unwrap_or_else(|| locale::t("System default", "系统默认").to_string());

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(12.0))
            .child(components::section_header(
                locale::t("Fonts", "字体"),
                locale::t(
                    "Pick any family installed on this machine.",
                    "可选择本机已安装的任意字体。",
                ),
            ))
            .child(components::font_slot_row(
                "font-slot-latin",
                locale::t("Latin:", "西文:"),
                latin_current,
                self.font_picker == Some(FontSlot::Latin),
                cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.font_picker = match this.font_picker {
                        Some(FontSlot::Latin) => None,
                        _ => Some(FontSlot::Latin),
                    };
                    cx.notify();
                }),
            ))
            .child(components::font_slot_row(
                "font-slot-han",
                locale::t("Han:", "中文:"),
                han_current,
                self.font_picker == Some(FontSlot::Han),
                cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.font_picker = match this.font_picker {
                        Some(FontSlot::Han) => None,
                        _ => Some(FontSlot::Han),
                    };
                    cx.notify();
                }),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .gap(px(12.0))
                    .child(components::fixed_label(locale::t("Search:", "搜索:")))
                    .child(div().flex_1().child(Input::new(&search).h(px(30.0)))),
            );

        if let Some(slot) = self.font_picker {
            let choices = match slot {
                FontSlot::Latin => self.latin_choices.clone(),
                FontSlot::Han => self.han_choices.clone(),
            };
            let selected = match slot {
                FontSlot::Latin => self.font_latin_choice.clone(),
                FontSlot::Han => self.font_han_choice.clone(),
            };
            let matching = fonts::matches(&choices, &query);

            let mut list = div()
                .id("font-list")
                .flex()
                .flex_col()
                .w_full()
                .h(px(PICKER_HEIGHT))
                .overflow_y_scroll();

            if matching.is_empty() {
                list = list.child(components::empty_state(locale::t(
                    "No font matches that search.",
                    "没有匹配的字体。",
                )));
            } else {
                for index in matching {
                    let name = choices[index].clone();
                    let is_selected = selected.as_deref() == Some(name.as_str());
                    list = list.child(div().mb(px(4.0)).child(components::font_row(
                        format!("font-{index}"),
                        name.clone(),
                        is_selected,
                        cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                            match slot {
                                FontSlot::Latin => this.set_latin_font(name.clone(), cx),
                                FontSlot::Han => this.set_han_font(name.clone()),
                            }
                            this.font_picker = None;
                            cx.notify();
                        }),
                    )));
                }
            }

            body = body.child(list);
        }

        components::section(body).into_any_element()
    }
}
