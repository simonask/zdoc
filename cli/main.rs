use std::io::Read as _;
use std::{error::Error, path::Path};

use clap::ColorChoice;
use clap::Parser;

#[derive(clap::Parser)]
#[clap(name = "zdoc", styles = clap_cargo::style::CLAP_STYLING)]
struct Args {
    /// Input file, use '-' for stdin
    #[clap(value_parser, default_value = "-")]
    input: clio::Input,
    /// Output file, use '-' for stdout
    #[clap(long, short, default_value = "-")]
    output: clio::Output,
    /// Input format. When absent, guess the format from the file extension of
    /// the input. Required when the input is '-'.
    #[clap(long, short)]
    input_format: Option<Format>,
    /// Output format. When absent, guess the format from the file extension of
    /// the output. Required when the output is '-'.
    #[clap(long, short)]
    format: Option<Format>,
    /// Attempt to pretty-print the output. This is the default when printing to
    /// stdout, and stdout is a terminal.
    #[clap(long)]
    pretty: bool,
    /// Opposite of `pretty`, overriding the default when printing to a
    /// terminal.
    #[clap(long)]
    compact: bool,
    /// When using `--pretty`, whether or not to output colorized
    /// (syntax-highlighted) output (when available for that format).
    #[arg(long, default_value_t = ColorChoice::Auto, value_name = "WHEN", value_enum, global = true)]
    color: ColorChoice,

    /// When passed, print analysis of the document to stderr instead of
    /// producing output.
    #[clap(long)]
    analyze: bool,

    #[command(flatten)]
    json: JsonArgs,
}

#[derive(clap::Args)]
#[command(next_help_heading = "JSON options")]
struct JsonArgs {
    #[clap(long, default_value = "$type")]
    json_type_tag: String,
    #[clap(long, default_value = "$items")]
    json_items_tag: String,
    #[clap(long, default_value = "$value")]
    json_value_tag: String,
}

fn try_main(args: Args) -> Result<(), Box<dyn Error>> {
    if args.input.is_std() && args.input_format.is_none() {
        return Err("--format is required when input is terminal/stdout".into());
    }
    if args.output.is_std() && args.format.is_none() && !args.analyze {
        return Err("--format is required when output is terminal/stdout".into());
    }
    if !args.output.is_std() && !args.input.is_std() && args.input.path() == args.output.path() {
        return Err("input and output files must be different".into());
    }

    let input_format = match args.input_format {
        Some(format) => format,
        None => guess_format(args.input.path())?,
    };
    let output_format = match args.format {
        Some(format) => format,
        None => {
            if args.analyze {
                Format::Zdoc
            } else {
                guess_format(args.output.path())?
            }
        }
    };
    let color = match args.color {
        ColorChoice::Auto => args.output.is_tty(),
        ColorChoice::Always => true,
        ColorChoice::Never => false,
    };

    let mut input = args.input;
    let mut output = args.output;

    // Skip any parsing steps if the formats are the same and no pretty/compact
    // options are set.
    if input_format == output_format && !args.pretty && !args.compact && !color && !args.analyze {
        std::io::copy(&mut input, &mut output)?;
        return Ok(());
    }

    let mut input_buffer = Vec::new();
    input.read_to_end(&mut input_buffer)?;

    let doc_buffer;
    let doc;

    if let Format::Zdoc = input_format {
        doc = zdoc::Document::from_slice(&input_buffer)?;
    } else {
        let builder = input_format.parse(&input_buffer)?;
        doc_buffer = builder.build();
        doc = &doc_buffer;
    }

    if args.analyze {
        if args.format.is_some_and(|f| f != Format::Zdoc) {
            eprintln!("Warning: Ignoring `--format` when using `--analyze`.");
        }
        if !output.is_std() {
            eprintln!("Warning: Ignoring output file when using `--analyze`.");
        }
        let doc = zdoc::Document::from_slice(&input_buffer)?;
        analyze(doc);
        return Ok(());
    }

    if output.is_tty() && output_format.is_binary() {
        eprintln!("Warning: Writing binary data to a terminal.");
    }
    output_format.emit(&mut output, doc)?;
    Ok(())
}

