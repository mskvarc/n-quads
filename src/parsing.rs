use crate::{
	Document, GrdfDocument,
	lexing::{Lexer, LexingError, Token},
};
use decoded_char::DecodedChar;
use iri_rs::IriBuf;
use locspan::{Span, Spanned};
use rdfx::{Datatype, GeneralizedQuad, Id, LexicalQuad, Literal, LiteralType, LocalTerm};
use std::fmt;

/// Parse error.
#[derive(Debug)]
pub enum Error<E = LexingError> {
	/// The token stream failed; typically a [`LexingError`].
	Lexer(E),

	/// Unexpected token, or unexpected end of input (`None`).
	Unexpected(Option<Token>, Span),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Unexpected(None, _) => write!(f, "unexpected end of file"),
			Self::Unexpected(Some(token), _) => write!(f, "unexpected {token}"),
			Self::Lexer(e) => e.fmt(f),
		}
	}
}

impl<E: 'static + std::error::Error> std::error::Error for Error<E> {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Lexer(e) => Some(e),
			_ => None,
		}
	}
}

/// N-Quads parser.
///
/// Wraps a token iterator — normally a [`Lexer`] — with the one-token
/// lookahead the grammar needs. Drive it through the [`Parse`] trait.
pub struct Parser<L> {
	lexer: L,
	position: usize,
	pending: Option<Token>,
}

impl<L> Parser<L> {
	/// Creates a new parser over the given token iterator.
	pub const fn new(lexer: L) -> Self {
		Self {
			lexer,
			position: 0,
			pending: None,
		}
	}
}

