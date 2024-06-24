use std::io::{self, ErrorKind};

const SLOT_SIZE: usize = 157;

#[derive(Clone, Copy)]
pub(super) struct Slot {
    data: [u8; SLOT_SIZE],
    current_len: u16,
    total_len: u16,
    is_valid: bool,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            data: [0u8; SLOT_SIZE],
            current_len: 0,
            total_len: 0,
            is_valid: false,
        }
    }
}
impl Slot {
    pub(crate) fn reset(&mut self) {
        self.current_len = 0;
        self.total_len = 0;
        self.is_valid = false;
    }
    pub(super) fn total_len(&self) -> u16 {
        self.total_len
    }

    pub(super) fn data(&self) -> &[u8] {
        &self.data[..self.total_len as usize]
    }

    pub(super) fn set_total_len(&mut self, len: u16) -> io::Result<()> {
        if len > SLOT_SIZE as u16 {
            return Err(io_invalid_input!(ErrorKind::InvalidInput, "invalid len"));
        }
        self.total_len = len;
        self.is_valid = true;
        Ok(())
    }

    pub(super) fn copy_from_slice(&mut self, src: &[u8]) -> io::Result<()> {
        let current_len = self.current_len as usize;
        if current_len + src.len() > self.total_len as usize {
            return Err(io_invalid_input!(ErrorKind::InvalidInput, "overflow"));
        }
        self.data[current_len..current_len + src.len()].copy_from_slice(src);
        self.current_len += src.len() as u16;
        Ok(())
    }

    pub(super) fn is_complete(&self) -> bool {
        self.is_valid && self.current_len == self.total_len
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_slot() {
        let mut slot = super::Slot::default();
        let data = [0u8; 158];
        assert!(slot.set_total_len(159).is_err());
        assert!(slot.set_total_len(30).is_ok());
        assert!(slot.copy_from_slice(&data[..50]).is_err());
        assert!(slot.copy_from_slice(&data[..29]).is_ok());
        assert!(!slot.is_complete());
        assert!(slot.copy_from_slice(&data[29..30]).is_ok());
        assert!(slot.is_complete());
        slot.reset();
        assert!(!slot.is_complete());
    }
}
