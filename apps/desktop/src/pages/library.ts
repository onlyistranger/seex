// Imported library page module.
//
// The library page owns its search, selection, editing, source management and
// import/export controls. Preview primitives remain in shared/preview.ts while
// the app root supplies cross-page refresh and configuration operations.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { mountIcons } from "../icons";
import { errorMessage } from "../ipc";
import { translate } from "../i18n";
import { renderSignature } from "../patched-render";
import { toast } from "../ui/toast";
import {
  closeImportedPreview,
  closeImportedStandalonePreview,
  importedRowKey,
  openImportedStandalonePreview,
  scheduleImportedPreview,
} from "../shared/preview";
import type {
  AppState,
  ExportMessageKind,
  ExportNotice,
  ImportedSymbol,
  ImportedSymbolsResponse,
  LibrarySource,
} from "../types";
import { $, applyTooltips, escapeAttr, escapeHtml, formatMessage, syncInputValue } from "../utils";

const t = translate;
const browserPreviewMode = !("__TAURI_INTERNALS__" in window);

export interface LibraryPageContext {
  refresh: () => Promise<void>;
  queueConfigWrite: (operation: () => Promise<void>) => Promise<void>;
  selectSaveFile: (title: string, defaultPath: string | undefined) => Promise<string | null>;
}

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

let lastState: AppState | null = null;
let pageContext: LibraryPageContext | null = null;
let mounted = false;
let lastImportedListSignature: string | null = null;
let lastImportedSourceOptionsSignature: string | null = null;
let lastImportedSourcesSignature: string | null = null;

export function librarySourceLabel(kind: string): string {
  return (
    (
      {
        export: "export",
        kicad_standard: "KiCad 标准库",
        project: "项目库",
        external: "外部库",
      } as Record<string, string>
    )[kind] ?? kind
  );
}

function normalizeImportedLcscPart(value: string): string {
  return value.trim().toUpperCase();
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
    return [
      item.lcsc_part,
      item.symbol_name,
      item.package,
      item.value,
      item.source_file,
      item.source_kind,
    ]
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
  const selected = dedupeImportedParts(
    selectedImportedItems().filter((item) => item.source_kind === "export"),
  );
  if (selected.length > 0) {
    return selected;
  }
  return dedupeImportedParts(
    filteredImportedItems().filter((item) => item.source_kind === "export"),
  );
}

function pruneImportedSelection(): void {
  const validKeys = new Set(importedUi.items.map((item) => importedRowKey(item)));
  importedUi.selectedKeys = new Set(
    Array.from(importedUi.selectedKeys).filter((key) => validKeys.has(key)),
  );

  if (importedUi.editingKey && !validKeys.has(importedUi.editingKey)) {
    closeImportedEditor();
  }
}

function openImportedEditor(item: ImportedSymbol): void {
  importedUi.editingKey = importedRowKey(item);
  importedUi.editDraftSymbolName = item.symbol_name;
  importedUi.editDraftLcscPart = item.lcsc_part;
  importedUi.editDraftSourceFile = item.source_file;
}

function closeImportedEditor(): void {
  importedUi.editingKey = null;
  importedUi.editDraftSymbolName = "";
  importedUi.editDraftLcscPart = "";
  importedUi.editDraftSourceFile = "";
}

