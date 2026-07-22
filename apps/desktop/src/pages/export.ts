// Export page module.
//
// The export page owns its progress/notice state, rendering and controls. The
// root still supplies the few cross-page operations it needs so the existing
// IPC contract and imported-library invalidation flow stay unchanged.

import { invoke } from "@tauri-apps/api/core";

import { errorMessage } from "../ipc";
import { translate } from "../i18n";
import { settingsPage } from "./settings";
import type {
  AppState,
  ExportCardOptions,
  ExportFinishedPayload,
  ExportMessageKind,
  ExportNotice,
  ExportProgressPayload,
  ExportProgressState,
  ExportTool,
} from "../types";
import { $ } from "../utils";

const t = translate;

export interface ExportPageContext {
  refresh: () => Promise<void>;
  queueConfigWrite: (operation: () => Promise<void>) => Promise<void>;
}

const exportUi: Record<
  ExportTool,
  {
    progress: ExportProgressState | null;
    notice: ExportNotice | null;
    resultKind: ExportMessageKind;
  }
> = {
  export: { progress: null, notice: null, resultKind: "info" },
};

let context: ExportPageContext | null = null;
let lastState: AppState | null = null;
let mounted = false;

function toolElementId(tool: ExportTool, suffix: string): string {
  return `${tool}-${suffix}`;
}

function messageClass(kind: ExportMessageKind): string {
  switch (kind) {
    case "warn":
      return "msg-warn";
    case "success":
      return "msg-success";
    case "error":
      return "msg-error";
    default:
      return "msg-info";
  }
}

function rerender(): void {
  if (lastState) {
    settingsPage.render(lastState);
    render(lastState);
  }
}

function renderExportProgress(tool: ExportTool, running: boolean, fallbackMessage: string): void {
  const container = $(toolElementId(tool, "progress"));
  const message = $(toolElementId(tool, "progress-message"));
  const meta = $(toolElementId(tool, "progress-meta"));
  const bar = $(toolElementId(tool, "progress-bar")) as HTMLDivElement;
  const progress =
    exportUi[tool].progress ??
    (running
      ? {
          determinate: false,
          current: 0,
          total: 0,
          message: fallbackMessage,
        }
      : null);

  if (!progress) {
    container.classList.add("hidden");
    container.classList.remove("indeterminate");
    message.textContent = "";
    meta.textContent = "";
    bar.style.width = "0%";
    return;
  }

  const determinate = progress.determinate && progress.total > 0;
  const current = determinate ? Math.min(progress.current, progress.total) : 0;
  const width = determinate
    ? `${Math.max(8, Math.round((current / progress.total) * 100))}%`
    : "42%";

  container.classList.remove("hidden");
  container.classList.toggle("indeterminate", !determinate);
  message.textContent = progress.message;
  meta.textContent = determinate ? `${current}/${progress.total}` : "";
  bar.style.width = width;
}

function renderExportNotice(tool: ExportTool, derivedNotice: ExportNotice | null = null): void {
  const status = $(toolElementId(tool, "status"));
  const notice = exportUi[tool].notice ?? derivedNotice;
  if (!notice) {
    status.textContent = "";
    status.className = "msg msg-warn hidden";
    return;
  }

  status.textContent = notice.message;
  status.className = `msg ${messageClass(notice.kind)}`;
}

function renderExportResult(
  tool: ExportTool,
  result: string | null,
  busy: boolean,
  derivedNotice: ExportNotice | null = null,
): void {
  const resultBox = $(toolElementId(tool, "result"));
  if (!result || busy || exportUi[tool].notice !== null || derivedNotice !== null) {
    resultBox.textContent = "";
    resultBox.className = "msg msg-info hidden";
    return;
  }

  resultBox.textContent = result;
  resultBox.className = `msg ${messageClass(exportUi[tool].resultKind)}`;
}

