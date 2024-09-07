use core::ops::Range;
use std::cell::RefCell;
use std::io::{Error, ErrorKind, Read, Write};
use std::ops::{Deref, DerefMut};
use crate::array::Array;
use crate::cpu::{CpuArchitecture, FromBytes, IntoBytes};
use crate::{array, error_creator};

type Ranges = Rc<RefCell<Vec<Range<CpuArchitecture>>>>;
pub struct Ram {
    memory: Rc<RefCell<Array<u8>>>,
    allocated_ranges: Ranges
}

error_creator!(
    RamError,
    RamErrorKind,
    IndexOutOfBounds => "The given index is out of bounds for the memory",
    OutOfMemory => "Not enough memory to allocate data",
    SegmentationFault => "Memory was read or written to that isn't allocated"
);

pub struct AllocatedRam {
    buffer: Rc<RefCell<Array<u8>>>,
    start: CpuArchitecture,
    end: CpuArchitecture,
    ranges: Ranges
}

impl Drop for AllocatedRam {
    fn drop(&mut self) {
        let mut borrow = self.ranges.borrow_mut();
        for index in 0..borrow.len() {
            let range = &borrow[index];
            if range == &self.range() {
                borrow.swap_remove(index);
                return;
            }
        }
    }
}

impl Default for AllocatedRam {
    fn default() -> Self {
        Self {
            buffer: Rc::default(),
            start: 0,
            end: 0,
            ranges: Rc::new(RefCell::new(vec![])),
        }
    }
}

fn is_index_out_of_bounds(range:Range<CpuArchitecture>, index:CpuArchitecture, size:usize) -> bool {
    let sub = (range.end - range.start).checked_sub(size as CpuArchitecture);
    match sub {
        Some(val) => (index - range.start) > val as CpuArchitecture,
        None => true,
    }
}

/// reads the generic type T to memory at the **byte** index
fn read_at<T : Sized + FromBytes>(buffer:&[u8], index:CpuArchitecture, range: Range<CpuArchitecture>) -> Result<T>
    where [(); size_of::<T>()]:
{
    let mut temp = [0u8;size_of::<T>()];
    read_buffer_at(buffer, index, &mut temp, range)?;

    Ok(T::from(temp))
}

fn read_buffer_at(buffer:&[u8], index:CpuArchitecture, into_buffer:&mut [u8], range: Range<CpuArchitecture>) -> Result<()> {
    if is_index_out_of_bounds(range, index, into_buffer.len()) {
        Err(RamError::new(RamErrorKind::IndexOutOfBounds))
    } else {
        let range = index as usize..index as usize + into_buffer.len();
        into_buffer.copy_from_slice(&buffer[range]);

        Ok(())
    }
}

/// writes the generic type T to memory at the **byte** index
fn write_at<T : Sized + IntoBytes>(buffer:&mut [u8], index: CpuArchitecture, value:&T, range: Range<CpuArchitecture>) -> Result<()>
    where [(); size_of::<T>()]:
{
    write_buffer_at(buffer, index, &IntoBytes::into(value), range)
}

fn write_buffer_at(buffer:&mut [u8], index: CpuArchitecture, from_buffer:&[u8], range: Range<CpuArchitecture>) -> Result<()> {
    if is_index_out_of_bounds(range, index, from_buffer.len()) {
        Err(RamError::new(RamErrorKind::IndexOutOfBounds))
    } else {
        let range = index as usize..index as usize + from_buffer.len();
        buffer[range].copy_from_slice(from_buffer);

        Ok(())
    }
}

fn create_segment_fault_error(index: CpuArchitecture) -> RamError {
    RamError::with_message(RamErrorKind::SegmentationFault, format!("(0x{:X})", index))
}

impl AllocatedRam {
    pub(crate) fn new(buffer: Rc<RefCell<Array<u8>>>, start: CpuArchitecture, end: CpuArchitecture, ranges: Ranges) -> Self {
        Self {
            buffer,
            start,
            end,
            ranges
        }
    }
    
    pub(crate) fn is_empty(&self) -> bool {
        self.start == 0 // start can never be 0 as that would be the null pointer
    }
    
    fn length(&self) -> CpuArchitecture {
        self.end - self.start
    }
    
    pub fn range(&self) -> Range<CpuArchitecture> {
        self.start..self.end
    }

    /// reads the generic type T to memory at the **byte** index
    pub fn read_at<T : Sized + FromBytes>(&self, index:CpuArchitecture) -> Result<T>
        where [(); size_of::<T>()]:
    {
        read_at(self.buffer.borrow().deref(), index + self.start, self.start..self.end)
    }

