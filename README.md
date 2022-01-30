# bibparser

A Rust crate for parsing BibTeχ and BibLaTeχ files without Teχ interpretation.

As opposed to the `biblatex` crate, this crate does not try to interpret the content of fields.
This crate resulted from the usecase that `biblatex` threw an error when math mode was interrupted prematurely.

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
bibparser = "0.5"
```

Parsing a bibliography and getting the author of an item is as simple as:

```rust
let src = "@book{tolkien1937, author = {J. R. R. Tolkien}}";
let bibliography = Bibliography::parse(src).unwrap();
let entry = bibliography.get("tolkien1937").unwrap();
let author = entry.author().unwrap();
assert_eq!(author[0].name, "Tolkien");
```

This library operates on a `Bibliography` struct, which is a collection of
_entries_ (the items in your `.bib` file that start with an `@` and are wrapped
in curly braces). The entries may hold multiple fields. Entries have getter
methods for each of the possible fields in a Bib(La)TeX file which handle
possible field aliases, composition and type conversion automatically.

Refer to the [WikiBook section on LaTeX bibliography management](https://en.wikibooks.org/wiki/LaTeX/Bibliography_Management)
and the [BibLaTeX package manual](http://ctan.ebinger.cc/tex-archive/macros/latex/contrib/biblatex/doc/biblatex.pdf)
to learn more about the intended meaning of each of the fields.

The generated documentation more specifically describes the selection and
behavior of the getters but generally, they follow the convention of being the
snake-case name of the corresponding field
(such that the getter for `booktitleaddon` is named `book_title_addon`).

## Limitations

This library attempts to provide fairly comprehensive coverage of the BibLaTeX
spec with which most of the `.bib` files in circulation can be processed.

However, the crate currently has some limitations:

- Math mode formatting is not being processed, instead, the output strings will
  contain the dollar-delimited math syntax as it is found in the input string.
- There is no explicit support for entry sets, although it is easy to account
  for them by manually getting the `entryset` field and calling
  `parse::<Vec<String>>()` on it
