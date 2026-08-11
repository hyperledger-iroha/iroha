//! Deterministic non-shipping fixture for native zk-X.509 release evidence.
//!
//! The fixture lives beside the reference relation so its DER construction
//! uses the exact closed grammar rather than a platform certificate builder.
//! It is compiled only for tests and the isolated privacy-release runner.

use iroha_crypto::{Algorithm, KeyPair};
#[cfg(test)]
use iroha_crypto::{Hash, HashOf};
#[cfg(test)]
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    privacy::{
        PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    },
};
use iroha_data_model::{
    account::AccountId,
    privacy::{
        PrivacyChallengeV1, PrivacyConsensusLimitsV1, PrivacyIssuerIdV1, PrivacyPolicyDigestV1,
        PrivacyPolicyIdV1, PrivacyRootPublicationV1, PrivacyRootRoleV1, PrivacyRootV1,
        PrivacyStatementContextV1, PrivacyX509KeyUsageRequirementV1, PrivacyX509TrustStoreDigestV1,
        PrivacyZkX509DisclosedAttributeV1, ZK_X509_MAX_CERTIFICATE_BYTES_V1,
    },
};
use mv::storage::Storage;
use p256::ecdsa::{
    Signature as P256Signature, SigningKey as P256SigningKey,
    signature::{Signer as _, hazmat::PrehashSigner as _},
};
use time::OffsetDateTime;

use super::*;
use crate::{
    privacy_engines::zk_x509::{
        codec::ZkX509AttributeOpeningV1,
        merkle::{
            ZK_X509_CA_COMPACT_TREE_CAPACITY_V1, ZkX509CaMembershipPathV1,
            ca_membership_path_from_complete_spkis_v1, ca_root_from_complete_spkis_v1,
        },
        profile::{
            ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1, ZK_X509_MAX_CHAIN_DEPTH_V1,
            ZK_X509_MAX_CRL_AGE_SECONDS_V1, ZK_X509_MAX_CRL_BYTES_V1, ZK_X509_MAX_SERIAL_BYTES_V1,
            ZK_X509_MIN_CHAIN_DEPTH_V1,
        },
    },
    privacy_state::{
        PrivacyCommitmentKeyV1, PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1, PrivacyRootKeyV1,
        PrivacyRootProvenanceV1, PrivacyStateItemRecordV1, PrivacyZkX509AuthoritativeStateV1,
        load_privacy_zk_x509_authoritative_state_v1, privacy_zk_x509_ca_namespace_v1,
    },
};

const CRL_THIS_UPDATE: u64 = 1_672_531_200; // 2023-01-01T00:00:00Z
const CRL_NEXT_UPDATE: u64 = CRL_THIS_UPDATE + 300;
const VALIDATION_TIME: u64 = CRL_THIS_UPDATE + 60;
const CANONICAL_LEAF_SERIAL_V1: [u8; 1] = [2];
const MAXIMUM_LEAF_SERIAL_V1: [u8; ZK_X509_MAX_SERIAL_BYTES_V1] =
    [0x40; ZK_X509_MAX_SERIAL_BYTES_V1];

#[derive(Clone, Copy)]
struct ZkX509ReleaseTimesV1 {
    crl_this_update_unix_seconds: u64,
    crl_next_update_unix_seconds: u64,
    presentation_not_before_unix_seconds: u64,
    presentation_not_after_unix_seconds: u64,
    revoked_at_unix_seconds: u64,
}

impl ZkX509ReleaseTimesV1 {
    const FIXED_V1: Self = Self {
        crl_this_update_unix_seconds: CRL_THIS_UPDATE,
        crl_next_update_unix_seconds: CRL_NEXT_UPDATE,
        presentation_not_before_unix_seconds: VALIDATION_TIME,
        presentation_not_after_unix_seconds: VALIDATION_TIME + 60,
        revoked_at_unix_seconds: CRL_THIS_UPDATE - 86_400,
    };

    fn from_trusted_block_timestamp_ms_v1(
        trusted_block_timestamp_ms: u64,
    ) -> Result<Self, &'static str> {
        let trusted_unix_seconds = trusted_block_timestamp_ms / 1_000;
        let presentation_not_after_unix_seconds = trusted_unix_seconds
            .checked_add(ZK_X509_MAX_CRL_AGE_SECONDS_V1)
            .ok_or("network release presentation window overflow")?;
        let crl_next_update_unix_seconds = presentation_not_after_unix_seconds
            .checked_add(1)
            .ok_or("network release CRL nextUpdate overflow")?;
        let revoked_at_unix_seconds = trusted_unix_seconds
            .checked_sub(1)
            .ok_or("network release CRL revocation time underflow")?;
        Ok(Self {
            crl_this_update_unix_seconds: trusted_unix_seconds,
            crl_next_update_unix_seconds,
            presentation_not_before_unix_seconds: trusted_unix_seconds,
            presentation_not_after_unix_seconds,
            revoked_at_unix_seconds,
        })
    }
}

#[derive(Clone)]
enum ZkX509ReleaseCrlLineageV1 {
    Origin,
    Successor(PrivacyZkX509CrlRecordV1),
}

/// Non-secret dimensions of one deterministic release fixture.
///
/// These values are deliberately copyable so the isolated release runner can
/// append exact input-shape evidence without retaining or serializing private
/// certificate, CRL, ownership-signature, or disclosure-opening bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509ReleaseResourceShapeV1 {
    /// Number of occupied certificate-length slots.
    pub(crate) certificate_chain_depth: u8,
    /// Exact DER length of each leaf-first certificate; unused slots are zero.
    pub(crate) certificate_der_lengths: [u32; ZK_X509_MAX_CHAIN_DEPTH_V1],
    /// Exact DER length of the complete signed CRL.
    pub(crate) crl_der_length: u32,
    /// Largest canonical positive serial magnitude in the chain or CRL.
    pub(crate) maximum_serial_bytes: u8,
    /// Subject-value lengths indexed as `C`, `O`, `OU`, and `CN`.
    pub(crate) disclosed_value_lengths: [u16; 4],
    /// Largest disclosed subject-value length.
    pub(crate) maximum_disclosed_value_bytes: u16,
    /// Canonical sorted-leaf index of the governed root CA.
    pub(crate) ca_membership_index: u16,
    /// Whether at least one compact-tree sibling is nonzero.
    pub(crate) ca_membership_path_has_nonzero_sibling: bool,
}

