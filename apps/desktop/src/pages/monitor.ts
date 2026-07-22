// Clipboard monitor page module.
//
// This module owns the monitor page's local view state, list rendering and
// monitor-specific commands. Cross-page configuration writes and state
// refreshes are injected so the root can remain the single app coordinator.

import { invoke } from "@tauri-apps/api/core";

import { errorMessage } from "../ipc";
import { translate } from "../i18n";
import { tupleListChanged } from "../patched-render";
import { toast } from "../ui/toast";
import { validateRequiredPath } from "../validation";
import type { AppState, ExportMessageKind } from "../types";
import { $, applyTooltips, escapeAttr, escapeHtml, syncInputValue } from "../utils";

const t = translate;

export interface MonitorPageContext {
  refresh: () => Promise<void>;
  queueConfigWrite: (operation: () => Promise<void>) => Promise<void>;
  selectSaveFile: (title: string, defaultPath: string | undefined) => Promise<string | null>;
}

const PATTERN_QUICK = "regex:(?m)^(C\\d{3,})$";
const PATTERN_FULL = "regex:\u7f16\u53f7[\uff1a:]\\s*(C\\d+)";

let lastState: AppState | null = null;
let mounted = false;
let showMatched = true;
let matchQuick = true;
let matchFull = true;
function buildKeyword(): string {
  const parts: string[] = [];
  if (matchFull) parts.push(PATTERN_FULL);
  if (matchQuick) parts.push(PATTERN_QUICK);
  return parts.join("||");
}

function classifySaveResult(message: string): ExportMessageKind {
  const lower = message.toLowerCase();
  if (lower.startsWith("saved") || lower.startsWith("exported") || lower.startsWith("queued")) {
    return "success";
  }
  if (lower.includes("failed")) return "error";
  return "warn";
}

function showMonitorSaveResult(message: string, kind?: ExportMessageKind): void {
  const resolvedKind = kind ?? classifySaveResult(message);
  toast[resolvedKind](message);
}

function renderRequiredPathValidation(
  inputId: string,
  feedbackId: string,
  buttonId: string,
): boolean {
  const result = validateRequiredPath(($(inputId) as HTMLInputElement).value);
  const feedback = $(feedbackId);
  const button = $(buttonId) as HTMLButtonElement;
  button.disabled = !result.valid;
  feedback.textContent = result.valid ? "" : t("validation.required");
  feedback.className = result.valid ? "field-feedback hidden" : "field-feedback";
  return result.valid;
}

function renderMatchedList(items: [string, string][]): void {
  const copyLabel = t("monitor.copy");
  const container = $("matched-list");
  container.innerHTML = "";
  items.forEach(([time, value], index) => {
    const row = document.createElement("div");
    row.className = "item-row";
    row.innerHTML = `
      <span class="item-time">${escapeHtml(time)}</span>
      <span class="item-value">${escapeHtml(value)}</span>
      <span class="item-actions">
        <button data-copy="${escapeAttr(value)}" title="${copyLabel}">${copyLabel}</button>
        <button data-delete-matched="${index}" title="${t("monitor.delete")}">&times;</button>
      </span>`;
    container.appendChild(row);
  });
  applyTooltips(container);
}

function renderHistoryList(items: [string, string][]): void {
  const copyLabel = t("monitor.copy");
  const container = $("history-list");
  container.innerHTML = "";
  items.forEach(([time, content], index) => {
    const preview = content.split("\n")[0].substring(0, 80);
    const row = document.createElement("div");
    row.className = "history-item";
    row.innerHTML = `
      <div class="item-row">
        <span class="item-time">${escapeHtml(time)}</span>
        <span class="item-value">${escapeHtml(preview)}</span>
        <span class="item-actions">
          <button data-copy="${escapeAttr(content)}" title="${copyLabel}">${copyLabel}</button>
          <button data-delete-history="${index}" title="${t("monitor.delete")}">&times;</button>
        </span>
      </div>`;
    container.appendChild(row);
  });
  applyTooltips(container);
}

function renderToggleMatched(): void {
  const button = $("btn-toggle-matched");
  button.classList.toggle("active", showMatched);
  button.textContent = showMatched ? t("monitor.show") : t("monitor.hide");
}

