import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ModelPreviewViewer, type ModelFormat } from "./model-preview";
import { mountIcons } from "./icons";
import { errorMessage, invokeState } from "./ipc";
import { translate } from "./i18n";
import {
  $,
  applyTooltips,
  escapeAttr,
  escapeHtml,
  formatMessage,
  parseOptionalHexColor,
  parsePositiveIntOrFallback,
  syncInputValue,
  syncSelectValue,
} from "./utils";
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
  ImportBomResult,
  ImportedSymbol,
  ImportedSymbolsResponse,
  InventoryAllocation,
  InventoryLocation,
  InventoryPart,
  InventoryResponse,
  LibrarySource,
  PageName,
  ProductionRecord,
  BomDeductionRow,
  BomImportRow,
  BomPreview,
  BomPreviewRow,
} from "./types";

const browserPreviewMode = !("__TAURI_INTERNALS__" in window);


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

let currentPage: PageName = "monitor";
let showMatched = true;
let matchQuick = true;
let matchFull = true;
let lastState: AppState | null = null;

const exportUi: Record<ExportTool, { progress: ExportProgressState | null; notice: ExportNotice | null; resultKind: ExportMessageKind }> = {
  export: { progress: null, notice: null, resultKind: "info" },
};

const exportUiState: {
  mode: Export3dPathMode;
} = {
  mode: "auto",
};

const importedUi: {
  loading: boolean;
  busy: boolean;
  initialized: boolean;
  scannedPath: string;
  sources: LibrarySource[];
  sourceFilter: string;
  items: ImportedSymbol[];
  error: string | null;
  notice: ExportNotice | null;
  query: string;
  selectedKeys: Set<string>;
  editingKey: string | null;
  editDraftSymbolName: string;
  editDraftLcscPart: string;
  editDraftSourceFile: string;
} = {
  loading: false,
  busy: false,
  initialized: false,
  scannedPath: "",
  sources: [],
  sourceFilter: "",
  items: [],
  error: null,
  notice: null,
  query: "",
  selectedKeys: new Set(),
  editingKey: null,
  editDraftSymbolName: "",
  editDraftLcscPart: "",
  editDraftSourceFile: "",
};

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

const inventoryUi: {
  initialized: boolean;
  loading: boolean;
  busy: boolean;
  query: string;
  revision: number;
  parts: InventoryPart[];
  allParts: InventoryPart[];
  error: string | null;
  notice: ExportNotice | null;
  editingId: string | null;
  draftSupplier: string;
  draftLibraryLcsc: string;
  draftLibrarySymbolName: string;
  draftLibrarySourceFile: string;
  draftLibraryMissing: boolean;
  draftName: string;
  draftPackage: string;
  draftNote: string;
  draftLocations: InventoryLocation[];
  bomPath: string;
  bomBoards: string;
  bomLoading: boolean;
  bomPreview: BomPreview | null;
  bomSkipped: Set<number>;
  bomLibrarySelections: Record<number, string>;
  bomError: string | null;
  productionRecords: ProductionRecord[];
  libraryItems: ImportedSymbol[];
  libraryPickerOpen: boolean;
  libraryPickerLoading: boolean;
  libraryPickerQuery: string;
} = {
  initialized: false,
  loading: false,
  busy: false,
  query: "",
  revision: 0,
  parts: [],
  allParts: [],
  error: null,
  notice: null,
  editingId: null,
  draftSupplier: "",
  draftLibraryLcsc: "",
  draftLibrarySymbolName: "",
  draftLibrarySourceFile: "",
  draftLibraryMissing: false,
  draftName: "",
  draftPackage: "",
  draftNote: "",
  draftLocations: [],
  bomPath: "",
  bomBoards: "1",
  bomLoading: false,
  bomPreview: null,
  bomSkipped: new Set(),
  bomLibrarySelections: {},
  bomError: null,
  productionRecords: [],
  libraryItems: [],
  libraryPickerOpen: false,
  libraryPickerLoading: false,
  libraryPickerQuery: "",
};

const PATTERN_QUICK = "regex:(?m)^(C\\d{3,})$";
const PATTERN_FULL = "regex:\u7f16\u53f7[\uff1a:]\\s*(C\\d+)";

function normalizeExport3dPathMode(value: unknown): Export3dPathMode | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase().replace(/[-\s]/g, "_");
  if (normalized === "auto") return "auto";
  if (["project_relative", "project", "kicad_project"].includes(normalized)) return "project_relative";
  if (["library_relative", "library", "relative"].includes(normalized)) return "library_relative";
  return null;
}

function t(key: string): string {
  return translate(key);
}

function normalizeImportedLcscPart(value: string): string {
  return value.trim().toUpperCase();
}

function importedRowKey(item: ImportedSymbol): string {
  return item.library_key || `${item.source_file}\u001f${item.symbol_name}\u001f${item.lcsc_part}`;
}

function inventoryLibraryItem(part: InventoryPart): ImportedSymbol | null {
  const lcscPart = part.library_lcsc;
  if (!lcscPart && (!part.library_source_file || !part.library_symbol_name)) return null;
  return (
    (lcscPart ? inventoryUi.libraryItems.find(
      (item) => item.lcsc_part === lcscPart && item.source_file === part.library_source_file,
    ) ?? inventoryUi.libraryItems.find((item) => item.lcsc_part === lcscPart) : null) ??
    inventoryUi.libraryItems.find((item) => item.source_file === part.library_source_file && item.symbol_name === part.library_symbol_name) ??
    null
  );
}

function formatModelSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${(sizeBytes / 1024).toFixed(1)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function librarySourceLabel(kind: string): string {
  return ({
    export: "export",
    kicad_standard: "KiCad 标准库",
    project: "项目库",
    external: "外部库",
  } as Record<string, string>)[kind] ?? kind;
}

function renderImportedPreview() {
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
  const model = item.models.find((candidate) => candidate.file_name === importedPreviewUi.fileName) ?? item.models[0];
  $("imported-preview-format").textContent = `${model.format.toUpperCase()} · ${formatModelSize(model.size_bytes)}`;

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

function closeImportedPreview() {
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

function positionImportedPreview() {
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

async function loadImportedPreviewModel() {
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

function scheduleImportedPreview(item: ImportedSymbol, row: HTMLElement) {
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
    const preferredModel = item.models.find(
      (model) => model.format === (lastState?.default_model_format ?? "wrl"),
    );
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

function renderImportedStandalonePreview() {
  const modal = $("imported-standalone-preview-modal");
  const item = importedStandalonePreviewUi.item;
  if (!item) {
    modal.classList.add("hidden");
    modal.setAttribute("aria-hidden", "true");
    return;
  }

  modal.classList.remove("hidden");
  modal.setAttribute("aria-hidden", "false");
  $("imported-standalone-preview-title").textContent = `${t("imported.previewTitle")} · ${item.symbol_name}`;
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

  ($("btn-reset-imported-standalone-preview") as HTMLButtonElement).disabled = importedStandalonePreviewUi.loading;
}

function closeImportedStandalonePreview() {
  importedStandalonePreviewViewer?.dispose();
  importedStandalonePreviewViewer = null;
  importedStandalonePreviewUi.itemKey = null;
  importedStandalonePreviewUi.item = null;
  importedStandalonePreviewUi.fileName = "";
  importedStandalonePreviewUi.loading = false;
  importedStandalonePreviewUi.error = null;
  renderImportedStandalonePreview();
}

async function loadImportedStandalonePreviewModel() {
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
    if (importedStandalonePreviewUi.itemKey !== itemKey || importedStandalonePreviewViewer !== viewer) {
      return;
    }
    await viewer.load(model.format, new Uint8Array(bytes));
  } catch (error) {
    if (importedStandalonePreviewUi.itemKey === itemKey && importedStandalonePreviewViewer === viewer) {
      importedStandalonePreviewUi.error = errorMessage(error);
    }
  } finally {
    if (importedStandalonePreviewUi.itemKey === itemKey && importedStandalonePreviewViewer === viewer) {
      importedStandalonePreviewUi.loading = false;
      renderImportedStandalonePreview();
    }
  }
}

async function openImportedStandalonePreview(item: ImportedSymbol) {
  if (item.models.length === 0) {
    importedUi.notice = { kind: "warn", message: t("imported.previewNoModels") };
    renderImportedPanel();
    return;
  }

  closeImportedPreview();
  closeImportedStandalonePreview();
  importedStandalonePreviewUi.itemKey = importedRowKey(item);
  importedStandalonePreviewUi.item = item;
  const preferredModel = item.models.find(
    (model) => model.format === (lastState?.default_model_format ?? "wrl"),
  );
  importedStandalonePreviewUi.fileName = (preferredModel ?? item.models[0]).file_name;
  importedStandalonePreviewUi.loading = true;
  importedStandalonePreviewUi.error = null;
  renderImportedStandalonePreview();

  try {
    importedStandalonePreviewViewer = new ModelPreviewViewer($("imported-standalone-preview-canvas"));
    await loadImportedStandalonePreviewModel();
  } catch (error) {
    importedStandalonePreviewUi.loading = false;
    importedStandalonePreviewUi.error = errorMessage(error);
    renderImportedStandalonePreview();
  }
}

function dedupeImportedParts(items: ImportedSymbol[]): string[] {
  const parts = new Set<string>();
  items.forEach((item) => {
    if (item.lcsc_part) parts.add(item.lcsc_part);
  });
  return Array.from(parts);
}

function filteredImportedItems(): ImportedSymbol[] {
  const query = importedUi.query.trim().toLowerCase();
  return importedUi.items.filter((item) => {
    const sourceMatches = !importedUi.sourceFilter || item.source_kind === importedUi.sourceFilter;
    if (!sourceMatches) return false;
    if (!query) return true;
    return [item.lcsc_part, item.symbol_name, item.package, item.value, item.source_file, item.source_kind]
      .join(" ")
      .toLowerCase()
      .includes(query);
  });
}

function selectedImportedItems(): ImportedSymbol[] {
  if (importedUi.selectedKeys.size === 0) {
    return [];
  }

  return importedUi.items.filter((item) => importedUi.selectedKeys.has(importedRowKey(item)));
}

function importedItemByKey(key: string | null): ImportedSymbol | null {
  if (!key) {
    return null;
  }
  return importedUi.items.find((item) => importedRowKey(item) === key) ?? null;
}

function activeImportedParts(): string[] {
  const selected = dedupeImportedParts(selectedImportedItems().filter((item) => item.source_kind === "export"));
  if (selected.length > 0) {
    return selected;
  }
  return dedupeImportedParts(filteredImportedItems().filter((item) => item.source_kind === "export"));
}

function pruneImportedSelection() {
  const validKeys = new Set(importedUi.items.map((item) => importedRowKey(item)));
  importedUi.selectedKeys = new Set(
    Array.from(importedUi.selectedKeys).filter((key) => validKeys.has(key)),
  );

  if (importedUi.editingKey && !validKeys.has(importedUi.editingKey)) {
    closeImportedEditor();
  }
}

function openImportedEditor(item: ImportedSymbol) {
  importedUi.editingKey = importedRowKey(item);
  importedUi.editDraftSymbolName = item.symbol_name;
  importedUi.editDraftLcscPart = item.lcsc_part;
  importedUi.editDraftSourceFile = item.source_file;
}

function closeImportedEditor() {
  importedUi.editingKey = null;
  importedUi.editDraftSymbolName = "";
  importedUi.editDraftLcscPart = "";
  importedUi.editDraftSourceFile = "";
}

function buildKeyword(): string {
  const parts: string[] = [];
  if (matchFull) parts.push(PATTERN_FULL);
  if (matchQuick) parts.push(PATTERN_QUICK);
  return parts.join("||");
}

function applyStaticTranslations() {
  document.documentElement.lang = "zh-CN";
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n")!;
    el.textContent = t(key);
  });

  document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    const key = el.getAttribute("data-i18n-placeholder")!;
    (el as HTMLInputElement).placeholder = t(key);
  });
  $("btn-toggle-matched").textContent = showMatched ? t("monitor.show") : t("monitor.hide");
  mountIcons();
  applyTooltips();
  renderImportedPanel();
  rerenderState();
}

function switchPage(pageName: PageName) {
  if (pageName !== "imported" && importedPreviewUi.item) {
    closeImportedPreview();
  }
  if (pageName !== "imported" && importedStandalonePreviewUi.item) {
    closeImportedStandalonePreview();
  }
  currentPage = pageName;
  document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
  document.querySelectorAll(".nav-item").forEach((n) => n.classList.remove("active"));

  const page = document.getElementById(`page-${pageName}`);
  const nav = document.querySelector(`.nav-item[data-page="${pageName}"]`);
  if (page) page.classList.add("active");
  if (nav) nav.classList.add("active");

  if (pageName === "imported" && !importedUi.initialized) {
    void loadImportedSymbols();
  }
  if (pageName === "inventory" && !inventoryUi.initialized) {
    void loadInventory();
  }
}

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

function rerenderState() {
  if (lastState) {
    renderState(lastState);
  }
}

function setExportNotice(tool: ExportTool, message: string | null, kind: ExportMessageKind = "warn") {
  exportUi[tool].notice = message ? { kind, message } : null;
  rerenderState();
}

function startExportProgress(tool: ExportTool, message: string) {
  exportUi[tool].notice = null;
  exportUi[tool].progress = {
    determinate: false,
    current: 0,
    total: 0,
    message,
  };
  exportUi[tool].resultKind = "info";
  rerenderState();
}

function updateExportProgress(payload: ExportProgressPayload) {
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].progress = {
    determinate: payload.determinate,
    current: payload.current ?? 0,
    total: payload.total ?? 0,
    message: payload.message,
  };
  rerenderState();
}

