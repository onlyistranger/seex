use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_paths::AppPaths;

const UNALLOCATED_LOCATION: &str = "未分配";

#[derive(Debug)]
pub struct InventoryRepository {
    connection: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryPart {
    pub id: String,
    pub library_lcsc: Option<String>,
    pub library_symbol_name: Option<String>,
    pub library_source_file: Option<String>,
    pub library_missing: bool,
    pub supplier_part_number: Option<String>,
    pub name: String,
    pub package: String,
    pub note: String,
    pub locations: Vec<InventoryLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLocation {
    pub location: String,
    pub quantity: i64,
    pub priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryPartInput {
    pub id: Option<String>,
    pub library_lcsc: Option<String>,
    pub library_symbol_name: Option<String>,
    pub library_source_file: Option<String>,
    pub supplier_part_number: Option<String>,
    pub name: String,
    pub package: String,
    pub note: String,
    pub locations: Vec<InventoryLocation>,
}

#[derive(Debug, Clone)]
pub struct InventoryLibraryPart {
    pub library_key: String,
    pub lcsc_part: String,
    pub value: String,
    pub symbol_name: String,
    pub package: String,
    pub source_file: String,
    pub has_model: bool,
    pub source_kind: String,
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryStockAdjustment {
    pub part_id: String,
    pub location: String,
    pub delta: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryResponse {
    pub revision: i64,
    pub parts: Vec<InventoryPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomPreview {
    pub path: String,
    pub boards: u64,
    pub revision: i64,
    pub rows: Vec<BomPreviewRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomPreviewRow {
    pub row_number: usize,
    pub identifier: String,
    pub references: String,
    pub supplier_part_number: Option<String>,
    pub supplier_part_number_source: Option<String>,
    pub supplier_part_number_conflict: bool,
    pub name: String,
    pub package: String,
    pub quantity_per_board: u64,
    pub required_quantity: u64,
    pub matched_part_id: Option<String>,
    pub candidates: Vec<InventoryCandidate>,
    pub library_candidates: Vec<InventoryLibraryCandidate>,
    pub match_kind: String,
    pub library_status: String,
    pub model_status: String,
    pub allocations: Vec<InventoryAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryCandidate {
    pub id: String,
    pub label: String,
    pub exact_supplier_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLibraryCandidate {
    pub library_key: String,
    pub lcsc_part: String,
    pub label: String,
    pub has_model: bool,
    pub already_in_inventory: bool,
    pub source_kind: String,
    pub source_file: String,
    pub symbol_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomImportRow {
    pub row_number: usize,
    pub skipped: bool,
    pub library_lcsc: Option<String>,
    #[serde(default)]
    pub library_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBomRequest {
    pub path: String,
    pub revision: i64,
    pub rows: Vec<BomImportRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBomResult {
    pub imported: usize,
    pub existing: usize,
    pub skipped: usize,
    pub manual: usize,
    pub pending_library: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAllocation {
    pub part_id: String,
    pub location: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDeductionRow {
    pub row_number: usize,
    pub part_id: Option<String>,
    pub skipped: bool,
    pub allocations: Vec<InventoryAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmBomDeductionRequest {
    pub path: String,
    pub boards: u64,
    pub revision: i64,
    pub rows: Vec<BomDeductionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionRecord {
    pub id: i64,
    pub path: String,
    pub boards: u64,
    pub created_at: String,
    pub total_rows: usize,
    pub matched_rows: usize,
    pub skipped_rows: usize,
}

#[derive(Debug, Clone)]
struct BomRow {
    row_number: usize,
    identifier: String,
    references: String,
    supplier_part_number: Option<String>,
    supplier_part_number_source: Option<String>,
    supplier_part_number_conflict: bool,
    name: String,
    package: String,
    quantity: u64,
}

impl InventoryRepository {
    #[cfg(test)]
    fn in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory().map_err(db_error)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
            .map_err(db_error)?;
        let mut repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }

    pub fn open(paths: &AppPaths) -> Result<Self, String> {
        let path = paths.inventory_database_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Create inventory directory failed: {err}"))?;
        }
        let connection = Connection::open(path).map_err(db_error)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
            .map_err(db_error)?;
        let mut repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&mut self) -> Result<(), String> {
        let mut version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(db_error)?;
        if version == 0 {
            self.connection
                .execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS parts (
                        id TEXT PRIMARY KEY NOT NULL,
                        library_lcsc TEXT,
                        library_symbol_name TEXT,
                        library_source_file TEXT,
                        library_missing INTEGER NOT NULL DEFAULT 0,
                        supplier_part_number TEXT,
                        name TEXT NOT NULL,
                        package TEXT NOT NULL,
                        note TEXT NOT NULL DEFAULT '',
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_parts_supplier ON parts(supplier_part_number);
                    CREATE INDEX IF NOT EXISTS idx_parts_name_package ON parts(name, package);
                    CREATE INDEX IF NOT EXISTS idx_parts_library_lcsc ON parts(library_lcsc);

                    CREATE TABLE IF NOT EXISTS locations (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        code TEXT NOT NULL UNIQUE
                    );

                    CREATE TABLE IF NOT EXISTS part_stock (
                        part_id TEXT NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
                        location_id INTEGER NOT NULL REFERENCES locations(id),
                        quantity INTEGER NOT NULL DEFAULT 0,
                        priority INTEGER NOT NULL DEFAULT 0,
                        PRIMARY KEY (part_id, location_id)
                    );

                    CREATE TABLE IF NOT EXISTS inventory_meta (
                        key TEXT PRIMARY KEY NOT NULL,
                        value INTEGER NOT NULL
                    );
                    INSERT OR IGNORE INTO inventory_meta(key, value) VALUES ('revision', 0);

                    CREATE TABLE IF NOT EXISTS production_records (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        csv_path TEXT NOT NULL,
                        boards INTEGER NOT NULL,
                        created_at TEXT NOT NULL,
                        total_rows INTEGER NOT NULL,
                        matched_rows INTEGER NOT NULL,
                        skipped_rows INTEGER NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS production_items (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        production_record_id INTEGER NOT NULL REFERENCES production_records(id) ON DELETE CASCADE,
                        row_number INTEGER NOT NULL,
                        identifier_text TEXT NOT NULL DEFAULT '',
                        references_text TEXT NOT NULL,
                        supplier_part_number TEXT,
                        name TEXT NOT NULL,
                        package TEXT NOT NULL,
                        quantity_per_board INTEGER NOT NULL,
                        required_quantity INTEGER NOT NULL,
                        part_id TEXT REFERENCES parts(id) ON DELETE SET NULL,
                        skipped_reason TEXT
                    );

                    CREATE TABLE IF NOT EXISTS production_allocations (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        production_item_id INTEGER NOT NULL REFERENCES production_items(id) ON DELETE CASCADE,
                        location_id INTEGER NOT NULL REFERENCES locations(id),
                        quantity INTEGER NOT NULL
                    );
                    PRAGMA user_version = 3;
                    ",
                )
                .map_err(db_error)?;
        }
        if version == 1 {
            self.connection
                .execute_batch(
                    "ALTER TABLE production_items ADD COLUMN identifier_text TEXT NOT NULL DEFAULT ''; PRAGMA user_version = 2;",
                )
                .map_err(db_error)?;
            version = 2;
        }
        if version == 2 {
            self.connection
                .execute_batch(
                    "ALTER TABLE parts ADD COLUMN library_lcsc TEXT; ALTER TABLE parts ADD COLUMN library_symbol_name TEXT; ALTER TABLE parts ADD COLUMN library_source_file TEXT; ALTER TABLE parts ADD COLUMN library_missing INTEGER NOT NULL DEFAULT 0; CREATE INDEX IF NOT EXISTS idx_parts_library_lcsc ON parts(library_lcsc); PRAGMA user_version = 3;",
                )
                .map_err(db_error)?;
            version = 3;
            self.backfill_legacy_library_links()?;
        }
        if version > 3 {
            return Err(format!("Unsupported inventory database version: {version}"));
        }
        Ok(())
    }

    pub fn get_parts(&self, query: &str) -> Result<InventoryResponse, String> {
        let filter = query.trim().to_lowercase();
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, library_lcsc, library_symbol_name, library_source_file, library_missing,
                        supplier_part_number, name, package, note
                 FROM parts
                 WHERE lower(id || ' ' || coalesce(library_lcsc, '') || ' ' || coalesce(library_symbol_name, '') || ' ' || coalesce(library_source_file, '') || ' ' || coalesce(supplier_part_number, '') || ' ' || name || ' ' || package || ' ' || note) LIKE ?1
                    OR EXISTS (
                        SELECT 1 FROM part_stock
                        JOIN locations ON locations.id = part_stock.location_id
                        WHERE part_stock.part_id = parts.id AND lower(locations.code) LIKE ?2
                    )
                 ORDER BY name COLLATE NOCASE, id",
            )
            .map_err(db_error)?;
        let pattern = format!("%{filter}%");
        let rows = statement
            .query_map(params![pattern, pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(db_error)?;
        let mut parts = Vec::new();
        for row in rows {
            let (
                id,
                library_lcsc,
                library_symbol_name,
                library_source_file,
                library_missing,
                supplier_part_number,
                name,
                package,
                note,
            ) = row.map_err(db_error)?;
            parts.push(InventoryPart {
                locations: self.locations_for_part(&id)?,
                id,
                library_lcsc,
                library_symbol_name,
                library_source_file,
                library_missing,
                supplier_part_number,
                name,
                package,
                note,
            });
        }
        Ok(InventoryResponse {
            revision: self.revision()?,
            parts,
        })
    }

    pub fn save_part(&mut self, input: InventoryPartInput) -> Result<(), String> {
        let library_lcsc = normalize_optional(input.library_lcsc.clone());
        let name = required_text(&input.name, "Name")?;
        let package = if library_lcsc.is_some() {
            input.package.trim().to_string()
        } else {
            required_text(&input.package, "Package")?
        };
        let id = input.id.unwrap_or_else(new_id);
        let locations = normalize_locations(input.locations)?;
        let now = chrono::Utc::now().to_rfc3339();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(library_lcsc) = &library_lcsc {
            let duplicate: Option<String> = tx
                .query_row(
                    "SELECT id FROM parts WHERE library_lcsc = ?1 AND id != ?2",
                    params![library_lcsc, id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            if duplicate.is_some() {
                return Err(format!(
                    "Library component {library_lcsc} is already in inventory"
                ));
            }
        }
        tx.execute(
            "INSERT INTO parts(id, library_lcsc, library_symbol_name, library_source_file, library_missing, supplier_part_number, name, package, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET library_lcsc = excluded.library_lcsc,
                 library_symbol_name = excluded.library_symbol_name, library_source_file = excluded.library_source_file,
                 library_missing = excluded.library_missing, supplier_part_number = excluded.supplier_part_number,
                 name = excluded.name, package = excluded.package, note = excluded.note, updated_at = excluded.updated_at",
            params![
                id,
                library_lcsc,
                normalize_text(input.library_symbol_name),
                normalize_text(input.library_source_file),
                normalize_optional(input.supplier_part_number),
                name,
                package,
                input.note.trim(),
                now
            ],
        )
        .map_err(db_error)?;
        tx.execute("DELETE FROM part_stock WHERE part_id = ?1", [&id])
            .map_err(db_error)?;
        if locations.is_empty() {
            insert_stock(&tx, &id, UNALLOCATED_LOCATION, 0, 0)?;
        } else {
            for location in locations {
                insert_stock(
                    &tx,
                    &id,
                    &location.location,
                    location.quantity,
                    location.priority,
                )?;
            }
        }
        bump_revision(&tx)?;
        tx.commit().map_err(db_error)
    }

    pub fn delete_part(&mut self, id: &str) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let changed = tx
            .execute("DELETE FROM parts WHERE id = ?1", [id])
            .map_err(db_error)?;
        if changed == 0 {
            return Err("Inventory part not found".to_string());
        }
        bump_revision(&tx)?;
        tx.commit().map_err(db_error)
    }

    pub fn adjust_stock(&mut self, adjustment: InventoryStockAdjustment) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT id FROM parts WHERE id = ?1",
                [&adjustment.part_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if exists.is_none() {
            return Err("Inventory part not found".to_string());
        }
        let location_id = ensure_location(&tx, &adjustment.location)?;
        let changed = tx
            .execute(
                "UPDATE part_stock SET quantity = quantity + ?1 WHERE part_id = ?2 AND location_id = ?3",
                params![adjustment.delta, adjustment.part_id, location_id],
            )
            .map_err(db_error)?;
        if changed == 0 {
            tx.execute(
                "INSERT INTO part_stock(part_id, location_id, quantity, priority) VALUES (?1, ?2, ?3, 0)",
                params![adjustment.part_id, location_id, adjustment.delta],
            )
            .map_err(db_error)?;
        }
        bump_revision(&tx)?;
        tx.commit().map_err(db_error)
    }

    pub fn import_supplier_parts(&mut self, supplier_parts: &[String]) -> Result<usize, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut added = 0;
        for supplier_part in supplier_parts {
            let supplier_part = supplier_part.trim().to_uppercase();
            if supplier_part.is_empty() {
                continue;
            }
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM parts WHERE supplier_part_number = ?1",
                    [&supplier_part],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            if exists.is_some() {
                continue;
            }
            let id = new_id();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO parts(id, supplier_part_number, name, package, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, '', ?5, ?5)",
                params![id, supplier_part, "待补充", "待补充", now],
            )
            .map_err(db_error)?;
            insert_stock(&tx, &id, UNALLOCATED_LOCATION, 0, 0)?;
            added += 1;
        }
        if added > 0 {
            bump_revision(&tx)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(added)
    }

    pub fn sync_library(
        &mut self,
        library_parts: &[InventoryLibraryPart],
    ) -> Result<usize, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut changed = tx
            .execute(
                "UPDATE parts SET library_missing = 1 WHERE (library_lcsc IS NOT NULL OR library_source_file IS NOT NULL) AND library_missing = 0",
                [],
            )
            .map_err(db_error)?
            > 0;
        let mut seen = HashSet::new();
        for item in library_parts {
            let identity = if let Some(library_lcsc) =
                normalize_optional(Some(item.lcsc_part.clone()))
            {
                format!("lcsc:{library_lcsc}")
            } else if !item.source_file.trim().is_empty() && !item.symbol_name.trim().is_empty() {
                format!("source:{}\u{001f}{}", item.source_file, item.symbol_name)
            } else {
                continue;
            };
            if !seen.insert(identity) {
                continue;
            }
            let value = required_text(&item.value, "Library value")?;
            let symbol_name = required_text(&item.symbol_name, "Library symbol")?;
            let package = item.package.trim();
            type ExistingPart = (
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                String,
                i64,
            );
            let current: Option<ExistingPart> = if let Some(library_lcsc) =
                normalize_optional(Some(item.lcsc_part.clone()))
            {
                tx.query_row(
                        "SELECT library_symbol_name, library_source_file, supplier_part_number, name, package, library_missing FROM parts WHERE upper(replace(coalesce(library_lcsc, ''), ' ', '')) = ?1",
                        [&library_lcsc],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                    ).optional().map_err(db_error)?
            } else {
                tx.query_row(
                        "SELECT library_symbol_name, library_source_file, supplier_part_number, name, package, library_missing FROM parts WHERE library_source_file = ?1 AND library_symbol_name = ?2",
                        params![item.source_file, symbol_name],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                    ).optional().map_err(db_error)?
            };
            let Some((
                current_symbol,
                current_source,
                current_supplier,
                current_name,
                current_package,
                current_missing,
            )) = current
            else {
                continue;
            };
            let next_package = if package.is_empty() {
                current_package.clone()
            } else {
                package.to_string()
            };
            let normalized_lcsc = normalize_optional(Some(item.lcsc_part.clone()));
            if current_symbol.as_deref() != Some(symbol_name.as_str())
                || current_source.as_deref() != Some(item.source_file.as_str())
                || current_supplier.as_deref() != normalized_lcsc.as_deref()
                || current_name != value
                || next_package != current_package
                || current_missing != 0
            {
                tx.execute(
                    "UPDATE parts SET library_lcsc = ?1, library_symbol_name = ?2, library_source_file = ?3, supplier_part_number = ?1, name = ?4, package = ?5, library_missing = 0, updated_at = ?6 WHERE ((?1 IS NOT NULL AND upper(replace(coalesce(library_lcsc, ''), ' ', '')) = ?1) OR (?1 IS NULL AND library_source_file = ?3 AND library_symbol_name = ?2))",
                    params![normalized_lcsc, symbol_name, item.source_file, value, next_package, Utc::now().to_rfc3339()],
                )
                .map_err(db_error)?;
                changed = true;
            }
        }
        if changed {
            bump_revision(&tx)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(changed as usize)
    }

    pub fn import_library_parts(
        &mut self,
        library_parts: &[InventoryLibraryPart],
    ) -> Result<usize, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut added = 0;
        for item in library_parts {
            let library_lcsc = normalize_optional(Some(item.lcsc_part.clone()));
            if library_lcsc.is_none()
                && (item.source_file.trim().is_empty() || item.symbol_name.trim().is_empty())
            {
                continue;
            }
            let value = required_text(&item.value, "Library value")?;
            let package = item.package.trim().to_string();
            let exists: Option<String> = if let Some(library_lcsc) = &library_lcsc {
                tx.query_row(
                    "SELECT id FROM parts WHERE upper(replace(coalesce(library_lcsc, ''), ' ', '')) = ?1",
                    [library_lcsc],
                    |row| row.get(0),
                ).optional().map_err(db_error)?
            } else {
                tx.query_row(
                    "SELECT id FROM parts WHERE library_source_file = ?1 AND library_symbol_name = ?2",
                    params![item.source_file, item.symbol_name],
                    |row| row.get(0),
                ).optional().map_err(db_error)?
            };
            if exists.is_some() {
                continue;
            }
            let id = new_id();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO parts(id, library_lcsc, library_symbol_name, library_source_file, library_missing, supplier_part_number, name, package, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?2, ?5, ?6, '', ?7, ?7)",
                params![id, library_lcsc, item.symbol_name, item.source_file, value, package, now],
            )
            .map_err(db_error)?;
            insert_stock(&tx, &id, UNALLOCATED_LOCATION, 0, 0)?;
            added += 1;
        }
        if added > 0 {
            bump_revision(&tx)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(added)
    }

    fn backfill_legacy_library_links(&mut self) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE parts
                 SET library_lcsc = upper(trim(supplier_part_number)), library_missing = 1
                 WHERE library_lcsc IS NULL AND trim(coalesce(supplier_part_number, '')) <> ''",
                [],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn preview_csv(
        &self,
        path: &str,
        boards: u64,
        library_parts: &[InventoryLibraryPart],
    ) -> Result<BomPreview, String> {
        if boards == 0 {
            return Err("Boards must be greater than zero".to_string());
        }
        let source_rows = parse_bom_csv(Path::new(path))?;
        let inventory = self.get_parts("")?;
        let rows = source_rows
            .into_iter()
            .map(|row| preview_row(row, boards, &inventory.parts, library_parts))
            .collect();
        Ok(BomPreview {
            path: path.to_string(),
            boards,
            revision: inventory.revision,
            rows,
        })
    }

    pub fn import_bom(
        &mut self,
        request: ImportBomRequest,
        library_parts: &[InventoryLibraryPart],
    ) -> Result<ImportBomResult, String> {
        if request.rows.is_empty() {
            return Err("BOM import request is empty".to_string());
        }
        let source_rows = parse_bom_csv(Path::new(&request.path))?;
        let source_by_row: HashMap<usize, BomRow> = source_rows
            .iter()
            .cloned()
            .map(|row| (row.row_number, row))
            .collect();
        let mut request_by_row = HashMap::new();
        for row in &request.rows {
            if request_by_row.insert(row.row_number, row).is_some() {
                return Err(format!("Duplicate BOM row {}", row.row_number));
            }
        }
        if source_by_row.len() != request_by_row.len()
            || source_by_row
                .keys()
                .any(|row| !request_by_row.contains_key(row))
        {
            return Err("BOM import is incomplete; please preview the CSV again".to_string());
        }

        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if revision_from(&tx)? != request.revision {
            return Err(
                "Inventory changed since preview; please preview the BOM again".to_string(),
            );
        }

        let mut result = ImportBomResult {
            imported: 0,
            existing: 0,
            skipped: 0,
            manual: 0,
            pending_library: 0,
        };

        for selected in &request.rows {
            if selected.skipped {
                result.skipped += 1;
                continue;
            }
            let source = source_by_row
                .get(&selected.row_number)
                .expect("validated above");
            let selected_lcsc = normalize_optional(selected.library_lcsc.clone())
                .or_else(|| source.supplier_part_number.clone());
            let library = selected
                .library_key
                .as_deref()
                .and_then(|key| library_parts.iter().find(|part| part.library_key == key))
                .or_else(|| {
                    selected_lcsc.as_deref().and_then(|lcsc| {
                        let mut matches = library_parts.iter().filter(|part| {
                            !part.lcsc_part.is_empty() && part.lcsc_part.eq_ignore_ascii_case(lcsc)
                        });
                        let first = matches.next();
                        first.filter(|_| matches.next().is_none())
                    })
                });
            let lcsc = selected_lcsc.clone();
            let (library_lcsc, library_missing, supplier, name, package, symbol, source_file) =
                if let Some(library) = library {
                    let library_lcsc = normalize_optional(Some(library.lcsc_part.clone()));
                    (
                        library_lcsc.clone(),
                        false,
                        library_lcsc,
                        library.value.clone(),
                        if library.package.trim().is_empty() {
                            source.package.clone()
                        } else {
                            library.package.clone()
                        },
                        Some(library.symbol_name.clone()),
                        Some(library.source_file.clone()),
                    )
                } else {
                    (
                        lcsc.clone(),
                        lcsc.is_some(),
                        lcsc.clone(),
                        source.name.clone(),
                        source.package.clone(),
                        None,
                        None,
                    )
                };

            let existing: Option<String> = if let Some(lcsc) = &library_lcsc {
                tx.query_row(
                    "SELECT id FROM parts
                     WHERE upper(replace(coalesce(library_lcsc, ''), ' ', '')) = ?1
                        OR upper(replace(coalesce(supplier_part_number, ''), ' ', '')) = ?1
                     LIMIT 1",
                    [lcsc],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?
            } else if let (Some(source_file), Some(symbol)) = (&source_file, &symbol) {
                tx.query_row(
                    "SELECT id FROM parts WHERE library_source_file = ?1 AND library_symbol_name = ?2 LIMIT 1",
                    params![source_file, symbol],
                    |row| row.get(0),
                ).optional().map_err(db_error)?
            } else {
                let mut statement = tx
                    .prepare("SELECT id, name, package FROM parts")
                    .map_err(db_error)?;
                let mut rows = statement.query([]).map_err(db_error)?;
                let mut matching_id = None;
                while let Some(row) = rows.next().map_err(db_error)? {
                    let candidate_name: String = row.get(1).map_err(db_error)?;
                    let candidate_package: String = row.get(2).map_err(db_error)?;
                    if component_name_key(&candidate_name) == component_name_key(&name)
                        && package_key(&candidate_package) == package_key(&package)
                    {
                        matching_id = Some(row.get(0).map_err(db_error)?);
                        break;
                    }
                }
                matching_id
            };
            if existing.is_some() {
                result.existing += 1;
                continue;
            }

            let id = new_id();
            let now = Utc::now().to_rfc3339();
            let note = if library_missing {
                "BOM 导入，待绑定库元件"
            } else if library_lcsc.is_none() {
                result.manual += 1;
                "BOM 导入，手动元件"
            } else {
                "BOM 导入"
            };
            if library_missing {
                result.pending_library += 1;
            }
            tx.execute(
                "INSERT INTO parts(id, library_lcsc, library_symbol_name, library_source_file, library_missing, supplier_part_number, name, package, note, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    id,
                    library_lcsc,
                    symbol,
                    source_file,
                    library_missing,
                    supplier,
                    name,
                    package,
                    note,
                    now,
                ],
            )
            .map_err(db_error)?;
            insert_stock(&tx, &id, UNALLOCATED_LOCATION, 0, 0)?;
            result.imported += 1;
        }

        if result.imported > 0 {
            bump_revision(&tx)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    pub fn confirm_csv(&mut self, request: ConfirmBomDeductionRequest) -> Result<String, String> {
        if request.boards == 0 || request.rows.is_empty() {
            return Err("Invalid BOM deduction request".to_string());
        }
        let source_rows = parse_bom_csv(Path::new(&request.path))?;
        let source_by_row: HashMap<usize, BomRow> = source_rows
            .iter()
            .cloned()
            .map(|row| (row.row_number, row))
            .collect();
        let mut request_by_row = HashMap::new();
        for row in &request.rows {
            if request_by_row.insert(row.row_number, row).is_some() {
                return Err(format!("Duplicate BOM row {}", row.row_number));
            }
        }
        if source_by_row.len() != request_by_row.len()
            || source_by_row
                .keys()
                .any(|row| !request_by_row.contains_key(row))
        {
            return Err("BOM preview is incomplete; please preview the CSV again".to_string());
        }

        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if revision_from(&tx)? != request.revision {
            return Err(
                "Inventory changed since preview; please preview the CSV again".to_string(),
            );
        }
        let matched_rows = request.rows.iter().filter(|row| !row.skipped).count();
        let skipped_rows = request.rows.len() - matched_rows;
        let created_at = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO production_records(csv_path, boards, created_at, total_rows, matched_rows, skipped_rows)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![request.path, request.boards as i64, created_at, request.rows.len() as i64, matched_rows as i64, skipped_rows as i64],
        )
        .map_err(db_error)?;
        let record_id = tx.last_insert_rowid();

        for row in &request.rows {
            let source = source_by_row.get(&row.row_number).expect("validated above");
            let required = source.quantity.saturating_mul(request.boards);
            if row.skipped {
                tx.execute(
                    "INSERT INTO production_items(production_record_id, row_number, identifier_text, references_text, supplier_part_number, name, package, quantity_per_board, required_quantity, skipped_reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![record_id, source.row_number as i64, source.identifier, source.references, source.supplier_part_number, source.name, source.package, source.quantity as i64, required as i64, "用户跳过"],
                )
                .map_err(db_error)?;
                continue;
            }
            let part_id = row.part_id.as_deref().ok_or_else(|| {
                format!("BOM row {} has no selected inventory part", row.row_number)
            })?;
            let allocation_total: i64 = row
                .allocations
                .iter()
                .map(|allocation| allocation.quantity)
                .sum();
            if allocation_total != required as i64
                || row.allocations.is_empty()
                || row
                    .allocations
                    .iter()
                    .any(|allocation| allocation.quantity <= 0 || allocation.part_id != part_id)
            {
                return Err(format!("Invalid allocation for BOM row {}", row.row_number));
            }
            let mut locations_seen = HashSet::new();
            let part_exists: Option<String> = tx
                .query_row("SELECT id FROM parts WHERE id = ?1", [part_id], |db_row| {
                    db_row.get(0)
                })
                .optional()
                .map_err(db_error)?;
            if part_exists.is_none() {
                return Err(format!(
                    "Inventory part not found for BOM row {}",
                    row.row_number
                ));
            }
            tx.execute(
                "INSERT INTO production_items(production_record_id, row_number, identifier_text, references_text, supplier_part_number, name, package, quantity_per_board, required_quantity, part_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![record_id, source.row_number as i64, source.identifier, source.references, source.supplier_part_number, source.name, source.package, source.quantity as i64, required as i64, part_id],
            )
            .map_err(db_error)?;
            let item_id = tx.last_insert_rowid();
            for allocation in &row.allocations {
                if !locations_seen.insert(allocation.location.clone()) {
                    return Err(format!("Duplicate location in BOM row {}", row.row_number));
                }
                let location_id = ensure_location(&tx, &allocation.location)?;
                let changed = tx
                    .execute(
                        "UPDATE part_stock SET quantity = quantity - ?1 WHERE part_id = ?2 AND location_id = ?3",
                        params![allocation.quantity, part_id, location_id],
                    )
                    .map_err(db_error)?;
                if changed == 0 {
                    return Err(format!(
                        "Location not found for BOM row {}: {}",
                        row.row_number, allocation.location
                    ));
                }
                tx.execute(
                    "INSERT INTO production_allocations(production_item_id, location_id, quantity) VALUES (?1, ?2, ?3)",
                    params![item_id, location_id, allocation.quantity],
                )
                .map_err(db_error)?;
            }
        }
        bump_revision(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(format!(
            "Production record {record_id} saved; deducted {} board(s)",
            request.boards
        ))
    }

    pub fn production_records(&self, limit: usize) -> Result<Vec<ProductionRecord>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, csv_path, boards, created_at, total_rows, matched_rows, skipped_rows
                 FROM production_records ORDER BY id DESC LIMIT ?1",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map([limit.clamp(1, 100) as i64], |row| {
                Ok(ProductionRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    boards: row.get::<_, i64>(2)? as u64,
                    created_at: row.get(3)?,
                    total_rows: row.get::<_, i64>(4)? as usize,
                    matched_rows: row.get::<_, i64>(5)? as usize,
                    skipped_rows: row.get::<_, i64>(6)? as usize,
                })
            })
            .map_err(db_error)?;
        rows.map(|row| row.map_err(db_error)).collect()
    }

    fn locations_for_part(&self, part_id: &str) -> Result<Vec<InventoryLocation>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT locations.code, part_stock.quantity, part_stock.priority
                 FROM part_stock JOIN locations ON locations.id = part_stock.location_id
                 WHERE part_stock.part_id = ?1 ORDER BY part_stock.priority, locations.code",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map([part_id], |row| {
                Ok(InventoryLocation {
                    location: row.get(0)?,
                    quantity: row.get(1)?,
                    priority: row.get::<_, i64>(2)? as u32,
                })
            })
            .map_err(db_error)?;
        rows.map(|row| row.map_err(db_error)).collect()
    }

    fn revision(&self) -> Result<i64, String> {
        self.connection
            .query_row(
                "SELECT value FROM inventory_meta WHERE key = 'revision'",
                [],
                |row| row.get(0),
            )
            .map_err(db_error)
    }
}

fn preview_row(
    row: BomRow,
    boards: u64,
    parts: &[InventoryPart],
    library_parts: &[InventoryLibraryPart],
) -> BomPreviewRow {
    let candidates = find_candidates(&row, parts);
    let library_candidates = find_library_candidates(&row, parts, library_parts);
    let matched_part_id = (candidates.len() == 1).then(|| candidates[0].id.clone());
    let allocations = matched_part_id
        .as_deref()
        .and_then(|id| parts.iter().find(|part| part.id == id))
        .map(|part| allocate(part, row.quantity.saturating_mul(boards)))
        .unwrap_or_default();
    let match_kind = if row.supplier_part_number_conflict {
        "conflict"
    } else if candidates.len() > 1 || library_candidates.len() > 1 {
        "ambiguous"
    } else if !candidates.is_empty() && row.supplier_part_number.is_some() {
        "inventory_supplier"
    } else if !library_candidates.is_empty() && row.supplier_part_number.is_some() {
        "library_supplier"
    } else if !candidates.is_empty() {
        "inventory_name_package"
    } else if !library_candidates.is_empty() {
        "library_name_package"
    } else {
        "unmatched"
    };
    let matched_part = matched_part_id
        .as_deref()
        .and_then(|id| parts.iter().find(|part| part.id == id));
    let library_status = if row.supplier_part_number_conflict
        || candidates.len() > 1
        || library_candidates.len() > 1
    {
        "ambiguous"
    } else if let Some(part) = matched_part {
        if part.library_lcsc.is_some() && !part.library_missing {
            "bound"
        } else if part.library_lcsc.is_some() {
            "missing"
        } else {
            "manual"
        }
    } else if !library_candidates.is_empty() {
        "available"
    } else if row.supplier_part_number.is_some() {
        "missing"
    } else {
        "manual"
    };
    let model_available = matched_part
        .and_then(|part| part.library_lcsc.as_deref())
        .and_then(|lcsc| {
            library_parts.iter().find(|library| {
                normalize_identifier(&library.lcsc_part) == normalize_identifier(lcsc)
            })
        })
        .map(|library| library.has_model)
        .unwrap_or_else(|| {
            library_candidates
                .iter()
                .any(|candidate| candidate.has_model)
        });
    let model_status = if model_available {
        "available"
    } else {
        "missing"
    };
    BomPreviewRow {
        row_number: row.row_number,
        identifier: row.identifier,
        references: row.references,
        supplier_part_number: row.supplier_part_number,
        supplier_part_number_source: row.supplier_part_number_source,
        supplier_part_number_conflict: row.supplier_part_number_conflict,
        name: row.name,
        package: row.package,
        quantity_per_board: row.quantity,
        required_quantity: row.quantity.saturating_mul(boards),
        matched_part_id,
        candidates,
        library_candidates,
        match_kind: match_kind.to_string(),
        library_status: library_status.to_string(),
        model_status: model_status.to_string(),
        allocations,
    }
}

fn find_candidates(row: &BomRow, parts: &[InventoryPart]) -> Vec<InventoryCandidate> {
    let supplier = row.supplier_part_number.as_deref();
    let mut candidates = parts
        .iter()
        .filter(|part| {
            if row.supplier_part_number_conflict {
                return false;
            }
            match supplier {
                Some(supplier) => part_supplier_part_number(part).is_some_and(|value| {
                    normalize_identifier(value) == normalize_identifier(supplier)
                }),
                None => {
                    component_name_key(&part.name) == component_name_key(&row.name)
                        && package_key(&part.package) == package_key(&row.package)
                }
            }
        })
        .map(|part| InventoryCandidate {
            id: part.id.clone(),
            label: part_label(part),
            exact_supplier_match: supplier.is_some(),
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    candidates
}

fn find_library_candidates(
    row: &BomRow,
    parts: &[InventoryPart],
    library_parts: &[InventoryLibraryPart],
) -> Vec<InventoryLibraryCandidate> {
    if row.supplier_part_number_conflict {
        return Vec::new();
    }
    let mut candidates = library_parts
        .iter()
        .filter(|library| match row.supplier_part_number.as_deref() {
            Some(supplier) => {
                normalize_identifier(&library.lcsc_part) == normalize_identifier(supplier)
            }
            None => {
                component_name_key(&library.value) == component_name_key(&row.name)
                    && package_key(&library.package) == package_key(&row.package)
            }
        })
        .map(|library| InventoryLibraryCandidate {
            library_key: library.library_key.clone(),
            lcsc_part: library.lcsc_part.clone(),
            label: format!(
                "{} · {} · {} · {}",
                library.value,
                if library.package.is_empty() {
                    "封装待补充"
                } else {
                    &library.package
                },
                if library.lcsc_part.is_empty() {
                    "无供应商编号"
                } else {
                    &library.lcsc_part
                },
                library.source_kind
            ),
            has_model: library.has_model,
            already_in_inventory: parts.iter().any(|part| {
                if library.lcsc_part.is_empty() {
                    part.library_source_file.as_deref() == Some(library.source_file.as_str())
                        && part.library_symbol_name.as_deref() == Some(library.symbol_name.as_str())
                } else {
                    part_supplier_part_number(part)
                        .is_some_and(|value| value.eq_ignore_ascii_case(&library.lcsc_part))
                }
            }),
            source_kind: library.source_kind.clone(),
            source_file: library.source_file.clone(),
            symbol_name: library.symbol_name.clone(),
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    candidates.dedup_by(|left, right| left.library_key == right.library_key);
    candidates
}

fn part_supplier_part_number(part: &InventoryPart) -> Option<&str> {
    part.library_lcsc
        .as_deref()
        .or(part.supplier_part_number.as_deref())
}

fn component_name_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

fn package_key(value: &str) -> String {
    let value = value
        .rsplit_once(':')
        .map(|(_, value)| value)
        .unwrap_or(value);
    let mut key = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_whitespace() || matches!(character, '_' | '/' | '\\' | '.' | '-') {
            if !key.is_empty() {
                separator = true;
            }
            continue;
        }
        if separator {
            key.push('-');
            separator = false;
        }
        key.push(character.to_ascii_uppercase());
    }
    key
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(|character| character.to_uppercase())
        .collect()
}

fn allocate(part: &InventoryPart, required: u64) -> Vec<InventoryAllocation> {
    let mut locations = part.locations.iter().collect::<Vec<_>>();
    locations.sort_by_key(|location| (location.priority, location.location.clone()));
    let mut remaining = required as i64;
    let mut allocations = Vec::new();
    for (index, location) in locations.iter().enumerate() {
        if remaining <= 0 {
            break;
        }
        let available = location.quantity.max(0);
        let quantity = if index + 1 == locations.len() {
            remaining
        } else {
            remaining.min(available)
        };
        if quantity > 0 {
            allocations.push(InventoryAllocation {
                part_id: part.id.clone(),
                location: location.location.clone(),
                quantity,
            });
            remaining -= quantity;
        }
    }
    allocations
}

fn parse_bom_csv(path: &Path) -> Result<Vec<BomRow>, String> {
    let bytes = fs::read(path).map_err(|err| format!("Read CSV failed: {err}"))?;
    let mut content = String::from_utf8(bytes).map_err(|_| "CSV must be UTF-8".to_string())?;
    if content.starts_with('\u{feff}') {
        content.remove(0);
    }
    let delimiter = content
        .lines()
        .next()
        .map(|line| if line.contains(';') { b';' } else { b',' })
        .unwrap_or(b';');
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());
    let headers = reader
        .headers()
        .map_err(|err| format!("CSV header failed: {err}"))?
        .clone();
    let required_index = |names: &[&str]| {
        headers
            .iter()
            .position(|value| names.iter().any(|name| value.eq_ignore_ascii_case(name)))
            .ok_or_else(|| format!("CSV is missing {} column", names[0]))
    };
    let quantity_index = required_index(&["数量", "Quantity"])?;
    let name_index = required_index(&["名称", "Value", "Name"])?;
    let package_index = required_index(&["封装", "Footprint", "Package"])?;
    let identifier_index = headers.iter().position(|value| {
        ["编号", "ID", "Identifier"]
            .iter()
            .any(|name| value.eq_ignore_ascii_case(name))
    });
    let reference_index = headers.iter().position(|value| {
        ["位号", "Reference", "References"]
            .iter()
            .any(|name| value.eq_ignore_ascii_case(name))
    });
    let supplier_index = headers.iter().position(|value| {
        ["供应商编号", "LCSC Part", "LCSC", "Supplier Part Number"]
            .iter()
            .any(|name| value.eq_ignore_ascii_case(name))
    });
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|err| format!("CSV row {} failed: {err}", index + 2))?;
        if record.iter().all(|value| value.trim().is_empty()) {
            continue;
        }
        let value = |position: usize| record.get(position).unwrap_or("").trim();
        let quantity = value(quantity_index)
            .parse::<u64>()
            .map_err(|_| format!("Invalid 数量 at CSV row {}", index + 2))?;
        if quantity == 0 {
            return Err(format!(
                "数量 must be greater than zero at CSV row {}",
                index + 2
            ));
        }
        let name = value(name_index).to_string();
        let package = value(package_index).to_string();
        if name.is_empty() || package.is_empty() {
            return Err(format!(
                "名称 and 封装 are required at CSV row {}",
                index + 2
            ));
        }
        let explicit_supplier = supplier_index
            .and_then(|position| normalize_optional(Some(value(position).to_string())))
            .map(|supplier| normalize_identifier(&supplier));
        let package_supplier = extract_lcsc_id(&package);
        let name_supplier = extract_lcsc_id(&name);
        let supplier_part_number = explicit_supplier
            .clone()
            .or_else(|| package_supplier.clone())
            .or_else(|| name_supplier.clone());
        let mut supplier_ids = HashSet::new();
        for supplier in [
            explicit_supplier.as_ref(),
            package_supplier.as_ref(),
            name_supplier.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            supplier_ids.insert(supplier.clone());
        }
        let supplier_part_number_conflict = supplier_ids.len() > 1;
        let supplier_part_number_source = if explicit_supplier.is_some() {
            Some("供应商编号".to_string())
        } else if package_supplier.is_some() {
            Some("封装".to_string())
        } else if name_supplier.is_some() {
            Some("名称".to_string())
        } else {
            None
        };
        rows.push(BomRow {
            row_number: index + 2,
            identifier: identifier_index
                .map(|position| value(position).to_string())
                .unwrap_or_default(),
            references: reference_index
                .map(|position| value(position).to_string())
                .unwrap_or_default(),
            supplier_part_number,
            supplier_part_number_source,
            supplier_part_number_conflict,
            name,
            package,
            quantity,
        });
    }
    if rows.is_empty() {
        return Err("CSV has no data rows".to_string());
    }
    Ok(rows)
}

fn extract_lcsc_id(value: &str) -> Option<String> {
    let token = value
        .rsplit(|character: char| character == '_' || character == '-' || character.is_whitespace())
        .next()?;
    if token.len() < 4 || !matches!(token.as_bytes().first(), Some(b'C' | b'c')) {
        return None;
    }
    if !token[1..]
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some(token.to_uppercase())
}

fn normalize_locations(
    mut locations: Vec<InventoryLocation>,
) -> Result<Vec<InventoryLocation>, String> {
    let mut seen = HashSet::new();
    locations.retain(|location| !location.location.trim().is_empty());
    for location in &mut locations {
        location.location = location.location.trim().to_string();
        if !seen.insert(location.location.clone()) {
            return Err(format!("Duplicate location: {}", location.location));
        }
    }
    locations.sort_by_key(|location| location.priority);
    for (index, location) in locations.iter_mut().enumerate() {
        location.priority = index as u32;
    }
    Ok(locations)
}

fn insert_stock(
    tx: &Transaction<'_>,
    part_id: &str,
    code: &str,
    quantity: i64,
    priority: u32,
) -> Result<(), String> {
    let location_id = ensure_location(tx, code)?;
    tx.execute(
        "INSERT INTO part_stock(part_id, location_id, quantity, priority) VALUES (?1, ?2, ?3, ?4)",
        params![part_id, location_id, quantity, priority as i64],
    )
    .map_err(db_error)?;
    Ok(())
}

fn ensure_location(tx: &Transaction<'_>, code: &str) -> Result<i64, String> {
    let code = required_text(code, "Location")?;
    tx.execute("INSERT OR IGNORE INTO locations(code) VALUES (?1)", [&code])
        .map_err(db_error)?;
    tx.query_row("SELECT id FROM locations WHERE code = ?1", [&code], |row| {
        row.get(0)
    })
    .map_err(db_error)
}

fn revision_from(tx: &Transaction<'_>) -> Result<i64, String> {
    tx.query_row(
        "SELECT value FROM inventory_meta WHERE key = 'revision'",
        [],
        |row| row.get(0),
    )
    .map_err(db_error)
}

fn bump_revision(tx: &Transaction<'_>) -> Result<(), String> {
    tx.execute(
        "UPDATE inventory_meta SET value = value + 1 WHERE key = 'revision'",
        [],
    )
    .map_err(db_error)?;
    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_uppercase();
        (!value.is_empty()).then_some(value)
    })
}

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn required_text(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(format!("{label} is required"))
    } else {
        Ok(value)
    }
}

fn part_label(part: &InventoryPart) -> String {
    format!(
        "{} · {}{}",
        part.name,
        part.package,
        part.supplier_part_number
            .as_deref()
            .map(|value| format!(" · {value}"))
            .unwrap_or_default()
    )
}

fn new_id() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    format!("part-{stamp}")
}

fn db_error(error: rusqlite::Error) -> String {
    format!("Inventory database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::{
        BomDeductionRow, BomImportRow, ConfirmBomDeductionRequest, ImportBomRequest,
        InventoryLibraryPart, InventoryLocation, InventoryPart, InventoryPartInput,
        InventoryRepository, InventoryStockAdjustment, allocate, find_candidates, parse_bom_csv,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("happyjlc_inventory_{name}_{stamp}.csv"))
    }

    #[test]
    fn parses_semicolon_csv_with_bom_and_trailing_empty_column() {
        let path = temp_file("csv");
        fs::write(&path, "\u{feff}\"编号\";\"位号\";\"封装\";\"数量\";\"名称\";\"供应商编号\";\n1;\"C1, C2\";\"C_0603\";2;\"10uf\";;\n").unwrap();
        let rows = parse_bom_csv(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].identifier, "1");
        assert_eq!(rows[0].references, "C1, C2");
        assert_eq!(rows[0].quantity, 2);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn extracts_lcsc_from_package_and_reports_conflicts() {
        let path = temp_file("metadata");
        fs::write(
            &path,
            "编号;位号;封装;数量;名称;供应商编号\n1;U1;ESP32-C3-MINI-1-N4_C2838502;1;ESP32-C3-MINI-1-N4;;\n2;U2;PART_C123;1;PART;;\n3;U3;PART_C123;1;PART;C456\n",
        )
        .unwrap();
        let rows = parse_bom_csv(&path).unwrap();
        assert_eq!(rows[0].supplier_part_number.as_deref(), Some("C2838502"));
        assert_eq!(rows[0].supplier_part_number_source.as_deref(), Some("封装"));
        assert!(!rows[0].supplier_part_number_conflict);
        assert!(rows[2].supplier_part_number_conflict);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalizes_lcsc_spacing_and_package_separators_without_merging_pins() {
        let path = temp_file("normalization");
        fs::write(
            &path,
            "数量;名称;封装;供应商编号\n1;R;Package_SO:SOT_23_3; c 123 \n1;R;SOT-23-5;C124\n",
        )
        .unwrap();
        let rows = parse_bom_csv(&path).unwrap();
        assert_eq!(rows[0].supplier_part_number.as_deref(), Some("C123"));
        assert_eq!(rows[1].supplier_part_number.as_deref(), Some("C124"));
        assert_ne!(
            super::package_key("Package_SO:SOT_23_3"),
            super::package_key("SOT-23-5")
        );
        assert_eq!(
            super::package_key("Package_SO:SOT_23_3"),
            super::package_key("SOT-23-3")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_missing_required_column_and_invalid_quantity() {
        let missing = temp_file("missing");
        fs::write(&missing, "名称;封装\nR;0603\n").unwrap();
        assert!(parse_bom_csv(&missing).unwrap_err().contains("数量"));
        let invalid = temp_file("invalid");
        fs::write(&invalid, "数量;名称;封装\nnope;R;0603\n").unwrap();
        assert!(parse_bom_csv(&invalid).unwrap_err().contains("Invalid"));
        let _ = fs::remove_file(missing);
        let _ = fs::remove_file(invalid);
    }

    #[test]
    fn prefers_supplier_match_and_reports_name_package_conflicts() {
        let parts = vec![
            InventoryPart {
                id: "a".to_string(),
                library_lcsc: None,
                library_symbol_name: None,
                library_source_file: None,
                library_missing: false,
                supplier_part_number: Some("C123".to_string()),
                name: "10uF".to_string(),
                package: "0603".to_string(),
                note: String::new(),
                locations: vec![],
            },
            InventoryPart {
                id: "b".to_string(),
                library_lcsc: None,
                library_symbol_name: None,
                library_source_file: None,
                library_missing: false,
                supplier_part_number: Some("C456".to_string()),
                name: "10uF".to_string(),
                package: "0603".to_string(),
                note: String::new(),
                locations: vec![],
            },
        ];
        let supplier_row = super::BomRow {
            row_number: 2,
            identifier: String::new(),
            references: String::new(),
            supplier_part_number: Some("C456".to_string()),
            supplier_part_number_source: Some("供应商编号".to_string()),
            supplier_part_number_conflict: false,
            name: "different".to_string(),
            package: "different".to_string(),
            quantity: 1,
        };
        assert_eq!(find_candidates(&supplier_row, &parts)[0].id, "b");
        let fallback_row = super::BomRow {
            supplier_part_number: None,
            name: "10uF".to_string(),
            package: "0603".to_string(),
            ..supplier_row
        };
        assert_eq!(find_candidates(&fallback_row, &parts).len(), 2);
    }

    #[test]
    fn allocates_shortage_to_last_location() {
        let part = InventoryPart {
            id: "p".to_string(),
            library_lcsc: None,
            library_symbol_name: None,
            library_source_file: None,
            library_missing: false,
            supplier_part_number: None,
            name: "R".to_string(),
            package: "0603".to_string(),
            note: String::new(),
            locations: vec![
                InventoryLocation {
                    location: "A".to_string(),
                    quantity: 2,
                    priority: 0,
                },
                InventoryLocation {
                    location: "B".to_string(),
                    quantity: 1,
                    priority: 1,
                },
            ],
        };
        let allocations = allocate(&part, 5);
        assert_eq!(
            allocations
                .iter()
                .map(|item| item.quantity)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn rejects_stale_preview_after_stock_change() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        repository
            .save_part(InventoryPartInput {
                id: Some("p".to_string()),
                library_lcsc: None,
                library_symbol_name: None,
                library_source_file: None,
                supplier_part_number: Some("C123".to_string()),
                name: "10uF".to_string(),
                package: "0603".to_string(),
                note: String::new(),
                locations: vec![
                    InventoryLocation {
                        location: "A".to_string(),
                        quantity: 2,
                        priority: 0,
                    },
                    InventoryLocation {
                        location: "B".to_string(),
                        quantity: 1,
                        priority: 1,
                    },
                ],
            })
            .unwrap();
        assert_eq!(repository.get_parts("A").unwrap().parts.len(), 1);
        let path = temp_file("confirm");
        fs::write(&path, "数量;名称;封装;供应商编号\n3;10uF;0603;C123\n").unwrap();
        let preview = repository
            .preview_csv(path.to_str().unwrap(), 1, &[])
            .unwrap();
        repository
            .adjust_stock(InventoryStockAdjustment {
                part_id: "p".to_string(),
                location: "A".to_string(),
                delta: 1,
            })
            .unwrap();
        let request = ConfirmBomDeductionRequest {
            path: path.to_str().unwrap().to_string(),
            boards: 1,
            revision: preview.revision,
            rows: vec![BomDeductionRow {
                row_number: 2,
                part_id: Some("p".to_string()),
                skipped: false,
                allocations: vec![
                    super::InventoryAllocation {
                        part_id: "p".to_string(),
                        location: "A".to_string(),
                        quantity: 1,
                    },
                    super::InventoryAllocation {
                        part_id: "p".to_string(),
                        location: "B".to_string(),
                        quantity: 2,
                    },
                ],
            }],
        };
        assert!(
            repository
                .confirm_csv(request)
                .unwrap_err()
                .contains("changed since preview")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn links_library_part_and_marks_it_missing_after_rescan() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        let library_part = InventoryLibraryPart {
            library_key: "symbol:happyjlc.kicad_sym:R_0603_C123".to_string(),
            lcsc_part: "C123".to_string(),
            value: "10k".to_string(),
            symbol_name: "R_0603_C123".to_string(),
            package: "R_0603".to_string(),
            source_file: "happyjlc.kicad_sym".to_string(),
            has_model: true,
            source_kind: "export".to_string(),
            editable: true,
        };
        assert_eq!(
            repository
                .import_library_parts(std::slice::from_ref(&library_part))
                .unwrap(),
            1
        );
        let linked = repository.get_parts("").unwrap().parts.remove(0);
        assert_eq!(linked.library_lcsc.as_deref(), Some("C123"));
        assert_eq!(linked.name, "10k");
        assert!(!linked.library_missing);

        repository.sync_library(&[]).unwrap();
        let missing = repository.get_parts("").unwrap().parts.remove(0);
        assert!(missing.library_missing);
        assert_eq!(missing.locations.len(), 1);
    }

    #[test]
    fn imports_bom_rows_as_zero_stock_library_and_manual_records() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        let library_parts = vec![InventoryLibraryPart {
            library_key: "symbol:happyjlc.kicad_sym:U_C123".to_string(),
            lcsc_part: "C123".to_string(),
            value: "Library part".to_string(),
            symbol_name: "U_C123".to_string(),
            package: "QFN-4".to_string(),
            source_file: "happyjlc.kicad_sym".to_string(),
            has_model: true,
            source_kind: "export".to_string(),
            editable: true,
        }];
        let path = temp_file("import");
        fs::write(
            &path,
            "数量;名称;封装\n1;Unknown_C999;CUSTOM_C999\n2;R;R_0603\n1;Library part;QFN-4_C123\n",
        )
        .unwrap();
        let preview = repository
            .preview_csv(path.to_str().unwrap(), 1, &library_parts)
            .unwrap();
        assert_eq!(preview.rows.len(), 3);
        assert_eq!(
            preview.rows[0].supplier_part_number.as_deref(),
            Some("C999")
        );
        assert_eq!(preview.rows[0].library_candidates.len(), 0);
        assert_eq!(preview.rows[0].library_status, "missing");
        assert_eq!(preview.rows[2].library_candidates.len(), 1);
        assert_eq!(preview.rows[2].library_status, "available");
        assert_eq!(preview.rows[2].model_status, "available");

        let result = repository
            .import_bom(
                ImportBomRequest {
                    path: path.to_str().unwrap().to_string(),
                    revision: preview.revision,
                    rows: preview
                        .rows
                        .iter()
                        .map(|row| BomImportRow {
                            row_number: row.row_number,
                            skipped: false,
                            library_lcsc: row
                                .library_candidates
                                .first()
                                .map(|candidate| candidate.lcsc_part.clone())
                                .or_else(|| row.supplier_part_number.clone()),
                            library_key: row
                                .library_candidates
                                .first()
                                .map(|candidate| candidate.library_key.clone()),
                        })
                        .collect(),
                },
                &library_parts,
            )
            .unwrap();
        assert_eq!(result.imported, 3);
        assert_eq!(result.manual, 1);
        assert_eq!(result.pending_library, 1);
        let parts = repository.get_parts("").unwrap().parts;
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.locations[0].quantity == 0));
        assert!(
            parts
                .iter()
                .any(|part| part.library_lcsc.as_deref() == Some("C123"))
        );
        assert!(
            parts
                .iter()
                .any(|part| part.library_lcsc.as_deref() == Some("C999") && part.library_missing)
        );
        assert!(
            parts
                .iter()
                .any(|part| part.library_lcsc.is_none() && part.name == "R")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn imports_no_lcsc_library_candidate_without_supplier_or_stock() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        let library_parts = vec![InventoryLibraryPart {
            library_key: "symbol:standard.kicad_sym\u{001f}AO3400A".to_string(),
            lcsc_part: String::new(),
            value: "AO3400A".to_string(),
            symbol_name: "AO3400A".to_string(),
            package: "SOT-23".to_string(),
            source_file: "standard.kicad_sym".to_string(),
            has_model: false,
            source_kind: "kicad_standard".to_string(),
            editable: false,
        }];
        let path = temp_file("standard-library");
        fs::write(&path, "数量;名称;封装\n1;AO3400A;SOT-23\n").unwrap();
        let preview = repository
            .preview_csv(path.to_str().unwrap(), 1, &library_parts)
            .unwrap();
        assert_eq!(preview.rows[0].library_candidates.len(), 1);
        assert_eq!(preview.rows[0].library_candidates[0].lcsc_part, "");

        let result = repository
            .import_bom(
                ImportBomRequest {
                    path: path.to_str().unwrap().to_string(),
                    revision: preview.revision,
                    rows: vec![BomImportRow {
                        row_number: 2,
                        skipped: false,
                        library_lcsc: None,
                        library_key: Some(library_parts[0].library_key.clone()),
                    }],
                },
                &library_parts,
            )
            .unwrap();
        assert_eq!(result.imported, 1);
        let part = repository.get_parts("").unwrap().parts.remove(0);
        assert_eq!(part.supplier_part_number, None);
        assert_eq!(part.library_lcsc, None);
        assert_eq!(part.library_symbol_name.as_deref(), Some("AO3400A"));
        assert_eq!(
            part.library_source_file.as_deref(),
            Some("standard.kicad_sym")
        );
        assert_eq!(part.locations[0].quantity, 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn deduplicates_normalized_manual_bom_rows() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        let path = temp_file("duplicate-manual");
        fs::write(
            &path,
            "数量;名称;封装\n1;10k;Package_SO:R_0603\n2; 10k ;R-0603\n",
        )
        .unwrap();
        let preview = repository
            .preview_csv(path.to_str().unwrap(), 1, &[])
            .unwrap();
        let result = repository
            .import_bom(
                ImportBomRequest {
                    path: path.to_str().unwrap().to_string(),
                    revision: preview.revision,
                    rows: preview
                        .rows
                        .iter()
                        .map(|row| BomImportRow {
                            row_number: row.row_number,
                            skipped: false,
                            library_lcsc: None,
                            library_key: None,
                        })
                        .collect(),
                },
                &[],
            )
            .unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.existing, 1);
        assert_eq!(repository.get_parts("").unwrap().parts.len(), 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn accepts_library_parts_without_package_metadata() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        let library_part = InventoryLibraryPart {
            library_key: "symbol:legacy.kicad_sym:R_C999".to_string(),
            lcsc_part: "C999".to_string(),
            value: "Legacy resistor".to_string(),
            symbol_name: "R_C999".to_string(),
            package: String::new(),
            source_file: "legacy.kicad_sym".to_string(),
            has_model: false,
            source_kind: "export".to_string(),
            editable: true,
        };
        repository
            .import_library_parts(std::slice::from_ref(&library_part))
            .unwrap();
        let part = repository.get_parts("").unwrap().parts.remove(0);
        assert_eq!(part.package, "");

        repository
            .save_part(InventoryPartInput {
                id: Some(part.id),
                library_lcsc: Some("C999".to_string()),
                library_symbol_name: Some("R_C999".to_string()),
                library_source_file: Some("legacy.kicad_sym".to_string()),
                supplier_part_number: Some("C999".to_string()),
                name: "Legacy resistor".to_string(),
                package: String::new(),
                note: String::new(),
                locations: vec![InventoryLocation {
                    location: "未分配".to_string(),
                    quantity: 0,
                    priority: 0,
                }],
            })
            .unwrap();
    }

    #[test]
    fn backfills_legacy_supplier_number_as_missing_library_link() {
        let mut repository = InventoryRepository::in_memory().unwrap();
        repository
            .save_part(InventoryPartInput {
                id: Some("legacy".to_string()),
                library_lcsc: None,
                library_symbol_name: None,
                library_source_file: None,
                supplier_part_number: Some(" c321 ".to_string()),
                name: "Legacy part".to_string(),
                package: "0603".to_string(),
                note: String::new(),
                locations: vec![],
            })
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE parts SET library_lcsc = NULL, library_missing = 0 WHERE id = 'legacy'",
                [],
            )
            .unwrap();

        repository.backfill_legacy_library_links().unwrap();
        let part = repository.get_parts("").unwrap().parts.remove(0);
        assert_eq!(part.library_lcsc.as_deref(), Some("C321"));
        assert!(part.library_missing);
        assert_eq!(part.locations.len(), 1);
    }
}
