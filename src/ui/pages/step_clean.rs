use gpui_kit::component::input::Input;
use gpui_kit::*;

use crate::ui::app::{FileDialogKind, InputSlot, SeexUi};
use crate::ui::{components, locale, theme};

const BODY: f32 = 20.0;
const SMALL: f32 = 14.0;

impl SeexUi {
    pub(crate) fn step_clean_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input_elem = self.input(InputSlot::StepCleanInput, window, cx);
        let output_elem = self.input(InputSlot::StepCleanOutput, window, cx);

        let is_running = self.step_clean_running;
        let can_start = !is_running
            && !self.step_clean_input.trim().is_empty()
            && !self.step_clean_output.trim().is_empty();

        let header = components::section_header(
            locale::t("3D STEP Logo Remover", "3D STEP 模型去 Logo"),
            locale::t(
                "Specify a STEP file or directory and a destination folder to strip logos and overwrite-save.",
                "指定单个 STEP 文件或目录与另存目标文件夹，逐一去除立创 3D 标志并覆盖保存。",
            ),
        );

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(components::fixed_label(locale::t(
                "STEP Source (file/folder):",
                "STEP源 (文件/文件夹):",
            )))
            .child(div().flex_1().child(Input::new(&input_elem).h(px(32.0))))
            .child(components::button(
                "btn-pick-step-file",
                locale::t("Pick File", "选择文件"),
                !is_running,
                cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.start_file_dialog(FileDialogKind::PickStepInputFile);
                    cx.notify();
                }),
            ))
            .child(components::button(
                "btn-pick-step-folder",
                locale::t("Pick Folder", "选择文件夹"),
                !is_running,
                cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.start_file_dialog(FileDialogKind::PickStepInputFolder);
                    cx.notify();
                }),
            ));

        let output_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(components::fixed_label(locale::t(
                "Destination folder:",
                "另存目标文件夹:",
            )))
            .child(div().flex_1().child(Input::new(&output_elem).h(px(32.0))))
            .child(components::button(
                "btn-pick-step-output",
                locale::t("Browse", "浏览"),
                !is_running,
                cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.start_file_dialog(FileDialogKind::PickStepOutputFolder);
                    cx.notify();
                }),
            ));

        let (curr_progress, total_progress) = self.step_clean_progress;
        let progress_label = if is_running {
            if total_progress > 0 {
                format!(
                    "{} ({} / {})",
                    locale::t("Processing...", "正在处理..."),
                    curr_progress,
                    total_progress
                )
            } else {
                locale::t("Scanning files...", "正在扫描文件...").to_string()
            }
        } else if let Some(summary) = &self.step_clean_summary {
            format!(
                "{} ({} {})",
                locale::t("Completed", "已完成"),
                summary.total_files,
                locale::t("files", "个文件")
            )
        } else {
            locale::t("Ready", "就绪").to_string()
        };

        let start_button = components::selectable(
            "btn-start-clean-step",
            if is_running {
                locale::t("Processing...", "处理中...")
            } else {
                locale::t("Start Cleaning", "开始去除 Logo")
            },
            is_running,
            cx.listener(|this, _event: &ClickEvent, _window, cx| {
                if !this.step_clean_running {
                    this.start_step_cleaning();
                    cx.notify();
                }
            }),
        );

        let action_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(if can_start {
                        start_button
                    } else {
                        start_button.opacity(0.5)
                    })
                    .child(
                        div()
                            .text_size(px(BODY))
                            .text_color(if is_running {
                                theme::primary()
                            } else {
                                theme::muted()
                            })
                            .child(progress_label),
                    ),
            );

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .gap(px(14.0))
            .child(header)
            .child(input_row)
            .child(output_row)
            .child(action_row);

        // Summary cards
        if let Some(summary) = &self.step_clean_summary {
            let stats_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(components::count_chip(
                    summary.total_files,
                    theme::text_color(),
                ))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::muted())
                        .child(locale::t("Scanned", "总文件数")),
                )
                .child(components::count_chip(
                    summary.cleaned_logos_count,
                    theme::success(),
                ))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::success())
                        .child(locale::t("Logos Removed", "去标成功")),
                )
                .child(components::count_chip(
                    summary.cleaned_files_count,
                    theme::primary(),
                ))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::primary())
                        .child(locale::t("Files Modified", "修改文件")),
                )
                .child(components::count_chip(
                    summary.not_found_count,
                    theme::muted(),
                ))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::muted())
                        .child(locale::t("Clean / No Logo", "无标/未改")),
                )
                .child(components::count_chip(summary.kept_count, theme::muted()))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::muted())
                        .child(locale::t("Kept (Safety)", "安全保留")),
                )
                .child(components::count_chip(summary.error_count, theme::danger()))
                .child(
                    div()
                        .text_size(px(SMALL))
                        .text_color(theme::danger())
                        .child(locale::t("Errors", "异常错误")),
                );

            body = body.child(
                div()
                    .p(px(10.0))
                    .rounded(px(theme::RADIUS_MD))
                    .bg(theme::row_bg())
                    .border(px(1.0))
                    .border_color(theme::subtle_border())
                    .child(stats_row),
            );
        }

        // Real-time log list
        let mut log_list = div()
            .id("step-clean-logs")
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .p(px(8.0))
            .rounded(px(theme::RADIUS_MD))
            .border(px(1.0))
            .border_color(theme::subtle_border())
            .bg(theme::field_bg())
            .overflow_y_scroll();

        if self.step_clean_logs.is_empty() {
            log_list = log_list.child(components::empty_state(locale::t(
                "No processing logs yet. Select a file/folder and click Start.",
                "暂无处理日志。请选择源文件/文件夹与保存目录后点击开始。",
            )));
        } else {
            let mono = self.mono_family.clone();
            for log_line in &self.step_clean_logs {
                log_list = log_list.child(
                    div()
                        .py(px(2.0))
                        .font_family(mono.clone())
                        .text_size(px(SMALL))
                        .text_color(theme::text_color())
                        .child(log_line.clone()),
                );
            }
        }

        body = body.child(log_list);

        body.into_any_element()
    }
}
