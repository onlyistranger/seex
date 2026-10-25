//! Batch and single-file 3D STEP logo removal tool.
//!
//! Provides the engine to scan a file or folder for STEP models, process them
//! using the algorithm in [`crate::step_logo::strip_logo`], and save the cleaned
//! models to a destination folder, overwriting existing files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::step_logo::{LogoOutcome, strip_logo};

/// Options for cleaning STEP models.
#[derive(Debug, Clone)]
pub struct CleanStepOptions {
    /// Input STEP file or directory containing STEP files.
    pub input: PathBuf,
    /// Destination directory where cleaned files will be saved.
    pub output: PathBuf,
    /// Whether to recursively search subdirectories when `input` is a folder.
    pub recursive: bool,
}

/// Processing result for a single STEP file.
#[derive(Debug, Clone)]
pub struct CleanStepItemDetail {
    pub src_path: PathBuf,
    pub dest_path: PathBuf,
    pub outcome: LogoOutcome,
    pub error: Option<String>,
}

/// Overall summary of the STEP cleaning operation.
#[derive(Debug, Default, Clone)]
pub struct CleanStepSummary {
    pub total_files: usize,
    /// Number of distinct logo instances / surfaces removed across all files.
    pub cleaned_logos_count: usize,
    /// Number of files that had at least one logo removed.
    pub cleaned_files_count: usize,
    /// Total logos removed (alias for `cleaned_logos_count`).
    pub cleaned_count: usize,
    pub cleaned_pieces: usize,
    pub cleaned_faces: usize,
    pub not_found_count: usize,
    pub kept_count: usize,
    pub unreadable_count: usize,
    pub error_count: usize,
    pub details: Vec<CleanStepItemDetail>,
}

/// Checks if a file path has a STEP extension (`.step` or `.stp`, case-insensitive).
pub fn is_step_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("step") || ext.eq_ignore_ascii_case("stp"))
        .unwrap_or(false)
}

/// Discovers all STEP files matching the input path (single file or directory).
pub fn collect_step_files(input: &Path, recursive: bool) -> Vec<PathBuf> {
    if input.is_file() {
        return vec![input.to_path_buf()];
    }
    if !input.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_from_dir(input, recursive, &mut files);
    files.sort();
    files
}

fn collect_from_dir(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if is_step_file(&path) {
                out.push(path);
            }
        } else if path.is_dir() && recursive {
            collect_from_dir(&path, recursive, out);
        }
    }
}

/// Computes the destination path for a given source file.
/// If `input_base` is a directory, preserves the relative path hierarchy under `output_dir`.
/// If `input_base` is a file, places it directly in `output_dir`.
pub fn compute_dest_path(input_base: &Path, src_file: &Path, output_dir: &Path) -> PathBuf {
    if input_base == src_file || input_base.is_file() {
        let file_name = src_file.file_name().unwrap_or_default();
        output_dir.join(file_name)
    } else {
        match src_file.strip_prefix(input_base) {
            Ok(rel) if !rel.as_os_str().is_empty() => output_dir.join(rel),
            _ => {
                let file_name = src_file.file_name().unwrap_or_default();
                output_dir.join(file_name)
            }
        }
    }
}

/// Cleans one STEP file in memory and saves the result to `dest_path`.
/// Returns the outcome and any write error encountered.
pub fn clean_single_step_file(src_path: &Path, dest_path: &Path) -> (LogoOutcome, Option<String>) {
    let data = match fs::read(src_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                LogoOutcome::Unreadable,
                Some(format!("Failed to read {}: {e}", src_path.display())),
            );
        }
    };

    let (maybe_cleaned, outcome) = strip_logo(&data);
    let bytes_to_write = match (&maybe_cleaned, outcome) {
        (Some(cleaned), LogoOutcome::Removed(_)) => cleaned.as_slice(),
        _ => data.as_slice(),
    };

    if let Some(parent) = dest_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return (
                outcome,
                Some(format!(
                    "Failed to create directory {}: {e}",
                    parent.display()
                )),
            );
        }
    }

    if let Err(e) = fs::write(dest_path, bytes_to_write) {
        return (
            outcome,
            Some(format!("Failed to write {}: {e}", dest_path.display())),
        );
    }

    (outcome, None)
}

