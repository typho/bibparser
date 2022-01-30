# bibparser

A Rust crate for parsing BibTeχ and BibLaTeχ files.

As opposed to the `biblatex` crate, this crate does not try to interpret the content of fields.
This crate resulted from the usecase that `biblatex` threw an error when math inline mode was not terminated before text was cut off.

## Who should use it?

Anyone, how wants to retrieve data from a `.bib` file.

## How does one use it?

Add this to your `Cargo.toml`:
```toml
[dependencies]
bibparser = "0.3.1"
```

Instantiate the parser and iterate over the items:
```rust
use bibparser::Parser;

//let mut p = Parser::from_file("source.bib")?;
let mut p = Parser::from_str(r#"@book{tolkien1937, author = {J. R. R. Tolkien}}"#)?;
for result in p.iter() {
  let entry = result?;
  println!("type = {}", entry.kind);
  println!("id = {}", entry.id);
  for (name, data) in entry.fields.iter() {
      println!("\t{}\t= {}", name, data);
  }
}
```

## How does one run it?

This library comes with one example:

```bash
$ cargo run --example cli -- --input refs.bib --query-id "tolkien1937"
```

In this example, the library would read file `refs.bib` and then only print the entry with ID `tolkien1937` to stdout.

## Where is the source code?

On [github](https://github.com/typho/bibparser).

## What is the content's license?

[MIT License](LICENSE.txt)

## Changelog

* **2022-01-30 version 0.3.1:** fix documentation & README
* **2022-01-30 version 0.3.0:** initial release

## Where can I ask you to fix a bug?

On [github](https://github.com/typho/bibparser/issues).

## What are known bugs / limitations?

- `.bib` are strongly associated with Teχ which is a programming language, not a markup language. As such only the plain Teχ engine would be capable of understanding the content (esp. the field's `data` content). This library explicitly takes the approach to assume the content to be a markup language, which hopefully serves 99% of all usecases.
- The markup language parsed by this library is not formalized.
- Compability to BibTeχ or biblatex was not comprehensively tested.
