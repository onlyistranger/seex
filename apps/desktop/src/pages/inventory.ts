// Inventory page module.
//
// This module owns inventory records, the editor, library picker and BOM
// workflow. The root only coordinates page switching and shared preview UI.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { mountIcons } from "../icons";
import { errorMessage } from "../ipc";
import { librarySourceLabel } from "./library";
import { closeImportedPreview, scheduleImportedPreview } from "../shared/preview";
import type {
  BomDeductionRow,
  BomImportRow,
  BomPreview,
  BomPreviewRow,
  ExportMessageKind,
  ExportNotice,
  ImportBomResult,
  ImportedSymbol,
  ImportedSymbolsResponse,
  InventoryAllocation,
  InventoryLocation,
  InventoryPart,
  InventoryResponse,
  ProductionRecord,
} from "../types";
import { $, applyTooltips, escapeAttr, escapeHtml } from "../utils";

import { translate } from "../i18n";

const t = translate;

const browserPreviewMode = !("__TAURI_INTERNALS__" in window);

let mounted = false;

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

export function render(): void {
  renderInventory();
}

export function ensureLoaded(): void {
  if (!inventoryUi.initialized) {
    void loadInventory();
  }
}

export function mount(): void {
  if (mounted) return;
  mounted = true;

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

  document.addEventListener("click", async (event) => {
    const target = event.target as HTMLElement;

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
  });

  renderInventory();
}

export const inventoryPage = { mount, render, ensureLoaded };
