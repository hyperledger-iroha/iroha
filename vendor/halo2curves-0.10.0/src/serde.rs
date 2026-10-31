#[cfg(feature = "std")]
use std::io::{self, Read, Write};

use core::fmt::Debug;

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Repr<const T: usize>(
    #[cfg_attr(feature = "derive_serde", serde(with = "serde_arrays"))] [u8; T],
);

impl<const T: usize> Repr<T> {
    pub fn inner(&self) -> &[u8; T] {
        &self.0
    }
}

impl<const T: usize> From<[u8; T]> for Repr<T> {
    fn from(bytes: [u8; T]) -> Self {
        Self(bytes)
    }
}

impl<'a, const T: usize> From<&'a [u8]> for Repr<T> {
    fn from(bytes: &[u8]) -> Self {
        Self(bytes.try_into().unwrap())
    }
}

impl<const T: usize> From<Repr<T>> for [u8; T] {
    fn from(repr: Repr<T>) -> Self {
        repr.0
    }
}

impl<const T: usize> Default for Repr<T> {
    fn default() -> Self {
        Self([0u8; T])
    }
}

impl<const T: usize> AsMut<[u8]> for Repr<T> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl<const T: usize> AsRef<[u8]> for Repr<T> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const T: usize> core::ops::Index<core::ops::Range<usize>> for Repr<T> {
    type Output = [u8];

