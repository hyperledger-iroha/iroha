//! The engineering continuation gate on the installed hundred-payer catalog.
//!
//! Order matters and is asserted by construction: the shared suffix verifier verifies all 100
//! genuine Receives from the genuine Bootstrap Omega; the Unload is derived from that verified
//! tip, its `σ_unload` is proved and its receipt is signed over a distinct suffix-authority
//! digest; only then is any Receive fold read. The folds must equal the verifier's adjusted
//! fields, and the unchanged Unload Q/A/Omega must fold that same receipt under the installed
//! keys to exactly the verifier's successor fields. Nothing changes a circuit, key, descriptor
//! or catalog; the receipt is never re-signed.
use super::*;
use ff::{Field, PrimeField};
use iroha_core_zk::{
    kagemusha_wallet_artifacts_v1::producer_inventory::{
        ImportedQV1, ImportedSigmaV1, OriginalSourceV1, PROVING_KEY_MAX_BYTES_V1,
        QualifiedOperationOwnerV1, QualifiedWalletSourcesV1,
    },
    kagemusha_wallet_preparation_v1::PreparationV1,
};
use iroha_data_model::{
    block::decode_framed_signed_block,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier, authenticated_genesis},
};
use iroha_kagemusha_proof::{
    a_relation::{native::consuming, schedule::compiled::OperationRoute},
    admin_sigma::ConsumingWitness,
    operation_relation::objects::{ObjectKind, signed_message_native},
    q_signature::SignatureWitness,
    tree::{IndexedInsert, IndexedLeaf},
};
use iroha_pasta::{Fp, PastaCurve};
use iroha_plonk::{
    ProverConfig, ProverRandomness,
    keys::{CosetCachePolicy, pk::artifact::ReadConfig},
};
use iroha_plonk_gadgets::p256::native::words_from_be;
use iroha_plonk_recursion::{FoldConfig, FoldTape, obligation::ledger::Variant};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use rand::rand_core::TryRng as _;

/// Read-only content-addressed public engineering fixture source. The installed source
/// owner still checks every exact length, SHA-256, descriptor and key. No private custody
/// directory is created or chmodded, and no wallet originals are rewritten.
pub(super) struct FixtureOriginals(PathBuf);

impl OriginalSourceV1 for FixtureOriginals {
    fn open(
        &mut self,
        digest: [u8; 32],
    ) -> Result<Box<dyn std::io::Read + '_>, iroha_core_zk::kagemusha_wallet_proofs_v1::Error> {
        use iroha_core_zk::kagemusha_wallet_proofs_v1::Error;
        let path = self.0.join(hex::encode(digest));
        let metadata = fs::symlink_metadata(&path).map_err(|_| Error::Unavailable)?;
        if !metadata.file_type().is_file() || metadata.len() > PROVING_KEY_MAX_BYTES_V1 as u64 {
            return Err(Error::Inventory);
        }
        let file = fs::File::open(path).map_err(|_| Error::Unavailable)?;
        Ok(Box::new(file.take(PROVING_KEY_MAX_BYTES_V1 as u64 + 1)))
    }
}

/// Public engineering harness merchant key: `MERCHANT_KEY_SEED = 250`. No phone key.
const MERCHANT_KEY: [u8; 32] = [250; 32];
/// Public engineering payer-1 key seed; used only as a wrong signer.
const OTHER_KEY: [u8; 32] = [1; 32];
const UNLOAD_AMOUNT: u128 = 150;

fn field(bytes: [u8; 32]) -> Fp {
    Option::from(Fp::from_repr(bytes)).unwrap()
}

fn random_field() -> [u8; 32] {
    let mut bytes = [0; 32];
    rand::rngs::SysRng.try_fill_bytes(&mut bytes).unwrap();
    bytes[31] &= 0x3f;
    assert!(bool::from(Fp::from_repr(bytes).is_some()));
    bytes
}

fn tape<C: PastaCurve>() -> FoldTape<C> {
    FoldTape::try_fill::<std::io::Error>(|value| {
        for _ in 0..134 {
            let mut bytes = [0; 32];
            rand::rngs::SysRng
                .try_fill_bytes(&mut bytes)
                .map_err(std::io::Error::other)?;
            bytes[31] &= 0x7f;
            if let Some(scalar) = Option::<C::ScalarExt>::from(C::ScalarExt::from_repr(bytes)) {
                *value = scalar;
                return Ok(());
            }
        }
        Err(std::io::Error::other("bounded entropy refusal"))
    })
    .unwrap()
}

