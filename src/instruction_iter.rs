use std::ops::{Deref, DerefMut};
use std::slice::Iter;
use crate::cpu::CpuArchitecture;
use crate::instructions::{InstructionSet, Instruction};
use crate::program::INSTRUCTION_SIZE;

#[derive(Debug)]
pub struct Instructions {
    instructions: Vec<InstructionSet>,
} 

pub struct InstructionsIter<'a> {
    instructions: Iter<'a, InstructionSet>,
    binary_position: CpuArchitecture,
}

impl Instructions {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new()
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            instructions: Vec::with_capacity(capacity)
        }
    }
    
    pub fn push(&mut self, instruction: InstructionSet) {
        self.instructions.push(instruction);
    }
    
    pub fn iter(&self) -> InstructionsIter<'_>  {
        InstructionsIter::new(self.deref().iter())
    }
}

impl Deref for Instructions {
    type Target = [InstructionSet];

    fn deref(&self) -> &Self::Target {
        self.instructions.deref()
    }
}

impl DerefMut for Instructions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.instructions.deref_mut()
    }
}

impl<'a> InstructionsIter<'a> {
    pub fn new(instructions: Iter<'a, InstructionSet>) -> Self {
        Self {
            instructions,
            binary_position: 0,
        }
    }
}

impl<'a> Iterator for InstructionsIter<'a> {
    type Item = (&'a InstructionSet, CpuArchitecture);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.instructions.next();
        match next {
            Some(val) => {
                let current_binary_position = self.binary_position;
                self.binary_position += val.binary_size() + INSTRUCTION_SIZE;
                Some((val, current_binary_position))
            }
            None => None
        }
    }
}