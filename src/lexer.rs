use std::collections::VecDeque;
use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::path;
use std::str;

use crate::errors;

/// A token is one semantic unit read from the biblatex file.
/// Remember, that bib file entry looks as follows:
///
/// ```tex
/// @Book{works:4,
///   author     = {Shakespeare, William},
///   title      = {Sonnets},
/// }
/// ```
///
/// In this case, the lexer would emit the following Token instances:
/// (EntrySymbol, EntryType("Book"), OpenEntry, EntryId("works:4"),
/// FieldName("author"), FieldData("Shakespeare, William"), FieldName("title"),
/// FieldData("Sonnets"), CloseEntry). Be aware that Token is just the
/// data contract between lexer and parser and not meant to be externally
/// visible.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
    EntrySymbol,
    EntryType(String),
    OpenEntry,
    EntryId(String),
    FieldName(String),
    FieldData(String),
    CloseEntry,
    EndOfFile,
}

impl<'t> fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::EntrySymbol => "@",
                Self::EntryType(s) => s,
                Self::OpenEntry => "{",
                Self::EntryId(s) => s,
                Self::FieldName(s) => s,
                Self::FieldData(s) => s,
                Self::CloseEntry => "}",
                Self::EndOfFile => "end of file",
            }
        )
    }
}

/// Additional source code information attached to a Token
/// for improved error messages
#[derive(Debug)]
pub(crate) struct TokenInfo {
    pub(crate) lineno: usize,
    pub(crate) colno: usize,
    pub(crate) current_line: String,
    pub(crate) current_id: Option<String>,
    pub(crate) previous_name: Option<String>,
}

#[derive(PartialEq)]
pub(crate) enum LexingState {
    Default,
    ReadingType,
    WaitForOpen,
    ReadingId,
    WaitForComma,
    ReadingName,
    WaitForAssign,
    ReadingDataStart,
    ReadingData,
    WaitForSep,
}

impl fmt::Display for LexingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Default => "waiting for next entry",
                Self::ReadingType => "reading entry type",
                Self::WaitForOpen => "expecting '{' for entry data",
                Self::ReadingId => "reading entry ID",
                Self::WaitForComma => "waiting for comma separating ID and fields",
                Self::ReadingName => "reading field name",
                Self::WaitForAssign => "expecting '=' for field assignment",
                Self::ReadingDataStart => "reading start of field data",
                Self::ReadingData => "reading field data",
                Self::WaitForSep => "expecting separator ',' between field",
            }
        )
    }
}

impl Eq for LexingState {}

pub(crate) struct LexingIterator<'s> {
    pub(crate) src: &'s str,
    pub(crate) next_tokens: VecDeque<(Token, TokenInfo)>,
    pub(crate) lineno: usize,
    pub(crate) colno: usize,
    pub(crate) state: LexingState,
    pub(crate) current_id: Option<String>, // the ID of the current entry, e.g. “DBLP:books/lib/Knuth97”
    pub(crate) arg_cache: String,          // accumulates token arguments which are strings
    pub(crate) dblquotes_terminator: bool, // is the current field data enclosed in "double quotes"?
    pub(crate) curlybrace_terminator: bool, // is the current field data enclosed in {curly braces}?
    pub(crate) curlybrace_level: usize, // inside how many levels of curly braces of the field data are we?
    pub(crate) eof: bool,               // did he file end?
}

