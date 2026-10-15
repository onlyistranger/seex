use crate::error::Result;
use crate::kicad::LibraryManager;
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasheetExportStatus {
    Downloaded,
    Skipped,
    NotFound,
    Failed,
}

static DATASHEET_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn datasheet_client() -> &'static reqwest::Client {
    DATASHEET_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create datasheet HTTP client")
    })
}

/// Download datasheet PDF for a component and save it to the library doc folder
pub async fn download_datasheet_pdf(
    client: &reqwest::Client,
    datasheet_url: &str,
    doc_name: &str,
    lcsc_id: &str,
    manager: &LibraryManager,
    force: bool,
) -> Result<DatasheetExportStatus> {
    let doc_path = manager.get_doc_path(doc_name);
    if !force && doc_path.exists() {
        if let Ok(meta) = fs::metadata(&doc_path)
            && meta.len() > 0
        {
            log::debug!("Datasheet PDF already exists: {}", doc_path.display());
            return Ok(DatasheetExportStatus::Skipped);
        }
    }

    let initial_url = datasheet_url.trim();
    let mut target_url = if initial_url.is_empty() {
        if lcsc_id.is_empty() {
            return Ok(DatasheetExportStatus::NotFound);
        }
        format!("https://www.lcsc.com/datasheet/{}.pdf", lcsc_id)
    } else {
        initial_url.to_string()
    };

    let mut tried_fallback = target_url.contains("lcsc.com/datasheet/");

    for attempt in 0..4 {
        log::debug!("Fetching datasheet attempt {} at URL: {}", attempt + 1, target_url);
        let resp = match client.get(&target_url).send().await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Failed request to {}: {}", target_url, e);
                if !tried_fallback && !lcsc_id.is_empty() {
                    tried_fallback = true;
                    target_url = format!("https://www.lcsc.com/datasheet/{}.pdf", lcsc_id);
                    continue;
                }
                return Ok(DatasheetExportStatus::Failed);
            }
        };

        if !resp.status().is_success() {
            log::debug!("Datasheet URL {} returned status {}", target_url, resp.status());
            if !tried_fallback && !lcsc_id.is_empty() {
                tried_fallback = true;
                target_url = format!("https://www.lcsc.com/datasheet/{}.pdf", lcsc_id);
                continue;
            }
            return Ok(DatasheetExportStatus::NotFound);
        }

        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Failed reading bytes from {}: {}", target_url, e);
                return Ok(DatasheetExportStatus::Failed);
            }
        };

        if bytes.starts_with(b"%PDF") {
            manager.write_doc_file(doc_name, &bytes)?;
            return Ok(DatasheetExportStatus::Downloaded);
        }

        // Response is HTML; parse it to extract the real PDF URL
        let html = String::from_utf8_lossy(&bytes);
        if let Some(next_url) = extract_pdf_url_from_html(&html, lcsc_id) {
            if next_url != target_url {
                target_url = next_url;
                continue;
            }
        }

        // If not found in HTML and fallback hasn't been tried yet
        if !tried_fallback && !lcsc_id.is_empty() {
            tried_fallback = true;
            target_url = format!("https://www.lcsc.com/datasheet/{}.pdf", lcsc_id);
            continue;
        }

        break;
    }

    log::warn!("Could not resolve valid PDF for {} (URL: {})", doc_name, datasheet_url);
    Ok(DatasheetExportStatus::NotFound)
}

