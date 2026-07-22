use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportedModel {
    pub file_name: String,
    pub format: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportedSymbol {
    pub library_key: String,
    pub lcsc_part: String,
    pub value: String,
    pub symbol_name: String,
    pub package: String,
    pub source_file: String,
    pub source_kind: String,
    pub editable: bool,
    pub model_available: bool,
    pub models: Vec<ImportedModel>,
}

impl Ord for ImportedSymbol {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            &self.lcsc_part,
            &self.source_file,
            &self.symbol_name,
            &self.library_key,
        )
            .cmp(&(
                &other.lcsc_part,
                &other.source_file,
                &other.symbol_name,
                &other.library_key,
            ))
    }
}

impl PartialOrd for ImportedSymbol {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LibrarySource {
    pub path: String,
    pub kind: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportedSymbolsResponse {
    pub scanned_path: String,
    pub sources: Vec<LibrarySource>,
    pub items: Vec<ImportedSymbol>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImportedSymbolUpdateRequest {
    pub source_file: String,
    pub symbol_name: String,
    pub new_symbol_name: String,
    pub lcsc_part: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImportedSymbolDeleteRequest {
    pub source_file: String,
    pub symbol_name: String,
    #[serde(default)]
    pub lcsc_part: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImportedModelRequest {
    pub source_file: String,
    pub lcsc_part: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryFile {
    path: PathBuf,
    source_file: String,
    source_kind: String,
    editable: bool,
}

#[derive(Debug, Clone)]
struct SymbolBlock<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    symbol_name: String,
    lcsc_part: Option<String>,
    value: Option<String>,
    package: Option<String>,
    model_available: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DeletedGeneratedAssets {
    footprints: usize,
    models: usize,
    checkpoint_entries: usize,
}

pub fn load_imported_symbols(output_path: &Path) -> Result<ImportedSymbolsResponse, String> {
    let mut response = load_imported_symbols_internal(output_path, &[])?;
    for item in &mut response.items {
        if let Some(name) = Path::new(&item.source_file)
            .file_name()
            .and_then(|value| value.to_str())
        {
            item.source_file = name.to_string();
            let prefix = if item.library_key.starts_with("footprint:") {
                "footprint"
            } else {
                "symbol"
            };
            item.library_key = format!("{prefix}:{}:{}", item.source_file, item.symbol_name);
        }
    }
    Ok(response)
}

pub fn load_imported_symbols_with_paths(
    output_path: &Path,
    configured_paths: &[String],
) -> Result<ImportedSymbolsResponse, String> {
    load_imported_symbols_internal(output_path, configured_paths)
}

fn load_imported_symbols_internal(
    output_path: &Path,
    configured_paths: &[String],
) -> Result<ImportedSymbolsResponse, String> {
    let scanned_path = output_path.display().to_string();
    let (library_files, sources) = discover_library_files(output_path, configured_paths)?;

    if library_files.is_empty() {
        return Ok(ImportedSymbolsResponse {
            scanned_path,
            sources,
            items: Vec::new(),
        });
    }

    let mut items = Vec::new();

    for library_file in library_files {
        let content = fs::read_to_string(&library_file.path)
            .map_err(|err| format!("Failed to read {}: {}", library_file.path.display(), err))?;
        let mut parsed = parse_kicad_library_file(&content, &library_file)
            .map_err(|err| format!("Failed to parse {}: {}", library_file.path.display(), err))?;
        for item in &mut parsed {
            if library_file.editable && !item.lcsc_part.is_empty() {
                item.models =
                    imported_models_for_library(output_path, &library_file.path, &item.lcsc_part)?;
            }
        }
        items.extend(parsed);
    }

    let items = deduplicate_library_items(items);

    Ok(ImportedSymbolsResponse {
        scanned_path,
        sources,
        items,
    })
}

pub fn read_imported_model(
    output_path: &Path,
    request: ImportedModelRequest,
) -> Result<Vec<u8>, String> {
    let library_path = ensure_library_is_within_output(output_path, &request.source_file)?;
    let lcsc_part = request.lcsc_part.trim();
    if lcsc_part.is_empty() {
        return Err("LCSC Part cannot be empty".to_string());
    }

    let model = imported_models_for_library(output_path, &library_path, lcsc_part)?
        .into_iter()
        .find(|model| model.file_name == request.file_name)
        .ok_or_else(|| "Requested 3D model was not found for this imported part".to_string())?;
    let library_name = library_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Source library filename is not valid UTF-8".to_string())?;
    let shapes_dir = output_path.join(format!("{library_name}.3dshapes"));
    let model_path = shapes_dir.join(model.file_name);
    let canonical_shapes_dir = fs::canonicalize(&shapes_dir).map_err(|err| {
        format!(
            "Failed to access 3D model directory {}: {}",
            shapes_dir.display(),
            err
        )
    })?;
    let canonical_model_path = fs::canonicalize(&model_path).map_err(|err| {
        format!(
            "Failed to access 3D model {}: {}",
            model_path.display(),
            err
        )
    })?;
    if !canonical_model_path.starts_with(&canonical_shapes_dir) {
        return Err("3D model path must stay inside the library model directory".to_string());
    }

    fs::read(&canonical_model_path).map_err(|err| {
        format!(
            "Failed to read 3D model {}: {}",
            canonical_model_path.display(),
            err
        )
    })
}

pub fn update_imported_symbol(
    output_path: &Path,
    request: ImportedSymbolUpdateRequest,
) -> Result<String, String> {
    let new_symbol_name = request.new_symbol_name.trim();
    let new_lcsc_part = request.lcsc_part.trim();

    if new_symbol_name.is_empty() {
        return Err("Symbol name cannot be empty".to_string());
    }
    if new_lcsc_part.is_empty() {
        return Err("LCSC Part cannot be empty".to_string());
    }

    let library_path = ensure_library_is_within_output(output_path, &request.source_file)?;
    ensure_editable_generated_library(&library_path, output_path)?;
    let content = fs::read_to_string(&library_path)
        .map_err(|err| format!("Failed to read {}: {}", library_path.display(), err))?;
    let blocks = top_level_symbol_blocks(&content)?;
    let current = blocks
        .iter()
        .find(|block| block.symbol_name == request.symbol_name)
        .ok_or_else(|| {
            format!(
                "Symbol {} was not found in {}",
                request.symbol_name, request.source_file
            )
        })?;

    ensure_symbol_name_available(&blocks, &request.symbol_name, new_symbol_name)?;
    let updated_block = update_symbol_block(
        current.text,
        &request.symbol_name,
        new_symbol_name,
        new_lcsc_part,
    )?;
    let updated_content =
        apply_replacements(&content, vec![(current.start..current.end, updated_block)])?;
    fs::write(&library_path, updated_content)
        .map_err(|err| format!("Failed to write {}: {}", library_path.display(), err))?;

    Ok(format!(
        "Updated {} in {}",
        request.symbol_name, request.source_file
    ))
}

pub fn delete_imported_symbol(
    output_path: &Path,
    request: ImportedSymbolDeleteRequest,
) -> Result<String, String> {
    let library_path = ensure_library_is_within_output(output_path, &request.source_file)?;
    ensure_editable_generated_library(&library_path, output_path)?;
    let content = fs::read_to_string(&library_path)
        .map_err(|err| format!("Failed to read {}: {}", library_path.display(), err))?;
    let blocks = top_level_symbol_blocks(&content)?;
    let current = blocks
        .iter()
        .find(|block| block.symbol_name == request.symbol_name)
        .ok_or_else(|| {
            format!(
                "Symbol {} was not found in {}",
                request.symbol_name, request.source_file
            )
        })?;

    let lcsc_part = current.lcsc_part.clone().or_else(|| {
        request
            .lcsc_part
            .as_deref()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
    });
    let footprint_name = footprint_name_from_symbol_block(current.text)
        .unwrap_or_else(|| current.symbol_name.clone());

    let updated_content = delete_symbol_block(&content, current);
    fs::write(&library_path, updated_content)
        .map_err(|err| format!("Failed to write {}: {}", library_path.display(), err))?;

    let deleted_assets = delete_generated_assets(
        output_path,
        &library_path,
        &footprint_name,
        lcsc_part.as_deref(),
    )?;

    Ok(format_delete_result(
        &request.symbol_name,
        &request.source_file,
        &deleted_assets,
    ))
}

pub fn unique_lcsc_parts(items: &[ImportedSymbol]) -> Vec<String> {
    let mut seen = HashSet::new();

    items
        .iter()
        .filter_map(|item| {
            let lcsc_part = (!item.lcsc_part.is_empty()).then_some(&item.lcsc_part)?;
            if seen.insert(lcsc_part.clone()) {
                Some(lcsc_part.clone())
            } else {
                None
            }
        })
        .collect()
}

fn deduplicate_library_items(items: Vec<ImportedSymbol>) -> Vec<ImportedSymbol> {
    let mut by_identity = BTreeMap::<String, ImportedSymbol>::new();

    for item in items {
        let identity = if !item.lcsc_part.trim().is_empty() {
            format!("lcsc:{}", normalize_lcsc_part(&item.lcsc_part))
        } else {
            format!(
                "source:{}\u{001f}symbol:{}\u{001f}package:{}",
                item.source_file, item.symbol_name, item.package
            )
        };

        match by_identity.get_mut(&identity) {
            None => {
                by_identity.insert(identity, item);
            }
            Some(current) if item.source_kind == "export" && current.source_kind != "export" => {
                *current = item;
            }
            Some(current) if current.source_kind == item.source_kind && item < *current => {
                *current = item;
            }
            Some(_) => {}
        }
    }

    let mut items = by_identity.into_values().collect::<Vec<_>>();
    items.sort();
    items
}

fn discover_library_files(
    output_path: &Path,
    configured_paths: &[String],
) -> Result<(Vec<LibraryFile>, Vec<LibrarySource>), String> {
    let mut files = Vec::new();
    let mut sources = Vec::new();
    let mut seen = HashSet::new();

    if output_path.exists() {
        for path in collect_kicad_files(output_path, false)? {
            add_library_file(&mut files, &mut seen, output_path, path, "export", true)?;
        }
        sources.push(LibrarySource {
            path: output_path.display().to_string(),
            kind: "export".to_string(),
            configured: false,
        });
    }

    for configured in configured_paths {
        let path = expand_path(configured);
        if !path.exists() {
            continue;
        }
        let files_for_source = collect_kicad_files(&path, true)?;
        if files_for_source.is_empty() {
            continue;
        }
        let kind = if is_standard_kicad_path(&path) {
            "kicad_standard"
        } else {
            "external"
        };
        for file in files_for_source {
            add_library_file(&mut files, &mut seen, output_path, file, kind, false)?;
        }
        sources.push(LibrarySource {
            path: path.display().to_string(),
            kind: kind.to_string(),
            configured: true,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    sources.dedup_by(|left, right| left.path == right.path);
    Ok((files, sources))
}

fn add_library_file(
    files: &mut Vec<LibraryFile>,
    seen: &mut HashSet<PathBuf>,
    output_path: &Path,
    path: PathBuf,
    source_kind: &str,
    editable: bool,
) -> Result<(), String> {
    let canonical = fs::canonicalize(&path)
        .map_err(|err| format!("Failed to access {}: {}", path.display(), err))?;
    if !canonical.is_file()
        || !is_supported_library_file(&canonical)
        || !seen.insert(canonical.clone())
    {
        return Ok(());
    }
    let source_file =
        if editable {
            canonical
                .strip_prefix(fs::canonicalize(output_path).map_err(|err| {
                    format!("Failed to access {}: {}", output_path.display(), err)
                })?)
                .map_err(|_| {
                    format!(
                        "Generated library file must stay inside {}",
                        output_path.display()
                    )
                })?
                .display()
                .to_string()
        } else {
            canonical.display().to_string()
        };
    files.push(LibraryFile {
        source_file,
        path: canonical,
        source_kind: source_kind.to_string(),
        editable,
    });
    Ok(())
}

fn collect_kicad_files(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(is_supported_library_file(path)
            .then(|| path.to_path_buf())
            .into_iter()
            .collect());
    }
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|err| format!("Failed to read {}: {err}", directory.display()))?
        {
            let entry = entry.map_err(|err| format!("Failed to read library entry: {err}"))?;
            let entry_path = entry.path();
            if entry_path.is_dir() && recursive {
                pending.push(entry_path);
            } else if entry_path.is_file() && is_supported_library_file(&entry_path) {
                files.push(entry_path);
            }
        }
    }
    Ok(files)
}

fn expand_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if let Some(home) = std::env::var_os("HOME")
        && let Some(rest) = trimmed.strip_prefix("~/")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(trimmed)
}

fn is_standard_kicad_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.contains("KiCad.app/Contents/SharedSupport") || text.contains("/share/kicad/")
}

fn is_supported_library_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("kicad_sym") | Some("kicad_mod")
    )
}

#[cfg(test)]
fn parse_kicad_symbol_lib(content: &str, source_file: &str) -> Result<Vec<ImportedSymbol>, String> {
    let library_file = LibraryFile {
        path: PathBuf::from(source_file),
        source_file: source_file.to_string(),
        source_kind: "export".to_string(),
        editable: true,
    };
    parse_kicad_library_file(content, &library_file)
}

fn parse_kicad_library_file(
    content: &str,
    library_file: &LibraryFile,
) -> Result<Vec<ImportedSymbol>, String> {
    if library_file
        .path
        .extension()
        .and_then(|value| value.to_str())
        == Some("kicad_mod")
    {
        return parse_kicad_footprint_lib(content, library_file);
    }
    let mut items = Vec::new();

    for block in top_level_symbol_blocks(content)? {
        items.push(ImportedSymbol {
            library_key: format!("symbol:{}:{}", library_file.source_file, block.symbol_name),
            lcsc_part: block
                .lcsc_part
                .map(|part| normalize_lcsc_part(&part))
                .or_else(|| extract_lcsc_suffix(&block.symbol_name))
                .or_else(|| block.value.as_deref().and_then(extract_lcsc_suffix))
                .unwrap_or_default(),
            value: block.value.unwrap_or_else(|| block.symbol_name.clone()),
            symbol_name: block.symbol_name,
            package: block.package.unwrap_or_default(),
            source_file: library_file.source_file.clone(),
            source_kind: library_file.source_kind.clone(),
            editable: library_file.editable,
            model_available: block.model_available,
            models: Vec::new(),
        });
    }

    Ok(items)
}

fn parse_kicad_footprint_lib(
    content: &str,
    library_file: &LibraryFile,
) -> Result<Vec<ImportedSymbol>, String> {
    let name = head_string_after_keyword(content, "footprint")
        .ok_or_else(|| "footprint file is missing a name".to_string())?;
    let lcsc_part = property_value(content, "LCSC Part")
        .map(|part| normalize_lcsc_part(&part))
        .or_else(|| extract_lcsc_suffix(&name));
    let package = property_value(content, "Package").unwrap_or_else(|| name.clone());
    let value = footprint_value(content).unwrap_or_else(|| name.clone());
    Ok(vec![ImportedSymbol {
        library_key: format!("footprint:{}:{}", library_file.source_file, name),
        lcsc_part: lcsc_part.unwrap_or_default(),
        value,
        symbol_name: name,
        package,
        source_file: library_file.source_file.clone(),
        source_kind: library_file.source_kind.clone(),
        editable: library_file.editable,
        model_available: content.contains("(model "),
        models: Vec::new(),
    }])
}

fn extract_lcsc_suffix(value: &str) -> Option<String> {
    value
        .split('_')
        .rev()
        .find(|part| {
            let normalized = part.trim();
            normalized.len() > 1
                && (normalized.starts_with('C') || normalized.starts_with('c'))
                && normalized[1..]
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
        .map(|part| part.trim().to_ascii_uppercase())
}

fn normalize_lcsc_part(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn ensure_editable_generated_library(path: &Path, output_path: &Path) -> Result<(), String> {
    let output = fs::canonicalize(output_path)
        .map_err(|err| format!("Failed to access generated library directory: {err}"))?;
    let canonical = fs::canonicalize(path)
        .map_err(|err| format!("Failed to access {}: {err}", path.display()))?;
    if !canonical.starts_with(&output) {
        return Err("KiCad 标准库和外部库为只读，不能编辑或删除".to_string());
    }
    Ok(())
}

fn footprint_value(content: &str) -> Option<String> {
    let start = content.find("(fp_text value")?;
    let rest = &content[start..];
    let quote_start = rest.find('"')?;
    let quote_end = quoted_string_end(rest, quote_start)?;
    Some(unescape_kicad_string(&rest[quote_start + 1..quote_end]))
}

fn top_level_symbol_blocks(content: &str) -> Result<Vec<SymbolBlock<'_>>, String> {
    let bytes = content.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0usize;
    let mut blocks = Vec::new();

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'(' => {
                if depth == 1 && starts_block_keyword(bytes, index, b"symbol") {
                    let end = matching_paren_end(bytes, index)
                        .ok_or_else(|| "unclosed top-level symbol block".to_string())?;
                    let text = &content[index..end];
                    let symbol_name = head_string_after_keyword(text, "symbol")
                        .ok_or_else(|| "symbol block is missing a name".to_string())?;
                    let lcsc_part = property_value(text, "LCSC Part");
                    let value = property_value(text, "Value");
                    let package = property_value(text, "Package").or_else(|| {
                        property_value(text, "Footprint").map(|value| {
                            value
                                .rsplit_once(':')
                                .map(|(_, package)| package.to_string())
                                .unwrap_or(value)
                        })
                    });
                    blocks.push(SymbolBlock {
                        text,
                        start: index,
                        end,
                        symbol_name,
                        lcsc_part,
                        value,
                        package,
                        model_available: text.contains("(model "),
                    });
                    index = end;
                    continue;
                }
                depth += 1;
            }
            b')' => {
                if depth == 0 {
                    return Err("unexpected ')' while scanning symbol library".to_string());
                }
                depth -= 1;
            }
            _ => {}
        }

        index += 1;
    }

    if in_string {
        return Err("unterminated string literal in symbol library".to_string());
    }

    if depth != 0 {
        return Err("unbalanced parentheses in symbol library".to_string());
    }

    Ok(blocks)
}

fn matching_paren_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = start;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }

