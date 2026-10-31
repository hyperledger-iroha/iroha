//! Deterministic original input supplier over the production encrypted software wallet.
//!
//! Only explicit initial preparation reproduces the fixture's actually activated notes.
//! Production custody owns reservation, original-round reopen, input exclusion and atomic
//! verified-success cleanup. Recover never invokes the deterministic input supplier.
//! Fixture signing seeds are test-only, not a production deterministic-key recommendation.
use super::*;
use iroha::client::{PrivateSettlementRequest, VerifiedPrivateSettlementOutcome};
use iroha::workflow::Digest32;
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    AtomicPrivateSettlementAuthorizedWalletLegV1, plan_atomic_private_settlement_initial_inputs_v1,
    prepare_atomic_private_settlement_authorization_inputs_v1,
};
use iroha_data_model::nexus::PrivateSettlementProofIntentV1;
use iroha_private_settlement::{
    FilePrivateSettlementWallet, MAX_PRIVATE_SETTLEMENT_WALLET_BYTES,
    OneCallPrivateSettlementError, PrivateSettlementBusinessTerms,
    PrivateSettlementRestrictedOpening, PrivateSettlementWallet,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(super) const MAX_WALLET_INDEX_BYTES: usize = MAX_PRIVATE_SETTLEMENT_WALLET_BYTES;
/// Exact parent-selected restricted openings, transported only through bounded runtime stdin.
/// Hybrid audit keys are generated once by the original pool owner, never regenerated on resume.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::OriginalOpening")]
pub(super) struct OriginalOpening {
    pub(super) governance: PrivateSettlementPoolGovernanceV1,
    pub(super) policy: PrivateSettlementAuditPolicyV1,
}
pub(super) fn original_openings(governed: &[GovernedLeg]) -> Vec<OriginalOpening> {
    governed
        .iter()
        .map(|leg| OriginalOpening {
            governance: leg.governance.clone(),
            policy: leg.policy.clone(),
        })
        .collect()
}
pub(super) struct OriginalWallet {
    originals: Vec<OriginalOpening>,
    custody: FilePrivateSettlementWallet,
    enabled: AtomicBool,
    pub(super) openings: AtomicUsize,
    pub(super) stages: AtomicUsize,
    pub(super) reservations_called: AtomicUsize,
    pub(super) reconciliations: AtomicUsize,
}
impl OriginalWallet {
    pub(super) fn open(
        originals: Vec<OriginalOpening>,
        path: &std::path::Path,
        key: &[u8; 32],
    ) -> Result<Self> {
        ensure!(
            originals.len() == PARTICIPANT_COUNT,
            "exact original wallet opening count required"
        );
        for opening in &originals {
            opening.governance.validate()?;
            opening.policy.validate()?;
        }
        Ok(Self {
            originals,
            custody: FilePrivateSettlementWallet::open(path, key, MAX_WALLET_INDEX_BYTES)?,
            enabled: AtomicBool::new(false),
            openings: AtomicUsize::new(0),
            stages: AtomicUsize::new(0),
            reservations_called: AtomicUsize::new(0),
            reconciliations: AtomicUsize::new(0),
        })
    }
    pub(super) fn enable_explicit_continuation(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }
    fn terms(
        &self,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<(&OriginalOpening, PrivateSettlementLegPrivateData)> {
        ensure!(
            self.enabled.load(Ordering::SeqCst),
            "read-only recovery invoked the original input supplier"
        );
        let governed = self
            .originals
            .get(usize::from(ordinal))
            .ok_or_else(|| eyre!("wallet leg absent"))?;
        let data = default_private_settlement_leg_data(usize::from(ordinal));
        ensure!(
            terms.asset == governed.governance.body.asset_definition_id
                && terms.payer == AccountId::new(data.payer.public_key().clone())
                && terms.recipient == AccountId::new(data.recipient.public_key().clone())
                && terms.amount == data.amount
                && terms.reimbursement
                    == if ordinal == intent.reimbursement_leg_ordinal {
                        5
                    } else {
                        0
                    }
                && terms.memo == data.memo,
            "independent original wallet terms changed"
        );
        Ok((governed, data))
    }
}
impl PrivateSettlementWallet for OriginalWallet {
    fn open_policy(
        &self,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError>
    {
        self.openings.fetch_add(1, Ordering::SeqCst);
        let (opening, authorized) = (|| -> Result<_> {
            let (governed, data) = self.terms(intent, ordinal, terms)?;
            let funding = private_settlement_funding(
                intent.network_id,
                &governed.governance,
                usize::from(ordinal),
                &data,
            )?;
            let authorized =
                authorize_original(intent, usize::from(ordinal), governed, &data, funding)?;
            Ok((
                PrivateSettlementRestrictedOpening {
                    governance: governed.governance.clone(),
                    policy: governed.policy.clone(),
                },
                authorized,
            ))
        })()
        .map_err(OneCallPrivateSettlementError::Provider)?;
        self.custody
            .stage_authorized(intent, ordinal, terms, opening, authorized)?;
        self.stages.fetch_add(1, Ordering::SeqCst);
        self.custody.open_policy(intent, ordinal, terms)
    }
    fn reserve_or_reopen(
        &self,
        round: Digest32,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        OneCallPrivateSettlementError,
    > {
        self.reservations_called.fetch_add(1, Ordering::SeqCst);
        if !self.enabled.load(Ordering::SeqCst) {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        self.custody
            .reserve_or_reopen(round, intent, ordinal, terms)
    }
    fn reconcile_verified(
        &self,
        round: Digest32,
        prepared: &PrivateSettlementRequest,
        outcome: &VerifiedPrivateSettlementOutcome,
    ) -> std::result::Result<(), OneCallPrivateSettlementError> {
        PrivateSettlementWallet::reconcile_verified(&self.custody, round, prepared, outcome)?;
        self.reconciliations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn authorize_original(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: usize,
    governed: &OriginalOpening,
    data: &PrivateSettlementLegPrivateData,
    funding: PrivateSettlementFunding,
) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1> {
    let material = |index, purpose: &[u8]| {
        private_settlement_leg_private_material_for_bundle(
            intent.bundle_id,
            &governed.governance,
            ordinal,
            index,
            purpose,
        )
    };
    let note = |purpose: &[u8], index: usize, active, value, spending_authority| {
        let field = |offset| material(index * 6 + offset, purpose);
        PrivateSettlementAuditNoteOpeningV1 {
            active,
            // This is private note construction state. The canonical input/output owner
            // replaces the commitment before it can enter a signed body or proof statement.
            commitment: PrivacyCommitmentV1::new([0; 32]),
            value,
            spending_authority,
            rho: field(2),
            blinding: field(3),
            memo_digest: field(4),
            dummy_domain: (!active).then(|| Hash::prehashed(field(5))),
        }
    };
    let spending_secrets = [
        funding.spending_secret,
        material(1, b"input-spending-secret"),
    ];
    let mut input_openings = vec![
        funding.opening,
        note(
            b"input-note",
            1,
            false,
            0,
            derive_note_authority_v1(&spending_secrets[1])?,
        ),
    ];
    let nullifiers = prepare_atomic_private_settlement_authorization_inputs_v1(
        intent,
        ordinal as u8,
        &mut input_openings,
        &spending_secrets,
    )?;
    ensure!(
        input_openings[0].commitment == funding.input_commitments[0],
        "cold wallet replaced the already funded positive note"
    );
    let payer = AccountId::new(data.payer.public_key().clone());
    let recipient = AccountId::new(data.recipient.public_key().clone());
    let payer_body = PrivateSettlementAuditPayerAuthorizationBodyV1::from_openings(
        intent.network_id,
        intent.bundle_id,
        ordinal as u8,
        governed.governance.body.route,
        &payer,
        intent.expiry_height,
        &input_openings,
        &nullifiers,
    )?;
    let payer_authorization = PrivateSettlementAuditPayerAuthorizationV1::new(
        payer_body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            data.payer.public_key().clone(),
            SignatureOf::try_new(data.payer.private_key(), &payer_body)?,
        )],
    );
    let reimbursement = if ordinal == usize::from(intent.reimbursement_leg_ordinal) {
        5
    } else {
        0
    };
    let mut outputs = Vec::with_capacity(3);
    for (index, (signer, active, amount, role)) in [
        (
            &data.recipient,
            true,
            data.amount,
            PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
        ),
        (
            &data.payer,
            true,
            7,
            PrivateSettlementAuditOutputRoleV1::PayerChange,
        ),
        (
            &*ALICE_KEYPAIR,
            reimbursement != 0,
            reimbursement,
            PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let opening = note(
            b"output-note",
            index,
            active,
            amount,
            derive_note_authority_v1(&material(index, b"output-spending-secret"))?,
        );
        let view = ivm_private_recipient_public_key_v1(&material(index, b"output-view-secret"))?;
        let body = PrivateSettlementAuditViewKeyAuthorizationBodyV1::from_output(
            intent.network_id,
            intent.bundle_id,
            ordinal as u8,
            governed.governance.body.route,
            intent.expiry_height,
            &recipient,
            &payer,
            &intent.sponsor,
            index,
            view,
            &opening,
        )?;
        outputs.push(PrivateSettlementAuditOutputV1 {
            role,
            recipient_view_key: view,
            view_key_authorization: PrivateSettlementAuditViewKeyAuthorizationV1::new(
                body.clone(),
                vec![PrivateSettlementAuditViewKeySignatureV1::new(
                    signer.public_key().clone(),
                    SignatureOf::try_new(signer.private_key(), &body)?,
                )],
            ),
            encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                ephemeral_secret: material(index, b"output-ephemeral-secret"),
            },
            note: opening,
        });
    }
    let plaintext = PrivateSettlementAuditPlaintextV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: intent.network_id,
        bundle_id: intent.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governed.governance.body.route,
        pool_id: governed.governance.body.pool_id,
        payer,
        payer_authorization,
        recipient,
        sponsor: intent.sponsor.clone(),
        asset_definition_id: governed.governance.body.asset_definition_id.clone(),
        asset_binding_salt: governed.governance.body.asset_binding_salt,
        amount: data.amount,
        sponsor_reimbursement_amount: reimbursement,
        fee_intent_digest: intent.fee_intent_digest,
        settlement_expiry_height: intent.expiry_height,
        reimbursement_terms_salt: private_settlement_reimbursement_terms_salt_for_terms(
            intent.network_id,
            intent.authority_context_height,
            intent.expiry_height,
            intent.fee_intent_digest,
            &governed.governance,
        ),
        memo: data.memo.clone(),
        policy_references: vec![governed.governance.governance_digest],
        inputs: input_openings,
        outputs,
    };
    let inputs = plan_atomic_private_settlement_initial_inputs_v1(
        governed.governance.body.pool_id,
        funding.input_commitments,
        spending_secrets,
    )?;
    Ok(AtomicPrivateSettlementAuthorizedWalletLegV1::new(
        plaintext, inputs,
    ))
}

const WALLET_CONTINUATION_INPUT_MAX: usize = 1024 * 1024;
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "integration_tests::private_onecall::WalletContinuationInput")]
struct WalletContinuationInput {
    originals: Vec<OriginalOpening>,
    intent: PrivateSettlementProofIntentV1,
    round: Digest32,
    path: String,
    original_authorization: Hash,
    key: [u8; 32],
}
impl Drop for WalletContinuationInput {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.key.zeroize();
    }
}

fn continue_wallet_in_fresh_process(input: &WalletContinuationInput) -> Result<()> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let (_, module) = module_path!()
        .split_once("::")
        .ok_or_else(|| eyre!("wallet child module must be nested in the test crate"))?;
    let filter = format!("{module}::cold_wallet_opening_continuity_child");
    let bytes = zeroize::Zeroizing::new(norito::encode_canonical(input)?);
    ensure!(bytes.len() <= WALLET_CONTINUATION_INPUT_MAX);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut child = tokio::process::Command::new(std::env::current_exe()?)
            .arg(&filter)
            .args(["--exact", "--ignored", "--test-threads=1", "--nocapture"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let pid = child.id().ok_or_else(|| eyre!("wallet child pid absent"))?;
        ensure!(pid != std::process::id());
        let outcome = tokio::time::timeout(Duration::from_secs(60), async {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| eyre!("wallet stdin absent"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| eyre!("wallet stdout absent"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| eyre!("wallet stderr absent"))?;
            let send = async {
                stdin.write_all(&bytes).await?;
                stdin.shutdown().await?;
                drop(stdin);
                let status = child.wait().await?;
                ensure!(status.success(), "original wallet child failed: {status}");
                Ok::<_, eyre::Report>(())
            };
            let read = |output: std::pin::Pin<Box<dyn tokio::io::AsyncRead>>| async move {
                let mut bytes = Vec::new();
                output.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
                ensure!(bytes.len() <= 64 * 1024, "wallet child log exceeds bound");
                Ok::<_, eyre::Report>(bytes)
            };
            let ((), stdout, _) =
                tokio::try_join!(send, read(Box::pin(stdout)), read(Box::pin(stderr)))?;
            let stdout = std::str::from_utf8(&stdout)?;
            ensure!(stdout.contains(&format!("test {filter} ... ")));
            ensure!(stdout.contains("1 passed; 0 failed; 0 ignored"));
            Ok::<_, eyre::Report>(())
        })
        .await;
        if !matches!(outcome, Ok(Ok(()))) {
            // Reap only the child created here; never target a node, compiler or agent process.
            let _ = child.start_kill();
            tokio::time::timeout(Duration::from_secs(5), child.wait()).await??;
        }
        outcome??;
        Ok(())
    })
}

#[test]
#[ignore = "bounded-stdin child of the original wallet opening continuity test"]
fn cold_wallet_opening_continuity_child() -> Result<()> {
    use iroha_core_privacy::privacy_engines::atomic_private_settlement::encode_atomic_private_settlement_reservation_v1;
    use std::io::Read as _;

    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(WALLET_CONTINUATION_INPUT_MAX as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(!bytes.is_empty() && bytes.len() <= WALLET_CONTINUATION_INPUT_MAX);
    let input: WalletContinuationInput =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))?;
    drop(bytes);
    ensure!(input.originals.len() == PARTICIPANT_COUNT);
    let data = default_private_settlement_leg_data(0);
    let terms = PrivateSettlementBusinessTerms {
        asset: input.originals[0]
            .governance
            .body
            .asset_definition_id
            .clone(),
        payer: AccountId::new(data.payer.public_key().clone()),
        recipient: AccountId::new(data.recipient.public_key().clone()),
        amount: data.amount,
        reimbursement: 5,
        memo: data.memo.clone(),
    };
    let wallet = OriginalWallet::open(
        input.originals.clone(),
        std::path::Path::new(&input.path),
        &input.key,
    )?;
    wallet.enable_explicit_continuation();
    // This explicit preparation continuation validates the supplied original openings again.
    // Read-only payment recovery never enters this supplier or restages inputs.
    wallet.open_policy(&input.intent, 0, &terms)?;
    let original = wallet.reserve_or_reopen(input.round, &input.intent, 0, &terms)?;
    let (material, _) =
        encode_atomic_private_settlement_reservation_v1(&input.intent, 0, &original)?;
    ensure!(Hash::new(&material) == input.original_authorization);
    ensure!(wallet.openings.load(Ordering::SeqCst) == 1);
    ensure!(wallet.stages.load(Ordering::SeqCst) == 1);
    ensure!(wallet.reservations_called.load(Ordering::SeqCst) == 1);
    ensure!(wallet.reconciliations.load(Ordering::SeqCst) == 0);
    Ok(())
}

