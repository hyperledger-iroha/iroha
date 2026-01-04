//! Pointer-ABI TLV helpers and type table.
//!
//! Layout (in INPUT region):
//! - type_id: u16 (BE)
//! - version: u8
//! - len: u32 (BE)
//! - payload: [u8; len]
//! - hash: [u8; 32] (Iroha Hash of payload)
//!
//! Validation rules:
//! - Entire TLV must lie within the INPUT region (no heap/stack).
//! - `version` currently must be 1.
//! - `type_id` must be known in the table below.
//! - Hash must match `iroha_crypto::Hash::new(payload)`.

use std::{cell::Cell, collections::HashSet, sync::OnceLock};

use crate::{SyscallPolicy, error::VMError};

/// Known pointer-ABI types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointerType {
    AccountId = 0x0001,
    AssetDefinitionId = 0x0002,
    Name = 0x0003,
    Json = 0x0004,
    NftId = 0x0005,
    /// Raw bytes payload
    Blob = 0x0006,
    /// Full AssetId (definition + account)
    AssetId = 0x0007,
    /// Domain identifier
    DomainId = 0x0008,
    /// Norito-encoded bytes payload (distinguishes structured Norito from raw blobs)
    NoritoBytes = 0x0009,
    /// Identifier of a data space / lane group used for cross-DS routing.
    DataSpaceId = 0x000A,
    /// Descriptor of an atomic cross-transaction (AXT) envelope.
    AxtDescriptor = 0x000B,
    /// Capability handle allowing guarded access to an asset DS inside an AXT.
    AssetHandle = 0x000C,
    /// Proof blob (deterministic Norito or compressed proof bytes).
    ProofBlob = 0x000D,
    /// Test-only pointer type used to exercise policy failures.
    #[cfg(test)]
    TestOnly = 0x0FFE,
}

impl PointerType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0001 => Some(Self::AccountId),
            0x0002 => Some(Self::AssetDefinitionId),
            0x0003 => Some(Self::Name),
            0x0004 => Some(Self::Json),
            0x0005 => Some(Self::NftId),
            0x0006 => Some(Self::Blob),
            0x0007 => Some(Self::AssetId),
            0x0008 => Some(Self::DomainId),
            0x0009 => Some(Self::NoritoBytes),
            0x000A => Some(Self::DataSpaceId),
            0x000B => Some(Self::AxtDescriptor),
            0x000C => Some(Self::AssetHandle),
            0x000D => Some(Self::ProofBlob),
            #[cfg(test)]
            0x0FFE => Some(Self::TestOnly),
            _ => None,
        }
    }

    /// Return all known pointer types in numeric ID order.
    pub const fn all() -> &'static [Self] {
        &[
            Self::AccountId,
            Self::AssetDefinitionId,
            Self::Name,
            Self::Json,
            Self::NftId,
            Self::Blob,
            Self::AssetId,
            Self::DomainId,
            Self::NoritoBytes,
            Self::DataSpaceId,
            Self::AxtDescriptor,
            Self::AssetHandle,
            Self::ProofBlob,
            #[cfg(test)]
            Self::TestOnly,
        ]
    }
}

/// Validated view of a TLV envelope.
pub struct Tlv<'a> {
    pub type_id: PointerType,
    pub version: u8,
    pub payload: &'a [u8],
}

impl<'a> Tlv<'a> {
    pub fn type_id_raw(&self) -> u16 {
        self.type_id as u16
    }
}

impl<'a> core::fmt::Debug for Tlv<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tlv")
            .field("type_id", &self.type_id)
            .field("version", &self.version)
            .field("len", &self.payload.len())
            .finish()
    }
}

/// Validate a TLV envelope provided as a raw byte slice and return a view over
/// its payload. The slice must contain the full header, payload, and hash.
pub fn validate_tlv_bytes(bytes: &[u8]) -> Result<Tlv<'_>, VMError> {
    if bytes.len() < 7 + iroha_crypto::Hash::LENGTH {
        return Err(VMError::NoritoInvalid);
    }

    let type_id = u16::from_be_bytes([bytes[0], bytes[1]]);
    let version = bytes[2];
    if version != 1 {
        return Err(VMError::NoritoInvalid);
    }

    let len = u32::from_be_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as usize;
    let total = 7usize
        .checked_add(len)
        .and_then(|x| x.checked_add(iroha_crypto::Hash::LENGTH))
        .ok_or(VMError::NoritoInvalid)?;
    if bytes.len() < total {
        return Err(VMError::NoritoInvalid);
    }

    let payload = &bytes[7..7 + len];
    let hash_bytes = &bytes[7 + len..total];
    let mut expected_hash = [0u8; iroha_crypto::Hash::LENGTH];
    expected_hash.copy_from_slice(hash_bytes);
    let computed: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
    if expected_hash != computed {
        return Err(VMError::NoritoInvalid);
    }

    let tlv_type = PointerType::from_u16(type_id).ok_or(VMError::NoritoInvalid)?;

    Ok(Tlv {
        type_id: tlv_type,
        version,
        payload,
    })
}

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
                if let Some((policy, abi_version)) = current_policy()
                    && !is_type_allowed_for_policy(policy, tlv.type_id)
                {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: abi_version,
                        type_id: tlv.type_id as u16,
                    });
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