        index += 1;
    }

    None
}

fn head_string_after_keyword(block: &str, keyword: &str) -> Option<String> {
    let range = string_after_keyword_range(block, 0, keyword)?;
    Some(unescape_kicad_string(&block[range]))
}

fn property_value(block: &str, property_name: &str) -> Option<String> {
    let range = property_value_range(block, property_name)?;
    Some(unescape_kicad_string(&block[range]))
}

fn footprint_name_from_symbol_block(block: &str) -> Option<String> {
    let footprint = property_value(block, "Footprint")?;
    let trimmed = footprint.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .rsplit_once(':')
            .map(|(_, name)| name)
            .unwrap_or(trimmed)
            .to_string(),
    )
}

fn property_value_range(block: &str, property_name: &str) -> Option<Range<usize>> {
    let bytes = block.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if starts_block_keyword(bytes, index, b"property") {
            let end = matching_paren_end(bytes, index)?;
            let property_block = &block[index..end];
            let ranges = quoted_string_ranges(property_block, 2);
            if ranges.len() >= 2
                && unescape_kicad_string(&property_block[ranges[0].clone()]) == property_name
            {
                let value = ranges[1].clone();
                return Some(index + value.start..index + value.end);
            }
            index = end;
            continue;
        }
        index += 1;
    }

    None
}

