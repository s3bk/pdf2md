//! PDF → Markdown.
//!
//! The heavy lifting (page interpretation, text spans, line/paragraph/table
//! segmentation) is done by [`pdf_text`]; this crate turns its [`Flow`] into
//! Markdown.
//!
//! ```no_run
//! let file = pdf::file::FileOptions::cached().open("input.pdf")?;
//! let md = pdf2md::to_markdown(&file, &pdf2md::Options::default())?;
//! std::fs::write("out.md", md)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt::Write;

use pdf::backend::Backend;
use pdf::file::CachedFile;
use pdf::object::{Page, Resolve};
use pdf::PdfError;
use pdf_text::entry::{Flow, Run, RunType, TableRun};

pub use pdf_text::entry::{Line, Word};

/// How the end of a PDF page is marked in the output.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PageBreak {
    /// Nothing at all: the page ends with a blank line like any paragraph.
    None,
    /// A thematic break (`---`), so a page boundary is visible in the text.
    Rule,
    /// A break that HTML/CSS renderers honour.
    Html,
}

/// Conversion settings.
#[derive(Clone, Debug)]
pub struct Options {
    pub page_break: PageBreak,
    /// Emit `<!-- page N -->` before each page's content.
    pub page_comments: bool,
    /// The page number of the first flow handed to [`flows_to_markdown`], so
    /// that the comments name the pages of the file even when only a slice of
    /// it was converted (`--pages 28-32`). Only used by `page_comments`.
    pub first_page: usize,
    /// Never emit a heading deeper than this level; deeper ones are clamped.
    pub max_heading_level: u8,
    /// Join words split by a line-end hyphen ("exam-" / "ple").
    pub dehyphenate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            page_break: PageBreak::Rule,
            page_comments: false,
            first_page: 1,
            max_heading_level: 3,
            dehyphenate: true,
        }
    }
}

/// Convert every page of `file` to one Markdown document.
pub fn to_markdown<B: Backend>(file: &CachedFile<B>, options: &Options) -> Result<String, PdfError> {
    let pages = to_flows(file)?;
    Ok(flows_to_markdown(&pages, options))
}
/// Convert a PDF on disk, optionally only the pages listed in `pages`
/// (1-based; `None` = all, unsorted and repeated numbers are fine).
///
/// Returns the document's page count and the Markdown. This is the entry point
/// for callers that hold a path rather than a `pdf::file::File` — an agent
/// tool, say — so they do not have to depend on `pdf` themselves.
pub fn to_markdown_file(
    path: impl AsRef<std::path::Path>,
    options: &Options,
    pages: Option<&[u32]>,
) -> Result<(usize, String), PdfError> {
    let file = pdf::file::FileOptions::cached().open(path)?;
    let flows = to_flows(&file)?;
    let total = flows.len();
    let Some(pages) = pages else {
        return Ok((total, flows_to_markdown(&flows, options)));
    };
    // Heading levels are judged against the body size of the whole document,
    // so the yardstick is measured over every page, not the chosen ones.
    let body = body_height(&flows);
    let mut out = String::new();
    let mut first = true;
    for nr in pages.iter().copied().collect::<std::collections::BTreeSet<_>>() {
        let Some(flow) = nr.checked_sub(1).and_then(|i| flows.get(i as usize)) else {
            continue;
        };
        if !first {
            page_break_between(&mut out, options.page_break);
        }
        first = false;
        if options.page_comments {
            let _ = writeln!(out, "<!-- page {nr} -->\n");
        }
        flow_to_markdown(&mut out, flow, body, options);
    }
    let mut s = out.trim_end().to_string();
    if !s.is_empty() {
        s.push('\n');
    }
    Ok((total, s))
}

/// The break `flows_to_markdown` puts between two pages.
fn page_break_between(out: &mut String, kind: PageBreak) {
    if let PageBreak::None = kind {
        return;
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
    }
    match kind {
        PageBreak::Rule => out.push_str("---\n\n"),
        PageBreak::Html => out.push_str("<div style=\"page-break-after: always\"></div>\n\n"),
        PageBreak::None => {}
    }
}

/// Convert a single page.
pub fn page_to_markdown<B: Backend>(
    file: &CachedFile<B>,
    page: &Page,
    resolve: &impl Resolve,
    options: &Options,
) -> Result<String, PdfError> {
    prepare();
    let flow = pdf_text::run(file, page, resolve)?;
    Ok(flows_to_markdown(std::slice::from_ref(&flow), options))
}