impl ZkX509ReleaseResourceShapeV1 {
    /// Validate every recorded dimension against the closed first-release
    /// profile.
    pub(crate) fn validate_v1(&self) -> Result<(), &'static str> {
        let chain_depth = usize::from(self.certificate_chain_depth);
        if !(ZK_X509_MIN_CHAIN_DEPTH_V1..=ZK_X509_MAX_CHAIN_DEPTH_V1).contains(&chain_depth)
            || self.certificate_der_lengths[..chain_depth]
                .iter()
                .any(|length| *length == 0 || *length > ZK_X509_MAX_CERTIFICATE_BYTES_V1)
            || self.certificate_der_lengths[chain_depth..]
                .iter()
                .any(|length| *length != 0)
        {
            return Err("deterministic release certificate shape is invalid");
        }
        if self.crl_der_length == 0
            || usize::try_from(self.crl_der_length)
                .map_err(|_| "deterministic release CRL length overflow")?
                > ZK_X509_MAX_CRL_BYTES_V1
        {
            return Err("deterministic release CRL shape is invalid");
        }
        if self.maximum_serial_bytes == 0
            || usize::from(self.maximum_serial_bytes) > ZK_X509_MAX_SERIAL_BYTES_V1
        {
            return Err("deterministic release serial shape is invalid");
        }
        let maximum_disclosed_value_bytes = self
            .disclosed_value_lengths
            .iter()
            .copied()
            .max()
            .ok_or("deterministic release disclosure shape is empty")?;
        if maximum_disclosed_value_bytes == 0
            || maximum_disclosed_value_bytes != self.maximum_disclosed_value_bytes
            || usize::from(maximum_disclosed_value_bytes) > ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
            || !matches!(self.disclosed_value_lengths[0], 0 | 2)
        {
            return Err("deterministic release disclosure shape is invalid");
        }
        if usize::from(self.ca_membership_index) >= ZK_X509_CA_COMPACT_TREE_CAPACITY_V1
            || !self.ca_membership_path_has_nonzero_sibling
        {
            return Err("deterministic release CA membership shape is invalid");
        }
        Ok(())
    }
}

/// Fully joined deterministic relation and ledger-state fixture.
pub(crate) struct ZkX509ReleaseFixtureV1 {
    pub(crate) statement: IrohaZkX509StarkP256StatementV1,
    pub(crate) witness: ZkX509WitnessV1,
    pub(crate) authoritative_state: PrivacyZkX509AuthoritativeStateV1,
    pub(crate) crl_entry_count: usize,
    pub(crate) resource_shape: ZkX509ReleaseResourceShapeV1,
}

/// Reference-test context retained independently of the compiled release
/// manifest. Release evidence supplies the actual compiled profile context.
#[cfg(test)]
pub(crate) fn reference_statement_context_v1() -> PrivacyStatementContextV1 {
    PrivacyStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x91; 32]),
        )),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x62; 32]),
        parameter_id: PrivacyParameterIdV1::new([0x63; 32]),
        parameter_digest: PrivacyParameterDigestV1::new([0x64; 32]),
        verifier_digest: PrivacyVerifierDigestV1::new([0x65; 32]),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0x66; 32]),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0x67; 32]),
    }
}

/// Build the canonical two-certificate, one-disclosure fixture.
#[cfg(test)]
pub(crate) fn build_zk_x509_reference_fixture_v1() -> Result<ZkX509ReleaseFixtureV1, &'static str> {
    build_zk_x509_fixture_v1(
        reference_statement_context_v1(),
        false,
        &[],
        ZkX509ReleaseTimesV1::FIXED_V1,
        fixed_release_wallet_account_v1()?,
        &ZkX509ReleaseCrlLineageV1::Origin,
    )
}

/// Build either the canonical or closed maximum-shape release fixture.
///
/// The maximum shape has three certificates, all four disclosure slots, and
/// the exact 64-entry complete CRL ceiling. The leaf serial is deliberately
/// absent from that CRL.
pub(crate) fn build_zk_x509_release_fixture_v1(
    context: PrivacyStatementContextV1,
    maximum_shape: bool,
) -> Result<ZkX509ReleaseFixtureV1, &'static str> {
    let revoked_serials = if maximum_shape {
        (0..ZK_X509_MAX_CRL_ENTRIES_V1)
            .map(maximum_crl_serial_v1)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    build_zk_x509_fixture_v1(
        context,
        maximum_shape,
        &revoked_serials,
        ZkX509ReleaseTimesV1::FIXED_V1,
        fixed_release_wallet_account_v1()?,
        &ZkX509ReleaseCrlLineageV1::Origin,
    )
}

/// Build the canonical network fixture around one actual trusted block time.
///
/// This keeps the fixed release KAT byte-for-byte stable while giving the
/// four-peer release gate a signed CRL and presentation window that can be
/// admitted by live consensus. The inclusive presentation window consumes the
/// complete five-minute freshness allowance; `nextUpdate` remains exclusive.
pub(crate) fn build_zk_x509_network_release_fixture_v1(
    context: PrivacyStatementContextV1,
    trusted_block_timestamp_ms: u64,
    wallet_account: AccountId,
) -> Result<ZkX509ReleaseFixtureV1, &'static str> {
    build_zk_x509_fixture_v1(
        context,
        false,
        &[],
        ZkX509ReleaseTimesV1::from_trusted_block_timestamp_ms_v1(trusted_block_timestamp_ms)?,
        wallet_account,
        &ZkX509ReleaseCrlLineageV1::Origin,
    )
}

/// Build a fresh network fixture whose complete signed CRL is the exact
/// active successor of `current_crl`.
pub(crate) fn build_zk_x509_network_release_successor_fixture_v1(
    context: PrivacyStatementContextV1,
    trusted_block_timestamp_ms: u64,
    wallet_account: AccountId,
    current_crl: PrivacyZkX509CrlRecordV1,
) -> Result<ZkX509ReleaseFixtureV1, &'static str> {
    current_crl
        .validate()
        .map_err(|_| "network release current CRL record failed validation")?;
    let times =
        ZkX509ReleaseTimesV1::from_trusted_block_timestamp_ms_v1(trusted_block_timestamp_ms)?;
    if times.crl_this_update_unix_seconds <= current_crl.this_update_unix_seconds {
        return Err("network release successor CRL thisUpdate did not increase");
    }
    let crl_lineage = ZkX509ReleaseCrlLineageV1::Successor(current_crl.clone());
    let fixture =
        build_zk_x509_fixture_v1(context, false, &[], times, wallet_account, &crl_lineage)?;
    let successor = fixture.authoritative_state.crl_record();
    iroha_data_model::privacy::validate_zk_x509_crl_rotation_v1(&current_crl, &successor)
        .map_err(|_| "network release successor CRL failed rotation validation")?;
    Ok(fixture)
}

fn fixed_release_wallet_account_v1() -> Result<AccountId, &'static str> {
    let wallet_key_pair = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519)
        .map_err(|_| "deterministic release wallet key failed")?;
    Ok(AccountId::new(wallet_key_pair.public_key().clone()))
}