fn starts_block_keyword(bytes: &[u8], index: usize, keyword: &[u8]) -> bool {
    let Some(rest) = bytes.get(index..) else {
        return false;
    };

    if rest.first() != Some(&b'(') {
        return false;
    }

    let Some(after_keyword) = rest.get(1 + keyword.len()) else {
        return false;
    };

    rest.get(1..1 + keyword.len()) == Some(keyword)
        && matches!(after_keyword, b' ' | b'\n' | b'\r' | b'\t')
}

fn quoted_string_ranges(input: &str, limit: usize) -> Vec<Range<usize>> {
    let bytes = input.as_bytes();
    let mut results = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() && results.len() < limit {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        if let Some(end) = quoted_string_end(input, index) {
            results.push(index + 1..end);
            index = end + 1;
        } else {
            break;
        }
    }

    results
}

fn quoted_string_end(input: &str, start_quote: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = start_quote + 1;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn string_after_keyword_range(block: &str, start: usize, keyword: &str) -> Option<Range<usize>> {
    let prefix = format!("({keyword}");
    let rest = block.get(start..)?;
    if !rest.starts_with(&prefix) {
        return None;
    }

    let content_start = start + prefix.len();
    let quote_offset = block.get(content_start..)?.find('"')?;
    let quote_start = content_start + quote_offset;
    let quote_end = quoted_string_end(block, quote_start)?;
    Some(quote_start + 1..quote_end)
}

fn unescape_kicad_string(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                output.push(next);
            }
        } else {
            output.push(ch);
        }
    }

    output
}

