//! Envelope sizes of the split-lineage layouts (§8, design §7).
//!
//! Every case is built from fully valid objects and asserted against its complete-frame bound.
//! `σ_send` is measured only for the prototype relation (3,296 bytes); Ω, `σ_recv` and the
//! compressed credit-digest opening are unmeasured, so this module names explicit placeholders
//! and reports, for each message, its fixed overhead and the largest proof that still fits.
//! The proof bytes are stand-ins; only their lengths matter here. The signed blacklist is not
//! a peer message (it is downloaded online from the issuer), so it has no envelope case here.

use super::{
    messages::messages_tests::{MessageFixture, low_bitmap, stand_in_siblings},
    vectors_tests::{vector_control, vector_offer, vector_world},
    *,
};

/// Measured `σ_send` of the prototype relation, in bytes: the exact PIPA-v1 proof length of the
/// two-level `sigma_send` of `iroha_kagemusha_proof` at its 3.5 KB budget shape (`k = 12`, one
/// lane), with the G1 statement encoding. Spec §11: σ is not yet measured for the full §3 core.
const MEASURED_SIGMA_SEND_BYTES: usize = 3_296;
/// Placeholder Ω transport proof length; Ω is unbuilt and unmeasured (spec §11).
// TODO(G3): replace with the measured single-parity transport proof length (owner question Q6).
const PLACEHOLDER_OMEGA_PROOF_BYTES: usize = 4_000;
/// Placeholder `σ_recv` length; `σ_recv` is unmeasured for the §3 core.
// TODO(G3): replace with the measured σ_recv length.
const PLACEHOLDER_SIGMA_RECV_BYTES: usize = MEASURED_SIGMA_SEND_BYTES;
/// Placeholder count of non-default siblings in a compressed credit-digest opening.
// TODO(G3/owner Q7): replace with the bound of the frozen credit-digest tree.
const PLACEHOLDER_OPENING_SIBLINGS: usize = 32;
/// Largest proof length searched for.
const SEARCH_LIMIT: usize = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;

/// Complete canonical envelope frame length of `message`, without validation.
fn envelope_len(message: &KagemushaWalletMessageV1) -> usize {
    norito::encode_canonical(&KagemushaWalletEnvelopeV1::new(message.clone()))
        .expect("encode envelope")
        .len()
}