fn analyze(doc: &zdoc::Document) {
    let zdoc::codec::Header {
        magic: _,
        version,
        root_node_index,
        size,
        nodes_len,
        args_len,
        strings_len,
        binary_len,
        ..
    } = doc.header();
    eprintln!("Document header:");
    eprintln!("  Version:      {version}");
    eprintln!("  Total bytes:  {size:>10}");
    eprintln!("  String bytes: {strings_len:>10}");
    eprintln!("  Binary bytes: {binary_len:>10}");
    eprintln!("  # nodes:      {nodes_len:>10}");
    eprintln!("  # node args:  {args_len:>10}");
    eprintln!("  Root index:   {}", root_node_index);
    eprintln!();

    let mut string_ranges = std::collections::BTreeMap::new();
    let mut register_duplicate = |range| {
        let s = doc.get_string(range).unwrap();
        string_ranges
            .entry(s)
            .or_insert(std::collections::HashSet::new())
            .insert(range);
    };
    for node in doc.nodes() {
        register_duplicate(node.name);
        register_duplicate(node.ty);
    }
    for arg in doc.args() {
        register_duplicate(arg.name);
        if let Ok(zdoc::codec::RawValue::String(range)) = zdoc::codec::RawValue::try_from(arg.value)
        {
            register_duplicate(range)
        }
    }

    let string_duplicates = string_ranges.iter().filter_map(|(s, ranges)| {
        if ranges.len() > 1 && s.len() <= 128 {
            Some(s)
        } else {
            None
        }
    });
    for dup in string_duplicates {
        eprintln!("Warning: Duplicate string below the auto-intern limit: {dup}");
    }
}

