//! Future Unload claim intake. A verified claim is a proof/terms result for a
//! World-owned verifier. This module has no ledger, replay store or payout path.
use super::*;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KagemushaWalletChargeKindV1, KagemushaWalletUnloadChargeV1,
        kagemusha_wallet_account_digest_v1, kagemusha_wallet_unload_account_payout_v1,
    },
};

pub const MAX_UNLOAD_CLAIM_BYTES: usize = 16_384;

/// Exact payment destinations and the optional quoted charge's signer original.
/// The enrollment signer and credential are carried in the separately verified head.
#[derive(Clone, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.UnloadDestinations.v1")]
pub struct UnloadDestinations {
    pub account: AccountId,
    pub charge: KagemushaWalletUnloadChargeV1,
    pub quote_certificate: Vec<u8>,
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.UnloadClaim.v1")]
struct Claim {
    version: u16,
    parameter_id: [u8; 32],
    credential: Vec<u8>,
    issuer_certificate: Vec<u8>,
    profile: Vec<u8>,
    pair: Vec<u8>,
    statement: Vec<u8>,
    receipt: Vec<u8>,
    destinations: UnloadDestinations,
}

impl Claim {
    fn decode_original(bytes: &[u8]) -> core::result::Result<Self, norito::Error> {
        if bytes.len() > MAX_UNLOAD_CLAIM_BYTES {
            return Err(norito::Error::LengthMismatch);
        }
        let limits = norito::DecodeLimits::new(
            MAX_UNLOAD_CLAIM_BYTES,
            MAX_UNLOAD_CLAIM_BYTES,
            4 * MAX_UNLOAD_CLAIM_BYTES,
            8 * MAX_UNLOAD_CLAIM_BYTES,
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
        Error::Unavailable("local Unload decoder capacity")
    } else {
        Error::Malformed("canonical Unload claim")
    }
}

impl NativeEngine {
    /// Read the validated common enrollment fields of an exact 596-byte Spartan credential.
    /// Callers that need enrollment authority must restore a verified issuer result first.
    pub fn credential_original_fields(
        bytes: &[u8],
    ) -> Result<iroha_data_model::kagemusha::KagemushaWalletCredentialBodyV1> {
        read_credential(bytes)?.enrollment_fields()
    }

    /// Verify the public terms of an account-signed relay request against the exact protected
    /// enrollment result. The World independently verifies the proofs and settles the payout.
    pub fn unload_relay_nullifier(
        bytes: &[u8],
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        asset: &[u8; 32],
        credential_original: &[u8],
        account: &AccountId,
    ) -> Result<[u8; 32]> {
        let claim = Claim::decode_original(bytes).map_err(claim_error)?;
        if claim.parameter_id != scheme.relation_id
            || claim.credential != credential_original
            || &claim.destinations.account != account
        {
            return Err(Error::AuthorityMismatch("Unload relay originals"));
        }
        let credential = read_credential(&claim.credential)?;
        let fields = credential.enrollment_fields()?;
        let id = scheme.scheme_id();
        let root = super::super::point(scheme.scheme_root_key.as_ref())?;
        let issuer = read_certificate(&claim.issuer_certificate)?;
        certificate::authenticate_signer(&issuer, &id, &root, certificate::Role::Enrollment)
            .map_err(Error::AuthorityMismatch)?;
        if fields.scheme_id != id
            || &fields.asset_digest != asset
            || fields.issuer_certificate != certificate::certificate_digest(&issuer).to_repr()
        {
            return Err(Error::AuthorityMismatch("Unload relay issuer/scope"));
        }
        let statement = self::fields::<26>(&claim.statement)?;
        claim
            .destinations
            .verify(&credential, &statement, &id, &root)?;
        Ok(statement[17].to_repr())
    }

