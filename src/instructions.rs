use std::cell::Cell;
use std::fmt::{Display, Formatter, Write};
use std::io::{Write as IOWrite, Read as IORead};
use std::str::FromStr;
use enum_dispatch::enum_dispatch;
use strum::AsRefStr;
use crate::computer::Computer;
use crate::cpu::{CpuArchitecture, CpuError, IntoBytes, FromBytes};
use crate::memory::RamError;
use crate::operand::{Literal, Operand, Register};
use crate::error_creator;
use num_derive::{ToPrimitive, FromPrimitive};
use num_traits::FromPrimitive;
use crate::compile::DEBUG;
use crate::window::Window;

error_creator!(
    InstructionError,
    InstructionErrorKind,
    OperandNop => "Operand cannot be Nop",
    DestinationInvalid => "Destination either has to be a register, register pointer or literal pointer",
    RamError(RamError) => "",
    CpuError(CpuError) => "",
    StringInstructionNotFound => "The instruction given was not found",
    InvalidOperandString => "The operand is invalid",
    InvalidOperandCount => "The string provided doesn't have the valid operand count for the instruction",
    SyscallFunctionNotFound => "The syscall function number is not found",
    PrintError => "an error occurred while printing",
    WindowAlreadyCreated => "cannot create multiple windows, a window already exists",
    Other => ""
);

pub trait Is {
    type Other;

    fn is(other:&Self::Other) -> Option<Self> where Self: Sized;
}

#[enum_dispatch]
pub trait Instruction : Clone + Copy + FromStr + Display + Is {
    fn execute(self, computer: &mut Computer) -> Result<()>;

    fn binary_size(self) -> CpuArchitecture;

    fn to_binary(self, stream: &mut impl IOWrite) -> std::io::Result<CpuArchitecture>;

    fn initialize(&mut self, stream: &mut impl IORead) -> std::io::Result<CpuArchitecture>;
}

macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {$sub};
}

macro_rules! count_tts {
    ($($tts:tt)*) => {{<[()]>::len(&[$(replace_expr!($tts ())),*])}};
}

macro_rules! compute_recursive {
    ($func_name:ident, $left:tt) => {{
        $left
    }};
    (
        $func_name:ident,
        $left:tt
        $($tts:tt)*
    ) => {{
        $func_name($left, compute_recursive!($func_name, $($tts)*))
    }};
}

macro_rules! create_instructions {
    ($($val:ident => $literal:literal),*) => {
        #[enum_dispatch(Instruction)]
        #[derive(AsRefStr, Clone, Copy, Debug)]
        pub enum InstructionSet {
            $($val),*
        }
        
        impl InstructionSet {
            pub fn to_num(self) -> CpuArchitecture {
                match self {
                    $(InstructionSet::$val(_) => $literal),*
                }
            }
            
            pub fn from_num(num: CpuArchitecture) -> Option<InstructionSet> {
                match num {
                    $($literal => Some($val::default().into())),*,
                    _ => None
                }
            }
            
            pub const fn max_instruction_number() -> CpuArchitecture {
                const fn max(a: usize, b: usize) -> usize {
                    [a, b][(a < b) as usize]
                }
                let max = compute_recursive!(max, $($literal)*); 
                max as CpuArchitecture
            }
        }
        
        impl std::str::FromStr for InstructionSet {
            type Err = InstructionError;
        
            fn from_str(str: &str) -> std::result::Result<Self, Self::Err> {
                $(
                    if str.len() >= stringify!($val).len() && str[..stringify!($val).len()].eq_ignore_ascii_case(stringify!($val)) {
                        return Ok($val::from_str(&str[stringify!($val).len()..])?.into());
                    }
                )*
                return Err(InstructionError::with_message(InstructionErrorKind::StringInstructionNotFound, format!("line: \"{}\"", str)));
            }
        }
        
        impl std::fmt::Display for InstructionSet {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                 match self {
                    $(InstructionSet::$val(val) => { 
                        f.write_str(stringify!($val))?;
                        f.write_char(' ')?;
                        val.fmt(f) 
                    }),*
                }
            }
        }
        
        impl Is for InstructionSet {
            type Other = InstructionSet;
            
            fn is(_: &Self::Other) -> Option<Self> {
                None
            }
        }
    };
}

