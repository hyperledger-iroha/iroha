//! Authenticated Spartan Bootstrap activation (wire §3.6 "Ledger control" and
//! "Activation"). Native builds the package from one released Bootstrap and
//! encodes the claim once the payment key has signed an Activate control. A relay
//! checks the public terms, and the World verifier repeats them and then
//! verifies the complete proof pair and both accumulated claims under its own
//! installed parameters. This module has no ledger, replay store or
//! idempotency record; those stay World state.
//!
//! Package digest (wire §3.2, existing domains):
//!
//! ```text
//! statement_digest = P(kgwstmt1, statement[0..26])              = profile[STATEMENT_IO]
//! proof_digest     = P_bytes(kgwstep1, LE32 len(σ) ‖ σ)          σ = exact projected Bootstrap pair
//! receipt_digest   = P(kgworcp1, [m, r_lo, r_hi, s_lo, s_hi])    m = P_bytes(kgwrcpt1, body) = profile[RECEIPT_IO]
//! package_digest   = P(kgwpkg_1, [statement_digest, proof_digest, receipt_digest])
//! ```
//!
//! Bootstrap carries no Ω(pred), so σ is digested under `kgwstep1`. Like the Payment
//! package, the Spartan proof term digests the exact pair bytes, because the advance
//! receipt is signed before the recursive proof that contains it exists. An unchanged
//! retained completion therefore reproduces the same digest. The public inputs σ is
//! verified against are carried, but they are not part of the digest: the three IO
//! words above must equal the statement and receipt digests, and the World verifier
//! checks the rest by verifying the proof.
//!
//! Constructor originals (exact bytes; no conversion is applied):
//! - `account`: `norito::encode_canonical(&AccountId)`, the canonical V1 frame
//!   of at most 4,096 bytes. `H("account", account)` must equal the credential's
//!   account digest.
//! - `asset`: `norito::encode_canonical(&KagemushaWalletAssetScopeV1)`, at most
//!   1,024 bytes and valid. Its `asset_digest()` must equal the credential's asset
//!   digest and the capsule's asset.
//! - `issuer`: the 172-byte signer certificate original `body(108) ‖ signature(64)`.
//!   It must be the credential's Enrollment-role issuer, certified by the installed
//!   scheme root, and the certificate retained in the released head.
//!
//! The claim is a Fixed-layout Norito frame in the spec's field order:
//! `{version, parameter_id, control, credential, bootstrap: {statement, profile,
//! step_proof, receipt}, asset, issuer_certificate}`. `control` is the exact
//! canonical `KagemushaWalletLedgerControlV1` frame. The Bootstrap advance receipt
//! and the Activate control are two separate required payment-key signatures.
use super::*;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_WALLET_PACKAGE_DOMAIN_V1, KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1,
        KagemushaWalletAssetScopeV1, KagemushaWalletCredentialBodyV1,
        KagemushaWalletLedgerControlActionV1, KagemushaWalletLedgerControlV1,
        KagemushaWalletSchemeV1, kagemusha_wallet_account_digest_v1,
    },
};

/// Bound of one complete canonical Activation claim original.
pub const MAX_ACTIVATION_CLAIM_BYTES: usize = 32 * 1024;
/// Canonical AccountId original bound, equal to the account-admission intake's
/// `ACCOUNT_ORIGINAL_MAX_BYTES_V1`.
const ACCOUNT_ORIGINAL_MAX_BYTES: usize = 4096;
/// Canonical asset-scope original bound, equal to the intake's
/// `ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1`.
const ASSET_ORIGINAL_MAX_BYTES: usize = 1024;
const CERTIFICATE_ORIGINAL_BYTES: usize = 172;
const RECEIPT_ORIGINAL_BYTES: usize = bootstrap::RECEIPT_BYTES + 64;

