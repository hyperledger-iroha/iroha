//! Native-suffix Unload stage orchestration. The suffix verifier double accepts by
//! construction and derives each chunk's end from its last step: these tests establish
//! staging, ordering, retries, conflicts and payout, never proof validity.
use super::*;
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1, KagemushaWalletSuffixAnchorV1 as Anchor,
    KagemushaWalletSuffixChunkV1 as Chunk, KagemushaWalletSuffixCreditWitnessV1 as Credit,
    KagemushaWalletSuffixDigestRoleV1 as Role, KagemushaWalletSuffixHeaderV1 as Header,
    KagemushaWalletSuffixOriginalRoleV1 as OriginalRole,
    KagemushaWalletSuffixOriginalV1 as Original, KagemushaWalletSuffixStepKindV1 as StepKind,
    KagemushaWalletSuffixStepV1 as Step, KagemushaWalletSuffixUnloadV1 as Unload,
    kagemusha_wallet_suffix_authority_digest_v1, kagemusha_wallet_suffix_digest_v1,
    kagemusha_wallet_suffix_payout_source_v1, kagemusha_wallet_suffix_recovery_witness_digest_v1,
};
use suffix::{StageId, SuffixTransaction};

const LOW: usize = KAGEMUSHA_WALLET_INDEXED_LEAF_OPENING_TRANSCRIPT_BYTES_V1;
const EMPTY: usize = KAGEMUSHA_WALLET_INDEXED_EMPTY_OPENING_TRANSCRIPT_BYTES_V1;
const MANIFEST: Digest = [0x61; 32];
const DEFINITION: Digest = [0x62; 32];

/// In-memory ledger with suffix rows and the exact proof work each stage reserved.
pub(super) struct Ledger {
    pub(super) memory: Memory,
    rows: BTreeMap<KagemushaWalletLedgerKeyV1, Vec<u8>>,
    reserved: Vec<(usize, u32)>,
    max_calls: u32,
}
impl Transaction for Ledger {
    fn authority(&self) -> &AccountId {
        self.memory.authority()
    }
    fn transaction_hash(&self) -> Digest {
        self.memory.transaction_hash()
    }
    fn block_height(&self) -> u64 {
        self.memory.block_height()
    }
    fn registration(&self, scheme: &Digest, asset: &Digest) -> Result<Registration> {
        self.memory.registration(scheme, asset)
    }
    fn wallet(&self, scheme: &Digest, wallet: &Digest) -> Result<Option<WalletRecord>> {
        self.memory.wallet(scheme, wallet)
    }
    fn issuance(
        &self,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
    ) -> Result<Option<Issuance>> {
        self.memory.issuance(scheme, wallet, request)
    }
    fn payout(
        &self,
        scheme: &Digest,
        key: KagemushaWalletPayoutKeyV1,
    ) -> Result<Option<KagemushaWalletPayoutRecordV1>> {
        self.memory.payout(scheme, key)
    }
    fn certificate(
        &self,
        scheme: &Digest,
        digest: &Digest,
    ) -> Result<KagemushaWalletSignerCertificateV1> {
        self.memory.certificate(scheme, digest)
    }
    fn fee_inputs(&self, claim: &KagemushaWalletFeeClaimV1) -> Result<FeeInputs> {
        self.memory.fee_inputs(claim)
    }
    fn apply(&mut self, batch: Batch) -> Result<()> {
        self.memory.apply(batch)
    }
}
impl SuffixTransaction for Ledger {
    fn suffix_row(&self, key: &KagemushaWalletLedgerKeyV1) -> Result<Option<Vec<u8>>> {
        self.rows
            .get(key)
            .map(|bytes| {
                validate_row(key, bytes)?;
                Ok(bytes.clone())
            })
            .transpose()
    }
    fn suffix_installation(&self, _: &Digest) -> Result<(Digest, Digest)> {
        Ok((MANIFEST, DEFINITION))
    }
    fn reserve_suffix_proofs(&mut self, proof_bytes: usize, verify_calls: u32) -> Result<()> {
        if verify_calls > self.max_calls {
            return Err(Error::Execution(
                iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
                    "verify calls per transaction exceeded".into(),
                ),
            ));
        }
        self.reserved.push((proof_bytes, verify_calls));
        Ok(())
    }
    fn insert_suffix(&mut self, rows: Vec<(KagemushaWalletLedgerKeyV1, Vec<u8>)>) -> Result<()> {
        for (key, bytes) in &rows {
            validate_row(key, bytes)?;
            if self.rows.get(key).is_some_and(|prior| prior != bytes) {
                return Err(Error::Conflict);
            }
        }
        self.rows.extend(rows);
        Ok(())
    }
}

