use clap::Parser;
use html_query::{process_html, HqConfig};
use std::error::Error;
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[expect(clippy::struct_excessive_bools)] // ok since it's a "central point" for options
struct Config {
    /// What CSS selector to filter with.
    #[arg(default_value = ":root")]
    selector: String,

    /// Where to read HTML input from.
    #[arg(short = 'f', long = "filename", default_value = "-")]
    input_path: String,

    /// Where to write the filtered HTML to.
    #[arg(short = 'o', long = "output", default_value = "-")]
    output_path: String,

    /// What URL to prepend to links without an origin, i.e. starting with a slash (/).
    #[arg(short, long)]
    base: Option<String>,

    /// Look for the `<base>` tag in input for the base.
    #[arg(short = 'B', long)]
    detect_base: bool,

    /// Output only the contained text of the filtered nodes, not the entire HTML.
    #[arg(short, long = "text")]
    text_only: bool,

    /// Skip over text nodes whose text that is solely whitespace.
    #[arg(short, long)]
    ignore_whitespace: bool,

    /// If to reformat the HTML to be more nicely user-readable.
    #[arg(short, long = "pretty")]
    pretty_print: bool,

    /// Do not output the nodes matching any of these selectors.
    #[arg(short, long)]
    remove_nodes: Vec<String>,

    /// Output only the contents of the given attributes.
    #[arg(short, long)]
    attributes: Vec<String>,

    /// Remove all whitespace from output.
    #[arg(short, long)]
    compact: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli_config = Config::parse();

    let mut input: Box<dyn Read> = match cli_config.input_path.as_ref() {
        "-" => Box::new(std::io::stdin()),
        f => Box::new(File::open(f).expect("should have opened input file")),
    };

    let mut html = String::new();
    input.read_to_string(&mut html)?;

    let hq_config = HqConfig {
        selector: cli_config.selector,
        base: cli_config.base,
        detect_base: cli_config.detect_base,
        text_only: cli_config.text_only,
        ignore_whitespace: cli_config.ignore_whitespace,
        pretty_print: cli_config.pretty_print,
        remove_nodes: cli_config.remove_nodes,
        attributes: cli_config.attributes,
        compact: cli_config.compact,
    };

    let result = process_html(&html, &hq_config)?;

    match cli_config.output_path.as_ref() {
        "-" => print!("{}", result),
        f => std::fs::write(f, result).expect("should have written output file"),
    }

    Ok(())
}
