use std::error;
use std::fmt;

use crate::lexer;

// Represents an error that happened during the lexing process
#[derive(Debug)]
pub(crate) enum LexingError {
    UnexpectedChar(char, &'static str, lexer::TokenInfo),
    UnexpectedEOF(String),
}

impl fmt::Display for LexingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(unexp, action, info) => {
                if let Some(id) = &info.current_id {
                    write!(f, "unexpected character '{unexp}' while {action} at line {lineno} col {colno} in entry {entry}",
                        lineno=info.lineno + 1, colno=info.colno + 1, entry=id)?;
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

impl LexingError {
    pub fn to_parsing_error(&self) -> ParsingError {
        match self {
            LexingError::UnexpectedChar(unexp, action, info)
                => ParsingError {
                    kind: ParsingErrorKind::UnexpectedText(unexp.to_string(), action.to_string()),
                    info: (*info).clone(),
                },
            LexingError::UnexpectedEOF(action)
                => ParsingError {
                    kind: ParsingErrorKind::UnexpectedEOF(action.to_string()),
                    info: lexer::TokenInfo{
                        lineno: usize::MAX,
                        colno: usize::MAX,
                        current_line: "".to_owned(),
                        current_id: None,
                    },
                },
        }
    }
}

impl error::Error for LexingError {}

#[derive(Debug)]
pub enum ParsingErrorKind {
    DuplicateName(String),
    UnexpectedText(String, String),
    UnexpectedEOF(String),
}

// Represents an error that happened during the parsing process.
#[derive(Debug)]
pub struct ParsingError {
    pub(crate) kind: ParsingErrorKind,
    pub(crate) info: lexer::TokenInfo,
}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParsingErrorKind::DuplicateName(name) => match &self.info.current_id {
                Some(id) => write!(f, "found duplicate name '{}' in entry '{}'", name, id),
                None => write!(f, "found duplicate name '{}'", name),
            },
            ParsingErrorKind::UnexpectedText(unexp, action)
                => write!(f, "unexpected text '{unexp}' while {action}"),
            ParsingErrorKind::UnexpectedEOF(action)
                => write!(f, "unexpected end of file while {action}"),
        }
    }
}

impl error::Error for ParsingError {}