fn escape_kicad_string(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '"' => {
                output.push('\\');
                output.push(ch);
            }
            _ => output.push(ch),
        }
    }
    output
}

fn ensure_library_is_within_output(
    output_path: &Path,
    source_file: &str,
) -> Result<PathBuf, String> {
    let source_file = normalize_source_file(source_file)?;
    let output_root = fs::canonicalize(output_path)
        .map_err(|err| format!("Failed to access {}: {}", output_path.display(), err))?;
    let joined = output_path.join(&source_file);
    let library_path = fs::canonicalize(&joined)
        .map_err(|err| format!("Failed to access {}: {}", joined.display(), err))?;
    if !library_path.starts_with(&output_root) {
        return Err("Source library file must stay inside the export output directory".to_string());
    }
    if !library_path.is_file() {
        return Err(format!(
            "Library path is not a file: {}",
            library_path.display()
        ));
    }

    Ok(library_path)
}

fn normalize_source_file(source_file: &str) -> Result<String, String> {
    if source_file.trim().is_empty() {
        return Err("Source library file is required".to_string());
    }

    let path = Path::new(source_file);
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return Err("Source library file must stay inside the export output directory".to_string());
    };

    if components.next().is_some() {
        return Err("Source library file must stay inside the export output directory".to_string());
    }

    let normalized = component
        .to_str()
        .ok_or_else(|| "Source library filename is not valid UTF-8".to_string())?
        .to_string();
    if !is_kicad_symbol_file(Path::new(&normalized)) {
        return Err("Source library file must be a .kicad_sym library".to_string());
    }

    Ok(normalized)
}

