use std::collections::VecDeque;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::iter;
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
///
/// BibTeX files can have `@preamble{…}` instructions to add `…` to the
/// LaTeχ preamble. This lexer can also read them. They are meant to be skipped
/// by the parser because they are not supplied through the public API.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
    EntrySymbol,
    EntryType(String),
    OpenEntry,
    EntryId(String),
    FieldName(String),
    FieldData(String),
    Preamble(String),
    CloseEntry,
    EndOfFile,
}

impl fmt::Display for Token {
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
                Self::Preamble(s) => s,
                Self::CloseEntry => "}",
                Self::EndOfFile => "end of file",
            }
        )
    }
}

/// Additional source code information attached to a Token
/// for improved error messages
#[derive(Clone,Debug)]
pub(crate) struct TokenInfo {
    pub(crate) lineno: usize,
    pub(crate) colno: usize,
    pub(crate) current_line: String,
    pub(crate) current_id: Option<String>,
}

#[derive(Debug, PartialEq)]
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
    ReadingPreambleStringStart,
    ReadingPreambleStringStartOrConcat,
    ReadingPreambleString,
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
                Self::ReadingPreambleStringStart => "reading start of preamble string",
                Self::ReadingPreambleString => "reading preamble content string",
                Self::ReadingPreambleStringStartOrConcat => "reading next preamble content string",
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
    pub(crate) escape_character: bool,     // was the previous character the escape character “\”?
    pub(crate) dblquotes_terminator: bool, // is the current field data enclosed in "double quotes"?
    pub(crate) curlybrace_terminator: bool, // is the current field data enclosed in {curly braces}?
    pub(crate) curlybrace_level: usize, // inside how many levels of curly braces of the field data are we?
    pub(crate) eof: bool,               // did the file end?
}

impl<'s> LexingIterator<'s> {
    /// Create a TokenInfo object for debugging
    fn info(&self, line: &str) -> TokenInfo {
        TokenInfo {
            lineno: self.lineno,
            colno: self.colno,
            current_line: line.to_string(),
            current_id: self.current_id.clone(),
        }
    }

    fn postprocess_field_value(s: &str) -> String {
        //r#"{\"a} {\^e} {\`i} {\.I} {\o} {\'u} {\aa} {\c c} {\u g} {\l} {\~n} {\H o} {\v r} {\ss} {\r u}"#
        // https://tex.stackexchange.com/a/57745
        s.to_string()
    }