impl<L, E> Parser<L>
where
	L: Iterator<Item = Result<Token, E>>,
{
	fn next_token(&mut self) -> Result<Option<Token>, Error<E>> {
		let token = match self.pending.take() {
			Some(token) => Some(token),
			None => self.lexer.next().transpose().map_err(Error::Lexer)?,
		};

		if let Some(token) = &token {
			self.position = token.span().end
		}

		Ok(token)
	}

	#[allow(clippy::type_complexity)]
	fn peek(&mut self) -> Result<Option<&Token>, Error<E>> {
		if self.pending.is_none() {
			self.pending = self.lexer.next().transpose().map_err(Error::Lexer)?;
		}

		Ok(self.pending.as_ref())
	}

	#[allow(clippy::type_complexity)]
	fn parse_literal(
		&mut self,
		value: String,
		mut span: Span,
	) -> Result<(Literal, Span), Error<E>> {
		match self.peek()? {
			Some(Token::LangTag(_, _)) => {
				let Some(Token::LangTag(tag, tag_span)) = self.next_token()? else {
					unreachable!()
				};

				span.append(tag_span);

				Ok((Literal::new(value, LiteralType::LangString(tag)), span))
			}
			Some(Token::Carets(_)) => {
				self.next_token()?;

				match self.next_token()? {
					Some(Token::Iri(iri, iri_span)) => {
						span.append(iri_span);
						Ok((
							Literal::new(
								value,
								LiteralType::Any(unsafe { Datatype::new_unchecked(iri) }),
							),
							span,
						))
					}
					Some(token) => {
						let span = token.span();
						Err(Error::Unexpected(Some(token), span))
					}
					None => Err(Error::Unexpected(None, self.position.into())),
				}
			}
			_ => Ok((Literal::new(value, LiteralType::xsd_string()), span)),
		}
	}

	fn parse_iri(&mut self) -> Result<(IriBuf, Span), Error<E>> {
		match self.next_token()? {
			Some(Token::Iri(iri, span)) => Ok((iri, span)),
			Some(token) => {
				let span = token.span();
				Err(Error::Unexpected(Some(token), span))
			}
			None => Err(Error::Unexpected(None, self.position.into())),
		}
	}

	fn parse_id(&mut self) -> Result<(Id, Span), Error<E>> {
		match self.next_token()? {
			Some(Token::Iri(iri, span)) => Ok((Id::Iri(iri), span)),
			Some(Token::BlankNodeLabel(label, span)) => Ok((Id::BlankId(label), span)),
			Some(token) => {
				let span = token.span();
				Err(Error::Unexpected(Some(token), span))
			}
			None => Err(Error::Unexpected(None, self.position.into())),
		}
	}

	fn parse_id_opt(&mut self) -> Result<Option<(Id, Span)>, Error<E>> {
		match self.peek()? {
			Some(Token::Dot(_)) => Ok(None),
			_ => self.parse_id().map(Some),
		}
	}

	fn parse_term(&mut self) -> Result<(LocalTerm, Span), Error<E>> {
		match self.next_token()? {
			Some(Token::Iri(iri, span)) => Ok((LocalTerm::iri(iri), span)),
			Some(Token::BlankNodeLabel(label, span)) => Ok((LocalTerm::BlankId(label), span)),
			Some(Token::StringLiteral(value, span)) => {
				let (literal, span) = self.parse_literal(value, span)?;
				Ok((LocalTerm::literal(literal), span))
			}
			Some(token) => {
				let span = token.span();
				Err(Error::Unexpected(Some(token), span))
			}
			None => Err(Error::Unexpected(None, self.position.into())),
		}
	}

	fn parse_term_opt(&mut self) -> Result<Option<(LocalTerm, Span)>, Error<E>> {
		match self.peek()? {
			Some(Token::Dot(_)) => Ok(None),
			_ => self.parse_term().map(Some),
		}
	}

	fn parse_dot(&mut self) -> Result<Span, Error<E>> {
		match self.next_token()? {
			Some(Token::Dot(span)) => Ok(span),
			Some(token) => {
				let span = token.span();
				Err(Error::Unexpected(Some(token), span))
			}
			None => Err(Error::Unexpected(None, self.position.into())),
		}
	}

	fn parse_quad(&mut self) -> Result<(LexicalQuad, QuadSpans), Error<E>> {
		let (s, s_span) = self.parse_id()?;
		let (p, p_span) = self.parse_iri()?;
		let (o, o_span) = self.parse_term()?;
		let (g, g_span) = split_option(self.parse_id_opt()?);
		let dot_span = self.parse_dot()?;
		let span = s_span.union(dot_span);
		Ok((
			LexicalQuad::new(s, p, o, g),
			(span, GeneralizedQuad(s_span, p_span, o_span, g_span)),
		))
	}

	fn parse_grdf_quad(&mut self) -> Result<(GeneralizedQuad, QuadSpans), Error<E>> {
		let (s, s_span) = self.parse_term()?;
		let (p, p_span) = self.parse_term()?;
		let (o, o_span) = self.parse_term()?;
		let (g, g_span) = split_option(self.parse_term_opt()?);
		let dot_span = self.parse_dot()?;
		let span = s_span.union(dot_span);
		Ok((
			GeneralizedQuad(s, p, o, g),
			(span, GeneralizedQuad(s_span, p_span, o_span, g_span)),
		))
	}

	fn parse_document(&mut self) -> Result<(Vec<LexicalQuad>, CodeMap), Error<E>> {
		let mut quads = Vec::new();
		let mut spans = Vec::new();

		while self.peek()?.is_some() {
			let (quad, span) = self.parse_quad()?;
			quads.push(quad);
			spans.push(span);
		}

		Ok((quads, spans))
	}

	fn parse_grdf_document(&mut self) -> Result<(Vec<GeneralizedQuad>, CodeMap), Error<E>> {
		let mut quads = Vec::new();
		let mut spans = Vec::new();

		while self.peek()?.is_some() {
			let (quad, span) = self.parse_grdf_quad()?;
			quads.push(quad);
			spans.push(span);
		}

		Ok((quads, spans))
	}
}

fn split_option<A, B>(value: Option<(A, B)>) -> (Option<A>, Option<B>) {
	match value {
		Some((a, b)) => (Some(a), Some(b)),
		None => (None, None),
	}
}

/// Source spans of a single quad.
///
/// The first element covers the whole statement, terminating `.` included. The
/// second holds one span per component — subject, predicate, object, and graph
/// label when present.
pub type QuadSpans = (Span, GeneralizedQuad<Span>);