#[test]
fn cold_wallet_original_note_cas_reopens_only_the_original_round_and_intent() {
    use iroha_core_privacy::privacy_engines::atomic_private_settlement::encode_atomic_private_settlement_reservation_v1;
    use iroha_private_settlement::PrivateSettlementWalletStorageError;
    let routes = (0..PARTICIPANT_COUNT)
        .map(|ordinal| PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(ordinal as u64 + 1),
            lane_id: LaneId::new(ordinal as u32 + 1),
            lane_incarnation: hash(0xE0 + ordinal as u8),
        })
        .collect::<Vec<_>>();
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xF8)),
    );
    let governed = governed_legs(&routes, 301, 401).unwrap();
    let intent = super::intent(network, 301, 401, &governed).unwrap();
    let data = default_private_settlement_leg_data(0);
    let terms = PrivateSettlementBusinessTerms {
        asset: governed[0].governance.body.asset_definition_id.clone(),
        payer: AccountId::new(data.payer.public_key().clone()),
        recipient: AccountId::new(data.recipient.public_key().clone()),
        amount: data.amount,
        reimbursement: 5,
        memo: data.memo.clone(),
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("original-wallet");
    let key = [73; 32];
    let wallet = OriginalWallet::open(original_openings(&governed), &path, &key).unwrap();
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    wallet.enable_explicit_continuation();
    wallet.open_policy(&intent, 0, &terms).unwrap();
    let original = wallet
        .reserve_or_reopen([23; 32], &intent, 0, &terms)
        .unwrap();
    let (original_bytes, inspected) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).unwrap();
    drop(original);
    assert_eq!(wallet.stages.load(Ordering::SeqCst), 1);
    drop(wallet);
    let reopened = OriginalWallet::open(original_openings(&governed), &path, &key).unwrap();
    reopened.enable_explicit_continuation();
    let same = reopened
        .reserve_or_reopen([23; 32], &intent, 0, &terms)
        .unwrap();
    let (same_bytes, same_inspection) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &same).unwrap();
    assert_eq!(same_bytes.as_slice(), original_bytes.as_slice());
    assert_eq!(inspected, same_inspection);
    drop(same);
    assert!(
        reopened
            .reserve_or_reopen([24; 32], &intent, 0, &terms)
            .is_err()
    );
    let changed = super::intent(network, 302, 401, &governed).unwrap();
    assert!(
        reopened
            .reserve_or_reopen([23; 32], &changed, 0, &terms)
            .is_err()
    );
    let original_envelope = std::fs::read(path.join("authorized-inputs.wallet")).unwrap();
    continue_wallet_in_fresh_process(&WalletContinuationInput {
        originals: original_openings(&governed),
        intent: intent.clone(),
        round: [23; 32],
        path: path.to_str().unwrap().to_owned(),
        original_authorization: Hash::new(&original_bytes),
        key,
    })
    .unwrap();
    assert_eq!(
        std::fs::read(path.join("authorized-inputs.wallet")).unwrap(),
        original_envelope
    );
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 2);
    let replacement = governed_legs(&routes, 302, 401).unwrap();
    let wrong_path = root.path().join("wrong-original-openings");
    let wrong = OriginalWallet::open(original_openings(&replacement), &wrong_path, &key).unwrap();
    wrong.enable_explicit_continuation();
    assert!(matches!(
        wrong.open_policy(&intent, 0, &terms),
        Err(OneCallPrivateSettlementError::WalletCustody(
            PrivateSettlementWalletStorageError::Binding
        ))
    ));
    assert_eq!(wrong.stages.load(Ordering::SeqCst), 0);
    assert_eq!(wrong.reservations_called.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read_dir(&wrong_path).unwrap().count(), 0);
    assert!(matches!(
        reopened.open_policy(&changed, 0, &terms),
        Err(OneCallPrivateSettlementError::WalletCustody(
            PrivateSettlementWalletStorageError::InputReserved
        ))
    ));
    assert_eq!(
        std::fs::read(path.join("authorized-inputs.wallet")).unwrap(),
        original_envelope
    );
    assert_eq!(
        reopened.stages.load(Ordering::SeqCst),
        0,
        "a new intent cannot stage an original active input"
    );
    assert_eq!(
        reopened.reconciliations.load(Ordering::SeqCst),
        0,
        "this original reservation test claims no successful settlement"
    );
    let foreign = OriginalWallet::open(original_openings(&governed), &path, &[74; 32]).unwrap();
    foreign.enable_explicit_continuation();
    assert!(
        foreign
            .reserve_or_reopen([23; 32], &intent, 0, &terms)
            .is_err()
    );
    assert_eq!(
        std::fs::read(path.join("authorized-inputs.wallet")).unwrap(),
        original_envelope
    );
}
