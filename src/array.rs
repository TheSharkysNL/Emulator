use std::alloc::{Layout, alloc, dealloc};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[macro_export] macro_rules! array {
    ($($val:expr),*) => {{
        Array::from_slice(&[$($val),*])
    }};
    ($fill:expr;$count:expr) => {{
        Array::with_capacity($fill, $count)
    }}
}

pub struct Array<T> {
    pointer: *mut u8,
    length: usize,
    
    phantom_data: PhantomData<T>,
}

impl<T> Array<T> {
    /// unsafe if array is not filled with items because some structs are not allowed to be zeroed out 
    unsafe fn with_capacity_unsafe(capacity:usize) -> Self {
        let option = capacity.checked_mul(size_of::<T>());
        let length = match option {
            Some(val) => val,
            None => panic!("Capacity too large, out of memory!"),
        };

        let layout = Layout::array::<u8>(length).unwrap();
        // SAFETY: pointer will be deallocated using Drop trait
        let pointer = unsafe { alloc(layout) };
        Self {
            pointer,
            length,
            phantom_data: Default::default(),
        }
    }
    
    pub fn from_slice(slice: &[T]) -> Self
        where T : Copy
    {
        // SAFETY: array is filled with data from the slice
        let mut array = unsafe { Self::with_capacity_unsafe(slice.len()) };
        array.copy_from_slice(slice);
        array
    }

    pub fn with_capacity(fill: T, capacity: usize) -> Self 
        where T : Clone
    {
        // SAFETY: array is filled with safe data
        let mut array = unsafe { Self::with_capacity_unsafe(capacity) };
        array.fill(fill);
        array
    }
    
    pub fn len(&self) -> usize {
        self.length / size_of::<T>()
    }
    
    fn index_out_of_bounds(&self, index: usize) -> bool {
        index >= self.len()
    }
    
    fn pointer_at_index(&self, index: usize) -> *mut T {
        assert!(!self.index_out_of_bounds(index));
        let offset = index * size_of::<T>();
        // SAFETY: offset * size_of::<T>() should never overflow because T is 1
        // offset should always be in bounds of the allocated object because of the assertion with index_out_of_bounds
        unsafe { self.pointer.add(offset) as *mut T }
    }
    
    // SAFETY can cause multiple mut references
    unsafe fn slice(&self) -> &mut [T] {
        let length = self.len();

        if length != 0 {
            let pointer = self.pointer_at_index(0);
            // SAFETY: pointer can never be out of bounds because of pointer_at_index
            std::slice::from_raw_parts_mut(pointer, length)
        } else {
            &mut []
        }
    }
}

impl<T> Deref for Array<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        // SAFETY: converts slice to immutable reference 
        unsafe { self.slice() }
    }
}

impl<T> DerefMut for Array<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: rust compiler handles multiple mut references now
        unsafe { self.slice() }
    }
}

impl<T : Copy> From<&[T]> for Array<T> {
    fn from(value: &[T]) -> Self {
        Self::from_slice(value)
    }
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        if self.pointer.is_null() {
            let layout = Layout::array::<u8>(self.length).unwrap();
            // SAFETY: pointer should always point to a allocated piece of memory here
            unsafe { dealloc(self.pointer, layout) }
        }
    }
}

impl<T> Default for Array<T> {
    fn default() -> Self {
        Self {
            pointer: std::ptr::null_mut::<u8>(),
            length: 0,
            phantom_data: Default::default(),
        }
    }
}