// const BREAKPOINT_MESSAGE: &str =
//     "breakpoint reached, please type \"continue\" to continue.
// commands:
//     register {register}
//     memory {address}, {size}";

use crate::computer::{Computer, ComputerError, ComputerErrorKind};
use crate::pattern_ignore_case::IgnoreCase;
use std::io::stdin;
use std::ops::Deref;
use std::str::FromStr;
use crate::cpu::CpuArchitecture;
use crate::instructions::read_operand;
use crate::operand::Operand;

enum StaticString {
    Static(&'static str),
    String(String),
}

impl From<&'static str> for StaticString {
    fn from(value: &'static str) -> Self {
        StaticString::Static(value)
    }
}

impl From<String> for StaticString {
    fn from(value: String) -> Self {
        StaticString::String(value)
    }
}

impl Deref for StaticString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self { 
            StaticString::Static(s) => s,
            StaticString::String(s) => s.as_str(),
        }
    }
}

macro_rules! join {
    ($separator: literal, $value: tt) => {
        concat!("{", stringify!($value), "}")
    };
    ($separator: literal, $value: tt $( $values: tt )*) => {
        concat!("{", stringify!($value), "}", $separator, join!($separator, $($values)*))
    };
}

macro_rules! break_commands {
    (
        $( $name:ident => | $computer:ident, $( $values:ident ),*  | $expr: expr ),*
    ) => {
        const BREAKPOINT_MESSAGE: &str = 
            concat!("breakpoint reached, please type \"continue\" to continue.
commands:\n", $( "    ", stringify!($name), " ", join!(", ", $($values)*), "\n" ),*);
        
        pub struct BreakPoint {}
        
        impl BreakPoint {
            pub fn create_breakpoint(computer: &mut Computer) -> Result<(), ComputerError> {
                let stdin = stdin();
        
                println!("{}", BREAKPOINT_MESSAGE);
        
                let mut str_buffer = String::with_capacity(64);
                loop {
                    str_buffer.clear();
                    let result = stdin.read_line(&mut str_buffer);
                    if let Err(err) = result {
                        return Err(ComputerError::with_message(ComputerErrorKind::Other, err.to_string()));
                    }
                    
                    let trimmed_str = str_buffer.trim();
                    $(
                        if let Some(stripped) = trimmed_str.strip_prefix(IgnoreCase::new(stringify!($name))) {
                            let mut split = stripped.split(',')
                            .map(| val | { val.trim() });
                            
                            let mut count = 0;
                            $(
                                count += 1;
                                let option = split.next();
                                let $values = match option {
                                    Some(val) => val,
                                    None => { println!("couldn't find argument {}", count); continue; }
                                };
                            )*
                            
                            let option: Option<StaticString> = (| $computer: &mut Computer, $($values),* | {
                                $expr
                            })(computer, $($values),*);
                            
                            if let Some(val) = option {
                                println!("{}", val.deref());
                            }
                        }
                    
                    )*
                    
                    if trimmed_str.eq_ignore_ascii_case("c") || trimmed_str.eq_ignore_ascii_case("continue") {
                        break;
                    }
                }
                
                Ok(())
            }
        }
    };
}

break_commands!(register => | computer, register | {
    let result = Operand::from_str(register);
    let operand = match result {
        Ok(op) => op,
        Err(err) => return Some(err.to_string().into()),
    };
    match operand {
        Operand::Register(register) => {
            let result = computer.cpu().get_register(register);
            let value = match result {
                Ok(val) => val,
                Err(err) => return Some(err.to_string().into()),
            };
            println!("{}", value);
        },
        _ => return Some("the value given is not a valid register".into()),
    };
    
    None
}, memory => | computer, address, size | {
    let result = CpuArchitecture::from_str(size);
    let size = match result {
        Ok(val) => val,
        Err(err) => return Some(err.to_string().into()),
    };
    
    let result = Operand::from_str(address);
    let address_operand = match result {
        Ok(op) => op,
        Err(err) => return Some(err.to_string().into()),
    };
    
    let result = read_operand(address_operand, computer);
    let address = match result {
        Ok(address) => address,
        Err(err) => return Some(err.to_string().into()),
    };
    
    if size > 1024 {
        return Some("a size greater than 1024 cannot be printed".into());
    }
    
    let mut buffer = [0u8;1024];
    let result = computer.ram().read_buffer_at_unchecked(address, &mut buffer[..size as usize]);
    
    if let Err(err) = result {
        return Some(err.to_string().into());
    }
    
    let result = Computer::print_bytes(&buffer[..size as usize]);
    if let Err(err) = result {
        Some(format!("unable to print out memory, error: {}", err).into())
    } else {
        None
    }
});

