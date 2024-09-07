use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::ops::Deref;
use std::rc::Rc;

pub(crate) struct ReadFileHandler {
    files: HashMap<Rc<String>, Rc<RefCell<File>>>
}

impl ReadFileHandler {
    pub(crate) fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            files: HashMap::with_capacity(capacity),
        }
    }
    
    pub(crate) fn open(&mut self, path: impl Into<String>) -> std::io::Result<Rc<RefCell<File>>> {
        let path = Rc::new(path.into());
        
        Ok(match self.files.entry(path.clone()) {
            Entry::Occupied(o) => {
                let file = o.get().clone();
                file.borrow_mut().seek(SeekFrom::Start(0))?;
                file
            },
            Entry::Vacant(v) => {
                let file = OpenOptions::new().read(true).open(path.deref())?;
                v.insert(Rc::new(RefCell::new(file))).clone()
            }
        })
    }
}