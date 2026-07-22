// Shared 3D model preview subsystem for the desktop frontend.
//
// Both the Library page and the Inventory page surface STEP/WRL model
// previews: an inline hover popover (importedPreviewUi) and a standalone modal
// (importedStandalonePreviewUi). The state and the nine render/close/load/
// schedule/open functions used to live inline in `main.ts`; they have no
// page-specific behaviour, so they live here. Callers are expected to wire two
// pieces of context that the preview subsystem cannot know about on its own:
//
//  - `setPreviewContext({ getDefaultModelFormat })` — picks the preferred
//    model when several are present. Defaults to WRL (`"wrl"`) until the root
//    state is available.
//  - `setNoModelsHandler(handler)` — invoked when a row asks to preview a part
//    that has no models. The default is a no-op; the Library page sets this to
//    surface a warning toast and re-render its panel.
//
// All DOM element ids (`imported-preview-popover`, `imported-preview-canvas`,
// `imported-standalone-preview-modal`, ...) are stable and owned by
// `index.html`; this module only reads/writes them.

import { invoke } from "@tauri-apps/api/core";

import { ModelPreviewViewer, type ModelFormat } from "../model-preview";
import { errorMessage } from "../ipc";
import { $ } from "../utils";
import { translate } from "../i18n";
import type { ImportedSymbol } from "../types";

const t = translate;