    /// Project an explicit signed transaction's public attempt for account-scoped finality reads.
    /// This result names the claimant and wallet. It grants no proof, payout or replay authority.
    /// A caller-local decoder refusal remains distinguishable from a malformed peer frame.
    pub fn unload_attempt_owner(
        bytes: &[u8],
        scheme: &[u8; 32],
        asset: &[u8; 32],
    ) -> core::result::Result<Option<([u8; 32], AccountId)>, norito::Error> {
        let claim = Claim::decode_original(bytes)?;
        let Ok(credential) = read_credential(&claim.credential) else {
            return Ok(None);
        };
        let wallet: [u8; 32] = credential.body[66..98]
            .try_into()
            .expect("fixed credential");
        if credential.body[2..34] != *scheme
            || credential.body[34..66] != *asset
            || wallet == [0; 32]
        {
            return Ok(None);
        }
        Ok(Some((wallet, claim.destinations.account)))
    }
}

/// Payout terms exposed only after the full proof pair and originals authenticate.
/// Core must use its installed World parameters before any replay or transfer.
pub struct VerifiedUnloadClaim {
    digest: [u8; 32],
    nullifier: [u8; 32],
    ordinal: u128,
    amount: u128,
    account_payout: u128,
    destinations: UnloadDestinations,
}
impl VerifiedUnloadClaim {
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn nullifier(&self) -> &[u8; 32] {
        &self.nullifier
    }
    pub fn ordinal(&self) -> u128 {
        self.ordinal
    }
    pub fn amount(&self) -> u128 {
        self.amount
    }
    pub fn account_payout(&self) -> u128 {
        self.account_payout
    }
    pub fn online_charge(&self) -> u128 {
        self.amount - self.account_payout
    }
    pub fn destinations(&self) -> &UnloadDestinations {
        &self.destinations
    }
}

fn model_error(e: impl std::fmt::Display) -> Error {
    Error::Rejected(e.to_string())
}

fn amount(field: Fp) -> Result<u128> {
    let bytes = field.to_repr();
    if bytes[16..].iter().any(|&x| x != 0) {
        return Err(Error::Malformed("Unload u128"));
    }
    Ok(u128::from_le_bytes(bytes[..16].try_into().unwrap()))
}

impl UnloadDestinations {
    /// Encode exact public payout terms for the wallet-owned Unload constructor.
    pub fn to_original(&self) -> Result<Vec<u8>> {
        let original = super::encode(self)?;
        if original.len() > MAX_UNLOAD_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        Ok(original)
    }

    pub(in crate::native) fn decode_bounded(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_UNLOAD_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        let limits = norito::DecodeLimits::new(
            MAX_UNLOAD_CLAIM_BYTES,
            MAX_UNLOAD_CLAIM_BYTES,
            4 * MAX_UNLOAD_CLAIM_BYTES,
            8 * MAX_UNLOAD_CLAIM_BYTES,
            16,
        );
        let value: Self = norito::core::with_decode_limits_scope(limits, || super::decode(bytes))?;
        if super::encode(&value)? != bytes {
            return Err(Error::Malformed("canonical Unload destinations"));
        }
        Ok(value)
    }

    pub(in crate::native) fn append_review(
        &self,
        step: &crate::control::Witness,
        capsule: &RecoveryCapsule,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let address = |id: &AccountId| {
            id.to_account_address()
                .and_then(|a| a.canonical_bytes())
                .map_err(model_error)
        };
        let account = address(&self.account)?;
        let (beneficiary, beneficiary_digest) = match &self.charge {
            KagemushaWalletUnloadChargeV1::None => (Vec::new(), [0; 32]),
            KagemushaWalletUnloadChargeV1::Quoted { beneficiary, quote } => {
                (address(beneficiary)?, quote.body.beneficiary_account_digest)
            }
        };
        let face = amount(step.statement[19])?;
        let charge = amount(step.statement[20])?;
        let net = kagemusha_wallet_unload_account_payout_v1(face, charge).map_err(model_error)?;
        out.extend_from_slice(&step.credential.body[34..66]);
        out.extend_from_slice(&step.statement[17].to_repr());
        for n in [amount(step.statement[18])?, face, charge, net] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&step.statement[21].to_repr());
        out.extend_from_slice(&step.credential.body[98..130]);
        out.extend_from_slice(&beneficiary_digest);
        out.extend_from_slice(capsule.predecessor());
        out.extend_from_slice(capsule.successor());
        if out.len() + 4 + account.len() + beneficiary.len() > MAX_UNLOAD_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        for bytes in [account, beneficiary] {
            out.extend_from_slice(
                &u16::try_from(bytes.len())
                    .map_err(|_| Error::ResourceLimit)?
                    .to_le_bytes(),
            );
            out.extend_from_slice(&bytes);
        }
        Ok(())
    }

