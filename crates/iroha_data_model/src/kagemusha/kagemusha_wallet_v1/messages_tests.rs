//! Peer message, envelope and `kgm1:` text tests.
//!
//! The fixtures are visible to the custody and ledger test modules, which build capsules,
//! completion records and fee claims from the same Payment.

use p256::ecdsa::SigningKey;

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1, KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_SESSION_TEXT_MAX_BYTES_V1, KagemushaWalletEvidenceKindV1,
    KagemushaWalletFeeRoundingV1, KagemushaWalletFeeScheduleBodyV1, KagemushaWalletLifecycleV1,
    KagemushaWalletPolicyUpdateV1, KagemushaWalletSchemePolicyBodyV1, KagemushaWalletStatementV1,
    codec_tests::{assert_every_flip_rejected_or_rebound, norito_tag, payload_range},
    identity::identity_tests::{
        IdentityFixture, identity_fixture, raw_output, signing_key, test_account, test_certificate,
    },
    kagemusha_wallet_account_digest_v1, kagemusha_wallet_enrollment_id_v1, kagemusha_wallet_id_v1,
    state::state_tests::{
        EMPTY_ROOTS, send_effect, signed_package, stand_in_proof, transition_statement,
    },
};

/// One mutation of an Offer body.
type OfferBodyMutation = fn(&mut KagemushaWalletOfferBodyV1);
/// One mutation of a Request body.
type RequestBodyMutation = fn(&mut KagemushaWalletRequestBodyV1);
/// One mutation of a Send effect.
type EffectMutation = fn(&mut KagemushaWalletEffectV1);
/// One mutation of a `CreditStatus` statement.
type ClaimMutation = fn(&mut KagemushaWalletCreditStatusStatementV1);
/// One mutation of a session control.
type ControlMutation = fn(&mut KagemushaWalletSessionControlV1);

/// Receiver's authenticated accepted time of the fixture Request.
pub(in crate::kagemusha::kagemusha_wallet_v1) const ACCEPTED_MS: u64 = 1_790_000_100_000;
const SESSION_NONCE: [u8; 32] = [0x5e; 32];

/// Payer and receiver wallets of one scheme with distinct issuers, and a fee signer.
pub(in crate::kagemusha::kagemusha_wallet_v1) struct MessageFixture {
    /// Payer; its enrollment signer and certificate are the payer's own issuer.
    pub(in crate::kagemusha::kagemusha_wallet_v1) payer: IdentityFixture,
    /// Receiver under the shared fixture issuer.
    pub(in crate::kagemusha::kagemusha_wallet_v1) receiver: IdentityFixture,
    /// `RegulatoryPolicy`-role signer.
    pub(in crate::kagemusha::kagemusha_wallet_v1) regulator: SigningKey,
    /// `RegulatoryPolicy`-role certificate.
    pub(in crate::kagemusha::kagemusha_wallet_v1) regulator_certificate:
        KagemushaWalletSignerCertificateV1,
}

/// Fixture whose payer issuer differs from the receiver issuer, so a fee-bearing Payment needs
/// three certificates.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn message_fixture() -> MessageFixture {
    let base = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x51);
    let payer_signer = signing_key(0x26);
    let payer_issuer = test_certificate(
        &base.scheme,
        &base.root,
        KagemushaWalletSignerRoleV1::Enrollment,
        &payer_signer,
        9,
    );
    let mut body = base.credential.body;
    body.issuer_certificate = payer_issuer.certificate_digest();
    let credential = KagemushaWalletCredentialV1::sign(
        body,
        &payer_issuer,
        raw_output(&payer_signer, &body.signing_message()),
    )
    .expect("payer credential");
    let payer = IdentityFixture {
        enrollment_signer: payer_signer,
        enrollment_certificate: payer_issuer,
        credential,
        ..base
    };
    let receiver = identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x52);
    let regulator = signing_key(0x33);
    let regulator_certificate = test_certificate(
        &payer.scheme,
        &payer.root,
        KagemushaWalletSignerRoleV1::RegulatoryPolicy,
        &regulator,
        2,
    );
    MessageFixture {
        payer,
        receiver,
        regulator,
        regulator_certificate,
    }
}

impl MessageFixture {
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn scheme_id(&self) -> [u8; 32] {
        self.payer.scheme.scheme_id()
    }

    pub(in crate::kagemusha::kagemusha_wallet_v1) fn asset_digest(&self) -> [u8; 32] {
        self.payer.credential.body.asset_digest
    }

