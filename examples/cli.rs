use std::error;
use bibparser::Parser;

use clap;
use clap::Parser as CLIParser;

#[cfg(not(feature = "serde_json"))]
#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Settings {
    /// Filepath to file to parse
    #[clap(short, long)]
    input: String,

    /// Return only entries with this ID
    #[clap(short, long)]
    query_id: Option<String>,
}

#[cfg(feature = "serde_json")]
#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Settings {
    /// Filepath to file to parse
    #[clap(short, long)]
    input: String,

    /// Return only entries with this ID
    #[clap(short, long)]
    query_id: Option<String>,

    #[clap(long)]
    json: bool,
}

fn print_human_readable(s: &Settings) -> Result<(), Box<dyn error::Error>> {
    let mut p = Parser::from_file(&s.input)?;
    for result in p.iter() {
        let entry = result?;
        if let Some(query) = &s.query_id {
            if query != &entry.id {
                continue;
            }
        }
        println!("type = {}", entry.kind);
        println!("id = {}", entry.id);
        for (name, _) in entry.fields.iter() {
            println!("\t{}\t= {}", name, entry.unicode_data(name).unwrap());
        }
    }

    Ok(())
}

#[cfg(feature = "serde_json")]
fn print_json(s: &Settings) -> Result<(), Box<dyn error::Error>> {
    use serde::{Deserialize, Serialize};
    use serde_json::Result;
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    struct Entry {
        kind: String,
        id: String,
        fields: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize)]
    struct Entries {
        data: Vec<Entry>,
    }

    let mut json_entries = Entries { data: Vec::new() };
    for result in Parser::from_file(&s.input)?.iter() {
        let entry = result?;
        if let Some(query) = &s.query_id {
            if query != &entry.id {
                continue;
            }
        }

        json_entries.data.push(
            Entry {
                kind: entry.kind,
                id: entry.id,
                fields: entry.fields.clone(),
            }
        );
    }

    println!("{}", serde_json::to_string(&json_entries)?);

    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let settings = Settings::parse();

    #[cfg(feature = "serde_json")]
    {
        print_json(&settings)?;
    }
    #[cfg(not(feature = "serde_json"))]
    {
        print_human_readable(&settings)?;
    }

    Ok(())
}