fn build_zk_x509_fixture_v1(
    context: PrivacyStatementContextV1,
    maximum_shape: bool,
    revoked_serials: &[Vec<u8>],
    times: ZkX509ReleaseTimesV1,
    wallet_account: AccountId,
    crl_lineage: &ZkX509ReleaseCrlLineageV1,
) -> Result<ZkX509ReleaseFixtureV1, &'static str> {
    let root_key = p256_key(1)?;
    let intermediate_key = p256_key(3)?;
    let leaf_key = p256_key(2)?;
    let root_name = name(&[(OID_COMMON_NAME, 0x0c, b"Iroha Test Root")]);
    let intermediate_name = name(&[(OID_COMMON_NAME, 0x0c, b"Iroha Test Intermediate")]);
    let maximum_organization = vec![b'O'; ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1];
    let maximum_organizational_unit = vec![b'U'; ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1];
    let maximum_common_name = vec![b'N'; ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1];
    let leaf_name = if maximum_shape {
        name(&[
            (OID_COUNTRY_NAME, 0x13, b"IL"),
            (OID_ORGANIZATION_NAME, 0x0c, maximum_organization.as_slice()),
            (
                OID_ORGANIZATIONAL_UNIT_NAME,
                0x0c,
                maximum_organizational_unit.as_slice(),
            ),
            (OID_COMMON_NAME, 0x0c, maximum_common_name.as_slice()),
        ])
    } else {
        name(&[
            (OID_COUNTRY_NAME, 0x13, b"IL"),
            (OID_ORGANIZATION_NAME, 0x0c, b"Iroha"),
            (OID_COMMON_NAME, 0x0c, b"Alice"),
        ])
    };
    let root_ski = [0x31; 20];
    let intermediate_ski = [0x32; 20];
    let leaf_ski = [0x42; 20];
    let root_path_len = u32::from(maximum_shape);
    let root_der = certificate(
        &[1],
        &root_name,
        &root_name,
        &root_key,
        &root_key,
        &root_ski,
        &root_ski,
        Some(root_path_len),
    );
    let intermediate_der = maximum_shape.then(|| {
        certificate(
            &[3],
            &root_name,
            &intermediate_name,
            &intermediate_key,
            &root_key,
            &intermediate_ski,
            &root_ski,
            Some(0),
        )
    });
    let (leaf_issuer_name, leaf_issuer_key, leaf_issuer_ski) = if maximum_shape {
        (
            intermediate_name.as_slice(),
            &intermediate_key,
            intermediate_ski.as_slice(),
        )
    } else {
        (root_name.as_slice(), &root_key, root_ski.as_slice())
    };
    let leaf_serial = maximum_shape
        .then_some(MAXIMUM_LEAF_SERIAL_V1.as_slice())
        .unwrap_or(CANONICAL_LEAF_SERIAL_V1.as_slice());
    let leaf_der = certificate(
        leaf_serial,
        leaf_issuer_name,
        &leaf_name,
        &leaf_key,
        leaf_issuer_key,
        &leaf_ski,
        leaf_issuer_ski,
        None,
    );
    let (crl_record_epoch, crl_number, previous_crl_record_digest) = match crl_lineage {
        ZkX509ReleaseCrlLineageV1::Origin => (1, 7, None),
        ZkX509ReleaseCrlLineageV1::Successor(current) => (
            current
                .record_epoch
                .checked_add(1)
                .ok_or("network release successor CRL epoch overflow")?,
            current
                .crl_number
                .checked_add(1)
                .ok_or("network release successor CRL number overflow")?,
            Some(current.record_digest),
        ),
    };
    let crl_der = crl(
        leaf_issuer_name,
        leaf_issuer_key,
        leaf_issuer_ski,
        crl_number,
        revoked_serials,
        times,
    )?;

    let mut certificate_chain_der = vec![leaf_der];
    if let Some(intermediate_der) = intermediate_der {
        certificate_chain_der.push(intermediate_der);
    }
    certificate_chain_der.push(root_der);
    let parsed_chain = certificate_chain_der
        .iter()
        .map(|certificate| parse_certificate_v1(certificate))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "deterministic release certificate failed strict parsing")?;
    let parsed_crl =
        parse_crl_v1(&crl_der).map_err(|_| "deterministic release CRL failed strict parsing")?;
    if parsed_crl.revoked_serials.len() != revoked_serials.len() {
        return Err("deterministic release CRL lost an entry");
    }

    let root = parsed_chain
        .last()
        .ok_or("deterministic release chain is empty")?;
    let extra_governed_keys = maximum_shape
        .then(|| Ok::<_, &'static str>([p256_key(5)?, p256_key(7)?]))
        .transpose()?;
    let extra_governed_spkis = extra_governed_keys.map(|keys| [spki(&keys[0]), spki(&keys[1])]);
    let governed_spkis = extra_governed_spkis.as_ref().map_or_else(
        || vec![root.spki_der],
        |spkis| vec![spkis[0].as_slice(), root.spki_der, spkis[1].as_slice()],
    );
    let ca_membership_path =
        ca_membership_path_from_complete_spkis_v1(&governed_spkis, root.spki_der)
            .map_err(|_| "deterministic release CA path failed")?;
    let ca_root = ca_root_from_complete_spkis_v1(&governed_spkis)
        .map_err(|_| "deterministic release CA root failed")?;
    let trust_anchor_id = PrivacyIssuerIdV1::new([0x51; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x52; 32]);
    let trust_anchor = PrivacyZkX509TrustAnchorRecordV1::new(
        trust_anchor_id,
        1,
        PrivacyX509TrustStoreDigestV1::new([0x53; 32]),
        PrivacyRootV1::new(ca_root),
        1,
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .map_err(|_| "deterministic release trust anchor failed")?;
    let key_usage = PrivacyX509KeyUsageV1 {
        digital_signature: PrivacyX509KeyUsageRequirementV1::new(true),
        content_commitment: PrivacyX509KeyUsageRequirementV1::new(false),
        key_encipherment: PrivacyX509KeyUsageRequirementV1::new(false),
        key_agreement: PrivacyX509KeyUsageRequirementV1::new(false),
    };
    let extended_key_usages = vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication];
    let disclosed_indices = if maximum_shape {
        vec![0, 1, 2, 3]
    } else {
        vec![3]
    };
    let policy = PrivacyZkX509CertificatePolicyRecordV1::new(
        trust_anchor_id,
        policy_id,
        1,
        PrivacyPolicyDigestV1::new([0x54; 32]),
        key_usage,
        extended_key_usages.clone(),
        disclosed_indices.clone(),
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .map_err(|_| "deterministic release certificate policy failed")?;
    let issuer = parsed_chain
        .get(1)
        .ok_or("deterministic release chain has no issuer")?;
    let crl = PrivacyZkX509CrlRecordV1::new(
        trust_anchor_id,
        policy_id,
        crl_record_epoch,
        crl_number,
        PrivacyX509CrlDerDigestV1::digest_exact_der(&crl_der),
        PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(issuer.spki_der),
        times.crl_this_update_unix_seconds,
        times.crl_next_update_unix_seconds,
        previous_crl_record_digest,
        PrivacyZkX509RecordLifecycleV1::Active,
    )
    .map_err(|_| "deterministic release CRL record failed")?;

    let mut statement = IrohaZkX509StarkP256StatementV1 {
        context,
        trust_anchor_id,
        certificate_policy_id: policy_id,
        trust_anchor_record_digest: trust_anchor.record_digest,
        trust_anchor_record_epoch: trust_anchor.record_epoch,
        certificate_policy_record_digest: policy.record_digest,
        certificate_policy_record_epoch: policy.record_epoch,
        crl_record_digest: crl.record_digest,
        crl_record_epoch: crl.record_epoch,
        subject_public_key_digest: PrivacyCertificateKeyDigestV1::new([0; 32]),
        ca_membership_root: PrivacyRootV1::new(ca_root),
        ca_membership_root_epoch: 1,
        key_usage,
        extended_key_usages,
        disclosed_attributes: disclosed_indices
            .iter()
            .copied()
            .map(|index| PrivacyZkX509DisclosedAttributeV1 {
                index,
                attribute_digest: PrivacyAttributeDigestV1::new([0; 32]),
            })
            .collect(),
        presentation_not_before_unix_seconds: times.presentation_not_before_unix_seconds,
        presentation_not_after_unix_seconds: times.presentation_not_after_unix_seconds,
        wallet_account,
        wallet_challenge: PrivacyChallengeV1::new([0x68; 32]),
        certificate_nullifier: PrivacyNullifierV1::new([0; 32]),
    };
    statement.subject_public_key_digest =
        derive_subject_public_key_digest_v1(&statement, &parsed_chain)
            .map_err(|_| "deterministic release subject-key digest failed")?;
    statement.certificate_nullifier =
        derive_certificate_nullifier_v1(&statement, issuer.spki_der, parsed_chain[0].serial)
            .map_err(|_| "deterministic release nullifier failed")?;
    let mut attribute_openings = Vec::with_capacity(disclosed_indices.len());
    for (position, index) in disclosed_indices.iter().copied().enumerate() {
        let salt_byte = 0x71_u8
            .checked_add(u8::try_from(position).map_err(|_| "release disclosure overflow")?)
            .ok_or("release disclosure salt overflow")?;
        let opening = ZkX509AttributeOpeningV1 {
            index,
            salt: [salt_byte; 32],
        };
        let value = parsed_chain[0]
            .subject
            .attributes
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or("release disclosure has no subject attribute")?;
        statement.disclosed_attributes[position].attribute_digest = PrivacyAttributeDigestV1::new(
            hash_frame_v1(
                ZK_X509_ATTRIBUTE_DOMAIN_V1,
                &[
                    ZK_X509_SUITE_V1,
                    statement.trust_anchor_id.as_bytes(),
                    statement.certificate_policy_id.as_bytes(),
                    &[index],
                    value,
                    &opening.salt,
                ],
            )
            .map_err(|_| "deterministic release attribute digest failed")?,
        );
        attribute_openings.push(opening);
    }
    let ownership_challenge = derive_ownership_challenge_digest_v1(&statement)
        .map_err(|_| "deterministic release ownership challenge failed")?;
    let ownership_signature: P256Signature = leaf_key
        .sign_prehash(&ownership_challenge)
        .map_err(|_| "deterministic release ownership signature failed")?;
    let ownership_signature = ownership_signature
        .normalize_s()
        .unwrap_or(ownership_signature);
    if ownership_signature.normalize_s().is_some() {
        return Err("deterministic release ownership signature is not low-S");
    }
    let resource_shape = release_resource_shape_v1(
        &parsed_chain,
        &parsed_crl,
        &disclosed_indices,
        &certificate_chain_der,
        &crl_der,
        &ca_membership_path,
    )?;
    resource_shape.validate_v1()?;
    if maximum_shape
        && (usize::from(resource_shape.certificate_chain_depth) != ZK_X509_MAX_CHAIN_DEPTH_V1
            || revoked_serials.len() != ZK_X509_MAX_CRL_ENTRIES_V1
            || disclosed_indices.len()
                != iroha_data_model::privacy::ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1
            || resource_shape.maximum_serial_bytes as usize != ZK_X509_MAX_SERIAL_BYTES_V1
            || resource_shape.disclosed_value_lengths
                != [
                    2,
                    ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
                    ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
                    ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
                ]
            || resource_shape.maximum_disclosed_value_bytes as usize
                != ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
            || resource_shape.ca_membership_index != 1
            || !resource_shape.ca_membership_path_has_nonzero_sibling)
    {
        return Err("deterministic maximum release shape missed a closed-profile ceiling");
    }

    let witness = ZkX509WitnessV1 {
        certificate_chain_der,
        crl_der,
        ca_membership_path,
        wallet_ownership_signature_rs: ownership_signature.to_bytes().into(),
        attribute_openings,
    };
    let authoritative_state = authoritative_state_v1(trust_anchor, policy.clone(), crl)?;
    validate_reference_relation_v1(
        &statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: &policy,
            crl: &crl,
        },
        &witness,
    )
    .map_err(|_| "deterministic release relation failed")?;

    Ok(ZkX509ReleaseFixtureV1 {
        statement,
        witness,
        authoritative_state,
        crl_entry_count: revoked_serials.len(),
        resource_shape,
    })
}