    /// Fee schedule body: 25 bp rounded up, fixed 3, clamped to `[5, 1000]`.
    fn fee_body(&self) -> KagemushaWalletFeeScheduleBodyV1 {
        KagemushaWalletFeeScheduleBodyV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            schedule_id: 7,
            beneficiary_account_digest: kagemusha_wallet_account_digest_v1(&test_account(0x5b))
                .expect("beneficiary"),
            basis_points: 25,
            fixed: 3,
            minimum: 5,
            maximum: 1_000,
            rounding: KagemushaWalletFeeRoundingV1::Up,
            signer_certificate: self.regulator_certificate.certificate_digest(),
        }
    }

    fn sign_fee_schedule(
        &self,
        body: KagemushaWalletFeeScheduleBodyV1,
    ) -> KagemushaWalletFeeScheduleV1 {
        KagemushaWalletFeeScheduleV1::sign(
            body,
            &self.regulator_certificate,
            raw_output(&self.regulator, &body.signing_message()),
        )
        .expect("fee schedule")
    }

    /// Signed fixture fee schedule.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn fee_schedule(
        &self,
    ) -> KagemushaWalletFeeScheduleV1 {
        self.sign_fee_schedule(self.fee_body())
    }

    /// Epoch-`epoch` scheme policy naming `fee_schedule`.
    fn scheme_policy(
        &self,
        epoch: u64,
        enabled_controls: u32,
        fee_schedule: [u8; 32],
    ) -> KagemushaWalletSchemePolicyV1 {
        let body = KagemushaWalletSchemePolicyBodyV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            policy_epoch: epoch,
            enabled_controls,
            fee_schedule,
            signer_certificate: self.regulator_certificate.certificate_digest(),
        };
        KagemushaWalletSchemePolicyV1::sign(
            body,
            &self.regulator_certificate,
            raw_output(&self.regulator, &body.signing_message()),
        )
        .expect("scheme policy")
    }

    /// Unsigned Request parts for `amount`, with or without the fixture fee schedule.
    fn request_parts(
        &self,
        fee: bool,
        amount: u128,
    ) -> (
        KagemushaWalletRequestBodyV1,
        KagemushaWalletFeeScheduleSlotV1,
        KagemushaWalletCertificateSetV1,
    ) {
        let slot = if fee {
            KagemushaWalletFeeScheduleSlotV1::Present {
                schedule: self.fee_schedule(),
            }
        } else {
            KagemushaWalletFeeScheduleSlotV1::None
        };
        let mut certificates = vec![self.receiver.enrollment_certificate];
        if fee {
            certificates.push(self.regulator_certificate);
        }
        let certificates = KagemushaWalletCertificateSetV1::new(certificates).expect("set");
        let policy = self.scheme_policy(1, 0, slot.fee_schedule_digest());
        let body = KagemushaWalletRequestBodyV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            payer_wallet_id: self.payer.credential.body.wallet_id,
            receiver_wallet_id: self.receiver.credential.body.wallet_id,
            send_ordinal: 4,
            receiver_credential_digest: self.receiver.credential.credential_digest(),
            amount,
            fee_schedule: slot.fee_schedule_digest(),
            fee: slot
                .schedule()
                .map_or(0, |schedule| schedule.fee(amount).expect("fee")),
            policy_epoch: 1,
            scheme_policy: policy.scheme_policy_digest(),
            receiver_accepted_time_ms: ACCEPTED_MS,
            certificates: certificates.digest().expect("set digest"),
            nonce: [0x6e; 32],
        };
        (body, slot, certificates)
    }

    fn sign_request(
        &self,
        body: &KagemushaWalletRequestBodyV1,
        slot: KagemushaWalletFeeScheduleSlotV1,
        certificates: KagemushaWalletCertificateSetV1,
    ) -> WalletResult<KagemushaWalletRequestV1> {
        KagemushaWalletRequestV1::sign(
            *body,
            self.receiver.credential,
            slot,
            certificates,
            raw_output(&self.receiver.payment, &body.signing_message()),
        )
    }

    /// Signed Request of 1,000 units.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn request(
        &self,
        fee: bool,
    ) -> KagemushaWalletRequestV1 {
        let (body, slot, certificates) = self.request_parts(fee, 1_000);
        self.sign_request(&body, slot, certificates)
            .expect("request")
    }

    /// Payer statement carrying `effect`.
    fn payer_statement(&self, effect: KagemushaWalletEffectV1) -> KagemushaWalletStatementV1 {
        transition_statement(
            &self.payer,
            3,
            0,
            KagemushaWalletLifecycleV1::Active,
            effect,
        )
    }

    /// Committed Send package of `request` with a `proof_len`-byte stand-in proof.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn send_package(
        &self,
        request: &KagemushaWalletRequestV1,
        proof_len: usize,
    ) -> KagemushaWalletPackageV1 {
        let interval =
            KagemushaWalletTimeIntervalV1::new(ACCEPTED_MS, ACCEPTED_MS + 4_000).expect("interval");
        let effect = request
            .send_effect(&self.payer.credential, &interval)
            .expect("send effect");
        signed_package(
            &self.payer,
            &self.payer.credential,
            &self.payer_statement(effect),
            stand_in_proof(proof_len),
        )
    }

    /// Complete Payment of 1,000 units.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn payment(
        &self,
        fee: bool,
        proof_len: usize,
    ) -> KagemushaWalletPaymentV1 {
        let request = self.request(fee);
        let send = self.send_package(&request, proof_len);
        KagemushaWalletPaymentV1::assemble(
            request,
            self.payer.credential,
            &self.payer.enrollment_certificate,
            send,
        )
        .expect("payment")
    }

    /// Receiver's Receive package of `payment`.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn receive_package(
        &self,
        payment: &KagemushaWalletPaymentV1,
        proof_len: usize,
    ) -> KagemushaWalletPackageV1 {
        let effect = payment
            .receive_effect(&self.receiver.credential)
            .expect("receive effect");
        let statement = transition_statement(
            &self.receiver,
            5,
            0,
            KagemushaWalletLifecycleV1::Active,
            effect,
        );
        signed_package(
            &self.receiver,
            &self.receiver.credential,
            &statement,
            stand_in_proof(proof_len),
        )
    }

    /// Credited evidence from the Receive package.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn credited_receive(
        &self,
        payment: &KagemushaWalletPaymentV1,
        proof_len: usize,
    ) -> KagemushaWalletCreditedV1 {
        KagemushaWalletCreditedV1::from_receive(
            self.receiver.credential,
            &self.receiver.enrollment_certificate,
            self.receive_package(payment, proof_len),
        )
        .expect("credited receive")
    }

    /// Receiver's later package carrying the largest effect (a Send to another wallet).
    fn current_package(&self, proof_len: usize) -> KagemushaWalletPackageV1 {
        let statement = transition_statement(
            &self.receiver,
            9,
            0,
            KagemushaWalletLifecycleV1::Active,
            send_effect(),
        );
        signed_package(
            &self.receiver,
            &self.receiver.credential,
            &statement,
            stand_in_proof(proof_len),
        )
    }

    /// `CreditStatus` of `payment` against `current`.
    fn status_for(
        &self,
        payment: &KagemushaWalletPaymentV1,
        current: &KagemushaWalletPackageV1,
        proof_len: usize,
    ) -> KagemushaWalletCreditStatusV1 {
        let digests = payment.digests().expect("payment digests");
        KagemushaWalletCreditStatusV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            statement: KagemushaWalletCreditStatusStatementV1::for_current(
                &self.receiver.credential,
                current,
                digests.credit_id,
                digests.payment,
            )
            .expect("status statement"),
            proof: stand_in_proof(proof_len),
        }
    }

    /// Credited evidence from a `CreditStatus` proof against a later receiver package.
    pub(in crate::kagemusha::kagemusha_wallet_v1) fn credited_status(
        &self,
        payment: &KagemushaWalletPaymentV1,
        current_proof_len: usize,
        status_proof_len: usize,
    ) -> KagemushaWalletCreditedV1 {
        let current = self.current_package(current_proof_len);
        let status = self.status_for(payment, &current, status_proof_len);
        KagemushaWalletCreditedV1::from_status(
            self.receiver.credential,
            &self.receiver.enrollment_certificate,
            current,
            status,
        )
        .expect("credited status")
    }

    /// Payer state holding the epoch-1 scheme policy of `request`'s fee schedule.
    fn payer_state(&self, request: &KagemushaWalletRequestV1) -> KagemushaWalletStateV1 {
        let mut state =
            KagemushaWalletStateV1::bootstrap(&self.payer.credential, &EMPTY_ROOTS, [0x5d; 32])
                .expect("bootstrap");
        let policy = self.scheme_policy(1, 0, request.body.fee_schedule);
        state.policy = state
            .refresh_policy(KagemushaWalletPolicyUpdateV1::SchemePolicy { policy: &policy })
            .expect("refresh")
            .policy;
        state.balance = 10_000;
        state.next_send = request.body.send_ordinal;
        state
    }

    fn offer_body(&self) -> KagemushaWalletOfferBodyV1 {
        KagemushaWalletOfferBodyV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            payer_wallet_id: self.payer.credential.body.wallet_id,
            payer_credential_digest: self.payer.credential.credential_digest(),
            next_send: 4,
            amount: 1_000,
            session_nonce: SESSION_NONCE,
        }
    }

    fn sign_offer(&self, body: KagemushaWalletOfferBodyV1) -> WalletResult<KagemushaWalletOfferV1> {
        KagemushaWalletOfferV1::sign(
            body,
            self.payer.credential,
            &self.payer.enrollment_certificate,
            raw_output(&self.payer.payment, &body.signing_message()),
        )
    }

    fn offer(&self) -> KagemushaWalletOfferV1 {
        self.sign_offer(self.offer_body()).expect("offer")
    }

    fn control(
        &self,
        kind: KagemushaWalletSessionControlKindV1,
    ) -> KagemushaWalletSessionControlV1 {
        let reason = match kind {
            KagemushaWalletSessionControlKindV1::SetupDeclined
            | KagemushaWalletSessionControlKindV1::ReceiveDeferred => 7,
            _ => 0,
        };
        let credit_id = if kind == KagemushaWalletSessionControlKindV1::ReceiveDeferred {
            [0x71; 32]
        } else {
            [0; 32]
        };
        KagemushaWalletSessionControlV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            sender_wallet_id: self.payer.credential.body.wallet_id,
            peer_wallet_id: self.receiver.credential.body.wallet_id,
            session_nonce: SESSION_NONCE,
            kind,
            reason,
            credit_id,
            auth: KagemushaWalletSessionAuthV1::Unsigned,
        }
    }

    fn signed_control(
        &self,
        kind: KagemushaWalletSessionControlKindV1,
    ) -> KagemushaWalletSessionControlV1 {
        let control = self.control(kind);
        control
            .sign(
                &self.payer.credential,
                raw_output(&self.payer.payment, &control.signing_message()),
            )
            .expect("signed control")
    }

    fn policy_data(&self, item: KagemushaWalletPolicyDataItemV1) -> KagemushaWalletPolicyDataV1 {
        KagemushaWalletPolicyDataV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: self.scheme_id(),
            asset_digest: self.asset_digest(),
            item,
        }
    }
}

/// Credential of a different wallet that reuses `f`'s payment key (another enrollment).
fn same_key_credential(f: &IdentityFixture) -> KagemushaWalletCredentialV1 {
    let mut body = f.credential.body;
    body.enrollment_id = kagemusha_wallet_enrollment_id_v1(&[0x3c; 32], &body.payment_key);
    body.wallet_id = kagemusha_wallet_id_v1(
        &body.scheme_id,
        &body.asset_digest,
        &body.payment_key,
        &body.enrollment_id,
    );
    f.issue(&body).expect("same-key credential")
}

/// Renewed credential of `f`: renewal sequence 1 with fresh evidence.
fn renewed_credential(f: &IdentityFixture) -> KagemushaWalletCredentialV1 {
    let mut body = f.credential.body;
    body.renewal_sequence = 1;
    body.issued_at_ms += 60_000;
    body.fresh_evidence.time_ms = body.issued_at_ms - 500;
    f.issue(&body).expect("renewed credential")
}

#[track_caller]
fn assert_invalid<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidField { field }) if field == expected => {}
        other => panic!("expected invalid `{expected}`, got {other:?}"),
    }
}

#[track_caller]
fn assert_mismatch<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch { field }) if field == expected => {}
        other => panic!("expected scheme mismatch `{expected}`, got {other:?}"),
    }
}

#[track_caller]
fn assert_signature<T: core::fmt::Debug>(result: WalletResult<T>, expected: Role) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidSignature { role }) if role == expected => {}
        other => panic!(
            "expected invalid `{}` signature, got {other:?}",
            expected.as_str()
        ),
    }
}

#[track_caller]
fn assert_too_large<T: core::fmt::Debug>(result: WalletResult<T>, expected_max: usize) {
    match result {
        Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { max, .. })
            if max == expected_max => {}
        other => panic!("expected frame bound {expected_max}, got {other:?}"),
    }
}

/// One message of every kind, in tag order.
fn sample_messages(f: &MessageFixture) -> [KagemushaWalletMessageV1; 6] {
    let payment = f.payment(true, 128);
    [
        KagemushaWalletMessageV1::Offer { offer: f.offer() },
        KagemushaWalletMessageV1::Request {
            request: f.request(true),
        },
        KagemushaWalletMessageV1::Credited {
            credited: f.credited_receive(&payment, 96),
        },
        KagemushaWalletMessageV1::Payment { payment },
        KagemushaWalletMessageV1::SessionControl {
            control: f.signed_control(KagemushaWalletSessionControlKindV1::Close),
        },
        KagemushaWalletMessageV1::PolicyData {
            data: f.policy_data(KagemushaWalletPolicyDataItemV1::SchemePolicy {
                policy: f.scheme_policy(2, 0, [0; 32]),
            }),
        },
    ]
}