/// Extract the text structure of every page, without rendering Markdown.
pub fn to_flows<B: Backend>(file: &CachedFile<B>) -> Result<Vec<Flow>, PdfError> {
    prepare();
    let resolver = file.resolver();
    let mut pages = vec![];
    let mut document = pdf_text::text::Stats::new();
    let cache = pdf_text::new_trace_cache();
    for page in file.pages() {
        let page = page?;
        pages.push(pdf_text::run_cached(file, &page, &resolver, &mut document, &cache)?);
    }
    Ok(pages)
}

/// `pdf_render` refuses to render a page that uses one of the standard 14
/// fonts unless `$STANDARD_FONTS` points at a directory holding substitutes
/// (`fonts.json` plus the font files). Text extraction needs none of the
/// outlines, but it goes through the same font loader, so point the variable
/// at a usable directory when the caller has not. Called for you by
/// [`to_markdown`] and [`to_flows`]; call it directly before running
/// [`pdf_text::run`] yourself.
pub fn prepare() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if std::env::var_os("STANDARD_FONTS").is_some() {
            return;
        }
        let candidates = [
            // compile-time override
            option_env!("PDF2MD_STANDARD_FONTS").unwrap_or(""),
            "standard_fonts",
            "../pdf_render/standard_fonts",
            // the local pdf_render checkout this crate is developed against
            "/home/hematite/task-clones/pdf_render/standard_fonts",
        ];
        for dir in candidates {
            if dir.is_empty() {
                continue;
            }
            let dir = std::path::Path::new(dir);
            if dir.join("fonts.json").is_file() {
                std::env::set_var("STANDARD_FONTS", dir);
                return;
            }
        }
    });
}

/// Render already extracted pages as Markdown.
pub fn flows_to_markdown(pages: &[Flow], options: &Options) -> String {
    let body = body_height(pages);
    let mut out = String::new();
    for (i, flow) in pages.iter().enumerate() {
        if i > 0 {
            page_break_between(&mut out, options.page_break);
        }
        if options.page_comments {
            let _ = writeln!(out, "<!-- page {} -->\n", i + options.first_page);
        }
        flow_to_markdown(&mut out, flow, body, options);
    }
    let mut s = out.trim_end().to_string();
    s.push('\n');
    s
}

/// The most common glyph height of body text: the yardstick for heading sizes.
/// `0.0` when the document has no paragraphs at all.
fn body_height(pages: &[Flow]) -> f32 {
    let mut heights: Vec<f32> = pages
        .iter()
        .flat_map(|flow| flow.runs.iter())
        .filter(|run| matches!(run.kind, RunType::Paragraph | RunType::ParagraphContinuation))
        .flat_map(|run| run.lines.iter())
        .flat_map(|line| line.words.iter())
        .map(|w| w.rect.h)
        .filter(|h| h.is_finite() && *h > 0.0)
        .collect();
    if heights.is_empty() {
        return 0.0;
    }
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    heights[heights.len() / 2]
}