    /// Check every quote and recipient binding before either Prepare or claim acceptance.
    /// The owned Unload constructor applies this same check before staging any map entry.
    pub(in crate::native) fn verify(
        &self,
        credential: &Credential,
        statement: &[Fp; 26],
        scheme: &[u8; 32],
        root: &p256::Point,
    ) -> Result<(u128, u128, u128)> {
        if statement[16] != Fp::from(6) || statement[17] == Fp::ZERO {
            return Err(Error::AuthorityMismatch("Unload statement kind/nullifier"));
        }
        let ordinal = amount(statement[18])?;
        let face = amount(statement[19])?;
        let charge = amount(statement[20])?;
        let net = kagemusha_wallet_unload_account_payout_v1(face, charge).map_err(model_error)?;
        let account = kagemusha_wallet_account_digest_v1(&self.account).map_err(model_error)?;
        if credential.body[2..34] != *scheme || credential.body[98..130] != account || face == 0 {
            return Err(Error::AuthorityMismatch(
                "Unload credential payout account/amount",
            ));
        }
        match &self.charge {
            KagemushaWalletUnloadChargeV1::None => {
                if charge != 0 || statement[21] != Fp::ZERO || !self.quote_certificate.is_empty() {
                    return Err(Error::AuthorityMismatch("Unload absent charge originals"));
                }
            }
            KagemushaWalletUnloadChargeV1::Quoted { quote, beneficiary } => {
                quote.validate().map_err(model_error)?;
                let body = &quote.body;
                let cert = read_certificate(&self.quote_certificate)?;
                let signer = certificate::authenticate_signer(
                    &cert,
                    scheme,
                    root,
                    certificate::Role::RegulatoryPolicy,
                )
                .map_err(Error::AuthorityMismatch)?;
                let beneficiary_digest =
                    kagemusha_wallet_account_digest_v1(beneficiary).map_err(model_error)?;
                if body.scheme_id != *scheme
                    || body.asset_digest != credential.body[34..66]
                    || body.signer_certificate != certificate::certificate_digest(&cert).to_repr()
                    || beneficiary_digest != body.beneficiary_account_digest
                    || quote.charge_quote_digest() != statement[21].to_repr()
                    || statement[21] == Fp::ZERO
                {
                    return Err(Error::AuthorityMismatch("Unload signed quote scope"));
                }
                quote
                    .require_terms(
                        KagemushaWalletChargeKindV1::Unload,
                        credential.body[66..98].try_into().unwrap(),
                        ordinal,
                        face,
                        charge,
                    )
                    .map_err(model_error)?;
                verify_signature(
                    &signer,
                    field(&body.signing_message())?,
                    quote.signature.as_ref().try_into().unwrap(),
                )?;
            }
        }
        Ok((ordinal, face, net))
    }
}

fn fields<const N: usize>(bytes: &[u8]) -> Result<[Fp; N]> {
    if bytes.len() != 32 * N {
        return Err(Error::Malformed("Unload field count"));
    }
    let mut values = [Fp::ZERO; N];
    for (slot, word) in values.iter_mut().zip(bytes.chunks_exact(32)) {
        *slot = field(word.try_into().unwrap())?;
    }
    Ok(values)
}

impl NativeEngine {
    /// Encode exact completed originals for independent proof-before-payout intake.
    /// An unchanged completion can reproduce this exact claim without proving again.
    pub fn unload_claim(
        &self,
        head: &VerifiedWalletHead,
        statement: &[[u8; 32]; 26],
        receipt: &[u8],
        destinations: UnloadDestinations,
        parameters: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        if receipt.len() != 402 || !matches!(destinations.quote_certificate.len(), 0 | 172) {
            return Err(Error::Malformed("Unload receipt/certificate lengths"));
        }
        head.require_parameters(parameters)?;
        let frame: HeadFrame = super::decode(head.exact_pair())?;
        if frame.kind as usize != full::CONTROL {
            return Err(Error::AuthorityMismatch("Unload head program"));
        }
        let bytes = super::encode(&Claim {
            version: 1,
            parameter_id: *parameters.id(),
            credential: frame.credential,
            issuer_certificate: frame.certificate,
            profile: frame.profile.concat(),
            pair: frame.pair,
            statement: statement.concat(),
            receipt: receipt.to_vec(),
            destinations,
        })?;
        self.verify_unload_claim(
            &bytes,
            parameters,
            head.credential.body[2..34].try_into().unwrap(),
            head.credential.body[34..66].try_into().unwrap(),
        )?;
        Ok(bytes)
    }

