#[macro_export]
macro_rules! field_bits {
    ($field:ident) => {
        #[cfg(feature = "bits")]
        #[cfg_attr(docsrs, doc(cfg(feature = "bits")))]
        impl ff::PrimeFieldBits for $field {
            #[cfg(target_pointer_width = "64")]
            type ReprBits = [u64; Self::NUM_LIMBS];
            #[cfg(not(target_pointer_width = "64"))]
            type ReprBits = [u32; Self::NUM_LIMBS * 2];

            fn to_le_bits(&self) -> ff::FieldBits<Self::ReprBits> {
                use ff::PrimeField;
                let bytes: [u8; Self::SIZE] = self.to_repr().into();

                #[cfg(target_pointer_width = "64")]
                const STEP: usize = 8;
                #[cfg(not(target_pointer_width = "64"))]
                const STEP: usize = 4;

                let limbs = (0..Self::NUM_LIMBS * 8 / STEP)
                    .map(|off| {
                        #[cfg(target_pointer_width = "64")]
                        let limb = u64::from_le_bytes(
                            bytes[off * STEP..(off + 1) * STEP].try_into().unwrap(),
                        );
                        #[cfg(not(target_pointer_width = "64"))]
                        let limb = u32::from_le_bytes(
                            bytes[off * STEP..(off + 1) * STEP].try_into().unwrap(),
                        );

                        limb
                    })
                    .collect::<Vec<_>>();

                ff::FieldBits::new(limbs.try_into().unwrap())
            }

            fn char_le_bits() -> ff::FieldBits<Self::ReprBits> {
                #[cfg(target_pointer_width = "64")]
                let bits = ff::FieldBits::new(Self::MODULUS_LIMBS);
                #[cfg(not(target_pointer_width = "64"))]
                let bits = ff::FieldBits::new(Self::MODULUS_LIMBS_32);

                bits
            }
        }
    };
}

#[macro_export]
macro_rules! impl_from_u64 {
    ($field:ident) => {
        impl From<u64> for $field {
            fn from(val: u64) -> $field {
                let limbs = core::iter::once(val)
                    .chain(core::iter::repeat(0))
                    .take(Self::NUM_LIMBS)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();

                $field(limbs) * Self::R2
            }
        }
    };
}

#[macro_export]
macro_rules! impl_from_bool {
    ($field:ident) => {
        impl From<bool> for $field {
            fn from(val: bool) -> $field {
                let limbs = core::iter::once(u64::from(val))
                    .chain(core::iter::repeat(0))
                    .take(Self::NUM_LIMBS)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();

                $field(limbs) * Self::R2
            }
        }
    };
}

/// A macro to help define serialization and deserialization for prime field
/// implementations that use `$field::Repr`` representations. This assumes the
/// concerned type implements PrimeField (for from_repr, to_repr).
#[macro_export]
macro_rules! serialize_deserialize_primefield {
    ($field:ident) => {
        #[cfg(feature = "derive_serde")]
        impl<'de> ::serde::Deserialize<'de> for $field {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                use ::serde::de::Error as _;
                let bytes = if deserializer.is_human_readable() {
                    hex::serde::deserialize(deserializer)?
                } else {
                    ::serde_arrays::deserialize::<_, u8, { $field::SIZE }>(deserializer)?
                };
                use ff::PrimeField;
                Option::from(Self::from_repr(bytes.into())).ok_or_else(|| {
                    D::Error::custom("deserialized bytes don't encode a valid field element")
                })
            }
        }
        #[cfg(feature = "derive_serde")]
        impl ::serde::Serialize for $field {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use ff::PrimeField;
                if serializer.is_human_readable() {
                    hex::serde::serialize(self.to_repr().as_ref(), serializer)
                } else {
                    let bytes: [u8; $field::SIZE] = self.to_repr().into();
                    ::serde_arrays::serialize(&bytes, serializer)
                }
            }
        }
    };
}
