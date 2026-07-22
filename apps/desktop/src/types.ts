// Shared type definitions for the HappyJLC desktop frontend.
//
// This module is the single source of truth for the TypeScript interfaces that
// mirror the Rust DTOs exposed through Tauri commands (see
// `apps/desktop/src-tauri/src/lib.rs` and friends). Keeping them here lets every
// page module import the same shapes instead of re-declaring them inline.

import type { ModelFormat } from "./model-preview";

export interface AppState {
  history: [string, string][];
  matched: [string, string][];
  keyword: string;
  export_output_path: string;
  export_last_result: string | null;
  export_show_terminal: boolean;
  export_parallel: number;
  export_running: boolean;
  export_path_mode?: string | null;
  export_symbol?: boolean;
  export_footprint?: boolean;
  export_model_3d?: boolean;
  export_overwrite_symbol?: boolean;
  export_overwrite_footprint?: boolean;
  export_overwrite_model_3d?: boolean;
  export_symbol_fill_color?: string | null;
  monitoring: boolean;
  always_on_top: boolean;
  history_count: number;
  matched_count: number;
  history_save_path: string;
  matched_save_path: string;
  imported_parts_save_path: string;
  default_model_format: ModelFormat;
}

export type ExportTool = "export";
export type ExportMessageKind = "info" | "warn" | "success" | "error";

export interface ExportFinishedPayload {
  tool: ExportTool;
  success: boolean;
  message: string;
}

export interface ExportProgressPayload {
  tool: ExportTool;
  message: string;
  determinate: boolean;
  current: number | null;
  total: number | null;
}

export interface ExportNotice {
  kind: ExportMessageKind;
  message: string;
}

export interface ExportProgressState {
  determinate: boolean;
  current: number;
  total: number;
  message: string;
}

export interface ImportedModel {
  file_name: string;
  format: ModelFormat;
  size_bytes: number;
}

export interface ImportedSymbol {
  library_key: string;
  lcsc_part: string;
  value: string;
  symbol_name: string;
  package: string;
  source_file: string;
  source_kind: string;
  editable: boolean;
  model_available: boolean;
  models: ImportedModel[];
}

export interface LibrarySource {
  path: string;
  kind: string;
  configured: boolean;
}

export interface ImportedSymbolsResponse {
  scanned_path: string;
  sources: LibrarySource[];
  items: ImportedSymbol[];
}

export type Export3dPathMode = "auto" | "project_relative" | "library_relative";
export type ExportAssetKey = "symbol" | "footprint" | "model_3d";
export type ExportField = "export_symbol" | "export_footprint" | "export_model_3d";
export type ExportOverwriteField =
  "export_overwrite_symbol" | "export_overwrite_footprint" | "export_overwrite_model_3d";
export type PageName = "monitor" | "imported" | "inventory" | "settings" | "about";

export interface InventoryLocation {
  location: string;
  quantity: number;
  priority: number;
}

export interface InventoryPart {
  id: string;
  library_lcsc: string | null;
  library_symbol_name: string | null;
  library_source_file: string | null;
  library_missing: boolean;
  supplier_part_number: string | null;
  name: string;
  package: string;
  note: string;
  locations: InventoryLocation[];
}

export interface InventoryCandidate {
  id: string;
  label: string;
  exact_supplier_match: boolean;
}

export interface InventoryLibraryCandidate {
  library_key: string;
  lcsc_part: string;
  label: string;
  has_model: boolean;
  already_in_inventory: boolean;
  source_kind: string;
  source_file: string;
  symbol_name: string;
}

export interface InventoryAllocation {
  part_id: string;
  location: string;
  quantity: number;
}

export interface InventoryResponse {
  revision: number;
  parts: InventoryPart[];
}

export interface BomPreviewRow {
  row_number: number;
  identifier: string;
  references: string;
  supplier_part_number: string | null;
  supplier_part_number_source: string | null;
  supplier_part_number_conflict: boolean;
  name: string;
  package: string;
  quantity_per_board: number;
  required_quantity: number;
  matched_part_id: string | null;
  candidates: InventoryCandidate[];
  library_candidates: InventoryLibraryCandidate[];
  match_kind: string;
  library_status: string;
  model_status: string;
  allocations: InventoryAllocation[];
}

export interface BomPreview {
  path: string;
  boards: number;
  revision: number;
  rows: BomPreviewRow[];
}

export interface BomDeductionRow {
  row_number: number;
  part_id: string | null;
  skipped: boolean;
  allocations: InventoryAllocation[];
}

export interface BomImportRow {
  row_number: number;
  skipped: boolean;
  library_lcsc: string | null;
  library_key: string | null;
}

export interface ImportBomResult {
  imported: number;
  existing: number;
  skipped: number;
  manual: number;
  pending_library: number;
}

export interface ProductionRecord {
  id: number;
  path: string;
  boards: number;
  created_at: string;
  total_rows: number;
  matched_rows: number;
  skipped_rows: number;
}

export interface ExportAssetToggle {
  key: ExportAssetKey;
  labelKey: string;
  exportField: ExportField;
  overwriteField: ExportOverwriteField;
  exportButtonId: string;
  overwriteButtonId: string;
  exportCommand: string;
  overwriteCommand: string;
}

export interface ExportCardOptions {
  tool: ExportTool;
  countId: string;
  buttonId: string;
  matchedCount: number;
  running: boolean;
  exportLabelKey: string;
  runningLabelKey: string;
  statusId: string;
  resultId: string;
  result: string | null;
  buttonDisabled?: boolean;
  derivedNotice?: ExportNotice | null;
}