fn config() -> ReadConfig {
    ReadConfig {
        maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::new(128 << 20),
    }
}

/// The complete installed engineering catalog: every sigma/Q/A/W/Omega source qualified
/// against the signed inventory and the run's native genesis. Reads no wallet custody.
pub(super) fn qualify(corpus: &Corpus) -> (QualifiedWalletSourcesV1, FixtureOriginals) {
    let catalog = corpus.root.join("catalog");
    let inventory = bounded(&catalog.join("producer-inventory.norito"), 8 << 20);
    let authenticated = corpus
        .installed
        .authenticate_producer_inventory(&inventory)
        .unwrap();
    let genesis = decode_framed_signed_block(&bounded(
        &corpus.root.join("setup/signed-genesis.wire"),
        16 << 20,
    ))
    .unwrap();
    let epoch = authenticated_genesis(&genesis).unwrap().into_parts().0;
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let finality = SumeragiFinalityVerifier::new(
        &genesis,
        "kagemusha-native-monetary-acceptance-v1",
        validators,
    )
    .unwrap();
    let mut originals = FixtureOriginals(catalog.join("originals"));
    let started = Instant::now();
    let sources = authenticated
        .qualify_wallet(&corpus.installed, &finality, &mut originals, config())
        .unwrap();
    eprintln!(
        "SUFFIX_GATE qualified_complete_installed_catalog=true ms={}",
        started.elapsed().as_millis()
    );
    (sources, originals)
}

/// The native recovery-map insertion for the Unload's redeem leaf. The merchant never
/// loaded or unloaded, so its recovery map is the empty indexed tree; assert that exactly.
fn recovery_insertion(
    tip: &KagemushaWalletStateV1,
    amount: u128,
) -> KagemushaWalletIndexedInsertV1 {
    let mut tree = KagemushaWalletIndexedTreeV1::new();
    assert_eq!(
        tree.root(),
        tip.core.load_redeem_recovery_root,
        "the merchant's recovery map is empty at the tip"
    );
    let ordinal = tip.core.next_redeem;
    let leaf = KagemushaWalletRedeemLeafV1 {
        ordinal,
        nullifier: kagemusha_wallet_unload_nullifier_v1(
            &tip.core.scheme_id,
            &tip.core.wallet_id,
            ordinal,
        ),
        amount,
        online_charge: 0,
    };
    tree.insert(leaf.key(), leaf.leaf_value().unwrap()).unwrap()
}

/// Sign the Unload receipt with `key` over `digest`; returns the receipt and its exact tape.
fn sign(
    credential: &KagemushaWalletCredentialV1,
    statement: &KagemushaWalletStatementV1,
    digest: &[u8; 32],
    capsule_commitment: [u8; 32],
    key: [u8; 32],
) -> (KagemushaWalletReceiptV1, Vec<u8>) {
    let signer = KagemushaWalletReceiptSignerV1::from_credential(credential).unwrap();
    let body = KagemushaWalletReceiptBodyV1::derive(
        &signer,
        statement,
        digest,
        capsule_commitment,
        [0; 32],
    )
    .unwrap();
    let key = SigningKey::from_slice(&key).unwrap();
    let signature: Signature = key.sign(&body.signing_message());
    let signature = signature.normalize_s();
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
    let receipt = KagemushaWalletReceiptV1 {
        version: 1,
        operation_id: body.operation_id,
        capsule_commitment,
        payment_digest: [0; 32],
        signature,
    };
    let mut tape = body.transcript();
    tape.extend(signature.as_raw_bytes());
    (receipt, tape)
}

struct Unload {
    derivation: SuffixUnloadDerivationV1,
    sigma: KagemushaWalletStepProofV1,
    authority: [u8; 32],
    receipt: KagemushaWalletReceiptV1,
}

impl Unload {
    fn claims(&self) -> SuffixUnloadClaimsV1<'_> {
        let derivation = &self.derivation;
        SuffixUnloadClaimsV1 {
            authority_digest: self.authority,
            anchor_sequence: 0,
            step_count: RECEIVE_COUNT as u128,
            tip: derivation.predecessor_adjusted.head,
            tip_sequence: derivation.predecessor.core.sequence,
            tip_adjusted: &derivation.predecessor_adjusted,
            statement: &derivation.statement,
            sigma: &self.sigma,
            after: &derivation.successor,
            recovery: [&derivation.openings[0], &derivation.openings[1]],
            charge: None,
            receipt: &self.receipt,
        }
    }
}