#[test]
fn kagemusha_wallet_v1_message_transcript_lengths_are_pinned() {
    assert_eq!(KAGEMUSHA_WALLET_OFFER_BODY_TRANSCRIPT_BYTES_V1, 194);
    assert_eq!(KAGEMUSHA_WALLET_REQUEST_BODY_TRANSCRIPT_BYTES_V1, 354);
    assert_eq!(KAGEMUSHA_WALLET_SEND_DEPENDENCIES_TRANSCRIPT_BYTES_V1, 100);
    assert_eq!(KAGEMUSHA_WALLET_SEND_DEPENDENCIES_COUNT_V1, 3);
    assert_eq!(KAGEMUSHA_WALLET_PAYMENT_TRANSCRIPT_BYTES_V1, 130);
    assert_eq!(
        KAGEMUSHA_WALLET_CREDIT_STATUS_STATEMENT_TRANSCRIPT_BYTES_V1,
        370
    );
    assert_eq!(KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1, 227);
    assert_eq!(KAGEMUSHA_WALLET_SESSION_CONTROL_UNION_BYTES_V1, 34);
    assert_eq!(
        KAGEMUSHA_WALLET_SESSION_CONTROL_BODY_TRANSCRIPT_BYTES_V1,
        197
    );
    assert_eq!(max_width_v1(&[3, 9, 1]), 9);
    assert_eq!(max_width_v1(&[]), 0);
    for kind in KagemushaWalletSessionControlKindV1::ALL {
        assert!(kind.fields_bytes() <= KAGEMUSHA_WALLET_SESSION_CONTROL_UNION_BYTES_V1);
    }
    assert_eq!(
        KagemushaWalletSessionControlKindV1::ReceiveDeferred.fields_bytes(),
        KAGEMUSHA_WALLET_SESSION_CONTROL_UNION_BYTES_V1
    );

    let f = message_fixture();
    let offer = f.offer();
    assert_eq!(
        offer.body.transcript().len(),
        KAGEMUSHA_WALLET_OFFER_BODY_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(
        offer.body.body_digest(),
        kagemusha_wallet_digest_v1(Role::OfferBody, &offer.body.transcript())
    );
    assert_eq!(
        offer.body.signing_message(),
        kagemusha_wallet_preimage_v1(Role::OfferBody, &offer.body.transcript())
    );
    let request = f.request(true);
    let transcript = request.body.transcript();
    assert_eq!(
        transcript.len(),
        KAGEMUSHA_WALLET_REQUEST_BODY_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(&transcript[..2], &[1, 0]);
    assert_eq!(&transcript[322..354], &request.body.nonce);
    assert_eq!(
        request.credit_id(),
        kagemusha_wallet_digest_v1(Role::Credit, &transcript)
    );
    assert_ne!(request.credit_id(), request.body.body_digest());
    let mut signed = request.body.body_digest().to_vec();
    signed.extend_from_slice(request.signature.as_raw_bytes());
    assert_eq!(
        request.request_digest(),
        kagemusha_wallet_digest_v1(Role::Request, &signed)
    );

    let control = f.control(KagemushaWalletSessionControlKindV1::ReceiveDeferred);
    let transcript = control.transcript();
    assert_eq!(
        transcript.len(),
        KAGEMUSHA_WALLET_SESSION_CONTROL_BODY_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(transcript[162], 3);
    assert_eq!(&transcript[163..165], &7_u16.to_le_bytes());
    assert_eq!(&transcript[165..], &[0x71; 32]);

    let payment = f.payment(true, 64);
    let digests = payment.digests().expect("payment");
    let status = KagemushaWalletCreditStatusStatementV1::for_current(
        &f.receiver.credential,
        &f.current_package(32),
        digests.credit_id,
        digests.payment,
    )
    .expect("status");
    assert_eq!(
        status.transcript().len(),
        KAGEMUSHA_WALLET_CREDIT_STATUS_STATEMENT_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(
        status.statement_digest(),
        kagemusha_wallet_digest_v1(Role::CreditStatusStatement, &status.transcript())
    );
    assert_eq!(
        f.credited_receive(&payment, 32)
            .transcript()
            .expect("credited")
            .len(),
        KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1
    );
}

#[test]
fn kagemusha_wallet_v1_message_enum_tags_equal_norito_tags() {
    for kind in KagemushaWalletSessionControlKindV1::ALL {
        assert_eq!(norito_tag(&kind), u32::from(kind.tag()));
    }
    let f = message_fixture();
    let slots = [
        KagemushaWalletFeeScheduleSlotV1::None,
        KagemushaWalletFeeScheduleSlotV1::Present {
            schedule: f.fee_schedule(),
        },
    ];
    for (tag, slot) in slots.iter().enumerate() {
        assert_eq!(usize::from(slot.tag()), tag);
        assert_eq!(norito_tag(slot), u32::from(slot.tag()));
    }
    let auths = [
        KagemushaWalletSessionAuthV1::Unsigned,
        KagemushaWalletSessionAuthV1::Signed {
            signature: f.offer().signature,
        },
    ];
    for (tag, auth) in auths.iter().enumerate() {
        assert_eq!(usize::from(auth.tag()), tag);
        assert_eq!(norito_tag(auth), u32::from(auth.tag()));
    }
    let payment = f.payment(false, 32);
    let receive = f.credited_receive(&payment, 32).evidence;
    let status = f.credited_status(&payment, 32, 16).evidence;
    assert_eq!((receive.tag(), status.tag()), (1, 2));
    assert_eq!(norito_tag(&receive), 1);
    assert_eq!(norito_tag(&status), 2);
    let items = [
        KagemushaWalletPolicyDataItemV1::SchemePolicy {
            policy: f.scheme_policy(1, 0, [0; 32]),
        },
        KagemushaWalletPolicyDataItemV1::FeeSchedule {
            schedule: f.fee_schedule(),
        },
        KagemushaWalletPolicyDataItemV1::Certificates {
            certificates: KagemushaWalletCertificateSetV1::new(vec![f.regulator_certificate])
                .expect("set"),
        },
    ];
    for (index, item) in items.iter().enumerate() {
        assert_eq!(usize::from(item.tag()), index + 1);
        assert_eq!(norito_tag(item), u32::from(item.tag()));
    }
    for (index, message) in sample_messages(&f).iter().enumerate() {
        let expected = match index {
            2 => 4,
            3 => 3,
            other => other + 1,
        };
        assert_eq!(usize::from(message.tag()), expected);
        assert_eq!(norito_tag(message), u32::from(message.tag()));
    }
}

#[test]
fn kagemusha_wallet_v1_offer_binds_the_payer() {
    let f = message_fixture();
    let offer = f.offer();
    offer.validate().expect("offer");
    offer.verify(&f.payer.scheme).expect("verify");
    let other = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x5f);
    let mut foreign_root = other.scheme;
    foreign_root.scheme_root_key = other.credential.body.payment_key;
    assert_mismatch(offer.verify(&foreign_root), "certificate.scheme_id");

    let mutations: [(OfferBodyMutation, &str); 5] = [
        (|body| body.amount = 0, "offer.amount"),
        (
            |body| body.payer_wallet_id = [0x0c; 32],
            "offer.payer_wallet_id",
        ),
        (
            |body| body.payer_credential_digest = [0x0d; 32],
            "offer.payer_credential_digest",
        ),
        (|body| body.asset_digest = [0x0e; 32], "offer.asset_digest"),
        (|body| body.session_nonce = [0; 32], "offer.session_nonce"),
    ];
    for (mutate, field) in mutations {
        let mut body = f.offer_body();
        mutate(&mut body);
        assert_invalid(f.sign_offer(body), field);
    }
    let mut body = f.offer_body();
    body.scheme_id = [0x0f; 32];
    assert_mismatch(f.sign_offer(body), "offer.scheme_id");

    // The certificate set is exactly the payer issuer certificate.
    let mut extra = offer.clone();
    extra.certificates = KagemushaWalletCertificateSetV1::new(vec![
        f.payer.enrollment_certificate,
        f.regulator_certificate,
    ])
    .expect("set");
    assert_invalid(extra.validate(), "certificates.set");
    let mut other_issuer = offer.clone();
    other_issuer.certificates =
        KagemushaWalletCertificateSetV1::new(vec![f.receiver.enrollment_certificate]).expect("set");
    assert_invalid(other_issuer.validate(), "certificates.set");

    let mut tampered = offer.clone();
    tampered.body.next_send += 1;
    assert_signature(tampered.validate(), Role::OfferBody);
    assert_signature(
        KagemushaWalletOfferV1::sign(
            f.offer_body(),
            f.payer.credential,
            &f.payer.enrollment_certificate,
            raw_output(&f.receiver.payment, &f.offer_body().signing_message()),
        ),
        Role::OfferBody,
    );

    let frame = norito::encode_canonical(&offer).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, offer.body.body_digest(), |bytes| {
        let offer: KagemushaWalletOfferV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1).ok()?;
        offer.validate().ok()?;
        Some(offer.body.body_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_request_fee_and_certificate_rules() {
    let f = message_fixture();
    for fee in [false, true] {
        let request = f.request(fee);
        request.validate().expect("request");
        request.verify(&f.payer.scheme).expect("verify");
        assert_eq!(request.body.fee, if fee { 6 } else { 0 });
        assert_eq!(request.certificates.len(), if fee { 2 } else { 1 });
    }
    // 25 bp of 1,000 is 2.5, rounded up to 3, plus the fixed 3.
    assert_eq!(f.fee_schedule().fee(1_000).expect("fee"), 6);

    let body_mutations: [(RequestBodyMutation, &str); 10] = [
        (|body| body.amount = 0, "request.amount"),
        (
            |body| body.payer_wallet_id = body.receiver_wallet_id,
            "request.payer_wallet_id",
        ),
        (|body| body.fee = u128::MAX, "request.gross"),
        (|body| body.policy_epoch = 0, "request.scheme_policy"),
        (|body| body.nonce = [0; 32], "request.nonce"),
        (
            |body| body.receiver_wallet_id = [0x0c; 32],
            "request.receiver_wallet_id",
        ),
        (
            |body| body.receiver_credential_digest = [0x0d; 32],
            "request.receiver_credential_digest",
        ),
        (
            |body| body.asset_digest = [0x0e; 32],
            "request.asset_digest",
        ),
        (|body| body.fee += 1, "request.fee"),
        (
            |body| body.certificates = [0x0f; 32],
            "request.certificates",
        ),
    ];
    for (mutate, field) in body_mutations {
        let (mut body, slot, certificates) = f.request_parts(true, 1_000);
        mutate(&mut body);
        let result = f.sign_request(&body, slot, certificates);
        if field == "request.gross" {
            assert!(matches!(
                result,
                Err(KagemushaWalletValidationErrorV1::ArithmeticOverflow {
                    field: "request.gross"
                })
            ));
        } else {
            assert_invalid(result, field);
        }
    }
    let (mut body, slot, certificates) = f.request_parts(true, 1_000);
    body.scheme_id = [0x10; 32];
    assert_mismatch(
        f.sign_request(&body, slot, certificates),
        "request.scheme_id",
    );

    // A fee without a schedule, and a schedule slot that disagrees with the body.
    let (mut body, _, _) = f.request_parts(false, 1_000);
    body.fee = 6;
    let (_, none, certificates) = f.request_parts(false, 1_000);
    assert_invalid(f.sign_request(&body, none, certificates), "request.fee");
    let (mut body, none, certificates) = f.request_parts(false, 1_000);
    body.fee_schedule = [0x12; 32];
    assert_invalid(
        f.sign_request(&body, none, certificates),
        "request.fee_schedule",
    );
    let (mut body, slot, certificates) = f.request_parts(true, 1_000);
    body.fee_schedule = [0x13; 32];
    assert_invalid(
        f.sign_request(&body, slot, certificates),
        "request.fee_schedule",
    );
    let mut foreign_asset = f.fee_body();
    foreign_asset.asset_digest = [0x14; 32];
    let (body, _, certificates) = f.request_parts(true, 1_000);
    assert_invalid(
        f.sign_request(
            &body,
            KagemushaWalletFeeScheduleSlotV1::Present {
                schedule: f.sign_fee_schedule(foreign_asset),
            },
            certificates,
        ),
        "fee_schedule.asset_digest",
    );
    // A schedule whose signer certificate has another role.
    let (mut body, _, _) = f.request_parts(true, 1_000);
    let mut wrong_role = f.fee_schedule();
    wrong_role.body.signer_certificate = f.receiver.enrollment_certificate.certificate_digest();
    let wrong_slot = KagemushaWalletFeeScheduleSlotV1::Present {
        schedule: wrong_role,
    };
    body.fee_schedule = wrong_slot.fee_schedule_digest();
    let wrong_set =
        KagemushaWalletCertificateSetV1::new(vec![f.receiver.enrollment_certificate]).expect("set");
    body.certificates = wrong_set.digest().expect("digest");
    assert_invalid(
        f.sign_request(&body, wrong_slot, wrong_set),
        "certificate.role",
    );

    // The certificate set is exactly the receiver issuer and the fee signer.
    let (body, slot, _) = f.request_parts(true, 1_000);
    let missing =
        KagemushaWalletCertificateSetV1::new(vec![f.receiver.enrollment_certificate]).expect("set");
    let mut missing_body = body;
    missing_body.certificates = missing.digest().expect("digest");
    assert_invalid(
        f.sign_request(&missing_body, slot, missing),
        "certificates.set",
    );
    let (body, none, _) = f.request_parts(false, 1_000);
    let extra = KagemushaWalletCertificateSetV1::new(vec![
        f.receiver.enrollment_certificate,
        f.regulator_certificate,
    ])
    .expect("set");
    let mut extra_body = body;
    extra_body.certificates = extra.digest().expect("digest");
    assert_invalid(f.sign_request(&extra_body, none, extra), "certificates.set");

    let mut tampered = f.request(true);
    tampered.body.receiver_accepted_time_ms += 1;
    assert_signature(tampered.validate(), Role::RequestBody);

    let request = f.request(true);
    let frame = norito::encode_canonical(&request).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, request.request_digest(), |bytes| {
        let request: KagemushaWalletRequestV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        request.validate().ok()?;
        Some(request.request_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_send_rule_and_send_effect() {
    let f = message_fixture();
    let request = f.request(true);
    let state = f.payer_state(&request);
    request
        .check_send_rule(&f.payer.credential, &state)
        .expect("send rule");

    let mut no_policy = state;
    no_policy.policy =
        KagemushaWalletStateV1::bootstrap(&f.payer.credential, &EMPTY_ROOTS, [0x5d; 32])
            .expect("bootstrap")
            .policy;
    assert_invalid(
        request.check_send_rule(&f.payer.credential, &no_policy),
        "request.policy_epoch",
    );
    let refreshed = |epoch: u64, enabled: u32, fee_schedule: [u8; 32]| {
        let mut payer = state;
        let policy = f.scheme_policy(epoch, enabled, fee_schedule);
        payer.policy.policy_epoch = 0;
        payer.policy.scheme_policy = [0; 32];
        payer.policy.fee_schedule = [0; 32];
        payer.policy = payer
            .refresh_policy(KagemushaWalletPolicyUpdateV1::SchemePolicy { policy: &policy })
            .expect("refresh")
            .policy;
        payer
    };
    // Same epoch, another policy object.
    assert_invalid(
        request.check_send_rule(
            &f.payer.credential,
            &refreshed(1, 1, request.body.fee_schedule),
        ),
        "request.scheme_policy",
    );
    // A newer payer epoch with the same fee schedule is accepted.
    request
        .check_send_rule(
            &f.payer.credential,
            &refreshed(2, 1, request.body.fee_schedule),
        )
        .expect("newer epoch");
    assert_invalid(
        request.check_send_rule(&f.payer.credential, &refreshed(2, 0, [0; 32])),
        "request.fee_schedule",
    );
    let mut ordinal = state;
    ordinal.next_send += 1;
    assert_invalid(
        request.check_send_rule(&f.payer.credential, &ordinal),
        "request.send_ordinal",
    );
    let mut poor = state;
    poor.balance = 1_005;
    assert_invalid(
        request.check_send_rule(&f.payer.credential, &poor),
        "state.balance",
    );
    poor.balance = 1_006;
    request
        .check_send_rule(&f.payer.credential, &poor)
        .expect("exact gross balance");

    // A wallet that reuses the receiver's payment key cannot pay it.
    let same_key = same_key_credential(&f.receiver);
    let (mut body, slot, certificates) = f.request_parts(true, 1_000);
    body.payer_wallet_id = same_key.body.wallet_id;
    let same_key_request = f.sign_request(&body, slot, certificates).expect("request");
    let mut same_key_state =
        KagemushaWalletStateV1::bootstrap(&same_key, &EMPTY_ROOTS, [0x5d; 32]).expect("state");
    same_key_state.policy = state.policy;
    same_key_state.balance = state.balance;
    same_key_state.next_send = state.next_send;
    assert_invalid(
        same_key_request.check_send_rule(&same_key, &same_key_state),
        "request.payment_key",
    );
    // The receiver's own state is not the payer.
    let receiver_state =
        KagemushaWalletStateV1::bootstrap(&f.receiver.credential, &EMPTY_ROOTS, [0x5d; 32])
            .expect("state");
    assert_invalid(
        request.check_send_rule(&f.receiver.credential, &receiver_state),
        "request.payer_wallet_id",
    );

    let interval =
        KagemushaWalletTimeIntervalV1::new(ACCEPTED_MS + 1, ACCEPTED_MS + 9).expect("interval");
    let effect = request
        .send_effect(&f.payer.credential, &interval)
        .expect("effect");
    let dependencies = kagemusha_wallet_send_dependencies_v1(
        &f.payer.enrollment_certificate.certificate_digest(),
        &f.receiver.enrollment_certificate.certificate_digest(),
        &f.regulator_certificate.certificate_digest(),
    );
    assert_eq!(
        effect,
        KagemushaWalletEffectV1::Send {
            credit_id: request.credit_id(),
            receiver_wallet_id: f.receiver.credential.body.wallet_id,
            send_ordinal: 4,
            amount: 1_000,
            fee: 6,
            request: request.request_digest(),
            dependencies,
            accepted_lower_ms: ACCEPTED_MS + 1,
            accepted_upper_ms: ACCEPTED_MS + 9,
        }
    );
    assert_eq!(
        request
            .send_dependencies(&f.payer.credential)
            .expect("payer dependencies"),
        dependencies
    );
    let transcript = kagemusha_wallet_send_dependencies_transcript_v1(&[1; 32], &[2; 32], &[0; 32]);
    assert_eq!(&transcript[..4], &3_u32.to_le_bytes());
    assert_eq!(&transcript[4..36], &[1; 32]);
    assert_eq!(&transcript[68..], &[0; 32]);
    assert_eq!(
        kagemusha_wallet_send_dependencies_v1(&[1; 32], &[2; 32], &[0; 32]),
        kagemusha_wallet_digest_v1(Role::Dependencies, &transcript)
    );
    let without_fee = f.request(false);
    assert_eq!(
        without_fee
            .send_dependencies(&f.payer.credential)
            .expect("payer dependencies"),
        kagemusha_wallet_send_dependencies_v1(
            &f.payer.enrollment_certificate.certificate_digest(),
            &f.receiver.enrollment_certificate.certificate_digest(),
            &[0; 32],
        )
    );
    let early = KagemushaWalletTimeIntervalV1::new(ACCEPTED_MS - 1, ACCEPTED_MS).expect("interval");
    assert_invalid(
        request.send_effect(&f.payer.credential, &early),
        "effect.accepted_time",
    );
    let inverted = KagemushaWalletTimeIntervalV1 {
        lower_ms: ACCEPTED_MS + 2,
        upper_ms: ACCEPTED_MS + 1,
    };
    assert_invalid(
        request.send_effect(&f.payer.credential, &inverted),
        "effect.accepted_time",
    );

    // Only the payer the Request names can build its irreversible Send effect.
    assert_invalid(
        request.send_effect(&f.receiver.credential, &interval),
        "request.payer_wallet_id",
    );
    assert_invalid(
        request.send_dependencies(&f.receiver.credential),
        "request.payer_wallet_id",
    );
    let stranger = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox, 0x5f);
    assert_invalid(
        request.send_effect(&stranger.credential, &interval),
        "request.payer_wallet_id",
    );
    let mut body = f.payer.credential.body;
    body.asset_digest = [0x77; 32];
    body.wallet_id = kagemusha_wallet_id_v1(
        &body.scheme_id,
        &body.asset_digest,
        &body.payment_key,
        &body.enrollment_id,
    );
    let other_asset = f.payer.issue(&body).expect("other asset credential");
    assert_invalid(
        request.send_effect(&other_asset, &interval),
        "request.payer_credential.asset_digest",
    );
    let same_key = same_key_credential(&f.receiver);
    let (mut body, slot, certificates) = f.request_parts(true, 1_000);
    body.payer_wallet_id = same_key.body.wallet_id;
    let same_key_request = f.sign_request(&body, slot, certificates).expect("request");
    assert_invalid(
        same_key_request.send_effect(&same_key, &interval),
        "request.payment_key",
    );
    assert_invalid(
        same_key_request.send_dependencies(&same_key),
        "request.payment_key",
    );
}

#[test]
fn kagemusha_wallet_v1_payment_digests_and_bindings() {
    let f = message_fixture();
    for fee in [false, true] {
        let payment = f.payment(fee, 64);
        let digests = payment.digests().expect("payment");
        assert_eq!(payment.verify(&f.payer.scheme).expect("verify"), digests);
        assert_eq!(payment.certificates.len(), 1);
        assert_eq!(digests.credit_id, payment.request.credit_id());
        assert_eq!(digests.request, payment.request.request_digest());
        assert_eq!(
            digests.payer_credential,
            f.payer.credential.credential_digest()
        );
        assert_eq!(
            digests.package,
            payment.send.verify(&f.payer.credential).expect("package")
        );
        let mut transcript = vec![1, 0];
        for digest in [
            digests.request,
            digests.payer_credential,
            digests.package.package,
            digests.certificates,
        ] {
            transcript.extend_from_slice(&digest);
        }
        assert_eq!(
            transcript,
            kagemusha_wallet_payment_transcript_v1(
                &digests.request,
                &digests.payer_credential,
                &digests.package.package,
                &digests.certificates,
            )
        );
        assert_eq!(
            digests.payment,
            kagemusha_wallet_digest_v1(Role::Payment, &transcript)
        );
        assert_eq!(payment.payment_digest().expect("digest"), digests.payment);
        payment.validate().expect("validate");

        assert_eq!(
            payment.pending_outgoing_leaf().expect("leaf"),
            KagemushaWalletPendingOutgoingLeafV1 {
                credit_id: digests.credit_id,
                receiver_wallet_id: f.receiver.credential.body.wallet_id,
                send_ordinal: 4,
                amount: 1_000,
                fee: payment.request.body.fee,
                request_digest: digests.request,
            }
        );
        assert_eq!(
            payment.consumed_credit_leaf().expect("leaf"),
            KagemushaWalletConsumedCreditLeafV1 {
                credit_id: digests.credit_id,
                payment_digest: digests.payment,
            }
        );
        let fee_leaf = payment.fee_claim_leaf().expect("fee leaf");
        assert_eq!(fee_leaf.is_some(), fee);
        if let Some(leaf) = fee_leaf {
            assert_eq!(leaf.fee, 6);
            assert_eq!(leaf.fee_schedule_digest, payment.request.body.fee_schedule);
        }
        assert_eq!(
            payment
                .receive_effect(&f.receiver.credential)
                .expect("receive"),
            KagemushaWalletEffectV1::Receive {
                credit_id: digests.credit_id,
                payer_wallet_id: f.payer.credential.body.wallet_id,
                payment: digests.payment,
                amount: 1_000,
            }
        );
        let frame = payment.to_canonical_bytes().expect("frame");
        assert_eq!(
            KagemushaWalletPaymentV1::decode_canonical(&frame, &f.scheme_id()).expect("decode"),
            payment
        );
        assert_mismatch(
            KagemushaWalletPaymentV1::decode_canonical(&frame, &[0x22; 32]),
            "payment.scheme_id",
        );
    }

    let payment = f.payment(true, 64);
    // The receiver matches by wallet identity, so a renewed receiver credential still receives.
    payment
        .receive_effect(&renewed_credential(&f.receiver))
        .expect("renewed receiver");
    assert_invalid(
        payment.receive_effect(&f.payer.credential),
        "payment.receiver_wallet_id",
    );

    // A payer under the receiver's issuer needs no further certificate.
    let shared = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox, 0x53);
    let (mut body, slot, certificates) = f.request_parts(false, 1_000);
    body.payer_wallet_id = shared.credential.body.wallet_id;
    let request = f.sign_request(&body, slot, certificates).expect("request");
    let interval = KagemushaWalletTimeIntervalV1::new(ACCEPTED_MS, ACCEPTED_MS).expect("interval");
    let statement = transition_statement(
        &shared,
        3,
        0,
        KagemushaWalletLifecycleV1::Active,
        request
            .send_effect(&shared.credential, &interval)
            .expect("effect"),
    );
    let send = signed_package(&shared, &shared.credential, &statement, stand_in_proof(32));
    let carried = KagemushaWalletPaymentV1::assemble(
        request,
        shared.credential,
        &shared.enrollment_certificate,
        send,
    )
    .expect("carried issuer");
    assert!(carried.certificates.is_empty());
    carried.verify(&f.payer.scheme).expect("verify");
    let mut redundant = carried.clone();
    redundant.certificates =
        KagemushaWalletCertificateSetV1::new(vec![shared.enrollment_certificate]).expect("set");
    assert_invalid(redundant.validate(), "certificates.set");

    let mut version = payment.clone();
    version.version = 2;
    assert!(matches!(
        version.digests(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "payment.version",
            version: 2
        })
    ));
    let mut wrong_payer = payment.clone();
    wrong_payer.payer_credential = f.receiver.credential;
    assert_invalid(wrong_payer.validate(), "payment.payer_wallet_id");
    let mut missing = payment.clone();
    missing.certificates = KagemushaWalletCertificateSetV1::default();
    assert_invalid(missing.validate(), "certificates.set");
    let mut extra = payment.clone();
    extra.certificates = KagemushaWalletCertificateSetV1::new(vec![
        f.payer.enrollment_certificate,
        f.regulator_certificate,
    ])
    .expect("set");
    assert_invalid(extra.validate(), "certificates.set");

    // The payer and receiver keys differ.
    let same_key = same_key_credential(&f.receiver);
    let mut same_key_payment = payment.clone();
    same_key_payment.payer_credential = same_key;
    same_key_payment.request.body.payer_wallet_id = same_key.body.wallet_id;
    assert_invalid(same_key_payment.validate(), "payment.payment_key");

    // Every Send effect field is bound to the Request.
    let base = payment.send.statement.effect;
    assert!(matches!(base, KagemushaWalletEffectV1::Send { .. }));
    let send_with = |effect: KagemushaWalletEffectV1| {
        let mut mutated = payment.clone();
        mutated.send = signed_package(
            &f.payer,
            &f.payer.credential,
            &f.payer_statement(effect),
            stand_in_proof(64),
        );
        mutated.validate()
    };
    let mutations: [(EffectMutation, &str); 8] = [
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { credit_id, .. } = effect {
                    *credit_id = [0x31; 32];
                }
            },
            "payment.effect.credit_id",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send {
                    receiver_wallet_id, ..
                } = effect
                {
                    *receiver_wallet_id = [0x32; 32];
                }
            },
            "payment.effect.receiver_wallet_id",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { send_ordinal, .. } = effect {
                    *send_ordinal += 1;
                }
            },
            "payment.effect.send_ordinal",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { amount, .. } = effect {
                    *amount += 1;
                }
            },
            "payment.effect.amount",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { fee, .. } = effect {
                    *fee += 1;
                }
            },
            "payment.effect.fee",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { request, .. } = effect {
                    *request = [0x33; 32];
                }
            },
            "payment.effect.request",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send { dependencies, .. } = effect {
                    *dependencies = [0x34; 32];
                }
            },
            "payment.effect.dependencies",
        ),
        (
            |effect| {
                if let KagemushaWalletEffectV1::Send {
                    accepted_lower_ms, ..
                } = effect
                {
                    *accepted_lower_ms = ACCEPTED_MS - 1;
                }
            },
            "payment.effect.accepted_time",
        ),
    ];
    for (mutate, field) in mutations {
        let mut effect = base;
        mutate(&mut effect);
        assert_invalid(send_with(effect), field);
    }
    assert_invalid(
        send_with(KagemushaWalletEffectV1::ArchiveSent {
            credit_id: [0x36; 32],
            credited: [0x37; 32],
        }),
        "payment.effect",
    );

    // A tampered receipt-bound statement and a tampered Request are rejected.
    let mut receipt = payment.clone();
    receipt.send.statement.successor.eq[0] ^= 1;
    assert_signature(receipt.validate(), Role::ReceiptBody);
    let mut request_signature = payment.clone();
    request_signature.request.body.receiver_accepted_time_ms += 1;
    assert_invalid(request_signature.validate(), "payment.effect.credit_id");
    let mut foreign = f.payer.scheme;
    foreign.relation_id = [0x35; 32];
    assert!(payment.verify(&foreign).is_err());
}

