// Settings page module.
//
// Export configuration controls live here so the export page can focus on
// running exports and rendering their progress/results.

import { invoke } from "@tauri-apps/api/core";

import { translate } from "../i18n";
import type { ModelFormat } from "../model-preview";
import { stateFieldsChanged } from "../patched-render";
import { validateHexColor, validateRequiredPath } from "../validation";
import type {
  AppState,
  Export3dPathMode,
  ExportAssetToggle,
  ExportField,
  ExportOverwriteField,
} from "../types";
import { $, parsePositiveIntOrFallback, syncInputValue, syncSelectValue } from "../utils";

const t = translate;

export interface SettingsPageContext {
  refresh: () => Promise<void>;
  selectDirectory: (title: string) => Promise<string | null>;
  queueConfigWrite: (operation: () => Promise<void>) => Promise<void>;
  onExportPathChanged: () => Promise<void>;
  clearExportNotice: () => void;
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

const settingsUi: { mode: Export3dPathMode } = {
  mode: "auto",
};

let context: SettingsPageContext | null = null;
let mounted = false;
let lastState: AppState | null = null;

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

function exportEnabled(state: AppState, field: ExportField): boolean {
  return Boolean(state[field]);
}

function exportOverwriteEnabled(state: AppState, field: ExportOverwriteField): boolean {
  return Boolean(state[field]);
}

export function hasAnyExportEnabled(state: AppState): boolean {
  return exportAssetToggles.some((toggle) => exportEnabled(state, toggle.exportField));
}

function renderExportAssetToggles(state: AppState): void {
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
}

function renderExport3dMode(): void {
  export3dModes.forEach(({ id, value }) => {
    const button = $(id) as HTMLButtonElement;
    button.classList.toggle("active", settingsUi.mode === value);
  });
}

function renderExportFillColorDraft(): void {
  const input = $("export-symbol-fill-color-input") as HTMLInputElement;
  const preview = $("export-symbol-fill-color-preview");
  const status = $("export-symbol-fill-color-status");
  const feedback = $("export-symbol-fill-color-feedback");
  const parsed = validateHexColor(input.value);

  if (!parsed.valid) {
    preview.classList.add("disabled");
    preview.setAttribute("aria-hidden", "true");
    preview.removeAttribute("style");
    status.textContent = t("export.exportFillColorAuto");
    feedback.textContent = t("validation.invalidColor");
    feedback.className = "msg msg-error";
    ($("btn-apply-export-symbol-fill-color") as HTMLButtonElement).disabled = true;
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
  ($("btn-apply-export-symbol-fill-color") as HTMLButtonElement).disabled = false;
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

function renderPathValidations(state: AppState): void {
  if (state.export_output_path !== undefined) {
    renderRequiredPathValidation(
      "export-path-input",
      "export-path-feedback",
      "btn-apply-export-path",
    );
  }
  renderRequiredPathValidation(
    "imported-parts-save-path-input",
    "imported-parts-save-path-feedback",
    "btn-apply-imported-parts-save-path",
  );
}

function syncOptionalExportState(state: AppState): void {
  const mode = normalizeExport3dPathMode(state.export_path_mode);
  if (mode) {
    settingsUi.mode = mode;
  }
}

export function render(state: AppState): void {
  const previousState = lastState;
  lastState = state;
  const pathInputsChanged = stateFieldsChanged(previousState, state, [
    "export_output_path",
    "export_parallel",
  ]);
  const colorChanged = stateFieldsChanged(previousState, state, ["export_symbol_fill_color"]);
  const modeChanged = stateFieldsChanged(previousState, state, ["export_path_mode"]);
  const formatChanged = stateFieldsChanged(previousState, state, ["default_model_format"]);
  const terminalChanged = stateFieldsChanged(previousState, state, ["export_show_terminal"]);
  const togglesChanged = stateFieldsChanged(previousState, state, [
    "export_symbol",
    "export_footprint",
    "export_model_3d",
    "export_overwrite_symbol",
    "export_overwrite_footprint",
    "export_overwrite_model_3d",
  ]);

  if (pathInputsChanged) {
    syncInputValue("export-path-input", state.export_output_path);
    syncInputValue("export-parallel-input", String(state.export_parallel));
  }
  if (colorChanged) {
    syncInputValue("export-symbol-fill-color-input", state.export_symbol_fill_color ?? "");
    renderExportFillColorDraft();
  }
  if (formatChanged) syncSelectValue("default-model-format-input", state.default_model_format);
  if (terminalChanged) {
    $("export-terminal-status").textContent = state.export_show_terminal
      ? t("export.terminalOn")
      : t("export.terminalOff");
  }
  if (togglesChanged) renderExportAssetToggles(state);
  if (modeChanged) {
    syncOptionalExportState(state);
    renderExport3dMode();
  }
  renderPathValidations(state);
}

export async function syncExportInputs(): Promise<void> {
  const path = ($("export-path-input") as HTMLInputElement).value;
  const parallelValue = ($("export-parallel-input") as HTMLInputElement).value;
  const parallel = parsePositiveIntOrFallback(parallelValue, 4);
  const colorInput = ($("export-symbol-fill-color-input") as HTMLInputElement).value;
  const parsedPath = validateRequiredPath(path);
  const parsedColor = validateHexColor(colorInput);

  if (!parsedPath.valid) throw new Error(t("validation.required"));
  if (!parsedColor.valid) {
    throw new Error(t("validation.invalidColor"));
  }

  await invoke("set_export_path", { path });
  await invoke("set_export_parallel", { parallel });
  await invoke("set_export_symbol_fill_color", { color: parsedColor.normalized });
}

async function setExport3dMode(mode: Export3dPathMode): Promise<void> {
  const currentContext = context;
  if (!currentContext) return;

  await invoke("set_export_path_mode", { pathMode: mode });
  settingsUi.mode = mode;
  renderExport3dMode();
  currentContext.clearExportNotice();
  await currentContext.refresh();
}

export function mount(nextContext: SettingsPageContext): void {
  if (mounted) return;
  mounted = true;
  context = nextContext;

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
    if (
      !renderRequiredPathValidation(
        "export-path-input",
        "export-path-feedback",
        "btn-apply-export-path",
      )
    )
      return;
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
    const parsed = validateHexColor(input.value);
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

  $("export-path-input").addEventListener("input", () => {
    renderRequiredPathValidation(
      "export-path-input",
      "export-path-feedback",
      "btn-apply-export-path",
    );
  });
  $("imported-parts-save-path-input").addEventListener("input", () => {
    renderRequiredPathValidation(
      "imported-parts-save-path-input",
      "imported-parts-save-path-feedback",
      "btn-apply-imported-parts-save-path",
    );
  });
}

export const settingsPage = {
  mount,
  render,
  syncExportInputs,
  hasAnyExportEnabled,
};
