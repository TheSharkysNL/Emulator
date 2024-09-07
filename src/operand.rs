use std::fmt::{Display, Formatter, Write};
use crate::cpu::CpuArchitecture;
use std::result::Result;
use std::str::FromStr;
use std::io::{Read as IORead, Write as IOWrite};
use crate::computer::Computer;
use crate::instructions::{InstructionError, InstructionErrorKind};
use crate::read_ext::ReadLine;
use crate::write_ext::WriteExt;

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct Register {
    register: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct Literal {
    literal: CpuArchitecture
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct Pointer {
    value: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct RegisterPointer {
    pointer: Pointer,
    register: Register
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub struct LiteralPointer {
    pointer: Pointer,
    literal: Literal,
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug)]
pub enum Operand {
    #[default]
    Nop,
    Register(Register),
    Literal(Literal),
    RegisterPointer(RegisterPointer),
    LiteralPointer(LiteralPointer),
}

const POINTER_PART: u8 = u8::MAX - size_of::<CpuArchitecture>().ilog2() as u8;
const LITERAL_PART: u8 = POINTER_PART - 1;
const REGISTER_CHARACTERS:[char;4] = ['l', 'x', 'e', 'r'];
const POINTER_STRINGS:[&str;4] = ["byte", "word", "dword", "qword"];
pub const STACK_POINTER_REGISTER: u8 = LITERAL_PART - 1;
const STACK_POINTER_NAME: &str = "sp";

impl Operand {
    pub fn from_stream(stream: &mut impl IORead) -> std::io::Result<Operand> {
        let lower = stream.read_type::<u8>()?;
        
        fn get_literal_or_register(lower: u8, stream: &mut impl IORead) -> std::io::Result<Operand> {
            if lower == LITERAL_PART {
                let literal = stream.read_type::<CpuArchitecture>()?;
                Ok(Operand::Literal(
                    Literal {
                        literal,
                    }
                ))
            } else {
                Ok(Operand::Register(
                    Register {
                        register: lower,
                    }
                ))
            }
        }
        
        if lower >= POINTER_PART {
            let upper = stream.read_type::<u8>()?;
            
            let operand = get_literal_or_register(upper, stream)?;
            let pointer = Pointer { value: lower };
            Ok(match operand {
                Operand::Register(reg) => Operand::RegisterPointer(RegisterPointer::new(reg, pointer)),
                Operand::Literal(lit) => Operand::LiteralPointer(LiteralPointer::new(lit, pointer)),
                _ => unreachable!("should be unreachable as get_literal_or_register should only return a register or literal"),
            })
        } else {
            get_literal_or_register(lower, stream)
        }
    }
    
    pub fn write_to_stream(self, stream: &mut impl IOWrite) -> std::io::Result<CpuArchitecture> {
        match self {
            Operand::Register(register) => register.write_to_stream(stream),
            Operand::Literal(literal) => literal.write_to_stream(stream),
            Operand::LiteralPointer(literal_pointer) => {
                stream.write_type(&literal_pointer.pointer.value)?;
                Ok(literal_pointer.literal.write_to_stream(stream)? +
                    size_of_val(&literal_pointer.pointer.value) as CpuArchitecture)
            },
            Operand::RegisterPointer(register_pointer) => {
                stream.write_type(&register_pointer.pointer.value)?;
                Ok(register_pointer.register.write_to_stream(stream)? +
                    size_of_val(&register_pointer.pointer.value) as CpuArchitecture)
            }
            Operand::Nop => Ok(0),
        }
    }
    
    pub fn binary_size(self) -> CpuArchitecture {
        match self {
            Operand::Register(_) => Register::binary_size(),
            Operand::Literal(_) => Literal::binary_size(),
            Operand::LiteralPointer(_) => Literal::binary_size() + Pointer::binary_size(),
            Operand::RegisterPointer(_) => Register::binary_size() + Pointer::binary_size(),
            Operand::Nop => 0,
        }
    }
    
    pub fn size(self) -> CpuArchitecture {
        match self {
            Operand::Register(register) => register.register_size() as CpuArchitecture,
            Operand::Literal(_) => size_of::<CpuArchitecture>() as CpuArchitecture,
            Operand::LiteralPointer(_) => size_of::<CpuArchitecture>() as CpuArchitecture,
            Operand::RegisterPointer(pointer) => pointer.pointer.pointed_to_size(),
            Operand::Nop => 0,
        }
    }
    
    pub fn read_from_computer(self, computer: &Computer) -> Result<CpuArchitecture, InstructionError> {
        Ok(match self {
            Operand::Register(register) => computer.cpu().get_register(register)?,
            Operand::RegisterPointer(register_pointer) => {
                let register_value = computer.cpu().get_register(register_pointer.register())?;
                register_pointer.pointer.get_pointed_to_value(register_value, computer)?
            },
            Operand::LiteralPointer(literal_pointer) => {
                literal_pointer.pointer.get_pointed_to_value(literal_pointer.address(), computer)?
            },
            Operand::Literal(literal) => literal.literal(),
            Operand::Nop => return Err(InstructionError::new(InstructionErrorKind::OperandNop)),
        })
    }

    pub fn write_to_computer(self, computer: &mut Computer, value: CpuArchitecture) -> Result<(), InstructionError> {
        match self {
            Operand::Register(register) => computer.cpu_mut().set_register(register, value)?,
            Operand::RegisterPointer(register_pointer) => {
                let register_value = computer.cpu().get_register(register_pointer.register())?;
                register_pointer.pointer.set_pointed_to_value(register_value, computer, value)?;
            },
            Operand::LiteralPointer(literal_pointer) => {
                literal_pointer.pointer.set_pointed_to_value(literal_pointer.address(), computer, value)?;
            },
            _ => return Err(InstructionError::new(InstructionErrorKind::DestinationInvalid)),
        };
        Ok(())
    }
}

impl FromStr for Operand {
    type Err = InstructionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed_str = s.trim();
        if trimmed_str.is_empty() {
            return Err(InstructionError::new(InstructionErrorKind::InvalidOperandString));
        }
        
        fn get_register_or_literal(s: &str) -> Result<Operand, InstructionError> {
            if s == STACK_POINTER_NAME {
                return Ok(Operand::Register(Register::stack_pointer()));
            }
            
            let first_char = s.as_bytes()[0];
            if let Some(index) = REGISTER_CHARACTERS.iter().position(| val | {
                val.to_lowercase().eq((first_char as char).to_lowercase())
            }) {
                let size = (2 as CpuArchitecture).pow(index as u32);
                let result = u8::from_str(&s[1..]);
                return match result {
                    Ok(val) => Ok(Operand::Register(Register::new(val - 1, size as u8))),
                    Err(_) => Err(InstructionError::new(InstructionErrorKind::InvalidOperandString)),
                }
            }

            let (base, stripped) = if let Some(stripped) = s.strip_prefix("0b") {
                (2, stripped)
            } else if let Some(stripped) = s.strip_prefix("0x") {
                (16, stripped)
            } else if let Some(stripped) = s.strip_prefix("0o") {
                (8, stripped)
            } else {
                (10, s)
            };
            
            if let Ok(val) = CpuArchitecture::from_str_radix(stripped, base) {
                return Ok(Operand::Literal(Literal::new(val)));
            }

            Err(InstructionError::new(InstructionErrorKind::InvalidOperandString))
        }
        
        let option =  trimmed_str.find('[');
        match option { 
            Some(index) => {
                let pointer_str = trimmed_str[..index].trim();
                if let Some(size_log2) = POINTER_STRINGS.iter().position(|val | {
                    val.eq_ignore_ascii_case(pointer_str)
                }) {
                    let size = (2 as CpuArchitecture).pow(size_log2 as u32);
                    let pointer = Pointer::new(size as u8);
                    if *trimmed_str.as_bytes().last().unwrap() != b']' {
                        Err(InstructionError::new(InstructionErrorKind::InvalidOperandString))
                    } else {
                        let inner_value = &trimmed_str[index + 1..trimmed_str.len() - 1];
                        let operand = get_register_or_literal(inner_value)?;
                        match operand {
                            Operand::Literal(literal) => Ok(Operand::LiteralPointer(LiteralPointer::new(literal, pointer))),
                            Operand::Register(register) => Ok(Operand::RegisterPointer(RegisterPointer::new(register, pointer))),
                            _ => unreachable!("the get_register_or_literal function should only return a literal or register"),
                        }
                    }
                } else {
                    get_register_or_literal(trimmed_str)
                }
            },
            None => get_register_or_literal(trimmed_str),
        }
        
    }
}

impl Display for Operand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Register(register) => register.fmt(f),
            Operand::Literal(literal) => literal.literal().fmt(f),
            Operand::RegisterPointer(register_pointer) => {
                let index = register_pointer.pointer.pointed_to_size().ilog2();
                f.write_str(POINTER_STRINGS[index as usize])?;
                f.write_char('[')?;
                register_pointer.register.fmt(f)?;
                f.write_char(']')
            },
            Operand::LiteralPointer(literal_pointer) => {
                let index = literal_pointer.pointer.pointed_to_size().ilog2();
                f.write_str(POINTER_STRINGS[index as usize])?;
                f.write_char('[')?;
                literal_pointer.literal.literal().fmt(f)?;
                f.write_char(']')
            },
            Operand::Nop => f.write_str("Nop"),
        }
    }
}