fn main() {
    let args = Args::parse();
    match try_main(args) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn guess_format(path: &Path) -> Result<Format, Box<dyn Error>> {
    let Some(ext) = path.extension() else {
        return Err(format!(
            "Could not guess format; file name does not have an extension: {}",
            path.display()
        )
        .into());
    };
    let Some(ext) = ext.to_str() else {
        return Err(UnsupportedFormatError(Path::new(ext).display().to_string()).into());
    };

    Ok(match ext {
        #[cfg(feature = "json")]
        "json" => Format::Json,
        #[cfg(feature = "kdl")]
        "kdl" => Format::Kdl,
        #[cfg(feature = "yaml")]
        "yaml" | "yml" => Format::Yaml,
        #[cfg(feature = "xml")]
        "xml" => Format::Xml,
        #[cfg(feature = "bincode")]
        "bincode" => Format::Bincode,
        #[cfg(feature = "toml")]
        "toml" => Format::Toml,
        "zdoc" => Format::Zdoc,
        _ => {
            return Err(UnsupportedFormatError(ext.to_string()).into());
        }
    })
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Kdl,
    Yaml,
    Xml,
    Bincode,
    Toml,
    Zdoc,
}

impl Format {
    pub fn is_binary(&self) -> bool {
        matches!(self, Format::Bincode | Format::Zdoc)
    }

    pub fn parse<'a>(self, input: &'a [u8]) -> Result<zdoc::Builder<'a>, Box<dyn Error + 'static>> {
        match self {
            #[cfg(feature = "json")]
            Format::Json => zdoc::json::builder_from_json(std::str::from_utf8(input)?)
                .map_err(|e| format!("Failed to parse JSON: {e}").into()),
            #[cfg(not(feature = "json"))]
            Format::Json => Err(UnsupportedFormatError("json".to_string()).into()),
            #[cfg(feature = "kdl")]
            Format::Kdl => zdoc::kdl::builder_from_kdl(std::str::from_utf8(input)?)
                .map_err(|e| format!("Failed to parse KDL: {e}").into()),
            #[cfg(not(feature = "kdl"))]
            Format::Kdl => Err(UnsupportedFormatError("kdl".to_string()).into()),
            #[cfg(feature = "yaml")]
            Format::Yaml => zdoc::yaml::builder_from_yaml(std::str::from_utf8(input)?)
                .map_err(|e| format!("Failed to parse YAML: {e}").into()),
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(UnsupportedFormatError("yaml".to_string()).into()),
            #[cfg(feature = "xml")]
            Format::Xml => zdoc::xml::builder_from_xml(std::str::from_utf8(input)?)
                .map_err(|e| format!("Failed to parse XML: {e}").into()),
            #[cfg(not(feature = "xml"))]
            Format::Xml => Err(UnsupportedFormatError("xml".to_string()).into()),
            // #[cfg(feature = "bincode")]
            // Format::Bincode => todo!(),
            // #[cfg(not(feature = "bincode"))]
            Format::Bincode => Err(UnsupportedFormatError("bincode".to_string()).into()),
            // #[cfg(feature = "toml")]
            // Format::Toml => todo!(),
            // #[cfg(not(feature = "toml"))]
            Format::Toml => Err(UnsupportedFormatError("toml".to_string()).into()),
            Format::Zdoc => {
                let doc = zdoc::Document::from_slice(input)
                    .map_err(|e| format!("Failed to parse zdoc: {e}"))?;
                let builder = zdoc::Builder::from_document(doc);
                Ok(builder)
            }
        }
    }

    pub fn emit<W: std::io::Write>(
        self,
        output: &mut W,
        doc: &zdoc::Document,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            #[cfg(feature = "json")]
            Format::Json => {
                let json = zdoc::json::document_to_json(doc)
                    .map_err(|e| format!("Failed to emit JSON: {e}"))?;
                output.write_all(json.as_bytes())?;
                Ok(())
            }
            #[cfg(not(feature = "json"))]
            Format::Json => Err(UnsupportedFormatError("json".to_string()).into()),
            #[cfg(feature = "kdl")]
            Format::Kdl => {
                let kdl = zdoc::kdl::document_to_kdl(doc)
                    .map_err(|e| format!("Failed to emit KDL: {e}"))?;
                output.write_all(kdl.as_bytes())?;
                Ok(())
            }
            #[cfg(not(feature = "kdl"))]
            Format::Kdl => Err(UnsupportedFormatError("kdl".to_string()).into()),
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let yaml = zdoc::yaml::document_to_yaml(doc)
                    .map_err(|e| format!("Failed to emit YAML: {e}"))?;
                output.write_all(yaml.as_bytes())?;
                Ok(())
            }
            #[cfg(not(feature = "yaml"))]
            Format::Yaml => Err(UnsupportedFormatError("yaml".to_string()).into()),
            #[cfg(feature = "xml")]
            Format::Xml => {
                let xml = zdoc::xml::document_to_xml(doc)
                    .map_err(|e| format!("Failed to emit XML: {e}"))?;
                output.write_all(xml.as_bytes())?;
                Ok(())
            }
            #[cfg(not(feature = "xml"))]
            Format::Xml => Err(UnsupportedFormatError("xml".to_string()).into()),
            // #[cfg(feature = "bincode")]
            // Format::Bincode => todo!(),
            // #[cfg(not(feature = "bincode"))]
            Format::Bincode => Err(UnsupportedFormatError("bincode".to_string()).into()),
            // #[cfg(feature = "toml")]
            // Format::Toml => todo!(),
            // #[cfg(not(feature = "toml"))]
            Format::Toml => Err(UnsupportedFormatError("toml".to_string()).into()),
            Format::Zdoc => output.write_all(doc.as_bytes()).map_err(Into::into),
        }
    }
}

#[derive(Debug)]
struct UnsupportedFormatError(String);
impl std::fmt::Display for UnsupportedFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported format: {}", self.0)
    }
}
impl std::error::Error for UnsupportedFormatError {}
