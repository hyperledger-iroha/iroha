//! One JSON key hash implementation shared by compile-time and runtime dispatch.
//!
//! The CRC feature uses a raw Castagnoli register seeded with all ones and
//! applies its 64-bit avalanche without a final complement. This internal field
//! dispatch hash does not use the xor-out convention of a CRC checksum.

/// Incremental key hash state, selected by the codec feature set.
pub(super) struct KeyHasher {
    #[cfg(feature = "crc-key-hash")]
    state: u32,
    #[cfg(not(feature = "crc-key-hash"))]
    state: u64,
}

impl KeyHasher {
    pub(super) const fn new() -> Self {
        Self {
            #[cfg(feature = "crc-key-hash")]
            state: u32::MAX,
            #[cfg(not(feature = "crc-key-hash"))]
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    const fn update_portable(&mut self, byte: u8) {
        #[cfg(feature = "crc-key-hash")]
        {
            self.state = crc32c_byte(self.state, byte);
        }
        #[cfg(not(feature = "crc-key-hash"))]
        {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(0x100_0000_01b3);
        }
    }

    #[inline]
    pub(super) fn update(&mut self, byte: u8) {
        #[cfg(all(
            feature = "crc-key-hash",
            feature = "simd-accel",
            target_arch = "aarch64"
        ))]
        if std::arch::is_aarch64_feature_detected!("crc") {
            // SAFETY: runtime detection guards the target-feature function.
            self.state = unsafe { crc32c_hardware(self.state, byte) };
            return;
        }
        #[cfg(all(
            feature = "crc-key-hash",
            feature = "simd-accel",
            target_arch = "x86_64"
        ))]
        if std::is_x86_feature_detected!("sse4.2") {
            // SAFETY: runtime detection guards the target-feature function.
            self.state = unsafe { crc32c_hardware(self.state, byte) };
            return;
        }
        self.update_portable(byte);
    }

    pub(super) const fn finish(self) -> u64 {
        #[cfg(feature = "crc-key-hash")]
        {
            let mut value = self.state as u64 ^ 0x9e37_79b9_7f4a_7c15;
            value ^= value >> 33;
            value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
            value ^= value >> 33;
            value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
            value ^ (value >> 33)
        }
        #[cfg(not(feature = "crc-key-hash"))]
        {
            self.state
        }
    }
}

pub(super) const fn hash_const(value: &str) -> u64 {
    let mut hash = KeyHasher::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        hash.update_portable(bytes[index]);
        index += 1;
    }
    hash.finish()
}

#[cfg(feature = "crc-key-hash")]
const fn crc32c_byte(mut state: u32, byte: u8) -> u32 {
    state ^= byte as u32;
    let mut bit = 0;
    while bit < 8 {
        let mask = (state & 1).wrapping_neg() & 0x82f6_3b78;
        state = (state >> 1) ^ mask;
        bit += 1;
    }
    state
}

#[cfg(all(
    feature = "crc-key-hash",
    feature = "simd-accel",
    target_arch = "aarch64"
))]
#[target_feature(enable = "crc")]
unsafe fn crc32c_hardware(state: u32, byte: u8) -> u32 {
    core::arch::aarch64::__crc32cb(state, byte)
}

#[cfg(all(
    feature = "crc-key-hash",
    feature = "simd-accel",
    target_arch = "x86_64"
))]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32c_hardware(state: u32, byte: u8) -> u32 {
    core::arch::x86_64::_mm_crc32_u8(state, byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_and_const_key_hashes_match() {
        for key in [
            "",
            "id",
            "public_key",
            "flow_label_bits",
            "quoted\"key",
            "é音🎼",
            "\n\t",
        ] {
            let mut runtime = KeyHasher::new();
            for byte in key.bytes() {
                runtime.update(byte);
            }
            assert_eq!(runtime.finish(), hash_const(key), "key {key:?}");
        }
    }

    #[test]
    fn every_byte_and_chained_state_matches_the_portable_backend() {
        for state_seed in ["", "previous-key-prefix", "é音🎼"] {
            for byte in u8::MIN..=u8::MAX {
                let mut portable = KeyHasher::new();
                let mut runtime = KeyHasher::new();
                for prefix in state_seed.bytes().chain([byte]) {
                    portable.update_portable(prefix);
                    runtime.update(prefix);
                }
                assert_eq!(portable.finish(), runtime.finish());
            }
        }
    }

    #[test]
    fn both_json_parsers_hash_logical_keys_identically() {
        for (encoded, logical) in [
            (r#""id""#, "id"),
            (r#""public_key""#, "public_key"),
            (r#""es\"caped""#, "es\"caped"),
            (r#""\u0069d""#, "id"),
            (r#""\uD834\uDD1E""#, "𝄞"),
            (r#""slash\\tab\t""#, "slash\\tab\t"),
        ] {
            let source = format!("{{{encoded}:1}}");
            let mut parser = crate::json::Parser::new(&source);
            parser.expect(b'{').unwrap();
            let mut tape = crate::json::TapeWalker::new(&source);
            tape.expect_object_start().unwrap();
            assert_eq!(parser.read_key_hash().unwrap(), hash_const(logical));
            assert_eq!(tape.read_key_hash().unwrap(), hash_const(logical));
        }
    }
}