impl Register {
    pub fn new(index:u8, size:u8) -> Self {
        if size == 0 {
            panic!("size of register cannot be 0");
        } else if size > size_of::<CpuArchitecture>() as u8 {
            panic!("size of register cannot be greater than {}", size_of::<CpuArchitecture>());
        }
        Self {
            register: index * size_of::<CpuArchitecture>() as u8 + (size.ilog2() as u8 + 1)
        }
    }
    
    pub fn stack_pointer() -> Self {
        Self {
            register: STACK_POINTER_REGISTER,
        }
    }
    
    const fn parts_per_register() -> u8 {
        size_of::<CpuArchitecture>().ilog2() as u8 + 1
    }
    
    pub fn register_number(self, cpu_size: u8) -> u8 {
        if self.is_stack_pointer() {
            cpu_size - 1
        } else {
            let parts = Self::parts_per_register();
            self.register / parts
        }
    }
    
    pub fn register_size(self) -> u8 {
        let parts = Self::parts_per_register();
        2u8.pow((parts - self.register % parts - 1) as u32)
    }

    pub fn write_to_stream(self, stream: &mut impl IOWrite) -> std::io::Result<CpuArchitecture> {
        stream.write_type(&self.register)?;

        Ok(size_of_val(&self.register) as CpuArchitecture)
    }
    
