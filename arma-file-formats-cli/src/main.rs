#![warn(missing_docs)]
#![warn(clippy::cargo)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![allow(missing_docs)]

use std::{
    borrow::Cow,
    collections::HashMap,
    env::current_dir,
    fmt::Write,
    fs::{self, File},
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Components, Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use arma_file_formats::real_virtuality::pbo::{Pbo, PboReader};
use clap::{command, Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use prettytable::{format, row, Table};
use ptree::{print_tree, TreeItem};
use sha1::{Digest, Sha1};

#[derive(Debug, Parser)]
#[command(name = "aff")]
#[command(about = "aff", long_about = None)]
#[command(author, version, about)]
#[command(next_line_help = true)]
struct Aff {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(arg_required_else_help = true)]
    #[command(about = "PBO means 'packed bank of files'. A .pbo is identical in purpose to a zip or rar. It is a container for folder(s) and file(s).", long_about = None)]
    Pbo(PboArgs),
}

impl Commands {
    fn execute(&self) -> Result<()> {
        match self {
            Self::Pbo(args) => handle_pbo(args),
        }
    }
}

#[derive(Clone)]
struct PboEntry {
    name: String,
    children: HashMap<String, PboEntry>,
}

impl TreeItem for PboEntry {
    type Child = Self;

    fn write_self<W: std::io::Write>(
        &self,
        f: &mut W,
        style: &ptree::Style,
    ) -> std::io::Result<()> {
        write!(f, "{}", style.paint(&self.name))
    }

    fn children(&self) -> Cow<[Self::Child]> {
        let childs: Vec<Self> = self.children.values().cloned().collect();
        Cow::from(childs)
    }
}

fn insert(entry: &mut PboEntry, components: &mut Components) {
    if let Some(comp) = components.next() {
        let path = comp.as_os_str().to_str().unwrap_or_default().to_string();
        if let Some(val) = entry.children.get_mut(&path) {
            insert(val, components);
        } else {
            let mut new_entry = PboEntry {
                name: path.clone(),
                children: HashMap::new(),
            };
            insert(&mut new_entry, components);
            entry.children.insert(path, new_entry);
        }
    }
}

fn handle_pbo(args: &PboArgs) -> std::prelude::v1::Result<(), anyhow::Error> {
    match &args.command {
        PboCommands::List { args } => {
            let input = &args.input;
            let pbo = PboReader::from_stream(BufReader::new(File::open(input)?))?;

            let mut entry = PboEntry {
                name: Path::new(input.as_str())
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
                children: HashMap::new(),
            };
            for (name, _) in pbo.pbo.entries {
                let path = Path::new(&name);
                insert(&mut entry, &mut path.components());
            }
            print_tree(&entry)?;
        }
        PboCommands::HashCheck { args } => {
            const BUF_SIZE: usize = 1024;
            let input = &args.input;
            let mut pbo = File::open(input)?;

            let data_len = pbo.seek(SeekFrom::End(-20))?;
            let mut hash = [0; 20];
            assert_eq!(pbo.read(&mut hash)?, 20);

            pbo.seek(SeekFrom::Start(0))?;

            let mut pbo_data = pbo.by_ref().take(data_len - 1);

            let mut hasher = Sha1::new();
            let mut buffer = [0; BUF_SIZE];

            loop {
                let count = pbo_data.read(&mut buffer)?;
                if count == 0 {
                    break;
                }

                hasher.update(&buffer[..count]);
            }

            let calc_hash: [u8; 20] = hasher.finalize().into();
            println!("{0: <20} {1: <30}", "Read hash:", hex::encode(hash));
            println!(
                "{0: <20} {1: <30}",
                "Calculated hash:",
                hex::encode(calc_hash)
            );

            println!();

            if hash.to_vec() == calc_hash {
                println!("Hash matches");
            } else {
                println!("Hash doesn't match");
            }
        }
        PboCommands::Meta { args } => {
            let input = &args.input;
            let pbo = PboReader::from_stream(BufReader::new(File::open(input)?))?;

            println!("PBO properties:");

            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

            table.set_titles(row!["Key", "Value"]);

            for (key, value) in pbo.pbo.properties {
                table.add_row(row![key, value]);
            }

            table.printstd();
        }
        PboCommands::Extract { args } => {
            extract_pbo(args, false)?;
        }
        PboCommands::ExtractFlat { args } => {
            extract_pbo(args, true)?;
        }
    };

    Ok(())
}

fn extract_pbo(args: &InputOutputCommandArgs, flat: bool) -> Result<(), anyhow::Error> {
    let input = &args.input;
    println!("Reading PBO...");
    let pbo = Pbo::from_path(input)?;
    println!("Extracting files...");
    let data_size: usize = pbo.entries.iter().map(|(_, e)| e.data.len()).sum();
    let pb = ProgressBar::new(data_size.try_into()?);
    pb
        .set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));
    let base_dir = if let Some(output) = &args.output {
        PathBuf::from_str(output)?
    } else {
        current_dir()?
    };
    let mut written = 0;
    for (name, entry) in pbo.entries {
        let mut cd = base_dir.clone();
        if flat {
            let file_name = PathBuf::from_str(&name)?;
            let file_name = file_name.file_name().unwrap_or_default();
            cd.push(file_name);
        } else {
            cd.push(name);
            if let Some(parent) = cd.parent() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(cd, &entry.data)?;
        written += entry.data.len();
        pb.set_position(written.try_into()?);
    }
    pb.finish_with_message("extracting complete");
    Ok(())
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
#[command(flatten_help = true)]
pub struct PboArgs {
    #[command(subcommand)]
    command: PboCommands,
}

#[derive(Debug, Subcommand)]
pub enum PboCommands {
    #[command(about = "List all files and folders.", long_about = None)]
    List {
        #[command(flatten)]
        args: InputCommandArgs,
    },
    #[command(about = "Checks the hash.", long_about = None)]
    HashCheck {
        #[command(flatten)]
        args: InputCommandArgs,
    },
    #[command(about = "Prints the properties", long_about = None)]
    Meta {
        #[command(flatten)]
        args: InputCommandArgs,
    },

    #[command(about = "Extracts the file", long_about = None)]
    Extract {
        #[command(flatten)]
        args: InputOutputCommandArgs,
    },

    #[command(about = "Extracts all files into the current dir", long_about = None)]
    ExtractFlat {
        #[command(flatten)]
        args: InputOutputCommandArgs,
    },
}

#[derive(Args, Debug, Clone)]
pub struct InputCommandArgs {
    input: String,
}

#[derive(Args, Debug, Clone)]
pub struct InputOutputCommandArgs {
    input: String,
    output: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct CommandArgs {
    input: String,
    #[arg(short, long)]
    output: String,
}

fn main() -> Result<()> {
    let cli = Aff::parse();

    cli.command.execute()?;

    Ok(())
}
