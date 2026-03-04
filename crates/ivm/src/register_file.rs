// RegisterFile: 256 64-bit registers with privacy tags.
// Provides checked accessors that return VMError on invalid indices.

use crate::error::VMError;

/// Register file with 256 entries and parallel privacy tags.
#[derive(Clone, Debug)]
pub struct RegisterFile {
    regs: [u64; 256],
    tags: [bool; 256],
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

impl RegisterFile {
    /// Create a new register file with all values and tags cleared.
    pub fn new() -> Self {
        Self {
            regs: [0u64; 256],
            tags: [false; 256],
        }
    }

    /// Read the value of a register.
    pub fn read(&self, index: usize) -> Result<u64, VMError> {
        if index >= 256 {
            return Err(VMError::RegisterOutOfBounds);
        }
        if index == 0 {
            Ok(0)
        } else {
            Ok(self.regs[index])
        }
    }

    /// Write a value to a register. Writes to r0 are ignored.
    pub fn write(&mut self, index: usize, value: u64) -> Result<(), VMError> {
        if index >= 256 {
            return Err(VMError::RegisterOutOfBounds);
        }
        if index != 0 {
            self.regs[index] = value;
        }
        Ok(())
    }

    /// Set the privacy tag of a register. Writes to r0 are ignored.
    pub fn set_tag(&mut self, index: usize, is_private: bool) -> Result<(), VMError> {
        if index >= 256 {
            return Err(VMError::RegisterOutOfBounds);
        }
        if index != 0 {
            self.tags[index] = is_private;
        }
        Ok(())
    }

    /// Get the privacy tag of a register.
    pub fn get_tag(&self, index: usize) -> Result<bool, VMError> {
        if index >= 256 {
            return Err(VMError::RegisterOutOfBounds);
        }
        Ok(self.tags[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r0_is_read_only_zero() {
        let mut rf = RegisterFile::new();
        assert_eq!(rf.read(0).unwrap(), 0);
        rf.write(0, 123).unwrap();
        assert_eq!(rf.read(0).unwrap(), 0);
    }

    #[test]
    fn read_write_non_zero_register() {
        let mut rf = RegisterFile::new();
        rf.write(10, 0xdeadbeef).unwrap();
        assert_eq!(rf.read(10).unwrap(), 0xdeadbeef);
    }

    #[test]
    fn tags_default_and_update() {
        let mut rf = RegisterFile::new();
        assert!(!rf.get_tag(10).unwrap());
        rf.set_tag(10, true).unwrap();
        assert!(rf.get_tag(10).unwrap());
    }

    #[test]
    fn out_of_bounds_errors() {
        let mut rf = RegisterFile::new();
        assert!(rf.read(256).is_err());
        assert!(rf.write(256, 1).is_err());
        assert!(rf.set_tag(256, true).is_err());
        assert!(rf.get_tag(256).is_err());
    }
}