/// Negative finish cases on fresh tips linked from the committed-style segment DATA.
fn finish_negatives(
    verifier: &SuffixVerifierV1<'_>,
    anchor: &SuffixAnchorPointV1,
    segment: &SuffixSegmentV1,
    unload: &Unload,
    credential: &KagemushaWalletCredentialV1,
) {
    let fresh = || verifier.link(anchor, &[*segment]).unwrap();
    let mut rejected = 0;
    let mut expect = |label: &str, result: Result<VerifiedSuffixUnloadV1, SuffixErrorV1>| {
        let error = result
            .err()
            .unwrap_or_else(|| panic!("{label} must be rejected"));
        assert!(error.is_rejection(), "{label}: {error}");
        eprintln!("SUFFIX_GATE_NEGATIVE {label} rejected={error}");
        rejected += 1;
    };
    // A changed authority digest: the receipt no longer signs it.
    let mut digest = unload.authority;
    digest[0] ^= 1;
    let claims = SuffixUnloadClaimsV1 {
        authority_digest: digest,
        ..unload.claims()
    };
    expect("changed_digest", verifier.finish(fresh(), claims, None));
    // A changed statement (amount): it no longer derives from the verified tip.
    let mut statement = unload.derivation.statement;
    if let KagemushaWalletEffectV1::Unload { amount, .. } = &mut statement.effect {
        *amount -= 1;
    }
    let claims = SuffixUnloadClaimsV1 {
        statement: &statement,
        ..unload.claims()
    };
    expect(
        "changed_statement_amount",
        verifier.finish(fresh(), claims, None),
    );
    // A changed source: forged tip adjusted credit root and step count.
    let mut adjusted = unload.derivation.predecessor_adjusted;
    adjusted.credit_digest_root = segment.start_adjusted.credit_digest_root;
    let claims = SuffixUnloadClaimsV1 {
        tip_adjusted: &adjusted,
        ..unload.claims()
    };
    expect(
        "forged_tip_credit_root",
        verifier.finish(fresh(), claims, None),
    );
    let mut adjusted = unload.derivation.predecessor_adjusted;
    adjusted.burned_total += 1;
    let claims = SuffixUnloadClaimsV1 {
        tip_adjusted: &adjusted,
        ..unload.claims()
    };
    expect(
        "forged_tip_burned_total",
        verifier.finish(fresh(), claims, None),
    );
    let claims = SuffixUnloadClaimsV1 {
        step_count: RECEIVE_COUNT as u128 - 1,
        ..unload.claims()
    };
    expect("changed_step_count", verifier.finish(fresh(), claims, None));
    // A wrong predecessor: the tip claim names the previous head.
    let claims = SuffixUnloadClaimsV1 {
        tip: segment.start,
        ..unload.claims()
    };
    expect("wrong_predecessor", verifier.finish(fresh(), claims, None));
    // A forged successor (balance) and a forged recovery root.
    let mut after = unload.derivation.successor;
    after.core.balance += 1;
    let claims = SuffixUnloadClaimsV1 {
        after: &after,
        ..unload.claims()
    };
    expect(
        "forged_successor_balance",
        verifier.finish(fresh(), claims, None),
    );
    let mut low = unload.derivation.openings[0].clone();
    let sibling = low.len() - 32;
    low[sibling] ^= 1;
    let claims = SuffixUnloadClaimsV1 {
        recovery: [&low, &unload.derivation.openings[1]],
        ..unload.claims()
    };
    expect(
        "forged_recovery_path",
        verifier.finish(fresh(), claims, None),
    );
    // A mutated σ and a receipt by another key.
    let mut sigma = unload.sigma.clone();
    sigma.bytes[40] ^= 1;
    let claims = SuffixUnloadClaimsV1 {
        sigma: &sigma,
        ..unload.claims()
    };
    expect("mutated_sigma", verifier.finish(fresh(), claims, None));
    let (other, _) = sign(
        credential,
        &unload.derivation.statement,
        &unload.authority,
        unload.receipt.capsule_commitment,
        OTHER_KEY,
    );
    let claims = SuffixUnloadClaimsV1 {
        receipt: &other,
        ..unload.claims()
    };
    expect(
        "wrong_receipt_signer",
        verifier.finish(fresh(), claims, None),
    );
    // An amount above the verified adjusted spendable balance cannot even be derived.
    let tip = fresh();
    let over = tip.point().0.core.balance + 1;
    let recovery = recovery_insertion(tip.point().0, over);
    assert!(
        verifier
            .derive_unload(&tip, over, &recovery, None, random_field())
            .unwrap_err()
            .is_rejection()
    );
    eprintln!("SUFFIX_GATE_NEGATIVE over_spendable_amount rejected=derive");
    assert_eq!(rejected, 10);
}