/// Return the set of pointer types allowed for a given syscall ABI policy.
///
/// Policy notes:
/// - Experimental policies are empty until a versioned surface is defined.
fn allowed_types_for_policy(policy: SyscallPolicy) -> &'static HashSet<PointerType> {
    static ABI_V1: OnceLock<HashSet<PointerType>> = OnceLock::new();
    let v1 = ABI_V1.get_or_init(|| {
        HashSet::from([
            PointerType::AccountId,
            PointerType::AssetDefinitionId,
            PointerType::Name,
            PointerType::Json,
            PointerType::NftId,
            PointerType::Blob,
            PointerType::AssetId,
            PointerType::DomainId,
            PointerType::NoritoBytes,
            PointerType::DataSpaceId,
            PointerType::AxtDescriptor,
            PointerType::AssetHandle,
            PointerType::ProofBlob,
        ])
    });
    match policy {
        SyscallPolicy::AbiV1 => v1,
        SyscallPolicy::Experimental(_) => {
            static EMPTY: OnceLock<HashSet<PointerType>> = OnceLock::new();
            EMPTY.get_or_init(HashSet::new)
        }
    }
}

/// Expose the policy allowlist for callers that need to diff surfaces.
pub fn policy_pointer_types(policy: SyscallPolicy) -> &'static HashSet<PointerType> {
    allowed_types_for_policy(policy)
}

/// Check whether a pointer-ABI `type_id` is allowed for the given ABI policy.
pub fn is_type_allowed_for_policy(policy: SyscallPolicy, ty: PointerType) -> bool {
    allowed_types_for_policy(policy).contains(&ty)
}

thread_local! {
    static ENFORCED_POLICY: Cell<Option<(SyscallPolicy, u8)>> = const { Cell::new(None) };
}

/// Guard that enforces a pointer-ABI policy for the current thread during its lifetime.
pub struct PointerPolicyGuard {
    previous: Option<(SyscallPolicy, u8)>,
}

impl PointerPolicyGuard {
    /// Install a guard that enforces `policy` and annotates violations with `abi_version`.
    pub fn install(policy: SyscallPolicy, abi_version: u8) -> Self {
        let previous = ENFORCED_POLICY.with(|cell| cell.get());
        ENFORCED_POLICY.with(|cell| cell.set(Some((policy, abi_version))));
        Self { previous }
    }
}

impl Drop for PointerPolicyGuard {
    fn drop(&mut self) {
        ENFORCED_POLICY.with(|cell| cell.set(self.previous));
    }
}

fn current_policy() -> Option<(SyscallPolicy, u8)> {
    ENFORCED_POLICY.with(|cell| cell.get())
}

/// Render a markdown table of pointer types and their policy allowlists.
///
/// The format is suitable to be embedded between doc markers in
/// `crates/ivm/docs/pointer_abi.md` and validated by a doc-sync test.
pub fn render_pointer_types_markdown_table() -> String {
    // Keep the list in numeric ID order for stable diffs/readability
    let mut all: Vec<(u16, PointerType)> = vec![
        (PointerType::AccountId as u16, PointerType::AccountId),
        (
            PointerType::AssetDefinitionId as u16,
            PointerType::AssetDefinitionId,
        ),
        (PointerType::Name as u16, PointerType::Name),
        (PointerType::Json as u16, PointerType::Json),
        (PointerType::NftId as u16, PointerType::NftId),
        (PointerType::Blob as u16, PointerType::Blob),
        (PointerType::AssetId as u16, PointerType::AssetId),
        (PointerType::DomainId as u16, PointerType::DomainId),
        (PointerType::NoritoBytes as u16, PointerType::NoritoBytes),
        (PointerType::DataSpaceId as u16, PointerType::DataSpaceId),
        (
            PointerType::AxtDescriptor as u16,
            PointerType::AxtDescriptor,
        ),
        (PointerType::AssetHandle as u16, PointerType::AssetHandle),
        (PointerType::ProofBlob as u16, PointerType::ProofBlob),
    ];
    all.sort_by_key(|(id, _)| *id);

    let mut out = String::new();
    out.push_str("| ID | Name | ABI v1 |\n");
    out.push_str("|---|---|---|\n");
    for (id, ty) in all {
        let v1 = if is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty) {
            "OK"
        } else {
            "-"
        };
        // name via Debug fmt
        let name = format!("{ty:?}");
        let _ =
            core::fmt::Write::write_fmt(&mut out, format_args!("| 0x{id:04X} | {name} | {v1} |\n"));
    }
    out
}
