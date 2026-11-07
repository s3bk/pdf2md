//! Temporary: dump the outline (bookmark) tree as a markdown table of contents.
use std::collections::HashMap;

use pdf::file::FileOptions;
use pdf::object::{Dest, MaybeNamedDest, Object, OutlineItem, PlainRef, Ref, Resolve};
use pdf::primitive::Primitive;

const MAX_DEPTH: usize = 12;

fn main() {
    let path = std::env::args().nth(1).expect("input pdf");
    let file = FileOptions::cached().open(&path).expect("open");

    // page object -> 1-based page number
    let mut page_nr: HashMap<PlainRef, usize> = HashMap::new();
    for (i, page) in file.pages().enumerate() {
        if let Ok(page) = page {
            page_nr.insert(page.get_ref(), i + 1);
        }
    }

    // named destinations
    let mut named: HashMap<Vec<u8>, Option<PlainRef>> = HashMap::new();
    if let Some(names) = file.trailer.root.names.as_ref() {
        if let Some(dests) = names.data().dests.as_ref() {
            let _ = dests.walk(&file, &mut |name, dest: &Option<Dest>| {
                named.insert(name.as_bytes().to_vec(), dest.as_ref().and_then(|d| d.page).map(|p| p.get_inner()));
            });
        }
    }

    let mut out = String::new();
    let mut count = 0usize;
    let mut depth_bad = 0usize;
    let entries = walk(&file, file.trailer.root.outlines.as_ref().and_then(|o| o.first), 0, &page_nr, &named, &mut out, &mut count, &mut depth_bad);

    eprintln!("pages={} entries={count} unresolved={entries} depth-limit={depth_bad}", page_nr.len());
    match std::env::args().nth(2) {
        Some(out_path) => {
            if let Err(e) = std::fs::write(&out_path, out) {
                eprintln!("cannot write {out_path}: {e}");
            }
        }
        None => print!("{out}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn walk<R: Resolve>(
    r: &R,
    mut item: Option<Ref<OutlineItem>>,
    depth: usize,
    page_nr: &HashMap<PlainRef, usize>,
    named: &HashMap<Vec<u8>, Option<PlainRef>>,
    out: &mut String,
    count: &mut usize,
    depth_bad: &mut usize,
) -> usize {
    let mut unresolved = 0;
    if depth > MAX_DEPTH {
        *depth_bad += 1;
        return unresolved;
    }
    while let Some(reference) = item {
        let Ok(it) = r.get(reference) else { break };
        *count += 1;
        let title = it
            .title
            .as_ref()
            .map(|t| t.to_string_lossy())
            .unwrap_or_default();
        let title = title.trim().replace(['\n', '\r'], " ");
        let target = dest_ref(&it, r, named).and_then(|p| page_nr.get(&p).copied());
        if target.is_none() && !title.is_empty() {
            unresolved += 1;
        }
        out.push_str(&"  ".repeat(depth.saturating_sub(1)));
        if depth == 0 {
            out.push_str("# ");
        } else {
            out.push_str("- ");
        }
        out.push_str(&title);
        match target {
            Some(n) => out.push_str(&format!("  ·  {n}\n")),
            None => out.push('\n'),
        }
        unresolved += walk(r, it.first, depth + 1, page_nr, named, out, count, depth_bad);
        item = it.next;
    }
    unresolved
}

fn dest_ref<R: Resolve>(item: &OutlineItem, r: &R, named: &HashMap<Vec<u8>, Option<PlainRef>>) -> Option<PlainRef> {
    if let Some(d) = &item.action {
        if let Some(d) = &d.d {
            return named_dest(d, named);
        }
    }
    if let Some(p) = &item.dest {
        if let Ok(d) = MaybeNamedDest::from_primitive(p.clone(), r) {
            return named_dest(&d, named);
        }
        // An /XYZ array the typed parser dislikes still names a page.
        if let Ok(Primitive::Array(a)) = p.clone().resolve(r) {
            if let Primitive::Reference(page) = a.first()? {
                return Some(*page);
            }
        }
    }
    None
}

fn named_dest(d: &MaybeNamedDest, named: &HashMap<Vec<u8>, Option<PlainRef>>) -> Option<PlainRef> {
    match d {
        MaybeNamedDest::Direct(d) => d.page.map(|p| p.get_inner()),
        MaybeNamedDest::Named(s) => named.get(s.as_bytes()).copied().flatten(),
    }
}
