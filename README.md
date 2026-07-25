# n-quads

[![Crate](https://img.shields.io/crates/v/n-quads.svg?style=flat-square)](https://crates.io/crates/n-quads)
[![Docs](https://img.shields.io/docsrs/n-quads?style=flat-square)](https://docs.rs/n-quads)
[![MSRV](https://img.shields.io/crates/msrv/n-quads?style=flat-square)](https://crates.io/crates/n-quads)
[![License](https://img.shields.io/crates/l/n-quads.svg?style=flat-square)](#license)

A [W3C Recommendation](https://www.w3.org/TR/n-quads/) compliant [N-Quads](https://www.w3.org/TR/n-quads/) parser for Rust, built on [`rdfx`](https://crates.io/crates/rdfx) terms — and it tells you where every quad came from.

```rust
let source = r#"<http://example.com/#a> <http://example.com/#b> "value"@en ."#;

let (document, code_map) = n_quads::document_from_str(source)?;
assert_eq!(document.len(), 1);

// Span of the whole quad, then of its subject, predicate and object.
let (quad_span, components) = code_map[0];
assert_eq!(&source[quad_span.range()], source);
assert_eq!(&source[components.0.range()], "<http://example.com/#a>");
assert_eq!(&source[components.2.range()], r#""value"@en"#);
```

---

## Why this fork

Fork of [`nquads-syntax`](https://crates.io/crates/nquads-syntax) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/nquads-syntax). Grammar and semantics are unchanged; this fork swaps the stack underneath:

- Terms come from [`rdfx`](https://crates.io/crates/rdfx) (fork of `rdf-types`) and IRIs from [`iri-rs`](https://crates.io/crates/iri-rs) (fork of `iref`), which is the point of the fork.
- `langtag` 0.4 → 1.1.
- Rust 2024 edition, MSRV 1.87.
- Renamed crate: `nquads-syntax` → `n-quads`.

Upstream went to `rdf-syntax` 1.0 for its own 0.20.0. If you are not otherwise invested in the `rdfx` / `iri-rs` stack, [upstream](https://crates.io/crates/nquads-syntax) is likely the crate you want.

Credit and history preserved — see [Attribution](#attribution).

## Install

```sh
cargo add n-quads
```

## Code maps

Parsing returns a `CodeMap` alongside the document: a `Vec` parallel to the quads themselves, so `code_map[i]` describes `document[i]`. Each entry is a pair — the span of the whole statement including its terminating `.`, and a `GeneralizedQuad<Span>` holding one span per component (subject, predicate, object, and graph label when present).

Spans live outside the data rather than inside it, so the quads are plain `rdfx` values: cheap to clone, compare and hash, and usable anywhere `rdfx` terms are expected. Drop the code map when you no longer need to point back at the source.

## Generalized RDF

`grdf_document_from_str` parses the generalized grammar, which lifts the restriction on which term kinds may appear where — any term may be a subject, predicate, object or graph label:

```rust
let (document, _) = n_quads::grdf_document_from_str(
    r#"_:s "literal predicate" <http://example.com/#o> ."#
)?;
```

## RDF version

The grammar implemented here is the RDF 1.1 one. `rdfx` models RDF 1.2, so its term types *can* represent triple terms and directional language strings, but this parser never produces them — the three RDF 1.2 syntax additions are rejected:

| Syntax | Example | Status |
| ------ | ------- | ------ |
| Triple terms | `<<( :a :b :c )>>` | rejected |
| Directional language strings | `"x"@en--ltr` | rejected |
| Reifiers | `:s :p "x" ~ :r` | rejected |

## Other input encodings

`document_from_str` covers the common case. For anything else — a fallible byte stream, UTF-16, a custom decoder — use the `Parse` trait, which is implemented for both `Document` and `GrdfDocument`:

```rust
use n_quads::{Document, Parse};

let (document, code_map) = Document::parse_utf16_infallible(utf16_chars)?;
```

Spans are reported in the units of the source encoding, so they stay usable for pointing at the original bytes.

## MSRV

Rust 1.87 (edition 2024) — floor set by `langtag` 1.1.

## Attribution

Original crate: [`nquads-syntax`](https://crates.io/crates/nquads-syntax) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/nquads-syntax). Upstream commits are preserved in this repo's history under their original authorship, including the code-mapping rework this fork is built on. The fork's own contribution is the dependency swap and the packaging around it.

## License

Dual-licensed, same as upstream. Pick whichever fits:

- [Apache-2.0](LICENSE-APACHE)
- [MIT](LICENSE-MIT)
