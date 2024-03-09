use anyhow::Result;
use clap::Parser;
use huffman_markov::Markov;
use std::{
    fs::File,
    io::{copy, stdout, Seek, SeekFrom, Write},
    path::PathBuf,
};

#[derive(Parser)]
pub struct Options {
    #[clap(flatten)]
    global: GlobalOptions,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
pub struct GlobalOptions {}

#[derive(Parser)]
pub enum Command {
    Markov(MarkovOptions),
    Compress(CompressOptions),
}

#[derive(Parser)]
pub struct MarkovOptions {
    #[clap(short, long, default_value = "4")]
    depth: usize,
    file: PathBuf,
}

pub trait Runnable {
    fn run(&self, global: &GlobalOptions) -> Result<()>;
}

impl Runnable for MarkovOptions {
    fn run(&self, global: &GlobalOptions) -> Result<()> {
        let mut markov = Markov::new(self.depth);
        let mut file = File::open(&self.file)?;
        copy(&mut file, &mut markov.writer())?;
        println!("{markov:?}");
        Ok(())
    }
}

#[derive(Parser)]
pub struct CompressOptions {
    #[clap(short, long, default_value = "4")]
    depth: usize,
    file: PathBuf,
}

impl Runnable for CompressOptions {
    fn run(&self, global: &GlobalOptions) -> Result<()> {
        let mut markov = Markov::new(self.depth);
        let mut file = File::open(&self.file)?;
        copy(&mut file, &mut markov.writer())?;

        let encoder = markov.encoder();
        file.seek(SeekFrom::Start(0))?;
        copy(&mut file, &mut encoder.writer(stdout()))?;

        Ok(())
    }
}

impl Runnable for Command {
    fn run(&self, global: &GlobalOptions) -> Result<()> {
        match self {
            Command::Markov(command) => command.run(global),
            Command::Compress(command) => command.run(global),
        }
    }
}

impl Options {
    fn run(&self) -> Result<()> {
        self.command.run(&self.global)
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    options.run()
}
