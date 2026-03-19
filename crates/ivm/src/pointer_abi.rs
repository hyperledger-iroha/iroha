//! Re-export pointer-ABI helpers from `ivm_abi`.
pub use ivm_abi::pointer_abi::*;

impl crate::memory::Memory {
    /// Validate a pointer-ABI TLV at `addr` in the INPUT region and return its view.
    pub fn validate_tlv(&self, addr: u64) -> Result<Tlv<'_>, crate::error::VMError> {
        use crate::error::VMError;

        // Enforce address lies in INPUT region and that header (2+1+4) fits
        let start = Self::INPUT_START;
        let end = Self::INPUT_START + Self::INPUT_SIZE;
        if !(addr >= start && addr + 7 <= end) {
            if crate::dev_env::decode_trace_enabled() {
                eprintln!("TLV header OOB: addr=0x{addr:08x} INPUT=[0x{start:08x}..0x{end:08x})");
            }
            return Err(VMError::NoritoInvalid);
        }
        // Read header to determine payload length before loading the full envelope
        let hdr = self
            .load_region(addr, 7)
            .map_err(|_| VMError::NoritoInvalid)?;
        let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
        if crate::dev_env::decode_trace_enabled() {
            let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
            let ver = hdr[2];
            eprintln!("TLV header: type=0x{type_id:04x} ver={ver} len={len} at=0x{addr:08x}");
        }

        // Total size including hash
        let total = 7usize
            .checked_add(len)
            .and_then(|x| x.checked_add(iroha_crypto::Hash::LENGTH))
            .ok_or(VMError::NoritoInvalid)?;
        // Bounds check entirely within INPUT
        if addr as usize + total > end as usize {
            return Err(VMError::NoritoInvalid);
        }
        // Load full envelope and validate header/hash/pointer type
        let envelope = self
            .load_region(addr, total as u64)
            .map_err(|_| VMError::NoritoInvalid)?;

        match validate_tlv_bytes(envelope) {
            Ok(tlv) => {
                if let Some((policy, abi_version)) = current_policy() {
                    let unsupported_abi = abi_version != 1;
                    let disallowed_type = !is_type_allowed_for_policy(policy, tlv.type_id);
                    if unsupported_abi || disallowed_type {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: abi_version,
                            type_id: tlv.type_id as u16,
                        });
                    }
                }
                Ok(tlv)
            }
            Err(err) => {
                if crate::dev_env::decode_trace_enabled() {
                    let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
                    let ver = hdr[2];
                    eprintln!(
                        "TLV validation failed: type=0x{type_id:04x} ver={ver} len={len} addr=0x{addr:08x} err={err:?}"
                    );
                }
                Err(err)
            }
        }
    }
}