/// Suffix verifier double: accepts every stage the ledger presents and returns, for a
/// chunk, its claimed start with the head of the last step's successor state.
pub(super) struct Double {
    pub(super) accept: bool,
    pub(super) calls: Cell<usize>,
}
impl Double {
    pub(super) fn new(accept: bool) -> Self {
        Self {
            accept,
            calls: Cell::new(0),
        }
    }
    fn result(&self) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        if self.accept {
            Ok(())
        } else {
            Err(Error::Proof)
        }
    }
}
impl NativePackageVerifier for Double {
    fn verify(
        &self,
        _: &KagemushaWalletSchemeV1,
        _: &KagemushaWalletCredentialV1,
        _: &KagemushaWalletPackageV1,
    ) -> Result<()> {
        Err(Error::Proof)
    }
}
impl SuffixUnloadVerifier for Double {
    fn verify_begin(&self, _: &KagemushaWalletSchemeV1, _: &Header, _: &Anchor) -> Result<()> {
        self.result()
    }
    fn verify_chunk(
        &self,
        _: &KagemushaWalletSchemeV1,
        header: &Header,
        _: &Anchor,
        start: &KagemushaWalletLineagePublicV1,
        steps: &[Step],
    ) -> Result<SuffixChunkVerdict> {
        self.result()?;
        assert_eq!(steps[0].before.commitment().unwrap(), start.head);
        let mut end = *start;
        end.head = steps.last().unwrap().after.commitment().unwrap();
        let segment = iroha_core_zk::kagemusha_wallet_suffix_unload_v1::SuffixSegmentV1 {
            version: 1,
            scheme_id: header.scheme_id,
            manifest_digest: header.artifact_manifest,
            credential_digest: start.credential_digest,
            start: start.head,
            start_sequence: steps[0].before.core.sequence,
            start_adjusted: *start,
            end: steps.last().unwrap().after,
            end_adjusted: end,
            steps: u32::try_from(steps.len()).unwrap(),
            receives: u32::try_from(steps.len()).unwrap(),
            prior_unloads: 0,
            chain: [1; 32],
        };
        Ok(SuffixChunkVerdict {
            end,
            record: segment.to_canonical_bytes().unwrap(),
        })
    }
    fn verify_finalize(
        &self,
        _: &KagemushaWalletSchemeV1,
        header: &Header,
        authority: &Digest,
        _: &Anchor,
        records: &[Vec<u8>],
        tip: &KagemushaWalletStateV1,
        _: &Unload,
    ) -> Result<()> {
        assert_eq!(tip.commitment().unwrap(), header.tip);
        assert_eq!(
            *authority,
            kagemusha_wallet_suffix_authority_digest_v1(&header.to_canonical_bytes().unwrap())
                .unwrap()
        );
        // Records arrive in ordinal order: the last names the tip.
        assert_eq!(records.len() as u128, header.chunk_count);
        let last =
            iroha_core_zk::kagemusha_wallet_suffix_unload_v1::SuffixSegmentV1::decode_canonical(
                records.last().unwrap(),
            )
            .unwrap();
        assert_eq!(last.end_adjusted.head, header.tip);
        self.result()
    }
}