macro_rules! empty_instruction {
    (
        $instruction:ident,
        $execute:expr
    ) => {
        operand_instruction!($instruction, | _: $instruction, computer: &mut Computer | {
            ($execute)(computer)
        },);
    };
}

macro_rules! operand_instruction {
    (
        $instruction:ident,
        $execute:expr,
        $($name:ident),*
    ) => {
        #[derive(Clone, Copy, Default, Debug)]
        pub struct $instruction { 
            $($name: Operand),*
        }
        
        impl $instruction {
            #[allow(unused)]
            pub fn new($($name: Operand),*) -> Self {
                Self {
                    $($name),*
                }
            }
            
            $(
                #[allow(unused)]
                pub fn $name(self) -> Operand {
                    self.$name
                }
            )*
        }
        
        impl Instruction for $instruction {
            fn execute(self, computer: &mut Computer) -> Result<()> {
                ($execute)(self, computer)
            }
        
            fn binary_size(self) -> CpuArchitecture {
                $(self.$name.binary_size() +)* 0
            }
        
            fn to_binary(self, #[allow(unused)] stream: &mut impl IOWrite) -> std::io::Result<CpuArchitecture> {
                #[allow(unused)] let mut total_size = 0;
                $(
                    total_size += self.$name.write_to_stream(stream)?;
                )*
                Ok(total_size)
            }
        
            fn initialize(&mut self, #[allow(unused)] stream: &mut impl IORead) -> std::io::Result<CpuArchitecture>{
                #[allow(unused)] let mut total_size = 0;
                $(
                    let operand = Operand::from_stream(stream)?;
                    self.$name = operand;
                    total_size += operand.binary_size();
                )*
                Ok(total_size)
            }
        }
        
        impl FromStr for $instruction {
            type Err = InstructionError;
        
            fn from_str(str: &str) -> std::result::Result<Self, Self::Err> {
                #[allow(unused)] let mut split = str.trim().split(',');
                #[allow(unused)] let mut index = 0;
                #[allow(unused)] let mut instruction = Self::default();
                $(
                    index += 1;
                    let option = split.next();
                    let str = match option {
                        Some(val) => val,
                        None => return Err(create_invalid_op_count_error(str, index, count_tts!($name) as CpuArchitecture))
                    };
                    let operand = Operand::from_str(str.trim())?;
                    instruction.$name = operand;
                )*
                
                Ok(instruction)
            }
        }
        
        impl Display for $instruction {
            fn fmt(&self, #[allow(unused)] f: &mut Formatter<'_>) -> std::fmt::Result {
                fmt_helper!(self, f, $($name)*);
                Ok(())
            }
        }
        
         impl Is for $instruction {
            type Other = InstructionSet;
            
            fn is(other: &Self::Other) -> Option<Self> {
                match other {
                    InstructionSet::$instruction(val) => Some(*val),
                    _ => None,
                }
            }
        }
    };
}

macro_rules! fmt_helper {
    ($self:expr, $f:expr, ) => { };
    ($self:expr, $f:expr, $last:tt) => {
        $self.$last.fmt($f)?;
    };
    ($self:expr, $f:expr, $head:tt $($val:tt)*) => {
        $self.$head.fmt($f)?;
        $f.write_str(", ")?;
        fmt_helper!($self, $f, $($val)*);
    };
}

create_instructions!(
    Exit => 0,
    Mov => 1,
    Add => 2,
    Sub => 3,
    Mul => 4,
    Div => 5,
    Call => 6,
    Ret => 7,
    Syscall => 8,
    Push => 9,
    Pop => 10,
    Jmp => 11,
    Cmpe => 12,
    Cmpne => 13,
    Cmple => 14,
    Cmpl => 15,
    Cmpge => 16,
    Cmpg => 17,
    Set => 18,
    Break => 19,
    Shl => 20,
    Shr => 21,
    Xor => 22,
    And => 23,
    Or => 24
);

