use std::io::Read;
use crate::memory::{AllocatedRam, Ram, RamError};
use crate::error_creator;
use crate::instructions::{InstructionSet, Instruction};
use crate::operand::Register;
use crate::program::INSTRUCTION_SIZE;

pub type CpuArchitecture = u16;

pub trait FromBytes : Sized {
    fn from(value: [u8; size_of::<Self>()]) -> Self;
}

pub trait IntoBytes : Sized {
    fn into(&self) -> [u8; size_of::<Self>()];
}

macro_rules! impl_bytes_traits {
    ($type:tt) => {
        impl FromBytes for $type {
            fn from(value: [u8; size_of::<Self>()]) -> Self {
                Self::from_ne_bytes(value)
            }
        }
        
        impl IntoBytes for $type {
            fn into(&self) -> [u8; size_of::<Self>()] {
                self.to_ne_bytes()
            }
        }
    };
}

impl_bytes_traits!(u8);
impl_bytes_traits!(CpuArchitecture);
impl_bytes_traits!(u32);

error_creator!(
    CpuError,
    CpuErrorKind,
    ProgramAlreadyRunning => "A program is already running cannot run another",
    ExpectedAnInstruction => "Program was not exited but no more instructions were found",
    InvalidInstruction => "An invalid instruction was given which the cpu does not recognise",
    EndOfProgram => "End of program has been reached",
    RegisterDoesNotExist => "The cpu doesn't have the register",
    StackOverflow => "A stackoverflow has occurred",
    StackUnderflow => "A stack underflow has occurred",
    RamError(RamError) => "",
    Other => ""
);

pub struct Cpu<const S : usize> {
    program_pointer: AllocatedRam,
    program_counter: CpuArchitecture,
    stack_base: AllocatedRam,
    exit_code: CpuArchitecture,
    registers: [CpuArchitecture; S],
    cmp_flag : bool,
}

/// converts a value into a new byte size
/// eg: 
/// ```
/// let value = CpuArchitecture::MAX; // 65535
/// let out = convert_to_byte_size(value, 1); // panics if size > size_of::<CpuArchitecture>()
/// println!("{out}") // 255
/// ```
fn convert_to_byte_size(value: CpuArchitecture, size: u8) -> CpuArchitecture {
    let value_as_bytes = value.to_le_bytes();
    let mut new_slice = [0u8;size_of::<CpuArchitecture>()];
    new_slice[..size as usize].copy_from_slice(&value_as_bytes[..size as usize]);

    CpuArchitecture::from_le_bytes(new_slice)
}

pub(crate) fn read_instruction(read: &mut impl Read) -> Result<(InstructionSet, CpuArchitecture)> {
    let mut bytes = [0u8;size_of::<CpuArchitecture>()];
    let result = read.read_exact(&mut bytes[..INSTRUCTION_SIZE as usize]);
    match result {
        Ok(_) => {},
        Err(_) => return Err(CpuError::new(CpuErrorKind::ExpectedAnInstruction)),
    };
    let instruction_number: CpuArchitecture = FromBytes::from(bytes);
    
    let option = InstructionSet::from_num(instruction_number);
    let mut instruction = match option {
        Some(val) => val,
        None => return Err(CpuError::with_message(CpuErrorKind::InvalidInstruction, format!("instruction number: {}", instruction_number))),
    };

    let result = instruction.initialize(read);
    let size = match result {
        Ok(val) => val,
        Err(err) => return Err(CpuError::with_message(CpuErrorKind::Other, err.to_string()))
    };

    Ok((instruction, size + INSTRUCTION_SIZE))
}

impl<const S : usize> Cpu<S> {
    pub fn new() -> Self {
        if S < 4 {
            panic!("The cpu needs a minimum of 4 registers to run properly currently has {} registers", S)
        }
        Self {
            program_pointer: Default::default(),
            program_counter: 0,
            stack_base: Default::default(),
            exit_code: 0,
            registers: [0; S],
            cmp_flag: true,
        }
    }
    
    pub fn is_running_program(&self) -> bool {
        !self.program_pointer.is_empty()
    }
    