function renderExporterCard(options: ExportCardOptions): void {
  $(options.countId).textContent = `${options.matchedCount} ${t("export.itemsReady")}`;

  const busy = options.running || exportUi[options.tool].progress !== null;
  const button = $(options.buttonId) as HTMLButtonElement;
  button.disabled = options.matchedCount === 0 || busy || Boolean(options.buttonDisabled);
  button.textContent = busy ? t("export.running") : t(options.exportLabelKey);

  renderExportProgress(options.tool, busy, t(options.runningLabelKey));
  renderExportNotice(options.tool, options.derivedNotice ?? null);
  renderExportResult(options.tool, options.result, busy, options.derivedNotice ?? null);
}

function syncExportProgressWithState(state: AppState): void {
  if (!state.export_running && exportUi.export.progress !== null) {
    exportUi.export.progress = null;
  }
}

export function render(state: AppState): void {
  lastState = state;
  syncExportProgressWithState(state);

  const exportHasExportSelection = settingsPage.hasAnyExportEnabled(state);

  renderExporterCard({
    tool: "export",
    countId: "export-count",
    buttonId: "btn-export",
    matchedCount: state.matched_count,
    running: state.export_running,
    exportLabelKey: "export.export",
    runningLabelKey: "export.exportRunning",
    statusId: "export-status",
    resultId: "export-result",
    result: state.export_last_result,
    buttonDisabled: !exportHasExportSelection,
    derivedNotice: exportHasExportSelection
      ? null
      : {
          kind: "warn",
          message: t("export.exportSelectAtLeastOne"),
        },
  });
}

function setExportNotice(message: string | null, kind: ExportMessageKind = "warn"): void {
  exportUi.export.notice = message ? { kind, message } : null;
  rerender();
}

function startExportProgress(message: string): void {
  exportUi.export.notice = null;
  exportUi.export.progress = {
    determinate: false,
    current: 0,
    total: 0,
    message,
  };
  exportUi.export.resultKind = "info";
  rerender();
}

function updateExportProgress(payload: ExportProgressPayload): void {
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].progress = {
    determinate: payload.determinate,
    current: payload.current ?? 0,
    total: payload.total ?? 0,
    message: payload.message,
  };
  rerender();
}

function finishExportProgress(payload: ExportFinishedPayload): void {
  exportUi[payload.tool].progress = null;
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].resultKind = payload.success ? "success" : "error";
  rerender();
}

function showExportStartResult(result: string): boolean {
  if (result === "Export started") {
    setExportNotice(null);
    return true;
  }

  exportUi.export.progress = null;
  exportUi.export.notice = { kind: "warn", message: result };
  rerender();
  return false;
}

function showExportError(error: string): void {
  exportUi.export.progress = null;
  exportUi.export.notice = { kind: "error", message: error };
  rerender();
}

async function exportComponents(): Promise<void> {
  if (lastState && !settingsPage.hasAnyExportEnabled(lastState)) {
    setExportNotice(t("export.selectAtLeastOne"));
    return;
  }

  const currentContext = context;
  if (!currentContext) return;

  try {
    await currentContext.queueConfigWrite(async () => {
      await settingsPage.syncExportInputs();
      await currentContext.refresh();
    });
    startExportProgress(t("export.exportRunning"));
    const result = await invoke<string>("export");
    showExportStartResult(result);
    await currentContext.refresh();
  } catch (error) {
    showExportError(errorMessage(error));
    await currentContext.refresh();
  }
}

export function mount(nextContext: ExportPageContext): void {
  if (mounted) return;
  mounted = true;
  context = nextContext;

  $("btn-export").addEventListener("click", () => {
    void exportComponents();
  });
}

export function onEvent(
  name: "export-progress" | "export-finished",
  payload: ExportProgressPayload | ExportFinishedPayload,
): void {
  if (name === "export-progress") {
    updateExportProgress(payload as ExportProgressPayload);
  } else {
    finishExportProgress(payload as ExportFinishedPayload);
  }
}

export function clearNotice(): void {
  setExportNotice(null);
}

export const exportPage = { mount, render, onEvent, clearNotice };
