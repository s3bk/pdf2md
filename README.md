# pdf2md

PDF → Markdown, built on [`pdf_text`](https://github.com/pdf-rs/pdf_text) (page
interpretation, line/paragraph/table segmentation) and
[`pdf_render`](https://github.com/pdf-rs/pdf_render) (tracing a page's content
streams) from the [pdf-rs](https://github.com/pdf-rs/pdf) family.

Still early: the output is plain Markdown with headings, paragraphs and pipe
tables. No bold/italic (font names are not mapped to emphasis yet), no lists, no
images, no links.

## Library

```rust
let file = pdf::file::FileOptions::cached().open("input.pdf")?;
let md = pdf2md::to_markdown(&file, &pdf2md::Options::default())?;
std::fs::write("out.md", md)?;
```

* `to_markdown(&file, &options)` — the whole document as one string.
* `to_flows(&file)` — the extracted structure ([`pdf_text::entry::Flow`] per
  page), if you want to do your own rendering.
* `page_to_markdown(&file, &page, &resolver, &options)` — a single page.
* `flows_to_markdown(&pages, &options)` — render pages you already extracted.
* `prepare()` — sets `$STANDARD_FONTS` to a usable substitute-font directory if
  the caller did not; called automatically by the functions above.

### Options

| field | default | meaning |
|---|---|---|
| `page_break` | `Rule` | `---`, an HTML page-break div, or nothing between pages |
| `page_comments` | `false` | emit `<!-- page N -->` before each page |
| `max_heading_level` | `3` | heading levels are clamped to this |
| `dehyphenate` | `true` | join words split by a line-end hyphen |

### How runs become Markdown

* `Header` → `#`/`##`/`###`, chosen by the run's median glyph height against
  the document's median body-text height (≥1.7× → `#`, ≥1.3× → `##`, ≥1.1× →
  `###`), clamped to `max_heading_level`.
* `Paragraph` starts a paragraph; `ParagraphContinuation` continues the
  previous one with a space.
* Line-end hyphens are removed when the line break split a word
  (`hyphen-` + `ation` → `hyphenation`), but not after a dash (`--`) or a
  capital.
* `Table` becomes a pipe table; the first row is the header. A "table" with
  only one row or one column is really a formula or a stray line, so it is
  emitted as text instead.

## Binary

```text
pdf2md [--pages 2-5] [--page-break none|rule|html] [--page-comments]
       [--max-heading N] [--no-dehyphenate] <input.pdf> [output.md]
```

Without `output.md` the Markdown goes to stdout.

## Requirements

`pdf_render` needs substitute fonts for the standard 14 (Helvetica, Times,
Courier, Symbol, ZapfDingbats) in `$STANDARD_FONTS` — see
[pdf_render's README](https://github.com/pdf-rs/pdf_render#fonts). `prepare()`
looks for `standard_fonts/`, `../pdf_render/standard_fonts` and a local
development checkout, in that order, and does nothing if the variable is set.

## Development

The local checkouts of `pdf` and `pdf_render` are pinned in `Cargo.toml` via
`[patch]`, because `pdf_text` needs a `pdf_render` that has caught up with the
`pdf` API.
