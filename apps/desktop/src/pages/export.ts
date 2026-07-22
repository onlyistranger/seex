// Export page module.
//
// The export page owns its progress/notice state, rendering and controls. The
// root still supplies the few cross-page operations it needs so the existing
// IPC contract and imported-library invalidation flow stay unchanged.

import { invoke } from "@tauri-apps/api/core";

import { errorMessage } from "../ipc";
import { translate } from "../i18n";
import type { ModelFormat } from "../model-preview";
import type {
  AppState,
  Export3dPathMode,
  ExportAssetToggle,
  ExportCardOptions,
  ExportField,
  ExportFinishedPayload,
  ExportMessageKind,
  ExportNotice,
  ExportOverwriteField,
  ExportProgressPayload,
  ExportProgressState,
  ExportTool,
} from "../types";
import {
  $,
  parseOptionalHexColor,
  parsePositiveIntOrFallback,
  syncInputValue,
  syncSelectValue,
} from "../utils";

const t = translate;

export interface ExportPageContext {
  refresh: () => Promise<void>;
  selectDirectory: (title: string) => Promise<string | null>;
  queueConfigWrite: (operation: () => Promise<void>) => Promise<void>;
  onExportPathChanged: () => Promise<void>;
}

const export3dModes: { id: string; value: Export3dPathMode }[] = [
  { id: "btn-export-3d-mode-auto", value: "auto" },
  { id: "btn-export-3d-mode-project", value: "project_relative" },
  { id: "btn-export-3d-mode-library", value: "library_relative" },
];

const exportAssetToggles: ExportAssetToggle[] = [
  {
    key: "symbol",
    labelKey: "export.exportAssetSymbol",
    exportField: "export_symbol",
    overwriteField: "export_overwrite_symbol",
    exportButtonId: "btn-toggle-export-symbol",
    overwriteButtonId: "btn-toggle-export-overwrite-symbol",
    exportCommand: "set_export_symbol",
    overwriteCommand: "set_export_overwrite_symbol",
  },
  {
    key: "footprint",
    labelKey: "export.exportAssetFootprint",
    exportField: "export_footprint",
    overwriteField: "export_overwrite_footprint",
    exportButtonId: "btn-toggle-export-footprint",
    overwriteButtonId: "btn-toggle-export-overwrite-footprint",
    exportCommand: "set_export_footprint",
    overwriteCommand: "set_export_overwrite_footprint",
  },
  {
    key: "model_3d",
    labelKey: "export.exportAssetModel3d",
    exportField: "export_model_3d",
    overwriteField: "export_overwrite_model_3d",
    exportButtonId: "btn-toggle-export-model-3d",
    overwriteButtonId: "btn-toggle-export-overwrite-model-3d",
    exportCommand: "set_export_model_3d",
    overwriteCommand: "set_export_overwrite_model_3d",
  },
];

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

const exportUiState: { mode: Export3dPathMode } = {
  mode: "auto",
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

function normalizeExport3dPathMode(value: unknown): Export3dPathMode | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase().replace(/[-\s]/g, "_");
  if (normalized === "auto") return "auto";
  if (["project_relative", "project", "kicad_project"].includes(normalized)) {
    return "project_relative";
  }
  if (["library_relative", "library", "relative"].includes(normalized)) {
    return "library_relative";
  }
  return null;
}

function rerender(): void {
  if (lastState) {
    render(lastState);
  }
}

function exportEnabled(state: AppState, field: ExportField): boolean {
  return Boolean(state[field]);
}

function exportOverwriteEnabled(state: AppState, field: ExportOverwriteField): boolean {
  return Boolean(state[field]);
}

