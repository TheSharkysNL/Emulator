use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write, UpperHex};
use std::io::{Read, Error, Write as IOWrite, ErrorKind, SeekFrom, Seek};
use std::str::FromStr;
use itertools::Itertools;
use crate::compile::DEBUG;
use crate::cpu::{CpuArchitecture, IntoBytes};
use crate::instructions::{InstructionSet, Instruction, InstructionError, Call, Jmp, Is, Break};
use crate::memory::{AllocatedRam, Ram, RamError};
use crate::error_creator;
use crate::instruction_iter::Instructions;
use crate::operand::{Literal, Operand};
use crate::read_ext::ReadLine;
use crate::write_ext::WriteExt;
use crate::cpu::read_instruction;
use crate::dependency::Dependency;

error_creator!(
    ProgramError,
    ProgramErrorKind,
    ProgramTooLarge => "The program that is currently is too large to be allocated to memory",
    FunctionAlreadyExits => "A function/label with the same name already exists",
    InvalidProgram => "program is invalid due to calls/jmp to functions/labels that don't exist",
    CannotReadDependency => "An error occurred while reading a dependency",
    DependencyFunctionDoesntExist => "A function within a dependency cannot be found",
    DependencyHasInvalidInstruction => "A dependency has a invalid instruction",
    RamError(RamError) => "",
    InstructionError(InstructionError) => ""
);

macro_rules! create_control_flows {
    (
        $instructions:expr,
        $temp_control_flows:expr,
        $control_flows:expr,
        $trimmed_line:expr,
        $control_flow_name:ident,
        $identifier:expr,
        $index:expr
    ) => {
        let name = stringify!($control_flow_name);
        if $trimmed_line.len() >= name.len() && $trimmed_line[..name.len()].eq_ignore_ascii_case(name) {
            let control_flow_name = $trimmed_line[name.len()..].trim();
            if CpuArchitecture::from_str(control_flow_name).is_err() {
                Program::add_temporary_control_flow_instruction::<$control_flow_name>(&mut $instructions, &mut $temp_control_flows, &mut $control_flows, control_flow_name);
                return Ok($index + $control_flow_name::const_function_binary_size() + INSTRUCTION_SIZE);
            }
        }
        
        if let Some(stripped) = $identifier {
            Program::on_control_flow_found::<$control_flow_name>(&mut $instructions, &mut $control_flows, &mut $temp_control_flows, stripped, $index)?;

            return Ok($index);
        }
    };
}

pub const DEPENDENCY_EXTENSION:&str = ".dat";

pub struct Program {
    instructions: Instructions,
    functions: HashMap<String, CpuArchitecture>,
    temporary_call_instructions: HashMap<String, Vec<usize>>,
    labels: HashMap<String, CpuArchitecture>,
    temporary_jmp_instructions: HashMap<String, Vec<usize>>,
}

pub const INSTRUCTION_SIZE: CpuArchitecture = get_instruction_size(InstructionSet::max_instruction_number());

const fn get_instruction_size(max_instruction_number: CpuArchitecture) -> CpuArchitecture {
    let log = max_instruction_number.ilog2();

    (log as CpuArchitecture) / 8 + 1
}