fn flow_to_markdown(out: &mut String, flow: &Flow, body: f32, options: &Options) {
    let mut para = String::new();

    fn flush(para: &mut String, out: &mut String) {
        let text = para.trim();
        if !text.is_empty() {
            out.push_str(text);
            out.push_str("\n\n");
        }
        para.clear();
    }

    for run in &flow.runs {
        match &run.kind {
            RunType::Paragraph => {
                flush(&mut para, out);
                para = run_text(run, options);
            }
            RunType::ParagraphContinuation => {
                let text = run_text(run, options);
                if text.is_empty() {
                    continue;
                }
                if para.is_empty() {
                    para = text;
                } else {
                    para.push(' ');
                    para.push_str(text.trim());
                }
            }
            RunType::Header => {
                flush(&mut para, out);
                let text = run_text(run, options);
                if text.is_empty() {
                    continue;
                }
                let level = heading_level(run, body, options.max_heading_level);
                for _ in 0..level {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(text.trim());
                out.push_str("\n\n");
            }
            RunType::Table(table) => {
                flush(&mut para, out);
                // The line splitter calls anything with wide gaps a table;
                // that includes formulas and stray lines, which render better
                // as text. A real table needs a second row and a second
                // column.
                let rows = table_rows(table);
                if rows.len() < 2 || table.columns < 2 {
                    let text = table_text(table);
                    if !text.is_empty() {
                        out.push_str(&text);
                        out.push_str("\n\n");
                    }
                } else {
                    write_table(out, table);
                }
            }
            RunType::Cell => {}
        }
    }
    flush(&mut para, out);
}

fn heading_level(run: &Run, body: f32, max: u8) -> u8 {
    let height = median_height(run);
    let level = if body <= 0.0 || !height.is_finite() {
        1
    } else {
        let ratio = height / body;
        if ratio >= 1.7 {
            1
        } else if ratio >= 1.3 {
            2
        } else if ratio >= 1.1 {
            3
        } else {
            4
        }
    };
    level.min(max.max(1))
}

fn median_height(run: &Run) -> f32 {
    let mut heights: Vec<f32> = run
        .lines
        .iter()
        .flat_map(|l| l.words.iter())
        .map(|w| w.rect.h)
        .filter(|h| h.is_finite() && *h > 0.0)
        .collect();
    if heights.is_empty() {
        return 0.0;
    }
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    heights[heights.len() / 2]
}

/// The text of a run: its lines joined, with line-end hyphens undone.
pub fn run_text(run: &Run, options: &Options) -> String {
    let mut s = String::new();
    for line in &run.lines {
        let text = words_text(&line.words);
        if text.is_empty() {
            continue;
        }
        if s.is_empty() {
            s.push_str(&text);
        } else if options.dehyphenate && ends_with_hyphenation(&s) {
            s.pop();
            s.push_str(&text);
        } else {
            s.push(' ');
            s.push_str(&text);
        }
    }
    s
}

fn ends_with_hyphenation(s: &str) -> bool {
    // A hyphen inside a line ("well- known") is not hyphenation, and a line
    // ending in "--" is a dash; a single trailing hyphen after a lowercase
    // letter is.
    if !s.ends_with('-') || s.ends_with("--") {
        return false;
    }
    let before = s[..s.len() - 1].chars().next_back();
    matches!(before, Some(c) if c.is_lowercase())
}

fn words_text(words: &[Word]) -> String {
    let mut s = String::new();
    for word in words {
        let text = word.text.trim();
        if text.is_empty() {
            continue;
        }
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(text);
    }
    s
}

/// A table's cells as escaped strings, row by row, with rows that hold no
/// text at all dropped and every row padded to `columns`.
fn table_rows(table: &TableRun) -> Vec<Vec<String>> {
    let columns = table.columns;
    let mut rows: Vec<Vec<String>> = vec![];
    for row in &table.rows {
        let mut cells: Vec<String> = (0..columns)
            .map(|c| {
                row.get(c)
                    .and_then(|cell| cell.as_ref())
                    .map(|cell| escape_cell(&cell.text))
                    .unwrap_or_default()
            })
            .collect();
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        cells.resize(columns, String::new());
        rows.push(cells);
    }
    rows
}

fn table_text(table: &TableRun) -> String {    let mut s = String::new();
    for row in &table.rows {
        for cell in row.iter().flatten() {
            let text = cell.text.trim();
            if text.is_empty() {
                continue;
            }
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(text);
        }
    }
    s
}

fn write_table(out: &mut String, table: &TableRun) {
    let columns = table.columns;
    if columns == 0 {
        return;
    }
    let rows = table_rows(table);
    if rows.is_empty() {
        return;
    }

    write_row(out, &rows[0]);
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &rows[1..] {
        write_row(out, row);
    }
    out.push('\n');
}

fn write_row(out: &mut String, cells: &[String]) {
    out.push('|');
    for cell in cells {
        out.push(' ');
        out.push_str(cell);
        out.push_str(" |");
    }
    out.push('\n');
}

fn escape_cell(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    let mut space = false;
    for c in text.trim().chars() {
        match c {
            '|' => {
                if space && !s.is_empty() {
                    s.push(' ');
                }
                space = false;
                s.push_str("\\|");
            }
            c if c.is_whitespace() => space = true,
            c => {
                if space && !s.is_empty() {
                    s.push(' ');
                }
                space = false;
                s.push(c);
            }
        }
    }
    s
}