    pub fn initialize_program(&mut self, ram: &mut Ram, program_pointer: AllocatedRam) -> Result<()> {
        if self.is_running_program() {
            Err(CpuError::new(CpuErrorKind::ProgramAlreadyRunning))
        } else {
            self.program_pointer = program_pointer;
            self.program_counter = 0;

            self.exit_code = 0;
            
            let size = ram.size();
            let result = if size > 8192 {
                ram.alloc(2048)
            } else {
                ram.alloc(size / 4)
            };
            
            let stack = match result  {
                Ok(stack) => stack,
                Err(err) => return Err(CpuError::new(CpuErrorKind::RamError(err)))
            };
            
            self.stack_base = stack;
            self.registers[S - 1] = self.stack_base.range().start;
            
            Ok(())
        }
    }
    
    pub fn fetch_instruction(&mut self) -> Result<InstructionSet> {
        if !self.is_running_program() {
            return Err(CpuError::new(CpuErrorKind::EndOfProgram))
        }
        
        let (instruction, size) = read_instruction(&mut self.program_pointer.as_stream(self.program_counter))?;
        self.program_counter += size;
        
        Ok(instruction)
    }
    
    fn check_register_exists(&self, register: Register) -> Result<()> {
        let register_index = register.register_number(S as u8);
        if register_index >= S as u8 {
            Err(CpuError::with_message(CpuErrorKind::RegisterDoesNotExist, register.to_string()))
        } else { 
            Ok(())
        }
    }
    
    pub fn get_register(&self, register: Register) -> Result<CpuArchitecture> {
        self.check_register_exists(register)?;
        
        let register_index = register.register_number(S as u8);
        let register_value = self.registers[register_index as usize];
        let register_size = register.register_size();
        
        // convert into smaller type if needed
        Ok(convert_to_byte_size(register_value, register_size))
    }
    
    pub fn set_register(&mut self, register: Register, value: CpuArchitecture) -> Result<()> {
        self.check_register_exists(register)?;
        
        let register_index = register.register_number(S as u8);
        let register_size = register.register_size();
        let value= convert_to_byte_size(value, register_size);
        self.registers[register_index as usize] = value;
        
        Ok(())
    }
    
    pub fn get_program_counter(&self) -> CpuArchitecture {
        self.program_counter
    }

    pub fn set_program_counter(&mut self, program_counter: CpuArchitecture) {
        self.program_counter = program_counter;
    }
    
    pub fn push<T : Sized + IntoBytes>(&mut self, value: &T) -> Result<()>
        where [();size_of::<T>()]:
    {
        self.push_buffer(&IntoBytes::into(value))
    }
    
    fn get_stack_pointer(&self) -> CpuArchitecture {
        self.registers[S - 1] - self.stack_base.range().start
    }

    pub fn push_buffer(&mut self, buffer: &[u8]) -> Result<()> {
        let result = self.stack_base.write_buffer_at(self.get_stack_pointer(), buffer);
        if result.is_err() {
            return Err(CpuError::new(CpuErrorKind::StackOverflow))
        }
        self.registers[S - 1] += buffer.len() as CpuArchitecture;
        Ok(())
    }

    pub fn pop<T : Sized + FromBytes>(&mut self) -> Result<T>
        where [();size_of::<T>()]:
    {
        let mut temp = [0u8; size_of::<T>()];
        self.pop_buffer(&mut temp)?;
        Ok(FromBytes::from(temp))
    }

    pub fn pop_buffer(&mut self, buffer: &mut [u8]) -> Result<()> {
        let option = self.get_stack_pointer().checked_sub(buffer.len() as CpuArchitecture);
        if option.is_none() {
            return Err(CpuError::new(CpuErrorKind::StackUnderflow));
        }
        self.registers[S - 1] -= buffer.len() as CpuArchitecture;
        self.stack_base.read_buffer_at(self.get_stack_pointer(), buffer)?;
        Ok(())
    }
    
    pub fn exit_program(&mut self) {
        // cpu is expected to have at least 4 registers
        let value = self.registers[0];
        
        self.exit_code = value;
        self.program_pointer = Default::default();
        self.stack_base = Default::default();
    }
    
    pub fn exit_code(&self) -> CpuArchitecture {
        self.exit_code
    }
    
    pub fn get_cmp_flag(&mut self) -> bool {
        let flag = self.cmp_flag;
        self.cmp_flag = true;
        flag
    }
    
    pub fn set_cmp_flag(&mut self, expr:bool) {
        self.cmp_flag = expr;
    }
}