#[test]
fn kagemusha_wallet_v1_maximum_payment_binds_every_byte() {
    let f = message_fixture();
    let payment = f.payment(true, KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1);
    assert_eq!(
        payment.request.certificates.len() + payment.certificates.len(),
        3
    );
    let digest = payment.payment_digest().expect("digest");
    let frame = norito::encode_canonical(&payment).expect("encode");
    assert!(frame.len() <= KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1);
    assert_every_flip_rejected_or_rebound(&frame, digest, |bytes| {
        let payment: KagemushaWalletPaymentV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        payment.payment_digest().ok()
    });
}

#[test]
fn kagemusha_wallet_v1_credited_receive_evidence() {
    let f = message_fixture();
    let payment = f.payment(true, 64);
    let digests = payment.digests().expect("payment");
    let credited = f.credited_receive(&payment, 48);
    assert_eq!(credited.credit_id, digests.credit_id);
    assert_eq!(credited.payment_digest, digests.payment);
    let digest = credited.verify(&f.payer.scheme).expect("verify");
    assert_eq!(credited.credited_digest().expect("digest"), digest);
    let KagemushaWalletCreditedEvidenceV1::Receive { package } = &credited.evidence else {
        panic!("receive evidence");
    };
    let package_digest = package
        .verify(&f.receiver.credential)
        .expect("package")
        .package;
    let mut transcript = vec![1, 0];
    transcript.extend_from_slice(&digests.credit_id);
    transcript.extend_from_slice(&digests.payment);
    transcript.extend_from_slice(&f.receiver.credential.credential_digest());
    transcript.push(1);
    transcript.extend_from_slice(&package_digest);
    transcript.extend_from_slice(&[0; 64]);
    transcript.extend_from_slice(&credited.certificates.digest().expect("set"));
    assert_eq!(credited.transcript().expect("transcript"), transcript);
    assert_eq!(
        digest,
        kagemusha_wallet_digest_v1(Role::Credited, &transcript)
    );

    let mut other_credit = credited.clone();
    other_credit.credit_id = [0x41; 32];
    assert_invalid(other_credit.validate(), "credited.receive.credit_id");
    let mut other_payment = credited.clone();
    other_payment.payment_digest = [0x42; 32];
    assert_invalid(other_payment.validate(), "credited.receive.payment");
    let mut not_receive = credited.clone();
    not_receive.evidence = KagemushaWalletCreditedEvidenceV1::Receive {
        package: f.current_package(32),
    };
    assert_invalid(not_receive.validate(), "credited.evidence");
    assert_invalid(
        KagemushaWalletCreditedV1::from_receive(
            f.receiver.credential,
            &f.receiver.enrollment_certificate,
            f.current_package(32),
        ),
        "credited.evidence",
    );
    let mut wrong_set = credited.clone();
    wrong_set.certificates =
        KagemushaWalletCertificateSetV1::new(vec![f.payer.enrollment_certificate]).expect("set");
    assert_invalid(wrong_set.validate(), "certificates.set");
    let mut other_credential = credited.clone();
    other_credential.receiver_credential = renewed_credential(&f.receiver);
    assert_invalid(other_credential.validate(), "statement.credential_digest");
    let mut version = credited.clone();
    version.version = 0;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));

    let frame = norito::encode_canonical(&credited).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, digest, |bytes| {
        let credited: KagemushaWalletCreditedV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        credited.credited_digest().ok()
    });
}