/// Complete public Bootstrap package: the statement, the 64 public inputs its
/// step proof is verified against, the exact projected step proof σ and the
/// hardware-signed advance receipt τ (`body ‖ signature`).
#[derive(Clone, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.BootstrapPackage.v1")]
struct Package {
    statement: Vec<u8>,
    profile: Vec<u8>,
    step_proof: Vec<u8>,
    receipt: Vec<u8>,
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ActivationClaim.v1")]
struct Claim {
    version: u16,
    parameter_id: [u8; 32],
    control: Vec<u8>,
    credential: Vec<u8>,
    bootstrap: Package,
    asset: Vec<u8>,
    issuer_certificate: Vec<u8>,
}

impl Claim {
    fn decode_original(bytes: &[u8]) -> std::result::Result<Self, norito::Error> {
        if bytes.len() > MAX_ACTIVATION_CLAIM_BYTES {
            return Err(norito::Error::LengthMismatch);
        }
        let limits = norito::DecodeLimits::new(
            MAX_ACTIVATION_CLAIM_BYTES,
            MAX_ACTIVATION_CLAIM_BYTES,
            4 * MAX_ACTIVATION_CLAIM_BYTES,
            8 * MAX_ACTIVATION_CLAIM_BYTES,
            16,
        );
        let claim: Self = norito::core::with_decode_limits_scope(limits, || {
            norito::decode_canonical_with_layout(bytes, norito::core::WireLayout::Fixed)
        })?;
        if claim.version != 1 {
            return Err(norito::Error::NonCanonicalEncoding);
        }
        Ok(claim)
    }
}

fn claim_error(error: norito::Error) -> Error {
    if matches!(error, norito::Error::AllocationFailed { .. })
        || norito::core::decode_error_matches_active_limits(&error)
    {
        Error::Unavailable("local Activation decoder capacity")
    } else {
        Error::Malformed("canonical Activation claim")
    }
}

fn model_error(e: impl std::fmt::Display) -> Error {
    Error::Rejected(e.to_string())
}

/// Native's activation package for one released Bootstrap. Only
/// [`NativeEngine::bootstrap_activation_package`] constructs it, after verifying
/// the retained capsule, its completion, the proof pair and every original. It
/// holds public originals only, never the recovery witness.
pub struct BootstrapActivationPackage {
    parameter_id: [u8; 32],
    scheme: [u8; 32],
    relation: [u8; 32],
    root: p256::Point,
    asset_digest: [u8; 32],
    credential: Vec<u8>,
    issuer: Vec<u8>,
    asset: Vec<u8>,
    package: Package,
    digest: [u8; 32],
}

impl BootstrapActivationPackage {
    /// The package digest the Activate control must name.
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Encode the exact claim for a control the wallet payment key signed
    /// separately from the Bootstrap advance receipt. The control must be this
    /// wallet's Activate naming [`Self::digest`]. The encoded claim passes the
    /// relay's public checks before it is returned. Nothing is signed here.
    pub fn authorize(&self, control: &KagemushaWalletLedgerControlV1) -> Result<Vec<u8>> {
        let control = control.to_canonical_bytes().map_err(model_error)?;
        let bytes = super::encode(&Claim {
            version: 1,
            parameter_id: self.parameter_id,
            control,
            credential: self.credential.clone(),
            bootstrap: self.package.clone(),
            asset: self.asset.clone(),
            issuer_certificate: self.issuer.clone(),
        })?;
        if bytes.len() > MAX_ACTIVATION_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        let claim = Claim::decode_original(&bytes).map_err(claim_error)?;
        let terms = public_terms(
            &claim,
            &self.scheme,
            &self.relation,
            &self.root,
            &self.asset_digest,
        )?;
        if terms.digest != self.digest {
            return Err(Error::AuthorityMismatch("Activate control package digest"));
        }
        Ok(bytes)
    }
}

/// A claim whose public terms, proof pair and both accumulated claims passed
/// under the verifier's installed parameters. Core still records activation
/// state (idempotency, abandonment) itself.
pub struct VerifiedActivationClaim {
    wallet_id: [u8; 32],
    account_digest: [u8; 32],
    package_digest: [u8; 32],
}

impl VerifiedActivationClaim {
    pub fn wallet_id(&self) -> &[u8; 32] {
        &self.wallet_id
    }
    /// `H("account", ·)` bound by the issuer-signed credential.
    pub fn account_digest(&self) -> &[u8; 32] {
        &self.account_digest
    }
    pub fn package_digest(&self) -> &[u8; 32] {
        &self.package_digest
    }
}

impl NativeEngine {
    /// Build the activation package of one released Bootstrap from its retained
    /// capsule and completion. Native verifies them exactly as recovery does
    /// (resumed preparation, hardware receipt, head proof and both accumulators).
    /// `account`, `asset` and `issuer` are exact originals; see the module
    /// documentation for their encodings.
    pub fn bootstrap_activation_package(
        &self,
        capsule: &RecoveryCapsule,
        completion: &CompletionRecord,
        account: &[u8],
        asset: &[u8],
        issuer: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<BootstrapActivationPackage> {
        if capsule.kind() != 1 || capsule.enrollment_marker().is_none() {
            return Err(Error::AuthorityMismatch(
                "Activation requires a released Bootstrap",
            ));
        }
        if issuer.len() != CERTIFICATE_ORIGINAL_BYTES {
            return Err(Error::Malformed("Activation issuer original length"));
        }
        let account_digest = account_original_digest(account)?;
        asset_scope(asset)?;
        let (prepared, completed) =
            self.verify_released_operation(capsule, completion, parameters)?;
        if !matches!(
            prepared.witness.as_ref(),
            ExactOperationWitness::Bootstrap(_)
        ) {
            return Err(Error::AuthorityMismatch(
                "Activation requires a released Bootstrap",
            ));
        }
        let (c, statement, _, _) = terms(&prepared.witness);
        let head = completed.head();
        let frame: HeadFrame = super::decode(head.exact_pair())?;
        if frame.kind as usize != full::BOOTSTRAP
            || frame.parameters != *parameters.id()
            || frame.credential != credential_bytes(c)
            || frame.certificate != issuer
            || certificate_bytes(&head.certificate) != issuer
        {
            return Err(Error::AuthorityMismatch(
                "Activation released Bootstrap originals",
            ));
        }
        if account_digest != c.body[98..130] {
            return Err(Error::AuthorityMismatch("Activation account"));
        }
        let p = &parameters.inner;
        let enrolled = enrollment(
            &frame.credential,
            issuer,
            asset,
            &p.scheme,
            &p.root,
            capsule.asset(),
        )?;
        let package = Package {
            statement: word_bytes(statement),
            profile: word_bytes(head.profile()),
            step_proof: frame.pair,
            receipt: completion.exact_receipt().to_vec(),
        };
        let (_, _, digest) = bootstrap_terms(&enrolled, &package, &p.scheme, &p.relation)?;
        Ok(BootstrapActivationPackage {
            parameter_id: *parameters.id(),
            scheme: p.scheme,
            relation: p.relation,
            root: p.root.clone(),
            asset_digest: *capsule.asset(),
            credential: frame.credential,
            issuer: issuer.to_vec(),
            asset: asset.to_vec(),
            package,
            digest,
        })
    }

    /// World verifier for an Activation claim under an independently selected
    /// scheme and asset. It repeats every relay check, then recomputes the
    /// certified key table on the installed generators, matches the credential
    /// context of the public inputs, and verifies the complete Bootstrap proof pair
    /// with both native matrix accumulators and both inherited generator claims.
    pub fn verify_activation_claim(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
        scheme: &[u8; 32],
        asset: &[u8; 32],
    ) -> Result<VerifiedActivationClaim> {
        if bytes.len() > MAX_ACTIVATION_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        let claim = Claim::decode_original(bytes).map_err(claim_error)?;
        scope(parameters, &claim.parameter_id)?;
        let p = &parameters.inner;
        if &p.scheme != scheme {
            return Err(Error::AuthorityMismatch("Activation reserve scheme/asset"));
        }
        let terms = public_terms(&claim, &p.scheme, &p.relation, &p.root, asset)?;
        let enrolled = &terms.enrollment;
        authenticate_credential(p, &enrolled.credential, &enrolled.issuer)?;
        profile_matches(p, &terms.profile, &enrolled.credential)?;
        let projected = projection(&terms.profile, full::BOOTSTRAP)?;
        let decoded = carrier::decode_projected_pair(
            &p.profile(full::BOOTSTRAP)?,
            &claim.bootstrap.step_proof,
            &projected,
        )
        .map_err(Error::Rejected)?;
        verify_pair(p, full::BOOTSTRAP, decoded)?;
        Ok(VerifiedActivationClaim {
            wallet_id: enrolled.fields.wallet_id,
            account_digest: enrolled.fields.account_digest,
            package_digest: terms.digest,
        })
    }

    /// Relay check of an account-signed Activation before submission. The claim
    /// must carry the exact authenticated E6 credential original, its singleton
    /// root-certified Enrollment issuer, the asset scope, a Bootstrap package whose
    /// statement, receipt and public-input digests agree and whose advance receipt
    /// the payment key signed, and a fully valid `kgwlctl1` Activate control by the
    /// same key that pins this package's digest. `account` is the submitting
    /// signer and must be the account the credential names. Returns the pinned
    /// package digest. The World verifier enforces the installed Bootstrap relation,
    /// the remaining public inputs and the accumulators independently.
    pub fn activation_relay_package(
        bytes: &[u8],
        scheme: &KagemushaWalletSchemeV1,
        asset: &[u8; 32],
        credential_original: &[u8],
        account: &AccountId,
    ) -> Result<[u8; 32]> {
        if bytes.len() > MAX_ACTIVATION_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        scheme.validate().map_err(model_error)?;
        let claim = Claim::decode_original(bytes).map_err(claim_error)?;
        if claim.credential != credential_original {
            return Err(Error::AuthorityMismatch(
                "Activation relay credential original",
            ));
        }
        let root = crate::native::point(scheme.scheme_root_key.as_ref())?;
        let terms = public_terms(
            &claim,
            &scheme.scheme_id(),
            &scheme.relation_id,
            &root,
            asset,
        )?;
        if kagemusha_wallet_account_digest_v1(account).map_err(model_error)?
            != terms.enrollment.fields.account_digest
        {
            return Err(Error::AuthorityMismatch("Activation relay account"));
        }
        Ok(terms.digest)
    }
}

/// An authenticated credential, its singleton issuer and validated enrollment fields.
struct Enrollment {
    credential: Credential,
    issuer: certificate::Certificate,
    fields: KagemushaWalletCredentialBodyV1,
}

struct Terms {
    enrollment: Enrollment,
    profile: [Fp; 64],
    digest: [u8; 32],
}

/// The public terms shared by the relay and the World verifier, following the
/// spec's order: scope, credential and certificates, asset, action, Bootstrap
/// package and digest, then the control signature.
fn public_terms(
    claim: &Claim,
    scheme: &[u8; 32],
    relation: &[u8; 32],
    root: &p256::Point,
    asset: &[u8; 32],
) -> Result<Terms> {
    if claim.parameter_id != *relation {
        return Err(Error::AuthorityMismatch("Activation parameter identity"));
    }
    let enrolled = enrollment(
        &claim.credential,
        &claim.issuer_certificate,
        &claim.asset,
        scheme,
        root,
        asset,
    )?;
    let control = KagemushaWalletLedgerControlV1::decode_canonical(&claim.control, scheme)
        .map_err(|e| Error::Rejected(format!("Activate control: {e}")))?;
    if control.to_canonical_bytes().map_err(model_error)? != claim.control {
        return Err(Error::Malformed("canonical Activate control"));
    }
    let KagemushaWalletLedgerControlActionV1::Activate { package_digest } = control.body.action
    else {
        return Err(Error::AuthorityMismatch(
            "Activation requires an Activate control",
        ));
    };
    if control.body.scheme_id != enrolled.fields.scheme_id
        || control.body.asset_digest != enrolled.fields.asset_digest
        || control.body.wallet_id != enrolled.fields.wallet_id
    {
        return Err(Error::AuthorityMismatch("Activate control wallet scope"));
    }
    let (profile, receipt, digest) =
        bootstrap_terms(&enrolled, &claim.bootstrap, scheme, relation)?;
    if package_digest != digest {
        return Err(Error::AuthorityMismatch("Activate control package digest"));
    }
    control
        .verify(&enrolled.fields.payment_key)
        .map_err(|_| Error::AuthorityMismatch("Activate control payment-key signature"))?;
    let control_signature: &[u8] = control.signature.as_ref();
    if control_signature == receipt.signature.as_slice() {
        return Err(Error::AuthorityMismatch(
            "Activate control reuses the advance receipt signature",
        ));
    }
    Ok(Terms {
        enrollment: enrolled,
        profile,
        digest,
    })
}

/// Credential (596), singleton issuer (172) and asset-scope originals under one
/// scheme root: validated enrollment fields, a root-certified Enrollment issuer,
/// its credential signature, and the asset scope the credential names. The
/// certified key table is recomputed only where installed generators exist.
fn enrollment(
    credential_original: &[u8],
    issuer_original: &[u8],
    asset_original: &[u8],
    scheme: &[u8; 32],
    root: &p256::Point,
    asset: &[u8; 32],
) -> Result<Enrollment> {
    let credential = read_credential(credential_original)?;
    let fields = credential.enrollment_fields()?;
    if fields.scheme_id != *scheme || fields.asset_digest != *asset {
        return Err(Error::AuthorityMismatch("Activation scheme/asset"));
    }
    let issuer = read_certificate(issuer_original)?;
    let key =
        certificate::authenticate_signer(&issuer, scheme, root, certificate::Role::Enrollment)
            .map_err(Error::AuthorityMismatch)?;
    if fields.issuer_certificate != certificate::certificate_digest(&issuer).to_repr() {
        return Err(Error::AuthorityMismatch("Activation issuer certificate"));
    }
    verify_signature(
        &key,
        crate::credential::message(&credential),
        &credential.signature,
    )?;
    if asset_scope(asset_original)?.asset_digest() != fields.asset_digest {
        return Err(Error::AuthorityMismatch("Activation asset scope"));
    }
    Ok(Enrollment {
        credential,
        issuer,
        fields,
    })
}

/// The exact Bootstrap package of `enrolled`: canonical words, the Bootstrap
/// statement scoped to this credential and relation, an advance receipt over that
/// statement signed by the credential's payment key, and public inputs that
/// name both. Returns the public inputs, the receipt and the package digest.
fn bootstrap_terms(
    enrolled: &Enrollment,
    package: &Package,
    scheme: &[u8; 32],
    relation: &[u8; 32],
) -> Result<([Fp; 64], bootstrap::Receipt, [u8; 32])> {
    let c = &enrolled.credential;
    let statement = words::<26>(&package.statement)?;
    let profile = words::<64>(&package.profile)?;
    let receipt = read_receipt(&package.receipt)?;
    if package.step_proof.is_empty() {
        return Err(Error::Malformed("Bootstrap step proof"));
    }
    if statement[16] != Fp::from(crate::statement::Kind::Bootstrap.tag()) {
        return Err(Error::AuthorityMismatch(
            "Activation requires a Bootstrap statement",
        ));
    }
    let limbs = crate::native::funding::limbs;
    if statement[0] != Fp::ONE
        || statement[1..3] != limbs(relation)
        || statement[3..5] != limbs(scheme)
        || statement[5..7] != limbs(&enrolled.fields.asset_digest)
        || statement[7] != crate::credential::digest(c)
        || statement[8] != Fp::ONE
        || statement[14] != Fp::ZERO
        || statement[15] == Fp::ZERO
        || statement[17..19] != limbs(&enrolled.fields.enrollment_id)
    {
        return Err(Error::AuthorityMismatch(
            "Bootstrap statement credential scope",
        ));
    }
    let statement_digest = crate::statement::digest(&statement);
    let body = &receipt.body;
    let zero = |bytes: &[u8]| bytes.iter().all(|b| *b == 0);
    if body[..2] != 1u16.to_le_bytes()
        || body[2..34] != scheme[..]
        || body[34..66] != enrolled.fields.wallet_id
        || body[66..98] != c.body[259..291]
        || !zero(&body[98..114])
        || body[114..146] != c.bootstrap_operation_id()
        || body[146..178] != statement[14].to_repr()
        || body[178..210] != statement[15].to_repr()
        || body[210..242] != statement_digest.to_repr()
        || zero(&body[242..274])
        || zero(&body[274..306])
        || !zero(&body[306..338])
    {
        return Err(Error::AuthorityMismatch("Bootstrap advance receipt terms"));
    }
    field(
        body[242..274]
            .try_into()
            .expect("fixed receipt proof digest"),
    )?;
    let key = crate::native::point(&c.body[130..195])?;
    verify_signature(&key, crate::receipt::message(&receipt), &receipt.signature)
        .map_err(|_| Error::AuthorityMismatch("Bootstrap advance receipt signature"))?;
    if profile[full::HEAD_IO] != statement[15]
        || profile[full::STATEMENT_IO] != statement_digest
        || profile[full::RECEIPT_IO] != crate::receipt::digest(&receipt)
    {
        return Err(Error::AuthorityMismatch("Bootstrap package public inputs"));
    }
    let digest = package_digest(&statement, &package.step_proof, &receipt)?.to_repr();
    Ok((profile, receipt, digest))
}

/// `P(kgwpkg_1, [statement_digest, P_bytes(kgwstep1, LE32 len(σ) ‖ σ), receipt_digest])`.
fn package_digest(
    statement: &[Fp; 26],
    step_proof: &[u8],
    receipt: &bootstrap::Receipt,
) -> Result<Fp> {
    let length = u32::try_from(step_proof.len()).map_err(|_| Error::ResourceLimit)?;
    let mut framed = Vec::with_capacity(4 + step_proof.len());
    framed.extend_from_slice(&length.to_le_bytes());
    framed.extend_from_slice(step_proof);
    let proof = crate::payment::byte_digest(KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1, &framed);
    Ok(iroha_pasta::poseidon::hash_with_domain(
        KAGEMUSHA_WALLET_PACKAGE_DOMAIN_V1,
        &[
            crate::statement::digest(statement),
            proof,
            crate::receipt::digest(receipt),
        ],
    ))
}

/// `H("account", ·)` of an exact canonical AccountId original.
fn account_original_digest(original: &[u8]) -> Result<[u8; 32]> {
    if original.is_empty() || original.len() > ACCOUNT_ORIGINAL_MAX_BYTES {
        return Err(Error::Malformed("account original length"));
    }
    let account: AccountId = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(|_| Error::Malformed("canonical account original"))?;
    if norito::encode_canonical(&account).map_err(model_error)? != original {
        return Err(Error::Malformed("canonical account original"));
    }
    kagemusha_wallet_account_digest_v1(&account).map_err(model_error)
}

/// An exact canonical, valid asset-scope original.
fn asset_scope(original: &[u8]) -> Result<KagemushaWalletAssetScopeV1> {
    if original.is_empty() || original.len() > ASSET_ORIGINAL_MAX_BYTES {
        return Err(Error::Malformed("asset scope original length"));
    }
    let asset: KagemushaWalletAssetScopeV1 = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(|_| Error::Malformed("canonical asset scope original"))?;
    asset.validate().map_err(model_error)?;
    if norito::encode_canonical(&asset).map_err(model_error)? != original {
        return Err(Error::Malformed("canonical asset scope original"));
    }
    Ok(asset)
}

fn read_receipt(bytes: &[u8]) -> Result<bootstrap::Receipt> {
    if bytes.len() != RECEIPT_ORIGINAL_BYTES {
        return Err(Error::Malformed("Bootstrap advance receipt length"));
    }
    Ok(bootstrap::Receipt {
        body: bytes[..bootstrap::RECEIPT_BYTES].try_into().unwrap(),
        signature: bytes[bootstrap::RECEIPT_BYTES..].try_into().unwrap(),
    })
}

fn words<const N: usize>(bytes: &[u8]) -> Result<[Fp; N]> {
    if bytes.len() != 32 * N {
        return Err(Error::Malformed("Activation field count"));
    }
    let mut values = [Fp::ZERO; N];
    for (slot, word) in values.iter_mut().zip(bytes.chunks_exact(32)) {
        *slot = field(word.try_into().unwrap())?;
    }
    Ok(values)
}

fn word_bytes(values: &[Fp]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_repr()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        full::WalletKey,
        native::device_fixture::{self, vector_asset},
    };
    use iroha_data_model::kagemusha::{
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaWalletLedgerControlBodyV1,
        KagemushaWalletObjectDigestDomainV1, kagemusha_wallet_id_v1,
        kagemusha_wallet_package_digest_v1, kagemusha_wallet_poseidon_bytes_v1,
        kagemusha_wallet_signed_object_digest_v1,
    };
    use std::sync::OnceLock;

