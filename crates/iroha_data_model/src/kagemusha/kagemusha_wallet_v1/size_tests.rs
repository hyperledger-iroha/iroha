//! Worst-case envelope sizes of design C2 at the provisional proof budgets.
//!
//! Every worst case is built from fully valid objects at the maximum proof sizes and asserted
//! against its complete-frame bound. The test prints the measured byte counts and the largest
//! proof sizes that still fit, so a change of overhead is visible before G3 freezes the
//! relation. The proof bytes are stand-ins; only their lengths matter here. The signed
//! blacklist is not a peer message (it is downloaded online from the issuer), so it has no
//! envelope case here; its standalone frame cap is measured by the policy tests.

use super::{
    messages::messages_tests::MessageFixture,
    vectors_tests::{vector_control, vector_offer, vector_world},
    *,
};

/// Largest proof length searched for.
const SEARCH_LIMIT: usize = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;

/// Complete canonical envelope frame length of `message`, without validation.
fn envelope_len(message: &KagemushaWalletMessageV1) -> usize {
    norito::encode_canonical(&KagemushaWalletEnvelopeV1::new(message.clone()))
        .expect("encode envelope")
        .len()
}

/// Stand-in proof of exactly `len` bytes.
fn proof_of(len: usize) -> KagemushaWalletProofV1 {
    KagemushaWalletProofV1 {
        bytes: vec![0xa5; len],
    }
}

/// Largest `len` in `0..=limit` with `frame_len(len) <= bound`, for a nondecreasing
/// `frame_len`; zero when even one byte does not fit.
fn largest_fitting(bound: usize, limit: usize, frame_len: impl Fn(usize) -> usize) -> usize {
    let (mut fits, mut exceeds) = (0, limit + 1);
    while exceeds - fits > 1 {
        let middle = fits + (exceeds - fits) / 2;
        if frame_len(middle) <= bound {
            fits = middle;
        } else {
            exceeds = middle;
        }
    }
    fits
}

/// Credited `message` with its evidence proofs replaced by the given lengths.
fn credited_with(
    credited: &KagemushaWalletCreditedV1,
    package_proof: usize,
    status_proof: usize,
) -> KagemushaWalletMessageV1 {
    let mut credited = credited.clone();
    match &mut credited.evidence {
        KagemushaWalletCreditedEvidenceV1::Receive { package } => {
            package.proof = proof_of(package_proof);
        }
        KagemushaWalletCreditedEvidenceV1::Status { current, status } => {
            current.proof = proof_of(package_proof);
            status.proof = proof_of(status_proof);
        }
    }
    KagemushaWalletMessageV1::Credited { credited }
}

/// Measured worst cases of design C2 in complete envelope bytes.
struct WorstCases {
    offer: usize,
    session_control: usize,
    request: usize,
    payment: usize,
    credited_receive: usize,
    credited_status: usize,
    certificates: usize,
    largest_payment_proof: usize,
    largest_receive_proof: usize,
    largest_status_transition_proof: usize,
    largest_status_proof: usize,
}

/// Validate every worst-case envelope at the proof caps and measure it.
fn measure(f: &MessageFixture) -> WorstCases {
    let proof_max = KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1;
    let status_max = KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1;
    let message_max = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;

    let payment = f.payment(true, proof_max);
    assert_eq!(
        payment.request.certificates.len() + payment.certificates.len(),
        KAGEMUSHA_WALLET_CERTIFICATE_SET_MAX_V1,
        "the worst Payment carries three certificates"
    );
    assert!(payment.request.fee_schedule.schedule().is_some());
    let credited_receive = f.credited_receive(&payment, proof_max);
    let credited_status = f.credited_status(&payment, proof_max, status_max);
    let KagemushaWalletCreditedEvidenceV1::Status { current, .. } = &credited_status.evidence
    else {
        panic!("status evidence");
    };
    assert_eq!(
        current.statement.effect.kind(),
        KagemushaWalletOperationKindV1::Send,
        "the current package carries the largest effect"
    );
    let certificates = KagemushaWalletPolicyDataV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: f.scheme_id(),
        asset_digest: f.asset_digest(),
        item: KagemushaWalletPolicyDataItemV1::Certificates {
            certificates: KagemushaWalletCertificateSetV1::new(vec![
                f.payer.enrollment_certificate,
                f.receiver.enrollment_certificate,
                f.regulator_certificate,
            ])
            .expect("three certificates"),
        },
    };

    let messages = [
        KagemushaWalletMessageV1::Offer {
            offer: vector_offer(f),
        },
        KagemushaWalletMessageV1::SessionControl {
            control: vector_control(f, KagemushaWalletSessionControlKindV1::ReceiveDeferred),
        },
        KagemushaWalletMessageV1::Request {
            request: payment.request.clone(),
        },
        KagemushaWalletMessageV1::Payment {
            payment: payment.clone(),
        },
        KagemushaWalletMessageV1::Credited {
            credited: credited_receive.clone(),
        },
        KagemushaWalletMessageV1::Credited {
            credited: credited_status.clone(),
        },
        KagemushaWalletMessageV1::PolicyData { data: certificates },
    ];
    // Every worst case is a fully valid envelope within its kind's bound.
    let lengths: Vec<usize> = messages
        .iter()
        .map(|message| {
            let frame = KagemushaWalletEnvelopeV1::new(message.clone())
                .to_canonical_bytes()
                .expect("valid worst-case envelope");
            assert_eq!(frame.len(), envelope_len(message));
            frame.len()
        })
        .collect();

    let payment_len = |len: usize| {
        let mut payment = payment.clone();
        payment.send.proof = proof_of(len);
        envelope_len(&KagemushaWalletMessageV1::Payment { payment })
    };
    let receive_len = |len: usize| envelope_len(&credited_with(&credited_receive, len, 0));
    let status_transition_len =
        |len: usize| envelope_len(&credited_with(&credited_status, len, status_max));
    let status_len = |len: usize| envelope_len(&credited_with(&credited_status, proof_max, len));
    assert_eq!(payment_len(proof_max), lengths[3]);
    assert_eq!(receive_len(proof_max), lengths[4]);
    assert_eq!(status_len(status_max), lengths[5]);

    WorstCases {
        offer: lengths[0],
        session_control: lengths[1],
        request: lengths[2],
        payment: lengths[3],
        credited_receive: lengths[4],
        credited_status: lengths[5],
        certificates: lengths[6],
        largest_payment_proof: largest_fitting(message_max, SEARCH_LIMIT, payment_len),
        largest_receive_proof: largest_fitting(message_max, SEARCH_LIMIT, receive_len),
        largest_status_transition_proof: largest_fitting(
            message_max,
            SEARCH_LIMIT,
            status_transition_len,
        ),
        largest_status_proof: largest_fitting(message_max, SEARCH_LIMIT, status_len),
    }
}