fn get_pointer_to_value(computer: &Computer, index:CpuArchitecture, size: CpuArchitecture) -> Result<CpuArchitecture> {
    let mut buffer = [0u8;size_of::<CpuArchitecture>()];
    let sized_buffer = &mut buffer[..size as usize];

    computer.ram().read_buffer_at_checked(index, sized_buffer)?;
    Ok(CpuArchitecture::from_ne_bytes(buffer))
}

fn set_pointer_to_value(computer: &mut Computer, index:CpuArchitecture, value: CpuArchitecture, size: CpuArchitecture) -> Result<()> {
    let bytes = value.to_ne_bytes();
    let sized_bytes = &bytes[..size as usize];

    computer.ram_mut().write_buffer_at_checked(index, sized_bytes)?;
    Ok(())
}

pub fn read_operand(operand: Operand, computer: &Computer) -> Result<CpuArchitecture> {
    Ok(match operand {
        Operand::Register(register) => computer.cpu().get_register(register)?,
        Operand::RegisterPointer(register_pointer) => {
            let register_value = computer.cpu().get_register(register_pointer.register())?;
            let size = register_pointer.pointed_to_size();
            get_pointer_to_value(computer, register_value, size)?
        },
        Operand::LiteralPointer(literal_pointer) => {
            let size = literal_pointer.pointed_to_size();
            get_pointer_to_value(computer, literal_pointer.address(), size)?
        },
        Operand::Literal(literal) => literal.literal(),
        Operand::Nop => return Err(InstructionError::new(InstructionErrorKind::OperandNop)),
    })
}

pub fn write_operand(operand: Operand, computer: &mut Computer, value: CpuArchitecture) -> Result<()> {
    match operand {
        Operand::Register(register) => computer.cpu_mut().set_register(register, value)?,
        Operand::RegisterPointer(register_pointer) => {
            let register_value = computer.cpu().get_register(register_pointer.register())?;
            let size = register_pointer.pointed_to_size();
            set_pointer_to_value(computer, register_value, value, size)?;
        },
        Operand::LiteralPointer(literal_pointer) => {
            let size = literal_pointer.pointed_to_size();
            set_pointer_to_value(computer, literal_pointer.address(), value, size)?;
        },
        _ => return Err(InstructionError::new(InstructionErrorKind::DestinationInvalid)),
    };
    Ok(())
}

fn create_invalid_op_count_error(str:&str, got:impl Display, expected:CpuArchitecture) -> InstructionError {
    InstructionError::with_message(InstructionErrorKind::InvalidOperandCount, format!("line: {}, got {} operands, expected {}", str, got, expected))
}

empty_instruction!(Exit, | computer: &mut Computer | {
    computer.cpu_mut().exit_program();
    Ok(())
});

operand_instruction!(Mov, | mov: Mov, computer: &mut Computer | {
    let value = read_operand(mov.source, computer)?;
        
    write_operand(mov.destination, computer, value)
}, destination, source);

macro_rules! operation_instruction {
    (
        $operation_name:ident,
        $operation: expr
    ) => {
        operand_instruction!($operation_name, | operation: $operation_name, computer: &mut Computer | {
            let value = read_operand(operation.destination, computer)?;
            let value2 = read_operand(operation.source, computer)?;
    
            let final_value = ($operation)(value, value2);
            
            write_operand(operation.destination, computer, final_value)
        }, destination, source);
    };
}

operation_instruction!(Add, | a:CpuArchitecture, b | { a.wrapping_add(b)});
operation_instruction!(Sub, | a:CpuArchitecture, b | { a.wrapping_sub(b) });
operation_instruction!(Mul, | a:CpuArchitecture, b | { a.wrapping_mul(b) });
operation_instruction!(Div, | a:CpuArchitecture, b | { a / b });
operation_instruction!(Shl, | a:CpuArchitecture, b | { a.wrapping_shl(b as u32) });
operation_instruction!(Shr, | a:CpuArchitecture, b | { a.wrapping_shr(b as u32) });
operation_instruction!(Xor, | a:CpuArchitecture, b | { a ^ b });
operation_instruction!(And, | a:CpuArchitecture, b | { a & b });
operation_instruction!(Or, | a:CpuArchitecture, b | { a | b });

