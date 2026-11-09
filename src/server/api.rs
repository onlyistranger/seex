use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::altium::workflow::download_step;
use crate::batch::{BatchOptions, export_batch};
use crate::cli::KicadVersion;
use crate::kicad::{
    KicadBatchOptions, KicadExportOptions, KicadLibraryOptions, export_kicad_batch,
};
use crate::lceda::LcedaClient;
use crate::step_logo::{CleanMode, LogoOutcome, strip_logo_with_mode};
use crate::step_model::ModelCleanup;
use crate::util::nested_string;

#[derive(Debug, Serialize)]
pub struct ServerStatusResponse {
    pub status: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub arch: &'static str,
    pub os: &'static str,
    pub uptime_secs: u64,
}

static SERVER_START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn init_server_start_time() {
    SERVER_START_TIME.get_or_init(Instant::now);
}

pub async fn status_handler() -> Json<ServerStatusResponse> {
    let start = SERVER_START_TIME.get_or_init(Instant::now);
    Json(ServerStatusResponse {
        status: "ok",
        name: "SeEx EDA Web Service",
        version: env!("CARGO_PKG_VERSION"),
        arch: std::env::consts::ARCH,
        os: std::env::consts::OS,
        uptime_secs: start.elapsed().as_secs(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct SearchItemResponse {
    pub lcsc_id: String,
    pub title: String,
    pub display_title: String,
    pub manufacturer: String,
    pub package: String,
    pub description: String,
    pub has_3d: bool,
    pub model_uuid: Option<String>,
}

pub async fn search_handler(
    Query(query): Query<SearchQuery>,
) -> std::result::Result<Json<Vec<SearchItemResponse>>, (StatusCode, Json<serde_json::Value>)> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let client = LcedaClient::new();
    let mut items = client.search_components(q).await.unwrap_or_default();

    // If search yielded no results but looks like a C-id (e.g. C6186), try selecting directly
    if items.is_empty() && q.starts_with(['c', 'C']) {
        if let Ok(single) = client.select_item(q, 1).await {
            items.push(single);
        }
    }

    let results: Vec<SearchItemResponse> = items
        .into_iter()
        .map(|item| {
            let lcsc_id = item.lcsc_id().unwrap_or_else(|| {
                if item.title.starts_with(['c', 'C']) {
                    item.title.clone()
                } else {
                    item.display_name().to_string()
                }
            });

            let package = nested_string(&item.raw, &["package"])
                .or_else(|| nested_string(&item.raw, &["footprint", "display_title"]))
                .or_else(|| nested_string(&item.raw, &["attributes", "Package"]))
                .unwrap_or_default();

            let description = nested_string(&item.raw, &["description"])
                .or_else(|| nested_string(&item.raw, &["attributes", "Description"]))
                .unwrap_or_default();

            SearchItemResponse {
                lcsc_id,
                title: item.title,
                display_title: item.display_title,
                manufacturer: item.manufacturer,
                package,
                description,
                has_3d: item.model_uuid.is_some(),
                model_uuid: item.model_uuid,
            }
        })
        .collect();

    Ok(Json(results))
}

#[derive(Debug, Deserialize)]
pub struct DetailQuery {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct DetailResponse {
    pub lcsc_id: String,
    pub title: String,
    pub manufacturer: String,
    pub package: String,
    pub supplier_footprint: String,
    pub description: String,
    pub datasheet: Option<String>,
    pub model_3d: Option<Model3dInfo>,
}

#[derive(Debug, Serialize)]
pub struct Model3dInfo {
    pub title: String,
    pub uuid: String,
}

pub async fn detail_handler(
    Query(query): Query<DetailQuery>,
) -> std::result::Result<Json<DetailResponse>, (StatusCode, Json<serde_json::Value>)> {
    let id = query.id.trim();
    if id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Component ID is required" })),
        ));
    }

    let client = LcedaClient::new();
    let item = client.select_item(id, 1).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Component not found: {e}") })),
        )
    })?;

    let lcsc_id = item.lcsc_id().unwrap_or_else(|| id.to_string());
    let title = if !item.title.is_empty() {
        item.title.clone()
    } else {
        item.display_name().to_string()
    };

    let package = nested_string(&item.raw, &["package"])
        .or_else(|| nested_string(&item.raw, &["attributes", "Package"]))
        .unwrap_or_default();

    let supplier_footprint = nested_string(&item.raw, &["footprint", "display_title"])
        .or_else(|| nested_string(&item.raw, &["attributes", "Footprint"]))
        .unwrap_or_default();

    let description = nested_string(&item.raw, &["description"])
        .or_else(|| nested_string(&item.raw, &["attributes", "Description"]))
        .unwrap_or_default();

    let datasheet = nested_string(&item.raw, &["attributes", "Datasheet"])
        .or_else(|| nested_string(&item.raw, &["datasheet"]))
        .or_else(|| nested_string(&item.raw, &["data_manual"]));

    let model_3d = item.model_uuid.as_ref().map(|uuid| Model3dInfo {
        title: item.choose_step_filename(),
        uuid: uuid.clone(),
    });

    Ok(Json(DetailResponse {
        lcsc_id,
        title,
        manufacturer: item.manufacturer,
        package,
        supplier_footprint,
        description,
        datasheet,
        model_3d,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CleanStepQuery {
    pub mode: Option<String>,
    pub filename: Option<String>,
}

pub async fn clean_step_handler(
    Query(query): Query<CleanStepQuery>,
    body: Bytes,
) -> std::result::Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Empty STEP file payload" })),
        ));
    }

    let clean_mode = match query.mode.as_deref() {
        Some("all_markings") => CleanMode::AllMarkings,
        _ => CleanMode::LogoOnly,
    };

    let raw_name = query
        .filename
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "model.step".to_string());
    let out_name = format!("cleaned_{raw_name}");

    let (cleaned_opt, outcome) = strip_logo_with_mode(&body, clean_mode);

    let (logos, pieces, faces, outcome_str) = match &outcome {
        LogoOutcome::Removed(rem) => (rem.logos, rem.pieces, rem.faces, "Removed"),
        LogoOutcome::NotFound => (0, 0, 0, "NotFound"),
        LogoOutcome::Kept => (0, 0, 0, "Kept (Safety Protection)"),
        LogoOutcome::Unreadable => (0, 0, 0, "Unreadable"),
    };

    let payload = cleaned_opt.unwrap_or_else(|| body.to_vec());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{out_name}\"")
            .parse()
            .unwrap(),
    );
    headers.insert("X-Removal-Logos", logos.to_string().parse().unwrap());
    headers.insert("X-Removal-Pieces", pieces.to_string().parse().unwrap());
    headers.insert("X-Removal-Faces", faces.to_string().parse().unwrap());
    headers.insert("X-Outcome", outcome_str.parse().unwrap());

    Ok((headers, payload).into_response())
}