/// Repeated withdrawal: a second suffix carries the signed suffix Unload as a prior step and
/// withdraws again at the next redemption ordinal; replays of the first Unload are refused.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn repeated_withdrawal(
    verifier: &SuffixVerifierV1<'_>,
    sources: &QualifiedWalletSourcesV1,
    originals: &mut FixtureOriginals,
    preparation: &PreparationV1<'_>,
    owner: &iroha_core_zk::kagemusha_wallet_preparation_v1::AuthenticatedCredentialV1,
    anchor: &SuffixAnchorPointV1,
    segment: &SuffixSegmentV1,
    unload: &Unload,
    credential: &KagemushaWalletCredentialV1,
) {
    const SECOND: u128 = 20;
    let first = &unload.derivation;
    let prior = SuffixPriorUnloadInputV1 {
        before: &first.predecessor,
        after: &first.successor,
        statement: &first.statement,
        sigma: &unload.sigma,
        receipt: &unload.receipt,
        proof_digest: unload.authority,
        recovery: [&first.openings[0], &first.openings[1]],
        charge: None,
    };
    let mut cursor = verifier
        .cursor_at_claim(
            anchor.credential(),
            &first.predecessor,
            &first.predecessor_adjusted,
        )
        .unwrap();
    // A prior Unload whose receipt does not sign the claimed digest debits nothing.
    let error = cursor
        .verify_step(
            verifier,
            SuffixStepInputV1::PriorUnload(SuffixPriorUnloadInputV1 {
                proof_digest: [7; 32],
                ..prior
            }),
            None,
        )
        .unwrap_err();
    assert!(error.is_rejection(), "prior Unload wrong digest: {error}");
    eprintln!("SUFFIX_GATE_NEGATIVE prior_unload_wrong_digest rejected={error}");
    cursor
        .verify_step(verifier, SuffixStepInputV1::PriorUnload(prior), None)
        .unwrap();
    assert_eq!(cursor.point().0, &first.successor);
    assert_eq!(cursor.point().1, &first.successor_adjusted);
    // Replays of the same prior Unload: as is, and with its predecessor patched to the head.
    for (label, input) in [
        ("prior_unload_replay", prior),
        (
            "prior_unload_replay_patched_predecessor",
            SuffixPriorUnloadInputV1 {
                before: &first.successor,
                ..prior
            },
        ),
    ] {
        let error = cursor
            .verify_step(verifier, SuffixStepInputV1::PriorUnload(input), None)
            .unwrap_err();
        assert!(error.is_rejection(), "{label}: {error}");
        eprintln!("SUFFIX_GATE_NEGATIVE {label} rejected={error}");
    }
    let second_segment = cursor.segment().unwrap();
    let tip = verifier.link(anchor, &[*segment, second_segment]).unwrap();
    assert_eq!(
        tip.steps(),
        (RECEIVE_COUNT as u32 + 1, RECEIVE_COUNT as u32, 1)
    );
    // The second redeem leaf goes into the recovery map that already holds the first.
    let mut tree = KagemushaWalletIndexedTreeV1::new();
    for (ordinal, amount) in [(0, UNLOAD_AMOUNT), (1, SECOND)] {
        let leaf = KagemushaWalletRedeemLeafV1 {
            ordinal,
            nullifier: kagemusha_wallet_unload_nullifier_v1(
                &credential.body.scheme_id,
                &credential.body.wallet_id,
                ordinal,
            ),
            amount,
            online_charge: 0,
        };
        let witness = tree.insert(leaf.key(), leaf.leaf_value().unwrap()).unwrap();
        if ordinal == 0 {
            assert_eq!(tree.root(), first.successor.core.load_redeem_recovery_root);
            continue;
        }
        let derivation = verifier
            .derive_unload(&tip, SECOND, &witness, None, random_field())
            .unwrap();
        let KagemushaWalletEffectV1::Unload {
            redeem_ordinal,
            nullifier,
            ..
        } = derivation.statement.effect
        else {
            unreachable!()
        };
        assert_eq!(redeem_ordinal, 1);
        let KagemushaWalletEffectV1::Unload {
            nullifier: first_nullifier,
            ..
        } = first.statement.effect
        else {
            unreachable!()
        };
        assert_ne!(nullifier, first_nullifier, "a distinct payout nullifier");
        let witness_fields = ConsumingWitness {
            predecessor: preparation
                .state_fields(
                    owner,
                    &derivation.predecessor,
                    &derivation.predecessor_adjusted,
                )
                .unwrap(),
            successor: preparation
                .state_fields(owner, &derivation.successor, &derivation.successor_adjusted)
                .unwrap(),
            statement: preparation
                .statement_fields(owner, &derivation.statement)
                .unwrap(),
        };
        let sigma = {
            let ImportedSigmaV1::Unload(prover) =
                sources.import_sigma(8, originals, config()).unwrap()
            else {
                panic!("selector 8 is the Unload sigma")
            };
            preparation
                .prove_unload_sigma(
                    owner,
                    &witness_fields,
                    &derivation.statement,
                    &prover,
                    ProverRandomness::os(),
                    MemoryBudget::new(128 << 20),
                )
                .unwrap()
        };
        let mut transcript = b"engineering-native-suffix-repeat-gate-v1".to_vec();
        transcript.extend(second_segment.chain);
        transcript.extend(derivation.statement.statement_digest().unwrap());
        transcript.extend(&sigma.bytes);
        let authority =
            kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*b"kgwsufa1"), &transcript);
        let (receipt, _) = sign(
            credential,
            &derivation.statement,
            &authority,
            random_field(),
            MERCHANT_KEY,
        );
        let second = Unload {
            derivation,
            sigma,
            authority,
            receipt,
        };
        let claims = SuffixUnloadClaimsV1 {
            step_count: RECEIVE_COUNT as u128 + 1,
            ..second.claims()
        };
        // Replaying the first Unload's statement on the new tip is refused.
        let replay = SuffixUnloadClaimsV1 {
            statement: &first.statement,
            ..claims
        };
        let error = verifier
            .finish(
                verifier.link(anchor, &[*segment, second_segment]).unwrap(),
                replay,
                None,
            )
            .err()
            .unwrap();
        assert!(error.is_rejection(), "nullifier replay: {error}");
        eprintln!("SUFFIX_GATE_NEGATIVE replay_first_nullifier rejected={error}");
        let verified = verifier.finish(tip, claims, None).unwrap();
        assert_eq!(verified.nullifier(), (nullifier, 1));
        assert_eq!(verified.terms(), (SECOND, 0, [0; 32]));
        assert_eq!(
            verified.steps(),
            (RECEIVE_COUNT as u32 + 1, RECEIVE_COUNT as u32)
        );
        eprintln!(
            "SUFFIX_GATE repeated_withdrawal_verified=true prior_suffix_unload_step=true second_ordinal=1"
        );
        return;
    }
    unreachable!("the second redemption ran");
}

