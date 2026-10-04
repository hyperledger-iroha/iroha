//! Cross-language KAGEMUSHA wallet V1 vectors and consolidated codec checks (design §8, C11).
//!
//! One deterministic fixture world is built from fixed P-256 scalars `[seed; 32]`. Every
//! signature is the RFC 6979 output of a p256 `SigningKey`, frozen to low S through
//! [`kagemusha_wallet_freeze_signature_v1`] by the objects' own constructors. The world feeds
//! `fixtures/kagemusha/wallet_v1_vectors.json`: digest vectors for every role, signature
//! vectors with their high-S twins and the low-S boundary scalars, one envelope vector per
//! message kind, frame identities of the top-level records, one pinned canonical frame of every
//! framed object type and marker state, and the enum tag table. The file
//! is compared byte for byte; `IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS=1` rewrites it (a
//! test-only convenience). Stand-in proof bytes, relation bindings and empty-map roots are
//! labelled: they change when G3 fixes the artifact set.

use std::{path::PathBuf, sync::OnceLock};

use norito::json::{Map, Number, Value};
use p256::{
    FieldBytes, ProjectivePoint, Scalar, U256,
    ecdsa::{Signature as P256Signature, SigningKey, VerifyingKey, signature::Verifier as _},
    elliptic_curve::{Field as _, PrimeField as _, ops::Reduce, point::AffineCoordinates as _},
};
use sha2::{Digest as _, Sha256};

use super::{
    codec_tests::{assert_every_flip_rejected_or_rebound, norito_tag, payload_range},
    identity::identity_tests::{
        public_key, raw_output, signing_key, test_account, test_certificate,
    },
    messages::messages_tests::{ACCEPTED_MS, MessageFixture, message_fixture},
    state::state_tests::{
        EMPTY_ROOTS, bootstrap_statement, signed_package, stand_in_proof, transition_statement,
    },
    *,
};
use crate::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};

type Role = KagemushaWalletDigestRoleV1;

/// Path of the vectors file relative to this crate.
const VECTORS_PATH: &str = "../../fixtures/kagemusha/wallet_v1_vectors.json";
/// Test-only switch that rewrites the vectors file instead of comparing it.
const UPDATE_VARIABLE: &str = "IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS";
/// Common prefix of every wallet V1 frame name.
const FRAME_NAME_PREFIX: &str = "iroha_data_model::kagemusha::kagemusha_wallet_v1::";

/// Stand-in relation bindings shared with the identity fixture (design C3); the real values
/// come from the G3 artifact set.
const EQ: [u8; 32] = [0x21; 32];
const EP: [u8; 32] = [0x22; 32];
const NATIVE: [u8; 32] = [0x23; 32];
const VK_SET: [u8; 32] = [0x24; 32];
const INVENTORY: [u8; 32] = [0x25; 32];

/// Fixed scalar seeds `[seed; 32]` of the vector signers.
const ROOT_SEED: u8 = 0x11;
const RECEIVER_ISSUER_SEED: u8 = 0x22;
const PAYER_ISSUER_SEED: u8 = 0x26;
const REGULATOR_SEED: u8 = 0x33;
const LOAD_SEED: u8 = 0x34;
const TIME_SEED: u8 = 0x35;
const ARTIFACT_SEED: u8 = 0x36;
const RENEWAL_KEY_SEED: u8 = 0x37;
const PAYER_SEED: u8 = 0x51;
const RECEIVER_SEED: u8 = 0x52;
/// Ed25519 seed of the fee and charge beneficiary account.
const BENEFICIARY_SEED: u8 = 0x5b;

/// Stand-in proof lengths of the vectored packages.
const VECTOR_PROOF_LEN: usize = 48;
const STATUS_PROOF_LEN: usize = 24;
/// Session nonce of the vectored Offer and session controls.
const SESSION_NONCE: [u8; 32] = [0x5e; 32];
/// Renewal challenge of the vectored renewal request.
const RENEWAL_CHALLENGE: [u8; 32] = [0x7c; 32];
/// Start of the vectored quota windows.
const QUOTA_START_MS: u64 = 1_789_948_800_000;
const DAY_MS: u64 = 86_400_000;

/// P-256 group order `n`.
const ORDER_HEX: &str = "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551";
/// `floor(n / 2)`, the largest accepted `s`.
const HALF_ORDER_HEX: &str = "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8";
/// Fixed nonce `k` of the boundary signature (design C11).
const BOUNDARY_NONCE: [u8; 32] = [0x5a; 32];
/// Signed role and body of the boundary signature.
const BOUNDARY_ROLE: Role = Role::ReceiptBody;
const BOUNDARY_BODY: &[u8] = b"kagemusha wallet v1 low-S boundary";

// ---------------------------------------------------------------------------------------
// Fixture world
// ---------------------------------------------------------------------------------------

/// One fully valid object of every vectored kind, built from fixed P-256 scalars.
pub(super) struct VectorWorld {
    /// Payer, receiver and regulator of one scheme.
    pub(super) f: MessageFixture,
    /// `LoadAuthorization`-role signer.
    load_signer: SigningKey,
    /// `LoadAuthorization`-role certificate.
    load_certificate: KagemushaWalletSignerCertificateV1,
    /// `TimeAnchor`-role signer.
    time_signer: SigningKey,
    /// `TimeAnchor`-role certificate.
    time_certificate: KagemushaWalletSignerCertificateV1,
    /// `Artifact`-role signer.
    artifact_signer: SigningKey,
    /// `Artifact`-role certificate.
    artifact_certificate: KagemushaWalletSignerCertificateV1,
    /// Epoch-1 scheme policy naming the fee schedule.
    scheme_policy: KagemushaWalletSchemePolicyV1,
    /// Fee schedule of the Request.
    fee_schedule: KagemushaWalletFeeScheduleV1,
    /// Three-entry blacklist.
    blacklist: KagemushaWalletBlacklistV1,
    /// Two-window quota share of the payer.
    quota_share: KagemushaWalletQuotaShareV1,
    /// Time anchor of the payer.
    time_anchor: KagemushaWalletTimeAnchorV1,
    /// Load charge quote of the payer.
    charge_quote: KagemushaWalletChargeQuoteV1,
    /// Quoted load voucher of the payer.
    voucher: KagemushaWalletLoadVoucherV1,
    /// Artifact manifest of the scheme.
    manifest: KagemushaWalletArtifactManifestV1,
    /// Android renewal request of the payer.
    renewal: KagemushaWalletRenewalRequestV1,
    /// Payer Offer.
    offer: KagemushaWalletOfferV1,
    /// Signed `Close` control of the payer.
    close: KagemushaWalletSessionControlV1,
    /// Unsigned `UnsupportedScheme` control.
    unsupported: KagemushaWalletSessionControlV1,
    /// Fee-bearing Payment with three certificates.
    payment: KagemushaWalletPaymentV1,
    /// Receiver's Receive package of the Payment, receipted over `capsule`.
    receive: KagemushaWalletPackageV1,
    /// Recovery capsule of the Receive.
    capsule: KagemushaWalletRecoveryCapsuleV1,
    /// Completion record of the Receive.
    completion: KagemushaWalletCompletionRecordV1,
    /// Credited evidence from the Receive package.
    credited_receive: KagemushaWalletCreditedV1,
    /// Credited evidence from a `CreditStatus` proof.
    credited_status: KagemushaWalletCreditedV1,
    /// Generation-0 enrollment marker of the payer.
    marker: KagemushaWalletMarkerV1,
    /// Bootstrap package of the payer, bound to `marker`.
    bootstrap: KagemushaWalletPackageV1,
    /// Activate ledger control of the payer.
    control: KagemushaWalletLedgerControlV1,
}

/// The fixture world, built once per test binary.
pub(super) fn vector_world() -> &'static VectorWorld {
    static WORLD: OnceLock<VectorWorld> = OnceLock::new();
    WORLD.get_or_init(build_world)
}

/// Payer Offer of 1,000 units at send ordinal 4.
pub(super) fn vector_offer(f: &MessageFixture) -> KagemushaWalletOfferV1 {
    let body = KagemushaWalletOfferBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: f.scheme_id(),
        asset_digest: f.asset_digest(),
        payer_wallet_id: f.payer.credential.body.wallet_id,
        payer_credential_digest: f.payer.credential.credential_digest(),
        next_send: 4,
        amount: 1_000,
        session_nonce: SESSION_NONCE,
    };
    KagemushaWalletOfferV1::sign(
        body,
        f.payer.credential,
        &f.payer.enrollment_certificate,
        raw_output(&f.payer.payment, &body.signing_message()),
    )
    .expect("offer")
}

/// Payer session control of `kind`; signed except `UnsupportedScheme`.
pub(super) fn vector_control(
    f: &MessageFixture,
    kind: KagemushaWalletSessionControlKindV1,
) -> KagemushaWalletSessionControlV1 {
    use KagemushaWalletSessionControlKindV1 as Kind;
    let control = KagemushaWalletSessionControlV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: f.scheme_id(),
        asset_digest: f.asset_digest(),
        sender_wallet_id: if kind == Kind::UnsupportedScheme {
            [0; 32]
        } else {
            f.payer.credential.body.wallet_id
        },
        peer_wallet_id: f.receiver.credential.body.wallet_id,
        session_nonce: SESSION_NONCE,
        kind,
        reason: match kind {
            Kind::SetupDeclined | Kind::ReceiveDeferred => 7,
            Kind::UnsupportedScheme | Kind::Close => 0,
        },
        credit_id: if kind == Kind::ReceiveDeferred {
            [0x71; 32]
        } else {
            [0; 32]
        },
        auth: KagemushaWalletSessionAuthV1::Unsigned,
    };
    if kind == Kind::UnsupportedScheme {
        control.validate().expect("unsigned control");
        return control;
    }
    control
        .sign(
            &f.payer.credential,
            raw_output(&f.payer.payment, &control.signing_message()),
        )
        .expect("signed control")
}

/// Signed blacklist of `count` distinct stand-in account digests.
fn vector_blacklist(f: &MessageFixture, count: u32) -> KagemushaWalletBlacklistV1 {
    let mut entries: Vec<KagemushaWalletBlacklistEntryV1> = (0..count)
        .map(|index| KagemushaWalletBlacklistEntryV1 {
            account_digest: kagemusha_wallet_digest_v1(Role::Account, &index.to_le_bytes()),
        })
        .collect();
    entries.sort();
    let body = KagemushaWalletBlacklistBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: f.scheme_id(),
        list_version: 3,
        issued_at_ms: ACCEPTED_MS - 10_000,
        entry_count: count,
        entries_root: kagemusha_wallet_blacklist_root_v1(&entries).expect("root"),
        signer_certificate: f.regulator_certificate.certificate_digest(),
    };
    KagemushaWalletBlacklistV1::sign(
        body,
        entries,
        &f.regulator_certificate,
        raw_output(&f.regulator, &body.signing_message()),
    )
    .expect("blacklist")
}

