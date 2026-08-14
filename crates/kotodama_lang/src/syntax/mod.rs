//! Lossless Kotodama concrete syntax trees.
//!
//! The lossless lexer is the canonical V1 scanner shared by compilation,
//! formatting, and editor tooling. The recovering CST parser preserves every
//! input byte and inserts zero-width missing tokens for interactive use; the
//! compiler lowers the same significant token stream directly into its AST.
pub mod cst;
pub mod kind;
pub mod lexer;
pub mod parser;
pub use cst::{GreenElement, GreenNode, GreenToken, SyntaxTree};
pub use kind::SyntaxKind;
pub use lexer::{Lexed, lex};
pub use parser::{ParseOutput, ProgramParseOutput, parse, parse_program};