    /// lex() continues its lexing process, but stops at some point (usually EOLs).
    /// The generated tokens are pushed to `self.next_tokens`.
    fn lex(&mut self) -> Result<(), errors::LexingError> {
        for line in self.src.lines() {
            // BUG: since we call .lines(), we loose information about the line terminator.
            //      Here we just claim it was U+000A LINE FEED
            let iterator = line.chars().chain(iter::once('\n'));
            for chr in iterator {
                let unexpected = |text: &'static str| -> Result<(), errors::LexingError> {
                    Err(errors::LexingError::UnexpectedChar(
                        chr,
                        text,
                        self.info(line),
                    ))
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
                                self.next_tokens.push_back((Token::EntrySymbol, self.info(line)));
                                self.state = LexingState::WaitForOpen;
                            }
                        } else if chr.is_alphanumeric()
                            || (!self.arg_cache.is_empty() && chr.is_whitespace())
                        {
                            self.arg_cache.push(chr);
                        } else if chr == '{' {
                            if !self.arg_cache.is_empty() {
                                self.current_id = Some(self.arg_cache.clone());
                            }
                            self.next_tokens.push_back((Token::EntrySymbol, self.info(line)));
                            self.next_tokens
                                .push_back((Token::EntryType(self.arg_cache.clone()), self.info(line)));
                            self.next_tokens.push_back((Token::OpenEntry, self.info(line)));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingId;

                            // handle the @preamble{…} specifier as special case
                            if let Some(id) = &self.current_id {
                                if id.to_lowercase() == "preamble" {
                                    self.state = LexingState::ReadingPreambleStringStart;
                                }
                            }
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
                                .push_back((Token::EntryType(self.arg_cache.clone()), self.info(line)));
                            self.next_tokens.push_back((Token::OpenEntry, self.info(line)));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingId;

                            // handle the @preamble{…} specifier as special case
                            if let Some(id) = &self.current_id {
                                if id.to_lowercase() == "preamble" {
                                    self.state = LexingState::ReadingPreambleStringStart;
                                }
                            }

                        } else {
                            return unexpected("expecting '{' to start list of fields");
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
                                .push_back((Token::EntryId(self.arg_cache.clone()), self.info(line)));
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
                                .push_back((Token::EntryId(self.arg_cache.clone()), self.info(line)));
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
                        } else if chr == '=' {
                            self.next_tokens
                                .push_back((Token::FieldName(self.arg_cache.clone()), self.info(line)));
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingDataStart;
                        } else if chr.is_ascii() {
                            self.arg_cache.push(chr);
                        } else {
                            return unexpected("expecting field name");
                        }
                    }
                    LexingState::WaitForAssign => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '=' {
                            self.next_tokens
                                .push_back((Token::FieldName(self.arg_cache.clone()), self.info(line)));
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
                        if chr == '\\' && !self.escape_character {
                            self.escape_character = true;
                        } else if chr == '\\' && self.escape_character {
                            self.escape_character = false;
                            self.arg_cache.push('\n');
                        } else if chr == '{' && !self.escape_character {
                            if self.curlybrace_terminator {
                                self.curlybrace_level += 1;
                            }
                            self.arg_cache.push(chr);
                        } else if chr == '}' && !self.escape_character {
                            if self.curlybrace_terminator && self.curlybrace_level == 0 {
                                let content = Self::postprocess_field_value(&self.arg_cache);
                                self.next_tokens
                                    .push_back((Token::FieldData(content), self.info(line)));
                                self.arg_cache.clear();
                                self.state = LexingState::WaitForSep;
                            } else {
                                if self.curlybrace_terminator {
                                    self.curlybrace_level -= 1;
                                }
                                self.arg_cache.push(chr);
                            }
                        } else if chr == '"' && !self.escape_character {
                            if self.dblquotes_terminator {
                                let content = Self::postprocess_field_value(&self.arg_cache);
                                self.next_tokens
                                    .push_back((Token::FieldData(content), self.info(line)));
                                self.arg_cache.clear();
                                self.state = LexingState::WaitForSep;
                            } else {
                                self.arg_cache.push(chr);
                            }
                        } else if self.escape_character && chr == '"' && self.dblquotes_terminator {
                            self.escape_character = false;
                            self.arg_cache.push(chr);
                        } else if self.escape_character && chr == '}' && self.curlybrace_terminator {
                            self.escape_character = false;
                            self.arg_cache.push(chr);
                        } else if self.escape_character {
                            self.escape_character = false;
                            self.arg_cache.push('\\');
                            self.arg_cache.push(chr);
                        } else {
                            self.arg_cache.push(chr);
                        }
                    }
                    LexingState::ReadingPreambleStringStart => {
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '"' {
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingPreambleString;
                        } else if chr == '}' {
                            self.next_tokens.push_back((Token::CloseEntry, self.info(line)));
                            self.state = LexingState::Default;
                        } else {
                            return unexpected("reading '\"' to start a preamble string or '}' to end preamble entry");
                        }
                    },
                    LexingState::ReadingPreambleStringStartOrConcat => {
                        // this state is similar to “ReadingPreambleStringStart”
                        // but also accepts "#" because this character concatenates strings
                        if chr.is_whitespace() {
                            // ignore
                        } else if chr == '"' {
                            self.arg_cache.clear();
                            self.state = LexingState::ReadingPreambleString;
                        } else if chr == '}' {
                            self.next_tokens.push_back((Token::CloseEntry, self.info(line)));
                            self.state = LexingState::Default;
                        } else if chr == '#' {
                            self.state = LexingState::ReadingPreambleStringStart;
                            // TODO: BUG: ReadingPreambleStringStart takes "}", but I think "# }" is invalid syntax
                        } else {
                            return unexpected("reading '\"' to start a preamble string or '}' to end preamble entry");
                        }
                    },
                    LexingState::ReadingPreambleString => {
                        if chr == '\\' && !self.escape_character {
                            self.escape_character = true;
                        } else if chr == '"' && self.escape_character {
                            self.escape_character = false;
                            self.arg_cache.push('"');
                        } else if chr == '"' && !self.escape_character {
                            self.next_tokens
                                .push_back((Token::Preamble(self.arg_cache.clone()), self.info(line)));
                            self.state = LexingState::ReadingPreambleStringStartOrConcat;
                        } else {
                            if self.escape_character {
                                self.arg_cache.push('\\');
                            }
                            self.arg_cache.push(chr);
                            self.escape_character = false;
                        }
                    },
                    LexingState::WaitForSep => {
                        if chr == ',' {
                            self.state = LexingState::ReadingName;
                        } else if chr == '}' {
                            self.next_tokens.push_back((Token::CloseEntry, self.info(line)));
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
            return Err(errors::LexingError::UnexpectedEOF(
                self.state.to_string(),
            ));
        }

        self.next_tokens.push_back((
            Token::EndOfFile,
            TokenInfo {
                lineno: self.lineno,
                colno: 0,
                current_line: String::from(""),
                current_id: None,
            },
        ));
        self.eof = true;

        Ok(())
    }
}

impl<'s> Iterator for LexingIterator<'s> {
    type Item = Result<(Token, TokenInfo), errors::LexingError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // there are some tokens? then send them out!
            if let Some(tok) = self.next_tokens.pop_front() {
                return Some(Ok(tok));
            }
            // finished? then terminate iterator.
            if self.eof {
                return None;
            }
            // try to generate new tokens.
            if let Err(e) = self.lex() {
                return Some(Err(e));
            }
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
            escape_character: false,
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
    use std::{str::FromStr, error::Error};

