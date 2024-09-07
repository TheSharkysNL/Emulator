use std::io::{stdout, Write};
use std::time::Instant;
use crate::break_point::BreakPoint;
use crate::compile::DEBUG;
use crate::cpu::{Cpu, CpuError, CpuErrorKind};
use crate::memory::Ram;
use crate::error_creator;
use crate::program::{Program, ProgramError};
use crate::instructions::{Instruction, InstructionError};

error_creator!(
    ComputerError,
    ComputerErrorKind,
    ProgramError(ProgramError) => "",
    CpuError(CpuError) => "",
    InstructionError(InstructionError) => "",
    Other => ""
);

pub const REGISTER_COUNT: usize = 12;

pub struct Computer {
    cpu: Cpu<REGISTER_COUNT>,
    ram: Ram,
}

impl Computer {
    pub fn new(cpu: Cpu<REGISTER_COUNT>, ram: Ram) -> Self {
        Self {
            cpu, 
            ram,
        }
    }
    
    pub fn start_program(&mut self, program: Program) -> Result<()> {
        let result = program.allocate(&mut self.ram);
        
        let instructions = match result {
            Ok(instructions) => instructions,
            Err(err) => return Err(ComputerError::new(ComputerErrorKind::ProgramError(err))),
        };
        
        let result = self.cpu.initialize_program(&mut self.ram, instructions);
        if let Err(err) = result {
            return Err(ComputerError::new(ComputerErrorKind::CpuError(err)));
        }
        
        let instant = Instant::now();
        
        loop {
            let result = self.execute_next_instruction();
            let exited = match result {
                Ok(exited) => exited,
                Err(err) => {
                    if DEBUG.get() {
                        println!("An error occurred whilst running program: {}. Starting a breakpoint", err.to_string());
                        self.breakpoint()?;
                    }
                    
                    return Err(err);
                },
            };
            
            if exited {
                break;
            }
        }
        
        println!("program exited with exit code: {}, time to run: {} ms", self.cpu.exit_code(), instant.elapsed().as_nanos() as f64 / 1e6);
        self.ram.deallocate_all();
        
        Ok(())
    }
    
    /// executes next instruction if true the program has exited
    pub fn execute_next_instruction(&mut self) -> Result<bool> {
        let result = self.cpu.fetch_instruction();
        let instruction = match result {
            Ok(instruction) => instruction,
            Err(err) => {
                if err.kind() == &CpuErrorKind::EndOfProgram {
                    return Ok(true);
                }
                return Err(ComputerError::new(ComputerErrorKind::CpuError(err)));
            }
        };

        instruction.execute(self)?;
        Ok(false)
    }
    
    pub fn breakpoint(&mut self) -> Result<()> {
        BreakPoint::create_breakpoint(self)
    }

    pub fn print_bytes(buffer: &[u8]) -> std::io::Result<()> {
        let mut stdout = stdout();
        stdout.write_all("{ ".as_bytes())?;

        if !buffer.is_empty() {
            stdout.write_all(format!("0x{:X}", buffer[0]).as_bytes())?;

            for value in buffer.iter().skip(1) {
                stdout.write_all(", ".as_bytes())?;
                stdout.write_all(format!("0x{:X}", value).as_bytes())?;
            }
        }

        stdout.write_all(" }\n".as_bytes())
    }
    
    pub fn cpu(&self) -> &Cpu<REGISTER_COUNT> {
        &self.cpu
    }
    
    pub fn cpu_mut(&mut self) -> &mut Cpu<REGISTER_COUNT> {
        &mut self.cpu
    }

    pub fn ram(&self) -> &Ram {
        &self.ram
    }
    
    pub fn ram_mut(&mut self) -> &mut Ram {
        &mut self.ram
    }
}