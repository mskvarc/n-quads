//! N-Quads parsing.
//!
//! N-Quads is a line-based, plain text format for encoding an RDF dataset.
//! This library provides a [W3C Recommendation](https://www.w3.org/TR/n-quads/)
//! compliant parser built on [`rdfx`] terms. Parsing also returns a
//! [`CodeMap`] recording the source span of every quad *and* of each of its
//! components, so a downstream validation error can be reported against the
//! exact byte range that caused it.
//!
//! Both the standard grammar and the generalized (gRDF) grammar are supported.
//! gRDF lifts the restrictions on which term kinds may appear where: any term
//! may be a subject, predicate, object or graph label.
//!
//! # RDF version
//!
//! The grammar implemented here is the RDF 1.1 one. [`rdfx`] models RDF 1.2,
//! so its term types can represent triple terms and directional language
//! strings, but this parser never produces them — the three RDF 1.2 syntax
//! additions are rejected:
//!
//! | Syntax | Example | Status |
//! | ------ | ------- | ------ |
//! | Triple terms | `<<( :a :b :c )>>` | rejected |
//! | Directional language strings | `"x"@en--ltr` | rejected |
//! | Reifiers | `:s :p "x" ~ :r` | rejected |
//!
//! # Usage
//!
//! ```
//! let source = r#"<http://example.com/#a> <http://example.com/#b> "value"@en ."#;
//!
//! let (document, code_map) = n_quads::document_from_str(source)?;
//! assert_eq!(document.len(), 1);
//!
//! // Span of the whole quad, then of its subject, predicate and object.
//! let (quad_span, components) = code_map[0];
//! assert_eq!(&source[quad_span.range()], source);
//! assert_eq!(&source[components.0.range()], "<http://example.com/#a>");
//! assert_eq!(&source[components.2.range()], r#""value"@en"#);
//! # Ok::<_, n_quads::Error>(())
//! ```
//!
//! For the generalized grammar, use [`grdf_document_from_str`]. For input that
//! is not already a `&str` — a fallible byte stream, UTF-16, a custom decoder
//! — use the [`Parse`] trait directly.
#![warn(missing_docs)]

pub mod lexing;
mod parsing;

pub use lexing::LexingError;
pub use parsing::*;
pub use rdfx::BlankIdBuf;
use rdfx::{GeneralizedQuad, LexicalQuad};

/// N-Quads document.
///
/// A document is just the list of its quads, in source order.
pub type Document = Vec<LexicalQuad>;

/// gRDF N-Quads document.
///
/// Like [`Document`], but parsed with the generalized grammar, where any term
/// may occupy any position of the quad.
pub type GrdfDocument = Vec<GeneralizedQuad>;

/// Parses an N-Quads document from a string.
///
/// Returns the quads along with the [`CodeMap`] describing where each one came
/// from.
pub fn document_from_str(string: &str) -> Result<(Document, CodeMap), parsing::Error> {
	Document::parse_str(string)
}

/// Parses a generalized (gRDF) N-Quads document from a string.
///
/// Returns the quads along with the [`CodeMap`] describing where each one came
/// from.
pub fn grdf_document_from_str(string: &str) -> Result<(GrdfDocument, CodeMap), parsing::Error> {
	GrdfDocument::parse_str(string)
}