fn maximum_crl_serial_v1(index: usize) -> Result<Vec<u8>, &'static str> {
    let suffix = u8::try_from(index)
        .ok()
        .and_then(|index| index.checked_add(1))
        .ok_or("deterministic maximum CRL serial overflow")?;
    let mut serial = vec![0x20; ZK_X509_MAX_SERIAL_BYTES_V1];
    *serial
        .last_mut()
        .ok_or("deterministic maximum CRL serial is empty")? = suffix;
    Ok(serial)
}

fn release_resource_shape_v1(
    parsed_chain: &[ParsedCertificateV1<'_>],
    parsed_crl: &ParsedCrlV1<'_>,
    disclosed_indices: &[u8],
    certificate_chain_der: &[Vec<u8>],
    crl_der: &[u8],
    ca_membership_path: &ZkX509CaMembershipPathV1,
) -> Result<ZkX509ReleaseResourceShapeV1, &'static str> {
    let mut certificate_der_lengths = [0_u32; ZK_X509_MAX_CHAIN_DEPTH_V1];
    for (target, certificate) in certificate_der_lengths
        .iter_mut()
        .zip(certificate_chain_der)
    {
        *target = u32::try_from(certificate.len())
            .map_err(|_| "deterministic release certificate length overflow")?;
    }
    let maximum_serial_bytes = parsed_chain
        .iter()
        .map(|certificate| certificate.serial.len())
        .chain(parsed_crl.revoked_serials.iter().map(|serial| serial.len()))
        .max()
        .ok_or("deterministic release serial shape is empty")?;
    let mut disclosed_value_lengths = [0_u16; 4];
    let leaf = parsed_chain
        .first()
        .ok_or("deterministic release chain is empty")?;
    for index in disclosed_indices {
        let value = leaf
            .subject
            .attributes
            .get(usize::from(*index))
            .copied()
            .flatten()
            .ok_or("deterministic release disclosure has no subject attribute")?;
        disclosed_value_lengths[usize::from(*index)] = u16::try_from(value.len())
            .map_err(|_| "deterministic release disclosure length overflow")?;
    }
    let maximum_disclosed_value_bytes = disclosed_value_lengths
        .iter()
        .copied()
        .max()
        .ok_or("deterministic release disclosure shape is empty")?;
    Ok(ZkX509ReleaseResourceShapeV1 {
        certificate_chain_depth: u8::try_from(certificate_chain_der.len())
            .map_err(|_| "deterministic release chain depth overflow")?,
        certificate_der_lengths,
        crl_der_length: u32::try_from(crl_der.len())
            .map_err(|_| "deterministic release CRL length overflow")?,
        maximum_serial_bytes: u8::try_from(maximum_serial_bytes)
            .map_err(|_| "deterministic release serial length overflow")?,
        disclosed_value_lengths,
        maximum_disclosed_value_bytes,
        ca_membership_index: ca_membership_path.index,
        ca_membership_path_has_nonzero_sibling: ca_membership_path
            .siblings
            .iter()
            .any(|sibling| *sibling != [0; 32]),
    })
}

