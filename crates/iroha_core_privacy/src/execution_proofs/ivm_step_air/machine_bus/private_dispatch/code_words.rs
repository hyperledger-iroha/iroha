//! Inline bounded public code from the original admitted executable bytes.

use super::MAX_WORDS;

pub(super) struct CodeWords {
    values: [u32; MAX_WORDS],
    len: usize,
}
impl CodeWords {
    pub(super) fn new(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(4) || bytes.len() / 4 > MAX_WORDS {
            return None;
        }
        let mut words = Self {
            values: [0; MAX_WORDS],
            len: bytes.len() / 4,
        };
        for (target, bytes) in words.values.iter_mut().zip(bytes.chunks_exact(4)) {
            *target = u32::from_le_bytes(bytes.try_into().expect("exact instruction word"));
        }
        Some(words)
    }
}
impl core::ops::Deref for CodeWords {
    type Target = [u32];
    fn deref(&self) -> &Self::Target {
        &self.values[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_inline_code_owner_bounds_and_decodes_every_admitted_word() {
        assert!(CodeWords::new(&[]).is_none());
        assert!(CodeWords::new(&[0; 3]).is_none());
        assert!(CodeWords::new(&[0; (MAX_WORDS + 1) * 4]).is_none());
        let bytes = (0..MAX_WORDS)
            .flat_map(|index| (index as u32 | 0xa000_0000).to_le_bytes())
            .collect::<Vec<_>>();
        let words = CodeWords::new(&bytes).unwrap();
        assert_eq!(words.len(), MAX_WORDS);
        for (index, word) in words.iter().enumerate() {
            assert_eq!(*word, index as u32 | 0xa000_0000);
        }
        let one = CodeWords::new(&[4, 3, 2, 1]).unwrap();
        assert_eq!(&*one, &[0x0102_0304]);
        assert!(one.values[1..].iter().all(|word| *word == 0));
    }
}