#[test]
fn kagemusha_wallet_v1_credited_status_evidence() {
    let f = message_fixture();
    let payment = f.payment(false, 64);
    let credited = f.credited_status(&payment, 40, 24);
    let digest = credited.verify(&f.payer.scheme).expect("verify");
    let KagemushaWalletCreditedEvidenceV1::Status { current, status } = &credited.evidence else {
        panic!("status evidence");
    };
    status.validate().expect("status");
    let transcript = credited.transcript().expect("transcript");
    assert_eq!(transcript[98], 2);
    assert_eq!(
        &transcript[99..131],
        &current
            .verify(&f.receiver.credential)
            .expect("current")
            .package
    );
    assert_eq!(&transcript[131..163], &status.statement.statement_digest());
    assert_eq!(&transcript[163..195], &status.proof.proof_digest());

    // One negative per equality of design C5.
    let invalid: [(ClaimMutation, &str); 9] = [
        (
            |claim| claim.credit_id = [0x51; 32],
            "credit_status.credit_id",
        ),
        (
            |claim| claim.payment_digest = [0x52; 32],
            "credit_status.payment_digest",
        ),
        (
            |claim| claim.asset_digest = [0x53; 32],
            "credit_status.asset_digest",
        ),
        (
            |claim| claim.receiver_wallet_id = [0x54; 32],
            "credit_status.receiver_wallet_id",
        ),
        (
            |claim| claim.receiver_credential_digest = [0x55; 32],
            "credit_status.receiver_credential_digest",
        ),
        (
            |claim| claim.current.eq = [0x56; 32],
            "credit_status.current",
        ),
        (
            |claim| claim.current_sequence += 1,
            "credit_status.current_sequence",
        ),
        (
            |claim| claim.current_statement_digest = [0x57; 32],
            "credit_status.current_statement_digest",
        ),
        (
            |claim| claim.current_receipt_digest = [0x58; 32],
            "credit_status.current_receipt_digest",
        ),
    ];
    let with_claim = |mutate: ClaimMutation| {
        let mut mutated = credited.clone();
        if let KagemushaWalletCreditedEvidenceV1::Status { status, .. } = &mut mutated.evidence {
            mutate(&mut status.statement);
        }
        mutated.validate()
    };
    for (mutate, field) in invalid {
        assert_invalid(with_claim(mutate), field);
    }
    assert_mismatch(
        with_claim(|claim| claim.scheme_id = [0x59; 32]),
        "credit_status.scheme_id",
    );
    assert_mismatch(
        with_claim(|claim| claim.relation_id = [0x5a; 32]),
        "credit_status.relation_id",
    );
    // Credited fields must equal the claim.
    let mut credit = credited.clone();
    credit.credit_id = [0x5b; 32];
    assert_invalid(credit.validate(), "credit_status.credit_id");
    // The current package must run under the receiver credential.
    let mut foreign_current = credited.clone();
    if let KagemushaWalletCreditedEvidenceV1::Status { current, .. } = &mut foreign_current.evidence
    {
        *current = payment.send.clone();
    }
    assert!(foreign_current.validate().is_err());
    let mut oversized = credited.clone();
    if let KagemushaWalletCreditedEvidenceV1::Status { status, .. } = &mut oversized.evidence {
        status.proof = stand_in_proof(KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1 + 1);
    }
    assert_invalid(oversized.validate(), "proof.bytes");
    let zero_claim = KagemushaWalletCreditStatusStatementV1 {
        credit_id: [0; 32],
        ..status.statement
    };
    assert_invalid(zero_claim.validate(), "credit_status.credit_id");

    let frame = norito::encode_canonical(&credited).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, digest, |bytes| {
        let credited: KagemushaWalletCreditedV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1).ok()?;
        credited.credited_digest().ok()
    });
}