impl<'s> LexingIterator<'s> {
    /// lex() continues its lexing process, but stops at some point (usually EOLs).
    /// The generated tokens are pushed to `self.next_tokens`.
    fn lex(&mut self) -> Option<Box<dyn error::Error>> {
        // TODO supply previous_name properly
        for line in self.src.lines() {
            for chr in line.chars() {
                let info = || TokenInfo {
                    lineno: self.lineno,
                    colno: self.colno,
                    current_line: line.to_string(),
                    current_id: self.current_id.clone(),
                    previous_name: self.current_id.as_ref().map(|name| name.to_string()),
                };
                let unexpected = |text: &'static str| -> Option<Box<dyn std::error::Error>> {
                    Some(Box::new(errors::LexingError::UnexpectedChar(
                        chr,
                        text,
                        info(),
                    )))
                };

                match self.state {
                    // expecting '@'
                    LexingState::Default => {
                        if chr == '@' {
                            self.state = LexingState::ReadingType;
                        } else if chr.is_whitespace() {
                            // ignore
                        } else {
                            return unexpected("reading next entry");
                        }
                    }
                    // expecting entry type, e.g. “book”
                    LexingState::ReadingType => {
                        if chr.is_whitespace() {
                            if self.arg_cache.is_empty() {
                                // ignore
                            } else {
                                self.next_tokens.push_back((Token::EntrySymbol, info()));
                                self.state = LexingState::WaitForOpen;
                            }
                        } else if chr.is_alphanumeric()
                            || (!self.arg_cache.is_empty() && chr.is_whitespace())
                        {
                            self.arg_cache.push(chr);
                        } else if chr == '{' {
                            self.next_tokens.push_back((Token::EntrySymbol, info()));
                            self.next_tokens
                                .push_back((Token::EntryType(self.arg_cache.clone()), info()));
                            self.next_tokens.push_back((Token::OpenEntry, info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingId;
                        } else {
                            return unexpected("reading entry type");
                        }
                    }
                    // expecting “{”
                    LexingState::WaitForOpen => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '{' {
                            self.next_tokens
                                .push_back((Token::EntryType(self.arg_cache.clone()), info()));
                            self.next_tokens.push_back((Token::OpenEntry, info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingId;
                        } else {
                            return unexpected("expecting '{' to end field");
                        }
                    }
                    // expecting e.g. “DBLP:books/lib/Knuth97”
                    LexingState::ReadingId => {
                        if chr.is_whitespace() {
                            if self.arg_cache.is_empty() {
                                // ignore
                            } else {
                                self.state = LexingState::WaitForComma;
                            }
                        } else if chr == ',' {
                            self.next_tokens
                                .push_back((Token::EntryId(self.arg_cache.clone()), info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingName;
                        } else if !chr.is_ascii() {
                            return unexpected("expecting ASCII entry name");
                        } else {
                            self.arg_cache.push(chr);
                        }
                    }
                    LexingState::WaitForComma => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == ',' {
                            self.next_tokens
                                .push_back((Token::EntryId(self.arg_cache.clone()), info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingName;
                        } else {
                            return unexpected("expecting ',' after name");
                        }
                    }
                    LexingState::ReadingName => {
                        if chr.is_whitespace() {
                            if self.arg_cache.is_empty() {
                                // ignore
                            } else {
                                self.state = LexingState::WaitForAssign;
                            }
                        } else if chr.is_ascii() {
                            self.arg_cache.push(chr);
                        } else if chr == '=' {
                            self.next_tokens
                                .push_back((Token::FieldName(self.arg_cache.clone()), info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingDataStart;
                        } else {
                            return unexpected("expecting field name");
                        }
                    }
                    LexingState::WaitForAssign => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '=' {
                            self.next_tokens
                                .push_back((Token::FieldName(self.arg_cache.clone()), info()));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingDataStart;
                        } else {
                            return unexpected("expecting field name");
                        }
                    }
                    LexingState::ReadingDataStart => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '{' {
                            self.curlybrace_terminator = true;
                            self.dblquotes_terminator = false;
                            self.curlybrace_level = 0;
                            self.state = LexingState::ReadingData;
                        } else if chr == '"' {
                            self.curlybrace_terminator = false;
                            self.dblquotes_terminator = true;
                            self.curlybrace_level = 0;
                            self.state = LexingState::ReadingData;
                        } else {
                            return unexpected("expecting field name");
                        }
                    }
                    LexingState::ReadingData => {
                        // TODO: is “\}” an escaped version to terminate “}”?
                        if chr == '{' {
                            self.curlybrace_level += 1;
                            self.arg_cache.push(chr);
                        } else if chr == '}' {
                            if self.curlybrace_terminator && self.curlybrace_level == 0 {
                                self.next_tokens
                                    .push_back((Token::FieldData(self.arg_cache.clone()), info()));
                                self.arg_cache.clear();
                                self.state = LexingState::WaitForSep;
                            } else {
                                self.curlybrace_level -= 1;
                                self.arg_cache.push(chr);
                            }
                        } else if chr == '"' {
                            if self.dblquotes_terminator {
                                self.next_tokens
                                    .push_back((Token::FieldData(self.arg_cache.clone()), info()));
                                self.arg_cache.clear();
                                self.state = LexingState::WaitForSep;
                            } else {
                                self.arg_cache.push(chr);
                            }
                        } else {
                            self.arg_cache.push(chr);
                        }
                    }
                    LexingState::WaitForSep => {
                        if chr == ',' {
                            self.state = LexingState::ReadingName;
                        } else if chr == '}' {
                            self.next_tokens.push_back((Token::CloseEntry, info()));
                            self.state = LexingState::Default;
                        } else if chr.is_whitespace() {
                            // ignore
                        }
                    }
                }
                self.colno += 1;
            }

            self.lineno += 1;
            self.colno = 0;
        }

        if self.state != LexingState::Default {
            return Some(Box::new(errors::LexingError::UnexpectedEOF(
                self.state.to_string(),
            )));
        }

        self.next_tokens.push_back((
            Token::EndOfFile,
            TokenInfo {
                lineno: self.lineno,
                colno: 0,
                current_line: String::from(""),
                current_id: None,
                previous_name: None,
            },
        ));
        self.eof = true;

        None
    }
}

impl<'s> Iterator for LexingIterator<'s> {
    type Item = Result<(Token, TokenInfo), Box<dyn error::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(tok) = self.next_tokens.pop_front() {
                return Some(Ok(tok));
            }
            if self.eof {
                return None;
            }

            self.lex(); // try to generate new tokens
        }
    }
}

pub(crate) struct Lexer {
    src: String,
}

impl Lexer {
    /// Use a file stored at a `path` as source for the lexing process.
    pub(crate) fn from_file<P: AsRef<path::Path>>(path: P) -> Result<Lexer, io::Error> {
        let mut fd = fs::File::open(path)?;
        let mut buf = String::new();
        fd.read_to_string(&mut buf)?;
        Ok(Lexer { src: buf })
    }

    /// Use a string as source for the lexing process.
    pub(crate) fn from_string(data: String) -> Result<Lexer, io::Error> {
        Ok(Lexer { src: data })
    }

    pub(crate) fn iter(&self) -> LexingIterator {
        LexingIterator {
            src: &self.src,
            next_tokens: VecDeque::new(),
            lineno: 0,
            colno: 0,
            state: LexingState::Default,
            current_id: None,
            arg_cache: String::new(),
            dblquotes_terminator: false,
            curlybrace_terminator: false,
            curlybrace_level: 0,
            eof: false,
        }
    }
}

impl str::FromStr for Lexer {
    type Err = io::Error;

    /// Use a string as source for the lexing process.
    fn from_str(data: &str) -> Result<Self, Self::Err> {
        Ok(Lexer {
            src: data.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_tolkien() -> Result<(), Box<dyn error::Error>> {
        let l = Lexer::from_str("@book{tolkien1937, author = {J. R. R. Tolkien}}")?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
        eprintln!("{:?}", seq);
        assert_eq!(seq[0], Token::EntrySymbol);
        assert_eq!(seq[1], Token::EntryType("book".to_string()));
        assert_eq!(seq[2], Token::OpenEntry);
        assert_eq!(seq[3], Token::EntryId("tolkien1937".to_string()));
        assert_eq!(seq[4], Token::FieldName("author".to_string()));
        assert_eq!(seq[5], Token::FieldData("J. R. R. Tolkien".to_string()));
        assert_eq!(seq[6], Token::CloseEntry);
        assert_eq!(seq[7], Token::EndOfFile);
        Ok(())
    }
}