fn authoritative_state_v1(
    trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
    certificate_policy: PrivacyZkX509CertificatePolicyRecordV1,
    crl: PrivacyZkX509CrlRecordV1,
) -> Result<PrivacyZkX509AuthoritativeStateV1, &'static str> {
    let mut commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
    commitments.insert(
        PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision(
            trust_anchor.trust_anchor_id,
            trust_anchor.record_epoch,
        )
        .map_err(|_| "release trust-anchor revision key failed")?,
        PrivacyStateItemRecordV1::zk_x509_trust_anchor_governance(trust_anchor, 1)
            .map_err(|_| "release trust-anchor state record failed")?,
    );
    commitments.insert(
        PrivacyCommitmentKeyV1::zk_x509_certificate_policy_revision(
            certificate_policy.trust_anchor_id,
            certificate_policy.policy_id,
            certificate_policy.record_epoch,
        )
        .map_err(|_| "release policy revision key failed")?,
        PrivacyStateItemRecordV1::zk_x509_certificate_policy_governance(
            certificate_policy.clone(),
            1,
        )
        .map_err(|_| "release policy state record failed")?,
    );
    commitments.insert(
        PrivacyCommitmentKeyV1::zk_x509_crl_current(crl.trust_anchor_id, crl.certificate_policy_id)
            .map_err(|_| "release current-CRL key failed")?,
        PrivacyStateItemRecordV1::zk_x509_crl_governance(crl, 1)
            .map_err(|_| "release CRL state record failed")?,
    );

    let ca_namespace = privacy_zk_x509_ca_namespace_v1(trust_anchor.trust_anchor_id)
        .map_err(|_| "release CA namespace failed")?;
    let root_key = PrivacyRootKeyV1::new(
        ca_namespace,
        PrivacyRootRoleV1::CertificateAuthorityMembership,
        trust_anchor.ca_membership_root_epoch,
        trust_anchor.ca_membership_root,
    )
    .map_err(|_| "release CA root key failed")?;
    let publication = PrivacyRootPublicationV1::new(
        ca_namespace,
        PrivacyRootRoleV1::CertificateAuthorityMembership,
        root_key.epoch(),
        root_key.root(),
    )
    .map_err(|_| "release CA publication failed")?;
    let provenance = PrivacyRootProvenanceV1::zk_x509_ca_governance(
        publication
            .digest()
            .map_err(|_| "release CA publication digest failed")?,
        publication.namespace,
        publication.epoch,
        publication.root,
        trust_anchor,
        1,
    )
    .map_err(|_| "release CA root provenance failed")?;
    let mut roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
    roots.insert(root_key, provenance);
    let mut root_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
    root_heads.insert(
        PrivacyRootHeadKeyV1::new(
            ca_namespace,
            PrivacyRootRoleV1::CertificateAuthorityMembership,
        )
        .map_err(|_| "release CA root-head key failed")?,
        PrivacyRootHeadRecordV1::new(root_key.epoch(), root_key.root(), provenance, None)
            .map_err(|_| "release CA root head failed")?,
    );
    load_privacy_zk_x509_authoritative_state_v1(
        trust_anchor.trust_anchor_id,
        certificate_policy.policy_id,
        PrivacyConsensusLimitsV1::taira_default().retained_root_count,
        &commitments.view(),
        &roots.view(),
        &root_heads.view(),
    )
    .map_err(|_| "release authoritative state join failed")
}

fn p256_key(seed: u8) -> Result<P256SigningKey, &'static str> {
    let mut bytes = [0_u8; 32];
    bytes[31] = seed;
    P256SigningKey::from_slice(&bytes).map_err(|_| "invalid deterministic P-256 key")
}

#[allow(clippy::too_many_arguments)]
fn certificate(
    serial: &[u8],
    issuer: &[u8],
    subject: &[u8],
    subject_key: &P256SigningKey,
    issuer_key: &P256SigningKey,
    subject_key_identifier: &[u8],
    authority_key_identifier: &[u8],
    ca_path_len: Option<u32>,
) -> Vec<u8> {
    let spki = spki(subject_key);
    let basic_constraints = ca_path_len.map_or_else(
        || sequence(&[]),
        |path_len| sequence(&[tlv(0x01, &[0xff]), integer(u64::from(path_len))]),
    );
    let key_usage = if ca_path_len.is_some() {
        bit_string(&[0x06], 1)
    } else {
        bit_string(&[0x80], 7)
    };
    let mut extensions = vec![
        extension(
            OID_AUTHORITY_KEY_IDENTIFIER,
            false,
            &aki_inner(authority_key_identifier),
        ),
        extension(
            OID_SUBJECT_KEY_IDENTIFIER,
            false,
            &octet_string(subject_key_identifier),
        ),
        extension(OID_KEY_USAGE, true, &key_usage),
        extension(OID_BASIC_CONSTRAINTS, true, &basic_constraints),
    ];
    if ca_path_len.is_none() {
        extensions.push(extension(
            OID_EXTENDED_KEY_USAGE,
            true,
            &sequence(&[object_identifier(OID_CLIENT_AUTHENTICATION)]),
        ));
    }
    let tbs = sequence(&[
        tlv(0xa0, &integer(2)),
        positive_integer(serial),
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        issuer.to_vec(),
        sequence(&[tlv(0x17, b"220101000000Z"), tlv(0x17, b"300101000000Z")]),
        subject.to_vec(),
        spki,
        tlv(0xa3, &sequence(&extensions)),
    ]);
    let signature: P256Signature = issuer_key.sign(&tbs);
    sequence(&[
        tbs,
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(signature.to_der().as_bytes(), 0),
    ])
}