    fn index(&self, range: core::ops::Range<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl<const T: usize> core::ops::Index<usize> for Repr<T> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<const T: usize> core::ops::Index<core::ops::RangeTo<usize>> for Repr<T> {
    type Output = [u8];

    fn index(&self, range: core::ops::RangeTo<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl<const T: usize> core::ops::Index<core::ops::RangeFrom<usize>> for Repr<T> {
    type Output = [u8];

    fn index(&self, range: core::ops::RangeFrom<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl<const T: usize> core::ops::IndexMut<core::ops::Range<usize>> for Repr<T> {
    fn index_mut(&mut self, range: core::ops::Range<usize>) -> &mut Self::Output {
        &mut self.0[range]
    }
}

/// Trait for converting raw bytes to/from the internal representation of a
/// type. For example, field elements are represented in Montgomery form and
/// serialized/deserialized without Montgomery reduction.
pub trait SerdeObject: Sized {
    /// The purpose of unchecked functions is to read the internal memory
    /// representation of a type from bytes as quickly as possible. No
    /// sanitization checks are performed to ensure the bytes represent a
    /// valid object. As such this function should only be used internally
    /// as an extension of machine memory. It should not be used to deserialize
    /// externally provided data.
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self;
    fn from_raw_bytes(bytes: &[u8]) -> Option<Self>;

    #[cfg(feature = "std")]
    fn to_raw_bytes(&self) -> Vec<u8>;

    /// The purpose of unchecked functions is to read the internal memory
    /// representation of a type from disk as quickly as possible. No
    /// sanitization checks are performed to ensure the bytes represent a
    /// valid object. This function should only be used internally when some
    /// machine state cannot be kept in memory (e.g., between runs)
    /// and needs to be reloaded as quickly as possible.
    #[cfg(feature = "std")]
    fn read_raw_unchecked<R: Read>(reader: &mut R) -> Self;
    #[cfg(feature = "std")]
    fn read_raw<R: Read>(reader: &mut R) -> io::Result<Self>;

    #[cfg(feature = "std")]
    fn write_raw<W: Write>(&self, writer: &mut W) -> io::Result<()>;
}

pub mod endian {
    extern crate alloc;
    use alloc::vec::Vec;

    pub trait EndianRepr: Sized {
        const ENDIAN: Endian;

        fn to_bytes(&self) -> Vec<u8>;

        fn from_bytes(x: &[u8]) -> subtle::CtOption<Self>;
    }

    pub enum Endian {
        LE,
        BE,
    }

    impl Endian {
        pub fn to_bytes(&self, res: &mut [u8], el: &[u64]) {
            match self {
                Endian::LE => {
                    el.iter().enumerate().for_each(|(i, limb)| {
                        let off = i * 8;
                        res[off..off + 8].copy_from_slice(&limb.to_le_bytes());
                    });
                }
                Endian::BE => {
                    el.iter().rev().enumerate().for_each(|(i, limb)| {
                        let off = i * 8;
                        res[off..off + 8].copy_from_slice(&limb.to_be_bytes());
                    });
                }
            }
        }

        pub fn from_bytes(&self, res: &[u8], el: &mut [u64]) {
            match self {
                Endian::LE => {
                    el.iter_mut().enumerate().for_each(|(i, limb)| {
                        let off = i * 8;
                        *limb = u64::from_le_bytes(res[off..off + 8].try_into().unwrap());
                    });
                }
                Endian::BE => {
                    el.iter_mut().rev().enumerate().for_each(|(i, limb)| {
                        let off = i * 8;
                        *limb = u64::from_be_bytes(res[off..off + 8].try_into().unwrap());
                    });
                }
            }
        }
    }
}

pub(crate) enum CompressedFlagConfig {
    // NOTE: if needed we can add fields for bit positions

    // Secp256k1, Secp256r1 curves should be encoded with
    Extra, // sign: 0 identity: 1

    // Pasta curves should be encoded with
    SingleSpare, // sign: 0

    // BN254 curve should be encoded with
    TwoSpare, // sign: 0, identity: 1

    // BLS12-{381, 377} curves should be encoded with
    ThreeSpare, // is_compressed: 0, sign: 1, identity: 2
}

impl CompressedFlagConfig {
    pub(crate) const fn has_extra_byte(&self) -> bool {
        matches!(self, CompressedFlagConfig::Extra)
    }
}

pub(crate) struct Flag {}

impl Flag {
    fn flag(pos: u8) -> u8 {
        1 << 7u8.checked_sub(pos).unwrap()
    }

    fn set(pos: u8, value: bool, flag_byte: &mut u8) {
        value.then(|| *flag_byte |= Self::flag(pos));
    }

    fn get(pos: u8, flag_byte: &mut u8) -> subtle::Choice {
        let flag = Self::flag(pos);
        let value = (*flag_byte & flag) != 0;
        *flag_byte &= !flag; // clear flag
        subtle::Choice::from(value as u8)
    }
}

pub(crate) trait Compressed<C: crate::CurveAffine>:
    Debug + Copy + Default + AsRef<[u8]> + AsMut<[u8]> + Send + Sync + 'static
where
    C::Base: crate::serde::endian::EndianRepr,
{
    const CONFIG: CompressedFlagConfig;

    fn flag_byte(&mut self) -> &mut u8 {
        use crate::serde::endian::EndianRepr;
        match Self::CONFIG {
            // Most sig byte is always the flag byte when extra byte flag is used
            CompressedFlagConfig::Extra => self.as_mut().first_mut().unwrap(),
            _ => match C::Base::ENDIAN {
                // Least sig byte is the flag byte
                crate::serde::endian::Endian::LE => self.as_mut().last_mut().unwrap(),
                // Most sig byte is the flag byte
                crate::serde::endian::Endian::BE => self.as_mut().first_mut().unwrap(),
            },
        }
    }

    fn sign(y: &C) -> subtle::Choice;

    fn resolve(x: C::Base, sign: subtle::Choice) -> subtle::CtOption<C>;

    fn pos_sign() -> u8 {
        match Self::CONFIG {
            CompressedFlagConfig::Extra => 0,
            CompressedFlagConfig::SingleSpare => 0,
            CompressedFlagConfig::TwoSpare => 0,
            CompressedFlagConfig::ThreeSpare => 2,
        }
    }

    fn pos_compressed() -> Option<u8> {
        match Self::CONFIG {
            CompressedFlagConfig::ThreeSpare => Some(0),
            _ => None,
        }
    }

    fn pos_idetity() -> Option<u8> {
        match Self::CONFIG {
            CompressedFlagConfig::Extra => Some(1),
            CompressedFlagConfig::SingleSpare => None,
            CompressedFlagConfig::TwoSpare => Some(1),
            CompressedFlagConfig::ThreeSpare => Some(1),
        }
    }

    fn set_sign(&mut self, c: &C) {
        let sign = bool::from(Self::sign(c));
        let pos = Self::pos_sign();
        Flag::set(pos, sign, self.flag_byte());
    }

    fn set_compressed(&mut self) {
        if let Some(pos) = Self::pos_compressed() {
            Flag::set(pos, true, self.flag_byte())
        }
    }

    fn set_identity(&mut self, c: &C) {
        if let Some(pos) = Self::pos_idetity() {
            Flag::set(pos, bool::from(c.is_identity()), self.flag_byte());
        };
    }

    fn get_sign(&mut self) -> subtle::Choice {
        Flag::get(Self::pos_sign(), self.flag_byte())
    }

    fn get_is_compressed(&mut self) -> Option<subtle::Choice> {
        Self::pos_compressed().map(|pos| Flag::get(pos, self.flag_byte()))
    }

    fn get_is_identity(&mut self) -> Option<subtle::Choice> {
        Self::pos_idetity().map(|pos| Flag::get(pos, self.flag_byte()))
    }

    fn set_flags(&mut self, c: &C) {
        self.set_identity(c);
        self.set_sign(c);
        self.set_compressed();
    }

    fn encode(c: &C) -> Self {
        use crate::serde::endian::EndianRepr;
        let mut this = Self::default();
        let coordinates = c.coordinates().unwrap();
        let x = coordinates.x();
        let x_bytes = x.to_bytes();
        match Self::CONFIG {
            CompressedFlagConfig::Extra => {
                // Most sig byte is always the flag byte when extra byte flag is used
                this.as_mut()[1..1 + x_bytes.len()].copy_from_slice(&x_bytes)
            }
            _ => this.as_mut()[..x_bytes.len()].copy_from_slice(&x_bytes),
        };
        this.set_identity(c);
        this.set_sign(c);
        this.set_compressed();
        this
    }

    fn decode(mut self) -> subtle::CtOption<C> {
        let is_compressed = self.get_is_compressed();
        // if is compressed set then it should be set one
        let is_valid_0: subtle::Choice = is_compressed.unwrap_or(subtle::Choice::from(1u8));

        let is_identity = self.get_is_identity();

        let sign = self.get_sign();

        // with extra byte config expect it goes to zero after it is read
        // otherwise `from_byte` checks if flag or rest unused bytes are zero
        let is_valid_1 = match Self::CONFIG {
            CompressedFlagConfig::Extra => *self.flag_byte() == 0,
            _ => true,
        };
        let is_valid_1: subtle::Choice = (is_valid_1 as u8).into();

        let x = match Self::CONFIG {
            CompressedFlagConfig::Extra => {
                // Most sig byte is always the flag byte when extra byte flag is used
                <C::Base as crate::serde::endian::EndianRepr>::from_bytes(&self.as_ref()[1..])
            }
            _ => <C::Base as crate::serde::endian::EndianRepr>::from_bytes(self.as_ref()),
        };

        x.and_then(|x| -> subtle::CtOption<C> {
            use ff::Field;
            let is_zero = x.is_zero();

            let (is_valid_2, is_identity) = match is_identity {
                // identity flag active
                Some(is_identity) => {
                    // identity flag set:
                    // * x must be zero
                    // * sign must not be set

                    // identity flag not set:
                    // * x must not be zero

                    let is_valid = (is_identity & is_zero & !sign) ^ (!is_identity & !is_zero);

                    (is_valid, is_identity)
                }

                // identitity flag inactive
                None => (subtle::Choice::from(1u8), is_zero),
            };

            let is_valid = is_valid_0 & is_valid_1 & is_valid_2;

            subtle::CtOption::new(C::identity(), is_valid & is_identity)
                .or_else(|| Self::resolve(x, sign).and_then(|c| subtle::CtOption::new(c, is_valid)))
        })
    }
}