/// Source spans of every quad in a document, in source order.
///
/// Parallel to the document itself: `code_map[i]` describes `document[i]`.
pub type CodeMap = Vec<QuadSpans>;

/// N-Quads parsing methods.
///
/// Implemented for [`Document`](crate::Document) and
/// [`GrdfDocument`](crate::GrdfDocument), which select the standard and the
/// generalized grammar respectively. Every method returns the document
/// together with its [`CodeMap`].
///
/// # Example
///
/// ```
/// use n_quads::{Document, Parse};
///
/// let (document, _) = Document::parse_str("<http://a.example/s> <http://a.example/p> \"o\" .")?;
/// assert_eq!(document.len(), 1);
/// # Ok::<_, n_quads::Error>(())
/// ```
pub trait Parse: Sized {
	/// Parses a document from an existing [`Parser`].
	///
	/// This is the single method implementors must provide; the rest are
	/// wrappers that build the lexer and parser for you.
	#[allow(clippy::type_complexity)]
	fn parse_with<L, E>(parser: &mut Parser<L>) -> Result<(Self, CodeMap), Error<E>>
	where
		L: Iterator<Item = Result<Token, E>>;

	/// Parses a document from a fallible iterator of decoded characters.
	///
	/// [`DecodedChar`] carries the byte length of each character in the
	/// *source* encoding, so spans stay accurate for non-UTF-8 input.
	#[inline(always)]
	fn parse<C, E>(chars: C) -> Result<(Self, CodeMap), Error<LexingError<E>>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>,
	{
		let mut parser = Parser::new(Lexer::new(chars));
		Self::parse_with(&mut parser)
	}

	/// Parses a document from an infallible iterator of decoded characters.
	#[inline(always)]
	fn parse_infallible<C>(chars: C) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = DecodedChar>,
	{
		Self::parse(chars.map(Ok))
	}

	/// Parses a document from a fallible iterator of UTF-8 characters.
	#[inline(always)]
	fn parse_utf8<C, E>(chars: C) -> Result<(Self, CodeMap), Error<LexingError<E>>>
	where
		C: Iterator<Item = Result<char, E>>,
	{
		Self::parse(decoded_char::FallibleUtf8Decoded::new(chars))
	}

	/// Parses a document from an infallible iterator of UTF-8 characters.
	#[inline(always)]
	fn parse_utf8_infallible<C>(chars: C) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = char>,
	{
		Self::parse_infallible(decoded_char::Utf8Decoded::new(chars))
	}

	/// Parses a document from a fallible iterator of UTF-16 characters.
	///
	/// Spans are reported in UTF-16 code units.
	#[inline(always)]
	fn parse_utf16<C, E>(chars: C) -> Result<(Self, CodeMap), Error<LexingError<E>>>
	where
		C: Iterator<Item = Result<char, E>>,
	{
		Self::parse(decoded_char::FallibleUtf16Decoded::new(chars))
	}

	/// Parses a document from an infallible iterator of UTF-16 characters.
	///
	/// Spans are reported in UTF-16 code units.
	#[inline(always)]
	fn parse_utf16_infallible<C>(chars: C) -> Result<(Self, CodeMap), Error>
	where
		C: Iterator<Item = char>,
	{
		Self::parse_infallible(decoded_char::Utf16Decoded::new(chars))
	}

	/// Parses a document from a string.
	#[inline(always)]
	fn parse_str(string: &str) -> Result<(Self, CodeMap), Error> {
		Self::parse_utf8_infallible(string.chars())
	}
}

impl Parse for Document {
	fn parse_with<L, E>(parser: &mut Parser<L>) -> Result<(Self, CodeMap), Error<E>>
	where
		L: Iterator<Item = Result<Token, E>>,
	{
		parser.parse_document()
	}
}

impl Parse for GrdfDocument {
	fn parse_with<L, E>(parser: &mut Parser<L>) -> Result<(Self, CodeMap), Error<E>>
	where
		L: Iterator<Item = Result<Token, E>>,
	{
		parser.parse_grdf_document()
	}
}
