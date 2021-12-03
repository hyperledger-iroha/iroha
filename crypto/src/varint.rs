use eyre::{eyre, Error, Result};

/// Variable length unsigned int. [ref](https://github.com/multiformats/unsigned-varint)
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct VarUint {
    /// Contains validated varuint number
    payload: Vec<u8>,
}

//XXX: impl<T: AsRef<[u8]>> TryFrom<T> for VarUint
// does not compile

macro_rules! try_into_uint(
    { $( $ty:ty ),* } => {
        $(
            #[allow(trivial_numeric_casts)]
            impl TryInto<$ty> for VarUint {
                type Error = Error;
                fn try_into(self) -> Result<$ty> {
                    let VarUint { payload } = self;
                    if std::mem::size_of::<$ty>() * 8 < payload.len() * 7 {
                        return Err(eyre!(
                            concat!("Number is too large for type ", stringify!($ty))
                        ));
                    }
                    let offsets = (0..payload.len()).map(|i| i * 7);
                    let bytes = payload.into_iter().map(|byte| byte & 0b0111_1111);
                    let number = bytes
                        .zip(offsets)
                        .map(|(byte, offset)| (byte as $ty) << offset)
                        .fold(0, |number, part| number + part);
                    Ok(number)
                }
            }
        )*
    }
);

try_into_uint!(u8, u16, u32, u64, u128);

impl std::convert::From<VarUint> for Vec<u8> {
    fn from(int: VarUint) -> Self {
        int.payload
    }
}

impl std::convert::AsRef<[u8]> for VarUint {
    fn as_ref(&self) -> &[u8] {
        self.payload.as_ref()
    }
}

macro_rules! from_uint(
    { $( $ty:ty ),* } => {
        $(
            #[allow(trivial_numeric_casts)]
            impl std::convert::From<$ty> for VarUint {
                fn from(n: $ty) -> VarUint {
                    let zeros = n.leading_zeros();
                    let end = std::mem::size_of::<$ty>() * 8 - zeros as usize;

                    let mut payload = (0..end)
                        .step_by(7)
                        .map(|offset| (((n >> offset) as u8) | 0b1000_0000))
                        .collect::<Vec<_>>();
                    *payload.last_mut().unwrap() &= 0b0111_1111;

                    Self { payload }
                }
            }
        )*
    }
);

from_uint!(u8, u16, u32, u64, u128);

impl VarUint {
    /// Default constructor for [`VarUint`] number
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let idx = bytes
            .as_ref()
            .iter()
            .enumerate()
            .find(|&(_, &byte)| (byte & 0b1000_0000) == 0)
            .ok_or_else(|| eyre!("Last byte should be less than 128"))?
            .0;
        let (payload, empty) = bytes.as_ref().split_at(idx + 1);
        let payload = payload.to_vec();

        if empty.is_empty() {
            Ok(Self { payload })
        } else {
            Err(eyre!("Last byte shouldn't be followed by anything"))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::restriction)]

    use super::*;

    #[test]
    fn test_basic_into() {
        let n = 0x4000_u64;
        let varuint: VarUint = n.into();
        let vec: Vec<_> = varuint.into();
        let should_be = vec![0b1000_0000, 0b1000_0000, 0b0000_0001];
        assert_eq!(vec, should_be);
    }

    #[test]
    fn test_basic_from() {
        let n_should: u64 = VarUint::new([0b1000_0000, 0b1000_0000, 0b0000_0001])
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(0x4000_u64, n_should);
    }

    #[test]
    fn test_basic_into_from() {
        let n = 0x4000_u64;
        let varuint: VarUint = n.into();
        let n_new: u64 = varuint.try_into().unwrap();
        assert_eq!(n, n_new);
    }

    #[test]
    fn test_multihash() {
        let n = 0xed;
        let varuint: VarUint = n.into();
        let vec: Vec<_> = varuint.clone().into();
        let n_new: u64 = varuint.try_into().unwrap();
        assert_eq!(n, n_new);
        assert_eq!(vec, vec![0xed, 0x01]);
    }
}