#[test]
fn kagemusha_wallet_v1_archive_sent_matches_by_receiver_wallet() {
    let f = message_fixture();
    let payment = f.payment(true, 64);
    let pending = payment.pending_outgoing_leaf().expect("pending");
    for credited in [
        f.credited_receive(&payment, 32),
        f.credited_status(&payment, 32, 16),
    ] {
        assert_eq!(
            credited
                .archive_sent_effect(&payment, &pending)
                .expect("archive"),
            KagemushaWalletEffectV1::ArchiveSent {
                credit_id: pending.credit_id,
                credited: credited.credited_digest().expect("digest"),
            }
        );
        let other = f.payment(false, 64);
        assert_invalid(
            credited.archive_sent_effect(&other, &other.pending_outgoing_leaf().expect("leaf")),
            "credited.payment_digest",
        );
        let mut wrong_leaf = pending;
        wrong_leaf.amount += 1;
        assert_invalid(
            credited.archive_sent_effect(&payment, &wrong_leaf),
            "pending_outgoing",
        );
    }

    // Evidence from a renewed receiver credential still matches by wallet identity.
    let renewed = renewed_credential(&f.receiver);
    let fixture = IdentityFixture {
        credential: renewed,
        ..identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x52)
    };
    let current = signed_package(
        &fixture,
        &renewed,
        &transition_statement(
            &fixture,
            11,
            0,
            KagemushaWalletLifecycleV1::Active,
            send_effect(),
        ),
        stand_in_proof(32),
    );
    let digests = payment.digests().expect("digests");
    let status = KagemushaWalletCreditStatusV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        statement: KagemushaWalletCreditStatusStatementV1::for_current(
            &renewed,
            &current,
            digests.credit_id,
            digests.payment,
        )
        .expect("status"),
        proof: stand_in_proof(16),
    };
    let credited = KagemushaWalletCreditedV1::from_status(
        renewed,
        &f.receiver.enrollment_certificate,
        current,
        status,
    )
    .expect("credited");
    assert_ne!(
        renewed.credential_digest(),
        payment.request.body.receiver_credential_digest
    );
    credited
        .archive_sent_effect(&payment, &pending)
        .expect("renewed receiver evidence");
    // Evidence from another wallet does not match.
    let other_wallet = identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x5c);
    let foreign = KagemushaWalletCreditedV1 {
        receiver_credential: other_wallet.credential,
        ..credited
    };
    assert!(foreign.archive_sent_effect(&payment, &pending).is_err());
}