fn crl(
    issuer: &[u8],
    issuer_key: &P256SigningKey,
    authority_key_identifier: &[u8],
    number: u64,
    revoked_serials: &[Vec<u8>],
    times: ZkX509ReleaseTimesV1,
) -> Result<Vec<u8>, &'static str> {
    let extensions = sequence(&[
        extension(
            OID_AUTHORITY_KEY_IDENTIFIER,
            false,
            &aki_inner(authority_key_identifier),
        ),
        extension(OID_CRL_NUMBER, false, &integer(number)),
    ]);
    let mut fields = vec![
        integer(1),
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        issuer.to_vec(),
        tlv(
            0x17,
            &utc_time_contents_v1(times.crl_this_update_unix_seconds)?,
        ),
        tlv(
            0x17,
            &utc_time_contents_v1(times.crl_next_update_unix_seconds)?,
        ),
    ];
    if !revoked_serials.is_empty() {
        let revoked_at = utc_time_contents_v1(times.revoked_at_unix_seconds)?;
        fields.push(sequence(
            &revoked_serials
                .iter()
                .map(|serial| sequence(&[positive_integer(serial), tlv(0x17, &revoked_at)]))
                .collect::<Vec<_>>(),
        ));
    }
    fields.push(tlv(0xa0, &extensions));
    let tbs = sequence(&fields);
    let signature: P256Signature = issuer_key.sign(&tbs);
    Ok(sequence(&[
        tbs,
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(signature.to_der().as_bytes(), 0),
    ]))
}

fn utc_time_contents_v1(unix_seconds: u64) -> Result<[u8; 13], &'static str> {
    let unix_seconds = i64::try_from(unix_seconds)
        .map_err(|_| "release fixture UTCTime exceeds signed timestamp range")?;
    let date_time = OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map_err(|_| "release fixture UTCTime is outside the supported calendar")?;
    let year = date_time.year();
    if !(1950..=2049).contains(&year) {
        return Err("release fixture UTCTime is outside RFC 5280 UTCTime years");
    }
    let encoded = format!(
        "{:02}{:02}{:02}{:02}{:02}{:02}Z",
        year.rem_euclid(100),
        u8::from(date_time.month()),
        date_time.day(),
        date_time.hour(),
        date_time.minute(),
        date_time.second(),
    );
    encoded
        .into_bytes()
        .try_into()
        .map_err(|_| "release fixture UTCTime encoded to a noncanonical width")
}

fn spki(key: &P256SigningKey) -> Vec<u8> {
    let point = key.verifying_key().to_encoded_point(false);
    sequence(&[
        ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(point.as_bytes(), 0),
    ])
}

fn name(attributes: &[(&[u8], u8, &[u8])]) -> Vec<u8> {
    sequence(
        &attributes
            .iter()
            .map(|(oid, tag, value)| {
                tlv(0x31, &sequence(&[object_identifier(oid), tlv(*tag, value)]))
            })
            .collect::<Vec<_>>(),
    )
}

fn extension(oid: &[u8], critical: bool, inner: &[u8]) -> Vec<u8> {
    let mut fields = vec![object_identifier(oid)];
    if critical {
        fields.push(tlv(0x01, &[0xff]));
    }
    fields.push(octet_string(inner));
    sequence(&fields)
}

fn aki_inner(identifier: &[u8]) -> Vec<u8> {
    sequence(&[tlv(0x80, identifier)])
}

fn object_identifier(contents: &[u8]) -> Vec<u8> {
    tlv(0x06, contents)
}

fn octet_string(contents: &[u8]) -> Vec<u8> {
    tlv(0x04, contents)
}

fn bit_string(bytes: &[u8], unused_bits: u8) -> Vec<u8> {
    let mut contents = Vec::with_capacity(bytes.len() + 1);
    contents.push(unused_bits);
    contents.extend_from_slice(bytes);
    tlv(0x03, &contents)
}

fn integer(value: u64) -> Vec<u8> {
    if value == 0 {
        return tlv(0x02, &[0]);
    }
    let bytes = value.to_be_bytes();
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    let mut magnitude = bytes[first..].to_vec();
    if magnitude[0] & 0x80 != 0 {
        magnitude.insert(0, 0);
    }
    tlv(0x02, &magnitude)
}

fn positive_integer(magnitude: &[u8]) -> Vec<u8> {
    let mut contents = magnitude.to_vec();
    if contents.first().is_some_and(|byte| byte & 0x80 != 0) {
        contents.insert(0, 0);
    }
    tlv(0x02, &contents)
}

fn sequence(values: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &values.concat())
}