export function formatModelSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${(sizeBytes / 1024).toFixed(1)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function importedRowKey(item: ImportedSymbol): string {
  return item.library_key || `${item.source_file}\u001f${item.symbol_name}\u001f${item.lcsc_part}`;
}

export interface PreviewContext {
  /** Resolve the preferred model format when a part ships several models. */
  getDefaultModelFormat: () => ModelFormat;
}

let getDefaultModelFormat: () => ModelFormat = () => "wrl";

let onNoModels: () => void = () => {};

export function setPreviewContext(ctx: PreviewContext): void {
  getDefaultModelFormat = ctx.getDefaultModelFormat;
}

export function setNoModelsHandler(handler: () => void): void {
  onNoModels = handler;
}

const importedPreviewUi: {
  itemKey: string | null;
  item: ImportedSymbol | null;
  fileName: string;
  loading: boolean;
  error: string | null;
} = {
  itemKey: null,
  item: null,
  fileName: "",
  loading: false,
  error: null,
};

let importedPreviewViewer: ModelPreviewViewer | null = null;
let importedPreviewHoverTimer: number | null = null;
let importedPreviewRow: HTMLElement | null = null;

const importedStandalonePreviewUi: {
  itemKey: string | null;
  item: ImportedSymbol | null;
  fileName: string;
  loading: boolean;
  error: string | null;
} = {
  itemKey: null,
  item: null,
  fileName: "",
  loading: false,
  error: null,
};

let importedStandalonePreviewViewer: ModelPreviewViewer | null = null;

export function renderImportedPreview() {
  const popover = $("imported-preview-popover");
  const item = importedPreviewUi.item;
  if (!item) {
    popover.classList.add("hidden");
    popover.setAttribute("aria-hidden", "true");
    return;
  }

  popover.classList.remove("hidden");
  popover.setAttribute("aria-hidden", "false");
  $("imported-preview-title").textContent = `${t("imported.previewTitle")} · ${item.symbol_name}`;
  $("imported-preview-meta").textContent = `${item.lcsc_part} · ${item.source_file}`;
  const model =
    item.models.find((candidate) => candidate.file_name === importedPreviewUi.fileName) ??
    item.models[0];
  $("imported-preview-format").textContent =
    `${model.format.toUpperCase()} · ${formatModelSize(model.size_bytes)}`;

  const status = $("imported-preview-status");
  if (importedPreviewUi.loading) {
    status.textContent = t("imported.previewLoading");
    status.className = "model-preview-status";
  } else if (importedPreviewUi.error) {
    status.textContent = `${t("imported.previewError")} ${importedPreviewUi.error}`;
    status.className = "model-preview-status model-preview-status-error";
  } else {
    status.textContent = "";
    status.className = "model-preview-status hidden";
  }

  positionImportedPreview();
}

export function closeImportedPreview() {
  if (importedPreviewHoverTimer !== null) {
    window.clearTimeout(importedPreviewHoverTimer);
    importedPreviewHoverTimer = null;
  }
  importedPreviewViewer?.dispose();
  importedPreviewViewer = null;
  importedPreviewRow = null;
  importedPreviewUi.itemKey = null;
  importedPreviewUi.item = null;
  importedPreviewUi.fileName = "";
  importedPreviewUi.loading = false;
  importedPreviewUi.error = null;
  renderImportedPreview();
}

export function positionImportedPreview() {
  const popover = $("imported-preview-popover");
  if (!importedPreviewUi.item || !importedPreviewRow || popover.classList.contains("hidden")) {
    return;
  }

  const rowRect = importedPreviewRow.getBoundingClientRect();
  const popoverRect = popover.getBoundingClientRect();
  const margin = 12;
  const gap = 12;
  let left = rowRect.right + gap;
  if (left + popoverRect.width > window.innerWidth - margin) {
    left = rowRect.left - popoverRect.width - gap;
  }
  left = Math.max(margin, Math.min(left, window.innerWidth - popoverRect.width - margin));

  let top = rowRect.top;
  if (top + popoverRect.height > window.innerHeight - margin) {
    top = window.innerHeight - popoverRect.height - margin;
  }
  top = Math.max(margin, top);

  popover.style.left = `${left}px`;
  popover.style.top = `${top}px`;
}

export async function loadImportedPreviewModel() {
  const item = importedPreviewUi.item;
  const fileName = importedPreviewUi.fileName;
  const viewer = importedPreviewViewer;
  if (!item || !fileName || !viewer) {
    return;
  }

  const itemKey = importedPreviewUi.itemKey;
  const model = item.models.find((candidate) => candidate.file_name === fileName);
  if (!model) {
    importedPreviewUi.error = t("imported.previewNoModels");
    renderImportedPreview();
    return;
  }

  importedPreviewUi.loading = true;
  importedPreviewUi.error = null;
  renderImportedPreview();

  try {
    const bytes = await invoke<number[]>("read_imported_model", {
      request: {
        source_file: item.source_file,
        lcsc_part: item.lcsc_part,
        file_name: model.file_name,
      },
    });
    if (importedPreviewUi.itemKey !== itemKey || importedPreviewViewer !== viewer) {
      return;
    }
    await viewer.load(model.format, new Uint8Array(bytes));
  } catch (error) {
    if (importedPreviewUi.itemKey === itemKey && importedPreviewViewer === viewer) {
      importedPreviewUi.error = errorMessage(error);
    }
  } finally {
    if (importedPreviewUi.itemKey === itemKey && importedPreviewViewer === viewer) {
      importedPreviewUi.loading = false;
      renderImportedPreview();
    }
  }
}

export function scheduleImportedPreview(item: ImportedSymbol, row: HTMLElement) {
  if (item.models.length === 0) {
    closeImportedPreview();
    return;
  }

  const itemKey = importedRowKey(item);
  if (importedPreviewUi.itemKey !== itemKey) {
    closeImportedPreview();
    importedPreviewRow = row;
    importedPreviewUi.itemKey = itemKey;
    importedPreviewUi.item = item;
    const preferredModel = item.models.find((model) => model.format === getDefaultModelFormat());
    importedPreviewUi.fileName = (preferredModel ?? item.models[0]).file_name;
    importedPreviewUi.loading = true;
    importedPreviewUi.error = null;
    renderImportedPreview();

    try {
      importedPreviewViewer = new ModelPreviewViewer($("imported-preview-canvas"));
    } catch (error) {
      importedPreviewUi.loading = false;
      importedPreviewUi.error = errorMessage(error);
      renderImportedPreview();
      return;
    }
  } else {
    importedPreviewRow = row;
    positionImportedPreview();
  }

  if (importedPreviewHoverTimer !== null) {
    window.clearTimeout(importedPreviewHoverTimer);
  }
  importedPreviewHoverTimer = window.setTimeout(() => {
    importedPreviewHoverTimer = null;
    void loadImportedPreviewModel();
  }, 120);
}

export function renderImportedStandalonePreview() {
  const modal = $("imported-standalone-preview-modal");
  const item = importedStandalonePreviewUi.item;
  if (!item) {
    modal.classList.add("hidden");
    modal.setAttribute("aria-hidden", "true");
    return;
  }

  modal.classList.remove("hidden");
  modal.setAttribute("aria-hidden", "false");
  $("imported-standalone-preview-title").textContent =
    `${t("imported.previewTitle")} · ${item.symbol_name}`;
  $("imported-standalone-preview-meta").textContent = `${item.lcsc_part} · ${item.source_file}`;

  const select = $("imported-standalone-preview-model-select") as HTMLSelectElement;
  select.replaceChildren();
  item.models.forEach((model) => {
    const option = document.createElement("option");
    option.value = model.file_name;
    option.textContent = `${model.file_name} (${formatModelSize(model.size_bytes)})`;
    option.selected = model.file_name === importedStandalonePreviewUi.fileName;
    select.appendChild(option);
  });
  select.disabled = importedStandalonePreviewUi.loading || item.models.length < 2;

  const status = $("imported-standalone-preview-status");
  if (importedStandalonePreviewUi.loading) {
    status.textContent = t("imported.previewLoading");
    status.className = "model-preview-status";
  } else if (importedStandalonePreviewUi.error) {
    status.textContent = `${t("imported.previewError")} ${importedStandalonePreviewUi.error}`;
    status.className = "model-preview-status model-preview-status-error";
  } else {
    status.textContent = "";
    status.className = "model-preview-status hidden";
  }

  ($("btn-reset-imported-standalone-preview") as HTMLButtonElement).disabled =
    importedStandalonePreviewUi.loading;
}

export function closeImportedStandalonePreview() {
  importedStandalonePreviewViewer?.dispose();
  importedStandalonePreviewViewer = null;
  importedStandalonePreviewUi.itemKey = null;
  importedStandalonePreviewUi.item = null;
  importedStandalonePreviewUi.fileName = "";
  importedStandalonePreviewUi.loading = false;
  importedStandalonePreviewUi.error = null;
  renderImportedStandalonePreview();
}

export async function loadImportedStandalonePreviewModel() {
  const item = importedStandalonePreviewUi.item;
  const fileName = importedStandalonePreviewUi.fileName;
  const viewer = importedStandalonePreviewViewer;
  if (!item || !fileName || !viewer) {
    return;
  }

  const itemKey = importedStandalonePreviewUi.itemKey;
  const model = item.models.find((candidate) => candidate.file_name === fileName);
  if (!model) {
    importedStandalonePreviewUi.error = t("imported.previewNoModels");
    renderImportedStandalonePreview();
    return;
  }

  importedStandalonePreviewUi.loading = true;
  importedStandalonePreviewUi.error = null;
  renderImportedStandalonePreview();

  try {
    const bytes = await invoke<number[]>("read_imported_model", {
      request: {
        source_file: item.source_file,
        lcsc_part: item.lcsc_part,
        file_name: model.file_name,
      },
    });
    if (
      importedStandalonePreviewUi.itemKey !== itemKey ||
      importedStandalonePreviewViewer !== viewer
    ) {
      return;
    }
    await viewer.load(model.format, new Uint8Array(bytes));
  } catch (error) {
    if (
      importedStandalonePreviewUi.itemKey === itemKey &&
      importedStandalonePreviewViewer === viewer
    ) {
      importedStandalonePreviewUi.error = errorMessage(error);
    }
  } finally {
    if (
      importedStandalonePreviewUi.itemKey === itemKey &&
      importedStandalonePreviewViewer === viewer
    ) {
      importedStandalonePreviewUi.loading = false;
      renderImportedStandalonePreview();
    }
  }
}

export async function openImportedStandalonePreview(item: ImportedSymbol) {
  if (item.models.length === 0) {
    onNoModels();
    return;
  }

  closeImportedPreview();
  closeImportedStandalonePreview();
  importedStandalonePreviewUi.itemKey = importedRowKey(item);
  importedStandalonePreviewUi.item = item;
  const preferredModel = item.models.find((model) => model.format === getDefaultModelFormat());
  importedStandalonePreviewUi.fileName = (preferredModel ?? item.models[0]).file_name;
  importedStandalonePreviewUi.loading = true;
  importedStandalonePreviewUi.error = null;
  renderImportedStandalonePreview();

  try {
    importedStandalonePreviewViewer = new ModelPreviewViewer(
      $("imported-standalone-preview-canvas"),
    );
    await loadImportedStandalonePreviewModel();
  } catch (error) {
    importedStandalonePreviewUi.loading = false;
    importedStandalonePreviewUi.error = errorMessage(error);
    renderImportedStandalonePreview();
  }
}

/** Reset the camera of the currently open standalone viewer. */
export function resetImportedStandalonePreview(): void {
  importedStandalonePreviewViewer?.resetView();
}

/** Select another model in the open standalone preview and load it. */
export async function selectImportedStandalonePreviewModel(fileName: string): Promise<void> {
  if (!importedStandalonePreviewUi.item) {
    return;
  }
  importedStandalonePreviewUi.fileName = fileName;
  await loadImportedStandalonePreviewModel();
}