    /// writes the generic type T to memory at the **byte** index
    pub fn write_at<T : Sized + IntoBytes>(&mut self, index: CpuArchitecture, value:&T) -> Result<()>
        where [(); size_of::<T>()]:
    {
        write_at(self.buffer.borrow_mut().deref_mut(), index + self.start, value, self.start..self.end)
    }
    
    pub fn read_buffer_at(&self, index:CpuArchitecture, buffer:&mut [u8]) -> Result<()> {
        read_buffer_at(self.buffer.borrow().deref(), index + self.start, buffer, self.start..self.end)
    }

    pub fn write_buffer_at(&self, index:CpuArchitecture, buffer:&[u8]) -> Result<()> {
        write_buffer_at(self.buffer.borrow_mut().deref_mut(), index + self.start, buffer, self.start..self.end)
    }
    
    pub fn borrow_buffer<F, U>(&self, callback: F) -> U 
        where F : FnOnce(&[u8]) -> U    
    {
        let borrow = self.buffer.borrow();
        callback(&borrow[self.start as usize..self.end as usize])
    }

    pub fn borrow_buffer_mut<F, U>(&mut self, callback: F) -> U
        where F : FnOnce(&mut [u8]) -> U
    {
        let mut borrow = self.buffer.borrow_mut();
        callback(&mut borrow[self.start as usize..self.end as usize])
    }
    
    pub fn into_stream(self, stream: &mut impl Write) -> std::io::Result<usize> {
        self.borrow_buffer(| buf | -> std::io::Result<usize> {
            stream.write(buf)
        })
    }
    
    pub fn as_stream(&mut self, position: CpuArchitecture) -> impl Write + Read + '_ {
        MemoryStream::new(self, position)
    }
    
    pub fn fill(&mut self, value: u8) {
        let curr_buf = &mut self.buffer.borrow_mut()[self.start as usize..self.end as usize];
        curr_buf.fill(value);
    }
}

impl Ram {
    pub fn new(amount:CpuArchitecture) -> Self {
        Self {
            memory: Rc::new(RefCell::new(array![0u8;amount as usize])),
            allocated_ranges: Rc::new(RefCell::new(Vec::new())),
        }
    }
    
    pub fn size(&self) -> CpuArchitecture {
        self.memory.borrow().len() as CpuArchitecture
    }
    
    pub fn size_left(&self) -> CpuArchitecture {
        self.memory.borrow().len() as CpuArchitecture - self.allocated_memory()
    }
    
    fn allocated_memory(&self) -> CpuArchitecture {
        let mut total_allocated_length = 0;
        
        for range in self.allocated_ranges.borrow().iter() {
            total_allocated_length += range.end - range.start
        }
        
        total_allocated_length
    }
    
    fn get_free_index(&mut self, length: CpuArchitecture) -> Option<CpuArchitecture> {
        self.allocated_ranges.borrow_mut().sort_by(| a, b | {
            a.start.cmp(&b.start)
        });
        
        let mut index = 1;
        for range in self.allocated_ranges.borrow().iter() {
            if range.start - index >= length {
                return Some(index)
            }
            index = range.end
        }
        
        if self.memory.borrow().len() as CpuArchitecture - index >= length {
            Some(index)
        } else {
            None
        }
    }
    
    fn is_index_allocated(&self, index:CpuArchitecture, length:usize) -> bool {
        for range in self.allocated_ranges.borrow().iter() {
            if index.wrapping_sub(range.start) <= (range.end - range.start).wrapping_sub(length as CpuArchitecture) {
                return true;
            }
        }
        
        false
    }

    /// reads the generic type T to memory at the **byte** index and checks if its allocated
    pub fn read_at_checked<T : Sized + FromBytes>(&self, index:CpuArchitecture) -> Result<T>
        where [(); size_of::<T>()]:
    {
        if !self.is_index_allocated(index, size_of::<T>()) {
            Err(create_segment_fault_error(index))
        } else {
            self.read_at_unchecked(index)
        }
    }
    
    pub fn read_at_unchecked<T : Sized + FromBytes>(&self, index: CpuArchitecture) -> Result<T>
        where [(); size_of::<T>()]:
    {
        let len = self.memory.borrow().len() as CpuArchitecture;
        read_at(self.memory.borrow().deref(), index, 0..len)
    }