#[derive(Debug, Deserialize)]
pub struct WebExportRequest {
    pub ids: Vec<String>,
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub library_name: Option<String>,
    #[serde(default)]
    pub download_pdf: bool,
    #[serde(default)]
    pub add_suffix: bool,
    #[serde(default)]
    pub replace_dot: bool,
    #[serde(default)]
    pub preserve_package: bool,
    #[serde(default = "default_true")]
    pub remove_model_logo: bool,
    #[serde(default)]
    pub clean_all_markings: bool,
    #[serde(default)]
    pub lcsc_english: bool,
}

fn default_target() -> String {
    "kicad".to_string()
}
fn default_mode() -> String {
    "full".to_string()
}
fn default_true() -> bool {
    true
}

pub async fn export_handler(
    Json(req): Json<WebExportRequest>,
) -> std::result::Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if req.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "No component IDs provided for export" })),
        ));
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!(
        "seex_web_export_{}_{}",
        std::process::id(),
        timestamp
    ));

    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({ "error": format!("Failed to create temporary directory: {e}") }),
            ),
        ));
    }

    let target_lower = req.target.to_lowercase();
    let mode_lower = req.mode.to_lowercase();
    let lib_name = req
        .library_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "SeExExport".to_string());

    let models = Arc::new(ModelCleanup::with_options(
        req.remove_model_logo,
        req.clean_all_markings,
    ));

    let export_result: Result<(), String> = if target_lower == "altium" {
        let client = LcedaClient::new();
        if mode_lower == "3d" {
            let mut failed = Vec::new();
            for id in &req.ids {
                match client.select_item(id, 1).await {
                    Ok(item) => {
                        if let Err(e) =
                            download_step(&client, &item, &temp_dir, true, &models).await
                        {
                            failed.push(format!("{id}: {e}"));
                        }
                    }
                    Err(e) => failed.push(format!("{id}: {e}")),
                }
            }
            if !failed.is_empty() && failed.len() == req.ids.len() {
                Err(format!(
                    "Altium 3D export failed for all components: {}",
                    failed.join("; ")
                ))
            } else {
                Ok(())
            }
        } else {
            let input_file = temp_dir.join("batch_ids_input.txt");
            if let Ok(mut f) = File::create(&input_file) {
                for id in &req.ids {
                    let _ = writeln!(f, "{id}");
                }
            }

            let batch_options = BatchOptions {
                input: input_file.clone(),
                output: temp_dir.clone(),
                schlib: mode_lower == "symbol" || mode_lower == "full",
                pcblib: mode_lower == "footprint" || mode_lower == "full",
                full: mode_lower == "full",
                merge: false,
                append: false,
                library_name: Some(lib_name.clone()),
                parallel: 4,
                continue_on_error: true,
                english_metadata: req.lcsc_english,
                force: true,
                schematic_stroke_color: [0, 0, 0],
                schematic_fill_color: None,
                models: Arc::clone(&models),
                progress: None,
            };

            let res = export_batch(&client, batch_options).await;
            let _ = std::fs::remove_file(&input_file);
            res.map(|_| ()).map_err(|e| e.to_string())
        }
    } else {
        // Default to KiCad
        let batch_options = KicadBatchOptions {
            ids: req.ids.clone(),
            library: KicadLibraryOptions {
                output: temp_dir.clone(),
                library_name: Some(lib_name.clone()),
                symbol_lib: None,
                footprint_lib: None,
                model_lib: None,
                doc_lib: None,
            },
            export: KicadExportOptions {
                symbol: mode_lower == "symbol",
                footprint: mode_lower == "footprint",
                model_3d: mode_lower == "3d",
                full: mode_lower == "full",
                force: true,
                kicad_version: KicadVersion::V6,
                models: Arc::clone(&models),
                download_pdf: req.download_pdf,
                add_suffix: req.add_suffix,
                replace_dot: req.replace_dot,
                preserve_package: req.preserve_package,
            },
            english_metadata: req.lcsc_english,
            parallel: 4,
            continue_on_error: true,
            append: false,
            progress: None,
        };

        export_kicad_batch(batch_options)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    };

    if let Err(err_msg) = export_result {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Export process failed: {err_msg}") })),
        ));
    }

    // Zip up the generated directory
    let zip_bytes = match zip_directory_to_memory(&temp_dir) {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to create ZIP package: {e}") })),
            ));
        }
    };

    // Clean up temporary files
    let _ = std::fs::remove_dir_all(&temp_dir);

    let zip_filename = if req.ids.len() == 1 {
        format!("{}_{}.zip", req.ids[0], target_lower)
    } else {
        format!("SeEx_{}_{}_parts.zip", target_lower, req.ids.len())
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/zip".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{zip_filename}\"")
            .parse()
            .unwrap(),
    );

    Ok((headers, zip_bytes).into_response())
}

