use std::collections::VecDeque;
use std::error;
use std::io;
use std::mem;
use std::path;
use std::str;

use crate::errors;
use crate::lexer;
use crate::types;

/// Parser parsing a `.bib` file allowing iteration over `BibEntry` instances
pub struct Parser {
    pub(crate) lexer: lexer::Lexer,
}

impl Parser {
    /// Use a file at some filepath as source for the parsing process.
    pub fn from_file<P: AsRef<path::Path>>(path: P) -> Result<Parser, io::Error> {
        let lexer = lexer::Lexer::from_file(path)?;
        Ok(Parser { lexer })
    }

    /// Use a string as source for the parsing process.
    pub fn from_string(data: String) -> Result<Parser, io::Error> {
        let lexer = lexer::Lexer::from_string(data)?;
        Ok(Parser { lexer })
    }

    pub fn iter(&mut self) -> BibEntries {
        BibEntries {
            iter: self.lexer.iter(),
            entries: VecDeque::new(),
            current: types::BibEntry::new(),
            name_cached: String::new(),
            finished: false,
        }
    }
}

impl str::FromStr for Parser {
    type Err = io::Error;

    /// Use a string as source for the parsing process.
    fn from_str(data: &str) -> Result<Self, Self::Err> {
        let lexer = lexer::Lexer::from_string(data.to_string())?;
        Ok(Parser { lexer })
    }
}

/// A stateful iterator yielding one BibEntry instance after another
pub struct BibEntries<'i> {
    pub(crate) iter: lexer::LexingIterator<'i>,
    pub(crate) entries: VecDeque<types::BibEntry>,
    pub(crate) current: types::BibEntry,
    pub(crate) name_cached: String,
    pub(crate) finished: bool,
}

impl<'i> BibEntries<'i> {
    /// parse() continues parsing and adds new elements to `self.entries`
    fn parse(&mut self) -> Option<Box<dyn error::Error>> {
        use lexer::Token as T;

        match self.iter.next() {
            Some(t) => match t {
                Ok((token, token_info)) => match token {
                    T::EntrySymbol => {}
                    T::EntryType(kind) => self.current.kind.push_str(&kind),
                    T::OpenEntry => {}
                    T::EntryId(id) => self.current.id.push_str(&id),
                    T::FieldName(name) => {
                        self.name_cached = name;
                    }
                    T::FieldData(data) => {
                        let name = mem::take(&mut self.name_cached);
                        if self.current.fields.get(&name).is_some() {
                            return Some(Box::new(errors::ParsingError {
                                kind: errors::ParsingErrorKind::DuplicateName(name),
                                info: token_info,
                            }));
                        }
                        self.current.fields.insert(name, data);
                    }
                    T::CloseEntry => {
                        let finished = mem::replace(&mut self.current, types::BibEntry::new());
                        self.entries.push_back(finished);
                    }
                    T::EndOfFile => {}
                },
                Err(e) => return Some(e),
            },
            None => self.finished = true,
        }
        None
    }
}

impl<'s> Iterator for BibEntries<'s> {
    type Item = Result<types::BibEntry, Box<dyn error::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.finished {
                return None;
            }
            if let Some(entry) = self.entries.pop_front() {
                return Some(Ok(entry));
            }
            if let Some(err) = self.parse() {
                return Some(Err(err));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error;
    use std::str::FromStr;

    #[test]
    fn test_tolkien() -> Result<(), Box<dyn error::Error>> {
        let mut p = Parser::from_str("@book{tolkien1937, author = {J. R. R. Tolkien}}")?;
        let mut count = 0;
        for e in p.iter() {
            let entry = e?;
            assert_eq!(entry.kind, "book");
            assert_eq!(entry.id, "tolkien1937");
            assert_eq!(
                entry.fields.get("author"),
                Some(&"J. R. R. Tolkien".to_string())
            );
            count += 1;
        }
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_taocp() -> Result<(), Box<dyn error::Error>> {
        let src = r#"@book{DBLP:books/lib/Knuth97,
  author    = {Donald Ervin Knuth},
  title     = {The art of computer programming, Volume {I:} Fundamental Algorithms,
               3rd Edition},
  publisher = {Addison-Wesley},
  year      = {1997},
  url       = {https://www.worldcat.org/oclc/312910844},
  isbn      = {0201896834},
  timestamp = {Fri, 17 Jul 2020 16:12:39 +0200},
  biburl    = {https://dblp.org/rec/books/lib/Knuth97.bib},
  bibsource = {{dblp computer science bibliography}, https://dblp.org}
}"#;
        let mut p = Parser::from_str(src)?;
        let mut iter = p.iter();
        let entry = iter.next().unwrap()?;
        assert_eq!(entry.kind, "book");
        assert_eq!(entry.id, "DBLP:books/lib/Knuth97");
        assert_eq!(entry.fields.get("year").unwrap(), "1997");
        assert_eq!(
            entry.fields.get("bibsource").unwrap(),
            "{dblp computer science bibliography}, https://dblp.org"
        );
        Ok(())
    }
}