fn ensure_symbol_name_available(
    blocks: &[SymbolBlock<'_>],
    current_name: &str,
    new_name: &str,
) -> Result<(), String> {
    if current_name == new_name {
        return Ok(());
    }

    if blocks.iter().any(|block| block.symbol_name == new_name) {
        return Err(format!(
            "Symbol {} already exists in this library",
            new_name
        ));
    }

    Ok(())
}

fn update_symbol_block(
    block: &str,
    current_name: &str,
    new_name: &str,
    new_lcsc_part: &str,
) -> Result<String, String> {
    let mut replacements = rename_symbol_names_in_block(block, current_name, new_name)?;
    let lcsc_range = property_value_range(block, "LCSC Part")
        .ok_or_else(|| "Symbol is missing LCSC Part property".to_string())?;
    replacements.push((lcsc_range, escape_kicad_string(new_lcsc_part)));
    apply_replacements(block, replacements)
}

fn rename_symbol_names_in_block(
    block: &str,
    current_name: &str,
    new_name: &str,
) -> Result<Vec<(Range<usize>, String)>, String> {
    let bytes = block.as_bytes();
    let mut index = 0usize;
    let mut replacements = Vec::new();

    while index < bytes.len() {
        if starts_block_keyword(bytes, index, b"symbol") {
            let range = symbol_head_string_range(block, index)
                .ok_or_else(|| "symbol block is missing a name".to_string())?;
            let existing = unescape_kicad_string(&block[range.clone()]);
            if existing == current_name {
                replacements.push((range, escape_kicad_string(new_name)));
            } else if let Some(suffix) = existing.strip_prefix(current_name)
                && suffix.starts_with('_')
            {
                replacements.push((range, escape_kicad_string(&format!("{new_name}{suffix}"))));
            }
        }
        index += 1;
    }

    Ok(replacements)
}

fn symbol_head_string_range(block: &str, start: usize) -> Option<Range<usize>> {
    string_after_keyword_range(block, start, "symbol")
}

fn apply_replacements(
    input: &str,
    mut replacements: Vec<(Range<usize>, String)>,
) -> Result<String, String> {
    replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0.start));
    let mut result = input.to_string();
    let mut previous_start = input.len();

    for (range, replacement) in replacements {
        if range.start > range.end || range.end > result.len() {
            return Err("replacement range is out of bounds".to_string());
        }
        if range.end > previous_start {
            return Err("replacement ranges overlap".to_string());
        }
        result.replace_range(range.clone(), &replacement);
        previous_start = range.start;
    }

    Ok(result)
}

fn delete_symbol_block(content: &str, block: &SymbolBlock<'_>) -> String {
    let mut start = line_start(content, block.start);
    if start == 0 {
        start = block.start;
    }

    let mut end = block.end;
    if let Some(rest) = content.get(end..) {
        if rest.starts_with("\r\n") {
            end += 2;
        } else if rest.starts_with('\n') {
            end += 1;
        }
    }

    let mut updated = String::with_capacity(content.len().saturating_sub(end - start));
    updated.push_str(&content[..start]);
    updated.push_str(&content[end..]);
    updated
}

fn delete_generated_assets(
    output_path: &Path,
    library_path: &Path,
    footprint_name: &str,
    lcsc_part: Option<&str>,
) -> Result<DeletedGeneratedAssets, String> {
    let mut deleted = DeletedGeneratedAssets::default();
    let Some(library_name) = library_path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(deleted);
    };

    let pretty_dir = output_path.join(format!("{library_name}.pretty"));
    let footprint_path = pretty_dir.join(format!("{footprint_name}.kicad_mod"));
    if remove_file_if_exists(&footprint_path)? {
        deleted.footprints += 1;
    }

    if let Some(lcsc_part) = lcsc_part {
        deleted.models += delete_model_files_for_lcsc(output_path, library_name, lcsc_part)?;
        if remove_checkpoint_entry(output_path, lcsc_part)? {
            deleted.checkpoint_entries += 1;
        }
    }

    Ok(deleted)
}

fn delete_model_files_for_lcsc(
    output_path: &Path,
    library_name: &str,
    lcsc_part: &str,
) -> Result<usize, String> {
    let shapes_dir = output_path.join(format!("{library_name}.3dshapes"));
    let Ok(entries) = fs::read_dir(&shapes_dir) else {
        return Ok(0);
    };
    let suffix = format!("_{lcsc_part}");
    let mut removed = 0usize;

    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "Failed to read 3D model directory {}: {}",
                shapes_dir.display(),
                err
            )
        })?;
        let path = entry.path();
        if !is_generated_model_for_lcsc(&path, &suffix) {
            continue;
        }
        if remove_file_if_exists(&path)? {
            removed += 1;
        }
    }

    Ok(removed)
}

fn imported_models_for_library(
    output_path: &Path,
    library_path: &Path,
    lcsc_part: &str,
) -> Result<Vec<ImportedModel>, String> {
    let Some(library_name) = library_path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(Vec::new());
    };
    let shapes_dir = output_path.join(format!("{library_name}.3dshapes"));
    let entries = match fs::read_dir(&shapes_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(format!(
                "Failed to read 3D model directory {}: {}",
                shapes_dir.display(),
                err
            ));
        }
    };

    let suffix = format!("_{lcsc_part}");
    let mut models = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "Failed to read 3D model directory {}: {}",
                shapes_dir.display(),
                err
            )
        })?;
        let path = entry.path();
        if !is_generated_model_for_lcsc(&path, &suffix) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(format) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        let size_bytes = fs::metadata(&path)
            .map_err(|err| format!("Failed to inspect 3D model {}: {}", path.display(), err))?
            .len();
        models.push(ImportedModel {
            file_name: file_name.to_string(),
            format: format.to_ascii_lowercase(),
            size_bytes,
        });
    }
    models.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(models)
}

fn is_generated_model_for_lcsc(path: &Path, lcsc_suffix: &str) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        "step" | "stp" | "wrl"
    ) {
        return false;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(lcsc_suffix))
}

fn remove_checkpoint_entry(output_path: &Path, lcsc_part: &str) -> Result<bool, String> {
    let checkpoint_path = output_path.join(".checkpoint");
    let content = match fs::read_to_string(&checkpoint_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(format!(
                "Failed to read {}: {}",
                checkpoint_path.display(),
                err
            ));
        }
    };

    let mut removed = false;
    let mut kept_lines = Vec::new();
    for line in content.lines() {
        let checkpoint_id = line
            .split_once('\t')
            .map(|(id, _)| id)
            .unwrap_or(line)
            .trim();
        if checkpoint_id == lcsc_part {
            removed = true;
        } else {
            kept_lines.push(line);
        }
    }

    if !removed {
        return Ok(false);
    }

    let mut updated = kept_lines.join("\n");
    if !updated.is_empty() {
        updated.push('\n');
    }
    fs::write(&checkpoint_path, updated)
        .map_err(|err| format!("Failed to write {}: {}", checkpoint_path.display(), err))?;
    Ok(true)
}