#[test]
fn kagemusha_wallet_v1_session_controls() {
    use KagemushaWalletSessionControlKindV1 as Kind;
    let f = message_fixture();
    for kind in [Kind::SetupDeclined, Kind::ReceiveDeferred, Kind::Close] {
        let control = f.signed_control(kind);
        control.validate().expect("valid");
        control
            .verify(Some(&f.payer.credential))
            .expect("signed control");
        assert_invalid(control.verify(None), "session_control.sender_key");
        assert!(control.verify(Some(&f.receiver.credential)).is_err());
        let mut tampered = control;
        tampered.session_nonce = [0x60; 32];
        assert_signature(
            tampered.verify(Some(&f.payer.credential)),
            Role::SessionControlBody,
        );
    }
    assert_eq!(
        f.signed_control(Kind::Close).body_digest(),
        kagemusha_wallet_digest_v1(
            Role::SessionControlBody,
            &f.control(Kind::Close).transcript()
        )
    );

    // Unsigned controls: UnsupportedScheme always, SetupDeclined only before the sender's key
    // is known in the session.
    let declined = f.control(Kind::SetupDeclined);
    declined.verify(None).expect("early decline");
    assert_invalid(
        declined.verify(Some(&f.payer.credential)),
        "session_control.auth",
    );
    assert_invalid(f.control(Kind::Close).verify(None), "session_control.auth");
    let mut unsupported = f.control(Kind::UnsupportedScheme);
    unsupported.sender_wallet_id = [0; 32];
    unsupported.verify(None).expect("unsupported scheme");
    unsupported
        .verify(Some(&f.payer.credential))
        .expect("unsupported scheme stays unsigned");
    assert_invalid(
        f.control(Kind::UnsupportedScheme).sign(
            &f.payer.credential,
            raw_output(&f.payer.payment, &unsupported.signing_message()),
        ),
        "session_control.auth",
    );
    let mut signed_unsupported = f.control(Kind::UnsupportedScheme);
    signed_unsupported.auth = f.signed_control(Kind::Close).auth;
    assert_invalid(signed_unsupported.validate(), "session_control.auth");

    let mutations: [(ControlMutation, &str); 6] = [
        (|control| control.reason = 1, "session_control.reason"),
        (
            |control| control.credit_id = [1; 32],
            "session_control.credit_id",
        ),
        (
            |control| control.peer_wallet_id = control.sender_wallet_id,
            "session_control.peer_wallet_id",
        ),
        (
            |control| control.session_nonce = [0; 32],
            "session_control.session_nonce",
        ),
        (
            |control| control.sender_wallet_id = [0; 32],
            "session_control.sender_wallet_id",
        ),
        (
            |control| control.asset_digest = [0; 32],
            "session_control.asset_digest",
        ),
    ];
    for (mutate, field) in mutations {
        let mut control = f.control(Kind::Close);
        mutate(&mut control);
        assert_invalid(control.validate(), field);
    }
    let mut deferred = f.control(Kind::ReceiveDeferred);
    deferred.credit_id = [0; 32];
    assert_invalid(deferred.validate(), "session_control.credit_id");
    let mut unknown_peer = f.control(Kind::Close);
    unknown_peer.peer_wallet_id = [0; 32];
    unknown_peer.validate().expect("unknown peer");
    let mut foreign = f.control(Kind::Close);
    foreign.sender_wallet_id = f.receiver.credential.body.wallet_id;
    foreign.peer_wallet_id = [0; 32];
    assert_invalid(
        foreign.sign(
            &f.payer.credential,
            raw_output(&f.payer.payment, &foreign.signing_message()),
        ),
        "session_control.sender_wallet_id",
    );

    let control = f.signed_control(Kind::ReceiveDeferred);
    let frame = norito::encode_canonical(&control).expect("encode");
    assert_every_flip_rejected_or_rebound(&frame, control.body_digest(), |bytes| {
        let control: KagemushaWalletSessionControlV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1).ok()?;
        control.verify(Some(&f.payer.credential)).ok()?;
        Some(control.body_digest())
    });
}

#[test]
fn kagemusha_wallet_v1_policy_data_items() {
    let f = message_fixture();
    let known = [f.regulator_certificate];
    let policy = f.policy_data(KagemushaWalletPolicyDataItemV1::SchemePolicy {
        policy: f.scheme_policy(2, 0, [0; 32]),
    });
    policy
        .verify(&f.payer.scheme, &known)
        .expect("scheme policy");
    assert_invalid(policy.verify(&f.payer.scheme, &[]), "certificates.missing");
    let schedule = f.policy_data(KagemushaWalletPolicyDataItemV1::FeeSchedule {
        schedule: f.fee_schedule(),
    });
    schedule
        .verify(&f.payer.scheme, &known)
        .expect("fee schedule");
    let certificates = f.policy_data(KagemushaWalletPolicyDataItemV1::Certificates {
        certificates: KagemushaWalletCertificateSetV1::new(vec![
            f.regulator_certificate,
            f.payer.enrollment_certificate,
        ])
        .expect("set"),
    });
    certificates
        .verify(&f.payer.scheme, &[])
        .expect("certificates");

    let mut other_asset = schedule.clone();
    other_asset.asset_digest = [0x61; 32];
    assert_invalid(other_asset.validate(), "policy_data.asset_digest");
    let mut other_scheme = policy.clone();
    other_scheme.scheme_id = [0x62; 32];
    assert_mismatch(other_scheme.validate(), "policy_data.scheme_id");
    assert_invalid(
        f.policy_data(KagemushaWalletPolicyDataItemV1::Certificates {
            certificates: KagemushaWalletCertificateSetV1::default(),
        })
        .validate(),
        "policy_data.certificates",
    );
    let other = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x63);
    let mut foreign_root = other.scheme;
    foreign_root.scheme_root_key = other.credential.body.payment_key;
    let foreign_certificate = test_certificate(
        &foreign_root,
        &other.payment,
        KagemushaWalletSignerRoleV1::TimeAnchor,
        &other.payment,
        1,
    );
    assert_mismatch(
        f.policy_data(KagemushaWalletPolicyDataItemV1::Certificates {
            certificates: KagemushaWalletCertificateSetV1::new(vec![foreign_certificate])
                .expect("set"),
        })
        .validate(),
        "policy_data.scheme_id",
    );
    let mut version = policy.clone();
    version.version = 2;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));
}

#[test]
fn kagemusha_wallet_v1_envelope_round_trips_every_kind() {
    let f = message_fixture();
    assert_eq!(
        norito::core::archived_payload_align::<KagemushaWalletEnvelopeV1>(),
        16
    );
    for message in sample_messages(&f) {
        let envelope = KagemushaWalletEnvelopeV1::new(message.clone());
        let frame = envelope.to_canonical_bytes().expect("frame");
        assert_eq!(payload_range(&frame).start, 48);
        assert!(frame.len() <= message.max_bytes());
        assert_eq!(
            KagemushaWalletEnvelopeV1::decode_canonical(&frame, &f.scheme_id()).expect("decode"),
            envelope
        );
        let text = envelope.to_text().expect("text");
        assert!(text.starts_with(KAGEMUSHA_WALLET_TEXT_PREFIX_V1));
        assert_eq!(text, kagemusha_wallet_text_encode_v1(&frame));
        assert_eq!(kagemusha_wallet_text_decode_v1(&text).expect("raw"), frame);
        assert_eq!(
            KagemushaWalletEnvelopeV1::from_text(&text, &f.scheme_id()).expect("from text"),
            envelope
        );
        let text_max = if message.max_bytes() == KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1 {
            KAGEMUSHA_WALLET_SESSION_TEXT_MAX_BYTES_V1
        } else {
            KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1
        };
        assert!(text.len() <= text_max);
        assert_eq!(message.version(), KAGEMUSHA_WALLET_VERSION_V1);
        assert_eq!(message.scheme_id(), &f.scheme_id());
    }
}

#[test]
fn kagemusha_wallet_v1_envelope_bounds_per_kind() {
    let f = message_fixture();
    for message in sample_messages(&f) {
        let max = message.max_bytes();
        let expected = match message {
            KagemushaWalletMessageV1::Offer { .. }
            | KagemushaWalletMessageV1::SessionControl { .. } => 2_048,
            _ => 10_000,
        };
        assert_eq!(max, expected);
        require_message_bound_v1(&message, max).expect("at the bound");
        assert!(matches!(
            require_message_bound_v1(&message, max + 1),
            Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { actual, max: limit })
                if actual == max + 1 && limit == max
        ));
    }

    // A decodable Offer above the session bound is rejected after decode, before validation.
    let mut offer = f.offer();
    offer.certificates = KagemushaWalletCertificateSetV1 {
        certificates: vec![f.payer.enrollment_certificate; 12],
    };
    let oversized = KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Offer { offer });
    let frame = norito::encode_canonical(&oversized).expect("encode");
    assert!(frame.len() > KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1);
    assert!(frame.len() <= KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1);
    assert_too_large(
        KagemushaWalletEnvelopeV1::decode_canonical(&frame, &[0x77; 32]),
        KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1,
    );
    // A Payment whose proof exceeds its cap is rejected at encode.
    let mut payment = f.payment(true, 64);
    payment.send.proof = stand_in_proof(KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1 + 1);
    assert!(
        KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Payment { payment })
            .to_canonical_bytes()
            .is_err()
    );
}

#[test]
fn kagemusha_wallet_v1_envelope_decode_order() {
    let f = message_fixture();
    let envelope =
        KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Offer { offer: f.offer() });
    let frame = envelope.to_canonical_bytes().expect("frame");
    let scheme_id = f.scheme_id();

    // The byte cap applies before any parsing.
    let garbage = vec![0xa5; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + 1];
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(&garbage, &scheme_id),
        Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded {
            actual: 10_001,
            max: 10_000
        })
    ));
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(&trailing, &scheme_id),
        Err(KagemushaWalletValidationErrorV1::Codec(_))
    ));
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(&frame[..frame.len() - 1], &scheme_id),
        Err(KagemushaWalletValidationErrorV1::Codec(_))
    ));

    // Versions are checked before the scheme.
    let mut version = envelope.clone();
    version.version = 2;
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&version).expect("encode"),
            &[0x78; 32]
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "envelope.version",
            version: 2
        })
    ));
    let mut message_version = envelope.clone();
    if let KagemushaWalletMessageV1::Offer { offer } = &mut message_version.message {
        offer.body.version = 3;
    }
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&message_version).expect("encode"),
            &[0x78; 32]
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "message.version",
            version: 3
        })
    ));
    assert_mismatch(
        KagemushaWalletEnvelopeV1::decode_canonical(&frame, &[0x78; 32]),
        "envelope.scheme_id",
    );
    // Every nested version precedes the per-kind bound and the scheme, including the version
    // of the nested body that supplies the checked scheme (design §0).
    let payment = f.payment(true, 64);
    let mut nested = payment.clone();
    nested.request.body.version = 2;
    nested.request.body.scheme_id = [0x78; 32];
    let nested_frame = norito::encode_canonical(&nested).expect("encode");
    assert!(matches!(
        KagemushaWalletPaymentV1::decode_canonical(&nested_frame, &scheme_id),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "request.version",
            version: 2
        })
    ));
    let nested_envelope =
        KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Payment { payment: nested });
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&nested_envelope).expect("encode"),
            &scheme_id
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "request.version",
            version: 2
        })
    ));
    let mut receipt_version = payment.clone();
    receipt_version.send.receipt.version = 4;
    assert!(matches!(
        KagemushaWalletPaymentV1::decode_canonical(
            &norito::encode_canonical(&receipt_version).expect("encode"),
            &[0x78; 32]
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "receipt.version",
            version: 4
        })
    ));
    let mut credential_version = f.credited_receive(&payment, 32);
    credential_version.receiver_credential.body.version = 2;
    credential_version.receiver_credential.body.scheme_id = [0x78; 32];
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&KagemushaWalletEnvelopeV1::new(
                KagemushaWalletMessageV1::Credited {
                    credited: credential_version
                }
            ))
            .expect("encode"),
            &scheme_id
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "credential.version",
            version: 2
        })
    ));
    let mut tampered = envelope.clone();
    if let KagemushaWalletMessageV1::Offer { offer } = &mut tampered.message {
        offer.body.amount += 1;
    }
    assert_signature(
        KagemushaWalletEnvelopeV1::decode_canonical(
            &norito::encode_canonical(&tampered).expect("encode"),
            &scheme_id,
        ),
        Role::OfferBody,
    );
    assert_signature(tampered.to_canonical_bytes(), Role::OfferBody);

    // The scheme field per kind (design C5).
    let payment = f.payment(true, 64);
    let credited = f.credited_receive(&payment, 32);
    assert_eq!(
        KagemushaWalletMessageV1::Payment {
            payment: payment.clone()
        }
        .scheme_id(),
        &payment.request.body.scheme_id
    );
    assert_eq!(
        KagemushaWalletMessageV1::Credited {
            credited: credited.clone()
        }
        .scheme_id(),
        &credited.receiver_credential.body.scheme_id
    );
}