/// Extract candidate PDF URL from an HTML page (szlcsc datasheet viewer, lcsc viewer, product-detail page)
pub fn extract_pdf_url_from_html(html: &str, lcsc_id: &str) -> Option<String> {
    // 1. Search for iframe or embed src
    static FRAME_RE: OnceLock<Regex> = OnceLock::new();
    let frame_re = FRAME_RE.get_or_init(|| {
        Regex::new(r#"(?i)<(?:iframe|embed)[^>]+src=["']([^"']+)["']"#).unwrap()
    });

    for cap in frame_re.captures_iter(html) {
        let src = &cap[1];
        if src.contains("viewer.html") && src.contains("file=") {
            if let Some(file_url) = extract_file_param(src) {
                return Some(clean_url(&file_url));
            }
        }
        if src.to_ascii_lowercase().contains(".pdf") {
            return Some(clean_url(src));
        }
    }

    // 2. Search for direct datasheet.lcsc.com or atta.szlcsc.com URLs
    if !lcsc_id.is_empty() {
        let pattern = format!(
            r#"(?i)https?://datasheet\.lcsc\.com/datasheet/pdf/[^\s"'<>]+\.pdf(?:\?[^\s"'<>]*productCode={}[^\s"'<>]*)?"#,
            regex::escape(lcsc_id)
        );
        if let Ok(re) = Regex::new(&pattern)
            && let Some(m) = re.find(html)
        {
            return Some(clean_url(m.as_str()));
        }
    }

    static LCSC_RE: OnceLock<Regex> = OnceLock::new();
    let lcsc_re = LCSC_RE.get_or_init(|| {
        Regex::new(r#"(?i)https?://datasheet\.lcsc\.com/datasheet/pdf/[^\s"'<>]+\.pdf(?:\?[^\s"'<>]*)?"#).unwrap()
    });
    if let Some(m) = lcsc_re.find(html) {
        return Some(clean_url(m.as_str()));
    }

    static ATTA_RE: OnceLock<Regex> = OnceLock::new();
    let atta_re = ATTA_RE.get_or_init(|| {
        Regex::new(r#"(?i)https?://atta\.szlcsc\.com/upload/[^\s"'<>]+\.pdf"#).unwrap()
    });
    if let Some(m) = atta_re.find(html) {
        return Some(clean_url(m.as_str()));
    }

    // 3. Search for links to lcsc datasheet page (/datasheet/Cxxxx.pdf)
    if !lcsc_id.is_empty() {
        let pattern = format!(
            r#"(?i)https?://www\.lcsc\.com/datasheet/{}\.pdf"#,
            regex::escape(lcsc_id)
        );
        if let Ok(re) = Regex::new(&pattern)
            && let Some(m) = re.find(html)
        {
            return Some(m.as_str().to_string());
        }
    }

    None
}

fn clean_url(url: &str) -> String {
    let mut cleaned = url.trim();
    // Strip fragment (#...)
    if let Some((base, _)) = cleaned.split_once('#') {
        cleaned = base;
    }
    if cleaned.starts_with("//") {
        format!("https:{}", cleaned)
    } else {
        cleaned.to_string()
    }
}

fn extract_file_param(src: &str) -> Option<String> {
    let query_str = src.split_once('?')
        .map(|(_, q)| q)
        .unwrap_or(src);
    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "file" {
                let decoded = percent_decode(v);
                return Some(decoded);
            }
        }
    }
    None
}

pub fn percent_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        let encoded = "https%3A%2F%2Fatta.szlcsc.com%2Fupload%2Ftest.pdf";
        assert_eq!(percent_decode(encoded), "https://atta.szlcsc.com/upload/test.pdf");
    }

    #[test]
    fn test_extract_szlcsc_iframe() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <iframe src="https://atta.szlcsc.com/upload/public/pdf/source/20211116/1637048705972.pdf#page=1&view=fitH" width="100%"></iframe>
            </body>
            </html>
        "#;
        let url = extract_pdf_url_from_html(html, "C48542676");
        assert_eq!(
            url.as_deref(),
            Some("https://atta.szlcsc.com/upload/public/pdf/source/20211116/1637048705972.pdf")
        );
    }

    #[test]
    fn test_extract_viewer_file_param() {
        let html = r#"
            <iframe src="/pdfjs/web/viewer.html?file=https%3A%2F%2Fatta.szlcsc.com%2Fupload%2Ftest.pdf#page=1"></iframe>
        "#;
        let url = extract_pdf_url_from_html(html, "C123");
        assert_eq!(url.as_deref(), Some("https://atta.szlcsc.com/upload/test.pdf"));
    }

    #[test]
    fn test_extract_lcsc_iframe() {
        let html = r#"
            <iframe src="https://datasheet.lcsc.com/datasheet/pdf/20230101.pdf?productCode=C77978" id="pdfFrame"></iframe>
        "#;
        let url = extract_pdf_url_from_html(html, "C77978");
        assert_eq!(
            url.as_deref(),
            Some("https://datasheet.lcsc.com/datasheet/pdf/20230101.pdf?productCode=C77978")
        );
    }

    #[test]
    fn test_extract_lcsc_product_detail() {
        let html = r#"
            <div class="product-info">
                <a href="https://www.lcsc.com/datasheet/C77978.pdf" target="_blank">Download Datasheet</a>
            </div>
        "#;
        let url = extract_pdf_url_from_html(html, "C77978");
        assert_eq!(url.as_deref(), Some("https://www.lcsc.com/datasheet/C77978.pdf"));
    }

    #[tokio::test]
    async fn test_live_download_szlcsc_datasheet() {
        let root = std::env::temp_dir().join(format!("test_ds_szlcsc_{}", std::process::id()));
        let manager = LibraryManager::new(&root);
        manager.create_directories().unwrap();

        let client = datasheet_client();
        let status = download_datasheet_pdf(
            client,
            "https://item.szlcsc.com/datasheet/DON70N03/50802276.html",
            "DON70N03_C48542676",
            "C48542676",
            &manager,
            true,
        )
        .await
        .unwrap();

        assert_eq!(status, DatasheetExportStatus::Downloaded);
        let path = manager.get_doc_path("DON70N03_C48542676");
        assert!(path.exists());
        let data = fs::read(&path).unwrap();
        assert!(data.starts_with(b"%PDF"));

        // Test skip
        let status_skip = download_datasheet_pdf(
            client,
            "https://item.szlcsc.com/datasheet/DON70N03/50802276.html",
            "DON70N03_C48542676",
            "C48542676",
            &manager,
            false,
        )
        .await
        .unwrap();
        assert_eq!(status_skip, DatasheetExportStatus::Skipped);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn test_live_download_lcsc_product_detail() {
        let root = std::env::temp_dir().join(format!("test_ds_lcsc_{}", std::process::id()));
        let manager = LibraryManager::new(&root);
        manager.create_directories().unwrap();

        let client = datasheet_client();
        let status = download_datasheet_pdf(
            client,
            "https://www.lcsc.com/product-detail/Switching-Diode_MCC_1N4148W-TP_1N4148W-TP_C77978.html",
            "1N4148W-TP_C77978",
            "C77978",
            &manager,
            true,
        )
        .await
        .unwrap();

        assert_eq!(status, DatasheetExportStatus::Downloaded);
        let path = manager.get_doc_path("1N4148W-TP_C77978");
        assert!(path.exists());
        let data = fs::read(&path).unwrap();
        assert!(data.starts_with(b"%PDF"));

        let _ = fs::remove_dir_all(root);
    }
}