function hasAnyExportEnabled(state: AppState): boolean {
  return exportAssetToggles.some((toggle) => exportEnabled(state, toggle.exportField));
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

function renderExportAssetToggles(state: AppState): boolean {
  const anyExportEnabled = hasAnyExportEnabled(state);

  exportAssetToggles.forEach((toggle) => {
    const exportButton = $(toggle.exportButtonId) as HTMLButtonElement;
    const overwriteButton = $(toggle.overwriteButtonId) as HTMLButtonElement;
    const exportEnabledForAsset = exportEnabled(state, toggle.exportField);
    const overwriteEnabled =
      exportEnabledForAsset && exportOverwriteEnabled(state, toggle.overwriteField);

    exportButton.classList.toggle("active", exportEnabledForAsset);
    exportButton.setAttribute("aria-pressed", String(exportEnabledForAsset));

    overwriteButton.classList.toggle("active", overwriteEnabled);
    overwriteButton.disabled = !exportEnabledForAsset;
    overwriteButton.setAttribute("aria-pressed", String(overwriteEnabled));
  });

  return anyExportEnabled;
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

function syncOptionalExportState(state: AppState): void {
  const mode = normalizeExport3dPathMode(state.export_path_mode);
  if (mode) {
    exportUiState.mode = mode;
  }
}

function renderExport3dMode(): void {
  export3dModes.forEach(({ id, value }) => {
    const button = $(id) as HTMLButtonElement;
    button.classList.toggle("active", exportUiState.mode === value);
  });
}

function renderExportFillColorDraft(): void {
  const input = $("export-symbol-fill-color-input") as HTMLInputElement;
  const preview = $("export-symbol-fill-color-preview");
  const status = $("export-symbol-fill-color-status");
  const feedback = $("export-symbol-fill-color-feedback");
  const parsed = parseOptionalHexColor(input.value);

  if (!parsed.valid) {
    preview.classList.add("disabled");
    preview.setAttribute("aria-hidden", "true");
    preview.removeAttribute("style");
    status.textContent = t("export.exportFillColorAuto");
    feedback.textContent = t("export.exportFillColorInvalid");
    feedback.className = "msg msg-error";
    return;
  }

  feedback.textContent = "";
  feedback.className = "msg msg-error hidden";
  if (parsed.normalized) {
    preview.classList.remove("disabled");
    preview.setAttribute("aria-hidden", "false");
    preview.style.background = parsed.normalized;
    status.textContent = parsed.normalized;
  } else {
    preview.classList.add("disabled");
    preview.setAttribute("aria-hidden", "true");
    preview.removeAttribute("style");
    status.textContent = t("export.exportFillColorAuto");
  }
}

export function render(state: AppState): void {
  lastState = state;
  syncOptionalExportState(state);
  syncExportProgressWithState(state);

  syncInputValue("export-path-input", state.export_output_path);
  syncInputValue("export-parallel-input", String(state.export_parallel));
  syncInputValue("export-symbol-fill-color-input", state.export_symbol_fill_color ?? "");
  syncSelectValue("default-model-format-input", state.default_model_format);

  $("export-terminal-status").textContent = state.export_show_terminal
    ? t("export.terminalOn")
    : t("export.terminalOff");
  const exportHasExportSelection = renderExportAssetToggles(state);
  renderExport3dMode();
  renderExportFillColorDraft();

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

async function syncExportInputs(): Promise<void> {
  const path = ($("export-path-input") as HTMLInputElement).value;
  const parallelValue = ($("export-parallel-input") as HTMLInputElement).value;
  const parallel = parsePositiveIntOrFallback(parallelValue, 4);
  const colorInput = ($("export-symbol-fill-color-input") as HTMLInputElement).value;
  const parsedColor = parseOptionalHexColor(colorInput);

  if (!parsedColor.valid) {
    throw new Error(t("export.exportFillColorInvalid"));
  }

  await invoke("set_export_path", { path });
  await invoke("set_export_parallel", { parallel });
  await invoke("set_export_symbol_fill_color", { color: parsedColor.normalized });
}

async function setExport3dMode(mode: Export3dPathMode): Promise<void> {
  const currentContext = context;
  if (!currentContext) return;

  await invoke("set_export_path_mode", { pathMode: mode });
  exportUiState.mode = mode;
  setExportNotice(null);
  await currentContext.refresh();
}

async function exportComponents(): Promise<void> {
  if (lastState && !hasAnyExportEnabled(lastState)) {
    setExportNotice(t("export.selectAtLeastOne"));
    return;
  }

  const currentContext = context;
  if (!currentContext) return;

  try {
    await currentContext.queueConfigWrite(async () => {
      await syncExportInputs();
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

  $("btn-browse-export-folder").addEventListener("click", async () => {
    const selected = await nextContext.selectDirectory("Select export directory");
    if (!selected) return;

    ($("export-path-input") as HTMLInputElement).value = selected;
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_export_path", { path: selected });
      await nextContext.onExportPathChanged();
    });
  });

  $("btn-apply-export-path").addEventListener("click", async () => {
    const path = ($("export-path-input") as HTMLInputElement).value;
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_export_path", { path });
      await nextContext.onExportPathChanged();
    });
  });

  $("btn-toggle-export-terminal").addEventListener("click", async () => {
    await nextContext.queueConfigWrite(async () => {
      await invoke("toggle_export_terminal");
      await nextContext.refresh();
    });
  });

  exportAssetToggles.forEach((toggle) => {
    $(toggle.exportButtonId).addEventListener("click", async () => {
      const active = $(toggle.exportButtonId).classList.contains("active");
      await nextContext.queueConfigWrite(async () => {
        await invoke(toggle.exportCommand, { enabled: !active });
        if (active) {
          await invoke(toggle.overwriteCommand, { overwrite: false });
        }
        await nextContext.refresh();
      });
    });

    $(toggle.overwriteButtonId).addEventListener("click", async () => {
      const button = $(toggle.overwriteButtonId) as HTMLButtonElement;
      if (button.disabled) return;

      const active = button.classList.contains("active");
      await nextContext.queueConfigWrite(async () => {
        await invoke(toggle.overwriteCommand, { overwrite: !active });
        await nextContext.refresh();
      });
    });
  });

  export3dModes.forEach(({ id, value }) => {
    $(id).addEventListener("click", async () => {
      await nextContext.queueConfigWrite(async () => {
        await setExport3dMode(value);
      });
    });
  });

  $("btn-apply-default-model-format").addEventListener("click", async () => {
    const format = ($("default-model-format-input") as HTMLSelectElement).value as ModelFormat;
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_default_model_format", { format });
      await nextContext.refresh();
    });
  });

  $("btn-apply-export-parallel").addEventListener("click", async () => {
    const value = ($("export-parallel-input") as HTMLInputElement).value;
    const parallel = parsePositiveIntOrFallback(value, 4);
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_export_parallel", { parallel });
      await nextContext.refresh();
    });
  });

  $("btn-apply-export-symbol-fill-color").addEventListener("click", async () => {
    const input = $("export-symbol-fill-color-input") as HTMLInputElement;
    const parsed = parseOptionalHexColor(input.value);
    renderExportFillColorDraft();
    if (!parsed.valid) return;

    await nextContext.queueConfigWrite(async () => {
      await invoke("set_export_symbol_fill_color", { color: parsed.normalized });
      await nextContext.refresh();
    });
  });

  $("btn-clear-export-symbol-fill-color").addEventListener("click", async () => {
    const input = $("export-symbol-fill-color-input") as HTMLInputElement;
    input.value = "";
    renderExportFillColorDraft();
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_export_symbol_fill_color", { color: null });
      await nextContext.refresh();
    });
  });

  $("export-symbol-fill-color-input").addEventListener("input", () => {
    renderExportFillColorDraft();
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

export const exportPage = { mount, render, onEvent };
