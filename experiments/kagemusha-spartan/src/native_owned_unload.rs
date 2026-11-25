//! Production owned Unload constructor for a wallet with normal software custody.
//!
//! `NativeEngine::unload_from_released` derives the exact unsigned Control-mode
//! Unload that follows the wallet's released preparation. It does not accept a
//! private witness, a signing key or an issuer: the predecessor opening comes
//! from `released`, checked against its verified head; the face amount is the
//! user's choice; the payout account and optional charge come from one exact
//! canonical `UnloadDestinations` original; and the wallet-owned map store
//! supplies only the recovery-map opening, which is verified here.
//!
//! Authentication precedes every effect. The payout account must be the one
//! the credential names, and a charge must be a RegulatoryPolicy-signed quote
//! for this wallet, its next redemption ordinal, this face amount and the
//! carried beneficiary (`UnloadDestinations::verify`, the check
//! `Engine::prepare` and claim acceptance also apply). Only then is fresh state
//! entropy sampled and the store asked to stage the redemption entry.
//!
//! The successor follows `unload::append_branch`: it debits exactly the face
//! amount from the lineage-adjusted spendable value (`balance - burned_total`
//! of the verified head, never the private Core burn), advances the sequence
//! and next redemption ordinal, materializes the head's lineage burn and
//! pending-outgoing root, inserts `(Redeem, ordinal)` into the load/redeem
//! recovery map and takes a fresh nonzero nonce. The online charge is withheld
//! from the online payout and never debited offline.
//!
//! The operation identity is `P(kgwopid1, wallet, 6, nullifier)` with the
//! nullifier `P(kgwnull1, scheme, wallet, ordinal)`. It is durable across fresh
//! blinding and identical for every Unload at one ordinal, so custody selects at
//! most one of them.
//!
//! The returned witness confers no authority. `Engine::prepare` checks the
//! destinations and the complete Control relation again, an installed wallet
//! needs the protected Unload review confirmation at both debit boundaries, and
//! the canonical Advance provider selects, obtains the hardware signature and
//! releases. The Control Unload mode has no time or quota inputs.
use super::funding::{fresh_state_nonce, insert_witness, uint, unsigned_receipt};
use super::*;
use crate::{
    bootstrap_context as context, credential, full, indexed_map, monetary_state,
    payment_wire::limbs, statement::Kind, unload,
};
use ff::{Field, PrimeField};
use iroha_data_model::kagemusha::{KagemushaWalletIndexedInsertV1, KagemushaWalletUnloadChargeV1};
use iroha_pasta::poseidon;

/// The installed trust the constructor reads. Production borrows it from
/// authenticated parameters only.
#[derive(Clone, Copy)]
struct Installed<'a> {
    id: &'a [u8; 32],
    manifest_digest: &'a [u8; 32],
    scheme: &'a [u8; 32],
    relation: &'a [u8; 32],
    root: &'a p256::Point,
}
impl<'a> Installed<'a> {
    fn of(parameters: &'a AuthenticatedParameters) -> Self {
        let inner = &parameters.inner;
        Self {
            id: &parameters.id,
            manifest_digest: &parameters.manifest_digest,
            scheme: &inner.scheme,
            relation: &inner.relation,
            root: &inner.root,
        }
    }
}

