import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { mountIcons } from "./icons";
import { errorMessage, invokeState } from "./ipc";
import { translate } from "./i18n";
import { exportPage } from "./pages/export";
import { monitorPage } from "./pages/monitor";
import {
  closeImportedPreview,
  closeImportedStandalonePreview,
  importedRowKey,
  openImportedStandalonePreview,
  positionImportedPreview,
  resetImportedStandalonePreview,
  scheduleImportedPreview,
  selectImportedStandalonePreviewModel,
  setNoModelsHandler,
  setPreviewContext,
} from "./shared/preview";
import { $, applyTooltips, escapeAttr, escapeHtml, formatMessage, syncInputValue } from "./utils";
import type {
  AppState,
  ExportFinishedPayload,
  ExportMessageKind,
  ExportNotice,
  ExportProgressPayload,
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

let currentPage: PageName = "monitor";
let lastState: AppState | null = null;

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

function t(key: string): string {
  return translate(key);
}

function normalizeImportedLcscPart(value: string): string {
  return value.trim().toUpperCase();
}

function inventoryLibraryItem(part: InventoryPart): ImportedSymbol | null {
  const lcscPart = part.library_lcsc;
  if (!lcscPart && (!part.library_source_file || !part.library_symbol_name)) return null;
  return (
    (lcscPart
      ? (inventoryUi.libraryItems.find(
          (item) => item.lcsc_part === lcscPart && item.source_file === part.library_source_file,
        ) ?? inventoryUi.libraryItems.find((item) => item.lcsc_part === lcscPart))
      : null) ??
    inventoryUi.libraryItems.find(
      (item) =>
        item.source_file === part.library_source_file &&
        item.symbol_name === part.library_symbol_name,
    ) ??
    null
  );
}

function librarySourceLabel(kind: string): string {
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
  mountIcons();
  applyTooltips();
  renderImportedPanel();
  rerenderState();
}

function switchPage(pageName: PageName) {
  if (pageName !== "imported") {
    closeImportedPreview();
  }
  if (pageName !== "imported") {
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

function renderState(state: AppState) {
  syncInputValue("imported-parts-save-path-input", state.imported_parts_save_path);

  monitorPage.render(state);
  exportPage.render(state);

  renderImportedPanel();
  applyTooltips();
}

function renderImportedList(items: ImportedSymbol[]) {
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
  sourceFilter.innerHTML = `<option value="">全部来源</option>${Array.from(
    new Set(importedUi.items.map((item) => item.source_kind)),
  )
    .sort()
    .map(
      (kind) =>
        `<option value="${escapeAttr(kind)}">${escapeHtml(librarySourceLabel(kind))}</option>`,
    )
    .join("")}`;
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
  sources.innerHTML = importedUi.sources
    .map(
      (source) =>
        `<span class="library-source-chip" title="${escapeAttr(source.path)}"><b>${escapeHtml(librarySourceLabel(source.kind))}</b><small>${escapeHtml(source.path)}</small>${source.configured ? `<button class="icon-only" data-remove-kicad-library="${escapeAttr(source.path)}" title="移除外部库来源" aria-label="移除外部库来源"><i data-lucide="x"></i></button>` : ""}</span>`,
    )
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
  empty.textContent =
    importedUi.items.length > 0 ? t("imported.noFilterResults") : t("imported.empty");
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

async function selectSaveFile(
  title: string,
  defaultPath: string | undefined,
): Promise<string | null> {
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

function classifySaveResult(message: string): ExportMessageKind {
  const lower = message.toLowerCase();
  if (lower.startsWith("saved") || lower.startsWith("exported") || lower.startsWith("queued"))
    return "success";
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
  return id ? (inventoryUi.allParts.find((part) => part.id === id) ?? null) : null;
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
      const existing = inventoryUi.allParts.find((part) =>
        item.lcsc_part
          ? part.library_lcsc === item.lcsc_part
          : part.library_source_file === item.source_file &&
            part.library_symbol_name === item.symbol_name,
      );
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
  const existing = inventoryUi.allParts.find((part) =>
    item.lcsc_part
      ? part.library_lcsc === item.lcsc_part
      : part.library_source_file === item.source_file &&
        part.library_symbol_name === item.symbol_name,
  );
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
  closeImportedPreview();
  const list = $("inventory-list");
  list.innerHTML = inventoryUi.parts
    .map((part) => {
      const libraryItem = inventoryLibraryItem(part);
      const locations = part.locations
        .map(
          (location) =>
            `<span class="inventory-location-chip"><b>${escapeHtml(location.location)}</b><em>${location.quantity}</em></span>`,
        )
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
    .sort(
      (left, right) =>
        left.priority - right.priority || left.location.localeCompare(right.location),
    )
    .map((location, index, locations) => {
      const quantity =
        index === locations.length - 1
          ? Math.max(remaining, 0)
          : Math.min(Math.max(location.quantity, 0), Math.max(remaining, 0));
      remaining -= quantity;
      return { part_id: part.id, location: location.location, quantity };
    });
}

function bomMatchStatus(row: BomPreviewRow): string {
  if (row.supplier_part_number_conflict) return t("inventory.statusConflict");
  if (row.match_kind === "ambiguous") return t("inventory.statusAmbiguous");
  if (row.matched_part_id) return t("inventory.statusInventory");
  if (row.library_candidates.some((candidate) => !candidate.already_in_inventory))
    return t("inventory.statusLibrary");
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
  return t(
    row.model_status === "available" ? "inventory.libraryModel" : "inventory.libraryNoModel",
  );
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
  for (const candidate of row.library_candidates)
    choices.set(
      candidate.library_key,
      `${candidate.label}${candidate.has_model ? ` · ${t("inventory.libraryModel")}` : ` · ${t("inventory.libraryNoModel")}`}`,
    );
  return `<option value="">${escapeHtml(t("inventory.manualRecord"))}</option>${Array.from(
    choices.entries(),
  )
    .map(
      ([key, label]) =>
        `<option value="${escapeAttr(key)}" ${key === selected ? "selected" : ""}>${escapeHtml(label)}</option>`,
    )
    .join("")}`;
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
          const valid =
            skipped ||
            Boolean(
              selectedPart &&
              !row.supplier_part_number_conflict &&
              total === row.required_quantity &&
              row.allocations.every((allocation) => allocation.quantity >= 0),
            );
          const candidates = row.candidates
            .map(
              (candidate) =>
                `<option value="${escapeAttr(candidate.id)}" ${candidate.id === row.matched_part_id ? "selected" : ""}>${escapeHtml(candidate.label)}</option>`,
            )
            .join("");
          const allocationLocations = selectedPart
            ? selectedPart.locations
                .slice()
                .sort(
                  (left, right) =>
                    left.priority - right.priority || left.location.localeCompare(right.location),
                )
                .map((location) => {
                  const allocation = row.allocations.find(
                    (item) => item.location === location.location,
                  );
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
    return (
      !row.supplier_part_number_conflict &&
      Boolean(row.matched_part_id) &&
      allocationTotal(row) === row.required_quantity &&
      row.allocations.every((allocation) => allocation.quantity >= 0)
    );
  });
  ($("btn-confirm-inventory-bom") as HTMLButtonElement).disabled =
    !canConfirm || inventoryUi.bomLoading;
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
    .map(
      (record) =>
        `<div class="production-record"><strong>#${record.id}</strong><span>${escapeHtml(record.path)}</span><small>${record.boards} board(s) · ${record.matched_rows}/${record.total_rows} matched${record.skipped_rows ? ` · ${record.skipped_rows} skipped` : ""} · ${escapeHtml(record.created_at)}</small></div>`,
    )
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
  editor.classList.toggle(
    "hidden",
    inventoryUi.editingId === null &&
      inventoryUi.draftName === "" &&
      inventoryUi.draftLocations.length === 0,
  );
  ($("inventory-supplier") as HTMLInputElement).value = inventoryUi.draftSupplier;
  ($("inventory-name") as HTMLInputElement).value = inventoryUi.draftName;
  const packageInput = $("inventory-package") as HTMLInputElement;
  packageInput.value = inventoryUi.draftPackage;
  packageInput.placeholder =
    inventoryUi.draftLibraryLcsc && !inventoryUi.draftPackage ? t("inventory.packagePending") : "";
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
  $("inventory-production-panel").classList.toggle(
    "hidden",
    inventoryUi.productionRecords.length === 0,
  );
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
    inventoryUi.productionRecords = await invoke<ProductionRecord[]>("get_production_records", {
      limit: 20,
    });
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

function readInventoryDraft(): {
  supplier: string;
  name: string;
  packageName: string;
  note: string;
  locations: InventoryLocation[];
} {
  const locationFields = Array.from(document.querySelectorAll(".inventory-location-row"));
  const locations = locationFields.map((row, index) => ({
    location: (
      (row.querySelector("[data-inventory-location]") as HTMLInputElement)?.value ?? ""
    ).trim(),
    quantity: Number.parseInt(
      (row.querySelector("[data-inventory-quantity]") as HTMLInputElement)?.value ?? "0",
      10,
    ),
    priority: index,
  }));
  return {
    supplier: ($("inventory-supplier") as HTMLInputElement).value.trim(),
    name: ($("inventory-name") as HTMLInputElement).value.trim(),
    packageName: ($("inventory-package") as HTMLInputElement).value.trim(),
    note: ($("inventory-note") as HTMLInputElement).value.trim(),
    locations: locations.map((location) => ({
      ...location,
      quantity: Number.isFinite(location.quantity) ? location.quantity : 0,
    })),
  };
}

async function saveInventoryPart() {
  const draft = readInventoryDraft();
  if (
    !draft.name ||
    (!draft.packageName && !inventoryUi.draftLibraryLcsc) ||
    draft.locations.some((location) => !location.location)
  ) {
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
  if (
    !inventoryUi.bomPath ||
    !/^\d+$/.test(inventoryUi.bomBoards.trim()) ||
    !Number.isSafeInteger(boards) ||
    boards < 1
  ) {
    inventoryUi.bomError = "请选择 CSV，并输入大于零的整数板数。";
    renderInventory();
    return;
  }
  inventoryUi.bomLoading = true;
  inventoryUi.bomError = null;
  renderInventory();
  try {
    const preview = await invoke<BomPreview>("preview_inventory_bom", {
      path: inventoryUi.bomPath,
      boards,
    });
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
        const candidate =
          row.library_candidates.find((item) => item.library_key === key) ??
          inventoryUi.libraryItems.find((item) => item.library_key === key);
        return candidate?.lcsc_part || row.supplier_part_number || null;
      })(),
      library_key: bomLibrarySelection(row) || null,
    }));
    const result = await invoke<ImportBomResult>("import_inventory_bom", {
      request: { path: preview.path, revision: preview.revision, rows },
    });
    inventoryUi.notice = {
      kind: "success",
      message: `已导入 ${result.imported} 条库存记录，${result.existing} 条已存在。`,
    };
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
    await invoke<string>("confirm_inventory_bom", {
      request: {
        path: preview.path,
        boards: preview.boards,
        revision: preview.revision,
        rows: requestRows,
      },
    });
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

async function loadImportedSymbols() {
  closeImportedPreview();
  closeImportedStandalonePreview();
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
  setPreviewContext({
    getDefaultModelFormat: () => lastState?.default_model_format ?? "wrl",
  });
  setNoModelsHandler(() => {
    importedUi.notice = {
      kind: "warn",
      message: t("imported.previewNoModels"),
    };
    renderImportedPanel();
  });
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
      exportPage.onEvent("export-progress", event.payload);
    });
  } catch {
    // Tauri event channels are unavailable in browser-only preview mode.
  }
  try {
    await listen<ExportFinishedPayload>("export-finished", async (event) => {
      exportPage.onEvent("export-finished", event.payload);
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

  exportPage.mount({
    refresh: refreshState,
    selectDirectory,
    queueConfigWrite: queueExportConfigWrite,
    onExportPathChanged: async () => {
      invalidateImportedSymbols(true);
      await refreshState();
      if (currentPage === "imported") {
        await loadImportedSymbols();
      }
    },
  });

  monitorPage.mount({
    refresh: refreshState,
    queueConfigWrite: queueExportConfigWrite,
    selectSaveFile,
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

  $("inventory-list").addEventListener("pointerover", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest(".inventory-row") as HTMLElement | null;
    const related = event.relatedTarget as Node | null;
    if (!row || (related && row.contains(related))) {
      return;
    }

    const part = inventoryUi.parts.find(
      (candidate) => candidate.id === row.dataset.inventoryPreviewRow,
    );
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
    resetImportedStandalonePreview();
  });

  $("imported-standalone-preview-model-select").addEventListener("change", async (event) => {
    const select = event.target as HTMLSelectElement;
    await selectImportedStandalonePreviewModel(select.value);
  });

  $("imported-standalone-preview-modal").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) {
      closeImportedStandalonePreview();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
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
    importedUi.editDraftLcscPart = normalizeImportedLcscPart(
      (event.target as HTMLInputElement).value,
    );
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
        const kind: ExportMessageKind = result.toLowerCase().includes("failed")
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
    inventoryUi.draftLocations = [
      ...draft.locations,
      { location: "", quantity: 0, priority: draft.locations.length },
    ];
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
    ($("inventory-bom-panel") as HTMLElement).scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
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
      const selected = await open({
        title: "Choose BOM CSV",
        multiple: false,
        filters: [
          { name: "CSV", extensions: ["csv"] },
          { name: "All files", extensions: ["*"] },
        ],
      });
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
    const index = Number.parseInt(
      (remove ?? move)?.getAttribute("data-location-index") ?? "-1",
      10,
    );
    if (index < 0 || index >= draft.locations.length) return;
    if (remove) {
      draft.locations.splice(index, 1);
    } else {
      const direction = move?.getAttribute("data-move-inventory-location");
      const nextIndex = direction === "up" ? index - 1 : index + 1;
      if (nextIndex < 0 || nextIndex >= draft.locations.length) return;
      [draft.locations[index], draft.locations[nextIndex]] = [
        draft.locations[nextIndex],
        draft.locations[index],
      ];
    }
    inventoryUi.draftLocations = draft.locations;
    renderInventory();
  });
  $("inventory-location-fields").addEventListener("input", (event) => {
    const target = event.target as HTMLInputElement;
    const index = Number.parseInt(
      target.getAttribute("data-inventory-location") ??
        target.getAttribute("data-inventory-quantity") ??
        "-1",
      10,
    );
    if (index < 0 || !inventoryUi.draftLocations[index]) return;
    if (target.hasAttribute("data-inventory-location")) {
      inventoryUi.draftLocations[index].location = target.value;
    } else {
      inventoryUi.draftLocations[index].quantity = Number.parseInt(target.value, 10) || 0;
    }
  });
  $("inventory-library-list").addEventListener("click", (event) => {
    const target = (event.target as HTMLElement).closest(
      "[data-select-inventory-library]",
    ) as HTMLElement | null;
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
      const row = inventoryUi.bomPreview?.rows.find(
        (item) => item.row_number === Number(candidateRow),
      );
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
      const row = inventoryUi.bomPreview?.rows.find(
        (item) => item.row_number === Number(allocationRow),
      );
      if (!row) return;
      const allocation = row.allocations.find((item) => item.location === location);
      const quantity = Math.max(0, Number.parseInt(target.value, 10) || 0);
      if (allocation) allocation.quantity = quantity;
      else if (row.matched_part_id)
        row.allocations.push({ part_id: row.matched_part_id, location, quantity });
      renderInventoryBomPreview();
    }
  });
  $("inventory-bom-preview").addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const newPartRow = target.closest("[data-new-bom-part]") as HTMLElement | null;
    if (newPartRow) {
      const row = inventoryUi.bomPreview?.rows.find(
        (item) => item.row_number === Number(newPartRow.getAttribute("data-new-bom-part")),
      );
      if (!row) return;
      openInventoryEditor();
      inventoryUi.libraryPickerQuery = [row.supplier_part_number ?? "", row.name, row.package]
        .join(" ")
        .trim();
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
        await invoke("adjust_inventory_stock", {
          adjustment: { part_id: partId, location, delta },
        });
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

    const importedStandalonePreviewEl = target.closest(
      "[data-standalone-preview-imported]",
    ) as HTMLElement | null;
    const importedStandalonePreview = importedStandalonePreviewEl?.getAttribute(
      "data-standalone-preview-imported",
    );
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
  });
});