impl Program {
    pub fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            functions: HashMap::with_capacity(4),
            temporary_call_instructions: HashMap::with_capacity(4),
            labels: HashMap::with_capacity(4),
            temporary_jmp_instructions: HashMap::with_capacity(4),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            instructions: Instructions::with_capacity(capacity),
            functions: HashMap::with_capacity(4),
            temporary_call_instructions: HashMap::with_capacity(4),
            labels: HashMap::with_capacity(4),
            temporary_jmp_instructions: HashMap::with_capacity(4),
        }
    }

    pub fn add(&mut self, instruction: InstructionSet) {
        self.instructions.push(instruction);
    }

    fn get_dependencies(temp_call_ins: &HashMap<String, Vec<usize>>) -> Result<Vec<Dependency>> {
        Dependency::get_dependencies(temp_call_ins.iter()
            .map(| (name, _) | { name.as_str() }))
    }

    fn binary_size(&self, dependencies: &[Dependency]) -> Result<(CpuArchitecture, CpuArchitecture)> {
        Self::binary_size_iter(dependencies, self.instructions.iter())
    }

    fn binary_size_iter<'a>(dependencies: &[Dependency], iter: impl Iterator<Item = (&'a InstructionSet, CpuArchitecture)>) -> Result<(CpuArchitecture, CpuArchitecture)> {
        let mut size = 0;

        for (instruction, _) in iter {
            size += instruction.binary_size() as usize + INSTRUCTION_SIZE as usize;
        }

        let instruction_size = size;

        for dependency in dependencies.iter() {
            size += dependency.binary_size() as usize;
        }

        let result = size.try_into();
        match result {
            Ok(val) => Ok((instruction_size as CpuArchitecture, val)),
            Err(_) => Err(ProgramError::new(ProgramErrorKind::ProgramTooLarge)),
        }
    }

    pub fn allocate(mut self, ram: &mut Ram) -> Result<AllocatedRam> {
        let dependencies = Self::get_dependencies(&self.temporary_call_instructions)?;
        if self.temporary_call_instructions.len() != dependencies.len() {
            return Err(ProgramError::new(ProgramErrorKind::InvalidProgram));
        }
        if !self.temporary_jmp_instructions.is_empty() {
            let instructions = self.temporary_jmp_instructions.iter()
                .map(| v | { v.0 } )
                .join(", ");
            return Err(ProgramError::with_message(ProgramErrorKind::InvalidProgram, format!("jmps: [{}]", instructions)));
        }

        let (instruction_size, binary_size) = self.binary_size(&dependencies)?;

        let mut allocated_ram = ram.alloc(binary_size)?;

        Self::allocate_iter(dependencies, &mut self.instructions, &mut allocated_ram, instruction_size, &mut self.temporary_call_instructions)?;

        Ok(allocated_ram)
    }

    fn allocate_iter(
        dependencies: Vec<Dependency>,
        instructions: &mut [InstructionSet],
        allocated_ram: &mut AllocatedRam,
        instruction_size: CpuArchitecture,
        tmp_call_instr: &mut HashMap<String, Vec<usize>>
    ) -> Result<()> {
        let mut dependency_position = instruction_size;
        for dependency in dependencies.iter() {
            let option = Self::try_set_temp_instruction_instruction::<Call>(dependency.function_name().as_str(), dependency_position, tmp_call_instr, instructions);
            if option.is_none() {
                unreachable!("this should not be possible as it shouldn't have been found as a dependency");
            }

            dependency_position += dependency.binary_size();
        }

        let mut index = 0;
        for instruction in instructions.iter() {
            let num = instruction.to_num();
            let bytes = IntoBytes::into(&num);

            allocated_ram.write_buffer_at(index, &bytes[..INSTRUCTION_SIZE as usize]).unwrap(); // should not panic here as the memory should be large enough
            index += instruction.to_binary(&mut allocated_ram.as_stream(index + INSTRUCTION_SIZE)).unwrap() + INSTRUCTION_SIZE; // same here as above
        }

        assert_eq!(index, instruction_size);

        for mut dependency in dependencies {
            allocated_ram.write_buffer_at(index, dependency.instructions(index)?).unwrap(); // should also not panic here
            index += dependency.binary_size();
        }

        Ok(())
    }

    pub fn write_as_library(mut self, stream: &mut impl IOWrite) -> std::io::Result<usize> {
        if self.functions.is_empty() {
            return Ok(0);
        }

        let mut functions:Vec<_> = self.functions.into_iter().collect();
        functions.sort_by(| a, b | {
            a.1.cmp(&b.1)
        });

        let starting_function_position = functions[0].1;
        let mut function_names_size = 0;
        for (function_name, _) in functions.iter() {
            function_names_size += function_name.len();
        }

        let total_identification_size = (function_names_size + (size_of::<CpuArchitecture>() + size_of::<u8>()) * functions.len() + size_of::<u32>()) as u32;
        stream.write_type(&total_identification_size)?;
        let mut bytes_written = size_of_val(&total_identification_size);

        for index in 0..(functions.len() - 1) {
            let (function_name, function_position) = &functions[index];

            let new_function_position = function_position - starting_function_position;

            stream.write_type(&(function_name.len() as u8))?;
            bytes_written += size_of::<u8>();
            bytes_written += stream.write(function_name.as_bytes())?;

            let next_function_position = functions[index + 1].1 - starting_function_position;
            let length = next_function_position - new_function_position;
            stream.write_type(&length)?;
            bytes_written += size_of_val(&length);
        }

        let result = Self::get_dependencies(&self.temporary_call_instructions);
        let dependencies = match result {
            Ok(val) => val,
            Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string())),
        };

        let instructions_iter = self.instructions.iter().skip_while(| (_, position) | {
            *position != starting_function_position
        });
        let result = Self::binary_size_iter(&dependencies, instructions_iter);
        let (instruction_size, binary_size) = match result {
            Ok(val) => val,
            Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string())),
        };

        let (function_name, function_position) = &functions[functions.len() - 1];

        let new_function_position = function_position - starting_function_position;

        stream.write_type(&(function_name.len() as u8))?;
        bytes_written += size_of::<u8>();
        bytes_written += stream.write(function_name.as_bytes())?;

        let length = binary_size - new_function_position;
        stream.write_type(&length)?;
        bytes_written += size_of_val(&length);

        let mut ram = Ram::new(binary_size + 1); // +1 as first byte cannot be allocated
        let mut alloc = ram.alloc(binary_size).unwrap(); // should never give an error here

        let a = self.instructions.iter().take_while(| (_, position) | {
            *position != starting_function_position
        }).count();
        let result = Self::allocate_iter(dependencies, &mut self.instructions[a..], &mut alloc, instruction_size, &mut self.temporary_call_instructions);
        if let Err(err) = result {
            return Err(Error::new(ErrorKind::Other, err.to_string()));
        }

        bytes_written += alloc.into_stream(stream)?;

        Ok(bytes_written)
    }

    fn add_temporary_control_flow_instruction<I : Into<InstructionSet> + From<Operand>>(
        instructions:&mut Instructions,
        temp_instructions: &mut HashMap<String, Vec<usize>>,
        control_flows: &mut HashMap<String, CpuArchitecture>,
        function_name:&str
    ) {
        if let Some(address) = control_flows.get(function_name) {
            instructions.push(I::from(Operand::Literal(Literal::new(*address))).into());
        } else {
            let position = instructions.len();
            instructions.push(I::from(Operand::Literal(Literal::new(0))).into());
            let temp_locations = match temp_instructions.entry(function_name.to_string()) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(Vec::new()),
            };
            temp_locations.push(position); // instruction locations should never change as there is no remove function
        }
    }

    fn try_set_temp_instruction_instruction<I : Is<Other = InstructionSet> + Into<InstructionSet> + From<Operand>>(
        control_flow_name: &str,
        control_flow_index: CpuArchitecture,
        temp_instructions: &mut HashMap<String, Vec<usize>>,
        instructions:&mut [InstructionSet]
    ) -> Option<()> {
        let option = temp_instructions.remove(control_flow_name);
        match option {
            Some(vec) => {
                for position in vec {
                    let instruction = instructions[position];
                    if I::is(&instruction).is_none() {
                        unreachable!("instruction here must be a control flow instruction, got: {}", instruction.as_ref());
                    }

                    instructions[position] = I::from(Operand::Literal(Literal::new(control_flow_index))).into();
                }

                Some(())
            },
            None => None
        }
    }

    fn on_control_flow_found<I : Into<InstructionSet> + From<Operand> + Is<Other = InstructionSet>>(instructions: &mut Instructions, control_flows: &mut HashMap<String, CpuArchitecture>, temp_instructions: &mut HashMap<String, Vec<usize>>, control_flow_name: &str, control_flow_index: CpuArchitecture) -> Result<()> {
        let function_string = control_flow_name.to_string();
        let inserted = control_flows.insert(function_string, control_flow_index);
        if inserted.is_some() {
            return Err(ProgramError::with_message(ProgramErrorKind::FunctionAlreadyExits, format!("function/label name: {}", control_flow_name)));
        }

        Self::try_set_temp_instruction_instruction::<I>(control_flow_name, control_flow_index, temp_instructions, instructions);
        Ok(())
    }

    fn remove_comments(line: &str) -> &str {
        if let Some(index) = line.find(';') {
            &line[..index]
        } else {
            line
        }
    }

    fn parse_line(&mut self, line: &str, index: CpuArchitecture, line_number: u32) -> Result<CpuArchitecture> {
        let trimmed_line = Self::remove_comments(line).trim();
        if trimmed_line.is_empty() {
            return Ok(index);
        }

        create_control_flows!(self.instructions, self.temporary_jmp_instructions, self.labels, trimmed_line, Jmp, trimmed_line.strip_prefix('.'), index);
        create_control_flows!(self.instructions, self.temporary_call_instructions, self.functions, trimmed_line, Call, trimmed_line.strip_suffix(':'), index);

        let result = InstructionSet::from_str(trimmed_line);
        let instruction = match result {
            Ok(val) => val,
            Err(err) => return Err(ProgramError::with_message(ProgramErrorKind::InstructionError(err), format!("line number: {}, line: {}", line_number, line)))
        };

        let binary_size = if !DEBUG.get() &&
            Break::is(&instruction).is_some() {
            0
        } else {
            self.add(instruction);
            instruction.binary_size() + INSTRUCTION_SIZE
        };

        Ok(index + binary_size)
    }

    pub fn from_stream(reader: &mut impl Read) -> std::io::Result<Self> {
        let mut program = Self::new();
        let mut str_buffer = String::with_capacity(128);
        let mut index = 0;
        let mut line_number = 0;

        reader.read_lines(| line | {
            line_number += 1;
            for character in line {
                str_buffer.push(*character as char);
            }

            match program.parse_line(&str_buffer, index, line_number) {
                Err(err) => return Err(err),
                Ok(i) => index = i,
            }
            str_buffer.clear();

            Ok(false)
        })?;

        Ok(program)
    }

    pub fn from_binary(mut reader: &mut (impl Read+Seek)) -> std::io::Result<Self> {
        let instruction_offset = reader.read_type::<u32>()?;

        let mut total_bytes_read = 0;
        let length = reader.seek(SeekFrom::End(0))? - instruction_offset as u64;

        reader.seek(SeekFrom::Start(instruction_offset as u64))?;

        let mut instructions = Instructions::with_capacity((length / 4) as usize);
        while total_bytes_read < length {
            let result = read_instruction(&mut reader);
            let (instruction, bytes_read) = match result {
                Ok(val) => val,
                Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string())),
            };

            total_bytes_read += bytes_read as u64;
            instructions.push(instruction);
        }

        let mut program = Self::new();
        program.instructions = instructions;
        Ok(program)
    }

    pub fn get_line(program_counter:CpuArchitecture, reader: &mut impl Read) -> std::io::Result<(u32, String)> {
        let mut program = Self::new();
        let mut str_buffer = String::with_capacity(128);
        let mut index = 0;
        let mut line_number = 0;

        reader.read_lines(| line | {
            for character in line {
                str_buffer.push(*character as char);
            }

            match program.parse_line(&str_buffer, index, line_number) {
                Err(err) => return Err(err),
                Ok(i) => index = i,
            }

            if index == program_counter {
                Ok(true)
            } else {
                line_number += 1;
                str_buffer.clear();

                Ok(false)
            }
        })?;

        Ok((line_number + 1, str_buffer))
    }
}