fn tlv(tag: u8, contents: &[u8]) -> Vec<u8> {
    let mut encoded = vec![tag];
    if contents.len() < 128 {
        encoded.push(contents.len() as u8);
    } else {
        let bytes = contents.len().to_be_bytes();
        let first = bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(bytes.len() - 1);
        let length = &bytes[first..];
        encoded.push(0x80 | length.len() as u8);
        encoded.extend_from_slice(length);
    }
    encoded.extend_from_slice(contents);
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::{
        codec::ZkX509WitnessCodecErrorV1,
        engine::{ZkX509EngineErrorV1, prepare_zk_x509_prover_input_v1},
        merkle::ZkX509MerkleErrorV1,
        profile::ZK_X509_ATTRIBUTE_SALT_BYTES_V1,
    };

    #[test]
    fn canonical_release_fixture_is_exact_round_trippable_and_state_joined() {
        let fixture = build_zk_x509_reference_fixture_v1().expect("canonical release fixture");
        assert_eq!(fixture.witness.certificate_chain_der.len(), 2);
        assert_eq!(fixture.statement.disclosed_attributes.len(), 1);
        assert_eq!(fixture.crl_entry_count, 0);
        assert_eq!(fixture.resource_shape.certificate_chain_depth, 2);
        assert_eq!(fixture.resource_shape.maximum_serial_bytes, 1);
        assert_eq!(fixture.resource_shape.disclosed_value_lengths, [0, 0, 0, 5]);
        fixture
            .resource_shape
            .validate_v1()
            .expect("canonical release resource shape");
        assert_eq!(
            fixture.statement.trust_anchor_record_digest,
            fixture.authoritative_state.trust_anchor().record_digest
        );
        assert_eq!(
            fixture.statement.certificate_policy_record_digest,
            fixture
                .authoritative_state
                .certificate_policy()
                .record_digest
        );
        assert_eq!(
            fixture.statement.crl_record_digest,
            fixture.authoritative_state.crl_record().record_digest
        );
        let encoded = fixture.witness.encode_v1().expect("canonical witness wire");
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&encoded).expect("exact witness decode"),
            fixture.witness
        );
    }

    #[test]
    fn network_release_fixture_binds_live_time_without_changing_the_fixed_kat_path() {
        const TRUSTED_BLOCK_TIMESTAMP_MS: u64 = 1_785_024_000_123;
        const TRUSTED_BLOCK_UNIX_SECONDS: u64 = TRUSTED_BLOCK_TIMESTAMP_MS / 1_000;

        let network_wallet = AccountId::new(
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
                .expect("network wallet key")
                .public_key()
                .clone(),
        );
        let fixture = build_zk_x509_network_release_fixture_v1(
            reference_statement_context_v1(),
            TRUSTED_BLOCK_TIMESTAMP_MS,
            network_wallet.clone(),
        )
        .expect("live-time network release fixture");
        let crl = fixture.authoritative_state.crl_record();
        assert_eq!(
            fixture.statement.presentation_not_before_unix_seconds,
            TRUSTED_BLOCK_UNIX_SECONDS
        );
        assert_eq!(
            fixture.statement.presentation_not_after_unix_seconds,
            TRUSTED_BLOCK_UNIX_SECONDS + ZK_X509_MAX_CRL_AGE_SECONDS_V1
        );
        assert_eq!(crl.this_update_unix_seconds, TRUSTED_BLOCK_UNIX_SECONDS);
        assert_eq!(
            crl.next_update_unix_seconds,
            TRUSTED_BLOCK_UNIX_SECONDS + ZK_X509_MAX_CRL_AGE_SECONDS_V1 + 1
        );
        let parsed_crl = parse_crl_v1(&fixture.witness.crl_der).expect("network release CRL");
        assert_eq!(parsed_crl.this_update, crl.this_update_unix_seconds);
        assert_eq!(parsed_crl.next_update, crl.next_update_unix_seconds);
        assert_eq!(fixture.statement.wallet_account, network_wallet);
        assert_ne!(
            fixture.statement.wallet_account,
            fixed_release_wallet_account_v1().expect("fixed KAT wallet")
        );
        prepare_zk_x509_prover_input_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            TRUSTED_BLOCK_TIMESTAMP_MS,
            &PrivacyConsensusLimitsV1::taira_default(),
            &fixture
                .witness
                .encode_v1()
                .expect("network release witness encoding"),
        )
        .expect("network release production prover preflight");

        let successor = build_zk_x509_network_release_successor_fixture_v1(
            reference_statement_context_v1(),
            TRUSTED_BLOCK_TIMESTAMP_MS + 1_000,
            network_wallet,
            crl.clone(),
        )
        .expect("signed network release CRL successor fixture");
        let successor_crl = successor.authoritative_state.crl_record();
        assert_eq!(successor_crl.record_epoch, crl.record_epoch + 1);
        assert_eq!(successor_crl.crl_number, crl.crl_number + 1);
        assert_eq!(
            successor_crl.previous_record_digest,
            Some(crl.record_digest)
        );
        assert_ne!(successor_crl.crl_der_digest, crl.crl_der_digest);
        assert_eq!(
            successor.statement.certificate_nullifier,
            fixture.statement.certificate_nullifier
        );
        assert_eq!(
            parse_crl_v1(&successor.witness.crl_der)
                .expect("signed successor CRL")
                .crl_number,
            successor_crl.crl_number
        );
        iroha_data_model::privacy::validate_zk_x509_crl_rotation_v1(&crl, &successor_crl)
            .expect("canonical release CRL rotation");

        let fixed = build_zk_x509_reference_fixture_v1().expect("fixed KAT fixture");
        let fixed_crl = parse_crl_v1(&fixed.witness.crl_der).expect("fixed KAT CRL");
        assert_eq!(fixed_crl.this_update, CRL_THIS_UPDATE);
        assert_eq!(fixed_crl.next_update, CRL_NEXT_UPDATE);
    }

    #[test]
    fn maximum_release_fixture_hits_every_closed_structural_ceiling() {
        let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
            .expect("maximum release fixture");
        assert_eq!(
            fixture.witness.certificate_chain_der.len(),
            ZK_X509_MAX_CHAIN_DEPTH_V1
        );
        assert_eq!(
            fixture.statement.disclosed_attributes.len(),
            iroha_data_model::privacy::ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1
        );
        assert_eq!(fixture.crl_entry_count, ZK_X509_MAX_CRL_ENTRIES_V1);
        assert_eq!(
            fixture.resource_shape.certificate_chain_depth as usize,
            ZK_X509_MAX_CHAIN_DEPTH_V1
        );
        assert_eq!(
            fixture.resource_shape.maximum_serial_bytes as usize,
            ZK_X509_MAX_SERIAL_BYTES_V1
        );
        assert_eq!(
            fixture.resource_shape.disclosed_value_lengths,
            [
                2,
                ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
                ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
                ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 as u16,
            ]
        );
        assert_eq!(
            fixture.resource_shape.maximum_disclosed_value_bytes as usize,
            ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
        );
        assert_eq!(fixture.resource_shape.ca_membership_index, 1);
        assert!(
            fixture
                .resource_shape
                .ca_membership_path_has_nonzero_sibling
        );
        fixture
            .resource_shape
            .validate_v1()
            .expect("maximum release resource shape");
        assert!(fixture.witness.crl_der.len() <= ZK_X509_MAX_CRL_BYTES_V1);
        assert!(
            fixture
                .witness
                .certificate_chain_der
                .iter()
                .all(|certificate| {
                    certificate.len() <= ZK_X509_MAX_CERTIFICATE_BYTES_V1 as usize
                })
        );
        assert_eq!(
            fixture.resource_shape.crl_der_length as usize,
            fixture.witness.crl_der.len()
        );
        for (index, certificate) in fixture.witness.certificate_chain_der.iter().enumerate() {
            assert_eq!(
                fixture.resource_shape.certificate_der_lengths[index] as usize,
                certificate.len()
            );
        }
        let leaf =
            parse_certificate_v1(&fixture.witness.certificate_chain_der[0]).expect("maximum leaf");
        assert_eq!(leaf.serial, MAXIMUM_LEAF_SERIAL_V1);
        assert_eq!(
            leaf.subject.attributes.map(|value| value.map(<[u8]>::len)),
            [
                Some(2),
                Some(ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1),
                Some(ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1),
                Some(ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1),
            ]
        );
        let parsed_crl = parse_crl_v1(&fixture.witness.crl_der).expect("maximum CRL");
        assert_eq!(parsed_crl.revoked_serials.len(), ZK_X509_MAX_CRL_ENTRIES_V1);
        assert!(
            parsed_crl
                .revoked_serials
                .iter()
                .all(|serial| serial.len() == ZK_X509_MAX_SERIAL_BYTES_V1)
        );
        assert!(
            parsed_crl
                .revoked_serials
                .iter()
                .all(|serial| *serial != leaf.serial)
        );
        let trust_anchor = fixture.authoritative_state.trust_anchor();
        let crl = fixture.authoritative_state.crl_record();
        prepare_zk_x509_prover_input_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            VALIDATION_TIME * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            &fixture
                .witness
                .encode_v1()
                .expect("maximum witness encoding"),
        )
        .expect("maximum production prover preflight");
        validate_reference_relation_v1(
            &fixture.statement,
            ZkX509GovernanceV1 {
                trust_anchor: &trust_anchor,
                certificate_policy: fixture.authoritative_state.certificate_policy(),
                crl: &crl,
            },
            &fixture.witness,
        )
        .expect("maximum relation");
    }

    #[test]
    fn production_preflight_rejects_matching_authoritative_malicious_crls_without_proving() {
        let revoked = preflight_with_matching_authoritative_crl_v1(&[vec![2]]);
        assert_eq!(
            revoked,
            ZkX509EngineErrorV1::ReferenceRelation(ZkX509RelationErrorV1::CertificateRevoked)
        );

        let serial_100 = maximum_crl_serial_v1(0).expect("serial 100");
        let serial_101 = maximum_crl_serial_v1(1).expect("serial 101");
        for (label, revoked_serials) in [
            ("duplicate", vec![serial_100.clone(), serial_100]),
            (
                "unordered",
                vec![
                    serial_101,
                    maximum_crl_serial_v1(0).expect("smaller unordered serial"),
                ],
            ),
            (
                "65 entries",
                (0..=ZK_X509_MAX_CRL_ENTRIES_V1)
                    .map(maximum_crl_serial_v1)
                    .collect::<Result<Vec<_>, _>>()
                    .expect("65 deterministic CRL serials"),
            ),
        ] {
            assert_eq!(
                preflight_with_matching_authoritative_crl_v1(&revoked_serials),
                ZkX509EngineErrorV1::ReferenceRelation(ZkX509RelationErrorV1::InvalidCrl),
                "{label} CRL reached proof construction"
            );
        }
    }

    #[test]
    fn production_preflight_rejects_wrong_ca_index_and_sibling_without_proving() {
        let fixture = build_zk_x509_reference_fixture_v1().expect("canonical release fixture");
        let mut wrong_index = fixture.witness.clone();
        wrong_index.ca_membership_path.index = 1;
        assert_eq!(
            production_preflight_error_v1(
                &fixture.statement,
                &fixture.authoritative_state,
                &wrong_index
                    .encode_v1()
                    .expect("wrong-index witness encoding"),
            ),
            ZkX509EngineErrorV1::ReferenceRelation(ZkX509RelationErrorV1::Merkle(
                ZkX509MerkleErrorV1::RootMismatch
            ))
        );

        let mut wrong_sibling = fixture.witness.clone();
        wrong_sibling.ca_membership_path.siblings[0][0] ^= 1;
        assert_eq!(
            production_preflight_error_v1(
                &fixture.statement,
                &fixture.authoritative_state,
                &wrong_sibling
                    .encode_v1()
                    .expect("wrong-sibling witness encoding"),
            ),
            ZkX509EngineErrorV1::ReferenceRelation(ZkX509RelationErrorV1::Merkle(
                ZkX509MerkleErrorV1::RootMismatch
            ))
        );
    }

    #[test]
    fn production_preflight_rejects_noncanonical_and_suffix_witness_bytes_without_proving() {
        let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
            .expect("maximum release fixture");
        let mut noncanonical = fixture
            .witness
            .encode_v1()
            .expect("maximum witness encoding");
        let opening_count = fixture.witness.attribute_openings.len();
        let opening_count_offset = noncanonical
            .len()
            .checked_sub(1 + opening_count * (1 + ZK_X509_ATTRIBUTE_SALT_BYTES_V1))
            .expect("opening suffix offset");
        assert_eq!(
            usize::from(noncanonical[opening_count_offset]),
            opening_count
        );
        let first_opening_index = opening_count_offset + 1;
        let second_opening_index = first_opening_index + 1 + ZK_X509_ATTRIBUTE_SALT_BYTES_V1;
        noncanonical[second_opening_index] = noncanonical[first_opening_index];
        assert_eq!(
            production_preflight_error_v1(
                &fixture.statement,
                &fixture.authoritative_state,
                &noncanonical,
            ),
            ZkX509EngineErrorV1::WitnessCodec(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );

        let mut suffix = fixture
            .witness
            .encode_v1()
            .expect("maximum witness encoding");
        suffix.push(0);
        assert_eq!(
            production_preflight_error_v1(
                &fixture.statement,
                &fixture.authoritative_state,
                &suffix,
            ),
            ZkX509EngineErrorV1::WitnessCodec(ZkX509WitnessCodecErrorV1::TrailingBytes)
        );
    }

    fn preflight_with_matching_authoritative_crl_v1(
        revoked_serials: &[Vec<u8>],
    ) -> ZkX509EngineErrorV1 {
        let fixture = build_zk_x509_reference_fixture_v1().expect("canonical release fixture");
        let root_key = p256_key(1).expect("deterministic root key");
        let root_name = name(&[(OID_COMMON_NAME, 0x0c, b"Iroha Test Root")]);
        let root_ski = [0x31; 20];
        let crl_der = crl(
            &root_name,
            &root_key,
            &root_ski,
            7,
            revoked_serials,
            ZkX509ReleaseTimesV1::FIXED_V1,
        )
        .expect("matching authoritative CRL DER");
        let issuer = parse_certificate_v1(&fixture.witness.certificate_chain_der[1])
            .expect("canonical issuer");
        let trust_anchor = fixture.authoritative_state.trust_anchor();
        let policy = fixture.authoritative_state.certificate_policy().clone();
        let crl_record = PrivacyZkX509CrlRecordV1::new(
            fixture.statement.trust_anchor_id,
            fixture.statement.certificate_policy_id,
            1,
            7,
            PrivacyX509CrlDerDigestV1::digest_exact_der(&crl_der),
            PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(issuer.spki_der),
            CRL_THIS_UPDATE,
            CRL_NEXT_UPDATE,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("matching authoritative CRL record");
        let authoritative_state =
            authoritative_state_v1(trust_anchor, policy, crl_record).expect("authoritative state");
        let mut statement = fixture.statement.clone();
        statement.crl_record_digest = crl_record.record_digest;
        let mut witness = fixture.witness.clone();
        witness.crl_der = crl_der;
        production_preflight_error_v1(
            &statement,
            &authoritative_state,
            &witness.encode_v1().expect("malicious witness encoding"),
        )
    }

    fn production_preflight_error_v1(
        statement: &IrohaZkX509StarkP256StatementV1,
        authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
        encoded_witness: &[u8],
    ) -> ZkX509EngineErrorV1 {
        match prepare_zk_x509_prover_input_v1(
            statement,
            authoritative_state,
            VALIDATION_TIME * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            encoded_witness,
        ) {
            Ok(_) => panic!("malicious witness passed production preflight"),
            Err(error) => error,
        }
    }
}
