//! Canonical topology and SoraFS anchor for compact collective evaluated keys.
//!
//! The online evaluator consumes one compact relinearization key and the 31
//! compact Galois keys in the frozen T256 binary-rotation schedule.  The large
//! key payload is content addressed; consensus binds this small exact manifest
//! and never interprets network availability as a cryptographic validity bit.
use std::collections::BTreeSet;
use super::{
    MKHE_VERSION_V1, ZkAmsMkheErrorV1, ZkAmsMkheGovernedRosterWireV1,
    manifest::release_profile_v1,
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
    resource::derive_resource_certificate_v1,
};
use crate::vega::sponge::{Keccak256, keccak256};
const COLLECTIVE_EVALUATED_KEY_MANIFEST_TAG_V1: [u8; 4] = *b"ZAEK";
const COLLECTIVE_EVALUATED_KEY_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-manifest";
const COLLECTIVE_EVALUATED_KEY_TABLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-table";
const COLLECTIVE_EVALUATED_KEY_COUNT_V1: usize = ZK_AMS_T256_GALOIS_KEY_COUNT_V1 + 1;
const COLLECTIVE_EVALUATED_KEY_ENTRY_BYTES_V1: usize = 1 + 1 + 4 + 8 + 8 + 32 + 32 + 32;
const COLLECTIVE_EVALUATED_KEY_HEADER_BYTES_V1: usize =
    4 + 1 + 32 + 32 + 8 + 32 + 32 + 1 + 8 + 8 + 32 + 32 + 8 + 32 + 32 + 32 + 32;
const COLLECTIVE_EVALUATED_KEY_MANIFEST_BYTES_V1: usize = COLLECTIVE_EVALUATED_KEY_HEADER_BYTES_V1
    + COLLECTIVE_EVALUATED_KEY_COUNT_V1 * COLLECTIVE_EVALUATED_KEY_ENTRY_BYTES_V1;
