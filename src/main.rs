#![feature(generic_const_exprs)]
#![feature(pattern)]
extern crate core;

use clap::Parser;
use clap_derive::{Parser, Subcommand};
use crate::compile::{build, run};
use crate::cpu::CpuArchitecture;

mod instructions;
mod cpu;
mod computer;
mod memory;
mod program;
mod error;
mod array;
mod operand;
mod read_ext;
mod instruction_iter;
mod write_ext;
mod file_handler;
mod display_handler;
mod compile;
mod pattern_ignore_case;
mod dependency;
mod window;
mod break_point;

#[derive(Subcommand)]
enum Commands {
    /// run a assembly or binary file
    Run {
        /// the path to an assembly or binary file that will be run
        path: String,
        /// the amount of memory that the emulator will have
        #[arg(short, long, default_value_t = 1024)]
        memory_amount: CpuArchitecture,
        /// indicate that the emulator should run in debug mode
        #[arg(short, long)]
        debug:bool,
    },
    /// build an assembly into a binary file
    Build { 
        /// the path to an assembly file that will be build
        path: String,
        /// the path where the compiled file will be saved [optional]
        #[arg(short = 'o')]
        out: Option<String>
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands
}

fn main() {
    let arguments = Args::parse();
    
    match arguments.command {
        Commands::Build { path, out } => build(path, out),
        Commands::Run { path, memory_amount, debug } => run(path, memory_amount, debug),
    }
}