export function render(state: AppState): void {
  const previousState = lastState;
  lastState = state;
  const matchedChanged = tupleListChanged(previousState?.matched ?? null, state.matched);
  const historyChanged = tupleListChanged(previousState?.history ?? null, state.history);

  const noneLabel = t("status.none");
  $("status-keyword").textContent = state.keyword || noneLabel;
  $("status-counts").textContent = `历史 ${state.history_count} · 匹配 ${state.matched_count}`;
  $("btn-toggle-always-on-top").textContent = state.always_on_top
    ? t("status.alwaysOnTopOn")
    : t("status.alwaysOnTopOff");
  $("btn-toggle-always-on-top").classList.toggle("active", state.always_on_top);

  syncInputValue("history-save-path-input", state.history_save_path);
  syncInputValue("matched-save-path-input", state.matched_save_path);
  renderRequiredPathValidation(
    "history-save-path-input",
    "history-save-path-feedback",
    "btn-apply-history-save-path",
  );
  renderRequiredPathValidation(
    "matched-save-path-input",
    "matched-save-path-feedback",
    "btn-apply-matched-save-path",
  );

  const monitorButton = $("btn-toggle-monitor");
  monitorButton.classList.toggle("active", state.monitoring);
  monitorButton.textContent = state.monitoring ? t("monitor.monitoring") : t("monitor.paused");

  $("matched-count").textContent = String(state.matched_count);
  if (showMatched && state.matched.length > 0) {
    $("matched-list").classList.remove("hidden");
    $("matched-empty").classList.add("hidden");
    if (matchedChanged) renderMatchedList(state.matched);
  } else if (state.matched.length === 0) {
    $("matched-list").classList.add("hidden");
    $("matched-empty").classList.remove("hidden");
  } else {
    $("matched-list").classList.add("hidden");
    $("matched-empty").classList.add("hidden");
  }

  if (state.history.length > 0) {
    $("latest-preview").classList.remove("hidden");
    $("history-waiting").classList.add("hidden");
    if (historyChanged) {
      const [time, content] = state.history[0];
      $("latest-time").textContent = `${t("monitor.latest")} ${time}`;
      ($("latest-content") as HTMLTextAreaElement).value = content;
    }
  } else {
    $("latest-preview").classList.add("hidden");
    $("history-waiting").classList.remove("hidden");
  }

  $("history-count-badge").textContent = String(state.history_count);
  if (state.history.length > 0) {
    $("history-list").classList.remove("hidden");
    $("history-empty").classList.add("hidden");
    if (historyChanged) renderHistoryList(state.history);
  } else {
    $("history-list").classList.add("hidden");
    $("history-empty").classList.remove("hidden");
  }

  renderToggleMatched();
}

async function saveHistory(): Promise<void> {
  try {
    const result = await invoke<string>("save_history");
    showMonitorSaveResult(result);
  } catch (error) {
    showMonitorSaveResult(errorMessage(error), "error");
  }
}

async function saveMatched(): Promise<void> {
  try {
    const result = await invoke<string>("save_matched");
    showMonitorSaveResult(result);
  } catch (error) {
    showMonitorSaveResult(errorMessage(error), "error");
  }
}

