//! Markdown rendering of hand-built `Flow`s: no PDF involved.

use pdf2md::{flows_to_markdown, Options, PageBreak};
use pdf_text::entry::{CellContent, Flow, Line, Run, RunType, TableRun};
use pdf_text::util::Rect;

fn word(text: &str, h: f32) -> pdf2md::Word {
    pdf2md::Word {
        text: text.to_string(),
        rect: Rect { x: 0.0, y: 0.0, w: 10.0, h },
    }
}

fn line(text: &str, h: f32) -> Line {
    Line {
        words: text.split_whitespace().map(|w| word(w, h)).collect(),
    }
}

fn run(kind: RunType, lines: &[&str], h: f32) -> Run {
    Run {
        kind,
        lines: lines.iter().map(|l| line(l, h)).collect(),
    }
}

fn flow(runs: Vec<Run>) -> Flow {
    Flow {
        lines: vec![],
        runs,
    }
}

fn md(pages: &[Flow]) -> String {
    flows_to_markdown(
        pages,
        &Options {
            page_break: PageBreak::None,
            ..Options::default()
        },
    )
}

#[test]
fn paragraphs_are_separated_and_continuations_joined() {
    let page = flow(vec![
        run(RunType::Paragraph, &["first", "paragraph"], 10.0),
        run(
            RunType::ParagraphContinuation,
            &["still", "the", "same"],
            10.0,
        ),
        run(RunType::Paragraph, &["second"], 10.0),
    ]);
    let out = md(&[page]);
    assert_eq!(out, "first paragraph still the same\n\nsecond\n");
}

#[test]
fn headings_are_sized_against_body_text() {
    let page = flow(vec![
        run(RunType::Header, &["Title"], 24.0),
        run(RunType::Header, &["Section"], 15.0),
        run(RunType::Paragraph, &["body text"], 10.0),
        run(RunType::Header, &["Small"], 10.5),
    ]);
    let out = md(&[page]);
    assert!(out.starts_with("# Title\n\n"), "{out}");
    assert!(out.contains("## Section\n\n"), "{out}");
    assert!(out.contains("### Small"), "{out}");
}

#[test]
fn headings_are_clamped_to_max_level() {
    let page = flow(vec![
        run(RunType::Paragraph, &["body"], 10.0),
        run(RunType::Header, &["Small"], 10.5),
    ]);
    let out = flows_to_markdown(
        &[page],
        &Options {
            max_heading_level: 2,
            page_break: PageBreak::None,
            ..Options::default()
        },
    );
    assert!(out.contains("## Small"), "{out}");
}

#[test]
fn tables_become_pipe_tables_with_a_header_row() {
    let cell = |text: &str| Some(CellContent {
        text: text.to_string(),
        rect: Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
    });
    let table = TableRun {
        columns: 3,
        rows: vec![
            vec![cell("name"), cell("qty"), cell("price")],
            vec![cell("bolt"), cell("4"), None],
            vec![None, None, None],
            vec![cell("nut | washer"), cell("1"), cell("0.50")],
        ],
    };
    let page = flow(vec![run(RunType::Table(table), &[], 10.0)]);
    let out = md(&[page]);
    assert_eq!(
        out,
        "| name | qty | price |\n| --- | --- | --- |\n| bolt | 4 |  |\n| nut \\| washer | 1 | 0.50 |\n"
    );
}

#[test]
fn hyphens_at_a_line_end_are_joined() {
    let page = flow(vec![run(
        RunType::Paragraph,
        &["hyphen-", "ation", "is", "one", "word"],
        10.0,
    )]);
    assert!(md(&[page]).contains("hyphenation is one word"));

    let page = flow(vec![run(RunType::Paragraph, &["well-", "known"], 10.0)]);
    let out = flows_to_markdown(
        &[page],
        &Options {
            dehyphenate: false,
            ..Options::default()
        },
    );
    assert!(out.contains("well- known"), "{out}");
}

#[test]
fn pages_are_separated() {
    let page = || flow(vec![run(RunType::Paragraph, &["text"], 10.0)]);
    let pages = [page(), page()];
    let out = flows_to_markdown(
        &pages,
        &Options {
            page_break: PageBreak::Rule,
            ..Options::default()
        },
    );
    assert_eq!(out, "text\n\n---\n\ntext\n");
}

#[test]
fn page_comments_are_emitted() {
    let page = flow(vec![run(RunType::Paragraph, &["text"], 10.0)]);
    let out = flows_to_markdown(
        &[page],
        &Options {
            page_comments: true,
            page_break: PageBreak::None,
            ..Options::default()
        },
    );
    assert_eq!(out, "<!-- page 1 -->\n\ntext\n");
}
