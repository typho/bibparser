use bibparser::Parser;

use clap;
use clap::Parser as CLIParser;

use std::error;

#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Settings {
    /// Filepath to file to parse
    #[clap(short, long)]
    input: String,

    /// Return only entries with this ID
    #[clap(short, long)]
    query_id: String,
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let settings = Settings::parse();

    let mut p = Parser::from_file(settings.input)?;
    for result in p.iter() {
        match result {
            Ok(entry) => {
                if !settings.query_id.is_empty() && settings.query_id != entry.id {
                    continue;
                }
                println!("type = {}", entry.kind);
                println!("id = {}", entry.id);
                for (name, _) in entry.fields.iter() {
                    println!("\t{}\t= {}", name, entry.unicode_data(name).unwrap());
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(())
}