export function mount(nextContext: MonitorPageContext): void {
  if (mounted) return;
  mounted = true;

  $("btn-toggle-always-on-top").addEventListener("click", async () => {
    const next = !(lastState?.always_on_top ?? false);
    await invoke("set_window_always_on_top", { alwaysOnTop: next });
    await nextContext.refresh();
  });

  $("btn-match-quick").addEventListener("click", async () => {
    matchQuick = !matchQuick;
    $("btn-match-quick").classList.toggle("active", matchQuick);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await nextContext.refresh();
  });

  $("btn-match-full").addEventListener("click", async () => {
    matchFull = !matchFull;
    $("btn-match-full").classList.toggle("active", matchFull);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await nextContext.refresh();
  });

  $("btn-toggle-monitor").addEventListener("click", async () => {
    await invoke("toggle_monitoring");
    await nextContext.refresh();
  });

  $("btn-toggle-matched").addEventListener("click", () => {
    showMatched = !showMatched;
    renderToggleMatched();
    void nextContext.refresh();
  });

  $("btn-copy-ids").addEventListener("click", async () => {
    try {
      const ids: string[] = await invoke("get_unique_ids");
      if (ids.length === 0) {
        toast.warn(t("monitor.noMatches"));
        return;
      }
      await invoke("copy_to_clipboard", { text: ids.join("\n") });
      toast.success(t("monitor.copiedIds"));
    } catch (error) {
      toast.error(errorMessage(error));
    }
  });

  $("btn-save-history").addEventListener("click", () => {
    void saveHistory();
  });

  $("btn-apply-history-save-path").addEventListener("click", async () => {
    const path = ($("history-save-path-input") as HTMLInputElement).value;
    if (
      !renderRequiredPathValidation(
        "history-save-path-input",
        "history-save-path-feedback",
        "btn-apply-history-save-path",
      )
    )
      return;
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_history_save_path", { path });
      await nextContext.refresh();
    });
  });

  $("btn-browse-history-save-path").addEventListener("click", async () => {
    const current = ($("history-save-path-input") as HTMLInputElement).value;
    const selected = await nextContext.selectSaveFile("Choose Save History file", current);
    if (selected) {
      ($("history-save-path-input") as HTMLInputElement).value = selected;
      await nextContext.queueConfigWrite(async () => {
        await invoke("set_history_save_path", { path: selected });
        await nextContext.refresh();
      });
    }
  });

  $("btn-save-matched").addEventListener("click", () => {
    void saveMatched();
  });

  $("btn-apply-matched-save-path").addEventListener("click", async () => {
    const path = ($("matched-save-path-input") as HTMLInputElement).value;
    if (
      !renderRequiredPathValidation(
        "matched-save-path-input",
        "matched-save-path-feedback",
        "btn-apply-matched-save-path",
      )
    )
      return;
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_matched_save_path", { path });
      await nextContext.refresh();
    });
  });

  $("btn-browse-matched-save-path").addEventListener("click", async () => {
    const current = ($("matched-save-path-input") as HTMLInputElement).value;
    const selected = await nextContext.selectSaveFile("Choose Export Matched file", current);
    if (selected) {
      ($("matched-save-path-input") as HTMLInputElement).value = selected;
      await nextContext.queueConfigWrite(async () => {
        await invoke("set_matched_save_path", { path: selected });
        await nextContext.refresh();
      });
    }
  });

  $("history-save-path-input").addEventListener("input", () => {
    renderRequiredPathValidation(
      "history-save-path-input",
      "history-save-path-feedback",
      "btn-apply-history-save-path",
    );
  });
  $("matched-save-path-input").addEventListener("input", () => {
    renderRequiredPathValidation(
      "matched-save-path-input",
      "matched-save-path-feedback",
      "btn-apply-matched-save-path",
    );
  });

  $("btn-clear-all").addEventListener("click", () => {
    $("btn-clear-all").classList.add("hidden");
    $("clear-confirm").classList.remove("hidden");
  });

  $("btn-clear-confirm").addEventListener("click", async () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
    await invoke("clear_all");
    await nextContext.refresh();
  });

  $("btn-clear-cancel").addEventListener("click", () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
  });

  document.addEventListener("click", async (event) => {
    const target = event.target as HTMLElement;
    const copy = target.closest(
      "#matched-list [data-copy], #history-list [data-copy]",
    ) as HTMLElement | null;
    if (copy) {
      const value = copy.getAttribute("data-copy");
      if (value !== null) {
        try {
          await invoke("copy_to_clipboard", { text: value });
          toast.success(t("monitor.copiedIds"));
        } catch (error) {
          toast.error(errorMessage(error));
        }
      }
      return;
    }

    const deleteMatched = target.closest(
      "#matched-list [data-delete-matched]",
    ) as HTMLElement | null;
    if (deleteMatched) {
      try {
        await invoke("delete_matched", {
          index: Number.parseInt(deleteMatched.getAttribute("data-delete-matched") ?? "0", 10),
        });
        await nextContext.refresh();
      } catch (error) {
        toast.error(errorMessage(error));
      }
      return;
    }

    const deleteHistory = target.closest(
      "#history-list [data-delete-history]",
    ) as HTMLElement | null;
    if (deleteHistory) {
      try {
        await invoke("delete_history", {
          index: Number.parseInt(deleteHistory.getAttribute("data-delete-history") ?? "0", 10),
        });
        await nextContext.refresh();
      } catch (error) {
        toast.error(errorMessage(error));
      }
    }
  });

  renderToggleMatched();
}

export const monitorPage = { mount, render };