/// Self-consistent suffix of the fixture's charged Unload claim: the payer's anchor at
/// sequence 2, three Receive steps in two chunks (2 + 1) and the Unload from sequence 5.
pub(super) struct Suffix {
    pub(super) scheme: Digest,
    pub(super) wallet: Digest,
    pub(super) authority: Digest,
    pub(super) account: AccountId,
    pub(super) header: Vec<u8>,
    pub(super) anchor: Vec<u8>,
    pub(super) chunks: Vec<Vec<u8>>,
    pub(super) unload: Vec<u8>,
    pub(super) terms: kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixPayoutTermsV1,
    pub(super) beneficiary: AccountId,
}

fn state_at(base: &KagemushaWalletStateV1, sequence: u128) -> KagemushaWalletStateV1 {
    let mut state = *base;
    state.core.sequence = sequence;
    state.core.balance = 1_000 + sequence;
    state
}

/// One Receive step frame; `full` pads every opaque original to its role's maximum.
fn step(before: &KagemushaWalletStateV1, after: &KagemushaWalletStateV1, full: bool) -> Vec<u8> {
    let package: KagemushaWalletPackageV1 = fixture("KagemushaWalletPackageV1");
    let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    let mut statement = package.statement;
    statement.sequence = after.core.sequence;
    statement.predecessor = before.commitment().unwrap();
    statement.successor = after.commitment().unwrap();
    let size = |role: OriginalRole, small: usize| if full { role.max_bytes() } else { small };
    let originals = [
        (
            OriginalRole::Request,
            vec![1; size(OriginalRole::Request, 900)],
        ),
        (OriginalRole::Payment, payment.to_canonical_bytes().unwrap()),
        (
            OriginalRole::PayerCredential,
            vec![3; size(OriginalRole::PayerCredential, 600)],
        ),
        (
            OriginalRole::PayerCertificates,
            vec![4; size(OriginalRole::PayerCertificates, 230)],
        ),
    ]
    .into_iter()
    .map(|(role, bytes)| Original { role, bytes })
    .collect();
    Step {
        version: 1,
        kind: StepKind::Receive {
            credit: Credit::Inserted {
                low: vec![0; LOW],
                empty: vec![0; EMPTY],
            },
        },
        before: *before,
        after: *after,
        statement,
        sigma: package.step_proof,
        receipt: package.receipt,
        map_openings: vec![vec![0; LOW], vec![0; EMPTY]],
        originals,
    }
    .to_canonical_bytes()
    .unwrap()
}

pub(super) fn build_suffix(network_id: Digest) -> Suffix {
    build_suffix_with(network_id, &[2, 1], false)
}