    pub const fn binary_size() -> CpuArchitecture {
        size_of::<u8>() as CpuArchitecture
    }
    
    pub fn is_stack_pointer(self) -> bool {
        self.register == STACK_POINTER_REGISTER
    }
}

impl Display for Register {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_stack_pointer() {
            f.write_str(STACK_POINTER_NAME)
        } else {
            let size = self.register_size();
            let char = REGISTER_CHARACTERS[size.ilog2() as usize];

            let index = self.register_number(u8::MAX) + 1;

            f.write_char(char)?;
            index.fmt(f)
        }
    }
}

impl Literal {
    pub fn new(literal: CpuArchitecture) -> Self {
        Self {
            literal
        }
    }
    
    pub fn literal(self) -> CpuArchitecture {
        self.literal
    }
    
    pub fn write_to_stream(self, stream: &mut impl IOWrite) -> std::io::Result<CpuArchitecture> {
        stream.write_type(&LITERAL_PART)?;
        stream.write_type(&self.literal)?;
        
        Ok((size_of_val(&LITERAL_PART) + size_of_val(&self.literal)) as CpuArchitecture)
    }
    
    pub const fn binary_size() -> CpuArchitecture {
        (size_of::<u8>()  + size_of::<CpuArchitecture>()) as CpuArchitecture
    }
}

impl Pointer {
    pub fn new(size:u8) -> Self {
        Self {
            value: size - 1 + POINTER_PART,
        }
    }
    
    pub fn pointed_to_size(self) -> CpuArchitecture {
        let pow = self.value - POINTER_PART;
        (2 as CpuArchitecture).pow(pow as u32)
    }
    
    pub const fn binary_size() -> CpuArchitecture {
        size_of::<u8>() as CpuArchitecture
    }
    
    pub fn get_pointed_to_value(self, index: CpuArchitecture, computer: &Computer) -> Result<CpuArchitecture, InstructionError> {
        let mut buffer = [0u8;size_of::<CpuArchitecture>()];
        let sized_buffer = &mut buffer[..self.pointed_to_size() as usize];

        computer.ram().read_buffer_at_checked(index, sized_buffer)?;
        Ok(CpuArchitecture::from_ne_bytes(buffer))
    }
    
    pub fn set_pointed_to_value(self, index: CpuArchitecture, computer: &mut Computer, value: CpuArchitecture) -> Result<(), InstructionError> {
        let bytes = value.to_ne_bytes();
        let sized_bytes = &bytes[..self.pointed_to_size() as usize];

        computer.ram_mut().write_buffer_at_checked(index, sized_bytes)?;
        Ok(())
    }
}

impl LiteralPointer {
    pub fn new(literal: Literal, pointer: Pointer) -> Self {
        Self {
            pointer,
            literal,
        }
    }
    
    pub fn pointed_to_size(self) -> CpuArchitecture {
        self.pointer.pointed_to_size()
    }
    
    pub fn address(self) -> CpuArchitecture {
        self.literal.literal()
    }

    pub fn pointer(self) -> Pointer {
        self.pointer
    }
}

impl RegisterPointer {
    pub fn new(register: Register, pointer: Pointer) -> Self {
        Self {
            pointer,
            register,
        }
    }
    
    pub fn pointed_to_size(self) -> CpuArchitecture {
        self.pointer.pointed_to_size()
    }

    pub fn register(self) -> Register {
        self.register
    }
    
    pub fn pointer(self) -> Pointer {
        self.pointer
    }
}