/// Build every vectored object and check that each one validates.
fn build_world() -> VectorWorld {
    let f = message_fixture();
    let scheme = f.payer.scheme;
    let scheme_id = scheme.scheme_id();
    let asset_digest = f.asset_digest();
    let payer = f.payer.credential;
    let receiver = f.receiver.credential;
    for (seed, key) in [
        (ROOT_SEED, scheme.scheme_root_key),
        (
            RECEIVER_ISSUER_SEED,
            f.receiver.enrollment_certificate.body.key,
        ),
        (PAYER_ISSUER_SEED, f.payer.enrollment_certificate.body.key),
        (REGULATOR_SEED, f.regulator_certificate.body.key),
        (PAYER_SEED, payer.body.payment_key),
        (RECEIVER_SEED, receiver.body.payment_key),
    ] {
        assert_eq!(public_key(&signing_key(seed)), key, "seed {seed:#04x}");
    }
    assert_eq!(
        kagemusha_wallet_relation_id_v1(&EQ, &EP, &NATIVE, &VK_SET, &INVENTORY),
        scheme.relation_id
    );

    let root = &f.payer.root;
    let load_signer = signing_key(LOAD_SEED);
    let load_certificate = test_certificate(
        &scheme,
        root,
        KagemushaWalletSignerRoleV1::LoadAuthorization,
        &load_signer,
        3,
    );
    let time_signer = signing_key(TIME_SEED);
    let time_certificate = test_certificate(
        &scheme,
        root,
        KagemushaWalletSignerRoleV1::TimeAnchor,
        &time_signer,
        4,
    );
    let artifact_signer = signing_key(ARTIFACT_SEED);
    let artifact_certificate = test_certificate(
        &scheme,
        root,
        KagemushaWalletSignerRoleV1::Artifact,
        &artifact_signer,
        5,
    );
    let regulator_certificate = f.regulator_certificate.certificate_digest();
    let beneficiary =
        kagemusha_wallet_account_digest_v1(&test_account(BENEFICIARY_SEED)).expect("beneficiary");

    let fee_schedule = f.fee_schedule();
    let policy_body = KagemushaWalletSchemePolicyBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest,
        policy_epoch: 1,
        enabled_controls: 0,
        fee_schedule: fee_schedule.fee_schedule_digest(),
        signer_certificate: regulator_certificate,
    };
    let scheme_policy = KagemushaWalletSchemePolicyV1::sign(
        policy_body,
        &f.regulator_certificate,
        raw_output(&f.regulator, &policy_body.signing_message()),
    )
    .expect("scheme policy");
    let blacklist = vector_blacklist(&f, 3);

    let windows = vec![
        KagemushaWalletQuotaWindowV1 {
            kind: KagemushaWalletQuotaWindowKindV1::Daily,
            start_ms: QUOTA_START_MS,
            end_ms: QUOTA_START_MS + DAY_MS,
            limit: 50_000,
        },
        KagemushaWalletQuotaWindowV1 {
            kind: KagemushaWalletQuotaWindowKindV1::Monthly,
            start_ms: QUOTA_START_MS,
            end_ms: QUOTA_START_MS + 30 * DAY_MS,
            limit: 500_000,
        },
    ];
    let share_body = KagemushaWalletQuotaShareBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest,
        wallet_id: payer.body.wallet_id,
        share_id: 1,
        issued_at_ms: QUOTA_START_MS,
        expires_at_ms: QUOTA_START_MS + 31 * DAY_MS,
        windows_root: kagemusha_wallet_quota_windows_root_v1(&windows).expect("windows root"),
        window_count: 2,
        signer_certificate: regulator_certificate,
    };
    let quota_share = KagemushaWalletQuotaShareV1::sign(
        share_body,
        windows,
        &f.regulator_certificate,
        raw_output(&f.regulator, &share_body.signing_message()),
    )
    .expect("quota share");

    let anchor_body = KagemushaWalletTimeAnchorBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        wallet_id: payer.body.wallet_id,
        nonce: [0x7e; 32],
        issuer_time_ms: ACCEPTED_MS + 1_000,
        signer_certificate: time_certificate.certificate_digest(),
    };
    let time_anchor = KagemushaWalletTimeAnchorV1::sign(
        anchor_body,
        &time_certificate,
        raw_output(&time_signer, &anchor_body.signing_message()),
    )
    .expect("time anchor");

    let quote_body = KagemushaWalletChargeQuoteBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest,
        wallet_id: payer.body.wallet_id,
        kind: KagemushaWalletChargeKindV1::Load,
        ordinal: 0,
        net_amount: 5_000,
        online_charge: 25,
        beneficiary_account_digest: beneficiary,
        issued_at_ms: ACCEPTED_MS - 60_000,
        signer_certificate: regulator_certificate,
    };
    let charge_quote = KagemushaWalletChargeQuoteV1::sign(
        quote_body,
        &f.regulator_certificate,
        raw_output(&f.regulator, &quote_body.signing_message()),
    )
    .expect("charge quote");
    let voucher_body = KagemushaWalletLoadVoucherBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest,
        wallet_id: payer.body.wallet_id,
        ordinal: 0,
        amount: 5_000,
        online_charge: 25,
        charge_quote: charge_quote.charge_quote_digest(),
        transaction_hash: [0x9a; 32],
        block_height: 42,
        authorizer_certificate: load_certificate.certificate_digest(),
    };
    let voucher = KagemushaWalletLoadVoucherV1::sign(
        voucher_body,
        &load_certificate,
        raw_output(&load_signer, &voucher_body.signing_message()),
    )
    .expect("voucher");
    voucher
        .require_charge_quote(Some(&charge_quote))
        .expect("quoted voucher");

    let manifest_body = KagemushaWalletArtifactManifestBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        network_id: scheme.network_id,
        relation_id: scheme.relation_id,
        eq_protocol_digest: EQ,
        ep_protocol_digest: EP,
        native_profile_digest: NATIVE,
        verifying_key_set_digest: VK_SET,
        artifact_inventory_digest: INVENTORY,
        provider_contract: scheme.provider_contract,
        signer_certificate: artifact_certificate.certificate_digest(),
    };
    let manifest = KagemushaWalletArtifactManifestV1::sign(
        manifest_body,
        &artifact_certificate,
        raw_output(&artifact_signer, &manifest_body.signing_message()),
    )
    .expect("manifest");
    manifest
        .verify(&scheme, &artifact_certificate)
        .expect("manifest verifies");

    let renewal_key = public_key(&signing_key(RENEWAL_KEY_SEED));
    let binding = kagemusha_wallet_renewal_key_binding_transcript_v1(
        &scheme_id,
        &payer.body.wallet_id,
        &RENEWAL_CHALLENGE,
        &renewal_key,
    );
    let evidence = KagemushaWalletRenewalEvidenceV1::android_signed(
        &payer,
        &RENEWAL_CHALLENGE,
        renewal_key,
        raw_output(
            &f.payer.payment,
            &kagemusha_wallet_preimage_v1(Role::RenewalKeyBinding, &binding),
        ),
        vec![
            KagemushaWalletDerCertificateV1 {
                der: b"renewal-leaf-der".to_vec(),
            },
            KagemushaWalletDerCertificateV1 {
                der: b"renewal-intermediate-der".to_vec(),
            },
        ],
    )
    .expect("renewal evidence");
    let possession = kagemusha_wallet_renewal_challenge_transcript_v1(
        &scheme_id,
        &payer.body.wallet_id,
        &payer.credential_digest(),
        &RENEWAL_CHALLENGE,
    );
    let renewal = KagemushaWalletRenewalRequestV1::sign(
        &payer,
        RENEWAL_CHALLENGE,
        raw_output(
            &f.payer.payment,
            &kagemusha_wallet_preimage_v1(Role::RenewalChallenge, &possession),
        ),
        evidence,
    )
    .expect("renewal request");

    let offer = vector_offer(&f);
    let close = vector_control(&f, KagemushaWalletSessionControlKindV1::Close);
    let unsupported = vector_control(&f, KagemushaWalletSessionControlKindV1::UnsupportedScheme);

    let payment = f.payment(true, VECTOR_PROOF_LEN);
    assert_eq!(
        payment.request.body.scheme_policy,
        scheme_policy.scheme_policy_digest()
    );
    assert_eq!(
        payment.request.body.fee_schedule,
        fee_schedule.fee_schedule_digest()
    );
    assert_eq!(
        payment.request.certificates.len() + payment.certificates.len(),
        3
    );

    // The Receive package is receipted over its real capsule, so the completion record verifies.
    let statement = transition_statement(
        &f.receiver,
        5,
        0,
        KagemushaWalletLifecycleV1::Active,
        payment.receive_effect(&receiver).expect("receive effect"),
    );
    let proof = stand_in_proof(VECTOR_PROOF_LEN);
    let mut state =
        KagemushaWalletStateV1::bootstrap(&receiver, &EMPTY_ROOTS, [0x5c; 32]).expect("state");
    state.sequence = statement.sequence;
    state.balance = 1_000;
    let capsule = KagemushaWalletRecoveryCapsuleV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        wallet_id: receiver.body.wallet_id,
        operation_id: statement.operation_id(&receiver.body.wallet_id),
        kind: KagemushaWalletOperationKindV1::Receive,
        predecessor_capsule_digest: [0x5b; 32],
        successor_state: state,
        statement,
        proof: proof.clone(),
        map_openings: vec![vec![0x6d; 32]],
        retained_inputs: vec![KagemushaWalletRetainedInputV1 {
            role: KagemushaWalletRetainedInputRoleV1::Payment,
            bytes: payment.to_canonical_bytes().expect("payment frame"),
        }],
        output: KagemushaWalletOutputDescriptorV1::for_transition(&statement, &proof, None)
            .expect("output"),
    };
    let capsule_digest = capsule.capsule_digest().expect("capsule digest");
    let receipt_body =
        KagemushaWalletReceiptBodyV1::derive(&receiver, &statement, &proof, capsule_digest)
            .expect("receipt body");
    let receipt = KagemushaWalletReceiptV1::sign(
        &receiver,
        &statement,
        &proof,
        capsule_digest,
        raw_output(&f.receiver.payment, &receipt_body.signing_message()),
    )
    .expect("receipt");
    let receive = KagemushaWalletPackageV1::new(statement, proof, receipt);
    let completion = KagemushaWalletCompletionRecordV1::new(
        &capsule,
        receipt,
        encode_frame_v1(&receive, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).expect("package frame"),
    )
    .expect("completion");
    completion
        .verify(&receiver, &capsule)
        .expect("completion verifies");
    let credited_receive = KagemushaWalletCreditedV1::from_receive(
        receiver,
        &f.receiver.enrollment_certificate,
        receive.clone(),
    )
    .expect("credited receive");
    let credited_status = f.credited_status(&payment, VECTOR_PROOF_LEN, STATUS_PROOF_LEN);

    let marker = KagemushaWalletMarkerV1::enrollment(&f.payer.challenge, payer.body.payment_key)
        .expect("enrollment marker");
    let bootstrap_body = KagemushaWalletStatementV1 {
        effect: marker.bootstrap_effect().expect("bootstrap effect"),
        ..bootstrap_statement(&f.payer)
    };
    let bootstrap = signed_package(
        &f.payer,
        &payer,
        &bootstrap_body,
        stand_in_proof(VECTOR_PROOF_LEN),
    );
    let control_body = KagemushaWalletLedgerControlBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest,
        wallet_id: payer.body.wallet_id,
        action: KagemushaWalletLedgerControlActionV1::Activate {
            package_digest: bootstrap.package_digest(&payer).expect("bootstrap digest"),
        },
        nonce: [0x6c; 32],
    };
    let control = KagemushaWalletLedgerControlV1::sign(
        control_body,
        &payer.body.payment_key,
        raw_output(&f.payer.payment, &control_body.signing_message()),
    )
    .expect("ledger control");

    VectorWorld {
        f,
        load_signer,
        load_certificate,
        time_signer,
        time_certificate,
        artifact_signer,
        artifact_certificate,
        scheme_policy,
        fee_schedule,
        blacklist,
        quota_share,
        time_anchor,
        charge_quote,
        voucher,
        manifest,
        renewal,
        offer,
        close,
        unsupported,
        payment,
        receive,
        capsule,
        completion,
        credited_receive,
        credited_status,
        marker,
        bootstrap,
        control,
    }
}

impl VectorWorld {
    fn scheme(&self) -> KagemushaWalletSchemeV1 {
        self.f.payer.scheme
    }

    fn scheme_id(&self) -> [u8; 32] {
        self.f.scheme_id()
    }

    fn payer(&self) -> &KagemushaWalletCredentialV1 {
        &self.f.payer.credential
    }

    fn receiver(&self) -> &KagemushaWalletCredentialV1 {
        &self.f.receiver.credential
    }

    fn status(&self) -> (&KagemushaWalletPackageV1, &KagemushaWalletCreditStatusV1) {
        match &self.credited_status.evidence {
            KagemushaWalletCreditedEvidenceV1::Status { current, status } => (current, status),
            KagemushaWalletCreditedEvidenceV1::Receive { .. } => panic!("status evidence"),
        }
    }

    fn close_signature(&self) -> KagemushaDeviceSignatureV1 {
        match self.close.auth {
            KagemushaWalletSessionAuthV1::Signed { signature } => signature,
            KagemushaWalletSessionAuthV1::Unsigned => panic!("signed control"),
        }
    }

    fn policy_data(&self, item: KagemushaWalletPolicyDataItemV1) -> KagemushaWalletPolicyDataV1 {
        let data = KagemushaWalletPolicyDataV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.f.asset_digest(),
            item,
        };
        data.validate().expect("policy data");
        data
    }
}

// ---------------------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------------------

fn json_object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        assert!(
            map.insert(key.to_owned(), value).is_none(),
            "duplicate key {key}"
        );
    }
    Value::Object(map)
}

fn json_hex(bytes: &[u8]) -> Value {
    Value::String(hex::encode(bytes))
}

fn json_text(text: &str) -> Value {
    Value::String(text.to_owned())
}

fn json_number(value: usize) -> Value {
    Value::Number(Number::U64(u64::try_from(value).expect("fits u64")))
}

fn bytes32(hex_text: &str) -> [u8; 32] {
    let mut out = [0; 32];
    out.copy_from_slice(&hex::decode(hex_text).expect("hex"));
    out
}

fn signed_object_body(body_digest: &[u8; 32], signature: &KagemushaDeviceSignatureV1) -> Vec<u8> {
    let mut body = body_digest.to_vec();
    body.extend_from_slice(signature.as_raw_bytes());
    body
}

// ---------------------------------------------------------------------------------------
// Digest vectors (one per role)
// ---------------------------------------------------------------------------------------

/// One `H(role, body)` vector.
struct DigestVector {
    role: Role,
    object: &'static str,
    body: Vec<u8>,
    stand_in_proof: bool,
}

/// Build one digest vector and check it against the digest its owner computes.
fn digest_vector(
    role: Role,
    object: &'static str,
    body: Vec<u8>,
    stand_in_proof: bool,
    expected: Option<[u8; 32]>,
) -> DigestVector {
    if let Some(expected) = expected {
        assert_eq!(
            kagemusha_wallet_digest_v1(role, &body),
            expected,
            "{}",
            role.as_str()
        );
    }
    DigestVector {
        role,
        object,
        body,
        stand_in_proof,
    }
}