/// The fixture claim's suffix with `partition[i]` steps in chunk `i`, after an anchor at
/// sequence 2; the Unload statement moves to the tip.
pub(super) fn build_suffix_with(network_id: Digest, partition: &[usize], full: bool) -> Suffix {
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let credential = claim.credential;
    let scheme = credential.body.scheme_id;
    let wallet = credential.body.wallet_id;
    let base = KagemushaWalletStateV1::bootstrap(&credential, [9; 32]).unwrap();
    let total: usize = partition.iter().sum();
    let states: Vec<_> = (2..=(3 + total as u128))
        .map(|sequence| state_at(&base, sequence))
        .collect();
    let omega = claim.package.lineage.lineage().unwrap().clone();
    let adjusted = |state: &KagemushaWalletStateV1| {
        let mut public = omega.public;
        public.head = state.commitment().unwrap();
        public
    };
    let anchor = Anchor {
        version: 1,
        credential,
        certificates: KagemushaWalletCertificateSetV1::new(vec![
            *claim
                .certificates
                .certificate(
                    &credential.body.issuer_certificate,
                    KagemushaWalletSignerRoleV1::Enrollment,
                )
                .unwrap(),
        ])
        .unwrap(),
        lineage: KagemushaWalletLineageV1 {
            public: adjusted(&states[0]),
            proof: omega.proof.clone(),
        },
        state: states[0],
    }
    .to_canonical_bytes()
    .unwrap();
    let mut chunks = Vec::new();
    let mut from = 0;
    let mut stream_bytes = 0_u128;
    for (index, count) in partition.iter().enumerate() {
        let frames: Vec<_> = (from..from + count)
            .map(|i| step(&states[i], &states[i + 1], full))
            .collect();
        let chunk = Chunk {
            version: 1,
            chunk_index: index as u128,
            stream_offset: stream_bytes,
            start: adjusted(&states[from]),
            data: Chunk::stream(&frames).unwrap(),
        };
        stream_bytes += chunk.data.len() as u128;
        chunks.push(chunk.to_canonical_bytes().unwrap());
        from += count;
    }
    let mut statement = claim.package.statement;
    statement.sequence = 3 + total as u128;
    statement.predecessor = states[total].commitment().unwrap();
    statement.successor = states[total + 1].commitment().unwrap();
    let map_openings = vec![vec![0; LOW], vec![0; EMPTY]];
    let header = Header {
        version: 1,
        method: 1,
        network_id,
        scheme_id: scheme,
        wallet_id: wallet,
        verifier_definition: DEFINITION,
        artifact_manifest: MANIFEST,
        source_capsule: [0x71; 32],
        request_id: [0x72; 32],
        anchor_digest: kagemusha_wallet_suffix_digest_v1(Role::Anchor, &anchor),
        anchor_sequence: 2,
        step_count: total as u128,
        chunk_count: partition.len() as u128,
        stream_bytes,
        chunk_digests: chunks
            .iter()
            .map(|frame| kagemusha_wallet_suffix_digest_v1(Role::Chunk, frame))
            .collect(),
        tip: states[total].commitment().unwrap(),
        tip_sequence: 2 + total as u128,
        tip_adjusted: adjusted(&states[total]),
        statement,
        sigma: claim.package.step_proof.clone(),
        recovery_witness_digest: kagemusha_wallet_suffix_recovery_witness_digest_v1(&map_openings)
            .unwrap(),
        account: claim.account.clone(),
        charge: claim.charge.clone(),
    };
    let terms = header.payout_terms().unwrap();
    let header = header.to_canonical_bytes().unwrap();
    let unload = Unload {
        version: 1,
        credential,
        certificates: claim.certificates.clone(),
        after: states[total + 1],
        receipt: claim.package.receipt,
        map_openings,
    }
    .to_canonical_bytes()
    .unwrap();
    Suffix {
        scheme,
        wallet,
        authority: kagemusha_wallet_suffix_authority_digest_v1(&header).unwrap(),
        account: claim.account,
        header,
        anchor,
        chunks,
        unload,
        terms,
        beneficiary: claim.charge.beneficiary().unwrap().clone(),
    }
}

impl Suffix {
    pub(super) fn stage(&self, ordinal: u128) -> StageId<'_> {
        StageId {
            scheme: self.scheme,
            wallet: self.wallet,
            authority: self.authority,
            account: &self.account,
            ordinal,
        }
    }
}

fn ledger() -> (Ledger, Suffix) {
    let mut memory = Memory::new();
    let suffix = build_suffix(memory.registration.scheme.network_id);
    memory.wallets.insert(
        suffix.wallet,
        WalletRecord {
            asset: memory.registration.asset.asset_digest(),
            phase: Phase::Active,
            activation: [1; 32],
            next_load: 0,
        },
    );
    memory
        .balances
        .insert(memory.registration.reserve.clone(), 10_000);
    (
        Ledger {
            memory,
            rows: BTreeMap::new(),
            reserved: Vec::new(),
            max_calls: 128,
        },
        suffix,
    )
}

fn run_all(
    tx: &mut Ledger,
    verifier: &Double,
    suffix: &Suffix,
    order: &[usize],
) -> Result<KagemushaWalletPayoutRecordV1> {
    suffix::begin(
        tx,
        verifier,
        &suffix.stage(0),
        &suffix.header,
        &suffix.anchor,
    )?;
    for index in order {
        suffix::append(
            tx,
            verifier,
            &suffix.stage(*index as u128 + 1),
            &suffix.chunks[*index],
        )?;
    }
    suffix::finalize(tx, verifier, &suffix.stage(3), &suffix.unload)
}