    /// writes the generic type T to memory at the **byte** index and checks if its allocated
    pub fn write_at_checked<T : Sized + IntoBytes>(&mut self, index: CpuArchitecture, value:&T) -> Result<()>
        where [(); size_of::<T>()]:
    {
        if !self.is_index_allocated(index, size_of::<T>()) {
            Err(create_segment_fault_error(index))
        } else {
            let len = self.memory.borrow().len() as CpuArchitecture;
            write_at(self.memory.borrow_mut().deref_mut(), index, value, 0..len)
        }
    }

    pub fn read_buffer_at_checked(&self, index:CpuArchitecture, buffer:&mut [u8]) -> Result<()> {
        if !self.is_index_allocated(index, buffer.len()) {
            Err(create_segment_fault_error(index))
        } else {
            self.read_buffer_at_unchecked(index, buffer)
        }
    }

    pub fn read_buffer_at_unchecked(&self, index:CpuArchitecture, buffer:&mut [u8]) -> Result<()> {
        let len = self.memory.borrow().len() as CpuArchitecture;
        read_buffer_at(self.memory.borrow().deref(), index, buffer, 0..len)
    }

    pub fn write_buffer_at_checked(&self, index:CpuArchitecture, buffer:&[u8]) -> Result<()> {
        if !self.is_index_allocated(index, buffer.len()) {
            Err(create_segment_fault_error(index))
        } else {
            let len = self.memory.borrow().len() as CpuArchitecture;
            write_buffer_at(self.memory.borrow_mut().deref_mut(), index, buffer, 0..len)
        }
    }
    
    /// allocates length amount of bytes
    pub fn alloc(&mut self, length: CpuArchitecture) -> Result<AllocatedRam> {
        // SAFETY: deallocates the memory using the AllocatedRam drop method
        let free_index = unsafe { self.alloc_unsafe(length)? };
        
        Ok(AllocatedRam::new(
            self.memory.clone(),
            free_index,
            free_index + length,
            self.allocated_ranges.clone(),
        ))
    }
    
    /// returns the index to allocated ram
    /// SAFETY: needs to be unallocated manually
    pub unsafe fn alloc_unsafe(&mut self, length: CpuArchitecture) -> Result<CpuArchitecture> {
        let option = self.get_free_index(length);

        let free_index = match option {
            Some(val) => val,
            None => return Err(RamError::new(RamErrorKind::OutOfMemory)),
        };

        let allocated_range = free_index..free_index + length;
        self.allocated_ranges.borrow_mut().push(allocated_range);
        
        Ok(free_index)
    }
    
    pub fn dealloc(&mut self, pointer: CpuArchitecture) -> Option<CpuArchitecture> {
        let mut borrow = self.allocated_ranges.borrow_mut();
        for index in 0..borrow.len() {
            let range = borrow[index].clone();
            if range.start == pointer {
                borrow.swap_remove(index);
                return Some(range.end - range.start);
            }
        }
        
        None
    }

    pub fn borrow_buffer_checked<F, U>(&self, index: CpuArchitecture, length: CpuArchitecture, callback: F) -> Result<U>
        where F : FnOnce(&[u8]) -> U
    {
        if !self.is_index_allocated(index, length as usize) {
            Err(create_segment_fault_error(index))
        } else {
            let borrow = self.memory.borrow();
            Ok(callback(&borrow[index as usize..(index + length) as usize]))
        }
    }
    
    pub fn deallocate_all(&mut self) {
        self.allocated_ranges.borrow_mut().clear()
    }
}

struct MemoryStream<'a> {
    memory: &'a mut AllocatedRam,
    position: CpuArchitecture,
}

impl<'a> MemoryStream<'a> {
    pub fn new(ram: &'a mut AllocatedRam, position: CpuArchitecture) -> Self {
        Self {
            memory: ram,
            position,
        }
    }
}

impl<'a> Read for MemoryStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let length = buf.len().min((self.memory.end - self.memory.start) as usize);
        let error = self.memory.read_buffer_at(self.position, &mut buf[..length]);
        match error {
            Ok(_) => {
                self.position += length as CpuArchitecture;
                Ok(length)
            },
            Err(err) => Err(Error::new(ErrorKind::Other, err.to_string()))
        }
    }
}

impl<'a> Write for MemoryStream<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let length = buf.len().min((self.memory.end - self.memory.start) as usize);
        let error = self.memory.write_buffer_at(self.position, &buf[..length]);
        match error {
            Ok(_) => {
                self.position += length as CpuArchitecture;
                Ok(length)
            },
            Err(err) => Err(Error::new(ErrorKind::Other, err.to_string()))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}