fn remove_file_if_exists(path: &Path) -> Result<bool, String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(format!("Failed to remove {}: {}", path.display(), err)),
    }
}

fn format_delete_result(
    symbol_name: &str,
    source_file: &str,
    deleted_assets: &DeletedGeneratedAssets,
) -> String {
    let mut details = Vec::new();
    if deleted_assets.footprints > 0 {
        details.push(format!("{} footprint", deleted_assets.footprints));
    }
    if deleted_assets.models > 0 {
        details.push(format!("{} 3D model file(s)", deleted_assets.models));
    }
    if deleted_assets.checkpoint_entries > 0 {
        details.push("checkpoint entry".to_string());
    }

    if details.is_empty() {
        format!("Deleted {symbol_name} from {source_file}")
    } else {
        format!(
            "Deleted {symbol_name} from {source_file}; also removed {}",
            details.join(", ")
        )
    }
}

fn line_start(content: &str, index: usize) -> usize {
    content[..index].rfind('\n').map(|pos| pos + 1).unwrap_or(0)
}

fn is_kicad_symbol_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kicad_sym"))
}

#[cfg(test)]
mod tests {
    use super::{
        ImportedModelRequest, ImportedSymbol, ImportedSymbolDeleteRequest,
        ImportedSymbolUpdateRequest, LibraryFile, delete_imported_symbol, load_imported_symbols,
        load_imported_symbols_with_paths, parse_kicad_library_file, parse_kicad_symbol_lib,
        read_imported_model, unique_lcsc_parts, update_imported_symbol,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "happyjlc_imported_symbols_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    fn symbol_block(symbol_name: &str, lcsc_part: Option<&str>) -> String {
        let default_child = format!("{symbol_name}_0_1");
        symbol_block_with_children(symbol_name, lcsc_part, &[default_child.as_str()])
    }

    fn symbol_block_with_children(
        symbol_name: &str,
        lcsc_part: Option<&str>,
        child_names: &[&str],
    ) -> String {
        let lcsc_property = lcsc_part
            .map(|part| {
                format!(
                    "    (property\n      \"LCSC Part\"\n      \"{}\"\n      (id 5)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) hide)\n    )\n",
                    part
                )
            })
            .unwrap_or_default();
        let child_blocks = child_names
            .iter()
            .map(|child_name| format!("    (symbol \"{}\"\n    )\n", child_name))
            .collect::<String>();

        format!(
            "  (symbol \"{}\"\n    (property\n      \"Reference\"\n      \"U\"\n      (id 0)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) )\n    )\n{}{}  )\n",
            symbol_name, lcsc_property, child_blocks
        )
    }

