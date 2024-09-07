use std::io::{Write, Result};
use crate::cpu::IntoBytes;

pub trait WriteExt : Write {
    fn write_type<T : IntoBytes>(&mut self, value: &T) -> Result<()> 
        where [(); size_of::<T>()]:
    {
        self.write_all(&IntoBytes::into(value))
    }
}

impl<T : Write> WriteExt for T {
    
}