/// Stand-in proof bytes of exactly `len` bytes.
fn bytes_of(len: usize) -> Vec<u8> {
    vec![0xa5; len]
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

/// `payment` with its Ω(pred) transport proof and `σ_send` resized.
fn payment_with(
    payment: &KagemushaWalletPaymentV1,
    omega: usize,
    sigma: usize,
) -> KagemushaWalletMessageV1 {
    let mut payment = payment.clone();
    payment.send.step_proof.bytes = bytes_of(sigma);
    if let KagemushaWalletLineageSlotV1::Present { lineage } = &mut payment.send.lineage {
        lineage.proof = bytes_of(omega);
    }
    KagemushaWalletMessageV1::Payment { payment }
}

/// `credited` with its `σ_recv` (Receive) or Ω(h) proof and opening (Status) resized.
fn credited_with(
    credited: &KagemushaWalletCreditedV1,
    proof: usize,
    siblings: usize,
) -> KagemushaWalletMessageV1 {
    let mut credited = credited.clone();
    match &mut credited.evidence {
        KagemushaWalletCreditedEvidenceV1::Receive { package } => {
            package.step_proof.bytes = bytes_of(proof);
        }
        KagemushaWalletCreditedEvidenceV1::Status { status } => {
            status.lineage.proof = bytes_of(proof);
            status.opening.path_bitmap = low_bitmap(siblings);
            status.opening.siblings = stand_in_siblings(siblings);
        }
    }
    KagemushaWalletMessageV1::Credited { credited }
}

/// `lineage` with its transport proof resized.
fn lineage_with(
    lineage: &KagemushaWalletLineageMessageV1,
    omega: usize,
) -> KagemushaWalletMessageV1 {
    let mut lineage = lineage.clone();
    lineage.lineage.proof = bytes_of(omega);
    KagemushaWalletMessageV1::Lineage { lineage }
}

/// Measured split-lineage sizes in complete envelope bytes.
struct Sizes {
    offer: usize,
    credential: usize,
    session_control: usize,
    request: usize,
    payment: usize,
    payment_overhead: usize,
    largest_omega_with_measured_sigma: usize,
    largest_joint_proof: usize,
    credited_receive: usize,
    credited_receive_overhead: usize,
    credited_status: usize,
    credited_status_overhead: usize,
    credit_status: usize,
    per_sibling: usize,
    largest_omega_in_status: usize,
    lineage: usize,
    lineage_overhead: usize,
    certificates: usize,
}

/// Validate every case at the placeholder sizes and measure it.
fn measure(f: &MessageFixture) -> Sizes {
    let message_max = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;
    let (omega, sigma, sigma_recv, siblings) = (
        PLACEHOLDER_OMEGA_PROOF_BYTES,
        MEASURED_SIGMA_SEND_BYTES,
        PLACEHOLDER_SIGMA_RECV_BYTES,
        PLACEHOLDER_OPENING_SIBLINGS,
    );
    let request = f.request(true);
    let payment = f.payment_with(true, omega, sigma);
    assert!(
        payment.request.body.fee > 0,
        "the largest Request carries a fee"
    );
    assert_eq!(request.certificates.len(), 2);
    let credited_receive =
        KagemushaWalletCreditedV1::from_receive(f.receive_package(&payment, sigma_recv))
            .expect("credited receive");
    let status = f.credit_status(&payment, omega, siblings, false);
    let credited_status =
        KagemushaWalletCreditedV1::from_status(status.clone()).expect("credited status");
    let lineage = f.lineage_message(&payment);
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
    let offer = vector_offer(f);
    let messages = [
        KagemushaWalletMessageV1::Offer {
            offer: offer.clone(),
        },
        KagemushaWalletMessageV1::SessionControl {
            control: vector_control(f, KagemushaWalletSessionControlKindV1::ReceiveDeferred),
        },
        KagemushaWalletMessageV1::Request { request },
        KagemushaWalletMessageV1::Payment {
            payment: payment.clone(),
        },
        KagemushaWalletMessageV1::Credited {
            credited: credited_receive.clone(),
        },
        KagemushaWalletMessageV1::Credited {
            credited: credited_status.clone(),
        },
        KagemushaWalletMessageV1::Lineage {
            lineage: lineage.clone(),
        },
        KagemushaWalletMessageV1::PolicyData { data: certificates },
    ];
    // Every case is a fully valid envelope within its kind's bound.
    let lengths: Vec<usize> = messages
        .iter()
        .map(|message| {
            let frame = KagemushaWalletEnvelopeV1::new(message.clone())
                .to_canonical_bytes()
                .expect("valid envelope");
            assert_eq!(frame.len(), envelope_len(message));
            frame.len()
        })
        .collect();

    let payment_len =
        |omega: usize, sigma: usize| envelope_len(&payment_with(&payment, omega, sigma));
    let receive_len = |proof: usize| envelope_len(&credited_with(&credited_receive, proof, 0));
    let status_len = |proof: usize, siblings: usize| {
        envelope_len(&credited_with(&credited_status, proof, siblings))
    };
    let lineage_len = |omega: usize| envelope_len(&lineage_with(&lineage, omega));
    assert_eq!(payment_len(omega, sigma), lengths[3]);
    assert_eq!(receive_len(sigma_recv), lengths[4]);
    assert_eq!(status_len(omega, siblings), lengths[5]);
    assert_eq!(lineage_len(omega), lengths[6]);
    let per_sibling = status_len(omega, siblings + 1) - status_len(omega, siblings);

    Sizes {
        offer: lengths[0],
        credential: offer
            .payer_credential
            .to_canonical_bytes()
            .expect("credential frame")
            .len(),
        session_control: lengths[1],
        request: lengths[2],
        payment: lengths[3],
        payment_overhead: lengths[3] - omega - sigma,
        largest_omega_with_measured_sigma: largest_fitting(message_max, SEARCH_LIMIT, |omega| {
            payment_len(omega, sigma)
        }),
        largest_joint_proof: largest_fitting(message_max, SEARCH_LIMIT, |joint| {
            payment_len(joint / 2, joint - joint / 2)
        }),
        credited_receive: lengths[4],
        credited_receive_overhead: lengths[4] - sigma_recv,
        credited_status: lengths[5],
        credited_status_overhead: lengths[5] - omega - siblings * per_sibling,
        credit_status: norito::encode_canonical(&status)
            .expect("credit status frame")
            .len(),
        per_sibling,
        largest_omega_in_status: largest_fitting(message_max, SEARCH_LIMIT, |omega| {
            status_len(omega, siblings)
        }),
        lineage: lengths[6],
        lineage_overhead: lengths[6] - omega,
        certificates: lengths[7],
    }
}

#[test]
fn kagemusha_wallet_v1_split_lineage_envelopes_fit_their_bounds() {
    let sizes = measure(&vector_world().f);
    let session_max = KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1;
    let message_max = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;
    println!(
        "KAGEMUSHA wallet V1 split-lineage envelopes (bytes): Offer {} / {session_max} \
         (credential frame {} / {}), SessionControl {} / {session_max}, Request {} / \
         {message_max}, Payment {} / {message_max} with σ_send {MEASURED_SIGMA_SEND_BYTES} and \
         placeholder Ω proof {PLACEHOLDER_OMEGA_PROOF_BYTES}, Credited::Receive {} / \
         {message_max} with placeholder σ_recv {PLACEHOLDER_SIGMA_RECV_BYTES}, Credited::Status \
         {} / {message_max} with placeholder Ω(h) proof {PLACEHOLDER_OMEGA_PROOF_BYTES} and \
         {PLACEHOLDER_OPENING_SIBLINGS} siblings (CreditStatus frame {}), Lineage {} / \
         {message_max}, PolicyData certificates (3) {} / {message_max}",
        sizes.offer,
        sizes.credential,
        KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
        sizes.session_control,
        sizes.request,
        sizes.payment,
        sizes.credited_receive,
        sizes.credited_status,
        sizes.credit_status,
        sizes.lineage,
        sizes.certificates,
    );
    println!(
        "KAGEMUSHA wallet V1 fixed overheads (bytes): Payment {} + |Ω proof| + |σ_send|, \
         Credited::Receive {} + |σ_recv|, Credited::Status {} + |Ω(h) proof| + {} per sibling, \
         Lineage {} + |Ω proof|; largest Ω proof in a Payment with the measured σ_send {}, \
         largest |Ω proof| + |σ_send| {}, largest Ω(h) proof in Credited::Status with {} \
         siblings {}",
        sizes.payment_overhead,
        sizes.credited_receive_overhead,
        sizes.credited_status_overhead,
        sizes.per_sibling,
        sizes.lineage_overhead,
        sizes.largest_omega_with_measured_sigma,
        sizes.largest_joint_proof,
        PLACEHOLDER_OPENING_SIBLINGS,
        sizes.largest_omega_in_status,
    );
    for (name, len, bound) in [
        ("Offer", sizes.offer, session_max),
        (
            "Offer credential",
            sizes.credential,
            KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
        ),
        ("SessionControl", sizes.session_control, session_max),
        ("Request", sizes.request, message_max),
        ("Payment", sizes.payment, message_max),
        ("Credited::Receive", sizes.credited_receive, message_max),
        ("Credited::Status", sizes.credited_status, message_max),
        ("Lineage", sizes.lineage, message_max),
        ("PolicyData certificates", sizes.certificates, message_max),
    ] {
        assert!(len <= bound, "{name}: {len} > {bound}");
    }
    // A Lineage message is smaller than the Payment that carries the same Ω (§8).
    assert!(sizes.lineage < sizes.payment);
    assert!(sizes.largest_omega_with_measured_sigma >= PLACEHOLDER_OMEGA_PROOF_BYTES);
    assert!(sizes.largest_omega_in_status >= PLACEHOLDER_OMEGA_PROOF_BYTES);
    assert!(sizes.payment_overhead < 2_000, "{}", sizes.payment_overhead);
    assert_eq!(sizes.per_sibling, 32);
}

#[test]
fn kagemusha_wallet_v1_size_search_helpers() {
    assert_eq!(largest_fitting(10, 100, |len| len), 10);
    assert_eq!(largest_fitting(10, 5, |len| len), 5);
    assert_eq!(largest_fitting(10, 100, |len| len + 11), 0);
    assert_eq!(largest_fitting(100, 1_000, |len| 3 * len + 1), 33);
    assert_eq!(bytes_of(5), vec![0xa5; 5]);

    let f = &vector_world().f;
    let payment = f.payment(false, 32);
    let message = KagemushaWalletMessageV1::Payment {
        payment: payment.clone(),
    };
    let frame = KagemushaWalletEnvelopeV1::new(message.clone())
        .to_canonical_bytes()
        .expect("frame");
    assert_eq!(envelope_len(&message), frame.len());
    let KagemushaWalletMessageV1::Payment { payment: resized } = payment_with(&payment, 50, 40)
    else {
        panic!("payment");
    };
    assert_eq!(resized.send.step_proof.bytes.len(), 40);
    assert_eq!(resized.send.lineage.lineage().expect("Ω").proof.len(), 50);

    let credited = f.credited_status(&payment, 32, 2);
    let KagemushaWalletMessageV1::Credited { credited: resized } = credited_with(&credited, 40, 3)
    else {
        panic!("credited");
    };
    let KagemushaWalletCreditedEvidenceV1::Status { status } = resized.evidence else {
        panic!("status");
    };
    assert_eq!(
        (
            status.lineage.proof.len(),
            status.opening.sibling_values().count()
        ),
        (40, 3)
    );
    status.opening.validate().expect("resized opening");
    let one_byte_more = envelope_len(&credited_with(&credited, 33, 2));
    assert_eq!(
        one_byte_more,
        envelope_len(&credited_with(&credited, 32, 2)) + 1
    );
    let lineage = f.lineage_message(&payment);
    assert_eq!(
        envelope_len(&lineage_with(&lineage, 49)),
        envelope_len(&lineage_with(&lineage, 48)) + 1
    );
}