impl FromStr for Program {
    type Err = ProgramError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let split = s.trim().split('\n');
        let mut program = Self::new();
        let mut index = 0;

        for (line_number, line) in split.enumerate() {
            index = program.parse_line(line, index, line_number as u32)?;
        }

        Ok(program)
    }
}

fn write_instruction_to_fmt(program: &Program, instruction: &InstructionSet, addr: CpuArchitecture, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str("0x")?;
    UpperHex::fmt(&addr, f)?;
    f.write_str(": ")?;

    match instruction {
        InstructionSet::Call(c) => {
            let addr = match c.address() {
                Operand::Literal(l) => l.literal(),
                _ => CpuArchitecture::MAX
            };
            if addr == 0 &&
                !program.functions.iter().any(| func | {
                    *func.1 == 0
                }) {
                f.write_str( concat!(stringify!(Call), " outer::function"))
            } else {
                instruction.fmt(f)
            }
        },
        _ => instruction.fmt(f)
    }
}

impl Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.instructions.iter();

        let first = iter.next();
        match first {
            Some((instruction, binary_position)) => {
                write_instruction_to_fmt(self, instruction, binary_position, f)?;
            },
            None => return Ok(()),
        }

        for (instruction, binary_position) in iter {
            f.write_char('\n')?;
            write_instruction_to_fmt(self, instruction, binary_position, f)?;
        }

        Ok(())
    }
}