#[allow(clippy::too_many_lines)]
pub(super) fn run(corpus: &Corpus) {
    let started = Instant::now();
    let budget = MemoryBudget::new(128 << 20);
    // 1. The complete installed source grant: proving σ_unload and the later fold need it.
    let (sources, mut originals) = qualify(corpus);
    // 2. Ledger verification of all 100 genuine Receives from the genuine Bootstrap Omega.
    let verifier = SuffixVerifierV1::ledger(corpus.installed.verifier(), budget);
    let certificates = corpus.certificates();
    let anchor = verifier
        .verify_anchor(corpus.anchor_input(&certificates), None)
        .unwrap();
    let mut cursor = verifier.cursor_at_anchor(&anchor).unwrap();
    let steps = corpus.steps(RECEIVE_COUNT as u128);
    let mut points = Vec::with_capacity(RECEIVE_COUNT);
    let verify_started = Instant::now();
    for step in &steps {
        cursor.verify_step(&verifier, step.input(), None).unwrap();
        points.push(*cursor.point().1);
    }
    let verify_ms = verify_started.elapsed().as_millis();
    let segment = cursor.segment().unwrap();
    assert_eq!(segment.steps, RECEIVE_COUNT as u32);
    let native = SuffixVerifierV1::native(&corpus.installed, &sources, budget).unwrap();
    assert!(
        native
            .committed_anchor(corpus.anchor_input(&certificates))
            .err()
            .unwrap()
            .is_rejection(),
        "Native must verify its anchor proof"
    );
    assert!(
        native
            .link(&anchor, &[segment])
            .err()
            .unwrap()
            .is_rejection(),
        "Native cannot promote decoded consensus segment DATA"
    );
    let tip = cursor.into_tip().unwrap();
    eprintln!("SUFFIX_GATE ledger_suffix_verified receives={RECEIVE_COUNT} ms={verify_ms}");
    // 3. The Unload from the verified tip, its σ under the installed Unload key, the distinct
    // suffix-authority digest and the engineering hardware-equivalent receipt.
    let recovery = recovery_insertion(tip.point().0, UNLOAD_AMOUNT);
    let derivation = verifier
        .derive_unload(&tip, UNLOAD_AMOUNT, &recovery, None, random_field())
        .unwrap();
    let preparation = PreparationV1::new(&corpus.installed).unwrap();
    let issuer = certificates
        .certificate(
            &corpus.credential.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )
        .unwrap();
    let owner = preparation
        .authenticate_credential(
            &corpus.credential.to_canonical_bytes().unwrap(),
            &issuer.to_canonical_bytes().unwrap(),
        )
        .unwrap();
    let witness = ConsumingWitness {
        predecessor: preparation
            .state_fields(
                &owner,
                &derivation.predecessor,
                &derivation.predecessor_adjusted,
            )
            .unwrap(),
        successor: preparation
            .state_fields(
                &owner,
                &derivation.successor,
                &derivation.successor_adjusted,
            )
            .unwrap(),
        statement: preparation
            .statement_fields(&owner, &derivation.statement)
            .unwrap(),
    };
    let sigma = {
        let ImportedSigmaV1::Unload(prover) =
            sources.import_sigma(8, &mut originals, config()).unwrap()
        else {
            panic!("selector 8 is the Unload sigma")
        };
        preparation
            .prove_unload_sigma(
                &owner,
                &witness,
                &derivation.statement,
                &prover,
                ProverRandomness::os(),
                budget,
            )
            .unwrap()
    };
    let stages = super::stages::Stages::prepare(corpus, &steps, &points, &derivation, &sigma);
    let authority = stages.authority();
    let (receipt, receipt_tape) = sign(
        &corpus.credential,
        &derivation.statement,
        &authority,
        random_field(),
        MERCHANT_KEY,
    );
    let unload = Unload {
        derivation,
        sigma,
        authority,
        receipt,
    };
    let verified = verifier.finish(tip, unload.claims(), None).unwrap();
    assert_eq!(verified.authority_digest(), authority);
    assert_eq!(verified.receipt(), &receipt);
    assert_eq!(
        verified.steps(),
        (RECEIVE_COUNT as u32, RECEIVE_COUNT as u32)
    );
    assert_eq!(
        verified.terms(),
        (UNLOAD_AMOUNT, 0, [0; 32]),
        "amount, charge and quote"
    );
    assert_eq!(verified.nullifier().1, 0);
    stages.publish(corpus, &verified, &unload.derivation.openings);
    eprintln!(
        "SUFFIX_GATE signed_before_reading_receive_folds=true authority_digest={} capability=true",
        hex::encode(authority)
    );
    finish_negatives(&verifier, &anchor, &segment, &unload, &corpus.credential);
    repeated_withdrawal(
        &verifier,
        &sources,
        &mut originals,
        &preparation,
        &owner,
        &anchor,
        &segment,
        &unload,
        &corpus.credential,
    );

    // 4. Only now read the genuine Receive folds: every adjusted field equals the verifier's.
    for (index, expected) in points.iter().enumerate() {
        let sequence = (index + 1) as u128;
        let record = corpus.fold(sequence);
        assert_eq!(
            &record.lineage.public, expected,
            "verifier adjusted fields equal the genuine fold at {sequence}"
        );
    }
    let previous_fold = corpus.fold(RECEIVE_COUNT as u128);
    corpus
        .installed
        .verifier()
        .verify_lineage(&previous_fold.lineage, budget)
        .unwrap();
    assert_eq!(
        previous_fold.lineage.public,
        unload.derivation.predecessor_adjusted
    );
    let ordinary = kagemusha_wallet_proof_digest_v1(
        KagemushaWalletOperationKindV1::Unload,
        Some(&previous_fold.lineage),
        &unload.sigma,
    )
    .unwrap();
    assert_ne!(authority, ordinary, "the suffix digest is distinct");
    eprintln!("SUFFIX_GATE receive_folds_equal_verifier_fields=true count={RECEIVE_COUNT}");

    // 5. The unchanged Unload Q/A/Omega fold the unchanged original receipt under the
    // installed keys, from the genuine Omega(100).
    let credential = &corpus.credential;
    let state = ConsumingWitness {
        predecessor: preparation
            .state_fields(
                &owner,
                &unload.derivation.predecessor,
                &previous_fold.lineage.public,
            )
            .unwrap(),
        successor: preparation
            .state_fields(
                &owner,
                &unload.derivation.successor,
                &unload.derivation.successor_adjusted,
            )
            .unwrap(),
        statement: preparation
            .statement_fields(&owner, &unload.derivation.statement)
            .unwrap(),
    };
    let insertion = recovery;
    let recovery = IndexedInsert {
        leaf: IndexedLeaf {
            key: field(insertion.low.key),
            value: field(insertion.low.value),
            next_key: field(insertion.low.next_key),
        },
        leaf_slot: insertion.low_opening.slot,
        leaf_siblings: insertion.low_opening.siblings.map(field),
        slot: insertion.slot_opening.slot,
        slot_siblings: insertion.slot_opening.siblings.map(field),
    };
    let key = credential.body.payment_key.as_sec1_bytes();
    let body_len = ObjectKind::Receipt.body_len();
    assert_eq!(receipt_tape.len(), body_len + 64);
    let signature = SignatureWitness {
        digest: signed_message_native(ObjectKind::Receipt, &receipt_tape[..body_len]),
        key: [
            words_from_be(key[1..33].try_into().unwrap()),
            words_from_be(key[33..].try_into().unwrap()),
        ],
        signature: [
            words_from_be(receipt_tape[body_len..body_len + 32].try_into().unwrap()),
            words_from_be(receipt_tape[body_len + 32..].try_into().unwrap()),
        ],
    };
    let route = OperationRoute {
        variant: Variant::Unload,
        own: 8,
        incoming: None,
    };
    let q = {
        let ImportedQV1::Signature(prover) = sources
            .import_q(route, 0, &mut originals, config())
            .unwrap()
        else {
            panic!("Unload Q is the own receipt signature")
        };
        eprintln!("SUFFIX_GATE proving_q=true");
        prover
            .prove(
                &[signature],
                ProverRandomness::os(),
                ProverConfig::default(),
            )
            .unwrap()
    };
    let transport = &previous_fold.lineage.proof;
    let proof_len = transport.len() - 2 * 544;
    let input = consuming::Inputs {
        state,
        objects: [receipt_tape.clone()],
        recovery: Some(recovery),
        predecessor: consuming::PredecessorInput {
            proof: transport[..proof_len].to_vec(),
            pallas: transport[proof_len..proof_len + 544].try_into().unwrap(),
            vesta: transport[proof_len + 544..].try_into().unwrap(),
        },
        q: [consuming::QInput {
            proof: q.bytes,
            instances: q.instances.to_vec(),
        }],
    };
    let selected = sources.route(route).unwrap();
    let QualifiedOperationOwnerV1::Consuming(prover) = selected.owner() else {
        panic!("the Unload source is the consuming owner")
    };
    let session = prover.prepare(input.clone(), budget).unwrap();
    let terminal = {
        let key = sources
            .import_a(route, 0, &mut originals, config())
            .unwrap();
        eprintln!("SUFFIX_GATE proving_a_unload=true");
        session
            .terminal(
                &key,
                tape(),
                &FoldConfig::default(),
                ProverRandomness::os(),
                ProverConfig::default(),
            )
            .unwrap()
    };
    let checkpoint = session
        .encode_terminal_checkpoint(&terminal, budget)
        .unwrap();
    let restored = prover
        .prepare(input.clone(), budget)
        .unwrap()
        .restore_terminal_checkpoint(&checkpoint, budget)
        .unwrap();
    assert_eq!(restored.proof, terminal.proof);
    assert_eq!(restored.instances, terminal.instances);
    let public = input.state.successor.lineage[..18].try_into().unwrap();
    let omega = {
        let owner = sources.import_omega(&mut originals, config()).unwrap();
        eprintln!("SUFFIX_GATE proving_omega=true");
        owner
            .prepare(
                iroha_kagemusha_proof::omega::native::Input {
                    key: selected.terminal().key().clone(),
                    proof: restored.proof,
                    frame: restored.instances.try_into().unwrap(),
                    public,
                    pallas: restored.pallas,
                },
                tape(),
                budget,
            )
            .unwrap()
            .prove(ProverRandomness::os(), ProverConfig::default())
            .unwrap()
    };
    let lineage = KagemushaWalletLineageV1 {
        public: unload.derivation.successor_adjusted,
        proof: omega.transport(),
    };
    corpus
        .installed
        .verifier()
        .verify_lineage(&lineage, budget)
        .unwrap();
    let (_, successor_adjusted) = verified.successor();
    assert_eq!(&lineage.public, successor_adjusted);
    receipt
        .verify(
            &KagemushaWalletReceiptSignerV1::from_credential(credential).unwrap(),
            &unload.derivation.statement,
            &authority,
        )
        .unwrap();
    eprintln!(
        "SUFFIX_GATE unload_fold_verified=true predicted_successor_fields_equal=true receipt_unchanged=true"
    );
    // Public sources must refuse original-checkpoint adoption. The recovery siblings are
    // private circuit witnesses: a prior valid proof can be restored under unchanged public
    // fields, so exercise a FRESH proof and its complete verification for that mutation.
    // Native custody independently binds exact originals to the capsule and checkpoint chain.
    for mutation in 0..6 {
        let mut changed = input.clone();
        match mutation {
            0 => changed.objects[0][2 + 3 * 32 + 16 + 4 * 32] ^= 1,
            1 => changed.state.statement[19] += Fp::ONE,
            2 => changed.state.predecessor.lineage[5] += Fp::ONE,
            3 => {
                changed.state.successor.core
                    [iroha_kagemusha_proof::witness::core_index::LOAD_REDEEM_ROOT] += Fp::ONE
            }
            4 => changed.recovery.as_mut().unwrap().leaf_siblings[0] += Fp::ONE,
            _ => changed.q[0].proof[0] ^= 1,
        }
        let refused = match prover.prepare(changed, budget) {
            Err(_) => true,
            Ok(changed) if mutation == 4 => {
                let key = sources
                    .import_a(route, 0, &mut originals, config())
                    .unwrap();
                match changed.terminal(
                    &key,
                    tape(),
                    &FoldConfig::default(),
                    ProverRandomness::os(),
                    ProverConfig::default(),
                ) {
                    Err(consuming::Error::Proof) => {
                        eprintln!(
                            "SUFFIX_GATE_CIRCUIT_NEGATIVE private_recovery_fresh_proof=cryptographic_refusal"
                        );
                        true
                    }
                    Err(error) => panic!(
                        "private recovery mutation did not complete proof verification: {error:?}"
                    ),
                    Ok(invalid) => changed
                        .encode_terminal_checkpoint(&invalid, budget)
                        .is_err(),
                }
            }
            Ok(changed) => changed
                .restore_terminal_checkpoint(&checkpoint, budget)
                .is_err(),
        };
        assert!(
            refused,
            "changed source {mutation} must fail its complete proof owner"
        );
        eprintln!("SUFFIX_GATE_CIRCUIT_NEGATIVE mutation={mutation} refused=true");
    }
    if let Some(directory) = std::env::var_os("KAGEMUSHA_SUFFIX_STAGE_OUTPUT") {
        fs::write(
            PathBuf::from(directory).join("OMEGA-CONTINUATION-PASSED"),
            format!(
                "The exact suffix receipt continued through unchanged installed engineering Q/A/Omega originals. Authority: {}\n",
                hex::encode(authority)
            ),
        )
        .unwrap();
    }
    eprintln!(
        "SUFFIX_CONTINUATION installed_engineering_keys=true unchanged_q_a_omega=true predicted_fields_equal=true signed_receipt_unchanged=true repeated_withdrawal=true verifier_negatives=15 circuit_negatives=6 elapsed_ms={}",
        started.elapsed().as_millis()
    );
}
