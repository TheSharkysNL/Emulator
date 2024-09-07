use std::io::{Read, Result, Error, ErrorKind};
use crate::cpu::FromBytes;

pub trait ReadLine : Read {
    fn read_lines<F, E : std::error::Error>(&mut self, mut callback:F) -> Result<usize>
        where F : FnMut(&[u8]) -> std::result::Result<bool, E>
    {
        let mut vec = Vec::new();
        let mut buffer = [0u8;1024];

        let mut total_read_bytes = 0;

        loop {
            let read_bytes = self.read(&mut buffer)?;
            if read_bytes == 0 {
                return Ok(total_read_bytes);
            }

            let current_buffer = &buffer[..read_bytes];

            let option = current_buffer.iter().position(| b | { *b == b'\n' });
            let mut position = match option {
                Some(val) => val,
                None => { vec.extend_from_slice(current_buffer); continue; }
            };

            if !vec.is_empty() {
                vec.extend_from_slice(&current_buffer[..position]);
                let result = callback(vec.as_slice());
                match result {
                    Ok(stop) => if stop { return Ok(read_bytes); }
                    Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string()))
                }
                vec.clear();
            } else {
                let result = callback(&current_buffer[..position]);
                match result {
                    Ok(stop) => if stop { return Ok(read_bytes); }
                    Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string()))
                }
            }

            position += 1;
            let mut previous_position = position;
            while {
                let option = if position < current_buffer.len() {
                    current_buffer[position..].iter().position(| b | { *b == b'\n' })
                } else {
                    None
                };
                position = match option {
                    Some(val) => val,
                    None => { if read_bytes == buffer.len() { vec.extend_from_slice(&current_buffer[position..]); } 0 }
                } + position;
                option.is_some()
            } {
                let result = callback(&current_buffer[previous_position..position]);
                match result {
                    Ok(stop) => if stop { return Ok(read_bytes); }
                    Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string()))
                }

                position += 1;
                previous_position = position;
            }

            total_read_bytes += read_bytes;
            if read_bytes != buffer.len() {
                if !vec.is_empty() {
                    let result = callback(vec.as_slice());
                    match result {
                        Ok(stop) => if stop { return Ok(read_bytes); }
                        Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string()))
                    }
                } else {
                    let result = callback(&current_buffer[position..]);
                    match result {
                        Ok(stop) => if stop { return Ok(read_bytes); }
                        Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string()))
                    }
                }
                return Ok(total_read_bytes);
            }
        }
    }

    fn read_type<T : FromBytes>(&mut self) -> Result<T>
        where [(); size_of::<T>()]:
    {
        let mut temp = [0u8;size_of::<T>()];
        self.read_exact(&mut temp)?;
        let t = T::from(temp);
        Ok(t)
    }
}

impl<R : Read> ReadLine for R {
    
}