function syncImportedDraftInput(id: string, value: string): void {
  const input = $(id) as HTMLInputElement;
  if (document.activeElement !== input || input.value === value) {
    input.value = value;
  }
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

function classifySaveResult(message: string): ExportMessageKind {
  const lower = message.toLowerCase();
  if (lower.startsWith("saved") || lower.startsWith("exported") || lower.startsWith("queued")) {
    return "success";
  }
  if (lower.includes("failed")) return "error";
  return "warn";
}

function showImportedResult(message: string, kind?: ExportMessageKind): void {
  const resolvedKind = kind ?? classifySaveResult(message);
  toast[resolvedKind](message);
}

function renderImportedList(items: ImportedSymbol[]): void {
  closeImportedPreview();
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
    const previewTitle =
      item.source_kind === "export"
        ? canPreview
          ? previewLabel
          : t("imported.previewNoModels")
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

function importedListSignature(items: ImportedSymbol[]): string {
  return renderSignature({
    items,
    selectedKeys: Array.from(importedUi.selectedKeys),
  });
}

function renderImportedPanel(): void {
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
  const loadingSkeleton = $("imported-loading-skeleton");
  const empty = $("imported-empty");
  const emptyTitle = $("imported-empty-title");
  const emptyHint = $("imported-empty-hint");
  const retryButton = $("btn-retry-imported") as HTMLButtonElement;
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
  syncInputValue("imported-search-input", importedUi.query);
  const sourceFilter = $("imported-source-filter") as HTMLSelectElement;
  sourceFilter.value = importedUi.sourceFilter;
  const sourceKinds = Array.from(new Set(importedUi.items.map((item) => item.source_kind))).sort();
  const sourceOptionsSignature = renderSignature(sourceKinds);
  if (sourceOptionsSignature !== lastImportedSourceOptionsSignature) {
    lastImportedSourceOptionsSignature = sourceOptionsSignature;
    sourceFilter.innerHTML = `<option value="">全部来源</option>${sourceKinds
      .map(
        (kind) =>
          `<option value="${escapeAttr(kind)}">${escapeHtml(librarySourceLabel(kind))}</option>`,
      )
      .join("")}`;
  }
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

  const resolvedPath = importedUi.scannedPath || lastState?.export_output_path || t("status.none");
  path.textContent = `${t("imported.scannedPath")} ${resolvedPath}`;
  const sources = $("imported-sources");
  const sourcesSignature = renderSignature(importedUi.sources);
  if (sourcesSignature !== lastImportedSourcesSignature) {
    lastImportedSourcesSignature = sourcesSignature;
    sources.innerHTML = importedUi.sources
      .map(
        (source) =>
          `<span class="library-source-chip" title="${escapeAttr(source.path)}"><b>${escapeHtml(librarySourceLabel(source.kind))}</b><small>${escapeHtml(source.path)}</small>${source.configured ? `<button class="icon-only" data-remove-kicad-library="${escapeAttr(source.path)}" title="移除外部库来源" aria-label="移除外部库来源"><i data-lucide="x"></i></button>` : ""}</span>`,
      )
      .join("");
    mountIcons();
  }

  if (editingItem) {
    editorCard.classList.remove("hidden");
    syncImportedDraftInput("imported-edit-symbol-name-input", importedUi.editDraftSymbolName);
    syncImportedDraftInput("imported-edit-lcsc-part-input", importedUi.editDraftLcscPart);
    $("imported-editor-source-file").textContent = importedUi.editDraftSourceFile;
  } else {
    editorCard.classList.add("hidden");
    syncImportedDraftInput("imported-edit-symbol-name-input", "");
    syncImportedDraftInput("imported-edit-lcsc-part-input", "");
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
    loadingSkeleton.classList.remove("hidden");
    empty.classList.add("hidden");
    return;
  }

  loadingSkeleton.classList.add("hidden");
  if (importedUi.error) {
    table.classList.add("hidden");
    empty.classList.remove("hidden");
    emptyTitle.textContent = t("imported.loadErrorTitle");
    emptyHint.textContent = t("imported.loadErrorGuide");
    retryButton.classList.remove("hidden");
    return;
  }

  retryButton.classList.add("hidden");
  if (filteredItems.length > 0) {
    const listSignature = importedListSignature(filteredItems);
    if (listSignature !== lastImportedListSignature) {
      lastImportedListSignature = listSignature;
      renderImportedList(filteredItems);
    }
    table.classList.remove("hidden");
    empty.classList.add("hidden");
    return;
  }

  table.classList.add("hidden");
  empty.classList.remove("hidden");
  if (importedUi.items.length > 0) {
    emptyTitle.textContent = t("imported.noFilterResults");
    emptyHint.textContent = t("imported.selectionHint");
  } else {
    emptyTitle.textContent = t("imported.emptyTitle");
    emptyHint.textContent = t("imported.emptyGuide");
  }
}

async function loadImportedSymbols(): Promise<void> {
  closeImportedPreview();
  closeImportedStandalonePreview();
  importedUi.loading = true;
  importedUi.error = null;
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
    if (!browserPreviewMode) toast.error(errorMessage(error));
    importedUi.selectedKeys.clear();
    closeImportedEditor();
  }

  renderImportedPanel();
}

export function invalidate(clearItems = false): void {
  importedUi.initialized = false;
  importedUi.scannedPath = "";
  importedUi.error = null;
  if (clearItems) {
    importedUi.items = [];
    importedUi.selectedKeys.clear();
  }
}

async function runImportedAction(operation: () => Promise<void>): Promise<void> {
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

async function saveActiveImportedParts(): Promise<void> {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const context = pageContext;
  if (!context) return;
  const path = ($("imported-parts-save-path-input") as HTMLInputElement).value;
  await context.queueConfigWrite(async () => {
    await invoke("set_imported_parts_save_path", { path });
    await context.refresh();
  });
  const result = await invoke<string>("save_lcsc_parts", { parts });
  showImportedResult(result);
}

async function queueActiveImportedParts(): Promise<void> {
  const parts = activeImportedParts();
  if (parts.length === 0) {
    showImportedResult(t("imported.noActionableParts"), "warn");
    return;
  }

  const context = pageContext;
  if (!context) return;
  const result = await invoke<string>("queue_lcsc_parts", { parts });
  showImportedResult(result);
  await context.refresh();
}

async function saveImportedEdit(): Promise<void> {
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

async function deleteImportedItem(item: ImportedSymbol): Promise<void> {
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

export function render(state?: AppState): void {
  if (state) lastState = state;
  renderImportedPanel();
}

export function ensureLoaded(): void {
  if (!importedUi.initialized) {
    void loadImportedSymbols();
  }
}

export function showPreviewNoModels(): void {
  toast.warn(t("imported.previewNoModels"));
}

export function mount(nextContext: LibraryPageContext): void {
  if (mounted) return;
  mounted = true;
  pageContext = nextContext;

  $("btn-refresh-imported").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    await loadImportedSymbols();
  });

  $("btn-retry-imported").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
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

  const addKicadLibraryPaths = async (paths: string[]): Promise<void> => {
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
    const selected = await open({
      directory: true,
      multiple: true,
      title: "添加只读 KiCad 元件库",
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    await addKicadLibraryPaths(paths);
  });

  $("imported-sources").addEventListener("click", async (event) => {
    const target = (event.target as HTMLElement).closest(
      "[data-remove-kicad-library]",
    ) as HTMLElement | null;
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
    importedUi.editDraftLcscPart = normalizeImportedLcscPart(
      (event.target as HTMLInputElement).value,
    );
    (event.target as HTMLInputElement).value = importedUi.editDraftLcscPart;
  });

  const cancelImportedEdit = (): void => {
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
    await nextContext.queueConfigWrite(async () => {
      await invoke("set_imported_parts_save_path", { path });
      await nextContext.refresh();
    });
  });

  $("btn-browse-imported-parts-save-path").addEventListener("click", async () => {
    if (importedUi.loading || importedUi.busy) return;
    importedUi.notice = null;
    const current = ($("imported-parts-save-path-input") as HTMLInputElement).value;
    const selected = await nextContext.selectSaveFile(t("imported.exportDialog"), current);
    if (selected) {
      ($("imported-parts-save-path-input") as HTMLInputElement).value = selected;
      await nextContext.queueConfigWrite(async () => {
        await invoke("set_imported_parts_save_path", { path: selected });
        await nextContext.refresh();
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
        await nextContext.queueConfigWrite(async () => {
          await invoke("set_imported_parts_save_path", { path });
          await nextContext.refresh();
        });
        const result = await invoke<string>("import_imported_parts");
        const kind: ExportMessageKind = result.toLowerCase().includes("failed")
          ? "error"
          : result.startsWith("Imported 0 ") || result.startsWith("No ")
            ? "warn"
            : result.startsWith("Imported ")
              ? "success"
              : "warn";
        showImportedResult(result, kind);
        await nextContext.refresh();
      } catch (error) {
        showImportedResult(errorMessage(error), "error");
      }
    });
  });

  document.addEventListener("change", (event) => {
    const target = event.target as HTMLElement;
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

  document.addEventListener("click", async (event) => {
    const target = event.target as HTMLElement;

    const importedCopy = target.closest("[data-copy-imported]") as HTMLElement | null;
    if (importedCopy) {
      const part = importedCopy.getAttribute("data-copy-imported");
      if (part !== null) {
        await invoke("copy_to_clipboard", { text: part });
        showImportedResult(t("imported.copied"), "success");
      }
      return;
    }

    const importedQueue = target.closest("[data-queue-imported]") as HTMLElement | null;
    if (importedQueue) {
      const part = importedQueue.getAttribute("data-queue-imported");
      if (part === null) return;
      await runImportedAction(async () => {
        try {
          const result = await invoke<string>("queue_lcsc_parts", { parts: [part] });
          showImportedResult(result);
          await nextContext.refresh();
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
      return;
    }

    const importedStandalonePreview = target.closest(
      "[data-standalone-preview-imported]",
    ) as HTMLElement | null;
    const previewKey = importedStandalonePreview?.getAttribute("data-standalone-preview-imported");
    if (previewKey !== null && previewKey !== undefined) {
      const item = importedItemByKey(previewKey);
      if (item) {
        await openImportedStandalonePreview(item);
      }
      return;
    }

    const importedEdit = target.closest("[data-edit-imported]") as HTMLElement | null;
    if (importedEdit) {
      const item = importedItemByKey(importedEdit.getAttribute("data-edit-imported"));
      if (!item) return;
      openImportedEditor(item);
      renderImportedPanel();
      return;
    }

    const importedDelete = target.closest("[data-delete-imported]") as HTMLElement | null;
    if (importedDelete) {
      const item = importedItemByKey(importedDelete.getAttribute("data-delete-imported"));
      if (!item) return;
      await runImportedAction(async () => {
        importedUi.notice = null;
        renderImportedPanel();

        try {
          await deleteImportedItem(item);
        } catch (error) {
          showImportedResult(errorMessage(error), "error");
        }
      });
    }
  });

  renderImportedPanel();
}

export const libraryPage = {
  mount,
  render,
  load: loadImportedSymbols,
  ensureLoaded,
  invalidate,
  showPreviewNoModels,
};
