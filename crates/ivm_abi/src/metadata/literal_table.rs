//! Borrowed, completely validated literal directories without temporary storage.

use super::{LiteralKindV1, ParsedLiteralSection, decode_literal_descriptor};
use crate::{
    SyscallPolicy, VMError,
    pointer_abi::{PointerType, is_type_allowed_for_policy, validate_tlv_bytes},
};

/// One value borrowed from an immutable, validated indexed literal directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidatedLiteral<'a> {
    /// An exact pointer envelope start and its authenticated nominal payload.
    Pointer {
        /// Address relative to the program's fixed header.
        address: u64,
        /// Validated pointer-ABI nominal type.
        type_id: PointerType,
        /// Original payload bytes, without copying their artifact owner.
        payload: &'a [u8],
    },
    /// Exact two's-complement scalar bits.
    I64(u64),
}

/// A validated directory retaining only borrowed artifact bytes and inline offsets.
///
/// Descriptor order gives every payload byte exactly one interpretation. Construction
/// validates all ranges, scalar widths, pointer hashes and ABI types before exposing
/// any value. Typed canonical payload decoding remains the admission owner's duty.
pub struct LiteralDirectory<'a> {
    program: &'a [u8],
    header_len: usize,
    section: Option<ParsedLiteralSection>,
    pointer_count: usize,
}

impl<'a> LiteralDirectory<'a> {
    /// Validate the complete directory without allocating or copying payload bytes.
    ///
    /// # Errors
    /// Returns invalid metadata for malformed ranges, descriptors or payload framing,
    /// or the existing typed ABI refusal for a forbidden pointer type.
    pub fn validate(
        program: &'a [u8],
        header_len: usize,
        section: Option<ParsedLiteralSection>,
        policy: SyscallPolicy,
    ) -> Result<Self, VMError> {
        let mut directory = Self {
            program,
            header_len,
            section,
            pointer_count: 0,
        };
        let Some(section) = section else {
            return Ok(directory);
        };
        if section.count > usize::from(u16::MAX) + 1 {
            return Err(VMError::InvalidMetadata);
        }
        let mut previous = None;
        for index in 0..section.count {
            let (_, target) = directory.descriptor(index)?;
            if target < section.data_start
                || target >= section.data_end
                || previous.is_some_and(|previous| target <= previous)
            {
                return Err(VMError::InvalidMetadata);
            }
            if index == 0 && target != section.data_start {
                return Err(VMError::InvalidMetadata);
            }
            previous = Some(target);
        }
        if section.count == 0 && section.data_start != section.data_end {
            return Err(VMError::InvalidMetadata);
        }
        for index in 0..section.count {
            let (kind, target, bytes) = directory.raw_entry(index)?;
            match kind {
                LiteralKindV1::PointerTlv => {
                    let tlv = validate_tlv_bytes(bytes).map_err(|_| VMError::InvalidMetadata)?;
                    if !is_type_allowed_for_policy(policy, tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: 1,
                            type_id: tlv.type_id as u16,
                        });
                    }
                    target
                        .checked_sub(header_len)
                        .and_then(|address| u64::try_from(address).ok())
                        .ok_or(VMError::InvalidMetadata)?;
                    directory.pointer_count += 1;
                }
                LiteralKindV1::I64 if bytes.len() != 8 => return Err(VMError::InvalidMetadata),
                LiteralKindV1::I64 => {}
            }
        }
        Ok(directory)
    }

    fn descriptor(&self, index: usize) -> Result<(LiteralKindV1, usize), VMError> {
        let section = self.section.ok_or(VMError::InvalidMetadata)?;
        if index >= section.count {
            return Err(VMError::InvalidMetadata);
        }
        let start = section
            .entries_start
            .checked_add(index.checked_mul(8).ok_or(VMError::InvalidMetadata)?)
            .ok_or(VMError::InvalidMetadata)?;
        let end = start.checked_add(8).ok_or(VMError::InvalidMetadata)?;
        let raw = u64::from_le_bytes(
            self.program
                .get(start..end)
                .ok_or(VMError::InvalidMetadata)?
                .try_into()
                .map_err(|_| VMError::InvalidMetadata)?,
        );
        let (kind, relative) = decode_literal_descriptor(raw)?;
        let target = section
            .start
            .checked_add(usize::try_from(relative).map_err(|_| VMError::InvalidMetadata)?)
            .ok_or(VMError::InvalidMetadata)?;
        Ok((kind, target))
    }

    fn raw_entry(&self, index: usize) -> Result<(LiteralKindV1, usize, &'a [u8]), VMError> {
        let (kind, target) = self.descriptor(index)?;
        let section = self.section.ok_or(VMError::InvalidMetadata)?;
        let end = if index + 1 < section.count {
            self.descriptor(index + 1)?.1
        } else {
            section.data_end
        };
        Ok((
            kind,
            target,
            self.program
                .get(target..end)
                .ok_or(VMError::InvalidMetadata)?,
        ))
    }

    /// Number of literal values in authenticated descriptor order.
    #[must_use]
    pub fn len(&self) -> usize {
        self.section.map_or(0, |section| section.count)
    }

    /// Whether the table contains no values and needs no native index owner.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return an indexed value without repeating hash validation or copying bytes.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ValidatedLiteral<'a>> {
        if index >= self.len() {
            return None;
        }
        let (kind, target, bytes) = self
            .raw_entry(index)
            .expect("validated immutable literal range");
        Some(match kind {
            LiteralKindV1::PointerTlv => ValidatedLiteral::Pointer {
                address: u64::try_from(target - self.header_len)
                    .expect("validated literal address"),
                type_id: PointerType::from_u16(u16::from_be_bytes([bytes[0], bytes[1]]))
                    .expect("validated literal type"),
                payload: &bytes[7..bytes.len() - iroha_crypto::Hash::LENGTH],
            },
            LiteralKindV1::I64 => ValidatedLiteral::I64(u64::from_le_bytes(
                bytes.try_into().expect("validated scalar width"),
            )),
        })
    }

    /// Iterate exact values in descriptor order without a temporary directory.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = ValidatedLiteral<'a>> + '_ {
        (0..self.len()).map(|index| self.get(index).expect("index within validated directory"))
    }

    /// Iterate only exact pointer starts in their already increasing address order.
    pub fn pointer_addresses(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        PointerAddresses {
            values: self.iter(),
            remaining: self.pointer_count,
        }
    }
}

struct PointerAddresses<I> {
    values: I,
    remaining: usize,
}
impl<'a, I: Iterator<Item = ValidatedLiteral<'a>>> Iterator for PointerAddresses<I> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        for value in self.values.by_ref() {
            if let ValidatedLiteral::Pointer { address, .. } = value {
                self.remaining -= 1;
                return Some(address);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<'a, I: Iterator<Item = ValidatedLiteral<'a>>> ExactSizeIterator for PointerAddresses<I> {}

#[cfg(test)]
mod tests;