/// One digest vector per role, in role declaration order.
fn digest_vectors(w: &VectorWorld) -> Vec<DigestVector> {
    let f = &w.f;
    let scheme = w.scheme();
    let scheme_id = w.scheme_id();
    let payer = w.payer();
    let receiver = w.receiver();
    let certificate = &f.payer.enrollment_certificate;
    let payment = &w.payment;
    let request = &payment.request;
    let digests = payment.digests().expect("payment digests");
    let send = &payment.send;
    let receipt_body = send
        .receipt
        .body(payer, &send.statement, &send.proof)
        .expect("receipt body");
    let (_, status) = w.status();
    let entries = &w.blacklist.entries;
    let leaf_0 = kagemusha_wallet_blacklist_leaf_v1(
        &KAGEMUSHA_WALLET_BLACKLIST_SENTINEL_LOW_V1,
        &entries[0].account_digest,
    );
    let leaf_1 =
        kagemusha_wallet_blacklist_leaf_v1(&entries[0].account_digest, &entries[1].account_digest);
    let windows = &w.quota_share.windows;
    let window_0 = windows[0].leaf_digest();
    let window_1 = windows[1].leaf_digest();
    let receiver_challenge = f.receiver.challenge.challenge_digest();
    let payer_challenge = f.payer.challenge.challenge_digest();
    let mut output_input = [0; KAGEMUSHA_WALLET_OUTPUT_KIND_INPUT_BYTES_V1];
    output_input[..32].copy_from_slice(&digests.payment);
    let receive_digests = w.receive.verify(receiver).expect("receive package");
    let KagemushaWalletRenewalEvidenceV1::Android {
        new_attested_key, ..
    } = w.renewal.evidence
    else {
        panic!("android renewal");
    };
    let nullifier = kagemusha_wallet_unload_nullifier_v1(&scheme_id, &payer.body.wallet_id, 2);

    vec![
        digest_vector(
            Role::Scheme,
            "scheme",
            scheme.transcript(),
            false,
            Some(scheme_id),
        ),
        digest_vector(
            Role::Relation,
            "stand-in relation bindings of the scheme",
            kagemusha_wallet_relation_transcript_v1(&EQ, &EP, &NATIVE, &VK_SET, &INVENTORY),
            false,
            Some(scheme.relation_id),
        ),
        digest_vector(
            Role::ProviderContract,
            "single V1 provider contract",
            kagemusha_wallet_provider_contract_transcript_v1(),
            false,
            Some(kagemusha_wallet_provider_contract_v1()),
        ),
        digest_vector(
            Role::AssetScope,
            "asset scope",
            f.payer.asset.transcript(),
            false,
            Some(payer.body.asset_digest),
        ),
        digest_vector(
            Role::Account,
            "payer AccountId frame",
            norito::encode_canonical(&test_account(PAYER_SEED)).expect("account frame"),
            false,
            Some(payer.body.account_digest),
        ),
        digest_vector(
            Role::EnrollmentChallenge,
            "payer enrollment challenge",
            f.payer.challenge.transcript(),
            false,
            Some(payer_challenge),
        ),
        digest_vector(
            Role::EnrollmentId,
            "payer enrollment identity",
            kagemusha_wallet_enrollment_key_transcript_v1(
                &payer_challenge,
                &payer.body.payment_key,
            ),
            false,
            Some(payer.body.enrollment_id),
        ),
        digest_vector(
            Role::EnrollmentKeyBinding,
            "receiver App Attest enrollment assertion client data",
            kagemusha_wallet_enrollment_key_transcript_v1(
                &receiver_challenge,
                &receiver.body.payment_key,
            ),
            false,
            Some(kagemusha_wallet_enrollment_key_binding_v1(
                &receiver_challenge,
                &receiver.body.payment_key,
            )),
        ),
        digest_vector(
            Role::WalletId,
            "payer wallet identity",
            kagemusha_wallet_id_transcript_v1(
                &scheme_id,
                &payer.body.asset_digest,
                &payer.body.payment_key,
                &payer.body.enrollment_id,
            ),
            false,
            Some(payer.body.wallet_id),
        ),
        digest_vector(
            Role::CertificateBody,
            "payer issuer certificate body",
            certificate.body.transcript(),
            false,
            Some(certificate.body.body_digest()),
        ),
        digest_vector(
            Role::Certificate,
            "payer issuer certificate",
            signed_object_body(&certificate.body.body_digest(), &certificate.signature),
            false,
            Some(certificate.certificate_digest()),
        ),
        digest_vector(
            Role::CertificateSet,
            "Request certificate set",
            request.certificates.transcript().expect("set transcript"),
            false,
            Some(request.body.certificates),
        ),
        digest_vector(
            Role::CredentialBody,
            "payer credential body",
            payer.body.transcript(),
            false,
            Some(payer.body.body_digest()),
        ),
        digest_vector(
            Role::Credential,
            "payer credential",
            signed_object_body(&payer.body.body_digest(), &payer.signature),
            false,
            Some(payer.credential_digest()),
        ),
        digest_vector(
            Role::SchemePolicyBody,
            "scheme policy body",
            w.scheme_policy.body.transcript(),
            false,
            Some(w.scheme_policy.body.body_digest()),
        ),
        digest_vector(
            Role::SchemePolicy,
            "scheme policy",
            signed_object_body(
                &w.scheme_policy.body.body_digest(),
                &w.scheme_policy.signature,
            ),
            false,
            Some(w.scheme_policy.scheme_policy_digest()),
        ),
        digest_vector(
            Role::FeeScheduleBody,
            "fee schedule body",
            w.fee_schedule.body.transcript(),
            false,
            Some(w.fee_schedule.body.body_digest()),
        ),
        digest_vector(
            Role::FeeSchedule,
            "fee schedule",
            signed_object_body(
                &w.fee_schedule.body.body_digest(),
                &w.fee_schedule.signature,
            ),
            false,
            Some(w.fee_schedule.fee_schedule_digest()),
        ),
        digest_vector(
            Role::BlacklistBody,
            "blacklist body",
            w.blacklist.body.transcript(),
            false,
            Some(w.blacklist.body.body_digest()),
        ),
        digest_vector(
            Role::Blacklist,
            "blacklist",
            signed_object_body(&w.blacklist.body.body_digest(), &w.blacklist.signature),
            false,
            Some(w.blacklist.blacklist_digest()),
        ),
        digest_vector(
            Role::BlacklistLeaf,
            "blacklist gap leaf 0",
            [
                &KAGEMUSHA_WALLET_BLACKLIST_SENTINEL_LOW_V1[..],
                &entries[0].account_digest[..],
            ]
            .concat(),
            false,
            Some(leaf_0),
        ),
        digest_vector(
            Role::BlacklistNode,
            "blacklist node over gap leaves 0 and 1",
            [&leaf_0[..], &leaf_1[..]].concat(),
            false,
            Some(kagemusha_wallet_blacklist_node_v1(&leaf_0, &leaf_1)),
        ),
        digest_vector(
            Role::QuotaShareBody,
            "quota share body",
            w.quota_share.body.transcript(),
            false,
            Some(w.quota_share.body.body_digest()),
        ),
        digest_vector(
            Role::QuotaShare,
            "quota share",
            signed_object_body(&w.quota_share.body.body_digest(), &w.quota_share.signature),
            false,
            Some(w.quota_share.quota_share_digest()),
        ),
        digest_vector(
            Role::QuotaWindow,
            "daily quota window leaf",
            windows[0].transcript(),
            false,
            Some(window_0),
        ),
        digest_vector(
            Role::QuotaNode,
            "quota node over window leaves 0 and 1",
            [&window_0[..], &window_1[..]].concat(),
            false,
            Some(kagemusha_wallet_quota_node_v1(&window_0, &window_1)),
        ),
        digest_vector(
            Role::TimeAnchorBody,
            "time anchor body",
            w.time_anchor.body.transcript(),
            false,
            Some(w.time_anchor.body.body_digest()),
        ),
        digest_vector(
            Role::TimeAnchor,
            "time anchor",
            signed_object_body(&w.time_anchor.body.body_digest(), &w.time_anchor.signature),
            false,
            Some(w.time_anchor.time_anchor_digest()),
        ),
        digest_vector(
            Role::OfferBody,
            "Offer body",
            w.offer.body.transcript(),
            false,
            Some(w.offer.body.body_digest()),
        ),
        digest_vector(
            Role::SessionControlBody,
            "Close session control",
            w.close.transcript(),
            false,
            Some(w.close.body_digest()),
        ),
        digest_vector(
            Role::RequestBody,
            "Request body",
            request.body.transcript(),
            false,
            Some(request.body.body_digest()),
        ),
        digest_vector(
            Role::Request,
            "Request",
            signed_object_body(&request.body.body_digest(), &request.signature),
            false,
            Some(digests.request),
        ),
        digest_vector(
            Role::Credit,
            "credit identity of the Request",
            request.body.transcript(),
            false,
            Some(digests.credit_id),
        ),
        digest_vector(
            Role::Dependencies,
            "positional Send dependencies",
            kagemusha_wallet_send_dependencies_transcript_v1(
                &payer.body.issuer_certificate,
                &request.receiver_credential.body.issuer_certificate,
                &request.fee_schedule.signer_certificate(),
            ),
            false,
            Some(
                request
                    .send_dependencies(payer)
                    .expect("payer dependencies"),
            ),
        ),
        digest_vector(
            Role::Statement,
            "Send statement",
            send.statement.transcript(),
            false,
            Some(digests.package.statement),
        ),
        digest_vector(
            Role::Proof,
            "stand-in Send proof",
            send.proof.bytes.clone(),
            true,
            Some(digests.package.proof),
        ),
        digest_vector(
            Role::ReceiptBody,
            "Send receipt body",
            receipt_body.transcript(),
            false,
            Some(receipt_body.body_digest()),
        ),
        digest_vector(
            Role::Receipt,
            "Send receipt",
            signed_object_body(&receipt_body.body_digest(), &send.receipt.signature),
            false,
            Some(digests.package.receipt),
        ),
        digest_vector(
            Role::Package,
            "Send package",
            [
                &digests.package.statement[..],
                &digests.package.proof[..],
                &digests.package.receipt[..],
            ]
            .concat(),
            false,
            Some(digests.package.package),
        ),
        digest_vector(
            Role::Payment,
            "Payment",
            kagemusha_wallet_payment_transcript_v1(
                &digests.request,
                &digests.payer_credential,
                &digests.package.package,
                &digests.certificates,
            ),
            false,
            Some(digests.payment),
        ),
        digest_vector(
            Role::CreditStatusStatement,
            "CreditStatus statement",
            status.statement.transcript(),
            false,
            Some(status.statement.statement_digest()),
        ),
        digest_vector(
            Role::Credited,
            "Credited from the Receive package",
            w.credited_receive
                .transcript()
                .expect("credited transcript"),
            false,
            Some(w.credited_receive.credited_digest().expect("credited")),
        ),
        digest_vector(
            Role::OperationId,
            "Receive operation identity",
            kagemusha_wallet_operation_id_transcript_v1(
                &receiver.body.wallet_id,
                KagemushaWalletOperationKindV1::Receive,
                &digests.credit_id,
            ),
            false,
            Some(w.capsule.operation_id),
        ),
        digest_vector(
            Role::Output,
            "Receive output descriptor",
            kagemusha_wallet_output_transcript_v1(
                KagemushaWalletOperationKindV1::Receive,
                &receive_digests.statement,
                &receive_digests.proof,
                &output_input,
            ),
            false,
            Some(w.capsule.output.digest),
        ),
        digest_vector(
            Role::Capsule,
            "Receive recovery capsule frame",
            w.capsule.to_canonical_bytes().expect("capsule frame"),
            true,
            Some(w.capsule.capsule_digest().expect("capsule digest")),
        ),
        digest_vector(
            Role::Marker,
            "payer enrollment marker frame",
            w.marker.to_canonical_bytes().expect("marker frame"),
            false,
            Some(w.marker.marker_digest().expect("marker digest")),
        ),
        digest_vector(
            Role::Completion,
            "Receive completion record frame",
            w.completion.to_canonical_bytes().expect("completion frame"),
            true,
            Some(w.completion.completion_digest().expect("completion digest")),
        ),
        digest_vector(
            Role::VoucherBody,
            "load voucher body",
            w.voucher.body.transcript(),
            false,
            Some(w.voucher.body.body_digest()),
        ),
        digest_vector(
            Role::Voucher,
            "load voucher",
            signed_object_body(&w.voucher.body.body_digest(), &w.voucher.signature),
            false,
            Some(w.voucher.voucher_digest()),
        ),
        digest_vector(
            Role::UnloadNullifier,
            "payer unload nullifier at redeem ordinal 2",
            kagemusha_wallet_unload_nullifier_transcript_v1(&scheme_id, &payer.body.wallet_id, 2),
            false,
            Some(nullifier),
        ),
        digest_vector(
            Role::LedgerControlBody,
            "Activate ledger control body",
            w.control.body.transcript(),
            false,
            Some(w.control.body.body_digest()),
        ),
        digest_vector(
            Role::RenewalChallenge,
            "renewal possession transcript",
            w.renewal.possession_transcript(),
            false,
            None,
        ),
        digest_vector(
            Role::RenewalKeyBinding,
            "Android renewal key binding",
            kagemusha_wallet_renewal_key_binding_transcript_v1(
                &scheme_id,
                &payer.body.wallet_id,
                &RENEWAL_CHALLENGE,
                &new_attested_key,
            ),
            false,
            None,
        ),
        digest_vector(
            Role::RenewalAssertion,
            "App Attest renewal assertion client data",
            w.renewal.possession_transcript(),
            false,
            Some(w.renewal.assertion_client_data_hash()),
        ),
        digest_vector(
            Role::ArtifactManifestBody,
            "artifact manifest body",
            w.manifest.body.transcript(),
            false,
            Some(w.manifest.body.body_digest()),
        ),
        digest_vector(
            Role::ArtifactManifest,
            "artifact manifest",
            signed_object_body(&w.manifest.body.body_digest(), &w.manifest.signature),
            false,
            Some(w.manifest.manifest_digest()),
        ),
        digest_vector(
            Role::ChargeQuoteBody,
            "load charge quote body",
            w.charge_quote.body.transcript(),
            false,
            Some(w.charge_quote.body.body_digest()),
        ),
        digest_vector(
            Role::ChargeQuote,
            "load charge quote",
            signed_object_body(
                &w.charge_quote.body.body_digest(),
                &w.charge_quote.signature,
            ),
            false,
            Some(w.charge_quote.charge_quote_digest()),
        ),
        digest_vector(
            Role::Evidence,
            "payer enrollment evidence items",
            kagemusha_wallet_evidence_transcript_v1(
                KagemushaWalletEvidenceKindV1::AndroidKeyMintTee,
                &[&b"tee-leaf-der"[..], &b"tee-intermediate-der"[..]],
            )
            .expect("evidence transcript"),
            false,
            Some(payer.body.enrollment_evidence.digest),
        ),
    ]
}

