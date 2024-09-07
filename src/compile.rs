use std::cell::Cell;
use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Seek, SeekFrom};
use std::path::Path;
use crate::computer::{Computer, REGISTER_COUNT};
use crate::cpu::{Cpu, CpuArchitecture};
use crate::memory::Ram;
use crate::program::{DEPENDENCY_EXTENSION, Program};

thread_local! {
    pub static DEBUG: Cell<bool> = const { Cell::new(false) };
}

pub fn build(path: String, out: Option<String>){
    let path = Path::new(&path);
    let out = out.unwrap_or_else(| | {
        path.with_extension(&DEPENDENCY_EXTENSION[1..]).to_str().unwrap().to_string()
    });

    let result = OpenOptions::new().read(true).open(path);
    let file = match result {
        Ok(file) => file,
        Err(err) => { println!("could not read file: {}, filename: {}", err, path.display()); return; }
    };
    let mut buf_reader = BufReader::with_capacity(4096, file);
    let result = Program::from_stream(&mut buf_reader);
    let program = match result {
        Ok(program) => program,
        Err(err) => { println!("could not compile program: {}", err); return; }
    };

    let out = Path::new(&out);
    let result = OpenOptions::new().write(true).create(true).truncate(true).open(out);
    let file = match result {
        Ok(file) => file,
        Err(err) => { println!("could not write to file: {}, filename: {}", err, out.display()); return; }
    };
    let mut buf_writer = BufWriter::with_capacity(4096, file);
    let result = program.write_as_library(&mut buf_writer);
    match result {
        Ok(val) => val,
        Err(err) => { println!("unable to write program to file: {}", err); return; }
    };
    
    println!("file has been successfully build and is stored at {}", out.display());
}

pub fn run(path: String, memory_amount: CpuArchitecture, debug: bool) {
    DEBUG.set(debug);
    
    let mem = Ram::new(memory_amount);
    let cpu = Cpu::<REGISTER_COUNT>::new();

    let mut computer = Computer::new(cpu, mem);

    let path = Path::new(&path);
    let result = OpenOptions::new().read(true).open(path);
    let file = match result {
        Ok(file) => file,
        Err(err) => { println!("could not read from file: {}, filename: {}", err, path.display()); return; }
    };

    let mut buf_reader = BufReader::with_capacity(4096, file);
    let result = if path.extension().unwrap_or("".as_ref()).eq(&DEPENDENCY_EXTENSION[1..]) {
        Program::from_binary(&mut buf_reader)
    } else {
        Program::from_stream(&mut buf_reader)
    };
    let program = match result {
        Ok(program) => program,
        Err(err) => { println!("could not compile program: {}", err); return; }
    };

    let result = computer.start_program(program);
    match result {
        Ok(_) => {},
        Err(err) => {
            println!("an error occurred while running emulator: {}", err);
            if debug {
                let result = buf_reader.seek(SeekFrom::Start(0));
                if let Err(err) = result {
                    println!("could not find the line where the error occurred: {}", err);
                } else {
                    let result = Program::get_line(computer.cpu().get_program_counter(), &mut buf_reader);
                    match result {
                        Ok((line_number, line)) => println!("the error occurred on the line: {}, \"{}\"", line_number, line.trim()),
                        Err(err) => println!("could not find the line where the error occurred: {}", err),
                    }
                }
            }
        }
    };
}