#[test]
fn suffix_unload_pays_once_after_every_stage_in_order() {
    let (mut tx, suffix) = ledger();
    let verifier = Double::new(true);
    let reserve = tx.memory.registration.reserve.clone();
    let payout = run_all(&mut tx, &verifier, &suffix, &[0, 1]).unwrap();
    assert_eq!(
        payout.key,
        KagemushaWalletPayoutKeyV1::Unload(suffix.terms.nullifier)
    );
    assert_eq!(payout.amount, suffix.terms.amount);
    assert_eq!(payout.transaction, tx.memory.transaction_hash);
    let unload = kagemusha_wallet_suffix_digest_v1(Role::Unload, &suffix.unload);
    assert_eq!(
        payout.source,
        kagemusha_wallet_suffix_payout_source_v1(&suffix.authority, &unload)
    );
    assert_eq!(tx.memory.balances[&reserve], 10_000 - suffix.terms.amount);
    assert_eq!(
        tx.memory.balances[&suffix.beneficiary],
        suffix.terms.online_charge
    );
    assert_eq!(
        tx.memory.balances[&suffix.account],
        1_000 + suffix.terms.account_payout
    );
    assert_eq!(tx.memory.writes, 1);
    assert_eq!(verifier.calls.get(), 4);
    // Begin: anchor Ω + early Unload σ; each Receive: own σ + payer σ_send + payer Ω;
    // Finalize: the Unload σ (the committed Begin already verified Ω).
    assert_eq!(
        tx.reserved
            .iter()
            .map(|(_, calls)| calls)
            .collect::<Vec<_>>(),
        [&2, &6, &3, &1]
    );
    assert_eq!(tx.rows.len(), 4);
    // Exact retries of every stage are applied no-ops: no work, no verification, no transfer.
    suffix::begin(
        &mut tx,
        &verifier,
        &suffix.stage(0),
        &suffix.header,
        &suffix.anchor,
    )
    .unwrap();
    for (index, chunk) in suffix.chunks.iter().enumerate() {
        suffix::append(&mut tx, &verifier, &suffix.stage(index as u128 + 1), chunk).unwrap();
    }
    tx.memory.transaction_hash = [43; 32];
    assert_eq!(
        suffix::finalize(&mut tx, &verifier, &suffix.stage(3), &suffix.unload).unwrap(),
        payout
    );
    assert_eq!(verifier.calls.get(), 4);
    assert_eq!(tx.reserved.len(), 4);
    assert_eq!(tx.memory.writes, 1);
    assert_eq!(tx.memory.balances[&reserve], 10_000 - suffix.terms.amount);
    for (key, bytes) in &tx.rows {
        validate_row(key, bytes).unwrap();
    }
}

#[test]
fn appends_execute_in_any_order_and_finalize_waits_for_all() {
    let (mut tx, suffix) = ledger();
    let verifier = Double::new(true);
    // Neither an Append nor a Finalize before its Begin has any effect; both can be resent.
    assert!(matches!(
        suffix::append(&mut tx, &verifier, &suffix.stage(1), &suffix.chunks[0]),
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        suffix::finalize(&mut tx, &verifier, &suffix.stage(3), &suffix.unload),
        Err(Error::Unavailable)
    ));
    suffix::begin(
        &mut tx,
        &verifier,
        &suffix.stage(0),
        &suffix.header,
        &suffix.anchor,
    )
    .unwrap();
    suffix::append(&mut tx, &verifier, &suffix.stage(2), &suffix.chunks[1]).unwrap();
    // A Finalize before every Append is committed is refused without effect.
    assert!(matches!(
        suffix::finalize(&mut tx, &verifier, &suffix.stage(3), &suffix.unload),
        Err(Error::Unavailable)
    ));
    assert_eq!(tx.memory.writes, 0);
    suffix::append(&mut tx, &verifier, &suffix.stage(1), &suffix.chunks[0]).unwrap();
    let payout = suffix::finalize(&mut tx, &verifier, &suffix.stage(3), &suffix.unload).unwrap();
    let (mut ordered, _) = ledger();
    assert_eq!(
        run_all(&mut ordered, &verifier, &suffix, &[0, 1]).unwrap(),
        payout
    );
    assert_eq!(ordered.rows, tx.rows);
}

