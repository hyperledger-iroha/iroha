//! Host-side validation for the non-shipping Android candidate-lab seed set.
use super::{
    KagemushaCanonicalDecodeSchema, KagemushaNoteOpeningV2, KagemushaOutputMembershipPathsV4,
    KagemushaRecipientOutputProverMaterialV2, decode_canonical_kagemusha_archive,
    derive_kagemusha_owned_note_v2,
};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    offline::{
        KagemushaRecipientPaymentRequestV2, KagemushaRecursiveSpendCandidateV4,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2, KagemushaTopUpFinalityProofV2,
        KagemushaTopUpFinalityRosterArtifactV2,
    },
};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, OpenOptions},
    io::Read,
    ops::Deref,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};
use zeroize::{Zeroize as _, Zeroizing};
const REPORT_SCHEMA: &str = "iroha.kagemusha.android_candidate_scenario_validation.v1";
const INVENTORY_DOMAIN: &[u8] = b"iroha.kagemusha.android-candidate-scenario-inventory.v1\0";
const MAX_CANDIDATE_BYTES: u64 = 1024 * 1024;
const MAX_ROSTER_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SCENARIO_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const SCENARIO_FILES: [&str; 33] = [
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
];
#[derive(Default)]
pub(super) struct ScenarioPayloads(pub(super) BTreeMap<String, Vec<u8>>);
impl Deref for ScenarioPayloads {
    type Target = BTreeMap<String, Vec<u8>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for ScenarioPayloads {
    fn drop(&mut self) {
        for payload in self.0.values_mut() {
            payload.zeroize();
        }
        self.0.clear();
    }
}
fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64, u32, u64, u32, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.nlink(),
        metadata.uid(),
        metadata.size(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}
fn read_at_most(reader: &mut impl Read, maximum: u64, bytes: &mut Vec<u8>) -> Result<(), String> {
    Read::by_ref(reader)
        .take(maximum.saturating_add(1))
        .read_to_end(bytes)
        .map_err(|error| format!("failed to read bounded input: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(format!(
            "input grew beyond its {maximum}-byte bound while reading"
        ));
    }
    Ok(())
}
pub(super) fn read_private_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    if !path.is_absolute() {
        return Err(format!("input path must be absolute: {}", path.display()));
    }
    let before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if !before.file_type().is_file()
        || before.nlink() != 1
        || before.uid() != unsafe { libc::geteuid() }
        || before.mode() & 0o077 != 0
        || before.size() == 0
        || before.size() > maximum
    {
        return Err(format!(
            "input must be owner-private, nonempty, singly linked, regular, and bounded: {}",
            path.display()
        ));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("failed to inspect open {}: {error}", path.display()))?;
    if metadata_identity(&opened) != metadata_identity(&before) {
        return Err(format!("input changed before open: {}", path.display()));
    }
    let capacity = usize::try_from(opened.size())
        .map_err(|_| format!("input is too large for this host: {}", path.display()))?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
    read_at_most(&mut file, maximum, &mut bytes)
        .map_err(|error| format!("{error}: {}", path.display()))?;
    let after_open = file
        .metadata()
        .map_err(|error| format!("failed to reinspect open {}: {error}", path.display()))?;
    let after_path = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to reinspect {}: {error}", path.display()))?;
    if bytes.len() as u64 != opened.size()
        || metadata_identity(&after_open) != metadata_identity(&before)
        || metadata_identity(&after_path) != metadata_identity(&before)
    {
        return Err(format!("input changed while read: {}", path.display()));
    }
    Ok(std::mem::take(&mut *bytes))
}
#[cfg(test)]
mod bounded_read_tests {
    use super::read_at_most;
    use std::io::Cursor;
    #[test]
    fn reader_cannot_append_beyond_the_admitted_size() {
        let mut reader = Cursor::new(vec![0xA5; 9]);
        let mut bytes = Vec::new();
        assert!(read_at_most(&mut reader, 8, &mut bytes).is_err());
        assert_eq!(bytes.len(), 9, "only the one-byte overflow probe is read");
    }
}
pub(super) fn load_scenario(directory: &Path) -> Result<ScenarioPayloads, String> {
    if !directory.is_absolute() {
        return Err("scenario directory path must be absolute".to_owned());
    }
    let before = fs::symlink_metadata(directory)
        .map_err(|error| format!("failed to inspect scenario directory: {error}"))?;
    if !before.file_type().is_dir()
        || before.uid() != unsafe { libc::geteuid() }
        || before.mode() & 0o077 != 0
    {
        return Err("scenario directory must be a real owner-private directory".to_owned());
    }
    let actual = fs::read_dir(directory)
        .map_err(|error| format!("failed to enumerate scenario directory: {error}"))?
        .map(|entry| {
            entry
                .map_err(|error| format!("failed to enumerate scenario directory: {error}"))?
                .file_name()
                .into_string()
                .map_err(|_| "scenario file name is not UTF-8".to_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = SCENARIO_FILES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "scenario inventory differs; missing={:?}, extra={:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&expected).collect::<Vec<_>>()
        ));
    }
    let mut files = ScenarioPayloads::default();
    for name in &expected {
        files.0.insert(
            name.clone(),
            read_private_regular(&directory.join(name), MAX_SCENARIO_BYTES)?,
        );
    }
    let after = fs::symlink_metadata(directory)
        .map_err(|error| format!("failed to reinspect scenario directory: {error}"))?;
    if metadata_identity(&after) != metadata_identity(&before) {
        return Err("scenario directory changed while validating".to_owned());
    }
    Ok(files)
}
pub(super) fn bytes<'a>(
    files: &'a BTreeMap<String, Vec<u8>>,
    name: &str,
) -> Result<&'a [u8], String> {
    files
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing scenario input {name}"))
}
fn decode<T>(payload: &[u8], label: &str) -> Result<T, String>
where
    T: KagemushaCanonicalDecodeSchema + norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    decode_canonical_kagemusha_archive(payload)
        .map_err(|_| format!("{label} is not one canonical typed Norito archive"))
}
pub(super) fn digest32(files: &BTreeMap<String, Vec<u8>>, name: &str) -> Result<[u8; 32], String> {
    bytes(files, name)?
        .try_into()
        .map_err(|_| format!("{name} must contain exactly 32 bytes"))
}
pub(super) fn positive_decimal(
    files: &BTreeMap<String, Vec<u8>>,
    name: &str,
) -> Result<u64, String> {
    let payload = bytes(files, name)?;
    let line = payload
        .strip_suffix(b"\n")
        .ok_or_else(|| format!("{name} must end in one newline"))?;
    if line.is_empty()
        || line.len() > 19
        || line[0] == b'0'
        || line.iter().any(|byte| !byte.is_ascii_digit())
    {
        return Err(format!(
            "{name} is not one canonical positive Android decimal"
        ));
    }
    let value = std::str::from_utf8(line)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0 && *value <= i64::MAX as u64)
        .ok_or_else(|| format!("{name} is outside the positive Android Long corridor"))?;
    Ok(value)
}
fn validate_request_opening(
    request: &KagemushaRecipientPaymentRequestV2,
    opening: &KagemushaNoteOpeningV2,
    verified_at_ms: u64,
    candidate: &KagemushaRecursiveSpendCandidateV4,
) -> Result<KagemushaSpendableNoteDescriptorV2, String> {
    request.validate_at(verified_at_ms).map_err(|_| {
        "recipient request signature, lifetime, or public binding is invalid".to_owned()
    })?;
    let manifest = &candidate.manifest;
    if request.network_id != manifest.network_id
        || request.asset != manifest.asset
        || request.amount.scale != manifest.asset_scale
    {
        return Err("recipient request mismatches candidate network/asset/scale".to_owned());
    }
    opening
        .validate()
        .map_err(|_| "recipient opening is invalid".to_owned())?;
    let material: KagemushaRecipientOutputProverMaterialV2 = decode(
        &request.sender_output_prover_material,
        "recipient sender-output prover material",
    )?;
    let owner_tag =
        iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
            &opening.spend_key,
            opening.diversifier,
        )
        .map_err(|_| "recipient opening owner tag is invalid".to_owned())?;
    if material.amount != request.amount.atomic_units
        || material.rho != opening.rho
        || material.owner_tag != owner_tag
    {
        return Err("recipient opening does not bind the signed prover material".to_owned());
    }
    let note = derive_kagemusha_owned_note_v2(
        &request.network_id,
        &request.asset,
        request.amount,
        opening,
    )
    .map_err(|_| "failed to derive recipient note from its opening".to_owned())?;
    if note != request.recipient_output {
        return Err("recipient opening does not derive the signed recipient output".to_owned());
    }
    Ok(note)
}
fn validate_request_material_without_opening(
    request: &KagemushaRecipientPaymentRequestV2,
) -> Result<(), String> {
    let material: KagemushaRecipientOutputProverMaterialV2 = decode(
        &request.sender_output_prover_material,
        "recipient sender-output prover material",
    )?;
    if material.amount != request.amount.atomic_units
        || material.rho == [0; 32]
        || material.owner_tag == [0; 32]
    {
        return Err(
            "recipient sender-output prover material is inert or amount-mismatched".to_owned(),
        );
    }
    let commitment = iroha_core::zk::confidential_v2::derive_confidential_note_v2(
        &request.asset.to_string(),
        material.amount,
        material.rho,
        material.owner_tag,
    )
    .map_err(|_| "failed to derive recipient commitment from signed prover material".to_owned())?;
    if commitment != request.recipient_output.note_commitment {
        return Err("signed recipient prover material does not derive its output".to_owned());
    }
    Ok(())
}
fn parse_canonical_account_id_for_chain(
    account_text: &str,
    account_chain_discriminant: u16,
) -> Result<AccountId, String> {
    // Parsing and canonical spelling both consult the thread-local network
    // context. Keep one explicit scope around the complete roundtrip so a
    // Taira literal cannot be silently re-rendered with SORA's default.
    let _chain_discriminant = ChainDiscriminantGuard::enter(account_chain_discriminant);
    let parsed = AccountId::parse_encoded(account_text)
        .map_err(|_| "redemption account is not a typed AccountId".to_owned())?;
    if parsed.canonical() != account_text {
        return Err("redemption account is not in canonical AccountId spelling".to_owned());
    }
    Ok(parsed.into_account_id())
}
pub(super) fn scenario_inventory_sha256(
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<[u8; 32], String> {
    let mut hasher = Sha256::new();
    hasher.update(INVENTORY_DOMAIN);
    hasher.update(
        u32::try_from(files.len())
            .map_err(|_| "scenario inventory count overflow".to_owned())?
            .to_be_bytes(),
    );
    for (name, payload) in files {
        let path = format!("scenario/{name}");
        hasher.update(
            u32::try_from(path.len())
                .map_err(|_| "scenario path length overflow".to_owned())?
                .to_be_bytes(),
        );
        hasher.update(path.as_bytes());
        hasher.update(
            u64::try_from(payload.len())
                .map_err(|_| "scenario file length overflow".to_owned())?
                .to_be_bytes(),
        );
        hasher.update(Sha256::digest(payload));
    }
    Ok(hasher.finalize().into())
}
/// Validate the exact 33-file Android candidate scenario against real bridge
/// carrier types, the candidate manifest, and the production finality verifier.
///
/// The returned bytes are canonical single-line JSON containing only public
/// content identities. Secret note openings are decoded in the bridge type and
/// zeroized on drop; they are never copied into the report.
pub fn validate_kagemusha_candidate_scenario_directory_v1(
    candidate_record_path: &Path,
    candidate_roster_path: &Path,
    scenario_directory: &Path,
    account_chain_discriminant: u16,
) -> Result<Vec<u8>, String> {
    let candidate_bytes = read_private_regular(candidate_record_path, MAX_CANDIDATE_BYTES)?;
    let candidate: KagemushaRecursiveSpendCandidateV4 =
        decode(&candidate_bytes, "candidate record")?;
    candidate
        .validate()
        .map_err(|_| "candidate record is invalid".to_owned())?;
    let candidate_sha256 = candidate
        .sha256()
        .map_err(|_| "failed to identify candidate record".to_owned())?;
    if candidate_sha256 != <[u8; 32]>::from(Sha256::digest(&candidate_bytes)) {
        return Err("candidate record identity is not canonical".to_owned());
    }
    let manifest_bytes =
        norito::to_bytes(&candidate.manifest).map_err(|_| "failed to encode candidate manifest")?;
    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
    let candidate_roster_bytes = read_private_regular(candidate_roster_path, MAX_ROSTER_BYTES)?;
    let candidate_roster: KagemushaTopUpFinalityRosterArtifactV2 =
        decode(&candidate_roster_bytes, "candidate finality roster")?;
    candidate_roster
        .validate()
        .map_err(|_| "candidate finality roster cryptography is invalid".to_owned())?;
    let files = load_scenario(scenario_directory)?;
    if bytes(&files, "init-top-up-finality-roster-artifact-v2.norito")? != candidate_roster_bytes {
        return Err(
            "scenario finality roster is not byte-identical to the candidate roster".to_owned(),
        );
    }
    let scenario_roster: KagemushaTopUpFinalityRosterArtifactV2 = decode(
        bytes(&files, "init-top-up-finality-roster-artifact-v2.norito")?,
        "scenario finality roster",
    )?;
    let anchor: iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4 = decode(
        bytes(&files, "init-top-up-anchor-v4.norito")?,
        "top-up anchor",
    )?;
    let proof: KagemushaTopUpFinalityProofV2 = decode(
        bytes(&files, "init-top-up-finality-proof-v2.norito")?,
        "top-up finality proof",
    )?;
    let verified = iroha_core::zk::kagemusha_finality::
        verify_kagemusha_topup_finality_candidate_evidence_lab_v4(
            &proof,
            &scenario_roster,
            &anchor,
            &candidate.manifest,
            manifest_sha256,
        )
        .map_err(|error| format!("candidate-bound top-up finality proof failed: {error}"))?;
    if verified.height() != anchor.finalized_height
        || verified.manifest_sha256() != manifest_sha256
        || verified.roster_sha256() != candidate.manifest.topup_finality_roster_artifact.sha256
    {
        return Err("typed finality result is not bound to the candidate inputs".to_owned());
    }
    let init_opening: KagemushaNoteOpeningV2 =
        decode(bytes(&files, "init-opening-v2.norito")?, "init opening")?;
    let init_note = derive_kagemusha_owned_note_v2(
        &candidate.manifest.network_id,
        &candidate.manifest.asset,
        anchor.amount,
        &init_opening,
    )
    .map_err(|_| "failed to derive the init note from its opening".to_owned())?;
    if init_note != anchor.current_note {
        return Err("init opening does not derive the finalized anchor note".to_owned());
    }
    let init_membership: KagemushaOutputMembershipPathsV4 = decode(
        bytes(&files, "init-output-membership-v4.norito")?,
        "init output membership",
    )?;
    init_membership
        .for_init_v4(&anchor)
        .map_err(|_| "init output membership is not bound to the anchor roots/note".to_owned())?;
    let hop_one_verified_at = positive_decimal(&files, "append-hop-01-verified-at-ms.txt")?;
    let hop_two_verified_at = positive_decimal(&files, "append-hop-02-verified-at-ms.txt")?;
    let duplicate_verified_at = positive_decimal(&files, "duplicate-input-verified-at-ms.txt")?;
    if !(hop_one_verified_at <= hop_two_verified_at && hop_two_verified_at <= duplicate_verified_at)
    {
        return Err("recipient verification times are not in lifecycle order".to_owned());
    }
    let hop_one_request: KagemushaRecipientPaymentRequestV2 = decode(
        bytes(&files, "append-hop-01-recipient-request-v2.norito")?,
        "hop-one recipient request",
    )?;
    let hop_one_recipient_opening: KagemushaNoteOpeningV2 = decode(
        bytes(&files, "append-hop-01-recipient-opening-v2.norito")?,
        "hop-one recipient opening",
    )?;
    let hop_one_recipient = validate_request_opening(
        &hop_one_request,
        &hop_one_recipient_opening,
        hop_one_verified_at,
        &candidate,
    )?;
    let hop_one_change_units = anchor
        .amount
        .atomic_units
        .checked_sub(hop_one_request.amount.atomic_units)
        .filter(|amount| *amount != 0)
        .ok_or_else(|| "hop one must leave positive sender change".to_owned())?;
    let hop_one_change_amount =
        KagemushaScaledAmountV2::new(hop_one_change_units, candidate.manifest.asset_scale)
            .map_err(|_| "hop-one change amount is invalid".to_owned())?;
    let hop_one_change_opening: KagemushaNoteOpeningV2 = decode(
        bytes(&files, "append-hop-01-change-opening-v2.norito")?,
        "hop-one change opening",
    )?;
    let hop_one_change = derive_kagemusha_owned_note_v2(
        &candidate.manifest.network_id,
        &candidate.manifest.asset,
        hop_one_change_amount,
        &hop_one_change_opening,
    )
    .map_err(|_| "failed to derive hop-one change note".to_owned())?;
    let hop_one_membership: KagemushaOutputMembershipPathsV4 = decode(
        bytes(&files, "append-hop-01-output-membership-v4.norito")?,
        "hop-one output membership",
    )?;
    if hop_one_membership.initial_root != init_membership.final_root {
        return Err("hop-one membership does not continue the finalized init root".to_owned());
    }
    hop_one_membership
        .to_core_witness(
            iroha_core::zk::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
            Some(hop_one_recipient.note_commitment),
            Some(hop_one_change.note_commitment),
        )
        .map_err(|_| {
            "hop-one output paths do not cryptographically bind both outputs".to_owned()
        })?;
    let hop_two_request: KagemushaRecipientPaymentRequestV2 = decode(
        bytes(&files, "append-hop-02-recipient-request-v2.norito")?,
        "hop-two recipient request",
    )?;
    let hop_two_recipient_opening: KagemushaNoteOpeningV2 = decode(
        bytes(&files, "append-hop-02-recipient-opening-v2.norito")?,
        "hop-two recipient opening",
    )?;
    let hop_two_recipient = validate_request_opening(
        &hop_two_request,
        &hop_two_recipient_opening,
        hop_two_verified_at,
        &candidate,
    )?;
    let hop_two_change_units = hop_one_change_units
        .checked_sub(hop_two_request.amount.atomic_units)
        .filter(|amount| *amount != 0)
        .ok_or_else(|| "hop two must leave positive sender change".to_owned())?;
    let hop_two_change_amount =
        KagemushaScaledAmountV2::new(hop_two_change_units, candidate.manifest.asset_scale)
            .map_err(|_| "hop-two change amount is invalid".to_owned())?;
    let hop_two_change_opening: KagemushaNoteOpeningV2 = decode(
        bytes(&files, "append-hop-02-change-opening-v2.norito")?,
        "hop-two change opening",
    )?;
    let hop_two_change = derive_kagemusha_owned_note_v2(
        &candidate.manifest.network_id,
        &candidate.manifest.asset,
        hop_two_change_amount,
        &hop_two_change_opening,
    )
    .map_err(|_| "failed to derive hop-two change note".to_owned())?;
    let hop_two_membership: KagemushaOutputMembershipPathsV4 = decode(
        bytes(&files, "append-hop-02-output-membership-v4.norito")?,
        "hop-two output membership",
    )?;
    if hop_two_membership.initial_root != hop_one_membership.final_root {
        return Err("hop-two membership does not continue the hop-one final root".to_owned());
    }
    hop_two_membership
        .to_core_witness(
            iroha_core::zk::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
            Some(hop_two_recipient.note_commitment),
            Some(hop_two_change.note_commitment),
        )
        .map_err(|_| {
            "hop-two output paths do not cryptographically bind both outputs".to_owned()
        })?;
    let duplicate_request: KagemushaRecipientPaymentRequestV2 = decode(
        bytes(&files, "duplicate-input-recipient-request-v2.norito")?,
        "duplicate-input recipient request",
    )?;
    duplicate_request
        .validate_at(duplicate_verified_at)
        .map_err(|_| "duplicate-input recipient request is invalid or expired".to_owned())?;
    validate_request_material_without_opening(&duplicate_request)?;
    if duplicate_request.network_id != candidate.manifest.network_id
        || duplicate_request.asset != candidate.manifest.asset
        || duplicate_request.amount != hop_one_request.amount
    {
        return Err(
            "duplicate-input request must consume the complete hop-one recipient value".to_owned(),
        );
    }
    let duplicate_membership: KagemushaOutputMembershipPathsV4 = decode(
        bytes(&files, "duplicate-input-output-membership-v4.norito")?,
        "duplicate-input output membership",
    )?;
    if duplicate_membership.initial_root != hop_one_membership.final_root {
        return Err(
            "duplicate-input membership is not rooted at the observed hop-one state".to_owned(),
        );
    }
    duplicate_membership
        .to_core_witness(
            iroha_core::zk::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
            Some(duplicate_request.recipient_output.note_commitment),
            None,
        )
        .map_err(|_| "duplicate-input output paths do not bind its recipient output".to_owned())?;
    let request_ids = [
        hop_one_request.request_id,
        hop_two_request.request_id,
        duplicate_request.request_id,
    ];
    if request_ids.into_iter().collect::<HashSet<_>>().len() != request_ids.len() {
        return Err("recipient request ids must be distinct".to_owned());
    }
    let openings = [
        &init_opening,
        &hop_one_recipient_opening,
        &hop_one_change_opening,
        &hop_two_recipient_opening,
        &hop_two_change_opening,
    ];
    for index in 0..openings.len() {
        if openings[index + 1..]
            .iter()
            .any(|opening| *opening == openings[index])
        {
            return Err("all five candidate-owned note openings must be distinct".to_owned());
        }
    }
    let notes = [
        &init_note,
        &hop_one_recipient,
        &hop_one_change,
        &hop_two_recipient,
        &hop_two_change,
        &duplicate_request.recipient_output,
    ];
    let commitments = notes
        .iter()
        .map(|note| note.note_commitment)
        .collect::<HashSet<_>>();
    let nullifiers = notes
        .iter()
        .map(|note| note.spend_nullifier)
        .collect::<HashSet<_>>();
    if commitments.len() != notes.len() || nullifiers.len() != notes.len() {
        return Err("scenario note commitments/nullifiers must all be distinct".to_owned());
    }
    let expected_transfer = iroha_core::zk::hash_vk(
        &iroha_core::zk::confidential_v2::confidential_transfer_v2_vk_box()
            .map_err(|error| format!("failed to construct canonical transfer verifier: {error}"))?,
    );
    if digest32(&files, "transfer-verifier-commitment-v2.bin")? != expected_transfer {
        return Err(
            "transfer verifier commitment is not the canonical current-source key".to_owned(),
        );
    }
    let expected_unshield = iroha_core::zk::hash_vk(
        &iroha_core::zk::confidential_v2::confidential_unshield_v2_vk_box()
            .map_err(|error| format!("failed to construct canonical unshield verifier: {error}"))?,
    );
    if digest32(&files, "unshield-verifier-commitment-v2.bin")? != expected_unshield {
        return Err(
            "unshield verifier commitment is not the canonical current-source key".to_owned(),
        );
    }
    let operation_names = [
        "append-hop-01-operation-id.bin",
        "append-hop-02-operation-id.bin",
        "duplicate-input-operation-id.bin",
        "redeem-hop-01-operation-id.bin",
        "redeem-hop-02-operation-id.bin",
        "redeem-sender-change-operation-id.bin",
    ];
    let mut operation_ids = HashSet::from([anchor.topup_operation_id]);
    for name in operation_names {
        let operation_id = digest32(&files, name)?;
        if operation_id == [0; 32] || !operation_ids.insert(operation_id) {
            return Err(format!("{name} is zero or reuses another operation id"));
        }
    }
    let hop_one_height = positive_decimal(&files, "append-hop-01-block-height.txt")?;
    let hop_two_height = positive_decimal(&files, "append-hop-02-block-height.txt")?;
    let duplicate_height = positive_decimal(&files, "duplicate-input-block-height.txt")?;
    let redeem_one_height = positive_decimal(&files, "redeem-hop-01-block-height.txt")?;
    let redeem_two_height = positive_decimal(&files, "redeem-hop-02-block-height.txt")?;
    let redeem_change_height = positive_decimal(&files, "redeem-sender-change-block-height.txt")?;
    let timeline = [
        anchor.finalized_height,
        hop_one_height,
        hop_two_height,
        duplicate_height,
        redeem_one_height,
        redeem_two_height,
        redeem_change_height,
    ];
    if timeline.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err("scenario block heights are not in lifecycle order".to_owned());
    }
    for height in [hop_one_height, hop_two_height, duplicate_height] {
        if height < candidate.manifest.activation_height
            || height >= candidate.manifest.withdrawal_height
        {
            return Err("append height is outside the candidate issuance window".to_owned());
        }
    }
    for height in [redeem_one_height, redeem_two_height, redeem_change_height] {
        if height < candidate.manifest.activation_height {
            return Err("redemption height precedes candidate activation".to_owned());
        }
    }
    let account_payload = bytes(&files, "redeem-recipient-account-id.txt")?;
    let account_line = account_payload
        .strip_suffix(b"\n")
        .ok_or_else(|| "redemption account must be one newline-terminated line".to_owned())?;
    let account_text = std::str::from_utf8(account_line)
        .map_err(|_| "redemption account is not UTF-8".to_owned())?;
    let _account = parse_canonical_account_id_for_chain(account_text, account_chain_discriminant)?;
    let inventory_sha256 = scenario_inventory_sha256(&files)?;
    Ok(format!(
        "{{\"candidate_manifest_sha256\":\"{}\",\"candidate_record_sha256\":\"{}\",\"finalized_height\":{},\"scenario_file_count\":{},\"scenario_inventory_sha256\":\"{}\",\"schema\":\"{}\"}}\n",
        hex::encode(manifest_sha256),
        hex::encode(candidate_sha256),
        anchor.finalized_height,
        files.len(),
        hex::encode(inventory_sha256),
        REPORT_SCHEMA,
    )
    .into_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    #[test]
    fn arbitrary_ascii_is_not_a_typed_norito_seed() {
        assert!(decode::<KagemushaNoteOpeningV2>(b"not norito\n", "opening").is_err());
        assert!(decode::<KagemushaRecipientPaymentRequestV2>(b"{}\n", "request").is_err());
    }
    #[test]
    fn account_canonical_roundtrip_uses_the_explicit_chain_discriminant() {
        const TAIRA: u16 = 369;
        const SORA: u16 = 753;
        let account = AccountId::new(
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
                .expect("derive account fixture")
                .public_key()
                .clone(),
        );
        let taira_literal = {
            let _taira = ChainDiscriminantGuard::enter(TAIRA);
            account.to_string()
        };
        let _ambient_sora = ChainDiscriminantGuard::enter(SORA);
        assert_eq!(
            parse_canonical_account_id_for_chain(&taira_literal, TAIRA)
                .expect("Taira literal under explicit Taira scope"),
            account
        );
        assert!(
            parse_canonical_account_id_for_chain(&taira_literal, SORA).is_err(),
            "Taira input must not roundtrip as a SORA account"
        );
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            SORA,
            "validation must restore the caller's ambient scope"
        );
    }
}