#[test]
fn kagemusha_wallet_v1_unsupported_scheme_reply_to_an_offer() {
    let f = message_fixture();
    let offer = f.offer();
    let envelope = KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Offer {
        offer: offer.clone(),
    });
    let frame = envelope.to_canonical_bytes().expect("frame");
    // A receiver without this scheme cannot decode the Offer for its own scheme...
    assert_mismatch(
        KagemushaWalletEnvelopeV1::decode_canonical(&frame, &[0x78; 32]),
        "envelope.scheme_id",
    );
    // ...but learns the offered scheme through the bounded path and declines it (§8).
    let offered = KagemushaWalletEnvelopeV1::decode_offered_scheme(&frame).expect("offered");
    assert_eq!(offered, offer.body);
    assert_eq!(
        KagemushaWalletEnvelopeV1::offered_scheme_from_text(&envelope.to_text().expect("text"))
            .expect("offered text"),
        offer.body
    );
    let reply = KagemushaWalletSessionControlV1::unsupported_scheme(&offered).expect("reply");
    assert_eq!(
        reply,
        KagemushaWalletSessionControlV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: offer.body.scheme_id,
            asset_digest: offer.body.asset_digest,
            sender_wallet_id: [0; 32],
            peer_wallet_id: offer.body.payer_wallet_id,
            session_nonce: offer.body.session_nonce,
            kind: KagemushaWalletSessionControlKindV1::UnsupportedScheme,
            reason: 0,
            credit_id: [0; 32],
            auth: KagemushaWalletSessionAuthV1::Unsigned,
        }
    );
    // The payer decodes the reply under its own scheme and accepts it unsigned.
    let reply_frame =
        KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::SessionControl { control: reply })
            .to_canonical_bytes()
            .expect("reply frame");
    let decoded =
        KagemushaWalletEnvelopeV1::decode_canonical(&reply_frame, &f.scheme_id()).expect("reply");
    assert_eq!(
        decoded.message,
        KagemushaWalletMessageV1::SessionControl { control: reply }
    );
    reply.verify(None).expect("unsigned UnsupportedScheme");
    // The returned body is unauthenticated: a broken signature does not stop the decline.
    let mut tampered = envelope.clone();
    if let KagemushaWalletMessageV1::Offer { offer } = &mut tampered.message {
        offer.body.amount += 1;
    }
    assert_eq!(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(
            &norito::encode_canonical(&tampered).expect("encode")
        )
        .expect("unauthenticated body")
        .amount,
        offer.body.amount + 1
    );
    // Only an Offer is accepted, in the bounded decode order.
    let request_frame = KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Request {
        request: f.request(true),
    })
    .to_canonical_bytes()
    .expect("request frame");
    assert_invalid(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(&request_frame),
        "envelope.message",
    );
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(&vec![
            0xa5;
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                + 1
        ]),
        Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    let mut version = envelope.clone();
    if let KagemushaWalletMessageV1::Offer { offer } = &mut version.message {
        offer.body.version = 2;
    }
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(
            &norito::encode_canonical(&version).expect("encode")
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "message.version",
            version: 2
        })
    ));
    let mut padded = frame.clone();
    padded.push(0);
    assert!(matches!(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(&padded),
        Err(KagemushaWalletValidationErrorV1::Codec(_))
    ));
    let mut zero_nonce = offer.body;
    zero_nonce.session_nonce = [0; 32];
    assert_invalid(
        KagemushaWalletSessionControlV1::unsupported_scheme(&zero_nonce),
        "offer.session_nonce",
    );
    let mut zero_nonce_offer = envelope;
    if let KagemushaWalletMessageV1::Offer { offer } = &mut zero_nonce_offer.message {
        offer.body.session_nonce = [0; 32];
    }
    assert_invalid(
        KagemushaWalletEnvelopeV1::decode_offered_scheme(
            &norito::encode_canonical(&zero_nonce_offer).expect("encode"),
        ),
        "offer.session_nonce",
    );
}

#[test]
fn kagemusha_wallet_v1_text_codec_is_strict() {
    let f = message_fixture();
    let envelope = KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::SessionControl {
        control: f.signed_control(KagemushaWalletSessionControlKindV1::Close),
    });
    let text = envelope.to_text().expect("text");
    let body = &text[KAGEMUSHA_WALLET_TEXT_PREFIX_V1.len()..];
    assert!(!body.contains('='));

    let cases: [(String, &str); 7] = [
        (body.to_owned(), "text.prefix"),
        (format!("kgm2:{body}"), "text.prefix"),
        ("KGM1:AA".to_owned(), "text.prefix"),
        ("kgm1:".to_owned(), "text.body"),
        (format!("{text}="), "text.alphabet"),
        (format!("{text}\n"), "text.alphabet"),
        (format!("{text}+"), "text.alphabet"),
    ];
    for (input, field) in cases {
        assert_invalid(kagemusha_wallet_text_decode_v1(&input), field);
    }
    assert_invalid(
        kagemusha_wallet_text_decode_v1("kgm1:AA/A"),
        "text.alphabet",
    );
    // Lengths of 1 mod 4 are rejected before decoding.
    let one_mod_four = format!("kgm1:{}", "A".repeat(5));
    assert_invalid(
        kagemusha_wallet_text_decode_v1(&one_mod_four),
        "text.length",
    );
    // Nonzero trailing bits do not round-trip.
    let noncanonical = "kgm1:AB";
    assert_invalid(
        kagemusha_wallet_text_decode_v1(noncanonical),
        "text.base64url",
    );
    assert_eq!(
        kagemusha_wallet_text_decode_v1("kgm1:AA").expect("canonical"),
        vec![0]
    );
    let oversized = format!(
        "kgm1:{}",
        "A".repeat(KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1)
    );
    assert_too_large(
        kagemusha_wallet_text_decode_v1(&oversized),
        KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1,
    );
    // Valid text of an invalid frame fails in the frame decoder.
    assert!(matches!(
        KagemushaWalletEnvelopeV1::from_text("kgm1:AAAA", &f.scheme_id()),
        Err(KagemushaWalletValidationErrorV1::Codec(_))
    ));
}

#[test]
fn kagemusha_wallet_v1_worst_case_message_sizes() {
    let f = message_fixture();
    let frame_len = |message: KagemushaWalletMessageV1| {
        norito::encode_canonical(&KagemushaWalletEnvelopeV1::new(message))
            .expect("encode")
            .len()
    };
    let offer = frame_len(KagemushaWalletMessageV1::Offer { offer: f.offer() });
    let control = frame_len(KagemushaWalletMessageV1::SessionControl {
        control: f.signed_control(KagemushaWalletSessionControlKindV1::ReceiveDeferred),
    });
    let request = frame_len(KagemushaWalletMessageV1::Request {
        request: f.request(true),
    });
    let payment = f.payment(true, KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1);
    assert_eq!(
        payment.request.certificates.len() + payment.certificates.len(),
        3
    );
    let receive = frame_len(KagemushaWalletMessageV1::Credited {
        credited: f.credited_receive(&payment, KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1),
    });
    let status = frame_len(KagemushaWalletMessageV1::Credited {
        credited: f.credited_status(
            &payment,
            KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1,
        ),
    });
    let payment = frame_len(KagemushaWalletMessageV1::Payment { payment });
    println!(
        "KAGEMUSHA wallet V1 worst-case envelopes: Offer {offer}, SessionControl {control}, \
         Request {request}, Payment {payment}, Credited::Receive {receive}, \
         Credited::Status {status} bytes"
    );
    println!(
        "KAGEMUSHA wallet V1 proof slack: Payment {} bytes, Credited::Receive {} bytes, \
         Credited::Status {} bytes beyond {} + {} proof bytes",
        KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 - payment,
        KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 - receive,
        KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1.saturating_sub(status),
        KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1,
    );
    assert!(
        offer <= KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1,
        "Offer {offer}"
    );
    assert!(
        control <= KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1,
        "SessionControl {control}"
    );
    for (name, len) in [
        ("Request", request),
        ("Payment", payment),
        ("Credited::Receive", receive),
        ("Credited::Status", status),
    ] {
        assert!(len <= KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, "{name} {len}");
    }
}