/// Cleans all STEP files defined in `options`, invoking `on_progress` after each file.
pub fn clean_step<F>(options: &CleanStepOptions, mut on_progress: F) -> CleanStepSummary
where
    F: FnMut(usize, usize, &CleanStepItemDetail),
{
    let files = collect_step_files(&options.input, options.recursive);
    let total_files = files.len();
    let mut summary = CleanStepSummary {
        total_files,
        ..Default::default()
    };

    for (index, src_path) in files.into_iter().enumerate() {
        let dest_path = compute_dest_path(&options.input, &src_path, &options.output);
        let (outcome, error) = clean_single_step_file(&src_path, &dest_path);

        match outcome {
            LogoOutcome::Removed(removal) => {
                summary.cleaned_files_count += 1;
                summary.cleaned_logos_count += removal.logos;
                summary.cleaned_count += removal.logos;
                summary.cleaned_pieces += removal.pieces;
                summary.cleaned_faces += removal.faces;
            }
            LogoOutcome::NotFound => {
                summary.not_found_count += 1;
            }
            LogoOutcome::Kept => {
                summary.kept_count += 1;
            }
            LogoOutcome::Unreadable => {
                summary.unreadable_count += 1;
            }
        }

        if error.is_some() {
            summary.error_count += 1;
        }

        let detail = CleanStepItemDetail {
            src_path,
            dest_path,
            outcome,
            error,
        };

        on_progress(index + 1, total_files, &detail);
        summary.details.push(detail);
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_step_file() {
        assert!(is_step_file(Path::new("c:/models/chip.step")));
        assert!(is_step_file(Path::new("c:/models/chip.STEP")));
        assert!(is_step_file(Path::new("c:/models/chip.stp")));
        assert!(is_step_file(Path::new("c:/models/chip.STP")));
        assert!(!is_step_file(Path::new("c:/models/chip.txt")));
        assert!(!is_step_file(Path::new("c:/models/chip.kicad_mod")));
    }

    #[test]
    fn test_compute_dest_path() {
        let input_dir = Path::new("D:/models");
        let output_dir = Path::new("D:/cleaned");

        // Subdirectory preservation
        let file1 = Path::new("D:/models/ic/sop8.step");
        assert_eq!(
            compute_dest_path(input_dir, file1, output_dir),
            PathBuf::from("D:/cleaned/ic/sop8.step")
        );

        // Root file
        let file2 = Path::new("D:/models/res.stp");
        assert_eq!(
            compute_dest_path(input_dir, file2, output_dir),
            PathBuf::from("D:/cleaned/res.stp")
        );

        // Single file input
        let single_file = Path::new("D:/models/res.stp");
        assert_eq!(
            compute_dest_path(single_file, single_file, output_dir),
            PathBuf::from("D:/cleaned/res.stp")
        );
    }

    #[test]
    fn test_clean_step_batch() {
        let temp_dir = std::env::temp_dir().join(format!("seex_clean_test_{}", std::process::id()));
        let input_dir = temp_dir.join("input");
        let output_dir = temp_dir.join("output");

        let _ = fs::create_dir_all(input_dir.join("sub"));
        let _ = fs::create_dir_all(&output_dir);

        // Create dummy STEP files
        let step_content = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('test'),'2;1');\nFILE_NAME('test.step','2024',(''),(''),'','','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
        fs::write(input_dir.join("root.step"), step_content).unwrap();
        fs::write(input_dir.join("sub").join("nested.stp"), step_content).unwrap();
        fs::write(input_dir.join("sub").join("ignore.txt"), b"not a step").unwrap();

        let options = CleanStepOptions {
            input: input_dir.clone(),
            output: output_dir.clone(),
            recursive: true,
        };

        let mut processed = 0;
        let summary = clean_step(&options, |_curr, _tot, _item| {
            processed += 1;
        });

        assert_eq!(processed, 2);
        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.not_found_count, 2);
        assert_eq!(summary.error_count, 0);

        // Check output files exist
        assert!(output_dir.join("root.step").exists());
        assert!(output_dir.join("sub").join("nested.stp").exists());
        assert!(!output_dir.join("sub").join("ignore.txt").exists());

        // Test overwrite
        let summary2 = clean_step(&options, |_, _, _| {});
        assert_eq!(summary2.total_files, 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_clean_step_summary_stats() {
        let mut summary = CleanStepSummary::default();
        let removal = crate::step_logo::LogoRemoval {
            logos: 3,
            pieces: 36,
            faces: 84,
        };
        let outcome = LogoOutcome::Removed(removal);
        if let LogoOutcome::Removed(rem) = outcome {
            summary.cleaned_files_count += 1;
            summary.cleaned_logos_count += rem.logos;
            summary.cleaned_count += rem.logos;
            summary.cleaned_pieces += rem.pieces;
            summary.cleaned_faces += rem.faces;
        }

        assert_eq!(summary.cleaned_files_count, 1);
        assert_eq!(summary.cleaned_logos_count, 3);
        assert_eq!(summary.cleaned_count, 3);
        assert_eq!(summary.cleaned_pieces, 36);
        assert_eq!(summary.cleaned_faces, 84);
    }
}