#[test]
fn a_stage_is_only_the_chunk_the_header_lists() {
    let (mut tx, suffix) = ledger();
    let verifier = Double::new(true);
    suffix::begin(
        &mut tx,
        &verifier,
        &suffix.stage(0),
        &suffix.header,
        &suffix.anchor,
    )
    .unwrap();
    // Swapped chunks and a re-partition of the same genuine steps are refused.
    assert!(matches!(
        suffix::append(&mut tx, &verifier, &suffix.stage(1), &suffix.chunks[1]),
        Err(Error::Binding)
    ));
    let mut repartitioned = Chunk::decode_canonical(&suffix.chunks[0], &suffix.scheme).unwrap();
    let first = repartitioned.step_frames().unwrap()[0].to_vec();
    repartitioned.data = Chunk::stream(&[first]).unwrap();
    assert!(matches!(
        suffix::append(
            &mut tx,
            &verifier,
            &suffix.stage(1),
            &repartitioned.to_canonical_bytes().unwrap()
        ),
        Err(Error::Binding)
    ));
    assert!(matches!(
        suffix::append(&mut tx, &verifier, &suffix.stage(3), &suffix.chunks[0]),
        Err(Error::Binding)
    ));
    assert_eq!(verifier.calls.get(), 1);
    assert_eq!(tx.rows.len(), 1);
}

#[test]
fn outer_identifiers_must_equal_the_begin_header() {
    let (mut tx, suffix) = ledger();
    let verifier = Double::new(true);
    let other = account_of(77);
    for stage in [
        StageId {
            ordinal: 1,
            ..suffix.stage(0)
        },
        StageId {
            wallet: [5; 32],
            ..suffix.stage(0)
        },
        StageId {
            authority: [5; 32],
            ..suffix.stage(0)
        },
        StageId {
            account: &other,
            ..suffix.stage(0)
        },
    ] {
        assert!(matches!(
            suffix::begin(&mut tx, &verifier, &stage, &suffix.header, &suffix.anchor),
            Err(Error::Binding)
        ));
    }
    run_all(&mut tx, &verifier, &suffix, &[0]).unwrap_err();
    assert!(matches!(
        suffix::append(
            &mut tx,
            &verifier,
            &StageId {
                account: &other,
                ..suffix.stage(2)
            },
            &suffix.chunks[1]
        ),
        Err(Error::Binding)
    ));
    suffix::append(&mut tx, &verifier, &suffix.stage(2), &suffix.chunks[1]).unwrap();
    for stage in [
        StageId {
            ordinal: 2,
            ..suffix.stage(3)
        },
        StageId {
            ordinal: 4,
            ..suffix.stage(3)
        },
        StageId {
            wallet: [5; 32],
            ..suffix.stage(3)
        },
    ] {
        assert!(suffix::finalize(&mut tx, &verifier, &stage, &suffix.unload).is_err());
    }
    assert_eq!(tx.memory.writes, 0);
}

