//! `to_markdown_file`: the path-in entry point, including page subsets.
//!
//! The samples live in the pdf-rs checkout; when that is missing the test is
//! skipped rather than failed.

use std::path::PathBuf;

use pdf2md::{Options, PageBreak};

fn sample(name: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(
        std::env::var("PDF2MD_SAMPLES")
            .unwrap_or_else(|_| "/home/hematite/task-clones/pdf/files".to_string()),
    );
    let path = dir.join(name);
    path.is_file().then_some(path)
}

fn comments() -> Options {
    Options {
        page_comments: true,
        page_break: PageBreak::None,
        ..Options::default()
    }
}

#[test]
fn every_page_of_the_file() {
    let Some(path) = sample("libreoffice.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    let (count, md) =
        pdf2md::to_markdown_file(&path, &Options::default(), None).expect("cannot convert");
    assert_eq!(count, 3, "page count: {md}");
    assert_eq!(md.matches("Contents of page").count(), 3, "{md}");
}

#[test]
fn a_subset_names_the_pages_of_the_file() {
    let Some(path) = sample("libreoffice.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    // The count is the document's, not the request's: the table of contents
    // wants to know how long the file is whatever page was asked for.
    let (count, md) =
        pdf2md::to_markdown_file(&path, &comments(), Some(&[2])).expect("cannot convert");
    assert_eq!(count, 3);
    assert_eq!(
        md,
        "<!-- page 2 -->\n\nContents of page 2\n",
        "one page, named by its number in the file"
    );
}

#[test]
fn a_scattered_subset_comes_out_in_page_order() {
    let Some(path) = sample("libreoffice.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    let (_, md) = pdf2md::to_markdown_file(&path, &comments(), Some(&[3, 1, 1]))
        .expect("cannot convert");
    assert_eq!(
        md,
        "<!-- page 1 -->\n\nContents of page 1\n\n<!-- page 3 -->\n\nContents of page 3\n",
        "sorted, a repeated page read once, no break between them"
    );
}

#[test]
fn pages_past_the_end_are_dropped() {
    let Some(path) = sample("libreoffice.pdf") else {
        eprintln!("skipping: no samples");
        return;
    };
    let (_, md) =
        pdf2md::to_markdown_file(&path, &comments(), Some(&[0, 99])).expect("cannot convert");
    assert!(md.is_empty(), "{md:?}");
}

#[test]
fn a_file_that_is_not_a_pdf_is_an_error() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert!(pdf2md::to_markdown_file(&path, &Options::default(), None).is_err());
}
