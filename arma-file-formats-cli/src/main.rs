#![warn(missing_docs)]
#![warn(clippy::cargo)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![allow(missing_docs)]

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};

use anyhow::Result;
use arma_file_formats::real_virtuality::pbo::PboReader;
use clap::{command, Args, Parser, Subcommand};
use prettytable::{format, row, Table};
use sha1::Digest;
use sha1::Sha1;

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

fn handle_pbo(args: &PboArgs) -> std::prelude::v1::Result<(), anyhow::Error> {
    //let mut pbo = File::open(&args.input)?;

    match &args.command {
        PboCommands::List { input } => {
            let pbo = PboReader::from_stream(BufReader::new(File::open(input)?))?;

            println!("PBO entries:");
            for (name, _) in pbo.pbo.entries {
                println!("{name}");
            }
        }
        PboCommands::HashCheck { input } => {
            const BUF_SIZE: usize = 1024;
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
        PboCommands::Meta { input } => {
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
    };

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
    List { input: String },
    #[command(about = "Checks the hash.", long_about = None)]
    HashCheck { input: String },
    #[command(about = "Prints the properties", long_about = None)]
    Meta { input: String },
}

#[derive(Args, Debug, Clone)]
pub struct CommandArgs {
    #[arg(short, long)]
    input: String,
    #[arg(short, long)]
    output: String,
}

fn main() -> Result<()> {
    let cli = Aff::parse();

    cli.command.execute()?;

    Ok(())
}