    fn wrap_library(blocks: &[String]) -> String {
        format!(
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator happyjlc-test)\n{}\n)\n",
            blocks.join("")
        )
    }

    #[test]
    fn parses_minimal_symbol_library_with_source_file() {
        let content = wrap_library(&[symbol_block("Device_C123", Some("C123"))]);

        let parsed =
            parse_kicad_symbol_lib(&content, "alpha.kicad_sym").expect("parse should succeed");

        assert_eq!(
            parsed,
            vec![ImportedSymbol {
                library_key: "symbol:alpha.kicad_sym:Device_C123".to_string(),
                lcsc_part: "C123".to_string(),
                value: "Device_C123".to_string(),
                symbol_name: "Device_C123".to_string(),
                package: String::new(),
                source_file: "alpha.kicad_sym".to_string(),
                source_kind: "export".to_string(),
                editable: true,
                model_available: false,
                models: Vec::new(),
            }]
        );
    }

    #[test]
    fn parses_value_and_package_metadata() {
        let content = wrap_library(&["  (symbol \"R_C123\"\n    (property \"Value\" \"10k\" (id 1) (at 0 0 0))\n    (property \"Package\" \"R_0603\" (id 2) (at 0 0 0))\n    (property \"LCSC Part\" \"C123\" (id 3) (at 0 0 0))\n    (symbol \"R_C123_0_1\")\n  )\n".to_string()]);
        let parsed =
            parse_kicad_symbol_lib(&content, "alpha.kicad_sym").expect("parse should succeed");

        assert_eq!(parsed[0].value, "10k");
        assert_eq!(parsed[0].package, "R_0603");
    }

    #[test]
    fn parses_standard_footprint_without_lcsc_as_read_only_candidate() {
        let content = "(footprint \"LED_0603_1608Metric_Red\"\n  (property \"Package\" \"LED_0603_1608Metric_Red\")\n  (fp_text value \"LED_0603_1608Metric_Red\")\n)\n";
        let library_file = LibraryFile {
            path: PathBuf::from("LED_0603_1608Metric_Red.kicad_mod"),
            source_file: "LED_0603_1608Metric_Red.kicad_mod".to_string(),
            source_kind: "kicad_standard".to_string(),
            editable: false,
        };
        let parsed = parse_kicad_library_file(content, &library_file).unwrap();
        assert_eq!(parsed[0].lcsc_part, "");
        assert_eq!(parsed[0].package, "LED_0603_1608Metric_Red");
        assert_eq!(parsed[0].source_kind, "kicad_standard");
        assert!(!parsed[0].editable);
    }

    #[test]
    fn ignores_symbols_without_lcsc_part() {
        let content = wrap_library(&[
            symbol_block("Device_C123", Some("C123")),
            symbol_block("Graphic_Only", None),
        ]);

        let parsed =
            parse_kicad_symbol_lib(&content, "alpha.kicad_sym").expect("parse should succeed");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].symbol_name, "Device_C123");
        assert_eq!(parsed[1].lcsc_part, "");
    }

    #[test]
    fn load_imported_symbols_merges_multiple_files_and_deduplicates() {
        let root = test_root("multi_file");
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("alpha.kicad_sym"),
            wrap_library(&[
                symbol_block("Device_C123", Some("C123")),
                symbol_block("Amplifier_C456", Some("C456")),
            ]),
        )
        .unwrap();

        fs::write(
            root.join("beta.kicad_sym"),
            wrap_library(&[
                symbol_block("Device_C123", Some("C123")),
                symbol_block("Switch_C789", Some("C789")),
            ]),
        )
        .unwrap();

        let response = load_imported_symbols(&root).expect("scan should succeed");

        assert_eq!(response.scanned_path, root.display().to_string());
        assert_eq!(
            response.items,
            vec![
                ImportedSymbol {
                    library_key: "symbol:alpha.kicad_sym:Device_C123".to_string(),
                    lcsc_part: "C123".to_string(),
                    value: "Device_C123".to_string(),
                    symbol_name: "Device_C123".to_string(),
                    package: String::new(),
                    source_file: "alpha.kicad_sym".to_string(),
                    source_kind: "export".to_string(),
                    editable: true,
                    model_available: false,
                    models: Vec::new(),
                },
                ImportedSymbol {
                    library_key: "symbol:alpha.kicad_sym:Amplifier_C456".to_string(),
                    lcsc_part: "C456".to_string(),
                    value: "Amplifier_C456".to_string(),
                    symbol_name: "Amplifier_C456".to_string(),
                    package: String::new(),
                    source_file: "alpha.kicad_sym".to_string(),
                    source_kind: "export".to_string(),
                    editable: true,
                    model_available: false,
                    models: Vec::new(),
                },
                ImportedSymbol {
                    library_key: "symbol:beta.kicad_sym:Switch_C789".to_string(),
                    lcsc_part: "C789".to_string(),
                    value: "Switch_C789".to_string(),
                    symbol_name: "Switch_C789".to_string(),
                    package: String::new(),
                    source_file: "beta.kicad_sym".to_string(),
                    source_kind: "export".to_string(),
                    editable: true,
                    model_available: false,
                    models: Vec::new(),
                },
            ]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_explicitly_configured_paths_are_scanned_and_export_wins_duplicates() {
        let root = test_root("configured_paths");
        let output = root.join("export");
        let external = root.join("external");
        fs::create_dir_all(&output).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(
            output.join("happyjlc.kicad_sym"),
            wrap_library(&[symbol_block("Switch_C123", Some("c 123"))]),
        )
        .unwrap();
        fs::write(
            external.join("project.kicad_sym"),
            wrap_library(&[
                symbol_block("CopiedSwitch_C123", Some("C123")),
                symbol_block("Device_C456", Some("C456")),
            ]),
        )
        .unwrap();

        let response = load_imported_symbols_with_paths(&output, &[external.display().to_string()])
            .expect("configured scan should succeed");

        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].lcsc_part, "C123");
        assert_eq!(response.items[0].source_kind, "export");
        assert_eq!(response.items[0].source_file, "happyjlc.kicad_sym");
        assert_eq!(response.items[1].lcsc_part, "C456");
        assert_eq!(response.items[1].source_kind, "external");
        assert!(
            response
                .sources
                .iter()
                .all(|source| source.kind != "project" && source.kind != "kicad_standard")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discovers_and_reads_matching_3d_models() {
        let root = test_root("models");
        let shapes_dir = root.join("alpha.3dshapes");
        fs::create_dir_all(&shapes_dir).unwrap();
        fs::write(
            root.join("alpha.kicad_sym"),
            wrap_library(&[symbol_block("Device_C123", Some("C123"))]),
        )
        .unwrap();
        fs::write(shapes_dir.join("Device_C123.step"), b"step-data").unwrap();
        fs::write(shapes_dir.join("Device_C123.stp"), b"stp-data").unwrap();
        fs::write(shapes_dir.join("Device_C123.wrl"), b"wrl-data").unwrap();
        fs::write(shapes_dir.join("Other_C456.step"), b"other-data").unwrap();
        fs::write(shapes_dir.join("notes.txt"), b"ignore").unwrap();

        let response = load_imported_symbols(&root).expect("scan should succeed");
        assert_eq!(response.items.len(), 1);
        assert_eq!(
            response.items[0]
                .models
                .iter()
                .map(|model| model.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Device_C123.step", "Device_C123.stp", "Device_C123.wrl"]
        );
        assert_eq!(response.items[0].models[0].size_bytes, 9);

        let content = read_imported_model(
            &root,
            ImportedModelRequest {
                source_file: "alpha.kicad_sym".to_string(),
                lcsc_part: "C123".to_string(),
                file_name: "Device_C123.wrl".to_string(),
            },
        )
        .expect("model read should succeed");
        assert_eq!(content, b"wrl-data");

        let error = read_imported_model(
            &root,
            ImportedModelRequest {
                source_file: "alpha.kicad_sym".to_string(),
                lcsc_part: "C123".to_string(),
                file_name: "../notes.txt".to_string(),
            },
        )
        .unwrap_err();
        assert!(error.contains("not found"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn updates_symbol_name_lcsc_part_and_child_symbol_names() {
        let root = test_root("update");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block_with_children(
                    "Alpha",
                    Some("C123"),
                    &["Alpha_0_1", "Alpha_1_1", "AlphaAlias"],
                ),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let result = update_imported_symbol(
            &root,
            ImportedSymbolUpdateRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                new_symbol_name: "Gamma".to_string(),
                lcsc_part: "C999".to_string(),
            },
        )
        .expect("update should succeed");

        assert!(result.contains("Updated Alpha"));
        let updated = fs::read_to_string(&library).unwrap();
        assert!(updated.contains("(symbol \"Gamma\""));
        assert!(updated.contains("(symbol \"Gamma_0_1\""));
        assert!(updated.contains("(symbol \"Gamma_1_1\""));
        assert!(updated.contains("(symbol \"AlphaAlias\""));
        assert!(updated.contains("\"LCSC Part\"\n      \"C999\""));
        assert!(!updated.contains("(symbol \"Alpha\""));
        assert!(!updated.contains("(symbol \"Alpha_0_1\""));
        assert!(!updated.contains("(symbol \"Alpha_1_1\""));

        let response = load_imported_symbols(&root).unwrap();
        assert!(
            response
                .items
                .iter()
                .any(|item| item.symbol_name == "Gamma" && item.lcsc_part == "C999")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_symbol_name_updates() {
        let root = test_root("duplicate_name");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block("Alpha", Some("C123")),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let error = update_imported_symbol(
            &root,
            ImportedSymbolUpdateRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                new_symbol_name: "Beta".to_string(),
                lcsc_part: "C123".to_string(),
            },
        )
        .unwrap_err();

        assert!(error.contains("already exists"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_source_file_outside_output_directory() {
        let root = test_root("outside");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("alpha.kicad_sym"),
            wrap_library(&[symbol_block("Alpha", Some("C123"))]),
        )
        .unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "../alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("inside the export output directory"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_library_source_files() {
        let root = test_root("non_library");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("alpha.txt"), "not a symbol library").unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.txt".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains(".kicad_sym"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deletes_only_target_symbol_block() {
        let root = test_root("delete");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block("Alpha", Some("C123")),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let result = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .expect("delete should succeed");

        assert!(result.contains("Deleted Alpha"));
        let updated = fs::read_to_string(&library).unwrap();
        assert!(!updated.contains("(symbol \"Alpha\""));
        assert!(updated.contains("(symbol \"Beta\""));

        let response = load_imported_symbols(&root).unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].symbol_name, "Beta");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_removes_matching_generated_assets_and_checkpoint_entry() {
        let root = test_root("delete_assets");
        let pretty_dir = root.join("happyjlc.pretty");
        let shapes_dir = root.join("happyjlc.3dshapes");
        fs::create_dir_all(&pretty_dir).unwrap();
        fs::create_dir_all(&shapes_dir).unwrap();

        let symbol_name = "ESP32-C3-MINI-1-N4_C2838502";
        let footprint_path = pretty_dir.join(format!("{symbol_name}.kicad_mod"));
        let step_path = shapes_dir.join("WIFIM-SMD_ESP32-C3-MINI-1_C2838502.step");
        let wrl_path = shapes_dir.join("WIFIM-SMD_ESP32-C3-MINI-1_C2838502.wrl");
        let other_model_path = shapes_dir.join("OTHER_C123.step");

        fs::write(&footprint_path, "footprint").unwrap();
        fs::write(&step_path, "step").unwrap();
        fs::write(&wrl_path, "wrl").unwrap();
        fs::write(&other_model_path, "other").unwrap();
        fs::write(root.join(".checkpoint"), "C123\tsfm\nC2838502\tsfm\n").unwrap();
        fs::write(
            root.join("happyjlc.kicad_sym"),
            wrap_library(&[format!(
                "  (symbol \"{symbol_name}\"\n    (property \"LCSC Part\" \"C2838502\" (id 5) (at 0 0 0))\n    (property \"Footprint\" \"happyjlc:{symbol_name}\" (id 2) (at 0 0 0))\n    (symbol \"{symbol_name}_0_1\")\n  )\n"
            )]),
        )
        .unwrap();

        let result = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "happyjlc.kicad_sym".to_string(),
                symbol_name: symbol_name.to_string(),
                lcsc_part: None,
            },
        )
        .expect("delete should succeed");

        assert!(result.contains("1 footprint"));
        assert!(result.contains("2 3D model file(s)"));
        assert!(result.contains("checkpoint entry"));
        assert!(!footprint_path.exists());
        assert!(!step_path.exists());
        assert!(!wrl_path.exists());
        assert!(other_model_path.exists());
        assert_eq!(
            fs::read_to_string(root.join(".checkpoint")).unwrap(),
            "C123\tsfm\n"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_uses_only_top_level_symbol_names() {
        let root = test_root("delete_nested_name");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[symbol_block_with_children(
                "Alpha",
                Some("C123"),
                &["Alpha_0_1", "Alpha_1_1"],
            )]),
        )
        .unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha_0_1".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("was not found"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_directory_returns_empty_response() {
        let root = test_root("missing");

        let response = load_imported_symbols(&root).expect("scan should succeed");

        assert_eq!(response.scanned_path, root.display().to_string());
        assert!(response.items.is_empty());
    }

    #[test]
    fn unique_lcsc_parts_preserves_first_sorted_occurrence() {
        let parts = unique_lcsc_parts(&[
            ImportedSymbol {
                library_key: "symbol:alpha.kicad_sym:Alpha".to_string(),
                lcsc_part: "C123".to_string(),
                value: "Alpha".to_string(),
                symbol_name: "Alpha".to_string(),
                package: String::new(),
                source_file: "alpha.kicad_sym".to_string(),
                source_kind: "export".to_string(),
                editable: true,
                model_available: false,
                models: Vec::new(),
            },
            ImportedSymbol {
                library_key: "symbol:beta.kicad_sym:Beta".to_string(),
                lcsc_part: "C123".to_string(),
                value: "Beta".to_string(),
                symbol_name: "Beta".to_string(),
                package: String::new(),
                source_file: "beta.kicad_sym".to_string(),
                source_kind: "export".to_string(),
                editable: true,
                model_available: false,
                models: Vec::new(),
            },
            ImportedSymbol {
                library_key: "symbol:beta.kicad_sym:Gamma".to_string(),
                lcsc_part: "C456".to_string(),
                value: "Gamma".to_string(),
                symbol_name: "Gamma".to_string(),
                package: String::new(),
                source_file: "beta.kicad_sym".to_string(),
                source_kind: "export".to_string(),
                editable: true,
                model_available: false,
                models: Vec::new(),
            },
        ]);

        assert_eq!(parts, vec!["C123".to_string(), "C456".to_string()]);
    }
}