operand_instruction!(Call, | call:Call, computer:&mut Computer | {
    let current_addr = computer.cpu().get_program_counter();
    let address = read_operand(call.address, computer)?;
    computer.cpu_mut().set_program_counter(address);
    computer.cpu_mut().push(&current_addr)?;
    Ok(())
}, address);

impl Call {
    pub fn const_function_binary_size() -> CpuArchitecture {
        Literal::binary_size()
    }
}

impl From<Operand> for Call {
    fn from(value: Operand) -> Self {
        Self { address: value }
    }
}

empty_instruction!(Ret, | computer: &mut Computer | {
    let address = computer.cpu_mut().pop()?;
    computer.cpu_mut().set_program_counter(address);
    Ok(())
});

thread_local! {
    pub static AWAITING_EVENT: Cell<bool> = const { Cell::new(false) };
    pub static REDRAW: Cell<bool> = const { Cell::new(false) };
}

empty_instruction!(Syscall, | computer: &mut Computer | {
    let register = Register::new(0, size_of::<CpuArchitecture>() as u8);
    let function_number = computer.cpu().get_register(register).unwrap(); // cpu is expected to have 4 registers
    let option = FromPrimitive::from_usize(function_number as usize);
    
    match option {
        Some(function) => match function {
            SyscallFunction::Allocate => {
                let alloc_amount_register = Register::new(1, size_of::<CpuArchitecture>() as u8); 
                let alloc_amount = computer.cpu().get_register(alloc_amount_register).unwrap(); // same as above
                
                // SAFETY: no safety :( allocation happens within the emulator by the user
                // or it will be deallocated when the program finishes
                let pointer = unsafe { computer.ram_mut().alloc_unsafe(alloc_amount)? };
                computer.cpu_mut().set_register(alloc_amount_register, pointer).unwrap(); // same as above
                Ok(())
            },
            SyscallFunction::Deallocate => {
                let pointer_register = Register::new(1, size_of::<CpuArchitecture>() as u8); 
                let pointer = computer.cpu().get_register(pointer_register).unwrap(); // same as above
                
                let option = computer.ram_mut().dealloc(pointer);
                computer.cpu_mut().set_register(pointer_register, option.unwrap_or(0)).unwrap(); // same as above
                
                Ok(())
            },
            SyscallFunction::Print => {
                let register = Register::new(1, size_of::<CpuArchitecture>() as u8); // same as above
                let pointer = computer.cpu().get_register(register)?;
                let register =  Register::new(2, size_of::<CpuArchitecture>() as u8);
                let length = computer.cpu().get_register(register)?;
                
                let error = computer.ram().borrow_buffer_checked(pointer, length, Computer::print_bytes)?;
                
                match error {
                    Ok(_) => Ok(()),
                    Err(err) => Err(InstructionError::with_message(InstructionErrorKind::PrintError, err.to_string()))
                }
            },
            SyscallFunction::CreateWindow => {
                let register = Register::new(1, size_of::<CpuArchitecture>() as u8);
                let pointer = computer.cpu().get_register(register).unwrap(); // same as above
                
                let (size, window_name) = if pointer != 0 {
                    let register = Register::new(2, size_of::<CpuArchitecture>() as u8);
                    let length = computer.cpu().get_register(register).unwrap(); // same as above
                    
                    let window_name = computer.ram().borrow_buffer_checked(pointer, length, | buffer | {
                        let mut str = String::with_capacity(buffer.len());
                        for b in buffer {
                            str.push(*b as char);
                        }
                        
                        str
                    })?;
                    
                    let width_register = Register::new(3, size_of::<CpuArchitecture>() as u8);
                    let width = computer.cpu().get_register(width_register).unwrap(); // same as above
                    
                    let height_register = Register::new(4, size_of::<CpuArchitecture>() as u8);
                    let height = computer.cpu().get_register(height_register)?;
                    
                    ((width, height), window_name)
                } else {
                    let width_register = Register::new(2, size_of::<CpuArchitecture>() as u8);
                    let width = computer.cpu().get_register(width_register).unwrap(); // same as above
                    
                    let height_register = Register::new(3, size_of::<CpuArchitecture>() as u8);
                    let height = computer.cpu().get_register(height_register).unwrap(); // same as above
                    
                    ((width, height), String::new())
                };
                let canvas_size = (size.0 as usize, size.1 as usize);
                
                let window_name_option = if window_name.is_empty() {
                    None
                } else {
                    Some(window_name.as_str())
                };
                
                Window::run(canvas_size, window_name_option, computer, register)
            },
            SyscallFunction::GetWindowEvent => {
                AWAITING_EVENT.set(true);
                Ok(())
            },
            SyscallFunction::Redraw => {
                REDRAW.set(true);
                Ok(())
            }
        },
        None => Err(InstructionError::with_message(InstructionErrorKind::SyscallFunctionNotFound, format!("got: {}", function_number)))
    }
});

