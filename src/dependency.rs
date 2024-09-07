use std::io::{Read, Seek, SeekFrom, Write};
use crate::cpu::{read_instruction, CpuArchitecture, FromBytes};
use crate::file_handler::ReadFileHandler;
use crate::instructions::InstructionSet;
use crate::operand::Operand;
use crate::program::{ProgramError, ProgramErrorKind, DEPENDENCY_EXTENSION};
use crate::read_ext::ReadLine;
use crate::write_ext::WriteExt;

pub struct Dependency {
    function_name: String,
    instructions: Vec<u8>,
}

macro_rules! conv_io_error {
    ($err:expr, $dependency_name:expr) => {{
        match $err {
            Ok(val) => val,
            Err(_) => return Err(ProgramError::with_message(ProgramErrorKind::CannotReadDependency, format!("filename: {}{}", $dependency_name, DEPENDENCY_EXTENSION))),
        }
    }};
}

impl Dependency {
    pub fn new(dependency_function: &str, file_handler: &mut ReadFileHandler) -> Result<Self, ProgramError> {
        let (dependency_name, function_name) = Self::split_dependency_function(dependency_function)?;

        let mut file_name = String::with_capacity(dependency_name.len() + DEPENDENCY_EXTENSION.len());
        file_name.push_str(dependency_name);
        file_name.push_str(DEPENDENCY_EXTENSION);

        let file_ref = conv_io_error!(file_handler.open(file_name), dependency_name);
        let mut file = file_ref.borrow_mut();

        let instruction_offset = conv_io_error!(file.read_type::<u32>(), dependency_name);
        let mut index = size_of_val(&instruction_offset);
        let mut name_buffer = [0u8;u8::MAX as usize + size_of::<CpuArchitecture>()];
        let mut current_instruction_offset = instruction_offset;

        while index < instruction_offset as usize {
            let name_length = conv_io_error!(file.read_type::<u8>(), dependency_name);
            let bytes_read = size_of_val(&name_length);
            if bytes_read == 0 {
                return Err(Self::create_function_not_found_error(dependency_function));
            }
            index += bytes_read;

            let read_length = (name_length as usize) + size_of::<CpuArchitecture>();
            let bytes_read = conv_io_error!(file.read(&mut name_buffer[..read_length]), dependency_name);
            if bytes_read != read_length {
                return Err(Self::create_function_not_found_error(dependency_function));
            }
            index += bytes_read;

            let (current_name, instruction_length_bytes) = name_buffer.split_at(name_length as usize);
            let instruction_length:CpuArchitecture = FromBytes::from(instruction_length_bytes[..size_of::<CpuArchitecture>()].try_into().unwrap());

            if current_name.eq(function_name.as_bytes()) {
                let mut vec = vec![0u8;instruction_length as usize];
                conv_io_error!(file.seek(SeekFrom::Start(current_instruction_offset as u64)), dependency_name);
                let bytes_read = conv_io_error!(file.read(vec.as_mut_slice()), dependency_name);
                if bytes_read != instruction_length as usize {
                    return Err(Self::create_function_not_found_error(dependency_function));
                }
                return Ok(
                    Self{
                        function_name: dependency_function.to_string(),
                        instructions: vec,
                    }
                )
            }
            current_instruction_offset += instruction_length as u32;
        }

        Err(Self::create_function_not_found_error(dependency_function))
    }
    
    pub fn get_dependencies<'a>(dependency_functions: impl Iterator<Item = &'a str>) -> Result<Vec<Self>, ProgramError> {
        let mut dependencies = Vec::with_capacity(4);
        let mut file_handler = ReadFileHandler::new();

        for function in dependency_functions {
            let dependency = Dependency::new(function, &mut file_handler)?;
            dependencies.push(dependency);
        }

        Ok(dependencies)
    }

    fn split_dependency_function(dependency_function: &str) -> Result<(&str, &str), ProgramError> {
        let mut split = dependency_function.split("::");

        let dependency_name = split.next().expect("panic function without a name should never have been stored?");
        let option = split.next();
        let function_name = match option {
            Some(val) => val,
            None => return Err(ProgramError::new(ProgramErrorKind::InvalidProgram)),
        };

        if split.next().is_some() {
            Err(ProgramError::new(ProgramErrorKind::InvalidProgram))
        } else {
            Ok((dependency_name, function_name))
        }
    }

    fn create_function_not_found_error(dependency_function: &str) -> ProgramError {
        ProgramError::with_message(ProgramErrorKind::DependencyFunctionDoesntExist, format!("function name: {}", dependency_function))
    }

    pub fn binary_size(&self) -> CpuArchitecture {
        self.instructions.len() as CpuArchitecture
    }

    pub fn instructions(&mut self, index: CpuArchitecture) -> Result<&[u8], ProgramError> {
        let mut stream = BufferStream::new(self.instructions.as_mut_slice());

        fn set_new_control_flow_position(stream: &mut BufferStream, index: CpuArchitecture, address: Operand) {
            if let Operand::Literal(lit) = address {
                stream.set_position(stream.position() - size_of::<CpuArchitecture>() as CpuArchitecture);
                stream.write_type(&(lit.literal() + index)).unwrap(); // should never panic
            }
        }

        // moves all call/jmp instruction to the new position where these functions/labels are
        while stream.length_left() > 0 {
            let result = read_instruction(&mut stream);
            let (instruction, _) = match result {
                Ok(val) => val,
                Err(err) => return Err(ProgramError::with_message(ProgramErrorKind::DependencyHasInvalidInstruction,
                                                                  format!("error: {}, function: {}", err, self.function_name()))),
            };

            match instruction {
                InstructionSet::Call(call) => set_new_control_flow_position(&mut stream, index, call.address()),
                InstructionSet::Jmp(jmp) => set_new_control_flow_position(&mut stream, index, jmp.address()),
                _ => {}
            }
        }

        Ok(self.instructions.as_slice())
    }

    pub fn function_name(&self) -> &String {
        &self.function_name
    }
}

struct BufferStream<'a> {
    memory: &'a mut [u8],
    position: CpuArchitecture
}

impl<'a> BufferStream<'a> {
    pub fn new(memory: &'a mut [u8]) -> Self {
        if memory.len() > CpuArchitecture::MAX as usize {
            panic!("memory length too large");
        }
        Self {
            memory,
            position: 0
        }
    }

    pub fn length_left(&self) -> CpuArchitecture {
        self.memory.len() as CpuArchitecture - self.position
    }

    pub fn position(&self) -> CpuArchitecture {
        self.position
    }

    pub fn set_position(&mut self, position: CpuArchitecture) {
        if position as usize >= self.memory.len() {
            panic!("position out of bounds of the buffer");
        }
        self.position = position;
    }
}

impl<'a> Read for BufferStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let length = buf.len().min(self.length_left() as usize);

        let range = self.position as usize..self.position as usize + length;
        buf[..length].copy_from_slice(&self.memory[range]);
        self.position += length as CpuArchitecture;

        Ok(length)
    }
}

impl<'a> Write for BufferStream<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let length = buf.len().min(self.length_left() as usize);

        let range = self.position as usize..self.position as usize + length;
        self.memory[range].copy_from_slice(&buf[..length]);
        self.position += length as CpuArchitecture;

        Ok(length)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}