    /// Authenticate the bounded claim for an independently selected scheme and asset.
    /// A ledger caller obtains these from the reserve registration and mounts the
    /// parameters from its own installation before consulting any replay record.
    pub fn verify_unload_claim(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
        expected_scheme: &[u8; 32],
        expected_asset: &[u8; 32],
    ) -> Result<VerifiedUnloadClaim> {
        if bytes.len() > MAX_UNLOAD_CLAIM_BYTES {
            return Err(Error::ResourceLimit);
        }
        let claim = Claim::decode_original(bytes).map_err(claim_error)?;
        scope(parameters, &claim.parameter_id)?;
        let p = &parameters.inner;
        let c = read_credential(&claim.credential)?;
        if &p.scheme != expected_scheme || c.body[34..66] != *expected_asset {
            return Err(Error::AuthorityMismatch("Unload reserve scheme/asset"));
        }
        authenticate_credential(p, &c, &read_certificate(&claim.issuer_certificate)?)?;
        let profile = fields::<64>(&claim.profile)?;
        let statement = fields::<26>(&claim.statement)?;
        if claim.receipt.len() != 402 {
            return Err(Error::Malformed("Unload own receipt"));
        }
        let receipt = bootstrap::Receipt {
            body: claim.receipt[..338].try_into().unwrap(),
            signature: claim.receipt[338..].try_into().unwrap(),
        };
        profile_matches(p, &profile, &c)?;
        if profile[crate::control::MODE_IO..crate::control::MODE_IO + 2] != [Fp::ONE, Fp::ZERO]
            || statement[16] != Fp::from(6)
            || profile[full::HEAD_IO] != statement[15]
            || profile[full::STATEMENT_IO] != crate::statement::digest(&statement)
            || profile[full::RECEIPT_IO] != crate::receipt::digest(&receipt)
        {
            return Err(Error::AuthorityMismatch("Unload proof originals"));
        }
        let projected = projection(&profile, full::CONTROL)?;
        let pair =
            carrier::decode_projected_pair(&p.profile(full::CONTROL)?, &claim.pair, &projected)
                .map_err(Error::Rejected)?;
        verify_pair(p, full::CONTROL, pair)?;
        let (ordinal, face, net) = claim
            .destinations
            .verify(&c, &statement, &p.scheme, &p.root)?;
        Ok(VerifiedUnloadClaim {
            digest: crate::payment::byte_digest(u64::from_le_bytes(*b"kgwuclm1"), bytes).to_repr(),
            nullifier: statement[17].to_repr(),
            ordinal,
            amount: face,
            account_payout: net,
            destinations: claim.destinations,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KagemushaDeviceSignatureV1, KagemushaWalletChargeQuoteBodyV1, KagemushaWalletChargeQuoteV1,
    };

    #[test]
    fn unload_attempt_projection_uses_canonical_original_and_credential_scope() {
        let account = AccountId::new(
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C"
                .parse()
                .unwrap(),
        );
        let mut credential = vec![0; 596];
        credential[2..34].fill(1);
        credential[34..66].fill(2);
        credential[66..98].fill(3);
        let claim = Claim {
            version: 1,
            parameter_id: [7; 32],
            credential,
            issuer_certificate: vec![],
            profile: vec![],
            pair: vec![],
            statement: vec![],
            receipt: vec![],
            destinations: UnloadDestinations {
                account: account.clone(),
                charge: KagemushaWalletUnloadChargeV1::None,
                quote_certificate: vec![],
            },
        };
        let bytes = super::super::encode(&claim).unwrap();
        let project = |frame: &[u8], scheme, asset| {
            NativeEngine::unload_attempt_owner(frame, &scheme, &asset)
        };
        assert_eq!(
            project(&bytes, [1; 32], [2; 32]).unwrap(),
            Some(([3; 32], account))
        );
        assert_eq!(project(&bytes, [9; 32], [2; 32]).unwrap(), None);
        assert_eq!(project(&bytes, [1; 32], [9; 32]).unwrap(), None);
        assert!(project(&bytes[..bytes.len() - 1], [1; 32], [2; 32]).is_err());
        let other = norito::encode_canonical_with_layout(&claim, norito::core::WireLayout::Compact)
            .unwrap();
        assert!(project(&other, [1; 32], [2; 32]).is_err());
    }

    fn quoted(dest: AccountId, credential: &Credential, ordinal: u128) -> UnloadDestinations {
        let beneficiary = AccountId::new(
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C"
                .parse()
                .unwrap(),
        );
        let scheme = credential.body[2..34].try_into().unwrap();
        let secret = BigUint::from(0x37u8);
        let key = p256::mul(&p256::generator(), &secret).unwrap();
        let cert = certificate::issue_fixture(
            &scheme,
            certificate::Role::RegulatoryPolicy as u8,
            &key,
            18,
            &BigUint::from_bytes_be(&TEST_ROOT_SECRET),
        );
        let body = KagemushaWalletChargeQuoteBodyV1 {
            version: 1,
            scheme_id: scheme,
            asset_digest: credential.body[34..66].try_into().unwrap(),
            wallet_id: credential.body[66..98].try_into().unwrap(),
            kind: KagemushaWalletChargeKindV1::Unload,
            ordinal,
            net_amount: 4,
            online_charge: 1,
            beneficiary_account_digest: kagemusha_wallet_account_digest_v1(&beneficiary).unwrap(),
            issued_at_ms: 1_763_400_000_000,
            signer_certificate: certificate::certificate_digest(&cert).to_repr(),
        };
        let signature = bootstrap::sign(
            field(&body.signing_message()).unwrap(),
            &secret,
            "Unload fixture RegulatoryPolicy",
        );
        UnloadDestinations {
            account: dest,
            charge: KagemushaWalletUnloadChargeV1::Quoted {
                quote: KagemushaWalletChargeQuoteV1 {
                    body,
                    signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature).unwrap(),
                },
                beneficiary,
            },
            quote_certificate: certificate_bytes(&cert),
        }
    }

    fn refused<T>(result: Result<T>, case: &str) {
        assert!(result.is_err(), "{case}");
    }

    #[test]
    fn unload_signed_account_quote_and_refusals() {
        let account = AccountId::new(
            "ed0120D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A"
                .parse()
                .unwrap(),
        );
        let key = crate::full::WalletKey::labelled("Unload quote authority test");
        let (ctx, input) = bootstrap::fixture(&key, &Gens::<Eq>::new(4096));
        let mut credential = input.credential;
        credential.body[98..130]
            .copy_from_slice(&kagemusha_wallet_account_digest_v1(&account).unwrap());
        let dest = quoted(account.clone(), &credential, 0);
        let mut st = [Fp::ZERO; 26];
        st[16] = Fp::from(6);
        st[17] = Fp::ONE;
        st[19] = Fp::from(4);
        st[20] = Fp::ONE;
        st[21] = field(&dest.charge.quote().unwrap().charge_quote_digest()).unwrap();
        let verify = |d: &UnloadDestinations, s: &[Fp; 26]| {
            d.verify(&credential, s, &ctx.root.scheme, &ctx.root.root)
        };
        assert_eq!(verify(&dest, &st).unwrap(), (0, 4, 3));
        for field in [16, 17, 18, 19, 20, 21] {
            let mut changed = st;
            if field == 17 {
                changed[field] = Fp::ZERO;
            } else {
                changed[field] += Fp::ONE;
            }
            refused(verify(&dest, &changed), "altered quote terms");
        }
        let mut changed = dest.clone();
        let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } = &mut changed.charge else {
            unreachable!()
        };
        std::mem::swap(&mut changed.account, beneficiary);
        refused(verify(&changed, &st), "substituted payout account");
        let mut changed = dest.clone();
        let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } = &mut changed.charge else {
            unreachable!()
        };
        *beneficiary = account.clone();
        refused(verify(&changed, &st), "substituted beneficiary");
        let mut changed = dest.clone();
        changed.quote_certificate[34] = certificate::Role::TimeAnchor as u8;
        refused(verify(&changed, &st), "wrong signer role");
        let mut changed = dest.clone();
        changed.quote_certificate[115] ^= 1;
        refused(verify(&changed, &st), "wrong root signature");
        let mut changed = dest.clone();
        let KagemushaWalletUnloadChargeV1::Quoted { quote, .. } = &mut changed.charge else {
            unreachable!()
        };
        quote.body.issued_at_ms += 1;
        let mut altered = st;
        altered[21] = field(&quote.charge_quote_digest()).unwrap();
        refused(verify(&changed, &altered), "changed signed quote");
        let mut no_fee = UnloadDestinations {
            account,
            charge: KagemushaWalletUnloadChargeV1::None,
            quote_certificate: vec![],
        };
        refused(verify(&no_fee, &st), "missing charged quote");
        st[20] = Fp::ZERO;
        st[21] = Fp::ZERO;
        assert_eq!(verify(&no_fee, &st).unwrap(), (0, 4, 4));
        no_fee.quote_certificate = dest.quote_certificate;
        refused(verify(&no_fee, &st), "spurious certificate");
    }