#[derive(FromPrimitive, ToPrimitive)]
enum SyscallFunction {
    Allocate = 0,
    Deallocate = 1,
    Print = 2,
    CreateWindow = 3,
    GetWindowEvent = 4,
    Redraw = 5,
}

operand_instruction!(Push, | push:Push, computer: &mut Computer | -> Result<()> {
    let value = read_operand(push.source, computer)?;
    
    let buffer = IntoBytes::into(&value);
    computer.cpu_mut().push_buffer(&buffer[..push.source.size() as usize])?;
    
    Ok(())
}, source);

operand_instruction!(Pop, | pop:Pop, computer: &mut Computer | -> Result<()> {
    let mut buffer = [0u8;size_of::<CpuArchitecture>()];
    computer.cpu_mut().pop_buffer(&mut buffer[..pop.destination.size() as usize])?;
    
    let value:CpuArchitecture = FromBytes::from(buffer);
    write_operand(pop.destination, computer, value)?;
    
    Ok(())
}, destination);


operand_instruction!(Jmp, | jmp:Jmp, computer:&mut Computer | -> Result<()> {
    let cmp_flag = computer.cpu_mut().get_cmp_flag();
    if cmp_flag {
        let address = read_operand(jmp.address, computer)?;
        computer.cpu_mut().set_program_counter(address);
    }
    Ok(())
}, address);

impl Jmp {
    pub fn const_function_binary_size() -> CpuArchitecture {
        Literal::binary_size()
    }
}

impl From<Operand> for Jmp {
    fn from(value: Operand) -> Self {
        Self { address: value }
    }
}

macro_rules! cmp_instruction {
    ($name:ident, $comparison:expr) => {
        operand_instruction!($name, | compare: $name, computer: &mut Computer | -> Result<()> {
            let value1 = read_operand(compare.a, computer)?;
            let value2 = read_operand(compare.b, computer)?;
            
            let cmp = ($comparison)(value1, value2);
            computer.cpu_mut().set_cmp_flag(cmp);
            
            Ok(())
        }, a, b);
    };
}

cmp_instruction!(Cmpe, | a, b | { a == b });
cmp_instruction!(Cmpne, | a, b | { a != b });
cmp_instruction!(Cmple, | a, b | { a <= b });
cmp_instruction!(Cmpl, | a, b | { a < b });
cmp_instruction!(Cmpge, | a, b | { a >= b });
cmp_instruction!(Cmpg, | a, b | { a > b });

operand_instruction!(Set, | set:Set, computer: &mut Computer | {
    let flag = computer.cpu_mut().get_cmp_flag();
    write_operand(set.destination, computer, flag as CpuArchitecture)
}, destination);

empty_instruction!(Break, | computer: &mut Computer | -> Result<()> {
    if DEBUG.get() {
        let result = computer.breakpoint();
        if let Err(err) = result {
            Err(InstructionError::with_message(InstructionErrorKind::Other, err.to_string()))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
});