fn digest_vectors_json(vectors: &[DigestVector]) -> Value {
    Value::Array(
        vectors
            .iter()
            .map(|vector| {
                let preimage = kagemusha_wallet_preimage_v1(vector.role, &vector.body);
                json_object(vec![
                    ("role", json_text(vector.role.as_str())),
                    ("object", json_text(vector.object)),
                    ("body_hex", json_hex(&vector.body)),
                    ("preimage_hex", json_hex(&preimage)),
                    (
                        "digest_hex",
                        json_hex(&kagemusha_wallet_digest_v1(vector.role, &vector.body)),
                    ),
                    ("stand_in_proof", Value::Bool(vector.stand_in_proof)),
                ])
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------------------
// Signature vectors and the low-S boundary (design C11)
// ---------------------------------------------------------------------------------------

/// One frozen signature of a vectored object.
struct SignatureVector {
    object: &'static str,
    role: Role,
    key: KagemushaDevicePublicKeyV1,
    body: Vec<u8>,
    signature: KagemushaDeviceSignatureV1,
}

/// Every signed body of the world with its signer key and frozen signature.
fn signature_vectors(w: &VectorWorld) -> Vec<SignatureVector> {
    let f = &w.f;
    let payer = w.payer();
    let payer_key = payer.body.payment_key;
    let regulator_key = f.regulator_certificate.body.key;
    let certificate = &f.payer.enrollment_certificate;
    let request = &w.payment.request;
    let send = &w.payment.send;
    let receipt_body = send
        .receipt
        .body(payer, &send.statement, &send.proof)
        .expect("receipt body");
    let KagemushaWalletRenewalEvidenceV1::Android {
        new_attested_key,
        key_binding_signature,
        ..
    } = w.renewal.evidence
    else {
        panic!("android renewal");
    };
    vec![
        SignatureVector {
            object: "payer issuer certificate",
            role: Role::CertificateBody,
            key: w.scheme().scheme_root_key,
            body: certificate.body.transcript(),
            signature: certificate.signature,
        },
        SignatureVector {
            object: "payer credential",
            role: Role::CredentialBody,
            key: certificate.body.key,
            body: payer.body.transcript(),
            signature: payer.signature,
        },
        SignatureVector {
            object: "Offer",
            role: Role::OfferBody,
            key: payer_key,
            body: w.offer.body.transcript(),
            signature: w.offer.signature,
        },
        SignatureVector {
            object: "Request",
            role: Role::RequestBody,
            key: request.receiver_credential.body.payment_key,
            body: request.body.transcript(),
            signature: request.signature,
        },
        SignatureVector {
            object: "Send receipt",
            role: Role::ReceiptBody,
            key: payer_key,
            body: receipt_body.transcript(),
            signature: send.receipt.signature,
        },
        SignatureVector {
            object: "Close session control",
            role: Role::SessionControlBody,
            key: payer_key,
            body: w.close.transcript(),
            signature: w.close_signature(),
        },
        SignatureVector {
            object: "scheme policy",
            role: Role::SchemePolicyBody,
            key: regulator_key,
            body: w.scheme_policy.body.transcript(),
            signature: w.scheme_policy.signature,
        },
        SignatureVector {
            object: "fee schedule",
            role: Role::FeeScheduleBody,
            key: regulator_key,
            body: w.fee_schedule.body.transcript(),
            signature: w.fee_schedule.signature,
        },
        SignatureVector {
            object: "blacklist",
            role: Role::BlacklistBody,
            key: regulator_key,
            body: w.blacklist.body.transcript(),
            signature: w.blacklist.signature,
        },
        SignatureVector {
            object: "quota share",
            role: Role::QuotaShareBody,
            key: regulator_key,
            body: w.quota_share.body.transcript(),
            signature: w.quota_share.signature,
        },
        SignatureVector {
            object: "time anchor",
            role: Role::TimeAnchorBody,
            key: w.time_certificate.body.key,
            body: w.time_anchor.body.transcript(),
            signature: w.time_anchor.signature,
        },
        SignatureVector {
            object: "load voucher",
            role: Role::VoucherBody,
            key: w.load_certificate.body.key,
            body: w.voucher.body.transcript(),
            signature: w.voucher.signature,
        },
        SignatureVector {
            object: "Activate ledger control",
            role: Role::LedgerControlBody,
            key: payer_key,
            body: w.control.body.transcript(),
            signature: w.control.signature,
        },
        SignatureVector {
            object: "renewal possession",
            role: Role::RenewalChallenge,
            key: payer_key,
            body: w.renewal.possession_transcript(),
            signature: w.renewal.possession_signature,
        },
        SignatureVector {
            object: "Android renewal key binding",
            role: Role::RenewalKeyBinding,
            key: payer_key,
            body: kagemusha_wallet_renewal_key_binding_transcript_v1(
                &w.renewal.scheme_id,
                &w.renewal.wallet_id,
                &w.renewal.challenge,
                &new_attested_key,
            ),
            signature: key_binding_signature,
        },
        SignatureVector {
            object: "artifact manifest",
            role: Role::ArtifactManifestBody,
            key: w.artifact_certificate.body.key,
            body: w.manifest.body.transcript(),
            signature: w.manifest.signature,
        },
        SignatureVector {
            object: "load charge quote",
            role: Role::ChargeQuoteBody,
            key: regulator_key,
            body: w.charge_quote.body.transcript(),
            signature: w.charge_quote.signature,
        },
    ]
}

/// Verdicts of 64 raw bytes over `preimage` under `key`.
///
/// `codec_ok`: the canonical fixed-width low-S codec accepts the bytes (`1 <= r < n`,
/// `1 <= s <= floor(n/2)`). `verify_ok`: the ECDSA-P256-SHA256 equation holds with the scalars
/// taken as they are, as a generic verifier (JCA, `CryptoKit`) would check it; a high-S twin
/// satisfies it. A consumer accepts exactly when both hold.
fn signature_verdicts(
    key: &KagemushaDevicePublicKeyV1,
    preimage: &[u8],
    raw: &[u8; 64],
) -> (bool, bool) {
    let codec = KagemushaDeviceSignatureV1::from_raw_bytes(raw);
    let equation = P256Signature::from_slice(raw).is_ok_and(|signature| {
        VerifyingKey::from_sec1_bytes(key.as_sec1_bytes())
            .expect("canonical key")
            .verify(preimage, &signature)
            .is_ok()
    });
    if let Ok(signature) = codec {
        assert_eq!(
            signature.verify(key, preimage).is_ok(),
            equation,
            "canonical verifier"
        );
    }
    (codec.is_ok(), equation)
}

/// `n - s` of a low-S signature: its high-S twin.
fn high_s_twin(signature: &KagemushaDeviceSignatureV1) -> [u8; 64] {
    let raw = signature.as_raw_bytes();
    let s = Option::<Scalar>::from(Scalar::from_repr(*FieldBytes::from_slice(&raw[32..])))
        .expect("s scalar");
    let mut twin = *raw;
    twin[32..].copy_from_slice(&(-s).to_repr());
    twin
}

fn signature_vectors_json(vectors: &[SignatureVector]) -> Value {
    Value::Array(
        vectors
            .iter()
            .map(|vector| {
                let preimage = kagemusha_wallet_preimage_v1(vector.role, &vector.body);
                let raw = vector.signature.as_raw_bytes();
                assert_eq!(
                    signature_verdicts(&vector.key, &preimage, raw),
                    (true, true),
                    "{}",
                    vector.object
                );
                let twin = high_s_twin(&vector.signature);
                assert_eq!(
                    signature_verdicts(&vector.key, &preimage, &twin),
                    (false, true),
                    "{} twin",
                    vector.object
                );
                let twin_der = P256Signature::from_slice(&twin).expect("twin").to_der();
                for output in [
                    KagemushaWalletSignerOutputV1::Raw(twin),
                    KagemushaWalletSignerOutputV1::Der(twin_der.as_bytes()),
                ] {
                    assert_eq!(
                        kagemusha_wallet_freeze_signature_v1(
                            &vector.key,
                            vector.role,
                            &vector.body,
                            output
                        )
                        .expect("freeze twin"),
                        vector.signature
                    );
                }
                json_object(vec![
                    ("object", json_text(vector.object)),
                    ("role", json_text(vector.role.as_str())),
                    ("public_key_hex", json_hex(vector.key.as_sec1_bytes())),
                    ("preimage_hex", json_hex(&preimage)),
                    (
                        "e_hex",
                        json_hex(&kagemusha_wallet_digest_v1(vector.role, &vector.body)),
                    ),
                    ("signature_hex", json_hex(raw)),
                    ("codec_ok", Value::Bool(true)),
                    ("verify_ok", Value::Bool(true)),
                    (
                        "high_s_twin",
                        json_object(vec![
                            ("signature_hex", json_hex(&twin)),
                            ("der_hex", json_hex(twin_der.as_bytes())),
                            ("codec_ok", Value::Bool(false)),
                            ("verify_ok", Value::Bool(true)),
                            ("frozen_signature_hex", json_hex(raw)),
                        ]),
                    ),
                ])
            })
            .collect(),
    )
}

/// Signature with `s = floor(n/2)` that verifies, derived per design C11.
///
/// Choose the nonce `k` and `s = floor(n/2)`; with `r = x(kG) mod n` and
/// `e = SHA-256(preimage) mod n`, the key `d = (s*k - e) * r^-1 mod n` makes `(r, s)` a valid
/// signature: `u1*G + u2*Q = (e + r*d)/s * G = k*G`.
struct BoundaryVector {
    k: Scalar,
    r: Scalar,
    e: Scalar,
    d: Scalar,
    key: KagemushaDevicePublicKeyV1,
    preimage: Vec<u8>,
}

#[allow(
    clippy::many_single_char_names,
    reason = "the ECDSA scalars k, r, e, s and d keep their standard names"
)]
fn boundary_vector() -> BoundaryVector {
    let preimage = kagemusha_wallet_preimage_v1(BOUNDARY_ROLE, BOUNDARY_BODY);
    let k = <Scalar as Reduce<U256>>::reduce_bytes(FieldBytes::from_slice(&BOUNDARY_NONCE));
    let r_point = (ProjectivePoint::GENERATOR * k).to_affine();
    let r = <Scalar as Reduce<U256>>::reduce_bytes(&r_point.x());
    let e = <Scalar as Reduce<U256>>::reduce_bytes(FieldBytes::from_slice(
        Sha256::digest(&preimage).as_slice(),
    ));
    let s = Option::<Scalar>::from(Scalar::from_repr(*FieldBytes::from_slice(&bytes32(
        HALF_ORDER_HEX,
    ))))
    .expect("half order is a scalar");
    let r_inverse = Option::<Scalar>::from(r.invert()).expect("r is nonzero");
    let d = (s * k - e) * r_inverse;
    assert!(!bool::from(d.is_zero()), "derived key is nonzero");
    let signing = SigningKey::from_bytes(&d.to_repr()).expect("derived key");
    BoundaryVector {
        k,
        r,
        e,
        d,
        key: public_key(&signing),
        preimage,
    }
}

fn raw_pair(r: &[u8], s: &[u8]) -> [u8; 64] {
    let mut raw = [0; 64];
    raw[..32].copy_from_slice(r);
    raw[32..].copy_from_slice(s);
    raw
}

/// Boundary cases with their expected verdicts `(codec_ok, verify_ok)`.
fn boundary_cases(boundary: &BoundaryVector) -> Vec<(&'static str, [u8; 64], bool, bool)> {
    let order = bytes32(ORDER_HEX);
    let half = bytes32(HALF_ORDER_HEX);
    let r = boundary.r.to_repr();
    let half_scalar =
        Option::<Scalar>::from(Scalar::from_repr(*FieldBytes::from_slice(&half))).expect("half");
    let half_plus_one = (-half_scalar).to_repr();
    vec![
        ("s_half_order", raw_pair(&r, &half), true, true),
        (
            "s_half_order_plus_one_high_s_twin",
            raw_pair(&r, &half_plus_one),
            false,
            true,
        ),
        ("r_zero", raw_pair(&[0; 32], &half), false, false),
        ("s_zero", raw_pair(&r, &[0; 32]), false, false),
        ("r_order", raw_pair(&order, &half), false, false),
        ("s_order", raw_pair(&r, &order), false, false),
    ]
}

fn boundary_json() -> Value {
    let boundary = boundary_vector();
    let cases = boundary_cases(&boundary);
    let mut rows = Vec::new();
    for (name, raw, codec_ok, verify_ok) in &cases {
        assert_eq!(
            signature_verdicts(&boundary.key, &boundary.preimage, raw),
            (*codec_ok, *verify_ok),
            "{name}"
        );
        rows.push(json_object(vec![
            ("name", json_text(name)),
            ("signature_hex", json_hex(raw)),
            ("codec_ok", Value::Bool(*codec_ok)),
            ("verify_ok", Value::Bool(*verify_ok)),
        ]));
    }
    // The high twin freezes to the boundary signature itself.
    let frozen = kagemusha_wallet_freeze_signature_v1(
        &boundary.key,
        BOUNDARY_ROLE,
        BOUNDARY_BODY,
        KagemushaWalletSignerOutputV1::Raw(cases[1].1),
    )
    .expect("freeze the high twin");
    assert_eq!(frozen.as_raw_bytes(), &cases[0].1);
    let order = bytes32(ORDER_HEX);
    let half = bytes32(HALF_ORDER_HEX);
    json_object(vec![
        ("order_hex", json_hex(&order)),
        ("half_order_hex", json_hex(&half)),
        ("half_order_plus_one_hex", json_hex(&cases[1].1[32..])),
        (
            "derivation",
            json_text(
                "r = x(k*G) mod n; s = floor(n/2); e = SHA-256(preimage) mod n; \
                 d = (s*k - e) * r^-1 mod n; public key Q = d*G",
            ),
        ),
        ("role", json_text(BOUNDARY_ROLE.as_str())),
        ("body_hex", json_hex(BOUNDARY_BODY)),
        ("preimage_hex", json_hex(&boundary.preimage)),
        ("k_hex", json_hex(&boundary.k.to_repr())),
        ("r_hex", json_hex(&boundary.r.to_repr())),
        ("e_hex", json_hex(&boundary.e.to_repr())),
        ("d_hex", json_hex(&boundary.d.to_repr())),
        ("public_key_hex", json_hex(boundary.key.as_sec1_bytes())),
        ("cases", Value::Array(rows)),
    ])
}

// ---------------------------------------------------------------------------------------
// Envelope vectors and frame identities
// ---------------------------------------------------------------------------------------

/// One message of every kind and variant, in tag order.
fn envelope_messages(w: &VectorWorld) -> Vec<(&'static str, KagemushaWalletMessageV1, bool)> {
    vec![
        (
            "Offer",
            KagemushaWalletMessageV1::Offer {
                offer: w.offer.clone(),
            },
            false,
        ),
        (
            "Request",
            KagemushaWalletMessageV1::Request {
                request: w.payment.request.clone(),
            },
            false,
        ),
        (
            "Payment",
            KagemushaWalletMessageV1::Payment {
                payment: w.payment.clone(),
            },
            true,
        ),
        (
            "Credited::Receive",
            KagemushaWalletMessageV1::Credited {
                credited: w.credited_receive.clone(),
            },
            true,
        ),
        (
            "Credited::Status",
            KagemushaWalletMessageV1::Credited {
                credited: w.credited_status.clone(),
            },
            true,
        ),
        (
            "SessionControl::Close",
            KagemushaWalletMessageV1::SessionControl { control: w.close },
            false,
        ),
        (
            "SessionControl::UnsupportedScheme",
            KagemushaWalletMessageV1::SessionControl {
                control: w.unsupported,
            },
            false,
        ),
        (
            "PolicyData::SchemePolicy",
            KagemushaWalletMessageV1::PolicyData {
                data: w.policy_data(KagemushaWalletPolicyDataItemV1::SchemePolicy {
                    policy: w.scheme_policy,
                }),
            },
            false,
        ),
    ]
}

fn message_kind_name(message: &KagemushaWalletMessageV1) -> &'static str {
    match message {
        KagemushaWalletMessageV1::Offer { .. } => "Offer",
        KagemushaWalletMessageV1::Request { .. } => "Request",
        KagemushaWalletMessageV1::Payment { .. } => "Payment",
        KagemushaWalletMessageV1::Credited { .. } => "Credited",
        KagemushaWalletMessageV1::SessionControl { .. } => "SessionControl",
        KagemushaWalletMessageV1::PolicyData { .. } => "PolicyData",
    }
}

fn envelope_vectors_json(w: &VectorWorld) -> Value {
    let scheme_id = w.scheme_id();
    let mut rows = Vec::new();
    for (variant, message, stand_in_proof) in envelope_messages(w) {
        let envelope = KagemushaWalletEnvelopeV1::new(message);
        let frame = envelope.to_canonical_bytes().expect("envelope frame");
        assert_eq!(
            KagemushaWalletEnvelopeV1::decode_canonical(&frame, &scheme_id).expect("decode"),
            envelope,
            "{variant}"
        );
        let header = norito::core::Header::read(frame.as_slice()).expect("header");
        let payload = payload_range(&frame);
        assert_eq!(
            header.schema,
            norito::schema::identity::frame_hash::<KagemushaWalletEnvelopeV1>()
        );
        assert_eq!(header.flags, norito::core::header_flags::COMPACT_LEN);
        assert_eq!(
            usize::try_from(header.length).expect("length"),
            payload.len()
        );
        assert_eq!(
            header.checksum,
            norito::crc64_fallback(&frame[payload.clone()])
        );
        let text = envelope.to_text().expect("text");
        assert_eq!(
            KagemushaWalletEnvelopeV1::from_text(&text, &scheme_id).expect("from text"),
            envelope
        );
        let bound = envelope.message.max_bytes();
        let text_bound = if bound == KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1 {
            KAGEMUSHA_WALLET_SESSION_TEXT_MAX_BYTES_V1
        } else {
            KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1
        };
        assert!(frame.len() <= bound && text.len() <= text_bound);
        rows.push(json_object(vec![
            ("kind", json_text(message_kind_name(&envelope.message))),
            ("variant", json_text(variant)),
            ("tag", json_number(usize::from(envelope.message.tag()))),
            (
                "frame_name",
                json_text(&<KagemushaWalletEnvelopeV1 as norito::NoritoSchema>::frame_name()),
            ),
            ("schema_hash_hex", json_hex(&header.schema)),
            ("flags", json_number(usize::from(header.flags))),
            (
                "padding_len",
                json_number(payload.start - norito::core::Header::SIZE),
            ),
            ("payload_len", json_number(payload.len())),
            ("frame_len", json_number(frame.len())),
            ("crc64_hex", json_text(&format!("{:016x}", header.checksum))),
            ("canonical_hex", json_hex(&frame)),
            ("text", json_text(&text)),
            ("bound", json_number(bound)),
            ("text_bound", json_number(text_bound)),
            ("scheme_id_hex", json_hex(&scheme_id)),
            ("stand_in_proof", Value::Bool(stand_in_proof)),
        ]));
    }
    Value::Array(rows)
}

/// Frame identity of one top-level record.
struct FramePin {
    short_name: &'static str,
    frame_name: String,
    frame_hash: [u8; 16],
    max_bytes: usize,
}

fn frame_pin<T: norito::NoritoSchema>(short_name: &'static str, max_bytes: usize) -> FramePin {
    let frame_name = <T as norito::NoritoSchema>::frame_name();
    assert_eq!(frame_name, format!("{FRAME_NAME_PREFIX}{short_name}"));
    let frame_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(frame_hash, norito::core::schema_hash_for_name(&frame_name));
    FramePin {
        short_name,
        frame_name,
        frame_hash,
        max_bytes,
    }
}

/// Frame identities and caps of the envelope and every top-level record.
fn frame_pins() -> Vec<FramePin> {
    vec![
        frame_pin::<KagemushaWalletEnvelopeV1>(
            "KagemushaWalletEnvelopeV1",
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletSchemeV1>(
            "KagemushaWalletSchemeV1",
            KAGEMUSHA_WALLET_SCHEME_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletSignerCertificateV1>(
            "KagemushaWalletSignerCertificateV1",
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletCredentialV1>(
            "KagemushaWalletCredentialV1",
            KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletRenewalRequestV1>(
            "KagemushaWalletRenewalRequestV1",
            KAGEMUSHA_WALLET_RENEWAL_REQUEST_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletArtifactManifestV1>(
            "KagemushaWalletArtifactManifestV1",
            KAGEMUSHA_WALLET_ARTIFACT_MANIFEST_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletSchemePolicyV1>(
            "KagemushaWalletSchemePolicyV1",
            KAGEMUSHA_WALLET_SCHEME_POLICY_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletFeeScheduleV1>(
            "KagemushaWalletFeeScheduleV1",
            KAGEMUSHA_WALLET_FEE_SCHEDULE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletBlacklistV1>(
            "KagemushaWalletBlacklistV1",
            KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletQuotaShareV1>(
            "KagemushaWalletQuotaShareV1",
            KAGEMUSHA_WALLET_QUOTA_SHARE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletTimeAnchorV1>(
            "KagemushaWalletTimeAnchorV1",
            KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletChargeQuoteV1>(
            "KagemushaWalletChargeQuoteV1",
            KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletPaymentV1>(
            "KagemushaWalletPaymentV1",
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletPackageV1>(
            "KagemushaWalletPackageV1",
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletMarkerV1>(
            "KagemushaWalletMarkerV1",
            KAGEMUSHA_WALLET_MARKER_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletRecoveryCapsuleV1>(
            "KagemushaWalletRecoveryCapsuleV1",
            KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletCompletionRecordV1>(
            "KagemushaWalletCompletionRecordV1",
            KAGEMUSHA_WALLET_COMPLETION_RECORD_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletLoadVoucherV1>(
            "KagemushaWalletLoadVoucherV1",
            KAGEMUSHA_WALLET_LOAD_VOUCHER_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletUnloadClaimV1>(
            "KagemushaWalletUnloadClaimV1",
            KAGEMUSHA_WALLET_UNLOAD_CLAIM_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletFeeClaimV1>(
            "KagemushaWalletFeeClaimV1",
            KAGEMUSHA_WALLET_FEE_CLAIM_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletLedgerControlV1>(
            "KagemushaWalletLedgerControlV1",
            KAGEMUSHA_WALLET_LEDGER_CONTROL_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletActivationV1>(
            "KagemushaWalletActivationV1",
            KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletCloseLoadsV1>(
            "KagemushaWalletCloseLoadsV1",
            KAGEMUSHA_WALLET_CLOSE_LOADS_MAX_BYTES_V1,
        ),
        frame_pin::<KagemushaWalletAbandonmentV1>(
            "KagemushaWalletAbandonmentV1",
            KAGEMUSHA_WALLET_ABANDONMENT_MAX_BYTES_V1,
        ),
    ]
}

fn frame_pins_json() -> Value {
    Value::Array(
        frame_pins()
            .iter()
            .map(|pin| {
                json_object(vec![
                    ("type", json_text(pin.short_name)),
                    ("frame_name", json_text(&pin.frame_name)),
                    ("frame_hash_hex", json_hex(&pin.frame_hash)),
                    ("max_bytes", json_number(pin.max_bytes)),
                ])
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------------------
// Canonical object pins (design §8: a canonical-bytes pin for every object)
// ---------------------------------------------------------------------------------------

/// Unload charge of the vectored unload claim.
const UNLOAD_AMOUNT: u128 = 700;
const UNLOAD_CHARGE: u128 = 7;
/// Stand-in capsule digest of the vectored Bootstrap head marker (the payer's capsule is not
/// vectored).
const HEAD_CAPSULE: [u8; 32] = [0x6b; 32];

/// Complete canonical frame of one vectored object.
struct ObjectPin {
    short_name: &'static str,
    variant: &'static str,
    frame_name: String,
    frame: Vec<u8>,
    stand_in_proof: bool,
}

/// Pin `frame`, the canonical frame of `value`, after checking that `decoded` (its bounded
/// decode) round-trips and that it is exactly `norito::encode_canonical(value)`.
fn object_pin<T>(
    short_name: &'static str,
    variant: &'static str,
    value: &T,
    frame: Vec<u8>,
    decoded: &T,
    stand_in_proof: bool,
) -> ObjectPin
where
    T: norito::NoritoSchema + norito::NoritoSerialize + PartialEq + core::fmt::Debug,
{
    assert_eq!(decoded, value, "{short_name} {variant} round trip");
    assert_eq!(
        norito::encode_canonical(value).expect("encode"),
        frame,
        "{short_name} {variant} canonical frame"
    );
    let frame_name = <T as norito::NoritoSchema>::frame_name();
    assert_eq!(frame_name, format!("{FRAME_NAME_PREFIX}{short_name}"));
    let header = norito::core::Header::read(frame.as_slice()).expect("header");
    assert_eq!(
        header.schema,
        norito::core::schema_hash_for_name(&frame_name)
    );
    ObjectPin {
        short_name,
        variant,
        frame_name,
        frame,
        stand_in_proof,
    }
}

/// Pin a value without a standalone frame bound (`norito::encode_canonical` only).
fn unbounded_pin<T>(short_name: &'static str, variant: &'static str, value: &T) -> ObjectPin
where
    T: norito::NoritoSchema
        + norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + PartialEq
        + core::fmt::Debug,
{
    let frame = norito::encode_canonical(value).expect("encode");
    let decoded: T =
        norito::decode_canonical_with_limits(&frame, norito::canonical_decode_limits(frame.len()))
            .expect("decode");
    object_pin(short_name, variant, value, frame, &decoded, false)
}

/// Ledger boundary objects of the payer built on the vector world.
struct LedgerObjects {
    unload_claim: KagemushaWalletUnloadClaimV1,
    fee_claim: KagemushaWalletFeeClaimV1,
    activation: KagemushaWalletActivationV1,
    close_loads: KagemushaWalletCloseLoadsV1,
    abandonment: KagemushaWalletAbandonmentV1,
    head_marker: KagemushaWalletMarkerV1,
    deleted_marker: KagemushaWalletMarkerV1,
    abandoned_marker: KagemushaWalletMarkerV1,
    anchored_time: KagemushaWalletAnchoredTimeV1,
}

/// Payer ledger control signed by its payment key.
fn payer_control(
    w: &VectorWorld,
    action: KagemushaWalletLedgerControlActionV1,
    nonce: [u8; 32],
) -> KagemushaWalletLedgerControlV1 {
    let payer = w.payer();
    let body = KagemushaWalletLedgerControlBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: payer.body.scheme_id,
        asset_digest: payer.body.asset_digest,
        wallet_id: payer.body.wallet_id,
        action,
        nonce,
    };
    KagemushaWalletLedgerControlV1::sign(
        body,
        &payer.body.payment_key,
        raw_output(&w.f.payer.payment, &body.signing_message()),
    )
    .expect("payer control")
}

fn ledger_objects(w: &VectorWorld) -> LedgerObjects {
    let f = &w.f;
    let payer = w.payer();
    let scheme_id = w.scheme_id();
    let issuer_set = KagemushaWalletCertificateSetV1::new(vec![f.payer.enrollment_certificate])
        .expect("issuer set");

    // A charged Unload of the payer and its self-contained claim (design C7).
    let beneficiary = test_account(BENEFICIARY_SEED);
    let quote_body = KagemushaWalletChargeQuoteBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id,
        asset_digest: payer.body.asset_digest,
        wallet_id: payer.body.wallet_id,
        kind: KagemushaWalletChargeKindV1::Unload,
        ordinal: 0,
        net_amount: UNLOAD_AMOUNT,
        online_charge: UNLOAD_CHARGE,
        beneficiary_account_digest: kagemusha_wallet_account_digest_v1(&beneficiary)
            .expect("beneficiary"),
        issued_at_ms: ACCEPTED_MS - 60_000,
        signer_certificate: f.regulator_certificate.certificate_digest(),
    };
    let quote = KagemushaWalletChargeQuoteV1::sign(
        quote_body,
        &f.regulator_certificate,
        raw_output(&f.regulator, &quote_body.signing_message()),
    )
    .expect("unload quote");
    let unload = transition_statement(
        &f.payer,
        6,
        1,
        KagemushaWalletLifecycleV1::Active,
        KagemushaWalletEffectV1::Unload {
            nullifier: kagemusha_wallet_unload_nullifier_v1(&scheme_id, &payer.body.wallet_id, 0),
            redeem_ordinal: 0,
            amount: UNLOAD_AMOUNT,
            online_charge: UNLOAD_CHARGE,
            charge_quote: quote.charge_quote_digest(),
        },
    );
    let unload_claim = KagemushaWalletUnloadClaimV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        credential: *payer,
        package: signed_package(&f.payer, payer, &unload, stand_in_proof(VECTOR_PROOF_LEN)),
        account: test_account(PAYER_SEED),
        charge: KagemushaWalletUnloadChargeV1::Quoted { quote, beneficiary },
        certificates: KagemushaWalletCertificateSetV1::new(vec![
            f.payer.enrollment_certificate,
            f.regulator_certificate,
        ])
        .expect("claim certificates"),
    };
    unload_claim
        .verify(&w.scheme())
        .expect("unload claim verifies");

    let fee_claim = KagemushaWalletFeeClaimV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        payment: w.payment.clone(),
        beneficiary: test_account(BENEFICIARY_SEED),
    };
    fee_claim.verify(&w.scheme()).expect("fee claim verifies");

    let activation = KagemushaWalletActivationV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        control: w.control,
        credential: *payer,
        bootstrap: w.bootstrap.clone(),
        asset: f.payer.asset.clone(),
        certificates: issuer_set.clone(),
    };
    activation.verify(&w.scheme()).expect("activation verifies");

    let retiring = signed_package(
        &f.payer,
        payer,
        &transition_statement(
            &f.payer,
            7,
            1,
            KagemushaWalletLifecycleV1::Retiring,
            KagemushaWalletEffectV1::Retiring,
        ),
        stand_in_proof(VECTOR_PROOF_LEN),
    );
    let close_loads = KagemushaWalletCloseLoadsV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        control: payer_control(
            w,
            KagemushaWalletLedgerControlActionV1::CloseLoads {
                package_digest: retiring.package_digest(payer).expect("retiring digest"),
                next_load: 1,
            },
            [0x6d; 32],
        ),
        credential: *payer,
        package: retiring,
        certificates: issuer_set,
    };
    close_loads
        .verify(&w.scheme())
        .expect("close loads verifies");

    let head_marker = w
        .marker
        .successor(KagemushaWalletMarkerStateV1::Head {
            sequence: 0,
            operation_id: w.bootstrap.statement.operation_id(&payer.body.wallet_id),
            head: w.bootstrap.statement.successor,
            capsule_digest: HEAD_CAPSULE,
            predecessor_capsule_digest: [0; 32],
        })
        .expect("head marker");
    let deleted_marker = head_marker
        .successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::CustodyDeleted,
            last_capsule_digest: HEAD_CAPSULE,
        })
        .expect("custody-deleted marker");
    let abandoned_marker = w
        .marker
        .successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::Abandoned,
            last_capsule_digest: [0; 32],
        })
        .expect("abandoned marker");
    let challenge_digest = f.payer.challenge.challenge_digest();
    let abandon_body = KagemushaWalletAbandonmentV1::control_body(
        &abandoned_marker,
        &challenge_digest,
        [0x6e; 32],
    )
    .expect("abandon body");
    let abandonment = KagemushaWalletAbandonmentV1::sign(
        &abandoned_marker,
        challenge_digest,
        [0x6e; 32],
        raw_output(&f.payer.payment, &abandon_body.signing_message()),
    )
    .expect("abandonment");

    let anchored_time =
        KagemushaWalletAnchoredTimeV1::new(w.time_anchor, [0xb0; 32], 1_000, 1_250, 5_000)
            .expect("anchored time");

    LedgerObjects {
        unload_claim,
        fee_claim,
        activation,
        close_loads,
        abandonment,
        head_marker,
        deleted_marker,
        abandoned_marker,
        anchored_time,
    }
}

/// Encode `$value` with its own bounded encoder, decode it with its own bounded decoder under
/// `$expected`, and pin the frame.
macro_rules! framed_pin {
    ($ty:ident, $variant:literal, $value:expr, $expected:expr, $stand_in:expr) => {{
        let value = &$value;
        let frame = value.to_canonical_bytes().expect(stringify!($ty));
        let decoded = $ty::decode_canonical(&frame, $expected).expect(stringify!($ty));
        object_pin(stringify!($ty), $variant, value, frame, &decoded, $stand_in)
    }};
}

/// One canonical frame of every framed object type other than the envelope (pinned by the
/// envelope vectors), every marker state, and the unframed asset scope and anchored time.
fn object_pins(w: &VectorWorld) -> Vec<ObjectPin> {
    let scheme = w.scheme();
    let scheme_id = w.scheme_id();
    let receiver_wallet = w.receiver().body.wallet_id;
    let l = ledger_objects(w);
    let package_frame =
        encode_frame_v1(&w.receive, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).expect("package frame");
    let decoded_package: KagemushaWalletPackageV1 =
        decode_frame_v1(&package_frame, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)
            .expect("package decode");
    decoded_package
        .verify(w.receiver())
        .expect("package verifies");
    vec![
        framed_pin!(KagemushaWalletSchemeV1, "", scheme, &scheme_id, false),
        framed_pin!(
            KagemushaWalletSignerCertificateV1,
            "RegulatoryPolicy",
            w.f.regulator_certificate,
            &scheme,
            false
        ),
        framed_pin!(
            KagemushaWalletCredentialV1,
            "AndroidKeyMintTee",
            *w.payer(),
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletRenewalRequestV1,
            "Android",
            w.renewal,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletArtifactManifestV1,
            "",
            w.manifest,
            &scheme,
            false
        ),
        framed_pin!(
            KagemushaWalletSchemePolicyV1,
            "",
            w.scheme_policy,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletFeeScheduleV1,
            "",
            w.fee_schedule,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletBlacklistV1,
            "3 entries",
            w.blacklist,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletQuotaShareV1,
            "2 windows",
            w.quota_share,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletTimeAnchorV1,
            "",
            w.time_anchor,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletChargeQuoteV1,
            "Load",
            w.charge_quote,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletPaymentV1,
            "fee, 3 certificates",
            w.payment,
            &scheme_id,
            true
        ),
        object_pin(
            "KagemushaWalletPackageV1",
            "Receive",
            &w.receive,
            package_frame,
            &decoded_package,
            true,
        ),
        framed_pin!(
            KagemushaWalletMarkerV1,
            "Enrollment",
            w.marker,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletMarkerV1,
            "Head",
            l.head_marker,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletMarkerV1,
            "Terminal::CustodyDeleted",
            l.deleted_marker,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletMarkerV1,
            "Terminal::Abandoned",
            l.abandoned_marker,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletRecoveryCapsuleV1,
            "Receive",
            w.capsule,
            &scheme_id,
            true
        ),
        framed_pin!(
            KagemushaWalletCompletionRecordV1,
            "Receive",
            w.completion,
            &receiver_wallet,
            true
        ),
        framed_pin!(
            KagemushaWalletLoadVoucherV1,
            "quoted",
            w.voucher,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletUnloadClaimV1,
            "quoted",
            l.unload_claim,
            &scheme_id,
            true
        ),
        framed_pin!(KagemushaWalletFeeClaimV1, "", l.fee_claim, &scheme_id, true),
        framed_pin!(
            KagemushaWalletLedgerControlV1,
            "Activate",
            w.control,
            &scheme_id,
            false
        ),
        framed_pin!(
            KagemushaWalletActivationV1,
            "",
            l.activation,
            &scheme_id,
            true
        ),
        framed_pin!(
            KagemushaWalletCloseLoadsV1,
            "",
            l.close_loads,
            &scheme_id,
            true
        ),
        framed_pin!(
            KagemushaWalletAbandonmentV1,
            "",
            l.abandonment,
            &scheme_id,
            false
        ),
        unbounded_pin("KagemushaWalletAssetScopeV1", "", &w.f.payer.asset),
        unbounded_pin("KagemushaWalletAnchoredTimeV1", "", &l.anchored_time),
    ]
}

fn object_pins_json(w: &VectorWorld) -> Value {
    Value::Array(
        object_pins(w)
            .iter()
            .map(|pin| {
                json_object(vec![
                    ("type", json_text(pin.short_name)),
                    ("variant", json_text(pin.variant)),
                    ("frame_name", json_text(&pin.frame_name)),
                    ("frame_len", json_number(pin.frame.len())),
                    ("canonical_hex", json_hex(&pin.frame)),
                    ("stand_in_proof", Value::Bool(pin.stand_in_proof)),
                ])
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------------------
// Enum tags
// ---------------------------------------------------------------------------------------

/// Variants of one wire enum: name, transcript tag and Norito wire tag.
struct EnumTags {
    name: &'static str,
    variants: Vec<(&'static str, u8, u32)>,
}

fn tag_row<T: norito::NoritoSerialize>(
    variant: &'static str,
    value: &T,
    tag: u8,
) -> (&'static str, u8, u32) {
    (variant, tag, norito_tag(value))
}

fn unit_tags<T: Copy + norito::NoritoSerialize>(
    name: &'static str,
    values: &[T],
    names: &[&'static str],
    tag: impl Fn(T) -> u8,
) -> EnumTags {
    assert_eq!(values.len(), names.len(), "{name}");
    EnumTags {
        name,
        variants: values
            .iter()
            .zip(names)
            .map(|(value, &variant)| tag_row(variant, value, tag(*value)))
            .collect(),
    }
}

/// Every Norito wire enum of the module with one sample of every variant.
fn enum_tag_table(w: &VectorWorld) -> Vec<EnumTags> {
    use KagemushaWalletEffectV1 as Effect;
    use KagemushaWalletLedgerControlActionV1 as Action;
    use KagemushaWalletMarkerStateV1 as MarkerState;
    use KagemushaWalletMessageV1 as Message;
    use KagemushaWalletPolicyDataItemV1 as Item;

    let payment = &w.payment;
    let evidence_android = w.renewal.evidence.clone();
    let evidence_apple = KagemushaWalletRenewalEvidenceV1::Apple {
        assertion: b"app-attest-assertion".to_vec(),
    };
    let effects = [
        ("Bootstrap", w.bootstrap.statement.effect),
        ("Load", w.voucher.load_effect().expect("load effect")),
        ("Send", payment.send.statement.effect),
        ("Receive", w.receive.statement.effect),
        (
            "ArchiveSent",
            Effect::ArchiveSent {
                credit_id: w.credited_receive.credit_id,
                credited: w.credited_receive.credited_digest().expect("credited"),
            },
        ),
        (
            "Unload",
            Effect::Unload {
                nullifier: kagemusha_wallet_unload_nullifier_v1(
                    &w.scheme_id(),
                    &w.payer().body.wallet_id,
                    2,
                ),
                redeem_ordinal: 2,
                amount: 700,
                online_charge: 0,
                charge_quote: [0; 32],
            },
        ),
        (
            "RefreshPolicy",
            Effect::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::SchemePolicy,
                update: w.scheme_policy.scheme_policy_digest(),
                accepted_time_floor_ms: 0,
            },
        ),
        ("Retiring", Effect::Retiring),
    ];
    let markers = [
        ("Enrollment", w.marker.state),
        (
            "Head",
            w.capsule.head_marker_state().expect("head marker state"),
        ),
        (
            "Terminal",
            MarkerState::Terminal {
                reason: KagemushaWalletTerminalReasonV1::Abandoned,
                last_capsule_digest: [0; 32],
            },
        ),
    ];
    let actions = [
        ("Activate", w.control.body.action),
        (
            "CloseLoads",
            Action::CloseLoads {
                package_digest: [0x4c; 32],
                next_load: 1,
            },
        ),
        (
            "Abandon",
            Action::Abandon {
                enrollment_id: w.payer().body.enrollment_id,
                marker_generation: 1,
                terminal_marker_digest: [0x4d; 32],
            },
        ),
    ];
    let items = [
        (
            "SchemePolicy",
            Item::SchemePolicy {
                policy: w.scheme_policy,
            },
        ),
        (
            "FeeSchedule",
            Item::FeeSchedule {
                schedule: w.fee_schedule,
            },
        ),
        (
            "Certificates",
            Item::Certificates {
                certificates: KagemushaWalletCertificateSetV1::new(vec![w.f.regulator_certificate])
                    .expect("set"),
            },
        ),
    ];
    let messages: Vec<(&'static str, Message)> = envelope_messages(w)
        .into_iter()
        .filter(|(variant, _, _)| {
            !matches!(
                *variant,
                "Credited::Status" | "SessionControl::UnsupportedScheme"
            )
        })
        .map(|(_, message, _)| (message_kind_name(&message), message))
        .collect();

    vec![
        unit_tags(
            "KagemushaWalletSignerRoleV1",
            &KagemushaWalletSignerRoleV1::ALL,
            &[
                "Enrollment",
                "LoadAuthorization",
                "RegulatoryPolicy",
                "TimeAnchor",
                "Artifact",
            ],
            KagemushaWalletSignerRoleV1::tag,
        ),
        unit_tags(
            "KagemushaWalletEvidenceKindV1",
            &KagemushaWalletEvidenceKindV1::ALL,
            &[
                "AndroidKeyMintTee",
                "AndroidKeyMintStrongBox",
                "AppleAppAttest",
            ],
            KagemushaWalletEvidenceKindV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletRenewalEvidenceV1",
            variants: vec![
                tag_row("Android", &evidence_android, evidence_android.tag()),
                tag_row("Apple", &evidence_apple, evidence_apple.tag()),
            ],
        },
        unit_tags(
            "KagemushaWalletLifecycleV1",
            &KagemushaWalletLifecycleV1::ALL,
            &["Active", "Retiring"],
            KagemushaWalletLifecycleV1::tag,
        ),
        unit_tags(
            "KagemushaWalletOperationKindV1",
            &KagemushaWalletOperationKindV1::ALL,
            &[
                "Bootstrap",
                "Load",
                "Send",
                "Receive",
                "ArchiveSent",
                "Unload",
                "RefreshPolicy",
                "Retiring",
            ],
            KagemushaWalletOperationKindV1::tag,
        ),
        unit_tags(
            "KagemushaWalletPolicyUpdateKindV1",
            &KagemushaWalletPolicyUpdateKindV1::ALL,
            &[
                "Credential",
                "SchemePolicy",
                "Blacklist",
                "QuotaShare",
                "TimeAnchor",
            ],
            KagemushaWalletPolicyUpdateKindV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletEffectV1",
            variants: effects
                .iter()
                .map(|&(variant, ref effect)| tag_row(variant, effect, effect.tag()))
                .collect(),
        },
        unit_tags(
            "KagemushaWalletFeeRoundingV1",
            &KagemushaWalletFeeRoundingV1::ALL,
            &["Down", "Up"],
            KagemushaWalletFeeRoundingV1::tag,
        ),
        unit_tags(
            "KagemushaWalletQuotaWindowKindV1",
            &KagemushaWalletQuotaWindowKindV1::ALL,
            &["Daily", "Monthly"],
            KagemushaWalletQuotaWindowKindV1::tag,
        ),
        unit_tags(
            "KagemushaWalletChargeKindV1",
            &KagemushaWalletChargeKindV1::ALL,
            &["Load", "Unload"],
            KagemushaWalletChargeKindV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletFeeScheduleSlotV1",
            variants: [
                ("None", KagemushaWalletFeeScheduleSlotV1::None),
                (
                    "Present",
                    KagemushaWalletFeeScheduleSlotV1::Present {
                        schedule: w.fee_schedule,
                    },
                ),
            ]
            .iter()
            .map(|&(variant, ref slot)| tag_row(variant, slot, slot.tag()))
            .collect(),
        },
        EnumTags {
            name: "KagemushaWalletCreditedEvidenceV1",
            variants: [
                ("Receive", &w.credited_receive.evidence),
                ("Status", &w.credited_status.evidence),
            ]
            .iter()
            .map(|&(variant, evidence)| tag_row(variant, evidence, evidence.tag()))
            .collect(),
        },
        unit_tags(
            "KagemushaWalletSessionControlKindV1",
            &KagemushaWalletSessionControlKindV1::ALL,
            &[
                "SetupDeclined",
                "UnsupportedScheme",
                "ReceiveDeferred",
                "Close",
            ],
            KagemushaWalletSessionControlKindV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletSessionAuthV1",
            variants: [("Unsigned", w.unsupported.auth), ("Signed", w.close.auth)]
                .iter()
                .map(|&(variant, ref auth)| tag_row(variant, auth, auth.tag()))
                .collect(),
        },
        EnumTags {
            name: "KagemushaWalletPolicyDataItemV1",
            variants: items
                .iter()
                .map(|&(variant, ref item)| tag_row(variant, item, item.tag()))
                .collect(),
        },
        EnumTags {
            name: "KagemushaWalletMessageV1",
            variants: messages
                .iter()
                .map(|&(variant, ref message)| tag_row(variant, message, message.tag()))
                .collect(),
        },
        unit_tags(
            "KagemushaWalletTerminalReasonV1",
            &KagemushaWalletTerminalReasonV1::ALL,
            &["Abandoned", "CustodyDeleted"],
            KagemushaWalletTerminalReasonV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletMarkerStateV1",
            variants: markers
                .iter()
                .map(|&(variant, ref state)| tag_row(variant, state, state.tag()))
                .collect(),
        },
        unit_tags(
            "KagemushaWalletRetainedInputRoleV1",
            &KagemushaWalletRetainedInputRoleV1::ALL,
            &[
                "Request",
                "Payment",
                "Credited",
                "LoadVoucher",
                "ChargeQuote",
                "PolicyUpdate",
                "CertificateSet",
                "Credential",
            ],
            KagemushaWalletRetainedInputRoleV1::tag,
        ),
        EnumTags {
            name: "KagemushaWalletLedgerControlActionV1",
            variants: actions
                .iter()
                .map(|&(variant, ref action)| tag_row(variant, action, action.tag()))
                .collect(),
        },
        EnumTags {
            name: "KagemushaWalletUnloadChargeV1",
            variants: [
                ("None", KagemushaWalletUnloadChargeV1::None),
                ("Quoted", ledger_objects(w).unload_claim.charge),
            ]
            .iter()
            .map(|&(variant, ref charge)| tag_row(variant, charge, charge.tag()))
            .collect(),
        },
    ]
}

fn enum_tags_json(table: &[EnumTags]) -> Value {
    let mut map = Map::new();
    for entry in table {
        let rows = entry
            .variants
            .iter()
            .map(|(variant, tag, _)| {
                json_object(vec![
                    ("variant", json_text(variant)),
                    ("tag", json_number(usize::from(*tag))),
                ])
            })
            .collect();
        assert!(
            map.insert(entry.name.to_owned(), Value::Array(rows))
                .is_none()
        );
    }
    Value::Object(map)
}

// ---------------------------------------------------------------------------------------
// Vectors file
// ---------------------------------------------------------------------------------------

fn keys_json(w: &VectorWorld) -> Value {
    let keys = [
        ("scheme_root", ROOT_SEED),
        ("receiver_issuer", RECEIVER_ISSUER_SEED),
        ("payer_issuer", PAYER_ISSUER_SEED),
        ("regulator", REGULATOR_SEED),
        ("load_authorizer", LOAD_SEED),
        ("time_anchor", TIME_SEED),
        ("artifact", ARTIFACT_SEED),
        ("renewal_attested_key", RENEWAL_KEY_SEED),
        ("payer_payment", PAYER_SEED),
        ("receiver_payment", RECEIVER_SEED),
    ];
    for (signer, certificate) in [
        (&w.load_signer, &w.load_certificate),
        (&w.time_signer, &w.time_certificate),
        (&w.artifact_signer, &w.artifact_certificate),
    ] {
        assert_eq!(public_key(signer), certificate.body.key);
    }
    Value::Array(
        keys.iter()
            .map(|(name, seed)| {
                json_object(vec![
                    ("name", json_text(name)),
                    ("scalar_hex", json_hex(&[*seed; 32])),
                    (
                        "public_key_hex",
                        json_hex(public_key(&signing_key(*seed)).as_sec1_bytes()),
                    ),
                ])
            })
            .collect(),
    )
}

fn stand_ins_json() -> Value {
    json_object(vec![
        (
            "note",
            json_text(
                "labelled stand-in values: proof bytes, relation bindings and empty-map roots \
                 are fixed by the G3 artifact set and map owner; vectors that depend on them \
                 change then",
            ),
        ),
        ("eq_protocol_digest_hex", json_hex(&EQ)),
        ("ep_protocol_digest_hex", json_hex(&EP)),
        ("native_profile_digest_hex", json_hex(&NATIVE)),
        ("verifying_key_set_digest_hex", json_hex(&VK_SET)),
        ("artifact_inventory_digest_hex", json_hex(&INVENTORY)),
        (
            "empty_quota_usage_root_hex",
            json_hex(&EMPTY_ROOTS.quota_usage),
        ),
        ("proof_byte_rule", json_text("byte i = i mod 251")),
    ])
}

fn header_json(w: &VectorWorld) -> Value {
    let frame = KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Offer {
        offer: w.offer.clone(),
    })
    .to_canonical_bytes()
    .expect("frame");
    let header = norito::core::Header::read(frame.as_slice()).expect("header");
    json_object(vec![
        ("magic_hex", json_hex(&header.magic)),
        ("major", json_number(usize::from(header.major))),
        ("minor", json_number(usize::from(header.minor))),
        ("compression", json_number(0)),
        ("header_bytes", json_number(norito::core::Header::SIZE)),
        (
            "schema_hash_rule",
            json_text("first 16 bytes of SHA-256(\"norito:v1:type-name\" || 0x00 || frame_name)"),
        ),
        (
            "crc64_rule",
            json_text("CRC-64/XZ of the payload, stored little-endian; crc64_hex is big-endian"),
        ),
    ])
}

fn bounds_json() -> Value {
    json_object(vec![
        (
            "version",
            json_number(usize::from(KAGEMUSHA_WALLET_VERSION_V1)),
        ),
        ("text_prefix", json_text(KAGEMUSHA_WALLET_TEXT_PREFIX_V1)),
        (
            "session_max_bytes",
            json_number(KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1),
        ),
        (
            "message_max_bytes",
            json_number(KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1),
        ),
        (
            "session_text_max_bytes",
            json_number(KAGEMUSHA_WALLET_SESSION_TEXT_MAX_BYTES_V1),
        ),
        (
            "message_text_max_bytes",
            json_number(KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1),
        ),
        (
            "proof_max_bytes",
            json_number(KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1),
        ),
        (
            "credit_status_proof_max_bytes",
            json_number(KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1),
        ),
        (
            "certificate_set_max",
            json_number(KAGEMUSHA_WALLET_CERTIFICATE_SET_MAX_V1),
        ),
    ])
}

/// Complete text of the vectors file.
fn vectors_file_text(w: &VectorWorld) -> String {
    let digests = digest_vectors(w);
    let document = json_object(vec![
        ("fixture_version", json_number(1)),
        (
            "domain_prefix",
            json_text(
                core::str::from_utf8(KAGEMUSHA_WALLET_DIGEST_PREFIX_V1).expect("ascii prefix"),
            ),
        ),
        (
            "domain_prefix_hex",
            json_hex(KAGEMUSHA_WALLET_DIGEST_PREFIX_V1),
        ),
        (
            "digest_rule",
            json_text("H(role, body) = SHA-256(prefix || role || 0x00 || LE64(len(body)) || body)"),
        ),
        (
            "signature_rule",
            json_text(
                "ECDSA-P256-SHA256 over preimage_hex; RFC 6979 from the fixed scalars, \
                 frozen to low S; a consumer accepts iff codec_ok and verify_ok",
            ),
        ),
        ("stand_ins", stand_ins_json()),
        ("bounds", bounds_json()),
        ("norito_header", header_json(w)),
        ("keys", keys_json(w)),
        ("digests", digest_vectors_json(&digests)),
        ("signatures", signature_vectors_json(&signature_vectors(w))),
        ("signature_boundaries", boundary_json()),
        ("envelopes", envelope_vectors_json(w)),
        ("frames", frame_pins_json()),
        ("objects", object_pins_json(w)),
        ("enum_tags", enum_tags_json(&enum_tag_table(w))),
    ]);
    let mut text = norito::json::to_string_pretty(&document).expect("vectors json");
    text.push('\n');
    text
}

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VECTORS_PATH)
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

#[test]
fn kagemusha_wallet_v1_vectors_file_matches_the_generated_vectors() {
    let text = vectors_file_text(vector_world());
    let path = vectors_path();
    if std::env::var_os(UPDATE_VARIABLE).is_some_and(|value| value == "1") {
        std::fs::create_dir_all(path.parent().expect("fixture directory"))
            .expect("create fixture directory");
        std::fs::write(&path, &text).expect("write vectors");
        return;
    }
    let Ok(existing) = std::fs::read_to_string(&path) else {
        panic!(
            "missing {}; regenerate it with `{UPDATE_VARIABLE}=1 cargo test -p iroha_data_model \
             --lib kagemusha_wallet_v1_vectors`",
            path.display()
        );
    };
    if existing != text {
        let line = existing
            .lines()
            .zip(text.lines())
            .position(|(old, new)| old != new)
            .unwrap_or_else(|| existing.lines().count().min(text.lines().count()));
        panic!(
            "{} differs from the generated vectors at line {}; regenerate it with \
             `{UPDATE_VARIABLE}=1 cargo test -p iroha_data_model \
             --lib kagemusha_wallet_v1_vectors`",
            path.display(),
            line + 1
        );
    }
}

#[test]
fn kagemusha_wallet_v1_vectors_are_deterministic_and_parse() {
    let world = vector_world();
    let text = vectors_file_text(world);
    assert_eq!(text, vectors_file_text(world));
    let parsed = norito::json::parse_value(&text).expect("valid json");
    let Value::Object(document) = parsed else {
        panic!("object");
    };
    for key in [
        "digests",
        "signatures",
        "signature_boundaries",
        "envelopes",
        "frames",
        "objects",
        "enum_tags",
        "keys",
    ] {
        assert!(document.contains_key(key), "{key}");
    }
    let Some(Value::Array(envelopes)) = document.get("envelopes") else {
        panic!("envelopes");
    };
    assert_eq!(envelopes.len(), envelope_messages(world).len());
}

#[test]
fn kagemusha_wallet_v1_vectors_cover_every_role_once_in_order() {
    let vectors = digest_vectors(vector_world());
    let roles: Vec<Role> = vectors.iter().map(|vector| vector.role).collect();
    assert_eq!(roles, Role::ALL.to_vec());
    let json = digest_vectors_json(&vectors);
    let Value::Array(rows) = json else {
        panic!("array");
    };
    assert_eq!(rows.len(), Role::ALL.len());
    // Only the vectors whose bytes carry stand-in proof bytes are labelled.
    let labelled: Vec<&str> = vectors
        .iter()
        .filter(|vector| vector.stand_in_proof)
        .map(|vector| vector.role.as_str())
        .collect();
    assert_eq!(labelled, ["proof", "capsule", "completion"]);
}

#[test]
fn kagemusha_wallet_v1_signature_vectors_and_boundaries() {
    let world = vector_world();
    let vectors = signature_vectors(world);
    assert_eq!(vectors.len(), 17);
    let Value::Array(rows) = signature_vectors_json(&vectors) else {
        panic!("array");
    };
    assert_eq!(rows.len(), vectors.len());

    let boundary = boundary_vector();
    let cases = boundary_cases(&boundary);
    let verdicts: Vec<(&str, bool, bool)> = cases
        .iter()
        .map(|(name, raw, _, _)| {
            let (codec_ok, verify_ok) = signature_verdicts(&boundary.key, &boundary.preimage, raw);
            (*name, codec_ok, verify_ok)
        })
        .collect();
    assert_eq!(
        verdicts,
        [
            ("s_half_order", true, true),
            ("s_half_order_plus_one_high_s_twin", false, true),
            ("r_zero", false, false),
            ("s_zero", false, false),
            ("r_order", false, false),
            ("s_order", false, false),
        ]
    );
    assert_eq!(
        hex::encode(&cases[1].1[32..]),
        "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a9"
    );
    let signature =
        KagemushaDeviceSignatureV1::from_raw_bytes(&cases[0].1).expect("s = floor(n/2)");
    kagemusha_wallet_verify_signature_v1(&boundary.key, BOUNDARY_ROLE, BOUNDARY_BODY, &signature)
        .expect("boundary signature verifies");
    assert!(matches!(boundary_json(), Value::Object(_)));
    assert_eq!(
        high_s_twin(&signature).as_slice(),
        cases[1].1.as_slice(),
        "the twin of floor(n/2) is floor(n/2) + 1"
    );
    assert_eq!(&raw_pair(&[1; 32], &[2; 32])[31..33], &[1, 2]);
}

#[test]
fn kagemusha_wallet_v1_envelope_vectors_and_frame_pins() {
    let world = vector_world();
    let Value::Array(rows) = envelope_vectors_json(world) else {
        panic!("array");
    };
    let kinds: Vec<&str> = envelope_messages(world)
        .iter()
        .map(|(_, message, _)| message_kind_name(message))
        .collect();
    for kind in [
        "Offer",
        "Request",
        "Payment",
        "Credited",
        "SessionControl",
        "PolicyData",
    ] {
        assert!(kinds.contains(&kind), "{kind}");
    }
    assert_eq!(rows.len(), kinds.len());
    let pins = frame_pins();
    let mut names: Vec<&str> = pins.iter().map(|pin| pin.short_name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), pins.len());
    assert!(matches!(frame_pins_json(), Value::Array(rows) if rows.len() == pins.len()));
    assert_eq!(
        hex::encode(norito::core::schema_hash_for_name(&format!(
            "{FRAME_NAME_PREFIX}KagemushaWalletEnvelopeV1"
        ))),
        hex::encode(norito::schema::identity::frame_hash::<
            KagemushaWalletEnvelopeV1,
        >())
    );
}

#[test]
fn kagemusha_wallet_v1_object_pins_cover_every_frame_type() {
    let world = vector_world();
    let pins = object_pins(world);
    for frame in frame_pins() {
        if frame.short_name == "KagemushaWalletEnvelopeV1" {
            // Pinned by the envelope vectors, one per message kind and variant.
            continue;
        }
        assert!(
            pins.iter().any(|pin| pin.short_name == frame.short_name),
            "{} has no canonical pin",
            frame.short_name
        );
        for pin in pins.iter().filter(|pin| pin.short_name == frame.short_name) {
            assert_eq!(pin.frame_name, frame.frame_name);
            assert!(pin.frame.len() <= frame.max_bytes, "{}", pin.short_name);
        }
    }
    let markers: Vec<&str> = pins
        .iter()
        .filter(|pin| pin.short_name == "KagemushaWalletMarkerV1")
        .map(|pin| pin.variant)
        .collect();
    assert_eq!(
        markers,
        [
            "Enrollment",
            "Head",
            "Terminal::CustodyDeleted",
            "Terminal::Abandoned"
        ]
    );
    let mut keys: Vec<(&str, &str)> = pins
        .iter()
        .map(|pin| (pin.short_name, pin.variant))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), pins.len());
    assert!(matches!(object_pins_json(world), Value::Array(rows) if rows.len() == pins.len()));
    // Every pinned frame declares compact lengths and carries its payload checksum.
    for pin in &pins {
        let header = norito::core::Header::read(pin.frame.as_slice()).expect("header");
        assert_eq!(header.flags, norito::core::header_flags::COMPACT_LEN);
        let payload = payload_range(&pin.frame);
        assert_eq!(header.checksum, norito::crc64_fallback(&pin.frame[payload]));
    }
}

#[track_caller]
fn assert_version_error<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { field, version: 2 })
            if field == expected => {}
        other => panic!("expected unsupported `{expected}` version 2, got {other:?}"),
    }
}

#[test]
fn kagemusha_wallet_v1_every_version_field_precedes_the_scheme() {
    let w = vector_world();
    let l = ledger_objects(w);
    let foreign = [0x78; 32];

    let mut unload = l.unload_claim.clone();
    unload.package.statement.version = 2;
    unload.credential.body.scheme_id = foreign;
    assert_version_error(unload.require_versions(), "statement.version");
    assert_version_error(
        KagemushaWalletUnloadClaimV1::decode_canonical(
            &norito::encode_canonical(&unload).expect("encode"),
            &w.scheme_id(),
        ),
        "statement.version",
    );
    let mut fee = l.fee_claim.clone();
    fee.payment.payer_credential.body.version = 2;
    fee.payment.request.body.scheme_id = foreign;
    assert_version_error(
        KagemushaWalletFeeClaimV1::decode_canonical(
            &norito::encode_canonical(&fee).expect("encode"),
            &w.scheme_id(),
        ),
        "credential.version",
    );
    let mut activation = l.activation.clone();
    activation.asset.version = 2;
    assert_version_error(
        KagemushaWalletActivationV1::decode_canonical(
            &norito::encode_canonical(&activation).expect("encode"),
            &foreign,
        ),
        "asset_scope.version",
    );
    let mut close = l.close_loads.clone();
    close.control.body.version = 2;
    assert_version_error(
        KagemushaWalletCloseLoadsV1::decode_canonical(
            &norito::encode_canonical(&close).expect("encode"),
            &foreign,
        ),
        "ledger_control.version",
    );
    let mut abandonment = l.abandonment;
    abandonment.control.body.version = 2;
    assert_version_error(abandonment.require_versions(), "ledger_control.version");
    let mut capsule = w.capsule.clone();
    capsule.successor_state.version = 2;
    assert_version_error(
        KagemushaWalletRecoveryCapsuleV1::decode_canonical(
            &norito::encode_canonical(&capsule).expect("encode"),
            &foreign,
        ),
        "state.version",
    );
    let mut completion = w.completion.clone();
    completion.receipt.version = 2;
    assert_version_error(completion.require_versions(), "receipt.version");
    let mut status = w.credited_status.clone();
    if let KagemushaWalletCreditedEvidenceV1::Status { status, .. } = &mut status.evidence {
        status.statement.version = 2;
    }
    assert_version_error(status.require_versions(), "credit_status.version");
    let mut request = w.payment.request.clone();
    if let KagemushaWalletFeeScheduleSlotV1::Present { schedule } = &mut request.fee_schedule {
        schedule.body.version = 2;
    }
    request.body.scheme_id = foreign;
    assert_version_error(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&KagemushaWalletEnvelopeV1::new(
                KagemushaWalletMessageV1::Request { request },
            ))
            .expect("encode"),
            &w.scheme_id(),
        ),
        "fee_schedule.version",
    );
    let mut offer = w.offer.clone();
    offer.certificates.certificates[0].body.version = 2;
    assert_version_error(offer.require_versions(), "certificate.version");
    let mut list = w.blacklist.clone();
    list.body.version = 2;
    assert_version_error(list.require_versions(), "blacklist.version");
    let mut policy = w.policy_data(KagemushaWalletPolicyDataItemV1::SchemePolicy {
        policy: w.scheme_policy,
    });
    if let KagemushaWalletPolicyDataItemV1::SchemePolicy { policy } = &mut policy.item {
        policy.body.version = 2;
    }
    assert_version_error(policy.require_versions(), "scheme_policy.version");
    let mut quote = l.unload_claim.charge.clone();
    if let KagemushaWalletUnloadChargeV1::Quoted { quote, .. } = &mut quote {
        quote.body.version = 2;
    }
    assert_version_error(quote.require_versions(), "charge_quote.version");
    assert!(
        KagemushaWalletUnloadChargeV1::None
            .require_versions()
            .is_ok()
    );
    assert!(
        KagemushaWalletFeeScheduleSlotV1::None
            .require_versions()
            .is_ok()
    );
    // Every vectored object passes.
    for envelope in envelope_messages(w) {
        KagemushaWalletEnvelopeV1::new(envelope.1)
            .require_versions()
            .expect("envelope versions");
    }
    l.unload_claim.require_versions().expect("unload claim");
    l.fee_claim.require_versions().expect("fee claim");
    l.activation.require_versions().expect("activation");
    l.close_loads.require_versions().expect("close loads");
    l.abandonment.require_versions().expect("abandonment");
}

#[test]
fn kagemusha_wallet_v1_every_enum_norito_tag_equals_its_transcript_tag() {
    let table = enum_tag_table(vector_world());
    assert_eq!(table.len(), 21);
    for entry in &table {
        let mut previous = None;
        for (variant, tag, wire) in &entry.variants {
            assert_eq!(u32::from(*tag), *wire, "{}::{variant}", entry.name);
            assert!(
                previous.is_none_or(|previous| previous < *tag),
                "{}::{variant} out of order",
                entry.name
            );
            previous = Some(*tag);
        }
    }
    let Value::Object(map) = enum_tags_json(&table) else {
        panic!("object");
    };
    assert_eq!(map.len(), table.len());
}

#[test]
fn kagemusha_wallet_v1_vector_world_objects_validate() {
    let w = vector_world();
    let scheme = w.scheme();
    w.payment.verify(&scheme).expect("payment");
    w.credited_receive
        .verify(&scheme)
        .expect("credited receive");
    w.credited_status.verify(&scheme).expect("credited status");
    w.offer.verify(&scheme).expect("offer");
    w.close.verify(Some(w.payer())).expect("close");
    w.unsupported.verify(None).expect("unsupported");
    w.scheme_policy
        .verify(&scheme, &w.f.regulator_certificate)
        .expect("scheme policy");
    w.blacklist
        .verify(&scheme, &w.f.regulator_certificate)
        .expect("blacklist");
    w.quota_share
        .verify(&scheme, &w.f.regulator_certificate)
        .expect("quota share");
    w.time_anchor
        .verify(&scheme, &w.time_certificate)
        .expect("time anchor");
    w.charge_quote
        .verify(&scheme, &w.f.regulator_certificate)
        .expect("charge quote");
    w.voucher
        .verify(&scheme, &w.load_certificate)
        .expect("voucher");
    w.renewal.verify(w.payer()).expect("renewal");
    w.control
        .verify(&w.payer().body.payment_key)
        .expect("ledger control");
    w.bootstrap.verify(w.payer()).expect("bootstrap");
    w.marker
        .bootstrap_effect()
        .expect("marker binds the bootstrap");
    assert_eq!(
        w.bootstrap.statement.effect,
        w.marker.bootstrap_effect().expect("effect")
    );
    w.completion
        .verify(w.receiver(), &w.capsule)
        .expect("completion");
    let pool = std::ptr::from_ref(vector_world());
    assert_eq!(pool, std::ptr::from_ref(w), "built once");
}

// ---------------------------------------------------------------------------------------
// Every-byte flips (design C11)
// ---------------------------------------------------------------------------------------

/// Flip every byte of `value`'s canonical frame: each flip must be rejected by `accept` or
/// change the digest it returns.
fn assert_flips<T: norito::NoritoSerialize>(
    value: &T,
    digest: [u8; 32],
    accept: impl Fn(&[u8]) -> Option<[u8; 32]>,
) {
    let frame = norito::encode_canonical(value).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, digest, accept);
}

#[test]
fn kagemusha_wallet_v1_flips_credential() {
    let w = vector_world();
    let (scheme, payer) = (w.scheme(), w.payer());
    assert_flips(payer, payer.credential_digest(), |bytes| {
        let credential =
            KagemushaWalletCredentialV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        credential
            .verify(&scheme, &w.f.payer.enrollment_certificate)
            .ok()?;
        Some(credential.credential_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_certificate() {
    let w = vector_world();
    let scheme = w.scheme();
    let certificate = &w.f.payer.enrollment_certificate;
    assert_flips(certificate, certificate.certificate_digest(), |bytes| {
        KagemushaWalletSignerCertificateV1::decode_canonical(bytes, &scheme)
            .ok()
            .map(|certificate| certificate.certificate_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_offer() {
    let w = vector_world();
    let scheme = w.scheme();
    assert_flips(&w.offer, w.offer.body.body_digest(), |bytes| {
        let offer: KagemushaWalletOfferV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1).ok()?;
        offer.verify(&scheme).ok()?;
        Some(offer.body.body_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_request() {
    let w = vector_world();
    let scheme = w.scheme();
    let request = &w.payment.request;
    assert_flips(request, request.request_digest(), |bytes| {
        let request: KagemushaWalletRequestV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        request.verify(&scheme).ok()?;
        Some(request.request_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_payment() {
    let w = vector_world();
    let scheme_id = w.scheme_id();
    let digest = w.payment.payment_digest().expect("payment digest");
    assert_flips(&w.payment, digest, |bytes| {
        KagemushaWalletPaymentV1::decode_canonical(bytes, &scheme_id)
            .ok()?
            .payment_digest()
            .ok()
    });
}

#[test]
fn kagemusha_wallet_v1_flips_credited_receive() {
    let w = vector_world();
    let digest = w.credited_receive.credited_digest().expect("credited");
    assert_flips(&w.credited_receive, digest, |bytes| {
        let credited: KagemushaWalletCreditedV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        credited.credited_digest().ok()
    });
}

#[test]
fn kagemusha_wallet_v1_flips_credited_status() {
    let w = vector_world();
    let digest = w.credited_status.credited_digest().expect("credited");
    assert_flips(&w.credited_status, digest, |bytes| {
        let credited: KagemushaWalletCreditedV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        credited.credited_digest().ok()
    });
}

#[test]
fn kagemusha_wallet_v1_flips_package() {
    let w = vector_world();
    let receiver = w.receiver();
    let digest = w.receive.package_digest(receiver).expect("package");
    assert_flips(&w.receive, digest, |bytes| {
        let package: KagemushaWalletPackageV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        package.package_digest(receiver).ok()
    });
}

#[test]
fn kagemusha_wallet_v1_flips_load_voucher() {
    let w = vector_world();
    let scheme = w.scheme();
    assert_flips(&w.voucher, w.voucher.voucher_digest(), |bytes| {
        let voucher =
            KagemushaWalletLoadVoucherV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        voucher.verify(&scheme, &w.load_certificate).ok()?;
        Some(voucher.voucher_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_scheme_policy() {
    let w = vector_world();
    let scheme = w.scheme();
    let policy = &w.scheme_policy;
    assert_flips(policy, policy.scheme_policy_digest(), |bytes| {
        let policy =
            KagemushaWalletSchemePolicyV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        policy.verify(&scheme, &w.f.regulator_certificate).ok()?;
        Some(policy.scheme_policy_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_fee_schedule() {
    let w = vector_world();
    let scheme = w.scheme();
    let schedule = &w.fee_schedule;
    assert_flips(schedule, schedule.fee_schedule_digest(), |bytes| {
        let schedule =
            KagemushaWalletFeeScheduleV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        schedule.verify(&scheme, &w.f.regulator_certificate).ok()?;
        Some(schedule.fee_schedule_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_blacklist_body_and_list() {
    let w = vector_world();
    let scheme = w.scheme();
    let list = &w.blacklist;
    assert_flips(&list.body, list.body.body_digest(), |bytes| {
        let body: KagemushaWalletBlacklistBodyV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1).ok()?;
        body.validate().ok()?;
        Some(body.body_digest())
    });
    assert_flips(list, list.blacklist_digest(), |bytes| {
        let list = KagemushaWalletBlacklistV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        list.verify(&scheme, &w.f.regulator_certificate).ok()?;
        Some(list.blacklist_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_quota_share() {
    let w = vector_world();
    let scheme = w.scheme();
    let share = &w.quota_share;
    assert_flips(share, share.quota_share_digest(), |bytes| {
        let share =
            KagemushaWalletQuotaShareV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        share.verify(&scheme, &w.f.regulator_certificate).ok()?;
        Some(share.quota_share_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_time_anchor() {
    let w = vector_world();
    let scheme = w.scheme();
    let anchor = &w.time_anchor;
    assert_flips(anchor, anchor.time_anchor_digest(), |bytes| {
        let anchor =
            KagemushaWalletTimeAnchorV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        anchor.verify(&scheme, &w.time_certificate).ok()?;
        Some(anchor.time_anchor_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_charge_quote() {
    let w = vector_world();
    let scheme = w.scheme();
    let quote = &w.charge_quote;
    assert_flips(quote, quote.charge_quote_digest(), |bytes| {
        let quote =
            KagemushaWalletChargeQuoteV1::decode_canonical(bytes, &scheme.scheme_id()).ok()?;
        quote.verify(&scheme, &w.f.regulator_certificate).ok()?;
        Some(quote.charge_quote_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_ledger_control() {
    let w = vector_world();
    let scheme_id = w.scheme_id();
    let key = w.payer().body.payment_key;
    assert_flips(&w.control, w.control.body.body_digest(), |bytes| {
        let control = KagemushaWalletLedgerControlV1::decode_canonical(bytes, &scheme_id).ok()?;
        control.verify(&key).ok()?;
        Some(control.body.body_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_flips_artifact_manifest() {
    let w = vector_world();
    let scheme = w.scheme();
    let manifest = &w.manifest;
    assert_flips(manifest, manifest.manifest_digest(), |bytes| {
        let manifest = KagemushaWalletArtifactManifestV1::decode_canonical(bytes, &scheme).ok()?;
        manifest.verify(&scheme, &w.artifact_certificate).ok()?;
        Some(manifest.manifest_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_vector_json_helpers() {
    assert_eq!(json_hex(&[0xab, 0x01]), Value::String("ab01".to_owned()));
    assert_eq!(json_text("x"), Value::String("x".to_owned()));
    assert_eq!(json_number(7), Value::Number(Number::U64(7)));
    let Value::Object(map) = json_object(vec![("b", json_number(1)), ("a", json_number(2))]) else {
        panic!("object");
    };
    assert_eq!(map.keys().collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(bytes32(&"01".repeat(32)), [1; 32]);
    let signature = vector_world().offer.signature;
    let body = signed_object_body(&[9; 32], &signature);
    assert_eq!(&body[..32], &[9; 32]);
    assert_eq!(&body[32..], signature.as_raw_bytes());
    assert!(vectors_path().ends_with("fixtures/kagemusha/wallet_v1_vectors.json"));
}