    pub(crate) fn test_unload_claim(
        parameters: &AuthenticatedParameters,
        before: &VerifiedWalletHead,
        refresh: &crate::control::Witness,
        load: &crate::load::Witness,
        key: &crate::full::WalletKey,
        account: AccountId,
    ) {
        let engine = NativeEngine::isolated_unload_fixture();
        let dest = quoted(
            account,
            &refresh.credential,
            amount(refresh.after_core[14]).unwrap(),
        );
        let digest = field(&dest.charge.quote().unwrap().charge_quote_digest()).unwrap();
        let step = crate::unload::tests::recursive_unload_from(
            refresh,
            *before.profile(),
            key,
            Some(load),
            digest,
        );
        let control = crate::control::Witness::unload(step);
        let op = ExactOperationWitness::Unload {
            step: Box::new(control.clone()),
            destinations: Box::new(dest.clone()),
        };
        let mut redirected = dest.clone();
        if let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } = &mut redirected.charge {
            *beneficiary = dest.account.clone();
        }
        let wrong = ExactOperationWitness::Unload {
            step: Box::new(control.clone()),
            destinations: Box::new(redirected),
        };
        assert!(engine.prepare(&wrong, before, parameters).is_err());
        let prepared = engine.prepare(&op, before, parameters).unwrap();
        let review = prepared.review_bytes().unwrap();
        assert_eq!(&review[..3], &[3, 0, 6]);
        assert_eq!(&review[211..227], &4u128.to_le_bytes());
        assert_eq!(&review[227..243], &1u128.to_le_bytes());
        assert_eq!(&review[243..259], &3u128.to_le_bytes());
        assert_eq!(&review[259..291], &digest.to_repr());
        assert_eq!(&review[291..323], &control.credential.body[98..130]);
        assert_eq!(
            &review[323..355],
            &dest.charge.quote().unwrap().body.beneficiary_account_digest
        );
        let capsule = prepared.capsule().clone();
        assert_eq!(
            NativeEngine::default()
                .resume(&capsule, parameters)
                .unwrap()
                .review_bytes()
                .unwrap(),
            review
        );
        let signature = bootstrap::sign(
            field(&prepared.signing_message()).unwrap(),
            &key.d,
            "isolated confirmed charged Unload",
        );
        let selected =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signature)
                .unwrap();
        engine.before_selection(&capsule, parameters).unwrap();
        let completed = engine.finalize(prepared, &selected, parameters).unwrap();
        let head = completed.head();
        let receipt = completed.completion().exact_receipt();
        let st = control.statement.map(|f| f.to_repr());
        let bytes = completed.completion().output_part(3).unwrap();
        let (opening, restored) = NativeEngine::default()
            .verify_released_operation(&capsule, completed.completion(), parameters)
            .unwrap();
        let (gross, burned, spendable) = restored.balance(&opening).unwrap();
        assert_eq!(Fp::from_u128(gross), control.after_core[9]);
        assert_eq!(
            Fp::from_u128(burned),
            restored.head().profile()
                [crate::bootstrap_context::LINEAGE_IO + crate::bootstrap_context::BURNED_TOTAL]
        );
        assert_eq!(gross - burned, spendable);
        assert_eq!(restored.completion().output_part(3).unwrap(), bytes);
        assert!(restored.completion().output_part(1).unwrap().is_empty());
        let verify = |wire: &[u8]| {
            engine.verify_unload_claim(wire, parameters, capsule.scheme(), capsule.asset())
        };
        let accepted = verify(&bytes).unwrap();
        let mut other_asset = *capsule.asset();
        other_asset[0] ^= 1;
        let mut other_scheme = *capsule.scheme();
        other_scheme[0] ^= 1;
        for (scheme, asset) in [
            (&other_scheme, capsule.asset()),
            (capsule.scheme(), &other_asset),
        ] {
            assert!(matches!(
                engine.verify_unload_claim(&bytes, parameters, scheme, asset),
                Err(Error::AuthorityMismatch("Unload reserve scheme/asset"))
            ));
        }
        assert_eq!(
            (
                accepted.ordinal(),
                accepted.amount(),
                accepted.account_payout(),
                accepted.online_charge()
            ),
            (0, 4, 3, 1)
        );
        assert_eq!(accepted.nullifier(), &st[17]);
        assert_eq!(control.after_core[9], Fp::from(96));
        assert_eq!(accepted.destinations().account, dest.account);
        assert_eq!(
            engine
                .unload_claim(&head, &st, &receipt, dest, parameters)
                .unwrap(),
            bytes
        );
        let tamper = |apply: &dyn Fn(&mut Claim)| {
            let mut changed: Claim = super::super::decode(&bytes).unwrap();
            apply(&mut changed);
            let encoded = super::super::encode(&changed).unwrap();
            refused(verify(&encoded), "altered completed claim");
        };
        tamper(&|c| c.pair[100] ^= 1);
        tamper(&|c| {
            let at = c.pair.len() - 35;
            c.pair[at] ^= 1;
        });
        tamper(&|c| c.receipt[339] ^= 1);
        tamper(&|c| c.statement[19 * 32] ^= 1);
        tamper(&|c| c.issuer_certificate[110] ^= 1);
        tamper(&|c| c.profile[62 * 32] = 0);
        tamper(&|c| c.parameter_id[0] ^= 1);
        tamper(&|c| c.destinations.quote_certificate[34] = 4);
        tamper(&|c| {
            if let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } =
                &mut c.destinations.charge
            {
                *beneficiary = c.destinations.account.clone();
            }
        });
        let mut trailing = bytes.clone();
        trailing.push(0);
        refused(verify(&trailing), "trailing claim bytes");
        refused(
            verify(&vec![0; MAX_UNLOAD_CLAIM_BYTES + 1]),
            "oversize claim",
        );
        let replay = verify(&bytes).unwrap();
        assert_eq!(accepted.digest(), replay.digest());
        if let Some(directory) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
            let path = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("signed-unload-claim.norito"), &bytes).unwrap();
            std::fs::write(
                path.join("charged-enrollment-credential.bin"),
                credential_bytes(&control.credential),
            )
            .unwrap();
            std::fs::write(path.join("charged-unload-review.bin"), &review).unwrap();
        }
        let pair = decode_head(head.exact_pair(), parameters).unwrap().4;
        eprintln!(
            "verified charged Unload claim {} bytes; actual P {} B S {} B, {} debited, {} account payout, {} beneficiary payout, 11 canonical/proof/authority refusals and identical claim digest on retry",
            bytes.len(),
            pair.pp.to_bytes().len(),
            pair.ps.to_bytes().len(),
            accepted.amount(),
            accepted.account_payout(),
            accepted.online_charge()
        );
    }
}