impl NativeEngine {
    /// Exact unsigned Unload of `face` that follows `released`.
    ///
    /// `released` is the frozen preparation of the wallet's current released
    /// operation, of any kind, and `head` the [`VerifiedWalletHead`] it
    /// released. The predecessor state is `released`'s own successor opening,
    /// checked against `head`'s public head, lineage and credential. A head
    /// already bound to custody must be bound to exactly this preparation's
    /// capsule.
    ///
    /// `face` is the offline value debited. `destinations_original` is the
    /// exact canonical [`UnloadDestinations`] encoding
    /// ([`UnloadDestinations::to_original`]): the payout account, which must be
    /// the credential's, and either no charge or a signed Unload quote for
    /// this wallet, its next redemption ordinal, exactly `face` and the carried
    /// beneficiary. The account payout is `face` minus the quoted charge.
    ///
    /// `recovery` is asked once, only after every authentication and
    /// arithmetic check has passed and fresh entropy is available, for the
    /// insertion of this redemption into the wallet-owned load/redeem recovery
    /// map. Its arguments are the committed recovery root, the map key and the
    /// leaf value, each a canonical field encoding derived here. A
    /// [`SpartanWalletMaps`] store answers it by staging
    /// `insert(SpartanWalletMap::Recovery, root, key, value)`. The answer is
    /// verified against that committed root before anything is returned.
    ///
    /// # Errors
    /// Rejects a foreign parameter scope or producer manifest, a head that is
    /// not `released`'s, a wrong opening or lifecycle, a non-canonical or
    /// oversized destinations original, a payout account the credential does
    /// not name, an unauthenticated, foreign, stale or mispriced quote, a zero
    /// face, a face above the lineage-adjusted spendable value, counter
    /// overflow, unavailable OS entropy and a wrong map insertion. An error of
    /// `recovery` is returned unchanged.
    pub fn unload_from_released<R>(
        &self,
        released: &PreparedOperation,
        head: &VerifiedWalletHead,
        face: u128,
        destinations_original: &[u8],
        recovery: R,
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness>
    where
        R: FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1>,
    {
        unload_witness(
            &Installed::of(parameters),
            released,
            head,
            face,
            destinations_original,
            recovery,
            |previous| fresh_state_nonce(Some(previous)),
        )
    }
}

/// The Unload that follows `released`. `nonce` receives the predecessor nonce
/// and returns the successor's; production samples the operating system.
fn unload_witness<R, N>(
    installed: &Installed<'_>,
    released: &PreparedOperation,
    head: &VerifiedWalletHead,
    face: u128,
    destinations_original: &[u8],
    recovery: R,
    nonce: N,
) -> Result<ExactOperationWitness>
where
    R: FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1>,
    N: FnOnce(Fp) -> Result<Fp>,
{
    // Installed scope of both capabilities.
    if &released.parameters_id != installed.id || &head.parameters_id != installed.id {
        return Err(Error::AuthorityMismatch("Unload parameter identity"));
    }
    if &released.manifest_digest != installed.manifest_digest
        || &head.manifest_digest != installed.manifest_digest
    {
        return Err(Error::AuthorityMismatch("released producer manifest"));
    }
    // `head` is a released recursive head, never an enrollment origin, and
    // custody already bound to it is this preparation's.
    if head.enrollment_marker.is_some()
        || head.pair.is_empty()
        || (head.custody_digest != [0; 32] && &head.custody_digest != released.capsule_digest())
    {
        return Err(Error::AuthorityMismatch("released predecessor custody"));
    }
    let c = released.credential();
    if c.body != head.credential.body
        || c.signature != head.credential.signature
        || c.body[2..34] != installed.scheme[..]
    {
        return Err(Error::AuthorityMismatch("released credential"));
    }
    // The private opening is the one the verified public head commits to.
    let (core, rest) = released.successor_opening();
    let before = monetary_state::commitment(core, rest);
    let lineage = &head.profile[context::LINEAGE_IO..context::LINEAGE_IO + context::LINEAGE_FIELDS];
    if before != head.profile[full::HEAD_IO]
        || before.to_repr() != *released.capsule.successor()
        || lineage[context::LIFECYCLE] != core[0]
        || lineage[context::POLICY_EPOCH] != core[27]
        || lineage[context::ENABLED_CONTROLS] != core[22]
        || core[1..3] != limbs(&c.body[2..34])
        || core[3..5] != limbs(&c.body[34..66])
        || core[5..7] != limbs(&c.body[66..98])
        || core[7] != credential::digest(c)
    {
        return Err(Error::AuthorityMismatch("released successor opening"));
    }
    // Active (1) or retiring (2) wallets redeem; two enabled-control bits.
    if core[0] != Fp::ONE && core[0] != Fp::from(2) {
        return Err(Error::AuthorityMismatch("Unload lifecycle"));
    }
    if uint(core[22])? > 3 {
        return Err(Error::Malformed("Unload enabled controls"));
    }
    // The verified lineage, not the private Core, fixes burn and pending root.
    let burned_word = lineage[context::BURNED_TOTAL];
    let pending = lineage[context::PENDING_ROOT];
    let burned = uint(burned_word)?;
    if pending == Fp::ZERO {
        return Err(Error::AuthorityMismatch("Unload lineage pending root"));
    }
    let destinations = UnloadDestinations::decode_bounded(destinations_original)?;
    // Checked integers: spendable value, counters and the redemption ordinal.
    if face == 0 {
        return Err(Error::AuthorityMismatch("Unload zero face amount"));
    }
    let balance = uint(core[9])?;
    let spendable = balance
        .checked_sub(burned)
        .ok_or(Error::Malformed("released balance below lineage burn"))?;
    if face > spendable {
        return Err(Error::AuthorityMismatch(
            "Unload insufficient spendable value",
        ));
    }
    let after_balance = balance - face;
    let sequence = uint(core[11])?
        .checked_add(1)
        .ok_or(Error::Malformed("Unload sequence overflow"))?;
    uint(core[13])?;
    let ordinal = uint(core[14])?;
    let following = ordinal
        .checked_add(1)
        .ok_or(Error::Malformed("Unload redemption ordinal overflow"))?;
    // The retained quote must price exactly the next redemption.
    let (charge, quote_digest) = match &destinations.charge {
        KagemushaWalletUnloadChargeV1::None => (0, Fp::ZERO),
        KagemushaWalletUnloadChargeV1::Quoted { quote, .. } => {
            if quote.body.ordinal < ordinal {
                return Err(Error::AuthorityMismatch(
                    "Unload quote ordinal already redeemed",
                ));
            }
            if quote.body.ordinal > ordinal {
                return Err(Error::AuthorityMismatch(
                    "Unload quote ordinal is ahead of the next redemption",
                ));
            }
            (
                quote.body.online_charge,
                field(&quote.charge_quote_digest())?,
            )
        }
    };
    let ordinal_word = Fp::from_u128(ordinal);
    let nullifier = unload::nullifier([core[1], core[2]], [core[5], core[6]], ordinal_word);
    let mut terms = zeroize::Zeroizing::new([Fp::ZERO; 26]);
    terms[16] = Fp::from(Kind::Unload.tag());
    terms[17] = nullifier;
    terms[18] = ordinal_word;
    terms[19] = Fp::from_u128(face);
    terms[20] = Fp::from_u128(charge);
    terms[21] = quote_digest;
    // Signed payout account and charge, before any entropy or map effect.
    destinations.verify(c, &terms, installed.scheme, installed.root)?;
    let nonce = nonce(core[29])?;
    if nonce == Fp::ZERO || nonce == core[29] {
        return Err(Error::Unavailable("fresh state nonce"));
    }
    // Redemption entry `(Redeem, ordinal)` against the committed root.
    let key = unload::recovery_key(ordinal_word);
    let value = unload::recovery_value(ordinal_word, nullifier, terms[19], terms[20]);
    let root = core[19];
    let (root_bytes, key_bytes, value_bytes) = (root.to_repr(), key.to_repr(), value.to_repr());
    let insertion = recovery(&root_bytes, &key_bytes, &value_bytes)?;
    let inserted = insertion
        .verify(&root_bytes, &key_bytes, &value_bytes)
        .map_err(|_| Error::AuthorityMismatch("Unload recovery insertion"))?;
    let new_root = field(&inserted)?;
    let opening = insert_witness(&insertion, root, key, value)?;
    let leaf = indexed_map::Leaf {
        key,
        value,
        next: opening.low.next,
    };
    if indexed_map::path_root(indexed_map::leaf_hash(leaf), &opening.slot) != new_root {
        return Err(Error::AuthorityMismatch("Unload recovery insertion"));
    }
    let mut after = zeroize::Zeroizing::new(*core);
    after[9] = Fp::from_u128(after_balance);
    after[10] = burned_word;
    after[11] = Fp::from_u128(sequence);
    after[14] = Fp::from_u128(following);
    after[18] = pending;
    after[19] = new_root;
    after[29] = nonce;
    terms[0] = Fp::ONE;
    terms[1..3].copy_from_slice(&limbs(installed.relation));
    terms[3..7].copy_from_slice(&after[1..5]);
    terms[7] = after[7];
    terms[8] = after[0];
    terms[9] = after[11];
    terms[10] = after[13];
    terms[11] = core[22];
    terms[12] = burned_word;
    terms[13] = pending;
    terms[14] = before;
    terms[15] = monetary_state::commitment(&after, rest);
    let operation = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[after[5], after[6], terms[16], terms[17]],
    );
    let step = crate::control::Witness::unload(unload::Witness {
        before_core: *core,
        before_rest: *rest,
        after_core: *after,
        after_rest: *rest,
        credential: c.clone(),
        statement: *terms,
        receipt: unsigned_receipt(c, &terms, operation),
        recovery: opening,
    });
    Ok(ExactOperationWitness::Unload {
        step: Box::new(step),
        destinations: Box::new(destinations),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bootstrap, certificate, full::WalletKey, ipa::Gens, operation_auth::tests as harness,
    };
    use iroha_data_model::{
        account::AccountId,
        kagemusha::{
            KagemushaDeviceSignatureV1, KagemushaWalletChargeKindV1,
            KagemushaWalletChargeQuoteBodyV1, KagemushaWalletChargeQuoteV1,
            KagemushaWalletIndexedTreeV1, KagemushaWalletRedeemLeafV1,
            kagemusha_wallet_account_digest_v1, kagemusha_wallet_unload_nullifier_v1,
        },
    };
    use iroha_pasta::Eq as Vesta;
    use std::sync::OnceLock;

    /// Public secret of the fixture Enrollment issuer.
    const ISSUER_SECRET: [u8; 32] = [0x26; 32];
    /// Public secret of the fixture RegulatoryPolicy quote signer.
    const POLICY_SECRET: u8 = 0x37;
    const MANIFEST: [u8; 32] = [0x6d; 32];

    fn payout() -> AccountId {
        AccountId::new(
            "ed0120D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A"
                .parse()
                .unwrap(),
        )
    }
    fn beneficiary() -> AccountId {
        AccountId::new(
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C"
                .parse()
                .unwrap(),
        )
    }

    /// The fixture wallet, its credential bound to `payout()` at enrollment.
    struct Base {
        ctx: bootstrap::Context,
        boot: bootstrap::Inputs,
    }
    fn base() -> &'static Base {
        static BASE: OnceLock<Base> = OnceLock::new();
        BASE.get_or_init(|| {
            let key = WalletKey::labelled("owned Unload fixture wallet");
            let (ctx, mut boot) = bootstrap::fixture(&key, &Gens::<Vesta>::new(4096));
            let c = &mut boot.credential;
            c.body[98..130]
                .copy_from_slice(&kagemusha_wallet_account_digest_v1(&payout()).unwrap());
            c.signature = bootstrap::sign(
                credential::message(c),
                &BigUint::from_bytes_be(&ISSUER_SECRET),
                "owned Unload fixture issuer",
            );
            boot.core[7] = credential::digest(&boot.credential);
            boot.statement[7] = boot.core[7];
            Base { ctx, boot }
        })
    }
    fn installed(b: &Base) -> Installed<'_> {
        Installed {
            id: &b.ctx.relation,
            manifest_digest: &MANIFEST,
            scheme: &b.ctx.root.scheme,
            relation: &b.ctx.relation,
            root: &b.ctx.root.root,
        }
    }
    fn boot_profile() -> [Fp; 64] {
        let b = base();
        bootstrap::public_profile(&b.ctx, &b.boot)
    }

    /// The genuine first Load (100 units, ordinal 0) after the fixture
    /// Bootstrap, with signed fixture-genesis finality and its exact originals.
    fn genuine_load() -> ExactOperationWitness {
        static LOAD: OnceLock<(crate::load::Witness, Arc<[u8]>, Arc<[u8]>)> = OnceLock::new();
        let (step, receipt, finality) = LOAD.get_or_init(|| {
            let b = base();
            let boot = ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
                scheme: b.ctx.root.scheme,
                root: b.ctx.root.root.clone(),
                relation: b.ctx.relation,
                input: b.boot.clone(),
            }));
            let ExactOperationWitness::Load {
                step,
                receipt_original,
                finality_original,
            } = &device_fixture::load100(&boot).unwrap()
            else {
                unreachable!()
            };
            (
                (**step).clone(),
                receipt_original.clone(),
                finality_original.clone(),
            )
        });
        ExactOperationWitness::Load {
            step: Box::new(step.clone()),
            receipt_original: receipt.clone(),
            finality_original: finality.clone(),
        }
    }

    fn two128() -> Fp {
        Fp::from_u128(1u128 << 127).double()
    }

    /// Wallet-owned recovery-map stand-in. It answers for its current root;
    /// `checks_root = false` models a store answering from another tree.
    struct Store {
        map: KagemushaWalletIndexedTreeV1,
        checks_root: bool,
        asked: usize,
    }
    impl Store {
        fn of(map: &KagemushaWalletIndexedTreeV1) -> Self {
            Self {
                map: map.clone(),
                checks_root: true,
                asked: 0,
            }
        }
        fn answer(
            &mut self,
        ) -> impl FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1> + '_
        {
            move |root, key, value| {
                self.asked += 1;
                if self.checks_root && self.map.root() != *root {
                    return Err(Error::AuthorityMismatch("test store recovery root"));
                }
                self.map
                    .insert(*key, *value)
                    .map_err(|e| Error::Rejected(e.to_string()))
            }
        }
    }

    /// What the engine and provider hold once a predecessor is released.
    struct Released {
        prepared: PreparedOperation,
        head: VerifiedWalletHead,
        profile: [Fp; 64],
        map: KagemushaWalletIndexedTreeV1,
    }

    fn receipt_of(witness: &mut ExactOperationWitness) -> &mut bootstrap::Receipt {
        match witness {
            ExactOperationWitness::Load { step, .. } => &mut step.receipt,
            ExactOperationWitness::Unload { step, .. } => &mut step.receipt,
            _ => unreachable!("owned Unload tests release only Load and Unload"),
        }
    }

    /// The frozen preparation and custody-bound head with public `profile`.
    fn release(
        mut witness: ExactOperationWitness,
        profile: [Fp; 64],
    ) -> (PreparedOperation, VerifiedWalletHead) {
        let b = base();
        let kind = if matches!(witness, ExactOperationWitness::Unload { .. }) {
            6
        } else {
            2
        };
        let credential = witness.credential().clone();
        let receipt = receipt_of(&mut witness);
        receipt.signature = [0; 64];
        receipt.body[242..274].copy_from_slice(&Fp::from(987).to_repr());
        receipt.body[274..306].fill(0);
        let body = receipt.body;
        let capsule = RecoveryCapsule {
            frame: CapsuleFrame {
                version: RECOVERY_CAPSULE_VERSION,
                parameters_id: b.ctx.relation,
                manifest_digest: MANIFEST,
                scheme: credential.body[2..34].try_into().unwrap(),
                asset: credential.body[34..66].try_into().unwrap(),
                wallet: credential.body[66..98].try_into().unwrap(),
                payment_key: credential.body[130..195].to_vec(),
                kind,
                sequence: u128::from_le_bytes(body[98..114].try_into().unwrap()),
                predecessor: body[146..178].try_into().unwrap(),
                predecessor_capsule_digest: [0x21; 32],
                successor: body[178..210].try_into().unwrap(),
                operation_id: body[114..146].try_into().unwrap(),
                proof_digest: body[242..274].try_into().unwrap(),
                enrollment_marker: None,
                receipt_body: body.to_vec(),
                recovery: vec![1],
            },
        };
        let digest = RecoveryCapsule::frame_digest(&capsule.encode().unwrap());
        receipt_of(&mut witness).body[274..306].copy_from_slice(&digest);
        let body = receipt_of(&mut witness).body;
        let head = VerifiedWalletHead {
            parameters_id: b.ctx.relation,
            manifest_digest: MANIFEST,
            profile,
            pair: Arc::from(vec![1u8]),
            credential,
            certificate: b.boot.certificate.clone(),
            custody_digest: digest,
            enrollment_marker: None,
        };
        (
            PreparedOperation {
                resumed_under: None,
                manifest_digest: MANIFEST,
                parameters_id: b.ctx.relation,
                capsule,
                witness: Arc::new(witness),
                receipt_body: body,
                receive: None,
                archive: None,
                predecessor: head.clone(),
            },
            head,
        )
    }

    /// The released genuine Load. `edit` may change its successor opening and
    /// the recovery map behind it (the statement and receipt are restated to
    /// commit to the result); `burn` and `pending` replace the lineage values
    /// of the verified head, as a historical Burn or archive leaves them.
    fn released(
        edit: impl FnOnce(&mut [Fp; 30], &mut KagemushaWalletIndexedTreeV1),
        burn: Option<Fp>,
        pending: Option<Fp>,
    ) -> Released {
        let mut load = genuine_load();
        let ExactOperationWitness::Load { step, .. } = &mut load else {
            unreachable!()
        };
        let mut map = KagemushaWalletIndexedTreeV1::new();
        map.insert(step.recovery.key.to_repr(), step.recovery.value.to_repr())
            .map_err(|e| e.to_string())
            .unwrap();
        assert_eq!(
            map.root(),
            step.after_core[19].to_repr(),
            "canonical Load map"
        );
        edit(&mut step.after_core, &mut map);
        step.after_core[19] = field(&map.root()).unwrap();
        step.statement[8] = step.after_core[0];
        step.statement[9] = step.after_core[11];
        step.statement[10] = step.after_core[13];
        step.statement[15] = monetary_state::commitment(&step.after_core, &step.after_rest);
        let operation = field(step.receipt.body[114..146].try_into().unwrap()).unwrap();
        step.receipt = unsigned_receipt(&step.credential, &step.statement, operation);
        let prior = boot_profile();
        let after = step.after_core;
        let profile = crate::chain_fixture::public_after(
            &prior,
            &step.statement,
            &step.receipt,
            [
                after[0],
                after[27],
                after[22],
                burn.unwrap_or(prior[context::LINEAGE_IO + context::BURNED_TOTAL]),
                pending.unwrap_or(prior[context::LINEAGE_IO + context::PENDING_ROOT]),
                prior[context::LINEAGE_IO + context::CREDIT_ROOT],
            ],
            full::CONTROL,
        );
        let (prepared, head) = release(load, profile);
        Released {
            prepared,
            head,
            profile,
            map,
        }
    }
    fn plain() -> Released {
        released(|_, _| {}, None, None)
    }

    fn certificate_original(cert: &certificate::Certificate) -> Vec<u8> {
        [cert.body.as_slice(), cert.signature.as_slice()].concat()
    }

    /// A RegulatoryPolicy-signed Unload quote under the root with `root_secret`.
    fn quote_with(
        wallet: [u8; 32],
        ordinal: u128,
        face: u128,
        charge: u128,
        root_secret: [u8; 32],
    ) -> UnloadDestinations {
        let c = &base().boot.credential;
        let scheme: [u8; 32] = c.body[2..34].try_into().unwrap();
        let secret = BigUint::from(POLICY_SECRET);
        let signer = p256::mul(&p256::generator(), &secret).unwrap();
        let cert = certificate::issue_fixture(
            &scheme,
            certificate::Role::RegulatoryPolicy as u8,
            &signer,
            18,
            &BigUint::from_bytes_be(&root_secret),
        );
        let body = KagemushaWalletChargeQuoteBodyV1 {
            version: 1,
            scheme_id: scheme,
            asset_digest: c.body[34..66].try_into().unwrap(),
            wallet_id: wallet,
            kind: KagemushaWalletChargeKindV1::Unload,
            ordinal,
            net_amount: face,
            online_charge: charge,
            beneficiary_account_digest: kagemusha_wallet_account_digest_v1(&beneficiary()).unwrap(),
            issued_at_ms: 1_763_400_000_000,
            signer_certificate: certificate::certificate_digest(&cert).to_repr(),
        };
        let signature = bootstrap::sign(
            field(&body.signing_message()).unwrap(),
            &secret,
            "owned Unload RegulatoryPolicy quote",
        );
        UnloadDestinations {
            account: payout(),
            charge: KagemushaWalletUnloadChargeV1::Quoted {
                quote: KagemushaWalletChargeQuoteV1 {
                    body,
                    signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature).unwrap(),
                },
                beneficiary: beneficiary(),
            },
            quote_certificate: certificate_original(&cert),
        }
    }
    fn charged(ordinal: u128, face: u128, charge: u128) -> UnloadDestinations {
        let wallet = base().boot.credential.body[66..98].try_into().unwrap();
        quote_with(
            wallet,
            ordinal,
            face,
            charge,
            super::super::engine::TEST_ROOT_SECRET,
        )
    }
    fn ordinary() -> UnloadDestinations {
        UnloadDestinations {
            account: payout(),
            charge: KagemushaWalletUnloadChargeV1::None,
            quote_certificate: Vec::new(),
        }
    }

    fn build_original(
        r: &Released,
        face: u128,
        original: &[u8],
        store: &mut Store,
    ) -> Result<ExactOperationWitness> {
        unload_witness(
            &installed(base()),
            &r.prepared,
            &r.head,
            face,
            original,
            store.answer(),
            |previous| fresh_state_nonce(Some(previous)),
        )
    }
    fn build(
        r: &Released,
        face: u128,
        d: &UnloadDestinations,
        store: &mut Store,
    ) -> Result<ExactOperationWitness> {
        build_original(r, face, &d.to_original().unwrap(), store)
    }
    fn step_of(witness: &ExactOperationWitness) -> &crate::control::Witness {
        match witness {
            ExactOperationWitness::Unload { step, .. } => step,
            _ => unreachable!("owned Unload constructor"),
        }
    }
    fn destinations_of(witness: &ExactOperationWitness) -> &UnloadDestinations {
        match witness {
            ExactOperationWitness::Unload { destinations, .. } => destinations,
            _ => unreachable!("owned Unload constructor"),
        }
    }

    /// Restate a changed successor opening so only the relation refuses it.
    fn reseal(step: &mut crate::control::Witness) {
        step.statement[15] = monetary_state::commitment(&step.after_core, &step.after_rest);
        let operation = field(step.receipt.body[114..146].try_into().unwrap()).unwrap();
        step.receipt = unsigned_receipt(&step.credential, &step.statement, operation);
    }

    /// Whether the Control preparation relation `Engine::prepare` checks holds
    /// for this Unload after the predecessor with `profile`.
    fn relation_holds(step: &crate::control::Witness, profile: [Fp; 64]) -> bool {
        let mut input = step.clone();
        input.receipt.body[242..274].copy_from_slice(&Fp::from(987).to_repr());
        input.receipt.body[274..306].fill(0x55);
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let prior = harness::predecessor(&mut w, family, profile);
        let Ok(prepared) = crate::control::append_preparation(
            &mut w,
            family,
            &base().ctx.root.root,
            &input,
            &prior,
        ) else {
            return false;
        };
        harness::expose(
            &mut w,
            family,
            4,
            &[prepared.head(), prepared.body().message()],
        );
        harness::holds(&harness::finish(w, 16))
    }

    fn assert_canonical_recovery_roundtrip(witness: &ExactOperationWitness) {
        let bytes = witness.encode().unwrap();
        let restored = ExactOperationWitness::decode(&bytes).unwrap();
        assert_eq!(restored.encode().unwrap(), bytes);
    }

    /// Every native invariant of one constructed Unload, from its inputs.
    fn assert_exact_transition(
        witness: &ExactOperationWitness,
        r: &Released,
        store: &Store,
        face: u128,
        charge: u128,
    ) {
        let step = step_of(witness);
        let (core, rest) = r.prepared.successor_opening();
        let lineage = &r.profile[context::LINEAGE_IO..];
        let c = &step.credential;
        assert_eq!(step.mode, crate::control::Mode::Unload);
        assert_eq!(step.before_core, *core);
        assert_eq!(step.before_rest, *rest);
        assert_eq!(step.after_rest, *rest);
        for i in (0..30).filter(|i| !unload::CHANGED.contains(i)) {
            assert_eq!(step.after_core[i], core[i], "preserved Core {i}");
        }
        let balance = uint(core[9]).unwrap();
        let ordinal = uint(core[14]).unwrap();
        assert_eq!(step.after_core[9], Fp::from_u128(balance - face));
        assert_eq!(step.after_core[10], lineage[context::BURNED_TOTAL]);
        assert_eq!(step.after_core[11], core[11] + Fp::ONE);
        assert_eq!(step.after_core[14], core[14] + Fp::ONE);
        assert_eq!(step.after_core[18], lineage[context::PENDING_ROOT]);
        assert_eq!(step.after_core[19].to_repr(), store.map.root());
        assert_ne!(step.after_core[29], Fp::ZERO);
        assert_ne!(step.after_core[29], core[29]);
        let scheme: [u8; 32] = c.body[2..34].try_into().unwrap();
        let wallet: [u8; 32] = c.body[66..98].try_into().unwrap();
        let nullifier = kagemusha_wallet_unload_nullifier_v1(&scheme, &wallet, ordinal);
        let s = &step.statement;
        assert_eq!(s[12], lineage[context::BURNED_TOTAL]);
        assert_eq!(s[13], lineage[context::PENDING_ROOT]);
        assert_eq!(s[14], r.profile[full::HEAD_IO]);
        assert_eq!(s[15], monetary_state::commitment(&step.after_core, rest));
        assert_eq!(s[16], Fp::from(6));
        assert_eq!(s[17].to_repr(), nullifier);
        assert_eq!(s[18], core[14]);
        assert_eq!(s[19], Fp::from_u128(face));
        assert_eq!(s[20], Fp::from_u128(charge));
        let quote = destinations_of(witness).charge.quote();
        assert_eq!(
            s[21].to_repr(),
            quote.map_or([0; 32], |q| q.charge_quote_digest())
        );
        assert!(s[22..].iter().all(|f| *f == Fp::ZERO));
        let leaf = KagemushaWalletRedeemLeafV1 {
            ordinal,
            nullifier,
            amount: face,
            online_charge: charge,
        };
        assert_eq!(step.recovery.key.to_repr(), leaf.key());
        assert_eq!(
            step.recovery.value.to_repr(),
            leaf.leaf_value().map_err(|e| e.to_string()).unwrap()
        );
        assert_eq!(step.recovery.old_root, core[19]);
        let body = &step.receipt.body;
        let operation = poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &[core[5], core[6], Fp::from(6), s[17]],
        );
        assert_eq!(body[98..114], (uint(core[11]).unwrap() + 1).to_le_bytes());
        assert_eq!(body[114..146], operation.to_repr());
        assert_eq!(body[146..178], s[14].to_repr());
        assert_eq!(body[178..210], s[15].to_repr());
        assert_eq!(body[210..242], crate::statement::digest(s).to_repr());
        assert!(body[242..].iter().all(|b| *b == 0));
        assert!(step.receipt.signature.iter().all(|b| *b == 0));
    }

    #[test]
    fn ordinary_and_charged_unload_follow_a_genuine_released_load() {
        let r = plain();
        let b = base();

        // Ordinary: no quote, the account receives the full face amount.
        let mut store = Store::of(&r.map);
        let d = ordinary();
        let built = build(&r, 40, &d, &mut store).unwrap();
        assert_eq!(store.asked, 1);
        assert_canonical_recovery_roundtrip(&built);
        assert_exact_transition(&built, &r, &store, 40, 0);
        let step = step_of(&built);
        assert_eq!(step.after_core[9], Fp::from(60));
        assert_eq!(
            destinations_of(&built).to_original().unwrap(),
            d.to_original().unwrap()
        );
        assert_eq!(
            destinations_of(&built)
                .verify(
                    &step.credential,
                    &step.statement,
                    &b.ctx.root.scheme,
                    &b.ctx.root.root
                )
                .unwrap(),
            (0, 40, 40)
        );
        assert!(relation_holds(step, r.profile));

        // Charged: the debit is still exactly the face; the charge is withheld online.
        let mut store = Store::of(&r.map);
        let d = charged(0, 40, 3);
        let quoted = build(&r, 40, &d, &mut store).unwrap();
        assert_canonical_recovery_roundtrip(&quoted);
        assert_exact_transition(&quoted, &r, &store, 40, 3);
        let charged_step = step_of(&quoted);
        assert_eq!(charged_step.after_core[9], Fp::from(60));
        assert_eq!(
            destinations_of(&quoted)
                .verify(
                    &charged_step.credential,
                    &charged_step.statement,
                    &b.ctx.root.scheme,
                    &b.ctx.root.root
                )
                .unwrap(),
            (0, 40, 37)
        );
        assert!(relation_holds(charged_step, r.profile));
        // One operation identity per ordinal; independent blinding per construction.
        assert_eq!(
            charged_step.receipt.body[114..146],
            step.receipt.body[114..146]
        );
        assert_ne!(charged_step.after_core[29], step.after_core[29]);
        assert_ne!(charged_step.statement[15], step.statement[15]);

        // The exact financial review binds face, charge, net and both accounts.
        let (prepared, _) = release(quoted.clone(), r.profile);
        let review = prepared.review_bytes().unwrap();
        assert_eq!(&review[..3], &[3, 0, 6]);
        assert_eq!(&review[195..211], &0u128.to_le_bytes());
        assert_eq!(&review[211..227], &40u128.to_le_bytes());
        assert_eq!(&review[227..243], &3u128.to_le_bytes());
        assert_eq!(&review[243..259], &37u128.to_le_bytes());
        assert_eq!(&review[259..291], &charged_step.statement[21].to_repr());
        assert_eq!(&review[291..323], &b.boot.credential.body[98..130]);
        assert_eq!(
            &review[323..355],
            &kagemusha_wallet_account_digest_v1(&beneficiary()).unwrap()
        );

        // The harness is not vacuous: an offline charge debit is refused.
        let mut debited = charged_step.clone();
        debited.after_core[9] -= Fp::from(3);
        reseal(&mut debited);
        assert!(!relation_holds(&debited, r.profile));
    }

    #[test]
    fn foreign_destination_or_quote_is_refused_before_the_store_is_asked() {
        let r = plain();
        let attempt = |d: &UnloadDestinations| {
            let mut store = Store::of(&r.map);
            let result = build(&r, 40, d, &mut store).map(drop);
            assert_eq!(store.asked, 0, "authentication precedes staging");
            result
        };
        let mut store = Store::of(&r.map);
        build(&r, 40, &ordinary(), &mut store).unwrap();
        assert_eq!(store.asked, 1);

        // A payout account other than the credential's, with and without a charge.
        let foreign = UnloadDestinations {
            account: beneficiary(),
            ..ordinary()
        };
        assert!(matches!(
            attempt(&foreign),
            Err(Error::AuthorityMismatch(
                "Unload credential payout account/amount"
            ))
        ));
        let mut swapped = charged(0, 40, 3);
        let KagemushaWalletUnloadChargeV1::Quoted { beneficiary: b, .. } = &mut swapped.charge
        else {
            unreachable!()
        };
        std::mem::swap(&mut swapped.account, b);
        assert!(matches!(
            attempt(&swapped),
            Err(Error::AuthorityMismatch(
                "Unload credential payout account/amount"
            ))
        ));
        // A beneficiary the signed quote does not name.
        let mut redirected = charged(0, 40, 3);
        let KagemushaWalletUnloadChargeV1::Quoted { beneficiary: b, .. } = &mut redirected.charge
        else {
            unreachable!()
        };
        *b = payout();
        assert!(matches!(
            attempt(&redirected),
            Err(Error::AuthorityMismatch("Unload signed quote scope"))
        ));
        // A genuinely signed quote for another wallet.
        let other_wallet = quote_with([0x99; 32], 0, 40, 3, super::super::engine::TEST_ROOT_SECRET);
        assert!(attempt(&other_wallet).is_err(), "quote of another wallet");
        // A signer of another role or certified by another root.
        let mut wrong_role = charged(0, 40, 3);
        wrong_role.quote_certificate[34] = certificate::Role::TimeAnchor as u8;
        assert!(matches!(
            attempt(&wrong_role),
            Err(Error::AuthorityMismatch(_))
        ));
        let foreign_root = quote_with(
            base().boot.credential.body[66..98].try_into().unwrap(),
            0,
            40,
            3,
            [0x12; 32],
        );
        assert!(matches!(
            attempt(&foreign_root),
            Err(Error::AuthorityMismatch(_))
        ));
        // A changed signed body.
        let mut altered = charged(0, 40, 3);
        let KagemushaWalletUnloadChargeV1::Quoted { quote, .. } = &mut altered.charge else {
            unreachable!()
        };
        quote.body.issued_at_ms += 1;
        assert!(attempt(&altered).is_err(), "changed signed quote body");
        // A certificate without a charge.
        let spurious = UnloadDestinations {
            quote_certificate: charged(0, 40, 3).quote_certificate,
            ..ordinary()
        };
        assert!(matches!(
            attempt(&spurious),
            Err(Error::AuthorityMismatch("Unload absent charge originals"))
        ));

        // Only the exact bounded canonical original is admitted.
        let exact = charged(0, 40, 3).to_original().unwrap();
        let mut trailing = exact.clone();
        trailing.push(0);
        let compact = norito::encode_canonical_with_layout(
            &charged(0, 40, 3),
            norito::core::WireLayout::Compact,
        )
        .unwrap();
        for (case, bytes) in [
            ("trailing byte", trailing),
            ("truncated", exact[..exact.len() - 1].to_vec()),
            ("compact layout", compact),
            ("oversize", vec![0; MAX_UNLOAD_CLAIM_BYTES + 1]),
        ] {
            let mut store = Store::of(&r.map);
            assert!(
                build_original(&r, 40, &bytes, &mut store).is_err(),
                "{case} destinations original"
            );
            assert_eq!(store.asked, 0, "{case}");
        }
        let mut store = Store::of(&r.map);
        build_original(&r, 40, &exact, &mut store).unwrap();
        assert_eq!(store.asked, 1);
    }

    #[test]
    fn arithmetic_boundaries_are_checked_integers() {
        let r = plain();
        let attempt = |r: &Released, face: u128, d: &UnloadDestinations| {
            let mut store = Store::of(&r.map);
            let result = build(r, face, d, &mut store);
            (result, store.asked)
        };
        let refused = |(result, asked): (Result<ExactOperationWitness>, usize), case: &str| {
            assert!(result.is_err(), "{case}");
            assert_eq!(asked, 0, "{case} before staging");
            result.err().unwrap()
        };

        // Face: nonzero, at most the spendable 100, exactly the quote's net.
        assert!(matches!(
            refused(attempt(&r, 0, &ordinary()), "zero face"),
            Error::AuthorityMismatch("Unload zero face amount")
        ));
        assert!(matches!(
            refused(attempt(&r, 101, &ordinary()), "overdraw"),
            Error::AuthorityMismatch("Unload insufficient spendable value")
        ));
        refused(attempt(&r, 4, &charged(0, 4, 5)), "charge above face");
        refused(
            attempt(&r, 41, &charged(0, 40, 3)),
            "face other than quoted",
        );
        refused(attempt(&r, 40, &charged(0, 40, 0)), "zero quoted charge");
        // The full spendable value redeems and leaves exactly the lineage burn.
        let (full, _) = attempt(&r, 100, &ordinary());
        let full = full.unwrap();
        assert_eq!(step_of(&full).after_core[9], Fp::ZERO);
        assert!(relation_holds(step_of(&full), r.profile));

        // Counters: the last representable ordinal redeems; overflow does not.
        let last = released(
            |core, _| core[14] = Fp::from_u128(u128::MAX - 1),
            None,
            None,
        );
        let (result, asked) = attempt(&last, 10, &ordinary());
        let last_step = result.unwrap();
        assert_eq!(asked, 1);
        assert_eq!(step_of(&last_step).after_core[14], Fp::from_u128(u128::MAX));
        let redeemed_out = released(|core, _| core[14] = Fp::from_u128(u128::MAX), None, None);
        assert!(matches!(
            refused(attempt(&redeemed_out, 10, &ordinary()), "ordinal overflow"),
            Error::Malformed("Unload redemption ordinal overflow")
        ));
        let sequenced_out = released(|core, _| core[11] = Fp::from_u128(u128::MAX), None, None);
        assert!(matches!(
            refused(
                attempt(&sequenced_out, 10, &ordinary()),
                "sequence overflow"
            ),
            Error::Malformed("Unload sequence overflow")
        ));
        // Integer widths and the lineage bound.
        let wide = released(
            |core, _| core[9] = Fp::from_u128(u128::MAX) + Fp::ONE,
            None,
            None,
        );
        assert!(matches!(
            refused(attempt(&wide, 10, &ordinary()), "wide balance"),
            Error::Malformed("released state integer width")
        ));
        let over_burned = released(|_, _| {}, Some(Fp::from(101)), None);
        assert!(matches!(
            refused(attempt(&over_burned, 1, &ordinary()), "balance below burn"),
            Error::Malformed("released balance below lineage burn")
        ));
        let no_pending = released(|_, _| {}, None, Some(Fp::ZERO));
        assert!(matches!(
            refused(attempt(&no_pending, 10, &ordinary()), "zero pending root"),
            Error::AuthorityMismatch("Unload lineage pending root")
        ));
        let dead = released(|core, _| core[0] = Fp::from(3), None, None);
        assert!(matches!(
            refused(attempt(&dead, 10, &ordinary()), "lifecycle"),
            Error::AuthorityMismatch("Unload lifecycle")
        ));

        // The successor nonce is fresh, nonzero and never the predecessor's;
        // entropy failure leaves the store untouched.
        for reused in [false, true] {
            let mut store = Store::of(&r.map);
            let result = unload_witness(
                &installed(base()),
                &r.prepared,
                &r.head,
                40,
                &ordinary().to_original().unwrap(),
                store.answer(),
                |previous| Ok(if reused { previous } else { Fp::ZERO }),
            );
            assert!(matches!(
                result,
                Err(Error::Unavailable("fresh state nonce"))
            ));
            assert_eq!(store.asked, 0);
        }
    }

    #[test]
    fn historical_linked_burn_bounds_spendable_value_and_is_materialized() {
        // A historical Receive Burn raised the verified lineage burn to 30 and an
        // archive moved the lineage pending root; the released private Core still
        // carries a stale burn of 10 and the pre-archive pending root.
        let archived = indexed_map::empty_insert::<32>(Fp::from(0x77), Fp::from(0x78)).1;
        let r = released(
            |core, _| core[10] = Fp::from(10),
            Some(Fp::from(30)),
            Some(archived),
        );
        let (core, _) = r.prepared.successor_opening();
        let (stale_burn, stale_pending) = (core[10], core[18]);
        assert_ne!(stale_pending, archived);
        for (face, case) in [
            (71, "one above the lineage-adjusted spendable value"),
            (90, "stale Core burn as affordability"),
            (100, "burned value redeemed"),
        ] {
            let mut store = Store::of(&r.map);
            assert!(
                matches!(
                    build(&r, face, &ordinary(), &mut store),
                    Err(Error::AuthorityMismatch(
                        "Unload insufficient spendable value"
                    ))
                ),
                "{case}"
            );
            assert_eq!(store.asked, 0, "{case}");
        }
        let mut store = Store::of(&r.map);
        let built = build(&r, 70, &charged(0, 70, 5), &mut store).unwrap();
        assert_exact_transition(&built, &r, &store, 70, 5);
        let step = step_of(&built);
        assert_eq!(step.after_core[9], Fp::from(30));
        assert_eq!(step.after_core[10], Fp::from(30));
        assert_eq!(step.after_core[18], archived);
        assert_eq!(step.statement[12], Fp::from(30));
        assert_eq!(step.statement[13], archived);
        assert!(relation_holds(step, r.profile));
        // Keeping either stale private value is a relation refusal.
        for (index, stale) in [(10, stale_burn), (18, stale_pending)] {
            let mut kept = step.clone();
            kept.after_core[index] = stale;
            reseal(&mut kept);
            assert!(!relation_holds(&kept, r.profile), "stale Core {index}");
        }
    }

    #[test]
    fn repeated_ordinal_is_refused_and_retry_keeps_the_operation_identity() {
        let r = plain();
        let d = charged(0, 40, 3);
        let mut store = Store::of(&r.map);
        let first = build(&r, 40, &d, &mut store).unwrap();
        let step = step_of(&first);
        // An independent retry from the same released head keeps the durable
        // operation identity and nullifier and blinds another successor.
        let retry = build(&r, 40, &d, &mut Store::of(&r.map)).unwrap();
        let again = step_of(&retry);
        assert_eq!(again.receipt.body[114..146], step.receipt.body[114..146]);
        assert_eq!(again.statement[17], step.statement[17]);
        assert_eq!(again.after_core[..29], step.after_core[..29]);
        assert_ne!(again.after_core[29], step.after_core[29]);
        assert_ne!(again.statement[15], step.statement[15]);

        // Once released, the first Unload's ordinal is consumed.
        let profile = crate::chain_fixture::public_after(
            &r.profile,
            &step.statement,
            &step.receipt,
            r.profile[context::LINEAGE_IO..context::LINEAGE_IO + context::LINEAGE_FIELDS]
                .try_into()
                .unwrap(),
            full::CONTROL,
        );
        let (prepared, head) = release(first.clone(), profile);
        let next = Released {
            prepared,
            head,
            profile,
            map: store.map.clone(),
        };
        for (quote, error) in [
            (charged(0, 40, 3), "Unload quote ordinal already redeemed"),
            (
                charged(2, 40, 3),
                "Unload quote ordinal is ahead of the next redemption",
            ),
        ] {
            let mut store = Store::of(&next.map);
            assert!(
                matches!(
                    build(&next, 40, &quote, &mut store),
                    Err(Error::AuthorityMismatch(e)) if e == error
                ),
                "{error}"
            );
            assert_eq!(store.asked, 0, "{error}");
        }
        // The next redemption takes ordinal 1, a new nullifier and identity.
        let mut store = Store::of(&next.map);
        let second = build(&next, 10, &ordinary(), &mut store).unwrap();
        assert_canonical_recovery_roundtrip(&second);
        assert_exact_transition(&second, &next, &store, 10, 0);
        let following = step_of(&second);
        assert_eq!(following.before_core, step.after_core);
        assert_eq!(following.statement[18], Fp::ONE);
        assert_ne!(following.statement[17], step.statement[17]);
        assert_ne!(
            following.receipt.body[114..146],
            step.receipt.body[114..146]
        );
        assert_eq!(following.after_core[9], Fp::from(50));
        assert!(relation_holds(following, next.profile));

        // A predecessor whose map already records its next redemption: the
        // canonical store cannot stage it, and another tree's answer is refused.
        let recorded = released(
            |_, map| {
                map.insert(
                    unload::recovery_key(Fp::ZERO).to_repr(),
                    Fp::from(0x99).to_repr(),
                )
                .map_err(|e| e.to_string())
                .unwrap();
            },
            None,
            None,
        );
        let mut store = Store::of(&recorded.map);
        assert!(matches!(
            build(&recorded, 10, &ordinary(), &mut store),
            Err(Error::Rejected(_))
        ));
        assert_eq!(store.asked, 1);
        let mut liar = Store::of(&r.map);
        liar.checks_root = false;
        assert!(matches!(
            build(&recorded, 10, &ordinary(), &mut liar),
            Err(Error::AuthorityMismatch("Unload recovery insertion"))
        ));
        assert_eq!(liar.asked, 1);
    }

    /// One attempt under `scope` and `head`, with the count of store requests.
    fn scoped(
        r: &Released,
        scope: &Installed<'_>,
        head: &VerifiedWalletHead,
    ) -> (Result<()>, usize) {
        let mut store = Store::of(&r.map);
        let result = unload_witness(
            scope,
            &r.prepared,
            head,
            40,
            &ordinary().to_original().unwrap(),
            store.answer(),
            |previous| fresh_state_nonce(Some(previous)),
        )
        .map(drop);
        (result, store.asked)
    }

    #[test]
    fn foreign_scope_custody_credential_or_opening_is_refused_before_staging() {
        let r = plain();
        let scope = installed(base());
        let (result, asked) = scoped(&r, &scope, &r.head);
        result.unwrap();
        assert_eq!(asked, 1);
        // A head not yet bound to custody is this preparation's.
        let mut unbound = r.head.clone();
        unbound.custody_digest = [0; 32];
        scoped(&r, &scope, &unbound).0.unwrap();

        let mut other_id = *scope.id;
        other_id[0] ^= 1;
        let mut foreign = scope;
        foreign.id = &other_id;
        let mut foreign_head = r.head.clone();
        foreign_head.parameters_id = other_id;
        let other_manifest = [0x6e; 32];
        let mut reissued = scope;
        reissued.manifest_digest = &other_manifest;
        let mut reissued_head = r.head.clone();
        reissued_head.manifest_digest = other_manifest;
        let mut other_scheme = *scope.scheme;
        other_scheme[0] ^= 1;
        let mut rescoped = scope;
        rescoped.scheme = &other_scheme;
        let mut another_capsule = r.head.clone();
        another_capsule.custody_digest[0] ^= 1;
        let mut origin = r.head.clone();
        origin.enrollment_marker = Some([0x44; 32]);
        let mut unproved = r.head.clone();
        unproved.pair = Arc::from(Vec::new());
        let mut another_credential = r.head.clone();
        another_credential.credential.signature[0] ^= 1;
        for (case, scope, head, error) in [
            (
                "parameter identity",
                foreign,
                &r.head,
                "Unload parameter identity",
            ),
            (
                "head parameter identity",
                scope,
                &foreign_head,
                "Unload parameter identity",
            ),
            (
                "producer manifest",
                reissued,
                &r.head,
                "released producer manifest",
            ),
            (
                "head producer manifest",
                scope,
                &reissued_head,
                "released producer manifest",
            ),
            ("installed scheme", rescoped, &r.head, "released credential"),
            (
                "another capsule",
                scope,
                &another_capsule,
                "released predecessor custody",
            ),
            (
                "enrollment origin",
                scope,
                &origin,
                "released predecessor custody",
            ),
            (
                "no recursive proof",
                scope,
                &unproved,
                "released predecessor custody",
            ),
            (
                "another credential",
                scope,
                &another_credential,
                "released credential",
            ),
        ] {
            let (result, asked) = scoped(&r, &scope, head);
            assert!(
                matches!(result, Err(Error::AuthorityMismatch(e)) if e == error),
                "{case}"
            );
            assert_eq!(asked, 0, "{case}");
        }
        // The private opening must be the verified public head's.
        for at in [
            full::HEAD_IO,
            context::LINEAGE_IO + context::LIFECYCLE,
            context::LINEAGE_IO + context::POLICY_EPOCH,
            context::LINEAGE_IO + context::ENABLED_CONTROLS,
        ] {
            let mut wrong = r.head.clone();
            wrong.profile[at] += Fp::ONE;
            let (result, asked) = scoped(&r, &scope, &wrong);
            assert!(
                matches!(
                    result,
                    Err(Error::AuthorityMismatch("released successor opening"))
                ),
                "public profile word {at}"
            );
            assert_eq!(asked, 0, "public profile word {at}");
        }
    }

    /// SplitMix64: reproducible randomized cases from one seed.
    struct Mix(u64);
    impl Mix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        fn u128(&mut self) -> u128 {
            (u128::from(self.next()) << 64) | u128::from(self.next())
        }
        fn below(&mut self, bound: u128) -> u128 {
            self.u128() % bound
        }
        /// A random field word, nonzero except with negligible probability.
        fn word(&mut self) -> Fp {
            Fp::from_u128(self.u128()) * Fp::from_u128(self.u128()) + Fp::ONE
        }
    }

    #[test]
    fn randomized_core_and_map_follow_the_exact_relation_transition() {
        let mut mix = Mix(0x6b67_756e_6c6f_6164);
        for case in 0..6 {
            let balance = (mix.u128() >> (mix.next() % 120)).max(1);
            let burn = mix.below(balance);
            let face = 1 + mix.below(balance - burn);
            let ordinal = mix.below(u128::MAX);
            let lifecycle = Fp::from(1 + mix.next() % 2);
            let counters: Vec<u128> = (0..4).map(|_| mix.below(u128::MAX)).collect();
            let words: Vec<Fp> = (0..7).map(|_| mix.word()).collect();
            let entries = mix.next() % 5;
            let keys: Vec<(Fp, Fp)> = (0..entries)
                .map(|_| {
                    let redeem = mix.next() % 2 == 1;
                    let mut low = mix.u128();
                    if redeem && low == ordinal {
                        low ^= 1;
                    }
                    (
                        Fp::from(1 + u64::from(redeem)) * two128() + Fp::from_u128(low),
                        mix.word(),
                    )
                })
                .collect();
            let r = released(
                |core, map| {
                    core[0] = lifecycle;
                    core[9] = Fp::from_u128(balance);
                    // Stale private burn; the lineage value governs.
                    core[10] = Fp::from_u128(counters[0]);
                    core[11] = Fp::from_u128(counters[1]);
                    core[12] = Fp::from_u128(counters[2]);
                    core[13] = Fp::from_u128(counters[3]);
                    core[14] = Fp::from_u128(ordinal);
                    for (index, word) in [15, 16, 17, 18, 20, 29].into_iter().zip(&words) {
                        core[index] = *word;
                    }
                    for (key, value) in &keys {
                        map.insert(key.to_repr(), value.to_repr())
                            .map_err(|e| e.to_string())
                            .unwrap();
                    }
                },
                Some(Fp::from_u128(burn)),
                Some(words[6]),
            );
            let charge = (case % 2 == 1).then(|| 1 + mix.below(face));
            let d = charge.map_or_else(ordinary, |charge| charged(ordinal, face, charge));
            let mut store = Store::of(&r.map);
            let built = build(&r, face, &d, &mut store)
                .unwrap_or_else(|e| panic!("randomized case {case}: {e}"));
            assert_eq!(store.asked, 1);
            assert_canonical_recovery_roundtrip(&built);
            assert_exact_transition(&built, &r, &store, face, charge.unwrap_or(0));
            assert!(
                relation_holds(step_of(&built), r.profile),
                "randomized case {case}"
            );
            // One above the lineage-adjusted spendable value is refused unstaged.
            if let Some(over) = (balance - burn).checked_add(1) {
                let mut store = Store::of(&r.map);
                assert!(
                    build(&r, over, &ordinary(), &mut store).is_err(),
                    "randomized case {case} overdraw"
                );
                assert_eq!(store.asked, 0);
            }
        }
    }
}