    #[test]
    fn test_tolkien() -> Result<(), Box<dyn Error>> {
        let l = Lexer::from_str("@book{tolkien1937, author = {J. R. R. Tolkien}}")?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
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


    #[test]
    fn test_dblp_okada_wang() -> Result<(), Box<dyn Error>> {
        let l = Lexer::from_str(r#"@article{DBLP:journals/iacr/OkadaW20,
            author    = {Satoshi Okada and
                         Yuntao Wang},
            title     = {Key Recovery Attack on Bob's Secrets in {CRYSTALS-KYBER} and {SABER}},
            journal   = {{IACR} Cryptol. ePrint Arch.},
            pages     = {1503},
            year      = {2020},
            url       = {https://eprint.iacr.org/2020/1503},
            timestamp = {Mon, 04 Jan 2021 17:01:43 +0100},
            biburl    = {https://dblp.org/rec/journals/iacr/OkadaW20.bib},
            bibsource = {dblp computer science bibliography, https://dblp.org}
          }
          "#)?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
        fn check(seq: &Vec<Token>, i: &mut usize, key: &str, val: &str) {
            assert_eq!(seq[*i + 1], Token::FieldName(key.to_string()));
            assert_eq!(seq[*i + 2], Token::FieldData(val.to_string()));
            *i += 2;
        }
        assert_eq!(seq[0], Token::EntrySymbol);
        assert_eq!(seq[1], Token::EntryType("article".to_string()));
        assert_eq!(seq[2], Token::OpenEntry);
        assert_eq!(seq[3], Token::EntryId("DBLP:journals/iacr/OkadaW20".to_string()));

        let mut idx = 3;
        check(&seq, &mut idx, "author", "Satoshi Okada and\n                         Yuntao Wang");
        check(&seq, &mut idx, "title", "Key Recovery Attack on Bob's Secrets in {CRYSTALS-KYBER} and {SABER}");
        check(&seq, &mut idx, "journal", "{IACR} Cryptol. ePrint Arch.");
        check(&seq, &mut idx, "pages", "1503");
        check(&seq, &mut idx, "year", "2020");
        check(&seq, &mut idx, "url", "https://eprint.iacr.org/2020/1503");
        check(&seq, &mut idx, "timestamp", "Mon, 04 Jan 2021 17:01:43 +0100");
        check(&seq, &mut idx, "biburl", "https://dblp.org/rec/journals/iacr/OkadaW20.bib");
        check(&seq, &mut idx, "bibsource", "dblp computer science bibliography, https://dblp.org");

        assert_eq!(seq[idx + 1], Token::CloseEntry);
        assert_eq!(seq[idx + 2], Token::EndOfFile);
        Ok(())
    }

    #[test]
    fn test_empty_preamble() -> Result<(), Box<dyn Error>> {
        let l = Lexer::from_str(r#"@PREAMBLE{}"#)?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
        assert_eq!(seq[0], Token::EntrySymbol);
        assert_eq!(seq[1], Token::EntryType("PREAMBLE".to_string()));
        assert_eq!(seq[2], Token::OpenEntry);
        assert_eq!(seq[3], Token::CloseEntry);
        assert_eq!(seq[4], Token::EndOfFile);
        Ok(())
    }


    #[test]
    fn test_preamble() -> Result<(), Box<dyn Error>> {
        let l = Lexer::from_str(r##"@PREAMBLE{ "\newcommand{\noopsort}[1]{} "
        # "\newcommand{\singleletter}[1]{\"#1\"} " }"##)?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
        assert_eq!(seq[0], Token::EntrySymbol);
        assert_eq!(seq[1], Token::EntryType("PREAMBLE".to_string()));
        assert_eq!(seq[2], Token::OpenEntry);
        assert_eq!(seq[3], Token::Preamble(r"\newcommand{\noopsort}[1]{} ".to_string()));
        assert_eq!(seq[4], Token::Preamble(r##"\newcommand{\singleletter}[1]{"#1"} "##.to_string()));
        assert_eq!(seq[5], Token::CloseEntry);
        assert_eq!(seq[6], Token::EndOfFile);
        Ok(())
    }

    #[test]
    fn test_accented_names_and_escaped_strings() -> Result<(), Box<dyn Error>> {
        let l = Lexer::from_str(r#"@book{some, author = "\AA{ke} {Jos{\’{e}} {\’{E}douard} G{\"o}del" }"#)?;
        let mut seq = Vec::<Token>::new();
        for t in l.iter() {
            let (token, _info) = t?;
            seq.push(token);
        }
        assert_eq!(seq[0], Token::EntrySymbol);
        assert_eq!(seq[1], Token::EntryType("book".to_string()));
        assert_eq!(seq[2], Token::OpenEntry);
        assert_eq!(seq[3], Token::EntryId("some".to_string()));
        assert_eq!(seq[4], Token::FieldName(r"author".to_string()));
        assert_eq!(seq[5], Token::FieldData(r#"\AA{ke} {Jos{\’{e}} {\’{E}douard} G{"o}del"#.to_string()));
        assert_eq!(seq[6], Token::CloseEntry);
        assert_eq!(seq[7], Token::EndOfFile);
        Ok(())
    }
}