#[test]
fn kagemusha_wallet_v1_worst_case_envelopes_fit_their_bounds() {
    let sizes = measure(&vector_world().f);
    let session_max = KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1;
    let message_max = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;
    let proof_max = KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1;
    let status_max = KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1;
    println!(
        "KAGEMUSHA wallet V1 worst-case envelopes (bytes): Offer {} / {session_max}, \
         SessionControl {} / {session_max}, Request {} / {message_max}, \
         Payment {} / {message_max}, Credited::Receive {} / {message_max}, \
         Credited::Status {} / {message_max}, PolicyData certificates (3) {} / {message_max}",
        sizes.offer,
        sizes.session_control,
        sizes.request,
        sizes.payment,
        sizes.credited_receive,
        sizes.credited_status,
        sizes.certificates,
    );
    println!(
        "KAGEMUSHA wallet V1 largest fitting proofs (bytes): Payment transition {} (cap \
         {proof_max}), Credited::Receive transition {} (cap {proof_max}), Credited::Status \
         transition {} with a {status_max}-byte status proof, Credited::Status status proof {} \
         with a {proof_max}-byte transition proof (combined {})",
        sizes.largest_payment_proof,
        sizes.largest_receive_proof,
        sizes.largest_status_transition_proof,
        sizes.largest_status_proof,
        proof_max + sizes.largest_status_proof,
    );

    for (name, len, bound) in [
        ("Offer", sizes.offer, session_max),
        ("SessionControl", sizes.session_control, session_max),
        ("Request", sizes.request, message_max),
        ("Payment", sizes.payment, message_max),
        ("Credited::Receive", sizes.credited_receive, message_max),
        ("Credited::Status", sizes.credited_status, message_max),
        ("PolicyData certificates", sizes.certificates, message_max),
    ] {
        assert!(len <= bound, "{name}: {len} > {bound}");
    }
    assert!(sizes.largest_payment_proof >= proof_max);
    assert!(sizes.largest_receive_proof >= proof_max);
    assert!(sizes.largest_status_transition_proof >= proof_max);
    assert!(sizes.largest_status_proof >= status_max);
}

#[test]
fn kagemusha_wallet_v1_size_search_helpers() {
    assert_eq!(largest_fitting(10, 100, |len| len), 10);
    assert_eq!(largest_fitting(10, 5, |len| len), 5);
    assert_eq!(largest_fitting(10, 100, |len| len + 11), 0);
    assert_eq!(largest_fitting(100, 1_000, |len| 3 * len + 1), 33);
    assert_eq!(proof_of(5).bytes, vec![0xa5; 5]);

    let f = &vector_world().f;
    let payment = f.payment(false, 32);
    let message = KagemushaWalletMessageV1::Payment {
        payment: payment.clone(),
    };
    let frame = KagemushaWalletEnvelopeV1::new(message.clone())
        .to_canonical_bytes()
        .expect("frame");
    assert_eq!(envelope_len(&message), frame.len());

    let credited = f.credited_status(&payment, 32, 16);
    let KagemushaWalletMessageV1::Credited { credited: resized } = credited_with(&credited, 40, 20)
    else {
        panic!("credited");
    };
    let KagemushaWalletCreditedEvidenceV1::Status { current, status } = resized.evidence else {
        panic!("status");
    };
    assert_eq!(
        (current.proof.bytes.len(), status.proof.bytes.len()),
        (40, 20)
    );
    let one_byte_more = envelope_len(&credited_with(&credited, 33, 16));
    assert_eq!(
        one_byte_more,
        envelope_len(&KagemushaWalletMessageV1::Credited { credited }) + 1
    );
}
