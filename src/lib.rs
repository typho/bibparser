//! This crate allows to read `.bib` files in pure, safe rust.
//! 
//! `.bib` files are popular in reference management since many resources
//! allow to export metadata in a BibTeχ or BibLaTeχ file. One entry
//! in such a file can look like this:
//! 
//! ```tex
//! @book{DBLP:books/aw/Knuth73a,
//!     author    = {Donald E. Knuth},
//!     title     = {The Art of Computer Programming, Volume {I:} Fundamental Algorithms,
//!                  2nd Edition},
//!     publisher = {Addison-Wesley},
//!     year      = {1973},
//!     url       = {https://www.worldcat.org/oclc/310903895},
//!     isbn      = {0201038218},
//!     timestamp = {Fri, 17 Jul 2020 16:12:45 +0200},
//!     biburl    = {https://dblp.org/rec/books/aw/Knuth73a.bib},
//!     bibsource = {dblp computer science bibliography, https://dblp.org}
//!  }
//! ```
//! 
//! ([original source](https://dblp.uni-trier.de/rec/books/aw/Knuth73a.html?view=bibtex&param=2))
//! In this example, we call `book`, a `kind` or `type`. We call `DBLP:books/aw/Knuth73a`, an `ID`.
//! Then we have a sequence of fields with `name` (like `year`) and `data` (like `1973`).
//! The formal grammar is not well-specified, but the [biblatex package documentation](https://ctan.ebinger.cc/tex-archive/macros/latex/contrib/biblatex/doc/biblatex.pdf)
//! and [Tame the BeaST](https://ftp.rrze.uni-erlangen.de/ctan/info/bibtex/tamethebeast/ttb_en.pdf) provide some insights.
//! 
//! Its API is built around the idea of iterating over the bib file's entries:
//! The API looks as follows:
//! 
//! ```rust
//! use bibparser::Parser;
//! use std::str::FromStr;
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     //let mut p = Parser::from_file("source.bib")?;
//!     let mut p = Parser::from_str(r#"@book{tolkien1937, author = {J. R. R. Tolkien}}"#)?;
//!     for result in p.iter() {
//!         let entry = result?;
//!         println!("type = {}", entry.kind);
//!         println!("id = {}", entry.id);
//!         for (name, data) in entry.fields.iter() {
//!             println!("\t{}\t= {}", name, data);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//! 
//! Since `data` is often some Teχ-like syntax, we provide the method `unicode_data` with `entry`
//! in order to generate a representation close to Unicode; resolving some Teχ semantics.
//! 
//! Currently, the entries are read at once. The entire source string is kept in memory and
//! parsed at once. This is meant to be changed in upcoming releases.

mod errors;
mod lexer;
mod parser;
mod types;

pub use crate::parser::Parser;
pub use crate::types::BibEntry;
pub use crate::parser::BibEntries;