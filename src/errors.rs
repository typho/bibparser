use std::error;
use std::fmt;

use crate::lexer;

// Represents an error that happened during the lexing process
#[derive(Debug)]
pub(crate) enum LexingError<'e> {
    UnexpectedChar(char, &'e str, lexer::TokenInfo),
    UnexpectedEOF(String),
}

impl<'e> fmt::Display for LexingError<'e> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(unexp, action, info) => {
                if let Some(id) = &info.current_id {
                    write!(f, "unexpected character '{unexp}' while {action} at line {lineno} col {colno} in entry {entry}",
                        lineno=info.lineno + 1, colno=info.colno + 1, entry=id)?;
                } else if let Some(name) = &info.previous_name {
                    write!(f, "unexpected character '{unexp}' while {action} at line {lineno} col {colno} after entry {entry}",
                        lineno=info.lineno + 1, colno=info.colno + 1, entry=name)?;
                } else {
                    write!(f, "unexpected character '{unexp}' while {action} at line {lineno} col {colno}",
                        lineno=info.lineno + 1, colno=info.colno + 1)?;
                }
                if !info.current_line.trim().is_empty() {
                    write!(f, ">> {}", info.current_line)?;
                    write!(f, "   {:skip$}↑ here", skip = info.colno)?;
                }
                Ok(())
            }
            Self::UnexpectedEOF(action) => {
                write!(f, "unexpected end of file while {action}")
            }
        }
    }
}

impl<'e> error::Error for LexingError<'e> {}

#[derive(Debug)]
pub enum ParsingErrorKind {
    DuplicateName(String),
}

// Represents an error that happened during the parsing process.
#[derive(Debug)]
pub struct ParsingError {
    pub(crate) kind: ParsingErrorKind,
    pub(crate) info: lexer::TokenInfo,
}

impl<'e> fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParsingErrorKind::DuplicateName(name) => match &self.info.current_id {
                Some(id) => write!(f, "found duplicate name '{}' in entry '{}'", name, id),
                None => write!(f, "found duplicate name '{}'", name),
            },
        }
    }
}

impl<'e> error::Error for ParsingError {}