/// Purpose of one compact collective evaluated key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
    /// Key-switch `c_2` from a multiplication back to the collective secret.
    Relinearization = 1,
    /// Key-switch one exact automorphed collective secret.
    Galois = 2,
}
impl ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
    fn decode(tag: u8) -> Result<Self, ZkAmsMkheErrorV1> {
        match tag {
            1 => Ok(Self::Relinearization),
            2 => Ok(Self::Galois),
            _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        }
    }
}
/// Content and proof identities for one exact seeded compact key blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyEntryV1 {
    ordinal: u8,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    galois_exponent: u32,
    payload_offset: u64,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvaluatedKeyEntryV1 {
    /// Construct one entry at its caller-asserted canonical position.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ordinal: u8,
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        galois_exponent: u32,
        payload_offset: u64,
        payload_bytes: u64,
        payload_blake3: [u8; 32],
        source_proof_set_digest: [u8; 32],
        cks_proof_set_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if payload_bytes == 0
            || payload_blake3 == [0; 32]
            || source_proof_set_digest == [0; 32]
            || cks_proof_set_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            ordinal,
            purpose,
            galois_exponent,
            payload_offset,
            payload_bytes,
            payload_blake3,
            source_proof_set_digest,
            cks_proof_set_digest,
        })
    }
    /// Zero-based exact key position: relinearization first, then schedule order.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
    /// Evaluated-key purpose.
    #[must_use]
    pub const fn purpose(self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }
    /// Odd automorphism exponent, or zero for the relinearization key.
    #[must_use]
    pub const fn galois_exponent(self) -> u32 {
        self.galois_exponent
    }
    /// Byte offset in the complete SoraFS payload.
    #[must_use]
    pub const fn payload_offset(self) -> u64 {
        self.payload_offset
    }
    /// Exact canonical seeded-key byte length.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }
    /// BLAKE3 digest of the exact key bytes at this entry.
    #[must_use]
    pub const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }
    /// Digest of all authenticated source RKG or Galois contribution proofs.
    #[must_use]
    pub const fn source_proof_set_digest(self) -> [u8; 32] {
        self.source_proof_set_digest
    }
    /// Digest of the exact full-roster CKS proof set compacting this key.
    #[must_use]
    pub const fn cks_proof_set_digest(self) -> [u8; 32] {
        self.cks_proof_set_digest
    }
}
/// SoraFS identities for the complete concatenated evaluated-key payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheEvaluatedKeySorafsPointerV1 {
    payload_blake3: [u8; 32],
    payload_bytes: u64,
    chunk_root: [u8; 32],
    sorafs_manifest_blake3: [u8; 32],
    chunker_profile_digest: [u8; 32],
}
impl ZkAmsMkheEvaluatedKeySorafsPointerV1 {
    /// Construct an externally verified content-addressed SoraFS pointer.
    pub fn new(
        payload_blake3: [u8; 32],
        payload_bytes: u64,
        chunk_root: [u8; 32],
        sorafs_manifest_blake3: [u8; 32],
        chunker_profile_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if payload_blake3 == [0; 32]
            || payload_bytes == 0
            || chunk_root == [0; 32]
            || sorafs_manifest_blake3 == [0; 32]
            || chunker_profile_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            payload_blake3,
            payload_bytes,
            chunk_root,
            sorafs_manifest_blake3,
            chunker_profile_digest,
        })
    }
    /// BLAKE3 digest of the complete payload.
    #[must_use]
    pub const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }
    /// Exact complete payload byte length.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }
    /// Root of the exact ordered SoraFS chunk list.
    #[must_use]
    pub const fn chunk_root(self) -> [u8; 32] {
        self.chunk_root
    }
    /// BLAKE3 digest of the canonical SoraFS manifest.
    #[must_use]
    pub const fn sorafs_manifest_blake3(self) -> [u8; 32] {
        self.sorafs_manifest_blake3
    }
    /// Digest of the governed SoraFS chunker profile.
    #[must_use]
    pub const fn chunker_profile_digest(self) -> [u8; 32] {
        self.chunker_profile_digest
    }
}
/// Small consensus-bound manifest for the complete compact evaluated-key set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyManifestV1 {
    version: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    galois_schedule_digest: [u8; 32],
    key_wire_bytes: u64,
    total_payload_bytes: u64,
    entry_table_digest: [u8; 32],
    entries: Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>,
    sorafs: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    manifest_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvaluatedKeyManifestV1 {
    /// Build the exact release topology without sorting or repairing caller input.
    pub fn new(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        transcript_digest: [u8; 32],
        entries: Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>,
        sorafs: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        let resources = derive_resource_certificate_v1(&profile, roster.parties().len())?;
        let schedule = zk_ams_t256_galois_key_schedule_v1()?;
        validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
        let mut value = Self {
            version: MKHE_VERSION_V1,
            profile_digest: roster.profile_digest(),
            roster_digest: roster.roster_digest(),
            epoch: roster.epoch(),
            transcript_digest,
            galois_schedule_digest: schedule.digest,
            key_wire_bytes: resources.seeded_collective_relinearization_key_wire_bytes,
            total_payload_bytes: resources.total_collective_evaluated_key_artifact_bytes,
            entry_table_digest: [0; 32],
            entries,
            sorafs,
            manifest_digest: [0; 32],
        };
        value.validate_fields(roster)?;
        value.entry_table_digest = entry_table_digest(&value.entries);
        value.manifest_digest = manifest_digest(&value);
        value.validate(roster)?;
        Ok(value)
    }
    /// Encode after revalidating against the independently trusted roster.
    pub fn encode(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
    ) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.validate(roster)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(COLLECTIVE_EVALUATED_KEY_MANIFEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.extend_from_slice(&COLLECTIVE_EVALUATED_KEY_MANIFEST_TAG_V1);
        bytes.push(self.version);
        bytes.extend_from_slice(&self.profile_digest);
        bytes.extend_from_slice(&self.roster_digest);
        bytes.extend_from_slice(&self.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.transcript_digest);
        bytes.extend_from_slice(&self.galois_schedule_digest);
        bytes.push(
            u8::try_from(self.entries.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        );
        bytes.extend_from_slice(&self.key_wire_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.total_payload_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.entry_table_digest);
        for entry in &self.entries {
            encode_entry(&mut bytes, *entry);
        }
        bytes.extend_from_slice(&self.sorafs.payload_blake3);
        bytes.extend_from_slice(&self.sorafs.payload_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.sorafs.chunk_root);
        bytes.extend_from_slice(&self.sorafs.sorafs_manifest_blake3);
        bytes.extend_from_slice(&self.sorafs.chunker_profile_digest);
        bytes.extend_from_slice(&self.manifest_digest);
        if bytes.len() != COLLECTIVE_EVALUATED_KEY_MANIFEST_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }
    /// Decode exactly under an independently trusted roster and transcript.
    pub fn decode_exact(
        bytes: &[u8],
        roster: &ZkAmsMkheGovernedRosterWireV1,
        expected_transcript_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != COLLECTIVE_EVALUATED_KEY_MANIFEST_BYTES_V1
            || expected_transcript_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        expect_bytes(
            bytes,
            &mut cursor,
            &COLLECTIVE_EVALUATED_KEY_MANIFEST_TAG_V1,
        )?;
        expect_u8(bytes, &mut cursor, MKHE_VERSION_V1)?;
        expect_array(bytes, &mut cursor, roster.profile_digest())?;
        expect_array(bytes, &mut cursor, roster.roster_digest())?;
        expect_u64(bytes, &mut cursor, roster.epoch())?;
        expect_array(bytes, &mut cursor, expected_transcript_digest)?;
        let schedule = zk_ams_t256_galois_key_schedule_v1()?;
        expect_array(bytes, &mut cursor, schedule.digest)?;
        expect_u8(
            bytes,
            &mut cursor,
            u8::try_from(COLLECTIVE_EVALUATED_KEY_COUNT_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        let resources =
            derive_resource_certificate_v1(&release_profile_v1(), roster.parties().len())?;
        expect_u64(
            bytes,
            &mut cursor,
            resources.seeded_collective_relinearization_key_wire_bytes,
        )?;
        expect_u64(
            bytes,
            &mut cursor,
            resources.total_collective_evaluated_key_artifact_bytes,
        )?;
        let encoded_table_digest = read_array(bytes, &mut cursor)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(COLLECTIVE_EVALUATED_KEY_COUNT_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..COLLECTIVE_EVALUATED_KEY_COUNT_V1 {
            entries.push(decode_entry(bytes, &mut cursor)?);
        }
        if encoded_table_digest != entry_table_digest(&entries) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let sorafs = ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
            read_array(bytes, &mut cursor)?,
            read_u64(bytes, &mut cursor)?,
            read_array(bytes, &mut cursor)?,
            read_array(bytes, &mut cursor)?,
            read_array(bytes, &mut cursor)?,
        )?;
        let encoded_manifest_digest = read_array(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let value = Self::new(roster, expected_transcript_digest, entries, sorafs)?;
        if value.entry_table_digest != encoded_table_digest
            || value.manifest_digest != encoded_manifest_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(value)
    }
    /// Exact canonical ordered key entries.
    #[must_use]
    pub fn entries(&self) -> &[ZkAmsMkheCollectiveEvaluatedKeyEntryV1] {
        &self.entries
    }
    /// Complete SoraFS payload pointer.
    #[must_use]
    pub const fn sorafs(&self) -> ZkAmsMkheEvaluatedKeySorafsPointerV1 {
        self.sorafs
    }
    /// Digest of the exact entry table.
    #[must_use]
    pub const fn entry_table_digest(&self) -> [u8; 32] {
        self.entry_table_digest
    }
    /// Consensus identity of the complete manifest.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
    fn validate_fields(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let resources = derive_resource_certificate_v1(&profile, roster.parties().len())?;
        let schedule = zk_ams_t256_galois_key_schedule_v1()?;
        validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != roster.profile_digest()
            || self.profile_digest != profile.digest()?
            || self.roster_digest != roster.roster_digest()
            || self.epoch != roster.epoch()
            || self.transcript_digest == [0; 32]
            || self.galois_schedule_digest != schedule.digest
            || self.key_wire_bytes != resources.seeded_collective_relinearization_key_wire_bytes
            || self.total_payload_bytes != resources.total_collective_evaluated_key_artifact_bytes
            || self.entries.len() != COLLECTIVE_EVALUATED_KEY_COUNT_V1
            || self.sorafs.payload_bytes != self.total_payload_bytes
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut payload_digests = BTreeSet::new();
        let mut source_digests = BTreeSet::new();
        let mut cks_digests = BTreeSet::new();
        for (index, entry) in self.entries.iter().copied().enumerate() {
            let expected_offset = self
                .key_wire_bytes
                .checked_mul(
                    u64::try_from(index).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if usize::from(entry.ordinal) != index
                || entry.payload_offset != expected_offset
                || entry.payload_bytes != self.key_wire_bytes
                || entry.payload_blake3 == [0; 32]
                || entry.source_proof_set_digest == [0; 32]
                || entry.cks_proof_set_digest == [0; 32]
                || !payload_digests.insert(entry.payload_blake3)
                || !source_digests.insert(entry.source_proof_set_digest)
                || !cks_digests.insert(entry.cks_proof_set_digest)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            if index == 0 {
                if entry.purpose != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization
                    || entry.galois_exponent != 0
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            } else {
                let expected = schedule
                    .entries
                    .get(index - 1)
                    .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
                if entry.purpose != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois
                    || entry.galois_exponent != expected.exponent
                {
                    return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
                }
            }
        }
        Ok(())
    }
    fn validate(&self, roster: &ZkAmsMkheGovernedRosterWireV1) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_fields(roster)?;
        if self.entry_table_digest == [0; 32]
            || self.entry_table_digest != entry_table_digest(&self.entries)
            || self.manifest_digest == [0; 32]
            || self.manifest_digest != manifest_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
fn encode_entry(bytes: &mut Vec<u8>, entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1) {
    bytes.push(entry.ordinal);
    bytes.push(entry.purpose as u8);
    bytes.extend_from_slice(&entry.galois_exponent.to_be_bytes());
    bytes.extend_from_slice(&entry.payload_offset.to_be_bytes());
    bytes.extend_from_slice(&entry.payload_bytes.to_be_bytes());
    bytes.extend_from_slice(&entry.payload_blake3);
    bytes.extend_from_slice(&entry.source_proof_set_digest);
    bytes.extend_from_slice(&entry.cks_proof_set_digest);
}
fn decode_entry(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheErrorV1> {
    ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
        read_u8(bytes, cursor)?,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::decode(read_u8(bytes, cursor)?)?,
        read_u32(bytes, cursor)?,
        read_u64(bytes, cursor)?,
        read_u64(bytes, cursor)?,
        read_array(bytes, cursor)?,
        read_array(bytes, cursor)?,
        read_array(bytes, cursor)?,
    )
}
fn entry_table_digest(entries: &[ZkAmsMkheCollectiveEvaluatedKeyEntryV1]) -> [u8; 32] {
    let mut frame = Vec::with_capacity(8 + entries.len() * COLLECTIVE_EVALUATED_KEY_ENTRY_BYTES_V1);
    frame.extend_from_slice(COLLECTIVE_EVALUATED_KEY_TABLE_DOMAIN_V1);
    frame.extend_from_slice(&(entries.len() as u64).to_be_bytes());
    for entry in entries {
        encode_entry(&mut frame, *entry);
    }
    keccak256(&frame)
}
fn manifest_digest(manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_EVALUATED_KEY_MANIFEST_DOMAIN_V1);
    hash.update(&[manifest.version]);
    hash.update(&manifest.profile_digest);
    hash.update(&manifest.roster_digest);
    hash.update(&manifest.epoch.to_be_bytes());
    hash.update(&manifest.transcript_digest);
    hash.update(&manifest.galois_schedule_digest);
    hash.update(&manifest.key_wire_bytes.to_be_bytes());
    hash.update(&manifest.total_payload_bytes.to_be_bytes());
    hash.update(&manifest.entry_table_digest);
    hash.update(&manifest.sorafs.payload_blake3);
    hash.update(&manifest.sorafs.payload_bytes.to_be_bytes());
    hash.update(&manifest.sorafs.chunk_root);
    hash.update(&manifest.sorafs.sorafs_manifest_blake3);
    hash.update(&manifest.sorafs.chunker_profile_digest);
    hash.finalize()
}
fn read_exact<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(length)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = end;
    Ok(value)
}
fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, ZkAmsMkheErrorV1> {
    Ok(read_exact(bytes, cursor, 1)?[0])
}
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ZkAmsMkheErrorV1> {
    Ok(u32::from_be_bytes(
        read_exact(bytes, cursor, 4)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    ))
}
fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, ZkAmsMkheErrorV1> {
    Ok(u64::from_be_bytes(
        read_exact(bytes, cursor, 8)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    ))
}
fn read_array(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    read_exact(bytes, cursor, 32)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
fn expect_bytes(bytes: &[u8], cursor: &mut usize, expected: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    if read_exact(bytes, cursor, expected.len())? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
fn expect_u8(bytes: &[u8], cursor: &mut usize, expected: u8) -> Result<(), ZkAmsMkheErrorV1> {
    if read_u8(bytes, cursor)? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
fn expect_u64(bytes: &[u8], cursor: &mut usize, expected: u64) -> Result<(), ZkAmsMkheErrorV1> {
    if read_u64(bytes, cursor)? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
fn expect_array(
    bytes: &[u8],
    cursor: &mut usize,
    expected: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1> {
    if read_array(bytes, cursor)? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::zk_ams::mkhe::ZkAmsMkhePartyIdV1;
    const TRANSCRIPT: [u8; 32] = [0xa1; 32];
    fn roster() -> ZkAmsMkheGovernedRosterWireV1 {
        let parties = core::array::from_fn(|index| {
            ZkAmsMkhePartyIdV1::new(
                [u8::try_from(index + 1).expect("release roster index fits u8"); 32],
            )
            .expect("nonzero party")
        });
        ZkAmsMkheGovernedRosterWireV1::new(
            release_profile_v1().digest().expect("profile digest"),
            7,
            parties,
        )
        .expect("release roster")
    }
    fn digest(seed: usize, domain: u8) -> [u8; 32] {
        let mut value = [domain; 32];
        value[0] = u8::try_from(seed + 1).expect("test seed fits u8");
        value[31] ^= u8::try_from(seed).expect("test seed fits u8");
        value
    }
    fn entries() -> Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1> {
        let roster = roster();
        let resources =
            derive_resource_certificate_v1(&release_profile_v1(), roster.parties().len())
                .expect("resources");
        let schedule = zk_ams_t256_galois_key_schedule_v1().expect("schedule");
        (0..COLLECTIVE_EVALUATED_KEY_COUNT_V1)
            .map(|index| {
                let (purpose, exponent) = if index == 0 {
                    (ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization, 0)
                } else {
                    (
                        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
                        schedule.entries[index - 1].exponent,
                    )
                };
                ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
                    u8::try_from(index).expect("entry index fits u8"),
                    purpose,
                    exponent,
                    resources
                        .seeded_collective_relinearization_key_wire_bytes
                        .checked_mul(u64::try_from(index).expect("index fits u64"))
                        .expect("offset"),
                    resources.seeded_collective_relinearization_key_wire_bytes,
                    digest(index, 0x41),
                    digest(index, 0x61),
                    digest(index, 0x81),
                )
                .expect("entry")
            })
            .collect()
    }
    fn pointer() -> ZkAmsMkheEvaluatedKeySorafsPointerV1 {
        ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
            [0xb1; 32],
            48_452_611_616,
            [0xb2; 32],
            [0xb3; 32],
            [0xb4; 32],
        )
        .expect("SoraFS pointer")
    }
    fn manifest() -> ZkAmsMkheCollectiveEvaluatedKeyManifestV1 {
        ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(&roster(), TRANSCRIPT, entries(), pointer())
            .expect("manifest")
    }
    #[test]
    fn exact_collective_topology_and_sorafs_wire_roundtrip() {
        let roster = roster();
        let manifest = manifest();
        assert_eq!(manifest.entries.len(), 32);
        assert_eq!(manifest.key_wire_bytes, 1_514_144_113);
        assert_eq!(manifest.total_payload_bytes, 48_452_611_616);
        assert_eq!(manifest.sorafs.payload_bytes(), 48_452_611_616);
        assert_ne!(manifest.entry_table_digest(), [0; 32]);
        assert_ne!(manifest.manifest_digest(), [0; 32]);
        assert_eq!(
            manifest.entries[0].purpose(),
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization
        );
        let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
        assert_eq!(
            manifest.entries[1..]
                .iter()
                .map(|entry| entry.galois_exponent())
                .collect::<Vec<_>>(),
            schedule
                .entries
                .iter()
                .map(|entry| entry.exponent)
                .collect::<Vec<_>>()
        );
        let bytes = manifest.encode(&roster).expect("encode");
        assert_eq!(bytes.len(), COLLECTIVE_EVALUATED_KEY_MANIFEST_BYTES_V1);
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(&bytes, &roster, TRANSCRIPT,),
            Ok(manifest)
        );
    }
    #[test]
    fn missing_duplicate_reordered_and_spliced_entries_fail_closed() {
        let roster = roster();
        let baseline = entries();
        let mut missing = baseline.clone();
        missing.pop();
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(&roster, TRANSCRIPT, missing, pointer(),),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        let mut reordered = baseline.clone();
        reordered.swap(1, 2);
        assert!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                reordered,
                pointer(),
            )
            .is_err()
        );
        let mut duplicate_payload = baseline.clone();
        duplicate_payload[2].payload_blake3 = duplicate_payload[1].payload_blake3;
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                duplicate_payload,
                pointer(),
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        let mut duplicate_proof = baseline.clone();
        duplicate_proof[2].cks_proof_set_digest = duplicate_proof[1].cks_proof_set_digest;
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                duplicate_proof,
                pointer(),
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        let mut wrong_exponent = baseline.clone();
        wrong_exponent[1].galois_exponent ^= 2;
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                wrong_exponent,
                pointer(),
            ),
            Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
        );
        let mut wrong_offset = baseline;
        wrong_offset[9].payload_offset += 1;
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                wrong_offset,
                pointer(),
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }
    #[test]
    fn roster_transcript_transport_and_wire_mutations_fail_closed() {
        let roster = roster();
        let manifest = manifest();
        let bytes = manifest.encode(&roster).unwrap();
        assert!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(
                &bytes[..bytes.len() - 1],
                &roster,
                TRANSCRIPT,
            )
            .is_err()
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(
            &trailing,
            &roster,
            TRANSCRIPT,
        )
        .is_err());
        assert!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(&bytes, &roster, [0xa2; 32],)
                .is_err()
        );
        for offset in [0, 4, 5, 37, 69, 77, 109, 141, 142, 150, 158, 190] {
            let mut mutation = bytes.clone();
            mutation[offset] ^= 1;
            assert!(
                ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(
                    &mutation, &roster, TRANSCRIPT,
                )
                .is_err(),
                "mutation at byte {offset} was accepted"
            );
        }
        assert_eq!(
            ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
                [0xb1; 32], 0, [0xb2; 32], [0xb3; 32], [0xb4; 32],
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        let wrong_length = ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
            [0xb1; 32],
            48_452_611_615,
            [0xb2; 32],
            [0xb3; 32],
            [0xb4; 32],
        )
        .unwrap();
        assert_eq!(
            ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
                &roster,
                TRANSCRIPT,
                entries(),
                wrong_length,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }
}