fn account_of(seed: u8) -> AccountId {
    AccountId::new(
        iroha_crypto::KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

#[test]
fn the_nullifier_pays_one_claim_and_other_claims_conflict() {
    let (mut tx, suffix) = ledger();
    let verifier = Double::new(true);
    // An ordinary Unload already paid this nullifier.
    let key = KagemushaWalletPayoutKeyV1::Unload(suffix.terms.nullifier);
    tx.memory.payouts.insert(
        key,
        KagemushaWalletPayoutRecordV1 {
            key,
            source: [3; 32],
            amount: suffix.terms.amount,
            transaction: [4; 32],
        },
    );
    assert!(matches!(
        run_all(&mut tx, &verifier, &suffix, &[0, 1]),
        Err(Error::Conflict)
    ));
    assert_eq!(tx.memory.writes, 0);
    // After a suffix payout, another Finalize frame for the same authority conflicts.
    let (mut tx, suffix) = ledger();
    run_all(&mut tx, &verifier, &suffix, &[1, 0]).unwrap();
    let mut other = Unload::decode_canonical(&suffix.unload, &suffix.scheme).unwrap();
    other.map_openings[0][0] = 1;
    assert!(matches!(
        suffix::finalize(
            &mut tx,
            &verifier,
            &suffix.stage(3),
            &other.to_canonical_bytes().unwrap()
        ),
        Err(Error::Conflict)
    ));
    // A conflicting Begin original under the same authority is refused.
    let mut anchor = Anchor::decode_canonical(&suffix.anchor, &suffix.scheme).unwrap();
    anchor.lineage.proof[0] ^= 1;
    assert!(matches!(
        suffix::begin(
            &mut tx,
            &verifier,
            &suffix.stage(0),
            &suffix.header,
            &anchor.to_canonical_bytes().unwrap()
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(tx.memory.writes, 1);
}

#[test]
fn rejected_verification_quota_or_reserve_leaves_no_effect() {
    let (mut tx, suffix) = ledger();
    let accept = Double::new(true);
    let reject = Double::new(false);
    assert!(matches!(
        suffix::begin(
            &mut tx,
            &reject,
            &suffix.stage(0),
            &suffix.header,
            &suffix.anchor
        ),
        Err(Error::Proof)
    ));
    assert!(tx.rows.is_empty());
    suffix::begin(
        &mut tx,
        &accept,
        &suffix.stage(0),
        &suffix.header,
        &suffix.anchor,
    )
    .unwrap();
    assert!(matches!(
        suffix::append(&mut tx, &reject, &suffix.stage(1), &suffix.chunks[0]),
        Err(Error::Proof)
    ));
    tx.max_calls = 5;
    assert!(matches!(
        suffix::append(&mut tx, &accept, &suffix.stage(1), &suffix.chunks[0]),
        Err(Error::Execution(_))
    ));
    assert_eq!(accept.calls.get(), 1);
    tx.max_calls = 128;
    suffix::append(&mut tx, &accept, &suffix.stage(1), &suffix.chunks[0]).unwrap();
    suffix::append(&mut tx, &accept, &suffix.stage(2), &suffix.chunks[1]).unwrap();
    assert!(matches!(
        suffix::finalize(&mut tx, &reject, &suffix.stage(3), &suffix.unload),
        Err(Error::Proof)
    ));
    let reserve = tx.memory.registration.reserve.clone();
    tx.memory.balances.insert(reserve, suffix.terms.amount - 1);
    assert!(matches!(
        suffix::finalize(&mut tx, &accept, &suffix.stage(3), &suffix.unload),
        Err(Error::InsufficientFunds)
    ));
    assert_eq!(tx.memory.writes, 0);
    assert!(tx.memory.payouts.is_empty());
    assert_eq!(tx.rows.len(), 3);
}

#[test]
fn restored_suffix_rows_are_validated_exactly() {
    let (mut tx, suffix) = ledger();
    run_all(&mut tx, &Double::new(true), &suffix, &[0, 1]).unwrap();
    for (key, bytes) in &tx.rows {
        validate_row(key, bytes).unwrap();
        let (kind, scheme, owner, entry) = key.components();
        // Another owner, entry or one changed byte is refused.
        // A stage row binds its authority only through its Begin (snapshot cross-link).
        let mut owner_moved = owner;
        owner_moved[0] ^= 1;
        assert_eq!(
            validate_row(
                &KagemushaWalletLedgerKeyV1::from_parts(kind, scheme, owner_moved, entry),
                bytes
            )
            .is_err(),
            kind != suffix::STAGE
        );
        let mut entry_moved = entry;
        entry_moved[31] ^= 1;
        assert!(
            validate_row(
                &KagemushaWalletLedgerKeyV1::from_parts(kind, scheme, owner, entry_moved),
                bytes
            )
            .is_err()
        );
        let mut truncated = bytes.clone();
        truncated.pop();
        assert!(validate_row(key, &truncated).is_err());
    }
}

#[test]
fn restored_stage_rejects_substituted_verifier_record() {
    let (mut tx, suffix) = ledger();
    run_all(&mut tx, &Double::new(true), &suffix, &[0, 1]).unwrap();
    let key = suffix::stage_key(suffix.scheme, suffix.authority, 1);
    let original: suffix::SuffixStage = storage::decode(&tx.rows[&key], suffix::STAGE_CAP).unwrap();
    let segment =
        iroha_core_zk::kagemusha_wallet_suffix_unload_v1::SuffixSegmentV1::decode_canonical(
            &original.record,
        )
        .unwrap();
    for mutation in 0..7 {
        let mut changed = segment;
        match mutation {
            0 => changed.scheme_id[0] ^= 1,
            1 => changed.credential_digest[0] ^= 1,
            2 => changed.start_sequence += 1,
            3 => changed.steps += 1,
            4 => changed.end.core.balance += 1,
            5 => changed.prior_unloads += 1,
            _ => changed.chain = [0; 32],
        }
        let mut row = original.clone();
        row.record = changed.to_canonical_bytes().unwrap();
        assert!(
            validate_row(&key, &storage::encode(&row).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
    // A valid record from a foreign installation still fails the same-snapshot Begin join.
    let mut changed = segment;
    changed.manifest_digest[0] ^= 1;
    let mut row = original;
    row.record = changed.to_canonical_bytes().unwrap();
    let encoded = storage::encode(&row).unwrap();
    validate_row(&key, &encoded).unwrap();
    assert!(
        suffix::validate_snapshot_row(&key, &encoded, &|key| tx.rows.get(key).map(Vec::as_slice))
            .is_err()
    );
}

/// Ledger-side cost of the largest stages, around a verifier double that does no work: an
/// upper bound on Core's own share of a stage's execution (debug builds overstate it).
#[test]
#[ignore = "timing report; run explicitly, ideally optimized"]
fn ledger_side_cost_of_the_largest_stages() {
    let mut memory = Memory::new();
    let suffix = build_suffix_with(
        memory.registration.scheme.network_id,
        &[KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1; 2],
        true,
    );
    memory.wallets.insert(
        suffix.wallet,
        WalletRecord {
            asset: memory.registration.asset.asset_digest(),
            phase: Phase::Active,
            activation: [1; 32],
            next_load: 0,
        },
    );
    memory
        .balances
        .insert(memory.registration.reserve.clone(), 10_000);
    let mut tx = Ledger {
        memory,
        rows: BTreeMap::new(),
        reserved: Vec::new(),
        max_calls: 128,
    };
    let verifier = Double::new(true);
    let time = |label: &str, run: &mut dyn FnMut()| {
        let started = std::time::Instant::now();
        run();
        eprintln!(
            "SUFFIX_LEDGER_COST stage={label} micros={}",
            started.elapsed().as_micros()
        );
    };
    eprintln!(
        "SUFFIX_LEDGER_COST header_bytes={} anchor_bytes={} chunk_bytes={} unload_bytes={}",
        suffix.header.len(),
        suffix.anchor.len(),
        suffix.chunks[0].len(),
        suffix.unload.len()
    );
    time("begin", &mut || {
        suffix::begin(
            &mut tx,
            &verifier,
            &suffix.stage(0),
            &suffix.header,
            &suffix.anchor,
        )
        .unwrap()
    });
    time("append_8_steps", &mut || {
        suffix::append(&mut tx, &verifier, &suffix.stage(1), &suffix.chunks[0]).unwrap()
    });
    time("append_8_steps_second", &mut || {
        suffix::append(&mut tx, &verifier, &suffix.stage(2), &suffix.chunks[1]).unwrap()
    });
    time("append_exact_retry", &mut || {
        suffix::append(&mut tx, &verifier, &suffix.stage(1), &suffix.chunks[0]).unwrap()
    });
    time("finalize", &mut || {
        suffix::finalize(&mut tx, &verifier, &suffix.stage(3), &suffix.unload).unwrap();
    });
}
