//! `pdf2md` — convert a PDF to Markdown on the command line.
//!
//! ```text
//! pdf2md [OPTIONS] <INPUT.pdf> [OUTPUT.md]
//!
//!   --pages 2-5        only these pages (1-based, inclusive)
//!   --page-break none|rule|html   (default: rule)
//!   --page-comments    emit <!-- page N -->
//!   --max-heading N    deepest heading level to emit (default: 3)
//!   --no-dehyphenate   keep line-end hyphens
//!   --skip-errors      skip pages that fail to extract (reported on stderr)
//! ```
//!
//! Without OUTPUT the Markdown goes to stdout.

use std::io::Write;
use std::process::ExitCode;

use pdf::file::FileOptions;
use pdf2md::{Options, PageBreak};
use pdf_text::entry::Flow;

fn usage() -> ExitCode {
    eprintln!(
        "usage: pdf2md [--pages A-B] [--page-break none|rule|html] [--page-comments]\n\
         \x20             [--max-heading N] [--no-dehyphenate] <input.pdf> [output.md]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut options = Options::default();
    let mut pages: Option<(usize, usize)> = None;
    let mut skip_errors = false;
    let mut positional: Vec<String> = vec![];

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--pages" => {
                let Some(spec) = args.next() else { return usage() };
                match parse_pages(&spec) {
                    Some(p) => pages = Some(p),
                    None => {
                        eprintln!("pdf2md: bad page range {spec:?}");
                        return ExitCode::from(2);
                    }
                }
            }
            "--page-break" => {
                let Some(kind) = args.next() else { return usage() };
                options.page_break = match kind.as_str() {
                    "none" => PageBreak::None,
                    "rule" => PageBreak::Rule,
                    "html" => PageBreak::Html,
                    _ => {
                        eprintln!("pdf2md: bad --page-break {kind:?}");
                        return ExitCode::from(2);
                    }
                };
            }
            "--page-comments" => options.page_comments = true,
            "--max-heading" => {
                let Some(n) = args.next().and_then(|n| n.parse().ok()) else {
                    return usage();
                };
                options.max_heading_level = n;
            }
            "--no-dehyphenate" => options.dehyphenate = false,
            "--skip-errors" => skip_errors = true,
            _ if arg.starts_with('-') && arg != "-" => {
                eprintln!("pdf2md: unknown option {arg:?}");
                return usage();
            }
            _ => positional.push(arg),
        }
    }

    let Some(input) = positional.first() else {
        return usage();
    };

    let file = match FileOptions::cached().open(input) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("pdf2md: cannot open {input}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (first, last) = pages.unwrap_or((1, usize::MAX));
    pdf2md::prepare();
    let resolver = file.resolver();
    let mut flows = vec![];
    let mut document = pdf_text::text::Stats::new();
    for (nr, page) in file.pages().enumerate() {
        let nr = nr + 1;
        if nr > last {
            break;
        }
        if nr < first {
            continue;
        }
        let run = page
            .map_err(|e| e.to_string())
            .and_then(|page| pdf_text::run_with_stats(&file, &page, &resolver, &mut document).map_err(|e| e.to_string()));
        match run {
            Ok(flow) => flows.push(flow),
            Err(e) if skip_errors => {
                // Leave an empty flow so the page numbering stays right.
                eprintln!("pdf2md: page {nr} skipped: {e}");
                flows.push(Flow::new());
            }
            Err(e) => {
                eprintln!("pdf2md: page {nr}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let markdown = pdf2md::flows_to_markdown(&flows, &options);

    match positional.get(1) {
        Some(output) => {
            if let Err(e) = std::fs::write(output, &markdown) {
                eprintln!("pdf2md: cannot write {output}: {e}");
                return ExitCode::FAILURE;
            }
        }
        None => {
            if let Err(e) = std::io::stdout().write_all(markdown.as_bytes()) {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("pdf2md: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn parse_pages(spec: &str) -> Option<(usize, usize)> {
    let (start, end) = spec.split_once('-').unwrap_or((spec, spec));
    let start: usize = start.trim().parse().ok()?;
    let end: usize = if end.trim().is_empty() {
        usize::MAX
    } else {
        end.trim().parse().ok()?
    };
    if start == 0 || end < start {
        return None;
    }
    Some((start, end))
}