function finishExportProgress(payload: ExportFinishedPayload) {
  exportUi[payload.tool].progress = null;
  exportUi[payload.tool].notice = null;
  exportUi[payload.tool].resultKind = payload.success ? "success" : "error";
  rerenderState();
}

function renderExportProgress(tool: ExportTool, running: boolean, fallbackMessage: string) {
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
  const width = determinate ? `${Math.max(8, Math.round((current / progress.total) * 100))}%` : "42%";

  container.classList.remove("hidden");
  container.classList.toggle("indeterminate", !determinate);
  message.textContent = progress.message;
  meta.textContent = determinate ? `${current}/${progress.total}` : "";
  bar.style.width = width;
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

function renderExportAssetToggles(state: AppState): boolean {
  const anyExportEnabled = hasAnyExportEnabled(state);

  exportAssetToggles.forEach((toggle) => {
    const exportButton = $(toggle.exportButtonId) as HTMLButtonElement;
    const overwriteButton = $(toggle.overwriteButtonId) as HTMLButtonElement;
    const exportEnabledForAsset = exportEnabled(state, toggle.exportField);
    const overwriteEnabled = exportEnabledForAsset && exportOverwriteEnabled(state, toggle.overwriteField);

    exportButton.classList.toggle("active", exportEnabledForAsset);
    exportButton.setAttribute("aria-pressed", String(exportEnabledForAsset));

    overwriteButton.classList.toggle("active", overwriteEnabled);
    overwriteButton.disabled = !exportEnabledForAsset;
    overwriteButton.setAttribute("aria-pressed", String(overwriteEnabled));
  });

  return anyExportEnabled;
}

function renderExportNotice(tool: ExportTool, derivedNotice: ExportNotice | null = null) {
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

function renderExportResult(tool: ExportTool, result: string | null, busy: boolean, derivedNotice: ExportNotice | null = null) {
  const resultBox = $(toolElementId(tool, "result"));
  if (!result || busy || exportUi[tool].notice !== null || derivedNotice !== null) {
    resultBox.textContent = "";
    resultBox.className = "msg msg-info hidden";
    return;
  }

  resultBox.textContent = result;
  resultBox.className = `msg ${messageClass(exportUi[tool].resultKind)}`;
}

function renderExporterCard(options: ExportCardOptions) {
  $(options.countId).textContent = `${options.matchedCount} ${t("export.itemsReady")}`;

  const busy = options.running || exportUi[options.tool].progress !== null;
  const button = $(options.buttonId) as HTMLButtonElement;
  button.disabled = options.matchedCount === 0 || busy || Boolean(options.buttonDisabled);
  button.textContent = busy ? t("export.running") : t(options.exportLabelKey);

  renderExportProgress(options.tool, busy, t(options.runningLabelKey));
  renderExportNotice(options.tool, options.derivedNotice ?? null);
  renderExportResult(options.tool, options.result, busy, options.derivedNotice ?? null);
}

function syncExportProgressWithState(state: AppState) {
  if (!state.export_running && exportUi.export.progress !== null) {
    exportUi.export.progress = null;
  }
}

function syncOptionalExportState(state: AppState) {
  const mode = normalizeExport3dPathMode(state.export_path_mode);
  if (mode) {
    exportUiState.mode = mode;
  }
}

function renderExport3dMode() {
  export3dModes.forEach(({ id, value }) => {
    const button = $(id) as HTMLButtonElement;
    button.classList.toggle("active", exportUiState.mode === value);
  });
}

function renderExportFillColorDraft() {
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

function renderState(state: AppState) {
  syncOptionalExportState(state);
  syncExportProgressWithState(state);
  const noneLabel = t("status.none");

  $("status-keyword").textContent = state.keyword || noneLabel;
  $("status-counts").textContent = `历史 ${state.history_count} · 匹配 ${state.matched_count}`;
  $("btn-toggle-always-on-top").textContent = state.always_on_top
    ? t("status.alwaysOnTopOn")
    : t("status.alwaysOnTopOff");
  $("btn-toggle-always-on-top").classList.toggle("active", state.always_on_top);

  syncInputValue("export-path-input", state.export_output_path);
  syncInputValue("export-parallel-input", String(state.export_parallel));
  syncInputValue("export-symbol-fill-color-input", state.export_symbol_fill_color ?? "");
  syncInputValue("history-save-path-input", state.history_save_path);
  syncInputValue("matched-save-path-input", state.matched_save_path);
  syncInputValue("imported-parts-save-path-input", state.imported_parts_save_path);
  syncSelectValue("default-model-format-input", state.default_model_format);

  $("export-terminal-status").textContent = state.export_show_terminal ? t("export.terminalOn") : t("export.terminalOff");
  const exportHasExportSelection = renderExportAssetToggles(state);
  renderExport3dMode();
  renderExportFillColorDraft();

  const monBtn = $("btn-toggle-monitor");
  monBtn.classList.toggle("active", state.monitoring);
  monBtn.textContent = state.monitoring ? t("monitor.monitoring") : t("monitor.paused");

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

  $("matched-count").textContent = String(state.matched_count);
  if (showMatched && state.matched.length > 0) {
    $("matched-list").classList.remove("hidden");
    $("matched-empty").classList.add("hidden");
    renderMatchedList(state.matched);
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
    const [time, content] = state.history[0];
    $("latest-time").textContent = `${t("monitor.latest")} ${time}`;
    ($("latest-content") as HTMLTextAreaElement).value = content;
  } else {
    $("latest-preview").classList.add("hidden");
    $("history-waiting").classList.remove("hidden");
  }

  $("history-count-badge").textContent = String(state.history_count);
  if (state.history.length > 0) {
    $("history-list").classList.remove("hidden");
    $("history-empty").classList.add("hidden");
    renderHistoryList(state.history);
  } else if (state.history.length === 0) {
    $("history-list").classList.add("hidden");
    $("history-empty").classList.remove("hidden");
  }

  renderImportedPanel();
  applyTooltips();
}

function renderMatchedList(items: [string, string][]) {
  const copyLabel = t("monitor.copy");
  const c = $("matched-list");
  c.innerHTML = "";
  items.forEach(([time, value], idx) => {
    const row = document.createElement("div");
    row.className = "item-row";
    row.innerHTML = `
      <span class="item-time">${escapeHtml(time)}</span>
      <span class="item-value">${escapeHtml(value)}</span>
      <span class="item-actions">
        <button data-copy="${escapeAttr(value)}" title="${copyLabel}">${copyLabel}</button>
        <button data-delete-matched="${idx}" title="${t("monitor.delete")}">&times;</button>
      </span>`;
    c.appendChild(row);
  });
  applyTooltips(c);
}

function renderHistoryList(items: [string, string][]) {
  const copyLabel = t("monitor.copy");
  const c = $("history-list");
  c.innerHTML = "";
  items.forEach(([time, content], idx) => {
    const preview = content.split("\n")[0].substring(0, 80);
    const div = document.createElement("div");
    div.className = "history-item";
    div.innerHTML = `
      <div class="item-row">
        <span class="item-time">${escapeHtml(time)}</span>
        <span class="item-value">${escapeHtml(preview)}</span>
        <span class="item-actions">
          <button data-copy="${escapeAttr(content)}" title="${copyLabel}">${copyLabel}</button>
          <button data-delete-history="${idx}" title="${t("monitor.delete")}">&times;</button>
        </span>
      </div>`;
    c.appendChild(div);
  });
  applyTooltips(c);
}

function renderImportedList(items: ImportedSymbol[]) {
  if (importedPreviewUi.item) {
    closeImportedPreview();
  }
  const copyLabel = t("monitor.copy");
  const copyTitle = t("imported.copyPart");
  const queueLabel = t("imported.queuePart");
  const previewLabel = t("imported.preview");
  const editLabel = t("imported.edit");
  const deleteLabel = t("imported.deleteSymbol");
  const container = $("imported-list");
  container.innerHTML = "";

  items.forEach((item) => {
    const key = importedRowKey(item);
    const checked = importedUi.selectedKeys.has(key);
    const canPreview = item.source_kind === "export" && item.models.length > 0;
    const previewTitle = item.source_kind === "export"
      ? (canPreview ? previewLabel : t("imported.previewNoModels"))
      : "外部库只读，仅显示 3D 模型状态，暂不支持预览";
    const canExport = item.source_kind === "export" && Boolean(item.lcsc_part);
    const exportTitle = canExport ? queueLabel : "外部库元件只读，不能加入 export 导出队列";
    const row = document.createElement("div");
    row.className = "imported-row";
    row.dataset.previewRow = key;
    row.innerHTML = `
      <label class="imported-select">
        <input type="checkbox" data-select-imported="${escapeAttr(key)}" ${checked ? "checked" : ""} />
      </label>
      <span class="imported-cell imported-part" title="${escapeAttr(item.lcsc_part || "无供应商编号")}">${escapeHtml(item.lcsc_part || "无供应商编号")}</span>
      <span class="imported-cell imported-symbol" title="${escapeAttr(item.symbol_name)}">${escapeHtml(item.symbol_name)}</span>
      <span class="imported-cell imported-package" title="${escapeAttr(`${item.package || t("inventory.packagePending")} · ${item.source_file}`)}">${escapeHtml(item.package || t("inventory.packagePending"))}<small class="library-source-label">${escapeHtml(librarySourceLabel(item.source_kind))}</small></span>
      <span class="imported-actions">
        <button data-standalone-preview-imported="${escapeAttr(key)}" title="${escapeAttr(previewTitle)}" aria-label="${escapeAttr(previewTitle)}" ${canPreview ? "" : "disabled"}><i data-lucide="maximize-2"></i><span>${previewLabel}</span></button>
        <button data-queue-imported="${escapeAttr(item.lcsc_part)}" title="${escapeAttr(exportTitle)}" aria-label="${escapeAttr(exportTitle)}" ${canExport ? "" : "disabled"}>${queueLabel}</button>
        <button data-copy-imported="${escapeAttr(item.lcsc_part)}" title="${escapeAttr(canExport ? copyTitle : "外部库元件不能导出 LCSC 编号")}" aria-label="${escapeAttr(canExport ? copyTitle : "外部库元件不能导出 LCSC 编号")}" ${canExport ? "" : "disabled"}>${copyLabel}</button>
        <button data-edit-imported="${escapeAttr(key)}" title="${editLabel}" ${!item.editable ? "disabled" : ""}>${editLabel}</button>
        <button data-delete-imported="${escapeAttr(key)}" title="${deleteLabel}" ${!item.editable ? "disabled" : ""}>${deleteLabel}</button>
      </span>`;
    container.appendChild(row);
  });
  mountIcons();
  applyTooltips(container);
}

function renderImportedPanel() {
  const filteredItems = filteredImportedItems();
  const selectedItems = selectedImportedItems();
  const activeParts = activeImportedParts();
  const totalParts = dedupeImportedParts(importedUi.items);
  const filteredParts = dedupeImportedParts(filteredItems);
  const selectedParts = dedupeImportedParts(selectedItems);
  const editingItem = importedItemByKey(importedUi.editingKey);
  const path = $("imported-scanned-path");
  const feedback = $("imported-feedback");
  const table = $("imported-table");
  const empty = $("imported-empty");
  const editorCard = $("imported-editor-card");
  const refreshButton = $("btn-refresh-imported") as HTMLButtonElement;
  const browseButton = $("btn-browse-imported-parts-save-path") as HTMLButtonElement;
  const applyButton = $("btn-apply-imported-parts-save-path") as HTMLButtonElement;
  const importButton = $("btn-import-imported-parts") as HTMLButtonElement;
  const exportButton = $("btn-export-imported-parts") as HTMLButtonElement;
  const copyButton = $("btn-copy-imported-parts") as HTMLButtonElement;
  const queueButton = $("btn-queue-imported-parts") as HTMLButtonElement;
  const selectVisibleButton = $("btn-select-imported-visible") as HTMLButtonElement;
  const clearSelectionButton = $("btn-clear-imported-selection") as HTMLButtonElement;
  const saveEditButton = $("btn-save-imported-edit") as HTMLButtonElement;
  const editSymbolInput = $("imported-edit-symbol-name-input") as HTMLInputElement;
  const editLcscInput = $("imported-edit-lcsc-part-input") as HTMLInputElement;
  const cancelEditButtons = [
    $("btn-cancel-imported-edit") as HTMLButtonElement,
    $("btn-cancel-imported-edit-secondary") as HTMLButtonElement,
  ];
  const controlsDisabled = importedUi.loading || importedUi.busy;

  $("imported-total-count").textContent = String(totalParts.length);
  $("imported-filtered-count").textContent = String(filteredParts.length);
  $("imported-selected-count").textContent = String(selectedParts.length);
  ($("imported-search-input") as HTMLInputElement).value = importedUi.query;
  const sourceFilter = $("imported-source-filter") as HTMLSelectElement;
  sourceFilter.value = importedUi.sourceFilter;
  sourceFilter.innerHTML = `<option value="">全部来源</option>${Array.from(new Set(importedUi.items.map((item) => item.source_kind))).sort().map((kind) => `<option value="${escapeAttr(kind)}">${escapeHtml(librarySourceLabel(kind))}</option>`).join("")}`;
  sourceFilter.value = importedUi.sourceFilter;
  refreshButton.disabled = controlsDisabled;
  browseButton.disabled = controlsDisabled;
  applyButton.disabled = controlsDisabled;
  importButton.disabled = controlsDisabled;
  exportButton.disabled = controlsDisabled || activeParts.length === 0;
  copyButton.disabled = controlsDisabled || activeParts.length === 0;
  queueButton.disabled = controlsDisabled || activeParts.length === 0;
  selectVisibleButton.disabled = controlsDisabled || filteredItems.length === 0;
  clearSelectionButton.disabled = controlsDisabled || importedUi.selectedKeys.size === 0;
  saveEditButton.disabled = controlsDisabled || !editingItem;
  editSymbolInput.disabled = controlsDisabled || !editingItem;
  editLcscInput.disabled = controlsDisabled || !editingItem;
  cancelEditButtons.forEach((button) => {
    button.disabled = controlsDisabled;
  });
  applyTooltips();

  const resolvedPath =
    importedUi.scannedPath || lastState?.export_output_path || t("status.none");
  path.textContent = `${t("imported.scannedPath")} ${resolvedPath}`;
  const sources = $("imported-sources");
  sources.innerHTML = importedUi.sources
    .map((source) => `<span class="library-source-chip" title="${escapeAttr(source.path)}"><b>${escapeHtml(librarySourceLabel(source.kind))}</b><small>${escapeHtml(source.path)}</small>${source.configured ? `<button class="icon-only" data-remove-kicad-library="${escapeAttr(source.path)}" title="移除外部库来源" aria-label="移除外部库来源"><i data-lucide="x"></i></button>` : ""}</span>`)
    .join("");
  mountIcons();

  if (editingItem) {
    editorCard.classList.remove("hidden");
    editSymbolInput.value = importedUi.editDraftSymbolName;
    editLcscInput.value = importedUi.editDraftLcscPart;
    $("imported-editor-source-file").textContent = importedUi.editDraftSourceFile;
  } else {
    editorCard.classList.add("hidden");
    editSymbolInput.value = "";
    editLcscInput.value = "";
    $("imported-editor-source-file").textContent = "";
  }

  if (importedUi.notice) {
    feedback.textContent = importedUi.notice.message;
    feedback.className = `msg ${messageClass(importedUi.notice.kind)}`;
  } else if (importedUi.error) {
    feedback.textContent = importedUi.error;
    feedback.className = "msg msg-error";
  } else {
    feedback.textContent = "";
    feedback.className = "msg msg-info hidden";
  }

  if (importedUi.loading) {
    table.classList.add("hidden");
    empty.classList.remove("hidden");
    empty.textContent = t("imported.loading");
    return;
  }

  if (importedUi.error) {
    table.classList.add("hidden");
    empty.classList.add("hidden");
    return;
  }

  if (filteredItems.length > 0) {
    renderImportedList(filteredItems);
    table.classList.remove("hidden");
    empty.classList.add("hidden");
    return;
  }

  table.classList.add("hidden");
  empty.classList.remove("hidden");
  empty.textContent = importedUi.items.length > 0 ? t("imported.noFilterResults") : t("imported.empty");
}

async function refreshState() {
  const state = await invokeState();
  lastState = state;
  renderState(state);
}

async function selectDirectory(title: string): Promise<string | null> {
  const selected = await open({ directory: true, title });
  return typeof selected === "string" ? selected : null;
}

async function selectSaveFile(title: string, defaultPath: string | undefined): Promise<string | null> {
  const selected = await save({
    title,
    defaultPath: defaultPath && defaultPath.trim().length > 0 ? defaultPath : undefined,
    filters: [
      { name: "Text", extensions: ["txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

let monitorSaveResultTimer: number | null = null;

function showMonitorSaveResult(message: string, kind?: ExportMessageKind) {
  const el = $("monitor-save-result");
  const resolvedKind: ExportMessageKind = kind ?? classifySaveResult(message);
  el.textContent = message;
  el.className = `msg ${messageClass(resolvedKind)}`;

  if (monitorSaveResultTimer !== null) {
    window.clearTimeout(monitorSaveResultTimer);
  }
  monitorSaveResultTimer = window.setTimeout(() => {
    el.textContent = "";
    el.className = "msg msg-info hidden";
    monitorSaveResultTimer = null;
  }, 6000);
}

function classifySaveResult(message: string): ExportMessageKind {
  const lower = message.toLowerCase();
  if (lower.startsWith("saved") || lower.startsWith("exported") || lower.startsWith("queued")) return "success";
  if (lower.includes("failed")) return "error";
  return "warn";
}

function showImportedResult(message: string, kind?: ExportMessageKind) {
  importedUi.notice = {
    kind: kind ?? classifySaveResult(message),
    message,
  };
  renderImportedPanel();
}

function inventoryPartById(id: string | null): InventoryPart | null {
  return id ? inventoryUi.allParts.find((part) => part.id === id) ?? null : null;
}

function inventoryTotal(part: InventoryPart): number {
  return part.locations.reduce((total, location) => total + location.quantity, 0);
}

function showInventoryNotice(message: string | null, kind: ExportMessageKind = "info") {
  inventoryUi.notice = message ? { kind, message } : null;
  renderInventory();
}

function resetInventoryEditor() {
  inventoryUi.editingId = null;
  inventoryUi.draftSupplier = "";
  inventoryUi.draftLibraryLcsc = "";
  inventoryUi.draftLibrarySymbolName = "";
  inventoryUi.draftLibrarySourceFile = "";
  inventoryUi.draftLibraryMissing = false;
  inventoryUi.draftName = "";
  inventoryUi.draftPackage = "";
  inventoryUi.draftNote = "";
  inventoryUi.draftLocations = [];
}

function openInventoryEditor(part: InventoryPart | null = null) {
  inventoryUi.editingId = part?.id ?? null;
  inventoryUi.draftSupplier = part?.library_lcsc ?? part?.supplier_part_number ?? "";
  inventoryUi.draftLibraryLcsc = part?.library_lcsc ?? "";
  inventoryUi.draftLibrarySymbolName = part?.library_symbol_name ?? "";
  inventoryUi.draftLibrarySourceFile = part?.library_source_file ?? "";
  inventoryUi.draftLibraryMissing = part?.library_missing ?? false;
  inventoryUi.draftName = part?.name ?? "";
  inventoryUi.draftPackage = part?.package ?? "";
  inventoryUi.draftNote = part?.note ?? "";
  inventoryUi.draftLocations = part
    ? part.locations.map((location) => ({ ...location }))
    : [{ location: "未分配", quantity: 0, priority: 0 }];
  inventoryUi.notice = null;
  renderInventory();
}

function scrollInventoryEditorIntoView() {
  const body = document.querySelector(".inventory-body") as HTMLElement | null;
  const editor = $("inventory-editor") as HTMLElement;
  if (!body || editor.classList.contains("hidden")) return;
  // Keep the editor header visible; scrollIntoView can also move the app shell.
  body.scrollTo({ top: 0, behavior: "smooth" });
}

function filteredInventoryLibraryItems(): ImportedSymbol[] {
  const query = inventoryUi.libraryPickerQuery.trim().toLowerCase();
  const seen = new Set<string>();
  return inventoryUi.libraryItems.filter((item) => {
    if (seen.has(item.library_key)) return false;
    seen.add(item.library_key);
    if (!query) return true;
    return [item.lcsc_part, item.value, item.symbol_name, item.package, item.source_file]
      .join(" ")
      .toLowerCase()
      .includes(query);
  });
}

function renderInventoryLibraryPicker() {
  const picker = $("inventory-library-picker");
  picker.classList.toggle("hidden", !inventoryUi.libraryPickerOpen);
  if (!inventoryUi.libraryPickerOpen) return;
  ($("inventory-library-search") as HTMLInputElement).value = inventoryUi.libraryPickerQuery;
  const list = $("inventory-library-list");
  const items = filteredInventoryLibraryItems();
  if (inventoryUi.libraryPickerLoading) {
    list.innerHTML = `<div class="empty-state">${escapeHtml(t("inventory.libraryLoading"))}</div>`;
    return;
  }
  if (items.length === 0) {
    list.innerHTML = `<div class="empty-state">${escapeHtml(t("inventory.libraryEmpty"))}</div>`;
    return;
  }
  list.innerHTML = items
    .map((item) => {
      const existing = inventoryUi.allParts.find((part) => item.lcsc_part
        ? part.library_lcsc === item.lcsc_part
        : part.library_source_file === item.source_file && part.library_symbol_name === item.symbol_name);
      return `<button class="inventory-library-option" data-select-inventory-library="${escapeAttr(item.library_key)}"><span><strong>${escapeHtml(item.lcsc_part || "无供应商编号")}</strong><b>${escapeHtml(item.value)}</b><small>${escapeHtml(item.package || t("inventory.packagePending"))} · ${escapeHtml(item.symbol_name)} · ${escapeHtml(librarySourceLabel(item.source_kind))}</small></span>${existing ? `<em>${escapeHtml(t("inventory.alreadyAdded"))}</em>` : ""}</button>`;
    })
    .join("");
}

async function openInventoryLibraryPicker() {
  inventoryUi.libraryPickerOpen = true;
  inventoryUi.libraryPickerLoading = true;
  renderInventoryLibraryPicker();
  try {
    const response = await invoke<ImportedSymbolsResponse>("get_imported_symbols");
    inventoryUi.libraryItems = response.items;
  } catch (error) {
    inventoryUi.notice = { kind: "error", message: errorMessage(error) };
  } finally {
    inventoryUi.libraryPickerLoading = false;
    renderInventoryLibraryPicker();
  }
}

function selectInventoryLibraryPart(libraryKey: string) {
  const item = inventoryUi.libraryItems.find((candidate) => candidate.library_key === libraryKey);
  if (!item) return;
  const existing = inventoryUi.allParts.find((part) => item.lcsc_part
    ? part.library_lcsc === item.lcsc_part
    : part.library_source_file === item.source_file && part.library_symbol_name === item.symbol_name);
  if (existing) {
    openInventoryEditor(existing);
  } else {
    inventoryUi.editingId = null;
    inventoryUi.draftSupplier = item.lcsc_part;
    inventoryUi.draftLibraryLcsc = item.lcsc_part;
    inventoryUi.draftLibrarySymbolName = item.symbol_name;
    inventoryUi.draftLibrarySourceFile = item.source_file;
    inventoryUi.draftLibraryMissing = false;
    inventoryUi.draftName = item.value;
    inventoryUi.draftPackage = item.package;
    inventoryUi.draftNote = "";
    inventoryUi.draftLocations = [{ location: "未分配", quantity: 0, priority: 0 }];
  }
  inventoryUi.libraryPickerOpen = false;
  renderInventory();
}

function renderInventoryLocationFields() {
  const container = $("inventory-location-fields");
  container.innerHTML = inventoryUi.draftLocations
    .map(
      (location, index) => `
        <div class="inventory-location-row" data-inventory-location-row="${index}">
          <input type="text" data-inventory-location="${index}" value="${escapeAttr(location.location)}" placeholder="${escapeAttr(t("inventory.locationPlaceholder"))}" />
          <input type="number" data-inventory-quantity="${index}" value="${location.quantity}" aria-label="${escapeAttr(t("inventory.quantity"))}" />
          <span class="inventory-priority">${index + 1}</span>
          <button class="btn-ghost btn-sm icon-only" data-move-inventory-location="up" data-location-index="${index}" title="${escapeAttr(t("inventory.moveUp"))}" ${index === 0 ? "disabled" : ""}><i data-lucide="arrow-up"></i></button>
          <button class="btn-ghost btn-sm icon-only" data-move-inventory-location="down" data-location-index="${index}" title="${escapeAttr(t("inventory.moveDown"))}" ${index === inventoryUi.draftLocations.length - 1 ? "disabled" : ""}><i data-lucide="arrow-down"></i></button>
          <button class="btn-ghost btn-sm icon-only" data-remove-inventory-location="${index}" title="${escapeAttr(t("inventory.removeLocation"))}"><i data-lucide="x"></i></button>
        </div>`,
    )
    .join("");
  mountIcons();
  applyTooltips(container);
}

function renderInventoryList() {
  if (importedPreviewUi.item) {
    closeImportedPreview();
  }
  const list = $("inventory-list");
  list.innerHTML = inventoryUi.parts
    .map((part) => {
      const libraryItem = inventoryLibraryItem(part);
      const locations = part.locations
        .map((location) => `<span class="inventory-location-chip"><b>${escapeHtml(location.location)}</b><em>${location.quantity}</em></span>`)
        .join("");
      return `
        <div class="inventory-row" data-inventory-preview-row="${escapeAttr(part.id)}">
          <div class="inventory-cell inventory-part-id">${escapeHtml(part.library_lcsc ?? part.supplier_part_number ?? "-")}</div>
          <div class="inventory-cell inventory-part-name"><strong>${escapeHtml(part.name)}</strong>${part.library_symbol_name ? `<small>${escapeHtml(part.library_symbol_name)}</small>` : ""}${part.library_missing ? `<small class="danger-text">${escapeHtml(t("inventory.libraryMissing"))}</small>` : ""}${part.note ? `<small>${escapeHtml(part.note)}</small>` : ""}</div>
          <div class="inventory-cell">${escapeHtml(part.package || t("inventory.packagePending"))}${libraryItem?.models.length ? `<span class="inventory-model-badge" title="${escapeAttr(t("inventory.modelPreview"))}"><i data-lucide="box"></i></span>` : ""}</div>
          <div class="inventory-cell inventory-locations">${locations}</div>
          <div class="inventory-cell inventory-total ${inventoryTotal(part) < 0 ? "negative" : ""}">${inventoryTotal(part)}</div>
          <div class="inventory-cell inventory-actions">
            <button class="btn-ghost btn-sm" data-edit-inventory="${escapeAttr(part.id)}"><i data-lucide="pencil"></i>${escapeHtml(t("inventory.edit"))}</button>
            <button class="btn-danger btn-sm" data-delete-inventory="${escapeAttr(part.id)}"><i data-lucide="trash-2"></i>${escapeHtml(t("inventory.delete"))}</button>
            ${part.locations.map((location) => `<button class="btn-outline btn-sm icon-only" data-stock-part="${escapeAttr(part.id)}" data-stock-location="${escapeAttr(location.location)}" data-stock-delta="-1" title="-1 ${escapeAttr(location.location)}">-</button><button class="btn-outline btn-sm icon-only" data-stock-part="${escapeAttr(part.id)}" data-stock-location="${escapeAttr(location.location)}" data-stock-delta="1" title="+1 ${escapeAttr(location.location)}">+</button>`).join("")}
          </div>
        </div>`;
    })
    .join("");
  mountIcons();
  applyTooltips(list);
}

function allocationTotal(row: BomPreviewRow): number {
  return row.allocations.reduce((total, allocation) => total + allocation.quantity, 0);
}

function defaultInventoryAllocations(part: InventoryPart, required: number): InventoryAllocation[] {
  let remaining = required;
  return [...part.locations]
    .sort((left, right) => left.priority - right.priority || left.location.localeCompare(right.location))
    .map((location, index, locations) => {
      const quantity = index === locations.length - 1 ? Math.max(remaining, 0) : Math.min(Math.max(location.quantity, 0), Math.max(remaining, 0));
      remaining -= quantity;
      return { part_id: part.id, location: location.location, quantity };
    });
}

function bomMatchStatus(row: BomPreviewRow): string {
  if (row.supplier_part_number_conflict) return t("inventory.statusConflict");
  if (row.match_kind === "ambiguous") return t("inventory.statusAmbiguous");
  if (row.matched_part_id) return t("inventory.statusInventory");
  if (row.library_candidates.some((candidate) => !candidate.already_in_inventory)) return t("inventory.statusLibrary");
  if (row.supplier_part_number) return t("inventory.statusPending");
  return t("inventory.statusManual");
}

function bomLibraryStatus(row: BomPreviewRow): string {
  const status = {
    bound: "inventory.libraryBound",
    available: "inventory.statusLibrary",
    missing: "inventory.statusPending",
    manual: "inventory.statusManual",
    ambiguous: "inventory.statusAmbiguous",
  }[row.library_status];
  return t(status || "inventory.statusUnmatched");
}

function bomModelStatus(row: BomPreviewRow): string {
  return t(row.model_status === "available" ? "inventory.libraryModel" : "inventory.libraryNoModel");
}

function bomLibrarySelection(row: BomPreviewRow): string {
  const selected = inventoryUi.bomLibrarySelections[row.row_number];
  if (selected !== undefined) return selected;
  if (row.library_candidates.length !== 1) return "";
  const available = row.library_candidates.find((candidate) => !candidate.already_in_inventory);
  return available?.library_key ?? "";
}

function bomLibraryOptions(row: BomPreviewRow): string {
  const selected = bomLibrarySelection(row);
  const choices = new Map<string, string>();
  for (const candidate of row.library_candidates) choices.set(candidate.library_key, `${candidate.label}${candidate.has_model ? ` · ${t("inventory.libraryModel")}` : ` · ${t("inventory.libraryNoModel")}`}`);
  return `<option value="">${escapeHtml(t("inventory.manualRecord"))}</option>${Array.from(choices.entries()).map(([key, label]) => `<option value="${escapeAttr(key)}" ${key === selected ? "selected" : ""}>${escapeHtml(label)}</option>`).join("")}`;
}

function renderInventoryBomPreview() {
  const container = $("inventory-bom-preview");
  const preview = inventoryUi.bomPreview;
  if (!preview) {
    container.classList.add("hidden");
    container.innerHTML = "";
    ($("btn-confirm-inventory-bom") as HTMLButtonElement).disabled = true;
    ($("btn-import-inventory-bom") as HTMLButtonElement).disabled = true;
    return;
  }

  container.classList.remove("hidden");
  container.innerHTML = `
    <div class="inventory-preview-summary">
      <span>${escapeHtml(t("inventory.previewHint"))}</span>
      <strong>${preview.rows.length} rows · ${preview.boards} board(s)</strong>
    </div>
    <div class="inventory-bom-table">
      ${preview.rows
        .map((row) => {
          const skipped = inventoryUi.bomSkipped.has(row.row_number);
          const selectedPart = inventoryPartById(row.matched_part_id);
          const total = allocationTotal(row);
          const valid = skipped || Boolean(selectedPart && !row.supplier_part_number_conflict && total === row.required_quantity && row.allocations.every((allocation) => allocation.quantity >= 0));
          const candidates = row.candidates
            .map((candidate) => `<option value="${escapeAttr(candidate.id)}" ${candidate.id === row.matched_part_id ? "selected" : ""}>${escapeHtml(candidate.label)}</option>`)
            .join("");
          const allocationLocations = selectedPart
            ? selectedPart.locations
                .slice()
                .sort((left, right) => left.priority - right.priority || left.location.localeCompare(right.location))
                .map((location) => {
                  const allocation = row.allocations.find((item) => item.location === location.location);
                  return `<label class="inventory-allocation"><span>${escapeHtml(location.location)}</span><input type="number" min="0" step="1" data-bom-allocation="${row.row_number}" data-allocation-location="${escapeAttr(location.location)}" value="${allocation?.quantity ?? 0}" ${skipped ? "disabled" : ""} /></label>`;
                })
                .join("")
            : `<span class="hint">${escapeHtml(t("inventory.noCandidates"))}</span>`;
          return `
            <div class="inventory-bom-row ${skipped ? "is-skipped" : ""} ${!valid ? "is-invalid" : ""}" data-bom-row="${row.row_number}">
              <div class="inventory-bom-source"><span class="inventory-row-number">#${row.row_number}</span><strong>${escapeHtml(row.name)}</strong><small>${escapeHtml(row.package)}${row.supplier_part_number ? ` · ${escapeHtml(row.supplier_part_number)}${row.supplier_part_number_source ? ` · ${escapeHtml(row.supplier_part_number_source)}` : ""}` : ""}</small><small>${row.references ? escapeHtml(row.references) : ""}${row.identifier ? ` · ${escapeHtml(row.identifier)}` : ""}</small></div>
              <div class="inventory-bom-need"><span>${escapeHtml(t("inventory.required"))}</span><strong>${row.required_quantity}</strong><small>${row.quantity_per_board} / board</small></div>
              <div class="inventory-bom-match"><strong class="inventory-bom-status">${escapeHtml(bomMatchStatus(row))}</strong><div class="inventory-bom-statuses"><span title="${escapeAttr(t("inventory.lcscStatusHint"))}">${escapeHtml(t("inventory.lcscStatus"))}: ${escapeHtml(row.supplier_part_number ? (row.supplier_part_number_conflict ? t("inventory.statusConflict") : t("inventory.statusIdentified")) : t("inventory.statusUnmatched"))}</span><span title="${escapeAttr(t("inventory.libraryStatusHint"))}">${escapeHtml(t("inventory.libraryStatus"))}: ${escapeHtml(bomLibraryStatus(row))}</span><span title="${escapeAttr(t("inventory.modelStatusHint"))}">${escapeHtml(t("inventory.modelStatus"))}: ${escapeHtml(bomModelStatus(row))}</span></div><select data-bom-candidate="${row.row_number}" title="${escapeAttr(t("inventory.chooseCandidate"))}" ${skipped || row.candidates.length === 0 ? "disabled" : ""}><option value="">${row.candidates.length > 1 ? escapeHtml(t("inventory.chooseCandidate")) : escapeHtml(t("inventory.noCandidates"))}</option>${candidates}</select><label class="inventory-bom-binding"><span>${escapeHtml(t("inventory.libraryBinding"))}</span><select data-bom-library="${row.row_number}" title="${escapeAttr(t("inventory.libraryBinding"))}" ${skipped ? "disabled" : ""}>${bomLibraryOptions(row)}</select></label><div class="inventory-bom-row-actions"><button class="btn-ghost btn-sm" data-toggle-bom-skip="${row.row_number}" title="${escapeAttr(skipped ? t("inventory.unskip") : t("inventory.skip"))}">${skipped ? escapeHtml(t("inventory.unskip")) : escapeHtml(t("inventory.skip"))}</button><button class="btn-outline btn-sm" data-new-bom-part="${row.row_number}" title="${escapeAttr(t("inventory.newFromBom"))}">${escapeHtml(t("inventory.newFromBom"))}</button></div>${row.supplier_part_number_conflict || row.candidates.length > 1 ? `<small class="danger-text">${escapeHtml(row.supplier_part_number_conflict ? t("inventory.statusConflict") : t("inventory.conflict"))}</small>` : ""}</div>
              <div class="inventory-bom-alloc"><div class="inventory-allocation-title"><span>${escapeHtml(t("inventory.allocation"))}</span><strong class="${total === row.required_quantity ? "ok" : "bad"}">${total} / ${row.required_quantity}</strong></div><div class="inventory-allocation-grid">${allocationLocations}</div></div>
            </div>`;
        })
        .join("")}
    </div>`;
  const canConfirm = preview.rows.every((row) => {
    if (inventoryUi.bomSkipped.has(row.row_number)) return true;
    return !row.supplier_part_number_conflict
      && Boolean(row.matched_part_id)
      && allocationTotal(row) === row.required_quantity
      && row.allocations.every((allocation) => allocation.quantity >= 0);
  });
  ($("btn-confirm-inventory-bom") as HTMLButtonElement).disabled = !canConfirm || inventoryUi.bomLoading;
  ($("btn-import-inventory-bom") as HTMLButtonElement).disabled = inventoryUi.bomLoading;
  applyTooltips(container);
}

function renderInventoryBomFeedback() {
  const feedback = $("inventory-bom-feedback");
  if (inventoryUi.bomError) {
    feedback.textContent = inventoryUi.bomError;
    feedback.className = "msg msg-error";
  } else {
    feedback.textContent = "";
    feedback.className = "msg msg-info hidden";
  }
}

function renderInventoryProductionRecords() {
  const container = $("inventory-production-records");
  if (inventoryUi.productionRecords.length === 0) {
    container.innerHTML = `<div class="empty-state">${escapeHtml(t("inventory.noProduction"))}</div>`;
    return;
  }
  container.innerHTML = inventoryUi.productionRecords
    .map((record) => `<div class="production-record"><strong>#${record.id}</strong><span>${escapeHtml(record.path)}</span><small>${record.boards} board(s) · ${record.matched_rows}/${record.total_rows} matched${record.skipped_rows ? ` · ${record.skipped_rows} skipped` : ""} · ${escapeHtml(record.created_at)}</small></div>`)
    .join("");
}

function renderInventory() {
  if (!document.getElementById("inventory-list")) return;
  $("inventory-count").textContent = String(inventoryUi.parts.length);
  const search = $("inventory-search") as HTMLInputElement;
  if (search.value !== inventoryUi.query) search.value = inventoryUi.query;
  const feedback = $("inventory-feedback");
  if (inventoryUi.notice) {
    feedback.textContent = inventoryUi.notice.message;
    feedback.className = `msg ${messageClass(inventoryUi.notice.kind)}`;
  } else if (inventoryUi.error) {
    feedback.textContent = inventoryUi.error;
    feedback.className = "msg msg-error";
  } else {
    feedback.textContent = "";
    feedback.className = "msg msg-info hidden";
  }
  const editor = $("inventory-editor");
  editor.classList.toggle("hidden", inventoryUi.editingId === null && inventoryUi.draftName === "" && inventoryUi.draftLocations.length === 0);
  ($("inventory-supplier") as HTMLInputElement).value = inventoryUi.draftSupplier;
  ($("inventory-name") as HTMLInputElement).value = inventoryUi.draftName;
  const packageInput = $("inventory-package") as HTMLInputElement;
  packageInput.value = inventoryUi.draftPackage;
  packageInput.placeholder = inventoryUi.draftLibraryLcsc && !inventoryUi.draftPackage ? t("inventory.packagePending") : "";
  ($("inventory-note") as HTMLInputElement).value = inventoryUi.draftNote;
  const libraryStatus = $("inventory-library-status");
  if (inventoryUi.draftLibraryLcsc) {
    libraryStatus.textContent = inventoryUi.draftLibraryMissing
      ? `${t("inventory.libraryMissing")} · ${inventoryUi.draftLibraryLcsc}`
      : `${inventoryUi.draftLibrarySymbolName || t("inventory.linked")} · ${inventoryUi.draftLibraryLcsc}`;
    libraryStatus.className = inventoryUi.draftLibraryMissing ? "danger-text" : "hint";
  } else {
    libraryStatus.textContent = t("inventory.statusManual");
    libraryStatus.className = "hint";
  }
  renderInventoryLocationFields();
  if (inventoryUi.loading) {
    $("inventory-table").classList.add("hidden");
    $("inventory-empty").classList.remove("hidden");
    $("inventory-empty").textContent = "Loading inventory...";
  } else if (inventoryUi.parts.length > 0) {
    renderInventoryList();
    $("inventory-table").classList.remove("hidden");
    $("inventory-empty").classList.add("hidden");
  } else {
    $("inventory-table").classList.add("hidden");
    $("inventory-empty").classList.remove("hidden");
    $("inventory-empty").textContent = t("inventory.empty");
  }
  ($("inventory-bom-path") as HTMLInputElement).value = inventoryUi.bomPath;
  ($("inventory-bom-boards") as HTMLInputElement).value = inventoryUi.bomBoards;
  renderInventoryBomFeedback();
  renderInventoryBomPreview();
  $("inventory-production-panel").classList.toggle("hidden", inventoryUi.productionRecords.length === 0);
  renderInventoryProductionRecords();
  renderInventoryLibraryPicker();
  applyTooltips();
}

async function loadInventory() {
  inventoryUi.loading = true;
  inventoryUi.error = null;
  renderInventory();
  try {
    const response = await invoke<InventoryResponse>("get_inventory", { query: inventoryUi.query });
    inventoryUi.revision = response.revision;
    inventoryUi.parts = response.parts;
    if (inventoryUi.query.trim()) {
      const all = await invoke<InventoryResponse>("get_inventory", { query: "" });
      inventoryUi.allParts = all.parts;
    } else {
      inventoryUi.allParts = response.parts;
    }
    inventoryUi.productionRecords = await invoke<ProductionRecord[]>("get_production_records", { limit: 20 });
    try {
      const libraryResponse = await invoke<ImportedSymbolsResponse>("get_imported_symbols");
      inventoryUi.libraryItems = libraryResponse.items;
    } catch {
      // Inventory remains usable when the configured KiCad library is unavailable.
      inventoryUi.libraryItems = [];
    }
    inventoryUi.initialized = true;
  } catch (error) {
    inventoryUi.error = browserPreviewMode ? null : errorMessage(error);
  } finally {
    inventoryUi.loading = false;
    renderInventory();
  }
}

function readInventoryDraft(): { supplier: string; name: string; packageName: string; note: string; locations: InventoryLocation[] } {
  const locationFields = Array.from(document.querySelectorAll(".inventory-location-row"));
  const locations = locationFields.map((row, index) => ({
    location: ((row.querySelector("[data-inventory-location]") as HTMLInputElement)?.value ?? "").trim(),
    quantity: Number.parseInt((row.querySelector("[data-inventory-quantity]") as HTMLInputElement)?.value ?? "0", 10),
    priority: index,
  }));
  return {
    supplier: ($("inventory-supplier") as HTMLInputElement).value.trim(),
    name: ($("inventory-name") as HTMLInputElement).value.trim(),
    packageName: ($("inventory-package") as HTMLInputElement).value.trim(),
    note: ($("inventory-note") as HTMLInputElement).value.trim(),
    locations: locations.map((location) => ({ ...location, quantity: Number.isFinite(location.quantity) ? location.quantity : 0 })),
  };
}

async function saveInventoryPart() {
  const draft = readInventoryDraft();
  if (!draft.name || (!draft.packageName && !inventoryUi.draftLibraryLcsc) || draft.locations.some((location) => !location.location)) {
    showInventoryNotice("元件库记录、封装和库位编号不能为空。", "error");
    return;
  }
  await invoke("save_inventory_part", {
    input: {
      id: inventoryUi.editingId,
      library_lcsc: inventoryUi.draftLibraryLcsc || null,
      library_symbol_name: inventoryUi.draftLibrarySymbolName || null,
      library_source_file: inventoryUi.draftLibrarySourceFile || null,
      supplier_part_number: draft.supplier || null,
      name: draft.name,
      package: draft.packageName,
      note: draft.note,
      locations: draft.locations,
    },
  });
  resetInventoryEditor();
  inventoryUi.notice = { kind: "success", message: "库存元件已保存。" };
  await loadInventory();
  if (inventoryUi.bomPreview && inventoryUi.bomPath) {
    await previewInventoryBom();
  }
}

async function previewInventoryBom() {
  const boards = Number.parseInt(inventoryUi.bomBoards.trim(), 10);
  if (!inventoryUi.bomPath || !/^\d+$/.test(inventoryUi.bomBoards.trim()) || !Number.isSafeInteger(boards) || boards < 1) {
    inventoryUi.bomError = "请选择 CSV，并输入大于零的整数板数。";
    renderInventory();
    return;
  }
  inventoryUi.bomLoading = true;
  inventoryUi.bomError = null;
  renderInventory();
  try {
    const preview = await invoke<BomPreview>("preview_inventory_bom", { path: inventoryUi.bomPath, boards });
    inventoryUi.bomPreview = preview;
    inventoryUi.bomSkipped = new Set();
    inventoryUi.bomLibrarySelections = {};
  } catch (error) {
    inventoryUi.bomPreview = null;
    inventoryUi.bomError = errorMessage(error);
  } finally {
    inventoryUi.bomLoading = false;
    renderInventory();
  }
}

async function importInventoryBom() {
  const preview = inventoryUi.bomPreview;
  if (!preview) return;
  inventoryUi.bomLoading = true;
  inventoryUi.bomError = null;
  renderInventory();
  try {
    const rows: BomImportRow[] = preview.rows.map((row) => ({
      row_number: row.row_number,
      skipped: inventoryUi.bomSkipped.has(row.row_number),
      library_lcsc: (() => {
        const key = bomLibrarySelection(row);
        const candidate = row.library_candidates.find((item) => item.library_key === key)
          ?? inventoryUi.libraryItems.find((item) => item.library_key === key);
        return candidate?.lcsc_part || row.supplier_part_number || null;
      })(),
      library_key: bomLibrarySelection(row) || null,
    }));
    const result = await invoke<ImportBomResult>("import_inventory_bom", {
      request: { path: preview.path, revision: preview.revision, rows },
    });
    inventoryUi.notice = { kind: "success", message: `已导入 ${result.imported} 条库存记录，${result.existing} 条已存在。` };
    await loadInventory();
    await previewInventoryBom();
  } catch (error) {
    inventoryUi.bomError = errorMessage(error);
  } finally {
    inventoryUi.bomLoading = false;
    renderInventory();
  }
}

async function confirmInventoryBom() {
  const preview = inventoryUi.bomPreview;
  if (!preview) return;
  const requestRows: BomDeductionRow[] = preview.rows.map((row) => ({
    row_number: row.row_number,
    part_id: row.matched_part_id,
    skipped: inventoryUi.bomSkipped.has(row.row_number),
    allocations: row.allocations.filter((allocation) => allocation.quantity > 0),
  }));
  inventoryUi.bomLoading = true;
  inventoryUi.bomError = null;
  renderInventory();
  try {
    await invoke<string>("confirm_inventory_bom", { request: { path: preview.path, boards: preview.boards, revision: preview.revision, rows: requestRows } });
    inventoryUi.bomPreview = null;
    inventoryUi.bomSkipped.clear();
    inventoryUi.notice = { kind: "success", message: t("inventory.confirmed") };
    await loadInventory();
  } catch (error) {
    inventoryUi.bomError = errorMessage(error);
  } finally {
    inventoryUi.bomLoading = false;
    renderInventory();
  }
}

function showExportStartResult(tool: ExportTool, result: string): boolean {
  if (result === "Export started") {
    setExportNotice(tool, null);
    return true;
  }

  exportUi[tool].progress = null;
  exportUi[tool].notice = { kind: "warn", message: result };
  rerenderState();
  return false;
}

function showExportError(tool: ExportTool, error: string) {
  exportUi[tool].progress = null;
  exportUi[tool].notice = { kind: "error", message: error };
  rerenderState();
}

async function loadImportedSymbols() {
  if (importedPreviewUi.item) {
    closeImportedPreview();
  }
  if (importedStandalonePreviewUi.item) {
    closeImportedStandalonePreview();
  }
  importedUi.loading = true;
  importedUi.notice = null;
  renderImportedPanel();

  try {
    const response = await invoke<ImportedSymbolsResponse>("get_imported_symbols");
    importedUi.loading = false;
    importedUi.initialized = true;
    importedUi.scannedPath = response.scanned_path;
    importedUi.sources = response.sources;
    importedUi.items = response.items;
    importedUi.error = null;
    pruneImportedSelection();
  } catch (error) {
    importedUi.loading = false;
    importedUi.initialized = true;
    importedUi.scannedPath = "";
    importedUi.sources = [];
    importedUi.items = [];
    importedUi.error = browserPreviewMode ? null : errorMessage(error);
    importedUi.selectedKeys.clear();
    closeImportedEditor();
  }

  renderImportedPanel();
}

function invalidateImportedSymbols(clearItems = false) {
  importedUi.initialized = false;
  importedUi.scannedPath = "";
  importedUi.error = null;
  if (clearItems) {
    importedUi.items = [];
    importedUi.selectedKeys.clear();
  }
}

let pendingExportConfigWrite: Promise<void> = Promise.resolve();

function queueExportConfigWrite(operation: () => Promise<void>): Promise<void> {
  const run = pendingExportConfigWrite.then(operation, operation);
  pendingExportConfigWrite = run.catch(() => {});
  return run;
}

async function runImportedAction(operation: () => Promise<void>) {
  if (importedUi.loading || importedUi.busy) return;
  importedUi.busy = true;
  renderImportedPanel();
  try {
    await operation();
  } finally {
    importedUi.busy = false;
    renderImportedPanel();
  }
}

async function syncExportInputs() {
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

async function setExport3dMode(mode: Export3dPathMode) {
  await invoke("set_export_path_mode", { pathMode: mode });
  exportUiState.mode = mode;
  setExportNotice("export", null);
  await refreshState();
}

async function saveActiveImportedParts() {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
  await queueExportConfigWrite(async () => {
    await invoke("set_imported_parts_save_path", { path });
    await refreshState();
  });
  const result = await invoke<string>("save_lcsc_parts", { parts });
  showImportedResult(result);
}

async function queueActiveImportedParts() {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const result = await invoke<string>("queue_lcsc_parts", { parts });
  showImportedResult(result);
  await refreshState();
}

async function saveImportedEdit() {
  const item = importedItemByKey(importedUi.editingKey);
  if (!item) {
    return;
  }

  const newSymbolName = importedUi.editDraftSymbolName.trim();
  const newLcscPart = normalizeImportedLcscPart(importedUi.editDraftLcscPart);
  const result = await invoke<string>("update_imported_symbol", {
    request: {
      source_file: item.source_file,
      symbol_name: item.symbol_name,
      new_symbol_name: newSymbolName,
      lcsc_part: newLcscPart,
    },
  });

  closeImportedEditor();
  await loadImportedSymbols();
  showImportedResult(result, "success");
}

async function deleteImportedItem(item: ImportedSymbol) {
  const confirmed = window.confirm(
    formatMessage("imported.deleteConfirm", {
      symbol: item.symbol_name,
      file: item.source_file,
    }),
  );
  if (!confirmed) {
    return;
  }

  const result = await invoke<string>("delete_imported_symbol", {
    request: {
      source_file: item.source_file,
      symbol_name: item.symbol_name,
      lcsc_part: item.lcsc_part,
    },
  });

  if (importedUi.editingKey === importedRowKey(item)) {
    closeImportedEditor();
  }
  await loadImportedSymbols();
  showImportedResult(result, "success");
}

window.addEventListener("DOMContentLoaded", async () => {
  applyStaticTranslations();
  try {
    await refreshState();
  } catch {
    // Keep the static shell usable when the SPA is opened directly in a browser.
  }
  try {
    await listen("clipboard-changed", () => {
      void refreshState();
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }
  try {
    await listen<ExportProgressPayload>("export-progress", (event) => {
      updateExportProgress(event.payload);
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }
  try {
    await listen<ExportFinishedPayload>("export-finished", async (event) => {
      finishExportProgress(event.payload);
      await refreshState();
      if (event.payload.tool === "export" && event.payload.success) {
        invalidateImportedSymbols();
        if (currentPage === "imported") {
          await loadImportedSymbols();
        }
      }
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }

  document.querySelectorAll("[data-page]").forEach((item) => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");
      if (page) switchPage(page as PageName);
    });
  });

  $("btn-collapse").addEventListener("click", () => {
    $("sidebar").classList.toggle("collapsed");
  });

  $("btn-toggle-always-on-top").addEventListener("click", async () => {
    const next = !(lastState?.always_on_top ?? false);
    await invoke("set_window_always_on_top", { alwaysOnTop: next });
    await refreshState();
  });

  $("btn-match-quick").addEventListener("click", async () => {
    matchQuick = !matchQuick;
    $("btn-match-quick").classList.toggle("active", matchQuick);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await refreshState();
  });

  $("btn-match-full").addEventListener("click", async () => {
    matchFull = !matchFull;
    $("btn-match-full").classList.toggle("active", matchFull);
    await invoke("set_keyword", { keyword: buildKeyword() });
    await refreshState();
  });

  $("btn-toggle-monitor").addEventListener("click", async () => {
    await invoke("toggle_monitoring");
    await refreshState();
  });

  $("btn-toggle-matched").addEventListener("click", () => {
    showMatched = !showMatched;
    $("btn-toggle-matched").classList.toggle("active", showMatched);
    $("btn-toggle-matched").textContent = showMatched ? t("monitor.show") : t("monitor.hide");
    void refreshState();
  });

  $("btn-copy-ids").addEventListener("click", async () => {
    const ids: string[] = await invoke("get_unique_ids");
    if (ids.length > 0) {
      await invoke("copy_to_clipboard", { text: ids.join("\n") });
    }
  });

  $("btn-export").addEventListener("click", async () => {
    if (lastState && !hasAnyExportEnabled(lastState)) {
      setExportNotice("export", t("export.selectAtLeastOne"));
      return;
    }

    try {
      await queueExportConfigWrite(async () => {
        await syncExportInputs();
        await refreshState();
      });
      startExportProgress("export", t("export.exportRunning"));
      const result = await invoke<string>("export");
      showExportStartResult("export", result);
      await refreshState();
    } catch (error) {
      const details = errorMessage(error);
      showExportError("export", details);
      await refreshState();
    }
  });

  $("btn-browse-export-folder").addEventListener("click", async () => {
    const selected = await selectDirectory("Select export directory");
    if (selected) {
      ($("export-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_export_path", { path: selected });
        invalidateImportedSymbols(true);
        await refreshState();
      });
      if (currentPage === "imported") {
        await loadImportedSymbols();
      }
    }
  });

  $("btn-apply-export-path").addEventListener("click", async () => {
    const path = ($("export-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_export_path", { path });
      invalidateImportedSymbols(true);
      await refreshState();
    });
    if (currentPage === "imported") {
      await loadImportedSymbols();
    }
  });

  $("btn-toggle-export-terminal").addEventListener("click", async () => {
    await queueExportConfigWrite(async () => {
      await invoke("toggle_export_terminal");
      await refreshState();
    });
  });

  exportAssetToggles.forEach((toggle) => {
    $(toggle.exportButtonId).addEventListener("click", async () => {
      const active = $(toggle.exportButtonId).classList.contains("active");
      await queueExportConfigWrite(async () => {
        await invoke(toggle.exportCommand, { enabled: !active });
        if (active) {
          await invoke(toggle.overwriteCommand, { overwrite: false });
        }
        await refreshState();
      });
    });

    $(toggle.overwriteButtonId).addEventListener("click", async () => {
      const button = $(toggle.overwriteButtonId) as HTMLButtonElement;
      if (button.disabled) {
        return;
      }

      const active = button.classList.contains("active");
      await queueExportConfigWrite(async () => {
        await invoke(toggle.overwriteCommand, { overwrite: !active });
        await refreshState();
      });
    });
  });

  export3dModes.forEach(({ id, value }) => {
    $(id).addEventListener("click", async () => {
      await queueExportConfigWrite(async () => {
        await setExport3dMode(value);
      });
    });
  });

  $("btn-apply-default-model-format").addEventListener("click", async () => {
    const format = ($("default-model-format-input") as HTMLSelectElement).value as ModelFormat;
    await queueExportConfigWrite(async () => {
      await invoke("set_default_model_format", { format });
      await refreshState();
    });
  });

  $("btn-apply-export-parallel").addEventListener("click", async () => {
    const value = ($("export-parallel-input") as HTMLInputElement).value;
    const parallel = parsePositiveIntOrFallback(value, 4);
    await queueExportConfigWrite(async () => {
      await invoke("set_export_parallel", { parallel });
      await refreshState();
    });
  });

  $("btn-apply-export-symbol-fill-color").addEventListener("click", async () => {
    const input = $("export-symbol-fill-color-input") as HTMLInputElement;
    const parsed = parseOptionalHexColor(input.value);
    renderExportFillColorDraft();
    if (!parsed.valid) {
      return;
    }

    await queueExportConfigWrite(async () => {
      await invoke("set_export_symbol_fill_color", { color: parsed.normalized });
      await refreshState();
    });
  });

  $("btn-clear-export-symbol-fill-color").addEventListener("click", async () => {
    const input = $("export-symbol-fill-color-input") as HTMLInputElement;
    input.value = "";
    renderExportFillColorDraft();
    await queueExportConfigWrite(async () => {
      await invoke("set_export_symbol_fill_color", { color: null });
      await refreshState();
    });
  });

  $("export-symbol-fill-color-input").addEventListener("input", () => {
    renderExportFillColorDraft();
  });

  $("btn-refresh-imported").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    await loadImportedSymbols();
  });

  $("imported-source-filter").addEventListener("change", (event) => {
    importedUi.sourceFilter = (event.target as HTMLSelectElement).value;
    renderImportedPanel();
  });
  $("btn-scan-kicad-library").addEventListener("click", async () => {
    await runImportedAction(async () => {
      await loadImportedSymbols();
      showImportedResult("KiCad 元件库扫描完成。", "success");
    });
  });
  const addKicadLibraryPaths = async (paths: string[]) => {
    if (paths.length === 0) return;
    await runImportedAction(async () => {
      for (const path of paths) {
        await invoke("add_kicad_library_path", { path });
      }
      await loadImportedSymbols();
      showImportedResult(`已添加 ${paths.length} 个外部库来源。`, "success");
    });
  };
  $("btn-add-kicad-library-file").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    const selected = await open({
      multiple: true,
      title: "添加只读 KiCad 外部库文件",
      filters: [{ name: "KiCad library", extensions: ["kicad_sym", "kicad_mod"] }],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await addKicadLibraryPaths(paths);
  });
  $("btn-add-kicad-library").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    const selected = await open({ directory: true, multiple: true, title: "添加只读 KiCad 元件库" });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await addKicadLibraryPaths(paths);
  });
  $("imported-sources").addEventListener("click", async (event) => {
    const target = (event.target as HTMLElement).closest("[data-remove-kicad-library]") as HTMLElement | null;
    const path = target?.getAttribute("data-remove-kicad-library");
    if (!path) return;
    await invoke("remove_kicad_library_path", { path });
    await loadImportedSymbols();
  });

  $("imported-list").addEventListener("pointerover", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest(".imported-row") as HTMLElement | null;
    const related = event.relatedTarget as Node | null;
    if (!row || (related && row.contains(related))) {
      return;
    }

    const key = row.dataset.previewRow;
    const item = key ? importedItemByKey(key) : null;
    if (item) {
      scheduleImportedPreview(item, row);
    }
  });

  $("imported-list").addEventListener("pointerout", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest(".imported-row") as HTMLElement | null;
    const related = event.relatedTarget as Node | null;
    if (row && (!related || !row.contains(related))) {
      closeImportedPreview();
    }
  });

  $("inventory-list").addEventListener("pointerover", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest(".inventory-row") as HTMLElement | null;
    const related = event.relatedTarget as Node | null;
    if (!row || (related && row.contains(related))) {
      return;
    }

    const part = inventoryUi.parts.find((candidate) => candidate.id === row.dataset.inventoryPreviewRow);
    const item = part ? inventoryLibraryItem(part) : null;
    if (item) {
      scheduleImportedPreview(item, row);
    } else {
      closeImportedPreview();
    }
  });

  $("inventory-list").addEventListener("pointerout", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest(".inventory-row") as HTMLElement | null;
    const related = event.relatedTarget as Node | null;
    if (row && (!related || !row.contains(related))) {
      closeImportedPreview();
    }
  });

  $("btn-close-imported-standalone-preview").addEventListener("click", () => {
    closeImportedStandalonePreview();
  });

  $("btn-reset-imported-standalone-preview").addEventListener("click", () => {
    importedStandalonePreviewViewer?.resetView();
  });

  $("imported-standalone-preview-model-select").addEventListener("change", async (event) => {
    const select = event.target as HTMLSelectElement;
    importedStandalonePreviewUi.fileName = select.value;
    await loadImportedStandalonePreviewModel();
  });

  $("imported-standalone-preview-modal").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) {
      closeImportedStandalonePreview();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && importedStandalonePreviewUi.item) {
      closeImportedStandalonePreview();
    }
  });

  window.addEventListener("resize", positionImportedPreview);

  $("imported-search-input").addEventListener("input", (event) => {
    importedUi.query = (event.target as HTMLInputElement).value;
    renderImportedPanel();
  });

  $("btn-select-imported-visible").addEventListener("click", () => {
    filteredImportedItems().forEach((item) => {
      importedUi.selectedKeys.add(importedRowKey(item));
    });
    renderImportedPanel();
  });

  $("btn-clear-imported-selection").addEventListener("click", () => {
    importedUi.selectedKeys.clear();
    renderImportedPanel();
  });

  $("imported-edit-symbol-name-input").addEventListener("input", (event) => {
    importedUi.editDraftSymbolName = (event.target as HTMLInputElement).value;
  });

  $("imported-edit-lcsc-part-input").addEventListener("input", (event) => {
    importedUi.editDraftLcscPart = normalizeImportedLcscPart((event.target as HTMLInputElement).value);
    (event.target as HTMLInputElement).value = importedUi.editDraftLcscPart;
  });

  const cancelImportedEdit = () => {
    if (importedUi.busy) return;
    closeImportedEditor();
    renderImportedPanel();
  };

  $("btn-cancel-imported-edit").addEventListener("click", cancelImportedEdit);
  $("btn-cancel-imported-edit-secondary").addEventListener("click", cancelImportedEdit);

  $("btn-save-imported-edit").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await saveImportedEdit();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-copy-imported-parts").addEventListener("click", async () => {
    const parts = activeImportedParts();
    if (parts.length === 0) return;
    await invoke("copy_to_clipboard", { text: parts.join("\n") });
    showImportedResult(t("imported.copied"), "success");
  });

  $("btn-queue-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await queueActiveImportedParts();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-apply-imported-parts-save-path").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_imported_parts_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-imported-parts-save-path").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    const current = ($("imported-parts-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile(t("imported.exportDialog"), current);
    if (selected) {
      ($("imported-parts-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_imported_parts_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-export-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        await saveActiveImportedParts();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-import-imported-parts").addEventListener("click", async () => {
    await runImportedAction(async () => {
      importedUi.notice = null;
      renderImportedPanel();

      try {
        const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
        await queueExportConfigWrite(async () => {
          await invoke("set_imported_parts_save_path", { path });
          await refreshState();
        });
        const result = await invoke<string>("import_imported_parts");
        const kind: ExportMessageKind =
          result.toLowerCase().includes("failed")
            ? "error"
            : result.startsWith("Imported 0 ") || result.startsWith("No ")
              ? "warn"
              : result.startsWith("Imported ")
                ? "success"
                : "warn";
        showImportedResult(result, kind);
        await refreshState();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  $("btn-save-history").addEventListener("click", async () => {
    try {
      const result = await invoke<string>("save_history");
      showMonitorSaveResult(result);
    } catch (error) {
      showMonitorSaveResult(errorMessage(error), "error");
    }
  });

  $("btn-apply-history-save-path").addEventListener("click", async () => {
    const path = ($("history-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_history_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-history-save-path").addEventListener("click", async () => {
    const current = ($("history-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile("Choose Save History file", current);
    if (selected) {
      ($("history-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_history_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-apply-matched-save-path").addEventListener("click", async () => {
    const path = ($("matched-save-path-input") as HTMLInputElement).value;
    await queueExportConfigWrite(async () => {
      await invoke("set_matched_save_path", { path });
      await refreshState();
    });
  });

  $("btn-browse-matched-save-path").addEventListener("click", async () => {
    const current = ($("matched-save-path-input") as HTMLInputElement).value;
    const selected = await selectSaveFile("Choose Export Matched file", current);
    if (selected) {
      ($("matched-save-path-input") as HTMLInputElement).value = selected;
      await queueExportConfigWrite(async () => {
        await invoke("set_matched_save_path", { path: selected });
        await refreshState();
      });
    }
  });

  $("btn-save-matched").addEventListener("click", async () => {
    try {
      const result = await invoke<string>("save_matched");
      showMonitorSaveResult(result);
    } catch (error) {
      showMonitorSaveResult(errorMessage(error), "error");
    }
  });

  $("btn-clear-all").addEventListener("click", () => {
    $("btn-clear-all").classList.add("hidden");
    $("clear-confirm").classList.remove("hidden");
  });

  $("btn-clear-confirm").addEventListener("click", async () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
    await invoke("clear_all");
    await refreshState();
  });

  $("btn-clear-cancel").addEventListener("click", () => {
    $("btn-clear-all").classList.remove("hidden");
    $("clear-confirm").classList.add("hidden");
  });

  $("inventory-search").addEventListener("input", () => {
    inventoryUi.query = ($("inventory-search") as HTMLInputElement).value;
    void loadInventory();
  });

  $("btn-new-inventory").addEventListener("click", () => {
    openInventoryEditor();
    inventoryUi.libraryPickerQuery = "";
    void openInventoryLibraryPicker();
    scrollInventoryEditorIntoView();
  });
  $("btn-select-inventory-library").addEventListener("click", () => {
    inventoryUi.libraryPickerQuery = "";
    void openInventoryLibraryPicker();
  });
  $("btn-close-inventory-library").addEventListener("click", () => {
    inventoryUi.libraryPickerOpen = false;
    renderInventoryLibraryPicker();
  });
  $("inventory-library-search").addEventListener("input", (event) => {
    inventoryUi.libraryPickerQuery = (event.target as HTMLInputElement).value;
    renderInventoryLibraryPicker();
  });
  const inventoryDraftInputs: Record<string, keyof typeof inventoryUi> = {
    "inventory-supplier": "draftSupplier",
    "inventory-name": "draftName",
    "inventory-package": "draftPackage",
    "inventory-note": "draftNote",
  };
  Object.entries(inventoryDraftInputs).forEach(([id, key]) => {
    $(id).addEventListener("input", (event) => {
      inventoryUi[key] = (event.target as HTMLInputElement).value as never;
    });
  });
  $("btn-cancel-inventory").addEventListener("click", () => {
    resetInventoryEditor();
    renderInventory();
  });
  $("btn-add-inventory-location").addEventListener("click", () => {
    const draft = readInventoryDraft();
    inventoryUi.draftSupplier = draft.supplier;
    inventoryUi.draftName = draft.name;
    inventoryUi.draftPackage = draft.packageName;
    inventoryUi.draftNote = draft.note;
    inventoryUi.draftLocations = [...draft.locations, { location: "", quantity: 0, priority: draft.locations.length }];
    renderInventory();
  });
  $("btn-save-inventory").addEventListener("click", async () => {
    if (inventoryUi.busy) return;
    inventoryUi.busy = true;
    try {
      await saveInventoryPart();
    } catch (error) {
      showInventoryNotice(errorMessage(error), "error");
    } finally {
      inventoryUi.busy = false;
      renderInventory();
    }
  });
  $("btn-import-matched-inventory").addEventListener("click", async () => {
    if (inventoryUi.busy) return;
    inventoryUi.busy = true;
    try {
      const result = await invoke<string>("import_matched_to_inventory");
      inventoryUi.notice = { kind: "success", message: result };
      await loadInventory();
    } catch (error) {
      showInventoryNotice(errorMessage(error), "error");
    } finally {
      inventoryUi.busy = false;
      renderInventory();
    }
  });
  $("btn-browse-inventory-bom").addEventListener("click", () => {
    $("inventory-bom-panel").classList.remove("hidden");
    ($("inventory-bom-panel") as HTMLElement).scrollIntoView({ behavior: "smooth", block: "start" });
  });
  $("btn-close-inventory-bom").addEventListener("click", () => {
    inventoryUi.bomPreview = null;
    inventoryUi.bomError = null;
    $("inventory-bom-panel").classList.add("hidden");
    renderInventoryBomPreview();
    renderInventoryBomFeedback();
  });
  $("btn-select-inventory-bom").addEventListener("click", async () => {
    try {
      const selected = await open({ title: "Choose BOM CSV", multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }, { name: "All files", extensions: ["*"] }] });
      if (typeof selected === "string") {
        inventoryUi.bomPath = selected;
        inventoryUi.bomPreview = null;
        inventoryUi.bomError = null;
        $("inventory-bom-panel").classList.remove("hidden");
        renderInventory();
      }
    } catch (error) {
      inventoryUi.bomError = errorMessage(error);
      renderInventoryBomFeedback();
    }
  });
  $("inventory-bom-boards").addEventListener("input", () => {
    inventoryUi.bomBoards = ($("inventory-bom-boards") as HTMLInputElement).value;
  });
  $("btn-preview-inventory-bom").addEventListener("click", () => void previewInventoryBom());
  $("btn-import-inventory-bom").addEventListener("click", () => void importInventoryBom());
  $("btn-confirm-inventory-bom").addEventListener("click", () => void confirmInventoryBom());

  $("inventory-location-fields").addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const remove = target.closest("[data-remove-inventory-location]") as HTMLElement | null;
    const move = target.closest("[data-move-inventory-location]") as HTMLElement | null;
    if (!remove && !move) return;
    const draft = readInventoryDraft();
    inventoryUi.draftSupplier = draft.supplier;
    inventoryUi.draftName = draft.name;
    inventoryUi.draftPackage = draft.packageName;
    inventoryUi.draftNote = draft.note;
    const index = Number.parseInt((remove ?? move)?.getAttribute("data-location-index") ?? "-1", 10);
    if (index < 0 || index >= draft.locations.length) return;
    if (remove) {
      draft.locations.splice(index, 1);
    } else {
      const direction = move?.getAttribute("data-move-inventory-location");
      const nextIndex = direction === "up" ? index - 1 : index + 1;
      if (nextIndex < 0 || nextIndex >= draft.locations.length) return;
      [draft.locations[index], draft.locations[nextIndex]] = [draft.locations[nextIndex], draft.locations[index]];
    }
    inventoryUi.draftLocations = draft.locations;
    renderInventory();
  });
  $("inventory-location-fields").addEventListener("input", (event) => {
    const target = event.target as HTMLInputElement;
    const index = Number.parseInt(target.getAttribute("data-inventory-location") ?? target.getAttribute("data-inventory-quantity") ?? "-1", 10);
    if (index < 0 || !inventoryUi.draftLocations[index]) return;
    if (target.hasAttribute("data-inventory-location")) {
      inventoryUi.draftLocations[index].location = target.value;
    } else {
      inventoryUi.draftLocations[index].quantity = Number.parseInt(target.value, 10) || 0;
    }
  });
  $("inventory-library-list").addEventListener("click", (event) => {
    const target = (event.target as HTMLElement).closest("[data-select-inventory-library]") as HTMLElement | null;
    const lcscPart = target?.getAttribute("data-select-inventory-library");
    if (lcscPart) selectInventoryLibraryPart(lcscPart);
  });

  $("inventory-bom-preview").addEventListener("change", (event) => {
    const target = event.target as HTMLInputElement | HTMLSelectElement;
    const libraryRow = target.getAttribute("data-bom-library");
    if (libraryRow) {
      inventoryUi.bomLibrarySelections[Number(libraryRow)] = target.value;
      renderInventoryBomPreview();
      return;
    }
    const candidateRow = target.getAttribute("data-bom-candidate");
    if (candidateRow) {
      const row = inventoryUi.bomPreview?.rows.find((item) => item.row_number === Number(candidateRow));
      if (!row) return;
      row.matched_part_id = target.value || null;
      const part = inventoryPartById(row.matched_part_id);
      row.allocations = part ? defaultInventoryAllocations(part, row.required_quantity) : [];
      inventoryUi.bomSkipped.delete(row.row_number);
      renderInventoryBomPreview();
      return;
    }
    const allocationRow = target.getAttribute("data-bom-allocation");
    const location = target.getAttribute("data-allocation-location");
    if (allocationRow && location) {
      const row = inventoryUi.bomPreview?.rows.find((item) => item.row_number === Number(allocationRow));
      if (!row) return;
      const allocation = row.allocations.find((item) => item.location === location);
      const quantity = Math.max(0, Number.parseInt(target.value, 10) || 0);
      if (allocation) allocation.quantity = quantity;
      else if (row.matched_part_id) row.allocations.push({ part_id: row.matched_part_id, location, quantity });
      renderInventoryBomPreview();
    }
  });
  $("inventory-bom-preview").addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const newPartRow = target.closest("[data-new-bom-part]") as HTMLElement | null;
    if (newPartRow) {
      const row = inventoryUi.bomPreview?.rows.find((item) => item.row_number === Number(newPartRow.getAttribute("data-new-bom-part")));
      if (!row) return;
      openInventoryEditor();
      inventoryUi.libraryPickerQuery = [row.supplier_part_number ?? "", row.name, row.package].join(" ").trim();
      void openInventoryLibraryPicker();
      scrollInventoryEditorIntoView();
      return;
    }
    const rowNumber = target.getAttribute("data-toggle-bom-skip");
    if (!rowNumber) return;
    const row = Number.parseInt(rowNumber, 10);
    if (inventoryUi.bomSkipped.has(row)) inventoryUi.bomSkipped.delete(row);
    else inventoryUi.bomSkipped.add(row);
    renderInventoryBomPreview();
  });

  document.addEventListener("change", (e) => {
    const target = e.target as HTMLElement;
    if (target instanceof HTMLInputElement && target.matches("input[data-select-imported]")) {
      const key = target.getAttribute("data-select-imported");
      if (!key) return;
      if (target.checked) {
        importedUi.selectedKeys.add(key);
      } else {
        importedUi.selectedKeys.delete(key);
      }
      renderImportedPanel();
    }
  });

  document.addEventListener("click", async (e) => {
    const target = e.target as HTMLElement;

    const inventoryEdit = target.closest("[data-edit-inventory]") as HTMLElement | null;
    if (inventoryEdit) {
      const part = inventoryPartById(inventoryEdit.getAttribute("data-edit-inventory"));
      if (part) openInventoryEditor(part);
      return;
    }

    const inventoryDelete = target.closest("[data-delete-inventory]") as HTMLElement | null;
    if (inventoryDelete) {
      const id = inventoryDelete.getAttribute("data-delete-inventory");
      if (!id || !window.confirm("确定删除这个库存元件吗？")) return;
      try {
        await invoke("delete_inventory_part", { id });
        inventoryUi.notice = { kind: "success", message: "库存元件已删除。" };
        await loadInventory();
      } catch (error) {
        showInventoryNotice(errorMessage(error), "error");
      }
      return;
    }

    const stockButton = target.closest("[data-stock-part]") as HTMLElement | null;
    if (stockButton) {
      const partId = stockButton.getAttribute("data-stock-part");
      const location = stockButton.getAttribute("data-stock-location");
      const delta = Number.parseInt(stockButton.getAttribute("data-stock-delta") ?? "0", 10);
      if (!partId || !location || !delta) return;
      try {
        await invoke("adjust_inventory_stock", { adjustment: { part_id: partId, location, delta } });
        await loadInventory();
      } catch (error) {
        showInventoryNotice(errorMessage(error), "error");
      }
      return;
    }

    const urlEl = target.closest("[data-url]") as HTMLElement | null;
    if (urlEl) {
      const url = urlEl.getAttribute("data-url");
      if (url) {
        await openUrl(url);
        return;
      }
    }

    const copyVal = target.getAttribute("data-copy");
    if (copyVal !== null) {
      await invoke("copy_to_clipboard", { text: copyVal });
      return;
    }

    const importedCopy = target.getAttribute("data-copy-imported");
    if (importedCopy !== null) {
      await invoke("copy_to_clipboard", { text: importedCopy });
      importedUi.notice = { kind: "success", message: t("imported.copied") };
      renderImportedPanel();
      return;
    }

    const importedQueue = target.getAttribute("data-queue-imported");
    if (importedQueue !== null) {
      await runImportedAction(async () => {
        try {
          const result = await invoke<string>("queue_lcsc_parts", { parts: [importedQueue] });
          showImportedResult(result);
          await refreshState();
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
      return;
    }

    const importedStandalonePreviewEl = target.closest("[data-standalone-preview-imported]") as HTMLElement | null;
    const importedStandalonePreview = importedStandalonePreviewEl?.getAttribute("data-standalone-preview-imported");
    if (importedStandalonePreview !== null && importedStandalonePreview !== undefined) {
      const item = importedItemByKey(importedStandalonePreview);
      if (item) {
        await openImportedStandalonePreview(item);
      }
      return;
    }

    const importedEdit = target.getAttribute("data-edit-imported");
    if (importedEdit !== null) {
      const item = importedItemByKey(importedEdit);
      if (!item) {
        return;
      }
      openImportedEditor(item);
      renderImportedPanel();
      return;
    }

    const importedDelete = target.getAttribute("data-delete-imported");
    if (importedDelete !== null) {
      const item = importedItemByKey(importedDelete);
      if (!item) {
        return;
      }
      await runImportedAction(async () => {
        importedUi.notice = null;
        renderImportedPanel();

        try {
          await deleteImportedItem(item);
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
      return;
    }

    const dm = target.getAttribute("data-delete-matched");
    if (dm !== null) {
      await invoke("delete_matched", { index: parseInt(dm, 10) });
      await refreshState();
      return;
    }

    const dh = target.getAttribute("data-delete-history");
    if (dh !== null) {
      await invoke("delete_history", { index: parseInt(dh, 10) });
      await refreshState();
    }
  });
});