    /// Public secret of the fixture Enrollment issuer `device_fixture` certifies.
    const ISSUER_SECRET: [u8; 32] = [0x26; 32];

    fn account(which: usize) -> AccountId {
        // RFC 8032 public test vectors.
        let key = [
            "ed0120D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A",
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C",
        ][which];
        AccountId::new(key.parse().unwrap())
    }

    fn other_asset(scope: &KagemushaWalletAssetScopeV1) -> KagemushaWalletAssetScopeV1 {
        let mut other = scope.clone();
        other.scale = if scope.scale == 0 { 1 } else { scope.scale - 1 };
        other.validate().unwrap();
        other
    }

    fn sec1(q: &p256::Point) -> [u8; 65] {
        let mut out = [0; 65];
        out[0] = 4;
        out[1..33].copy_from_slice(&crate::certificate::scalar_bytes(&q.x));
        out[33..].copy_from_slice(&crate::certificate::scalar_bytes(&q.y));
        out
    }

    /// Public Bootstrap originals of one fixture wallet enrolled for `account(0)`
    /// and the vector asset. No proof is made: σ is fixed public bytes, which
    /// the relay digests but never verifies.
    struct Fixture {
        scheme: KagemushaWalletSchemeV1,
        key: WalletKey,
        asset: Vec<u8>,
        asset_digest: [u8; 32],
        credential: Vec<u8>,
        issuer: Vec<u8>,
        package: Package,
        digest: [u8; 32],
    }

