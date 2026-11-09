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

## Building elsewhere (and on macOS)

Nothing here is platform-specific: no `cfg(unix)`, no C dependencies, no
system libraries. Two things are about *reaching* the sources, not compiling:

* The four crates come from Sebastian's forks over **ssh** (`pdf`, `pdf_text`,
  `pdf_render`, and through them `font`). The machine needs a key GitHub
  accepts (`ssh -T git@github.com`).
* `.cargo/config.toml` sets `net.git-fetch-with-cli = true`. Cargo's built-in
  git client has no ssh-agent, so on macOS (key in the agent, or a passphrase)
  it cannot fetch `ssh://git@github.com/...` and the build stops at
  "failed to load source"; the `git` command handles the same URL fine.

On macOS that is usually the whole story. `cargo build` also needs a linker —
if Xcode's Command Line Tools are missing, `xcode-select --install` first.
`$STANDARD_FONTS` (see above) is a *runtime* requirement, not a build one.

## Known gaps

* No bold/italic: `pdf_text` reports bold in its run classification, but the
  classification is not exported per word.
* A page's tables are whatever `pdf_text`'s line splitter thinks a table is; a
  formula with wide gaps can come out as a one-row table. Rows/columns are not
  re-checked against column x positions.
* No lists, links, footnotes or images.
* Text within a word can pick up a spurious space where glyph positions jump
  ("AMENDM ENTS").