/// Compress an entire directory tree into an in-memory ZIP buffer
fn zip_directory_to_memory(src_dir: &Path) -> std::io::Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = match path.strip_prefix(src_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let name_str = name.to_string_lossy().replace('\\', "/");
            if name_str.is_empty() {
                continue;
            }

            if path.is_dir() {
                zip.add_directory(format!("{name_str}/"), options)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            } else if path.is_file() {
                zip.start_file(name_str, options)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let mut f = File::open(path)?;
                std::io::copy(&mut f, &mut zip)?;
            }
        }

        zip.finish()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    }

    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_zip_directory_to_memory() {
        let temp = std::env::temp_dir().join(format!("seex_test_zip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(temp.join("sub")).unwrap();

        std::fs::write(temp.join("test.txt"), "Hello, SeEx Web!").unwrap();
        std::fs::write(temp.join("sub").join("nested.step"), "ISO-10303-21;").unwrap();

        let zip_bytes = zip_directory_to_memory(&temp).expect("zip should succeed");
        assert!(!zip_bytes.is_empty());

        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("valid zip");
        assert!(archive.len() >= 2);

        let mut file = archive.by_name("test.txt").expect("test.txt in zip");
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(content, "Hello, SeEx Web!");

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn test_server_status_handler() {
        init_server_start_time();
        let res = status_handler().await;
        assert_eq!(res.status, "ok");
        assert_eq!(res.name, "SeEx EDA Web Service");
    }
}