    fn fixture() -> &'static Fixture {
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let scheme = device_fixture::scheme();
            let cert = device_fixture::enrollment_certificate();
            let key = WalletKey::labelled("Activation relay fixture wallet");
            let (context, mut input) = bootstrap::fixture_with_certificate(
                &key,
                &crate::ipa::Gens::<iroha_pasta::Eq>::new(4096),
                cert.clone(),
                scheme.relation_id,
            );
            assert_eq!(context.root.scheme, scheme.scheme_id());
            let scope = vector_asset();
            let asset_digest = scope.asset_digest();
            let limbs = crate::native::funding::limbs;
            // Re-issue the credential for this asset and account; the wallet
            // identity follows the asset.
            let c = &mut input.credential;
            c.body[34..66].copy_from_slice(&asset_digest);
            c.body[98..130]
                .copy_from_slice(&kagemusha_wallet_account_digest_v1(&account(0)).unwrap());
            let payment_key = KagemushaDevicePublicKeyV1::try_from(&c.body[130..195]).unwrap();
            let wallet = kagemusha_wallet_id_v1(
                &scheme.scheme_id(),
                &asset_digest,
                &payment_key,
                c.body[448..480].try_into().unwrap(),
            );
            c.body[66..98].copy_from_slice(&wallet);
            c.signature = bootstrap::sign(
                crate::credential::message(c),
                &BigUint::from_bytes_be(&ISSUER_SECRET),
                "Activation fixture issuer",
            );
            c.enrollment_fields().unwrap();
            let c = input.credential.clone();
            // Restate the zero state, statement and signed advance receipt.
            input.core[3..5].copy_from_slice(&limbs(&asset_digest));
            input.core[5..7].copy_from_slice(&limbs(&wallet));
            input.core[7] = crate::credential::digest(&c);
            input.statement[3..7].copy_from_slice(&input.core[1..5]);
            input.statement[7] = input.core[7];
            input.statement[15] = crate::monetary_state::commitment(&input.core, &input.rest);
            let head = input.statement[15].to_repr();
            let statement_digest = crate::statement::digest(&input.statement).to_repr();
            let body = &mut input.receipt.body;
            body[34..66].copy_from_slice(&wallet);
            body[114..146].copy_from_slice(&c.bootstrap_operation_id());
            body[178..210].copy_from_slice(&head);
            body[210..242].copy_from_slice(&statement_digest);
            input.receipt.signature = bootstrap::sign(
                crate::receipt::message(&input.receipt),
                &key.d,
                "Activation fixture advance receipt",
            );
            let profile = bootstrap::public_profile(&context, &input);
            assert_eq!(profile[full::HEAD_IO], input.statement[15]);
            let package = Package {
                statement: word_bytes(&input.statement),
                profile: word_bytes(&profile),
                step_proof: (0..1536u32)
                    .map(|i| (i.wrapping_mul(131) >> 3) as u8)
                    .collect(),
                receipt: [
                    input.receipt.body.as_slice(),
                    input.receipt.signature.as_slice(),
                ]
                .concat(),
            };
            let digest = package_digest(&input.statement, &package.step_proof, &input.receipt)
                .unwrap()
                .to_repr();
            Fixture {
                asset: norito::encode_canonical(&scope).unwrap(),
                scheme,
                key,
                asset_digest,
                credential: credential_bytes(&c),
                issuer: certificate_bytes(&cert),
                package,
                digest,
            }
        })
    }

    fn wallet(f: &Fixture) -> [u8; 32] {
        f.credential[66..98].try_into().unwrap()
    }

    fn signed(
        key: &WalletKey,
        body: KagemushaWalletLedgerControlBodyV1,
    ) -> KagemushaWalletLedgerControlV1 {
        let signature = bootstrap::sign(
            field(&body.signing_message()).unwrap(),
            &key.d,
            "Activation fixture ledger control",
        );
        KagemushaWalletLedgerControlV1 {
            body,
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature).unwrap(),
        }
    }

    fn control(
        f: &Fixture,
        key: &WalletKey,
        edit: impl FnOnce(&mut KagemushaWalletLedgerControlBodyV1),
    ) -> KagemushaWalletLedgerControlV1 {
        let mut body = KagemushaWalletLedgerControlBodyV1 {
            version: 1,
            scheme_id: f.scheme.scheme_id(),
            asset_digest: f.asset_digest,
            wallet_id: wallet(f),
            action: KagemushaWalletLedgerControlActionV1::Activate {
                package_digest: f.digest,
            },
            nonce: [0x79; 32],
        };
        edit(&mut body);
        signed(key, body)
    }

    fn claim_with(
        f: &Fixture,
        control: &KagemushaWalletLedgerControlV1,
        edit: impl FnOnce(&mut Claim),
    ) -> Vec<u8> {
        let mut claim = Claim {
            version: 1,
            parameter_id: f.scheme.relation_id,
            control: control.to_canonical_bytes().unwrap(),
            credential: f.credential.clone(),
            bootstrap: f.package.clone(),
            asset: f.asset.clone(),
            issuer_certificate: f.issuer.clone(),
        };
        edit(&mut claim);
        crate::native::encode(&claim).unwrap()
    }

    /// Re-sign the claim's Activate control for its (possibly edited) package.
    fn reseal(claim: &mut Claim, key: &WalletKey, scheme: &[u8; 32]) -> [u8; 32] {
        let statement = words::<26>(&claim.bootstrap.statement).unwrap();
        let receipt = read_receipt(&claim.bootstrap.receipt).unwrap();
        let digest = package_digest(&statement, &claim.bootstrap.step_proof, &receipt)
            .unwrap()
            .to_repr();
        let mut body = KagemushaWalletLedgerControlV1::decode_canonical(&claim.control, scheme)
            .unwrap()
            .body;
        body.action = KagemushaWalletLedgerControlActionV1::Activate {
            package_digest: digest,
        };
        claim.control = signed(key, body).to_canonical_bytes().unwrap();
        digest
    }

    fn set_word(bytes: &mut [u8], index: usize, value: Fp) {
        bytes[32 * index..32 * index + 32].copy_from_slice(&value.to_repr());
    }

    fn relay(f: &Fixture, bytes: &[u8]) -> Result<[u8; 32]> {
        NativeEngine::activation_relay_package(
            bytes,
            &f.scheme,
            &f.asset_digest,
            &f.credential,
            &account(0),
        )
    }

    fn mismatch<T>(result: Result<T>, expected: &str) {
        match result {
            Err(Error::AuthorityMismatch(message)) if message == expected => {}
            Err(e) => panic!("expected `{expected}`, got {e}"),
            Ok(_) => panic!("expected `{expected}`, accepted"),
        }
    }

    #[test]
    fn package_digest_is_kgwpkg_1_over_statement_kgwstep1_proof_and_receipt_object() {
        let f = fixture();
        let statement = words::<26>(&f.package.statement).unwrap();
        let profile = words::<64>(&f.package.profile).unwrap();
        let receipt = read_receipt(&f.package.receipt).unwrap();
        let mut framed = (f.package.step_proof.len() as u32).to_le_bytes().to_vec();
        framed.extend_from_slice(&f.package.step_proof);
        let proof =
            kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1, &framed);
        let receipt_digest = kagemusha_wallet_signed_object_digest_v1(
            KagemushaWalletObjectDigestDomainV1::Receipt,
            &crate::receipt::message(&receipt).to_repr(),
            &KagemushaDeviceSignatureV1::from_raw_bytes(&receipt.signature).unwrap(),
        );
        assert_eq!(receipt_digest, profile[full::RECEIPT_IO].to_repr());
        let statement_digest = crate::statement::digest(&statement).to_repr();
        assert_eq!(statement_digest, profile[full::STATEMENT_IO].to_repr());
        assert_eq!(
            kagemusha_wallet_package_digest_v1(&statement_digest, &proof, &receipt_digest).unwrap(),
            f.digest
        );
        // Every σ byte, its length, each statement word and the receipt move it.
        let digest = |s: &[Fp; 26], proof: &[u8], r: &bootstrap::Receipt| {
            package_digest(s, proof, r).unwrap().to_repr()
        };
        let mut longer = f.package.step_proof.clone();
        longer.push(0);
        let mut flipped = f.package.step_proof.clone();
        flipped[700] ^= 1;
        let mut moved = statement;
        moved[9] += Fp::ONE;
        let mut resigned = receipt.clone();
        resigned.signature = bootstrap::sign(
            crate::receipt::message(&receipt),
            &f.key.d,
            "Activation fixture second advance signature",
        );
        for other in [
            digest(&statement, &longer, &receipt),
            digest(&statement, &flipped, &receipt),
            digest(&moved, &f.package.step_proof, &receipt),
            digest(&statement, &f.package.step_proof, &resigned),
        ] {
            assert_ne!(other, f.digest);
        }
        assert_eq!(
            digest(&statement, &f.package.step_proof, &receipt),
            f.digest
        );
    }

    #[test]
    fn relay_returns_the_pinned_digest_of_the_exact_bootstrap_activation() {
        let f = fixture();
        let activate = control(f, &f.key, |_| {});
        let bytes = claim_with(f, &activate, |_| {});
        assert!(bytes.len() <= MAX_ACTIVATION_CLAIM_BYTES);
        assert_eq!(relay(f, &bytes).unwrap(), f.digest);
        let decoded = Claim::decode_original(&bytes).unwrap();
        assert_eq!(crate::native::encode(&decoded).unwrap(), bytes);
        // Another retained envelope of the same wallet and package activates
        // the same package under its own nonce.
        let again = claim_with(f, &control(f, &f.key, |b| b.nonce = [0x7a; 32]), |_| {});
        assert_eq!(relay(f, &again).unwrap(), f.digest);
        // The constructor's package encodes exactly this claim and refuses a
        // control it does not pin.
        let package = BootstrapActivationPackage {
            parameter_id: f.scheme.relation_id,
            scheme: f.scheme.scheme_id(),
            relation: f.scheme.relation_id,
            root: crate::native::point(f.scheme.scheme_root_key.as_ref()).unwrap(),
            asset_digest: f.asset_digest,
            credential: f.credential.clone(),
            issuer: f.issuer.clone(),
            asset: f.asset.clone(),
            package: f.package.clone(),
            digest: f.digest,
        };
        assert_eq!(package.authorize(&activate).unwrap(), bytes);
        let other = WalletKey::labelled("Activation alternate signer");
        mismatch(
            package.authorize(&control(f, &other, |_| {})),
            "Activate control payment-key signature",
        );
        mismatch(
            package.authorize(&control(f, &f.key, |b| {
                b.action = KagemushaWalletLedgerControlActionV1::Activate {
                    package_digest: Fp::from(77).to_repr(),
                }
            })),
            "Activate control package digest",
        );
        let oversized = BootstrapActivationPackage {
            parameter_id: package.parameter_id,
            scheme: package.scheme,
            relation: package.relation,
            root: package.root.clone(),
            asset_digest: package.asset_digest,
            credential: f.credential.clone(),
            issuer: f.issuer.clone(),
            asset: f.asset.clone(),
            package: Package {
                step_proof: vec![1; MAX_ACTIVATION_CLAIM_BYTES],
                ..f.package.clone()
            },
            digest: package.digest,
        };
        assert!(matches!(
            oversized.authorize(&activate),
            Err(Error::ResourceLimit)
        ));
    }

    #[test]
    fn relay_refuses_alternate_signer_account_asset_relation_wallet_and_control() {
        let f = fixture();
        let activate = control(f, &f.key, |_| {});
        let bytes = claim_with(f, &activate, |_| {});
        let other = WalletKey::labelled("Activation alternate signer");
        // Alternate signer of an otherwise exact control.
        mismatch(
            relay(f, &claim_with(f, &control(f, &other, |_| {}), |_| {})),
            "Activate control payment-key signature",
        );
        // Another submitting account.
        mismatch(
            NativeEngine::activation_relay_package(
                &bytes,
                &f.scheme,
                &f.asset_digest,
                &f.credential,
                &account(1),
            ),
            "Activation relay account",
        );
        // Another reserve asset, or another carried asset scope.
        let mut asset = f.asset_digest;
        asset[0] ^= 1;
        mismatch(
            NativeEngine::activation_relay_package(
                &bytes,
                &f.scheme,
                &asset,
                &f.credential,
                &account(0),
            ),
            "Activation scheme/asset",
        );
        let foreign_scope = norito::encode_canonical(&other_asset(&vector_asset())).unwrap();
        mismatch(
            relay(f, &claim_with(f, &activate, |c| c.asset = foreign_scope)),
            "Activation asset scope",
        );
        // Another relation, by claim or by installed scheme.
        mismatch(
            relay(f, &claim_with(f, &activate, |c| c.parameter_id[0] ^= 1)),
            "Activation parameter identity",
        );
        let mut relation = f.scheme.clone();
        relation.relation_id[0] ^= 1;
        mismatch(
            NativeEngine::activation_relay_package(
                &bytes,
                &relation,
                &f.asset_digest,
                &f.credential,
                &account(0),
            ),
            "Activation parameter identity",
        );
        // Another scheme root.
        let mut rooted = f.scheme.clone();
        let foreign_root = p256::mul(&p256::generator(), &BigUint::from(777u64)).unwrap();
        rooted.scheme_root_key =
            KagemushaDevicePublicKeyV1::try_from(sec1(&foreign_root).as_slice()).unwrap();
        assert!(matches!(
            NativeEngine::activation_relay_package(
                &bytes,
                &rooted,
                &f.asset_digest,
                &f.credential,
                &account(0),
            ),
            Err(Error::AuthorityMismatch(_))
        ));
        // Not the exact retained E6 credential.
        let mut changed = f.credential.clone();
        changed[595] ^= 1;
        mismatch(
            NativeEngine::activation_relay_package(
                &bytes,
                &f.scheme,
                &f.asset_digest,
                &changed,
                &account(0),
            ),
            "Activation relay credential original",
        );
        // A root-certified key of another role is not the Enrollment issuer.
        let issuer_key =
            p256::mul(&p256::generator(), &BigUint::from_bytes_be(&ISSUER_SECRET)).unwrap();
        let artifact = certificate::issue_fixture(
            &f.scheme.scheme_id(),
            certificate::Role::Artifact as u8,
            &issuer_key,
            1,
            &BigUint::from_bytes_be(&crate::native::engine::TEST_ROOT_SECRET),
        );
        assert!(matches!(
            relay(
                f,
                &claim_with(f, &activate, |c| c.issuer_certificate =
                    certificate_bytes(&artifact))
            ),
            Err(Error::AuthorityMismatch(_))
        ));
        // Substituted controls: another action, wallet, asset or package.
        let close = control(f, &f.key, |b| {
            b.action = KagemushaWalletLedgerControlActionV1::CloseLoads {
                package_digest: f.digest,
                next_load: 0,
            }
        });
        mismatch(
            relay(f, &claim_with(f, &close, |_| {})),
            "Activation requires an Activate control",
        );
        mismatch(
            relay(
                f,
                &claim_with(f, &control(f, &f.key, |b| b.wallet_id = [0x5a; 32]), |_| {}),
            ),
            "Activate control wallet scope",
        );
        mismatch(
            relay(
                f,
                &claim_with(f, &control(f, &f.key, |b| b.asset_digest = asset), |_| {}),
            ),
            "Activate control wallet scope",
        );
        mismatch(
            relay(
                f,
                &claim_with(
                    f,
                    &control(f, &f.key, |b| {
                        b.action = KagemushaWalletLedgerControlActionV1::Activate {
                            package_digest: Fp::from(77).to_repr(),
                        }
                    }),
                    |_| {},
                ),
            ),
            "Activate control package digest",
        );
        assert_eq!(relay(f, &bytes).unwrap(), f.digest);
    }

    #[test]
    fn relay_refuses_non_bootstrap_and_substituted_package_terms() {
        let f = fixture();
        let scheme = f.scheme.scheme_id();
        let activate = control(f, &f.key, |_| {});
        let other = WalletKey::labelled("Activation alternate signer");
        // Each edit is resealed under a fresh Activate for the edited package, so
        // only the package check itself can refuse it.
        let resealed = |edit: &dyn Fn(&mut Claim)| {
            claim_with(f, &activate, |c| {
                edit(c);
                reseal(c, &f.key, &scheme);
            })
        };
        mismatch(
            relay(
                f,
                &resealed(&|c: &mut Claim| set_word(&mut c.bootstrap.statement, 16, Fp::from(2))),
            ),
            "Activation requires a Bootstrap statement",
        );
        mismatch(
            relay(
                f,
                &resealed(&|c: &mut Claim| {
                    let moved = words::<26>(&c.bootstrap.statement).unwrap()[7] + Fp::ONE;
                    set_word(&mut c.bootstrap.statement, 7, moved);
                }),
            ),
            "Bootstrap statement credential scope",
        );
        mismatch(
            relay(f, &resealed(&|c: &mut Claim| c.bootstrap.receipt[120] ^= 1)),
            "Bootstrap advance receipt terms",
        );
        mismatch(
            relay(
                f,
                &resealed(&|c: &mut Claim| {
                    let mut receipt = read_receipt(&c.bootstrap.receipt).unwrap();
                    receipt.signature = bootstrap::sign(
                        crate::receipt::message(&receipt),
                        &other.d,
                        "Activation alternate advance signer",
                    );
                    c.bootstrap.receipt[bootstrap::RECEIPT_BYTES..]
                        .copy_from_slice(&receipt.signature);
                }),
            ),
            "Bootstrap advance receipt signature",
        );
        mismatch(
            relay(
                f,
                &resealed(&|c: &mut Claim| {
                    set_word(&mut c.bootstrap.profile, full::RECEIPT_IO, Fp::from(7))
                }),
            ),
            "Bootstrap package public inputs",
        );
        // The pinned digest names the exact σ: a substituted σ under the original
        // control is refused, and a resealed one pins another package. The
        // relay never verifies σ; the World verifier does.
        mismatch(
            relay(
                f,
                &claim_with(f, &activate, |c| c.bootstrap.step_proof[100] ^= 1),
            ),
            "Activate control package digest",
        );
        let mut substituted = None;
        let bytes = claim_with(f, &activate, |c| {
            c.bootstrap.step_proof[100] ^= 1;
            substituted = Some(reseal(c, &f.key, &scheme));
        });
        let substituted = substituted.unwrap();
        assert_ne!(substituted, f.digest);
        assert_eq!(relay(f, &bytes).unwrap(), substituted);
        assert!(matches!(
            relay(
                f,
                &claim_with(f, &activate, |c| c.bootstrap.step_proof.clear())
            ),
            Err(Error::Malformed("Bootstrap step proof"))
        ));
    }

    #[test]
    fn activation_originals_are_bounded_and_canonical() {
        let f = fixture();
        let activate = control(f, &f.key, |_| {});
        let bytes = claim_with(f, &activate, |_| {});
        let malformed = |result: Result<[u8; 32]>, case: &str| {
            assert!(matches!(result, Err(Error::Malformed(_))), "{case}");
        };
        // Frame-level refusals (their decoder error class is Norito's).
        let mut trailing = bytes.clone();
        trailing.push(0);
        let decoded = Claim::decode_original(&bytes).unwrap();
        let compact =
            norito::encode_canonical_with_layout(&decoded, norito::core::WireLayout::Compact)
                .unwrap();
        for (case, frame) in [
            ("trailing claim byte", trailing),
            ("truncated claim", bytes[..bytes.len() - 1].to_vec()),
            ("compact layout", compact),
            ("claim version", claim_with(f, &activate, |c| c.version = 2)),
        ] {
            assert!(relay(f, &frame).is_err(), "{case}");
        }
        assert!(matches!(
            relay(f, &vec![0; MAX_ACTIVATION_CLAIM_BYTES + 1]),
            Err(Error::ResourceLimit)
        ));
        // Exact fixed lengths and canonical words. A shortened credential is
        // also the E6 original the relay is given, so its length is what refuses.
        let short = f.credential[..f.credential.len() - 1].to_vec();
        malformed(
            NativeEngine::activation_relay_package(
                &claim_with(f, &activate, |c| c.credential = short.clone()),
                &f.scheme,
                &f.asset_digest,
                &short,
                &account(0),
            ),
            "credential length",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| {
                    c.issuer_certificate.pop();
                }),
            ),
            "issuer length",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| {
                    c.bootstrap.receipt.pop();
                }),
            ),
            "receipt length",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| {
                    c.bootstrap.statement.pop();
                }),
            ),
            "statement width",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| {
                    c.bootstrap.profile.pop();
                }),
            ),
            "profile width",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| c.bootstrap.statement[..32].fill(0xff)),
            ),
            "noncanonical statement word",
        );
        // Exact asset and control originals.
        malformed(
            relay(f, &claim_with(f, &activate, |c| c.asset.push(0))),
            "trailing asset byte",
        );
        malformed(
            relay(
                f,
                &claim_with(f, &activate, |c| {
                    c.asset = vec![0; ASSET_ORIGINAL_MAX_BYTES + 1]
                }),
            ),
            "oversize asset",
        );
        assert!(
            relay(f, &claim_with(f, &activate, |c| c.control.push(0))).is_err(),
            "trailing control byte"
        );
        // Constructor account and asset originals.
        let original = norito::encode_canonical(&account(0)).unwrap();
        assert_eq!(
            account_original_digest(&original).unwrap(),
            kagemusha_wallet_account_digest_v1(&account(0)).unwrap()
        );
        assert_eq!(
            &f.credential[98..130],
            &account_original_digest(&original).unwrap()
        );
        let mut extended = original.clone();
        extended.push(0);
        for (case, bytes) in [
            ("empty", Vec::new()),
            ("trailing", extended),
            ("truncated", original[..original.len() - 1].to_vec()),
            ("oversize", vec![0; ACCOUNT_ORIGINAL_MAX_BYTES + 1]),
        ] {
            assert!(
                matches!(account_original_digest(&bytes), Err(Error::Malformed(_))),
                "{case} account original"
            );
        }
        assert_eq!(
            asset_scope(&f.asset).unwrap().asset_digest(),
            f.asset_digest
        );
        assert!(asset_scope(&f.asset[..f.asset.len() - 1]).is_err());
        assert!(asset_scope(&[]).is_err());
    }

    /// Real released Bootstrap under the installed genesis-bound programs:
    /// constructor bindings, the World verifier's proof, profile and accumulator
    /// decisions, and agreement with the relay.
    #[test]
    #[ignore = "requires KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS: the genesis-bound signed program bundle; proves one Bootstrap"]
    fn installed_activation_binds_one_released_bootstrap_and_refuses_substitutions() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap(),
        );
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        let mut parameters = device_fixture::install(
            &read("program-bundle.norito"),
            &read("parameter-manifest.norito"),
            read("parameter-signature.bin").try_into().unwrap(),
        )
        .unwrap();
        let scheme = device_fixture::scheme();
        let scope = vector_asset();
        let asset = norito::encode_canonical(&scope).unwrap();
        let account = account(0);
        let account_original = norito::encode_canonical(&account).unwrap();
        let (key, boot, marker) =
            device_fixture::registered_bootstrap(&parameters, &scope, &account);
        let ExactOperationWitness::Bootstrap(step) = &boot else {
            unreachable!()
        };
        let credential = credential_bytes(&step.input.credential);
        let engine = NativeEngine::default();
        let origin = engine.enrollment_origin(step, marker, &parameters).unwrap();
        let prepared = engine.prepare(&boot, &origin, &parameters).unwrap();
        let capsule = prepared.capsule().clone();
        // A reissued manifest may keep the same relation label. The signed
        // capsule identifies the exact producer selected before a restart.
        let manifest = parameters.manifest_digest;
        parameters.manifest_digest[0] ^= 1;
        assert_eq!(capsule.parameters_id(), parameters.id());
        assert!(matches!(
            engine.prepare(&boot, &origin, &parameters),
            Err(Error::AuthorityMismatch("verified head parameter manifest"))
        ));
        assert!(matches!(
            engine.resume(&capsule, &parameters),
            Err(Error::AuthorityMismatch("recovery producer manifest"))
        ));
        assert!(matches!(
            engine.require_selected_debit(&prepared, *prepared.capsule_digest(), &parameters),
            Err(Error::AuthorityMismatch("prepared producer manifest"))
        ));
        parameters.manifest_digest = manifest;
        let resumed = engine.resume(&capsule, &parameters).unwrap();
        assert_eq!(
            resumed.capsule().encode().unwrap(),
            capsule.encode().unwrap()
        );
        let signature = bootstrap::sign(
            field(&prepared.signing_message()).unwrap(),
            &key.d,
            "installed Activation Bootstrap advance",
        );
        let selected =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signature)
                .unwrap();
        let released = engine.finalize(resumed, &selected, &parameters).unwrap();
        // A spendable predecessor is admitted through the same private head
        // capability as a normal wallet's first Load after Bootstrap.
        let first_load = device_fixture::load100(&boot).unwrap();
        let fresh_load = engine
            .prepare(&first_load, released.head(), &parameters)
            .unwrap();
        assert_eq!(fresh_load.capsule().kind(), 2);
        parameters.manifest_digest[0] ^= 1;
        assert!(matches!(
            engine.prepare(&first_load, released.head(), &parameters),
            Err(Error::AuthorityMismatch("verified head parameter manifest"))
        ));
        parameters.manifest_digest = manifest;
        let issuer = certificate_bytes(&device_fixture::enrollment_certificate());
        let build = |account: &[u8], asset: &[u8], issuer: &[u8]| {
            engine.bootstrap_activation_package(
                &capsule,
                released.completion(),
                account,
                asset,
                issuer,
                &parameters,
            )
        };
        // Constructor bindings.
        let foreign_account = norito::encode_canonical(&self::account(1)).unwrap();
        mismatch(
            build(&foreign_account, &asset, &issuer),
            "Activation account",
        );
        let foreign_asset = norito::encode_canonical(&other_asset(&scope)).unwrap();
        mismatch(
            build(&account_original, &foreign_asset, &issuer),
            "Activation asset scope",
        );
        let mut foreign_issuer = issuer.clone();
        foreign_issuer[34] = certificate::Role::Artifact as u8;
        mismatch(
            build(&account_original, &asset, &foreign_issuer),
            "Activation released Bootstrap originals",
        );
        let mut trailing = account_original.clone();
        trailing.push(0);
        assert!(matches!(
            build(&trailing, &asset, &issuer),
            Err(Error::Malformed(_))
        ));
        let package = build(&account_original, &asset, &issuer).unwrap();
        let again = build(&account_original, &asset, &issuer).unwrap();
        assert_eq!(package.digest(), again.digest(), "stable retained digest");
        let body = KagemushaWalletLedgerControlBodyV1 {
            version: 1,
            scheme_id: *capsule.scheme(),
            asset_digest: *capsule.asset(),
            wallet_id: *capsule.wallet(),
            action: KagemushaWalletLedgerControlActionV1::Activate {
                package_digest: *package.digest(),
            },
            nonce: [0x79; 32],
        };
        let claim = package.authorize(&signed(&key, body)).unwrap();
        // World verifier and relay agree on the exact claim.
        let verified = engine
            .verify_activation_claim(&claim, &parameters, capsule.scheme(), capsule.asset())
            .unwrap();
        assert_eq!(verified.wallet_id(), capsule.wallet());
        assert_eq!(
            verified.account_digest(),
            &kagemusha_wallet_account_digest_v1(&account).unwrap()
        );
        assert_eq!(verified.package_digest(), package.digest());
        assert_eq!(
            NativeEngine::activation_relay_package(
                &claim,
                &scheme,
                capsule.asset(),
                &credential,
                &account
            )
            .unwrap(),
            *package.digest()
        );
        // World refusals for another reserve.
        let mut other = *capsule.scheme();
        other[0] ^= 1;
        mismatch(
            engine.verify_activation_claim(&claim, &parameters, &other, capsule.asset()),
            "Activation reserve scheme/asset",
        );
        let mut other = *capsule.asset();
        other[0] ^= 1;
        mismatch(
            engine.verify_activation_claim(&claim, &parameters, capsule.scheme(), &other),
            "Activation scheme/asset",
        );
        // A resealed σ or a changed public input passes the relay's public terms
        // and is refused only by the World's proof decision.
        let scheme_id = scheme.scheme_id();
        let edits: [(&str, &dyn Fn(&mut Claim)); 2] = [
            ("σ byte", &|c: &mut Claim| {
                let at = c.bootstrap.step_proof.len() / 2;
                c.bootstrap.step_proof[at] ^= 1;
            }),
            ("lineage public input", &|c: &mut Claim| {
                let at =
                    crate::bootstrap_context::LINEAGE_IO + crate::bootstrap_context::BURNED_TOTAL;
                let moved = words::<64>(&c.bootstrap.profile).unwrap()[at] + Fp::ONE;
                set_word(&mut c.bootstrap.profile, at, moved);
            }),
        ];
        for (case, edit) in edits {
            let mut changed = Claim::decode_original(&claim).unwrap();
            edit(&mut changed);
            reseal(&mut changed, &key, &scheme_id);
            let bytes = crate::native::encode(&changed).unwrap();
            assert!(
                NativeEngine::activation_relay_package(
                    &bytes,
                    &scheme,
                    capsule.asset(),
                    &credential,
                    &account
                )
                .is_ok(),
                "{case}: relay public terms"
            );
            assert!(
                engine
                    .verify_activation_claim(&bytes, &parameters, capsule.scheme(), capsule.asset())
                    .is_err(),
                "{case}: World proof decision"
            );
        }
        let mut trailing = claim.clone();
        trailing.push(0);
        assert!(matches!(
            engine.verify_activation_claim(
                &trailing,
                &parameters,
                capsule.scheme(),
                capsule.asset()
            ),
            Err(Error::Malformed(_))
        ));
        if let Some(output) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
            use std::{fs, io::Write, path::PathBuf};
            let output = PathBuf::from(output);
            fs::create_dir(&output).unwrap();
            for (name, bytes) in [
                ("signed-activation-claim.norito", claim.as_slice()),
                ("activation-credential.bin", credential.as_slice()),
                ("activation-account.norito", account_original.as_slice()),
            ] {
                let mut file = fs::File::create_new(output.join(name)).unwrap();
                file.write_all(bytes).unwrap();
                file.sync_all().unwrap();
            }
            eprintln!(
                "Fixture registration asset={} incarnation={:02x?} scale={} account={}",
                scope.asset, scope.asset_incarnation, scope.scale, account,
            );
        }
        eprintln!(
            "verified {}-byte Bootstrap Activation; constructor, reserve, σ and public-input substitutions refused",
            claim.len()
        );
    }
}
