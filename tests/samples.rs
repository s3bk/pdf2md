//! Real documents: extract a few sample PDFs and write the Markdown next to
//! the test binary so it can be inspected.
//!
//! The samples live in the pdf-rs checkout; when that is missing the test is
//! skipped rather than failed.

use std::path::{Path, PathBuf};

use pdf::file::FileOptions;

fn samples_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var("PDF2MD_SAMPLES")
            .unwrap_or_else(|_| "/home/hematite/task-clones/pdf/files".to_string()),
    );
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn out_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/md");
    std::fs::create_dir_all(&dir).expect("cannot create output dir");
    dir
}

fn convert(name: &str) -> Option<(String, PathBuf)> {
    let samples = samples_dir()?;
    let path = samples.join(name);
    if !path.is_file() {
        return None;
    }
    let file = FileOptions::cached()
        .open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let md = pdf2md::to_markdown(&file, &pdf2md::Options::default())
        .unwrap_or_else(|e| panic!("cannot convert {}: {e}", path.display()));
    let out = out_dir().join(format!("{name}.md"));
    std::fs::write(&out, &md).expect("cannot write markdown");
    Some((md, out))
}

#[test]
fn libreoffice_document() {
    let Some((md, path)) = convert("libreoffice.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    eprintln!("wrote {}", path.display());
    // The sample is three pages, one line each.
    assert!(md.contains("Contents of page 1"), "{md}");
    assert_eq!(md.matches("Contents of page").count(), 3, "{md}");
    assert!(!md.contains("�"), "replacement characters in output");
}

#[test]
fn xelatex_document() {
    let Some((md, path)) = convert("xelatex.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    eprintln!("wrote {}", path.display());
    assert!(md.len() > 500, "{} bytes only: {md}", md.len());
    assert!(md.contains("Problem 1"), "{md}");
    assert!(!md.contains("�"), "replacement characters in output");
}

#[test]
fn sample_document() {
    let Some((md, path)) = convert("pdf-sample.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    eprintln!("wrote {}", path.display());
    assert!(md.len() > 200, "{} bytes only: {md}", md.len());
    assert!(md.contains("Portable Document Format"), "{md}");
}

/// A few more documents, converted for inspection only: the interesting part
/// is that they do not panic and produce sane text.
#[test]
fn more_documents() {
    for name in ["ep.pdf", "ep2.pdf", "example.pdf", "xelatex-drawboard.pdf"] {
        let Some((md, path)) = convert(name) else {
            eprintln!("skipping {name}: not present");
            continue;
        };
        eprintln!("wrote {} ({} bytes)", path.display(), md.len());
        assert!(!md.trim().is_empty(), "{name} produced nothing");
        assert!(!md.contains('\0'), "{name} contains NUL");
    }
}
