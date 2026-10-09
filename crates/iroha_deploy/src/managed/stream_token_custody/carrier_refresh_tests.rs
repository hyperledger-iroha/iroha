//! Post-Applied decision coverage with the existing real native Configure H2-to-H3 fixture.
//! This exercises the shared decision and native proof owners, not coordinator HTTP integration.

use super::*;
use crate::{
    managed::native_operation::retain_carrier_progress,
    verify::finality::{FinalityAttestation, FinalitySource},
};
use iroha_data_model::sumeragi_finality::SumeragiFinalityProof;
use iroha_model_base::peer::PeerId;
use iroha_wallet::operations::OperationReport;
use std::{cell::Cell, io};

struct UnavailableProof;
impl FinalitySource for UnavailableProof {
    type Error = io::Error;

    fn finality_proof(&self, _: NonZeroU64) -> io::Result<SumeragiFinalityProof> {
        Err(io::Error::other("native proof unavailable"))
    }

    fn latest_attestation(&self, _: &PeerId, _: &[u8; 32]) -> io::Result<FinalityAttestation> {
        unreachable!("carrier replay fetches proofs, never node-status substitutes")
    }
}

pub(super) fn retain_after_applied(
    owner: &mut ManagedStreamTokenCustody,
    original: &Selected<Original>,
    signed: &SignedTransaction,
    native: &NativeFixture,
    before: &FinalityVerifier,
    deadline: Instant,
    expected: ManagedTransactionFinality,
) {
    let directory = original.directory();
    let authorization = directory.read("authorization.nrt", 64 * 1024).unwrap();
    let wire = signed.encode_wire_v1().unwrap();
    assert_eq!(before.checkpoint().height(), 2);
    assert_eq!(native.chain.height(), 3);
    assert_eq!(expected.height, 3);
    assert!(!directory.path().join("carrier.nrt").exists());
    // The hint comes from the fixture's actual successful native commit. As in production,
    // Applied is only a replay lookup hint and cannot supply successful inclusion itself.
    let report = OperationReport {
        status: OperationStatus::Applied,
        data: norito::json!({ "evidence": { "block_height": (native.chain.height()) } }),
    };
    // This is exactly the old production call: its pre-submission H2 frontier refuses H3
    // before reading any source, so repeating that bound cannot retain this carrier.
    assert!(
        owner
            .authority
            .advance_carrier(
                directory,
                &original.checkpoint,
                signed,
                &report,
                before.checkpoint().height(),
                deadline,
            )
            .unwrap()
            .is_none()
    );

    assert!(
        applied_carrier(
            &mut owner.authority,
            OperationStatus::Pending,
            deadline,
            |_, _| panic!("pending is not an Applied carrier request"),
            |_, _, _| panic!("pending cannot replay"),
        )
        .unwrap()
        .is_none()
    );
    assert!(
        applied_carrier(
            &mut owner.authority,
            report.status,
            deadline,
            |_, passed| {
                assert_eq!(passed, deadline);
                Err(invalid("fresh finality unavailable"))
            },
            |_, _, _| panic!("an Applied hint cannot replace a fresh observation"),
        )
        .unwrap()
        .is_none()
    );
    // The real observation owner rejects the original expired deadline before source access.
    let elapsed = Instant::now() - Duration::from_secs(1);
    assert!(
        applied_carrier(
            &mut owner.authority,
            report.status,
            elapsed,
            ServiceAuthority::observe_finality,
            |_, _, _| panic!("an elapsed original budget cannot replay"),
        )
        .unwrap()
        .is_none()
    );
    assert!(!directory.path().join("carrier.nrt").exists());

    let observations = Cell::new(0);
    let fresh = |authority: &mut ServiceAuthority, passed: Instant| {
        assert_eq!(passed, deadline, "no replacement operation budget");
        require_deadline(passed).unwrap();
        observations.set(observations.get() + 1);
        // Fresh independent node signatures, full native prefix verification and exact tip;
        // this never promotes the wallet's Applied report into authenticated finality.
        Ok(native.observe(authority))
    };
    assert!(
        applied_carrier(
            &mut owner.authority,
            report.status,
            deadline,
            fresh,
            |authority, observed, passed| {
                assert_eq!(observed, 3);
                assert_eq!(passed, deadline);
                let mut replay = authority.decode_checkpoint(&original.checkpoint)?;
                retain_carrier_progress(directory, signed, &mut replay, observed, &UnavailableProof)
            },
        )
        .is_err()
    );
    assert!(!directory.path().join("carrier.nrt").exists());
    // Even genuine fresh H3 finality cannot retain a different signed envelope as included.
    let other = quote_instructions(
        native,
        &owner.authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "not the retained custody operation".into(),
        ))],
    );
    assert_ne!(other.hash(), signed.hash());
    assert!(
        applied_carrier(
            &mut owner.authority,
            report.status,
            deadline,
            fresh,
            |authority, observed, passed| {
                assert_eq!(passed, deadline);
                let mut replay = authority.decode_checkpoint(&original.checkpoint)?;
                retain_carrier_progress(directory, &other, &mut replay, observed, native)
            },
        )
        .is_err()
    );
    assert!(!directory.path().join("carrier.nrt").exists());

    let prior_observations = observations.get();
    let finalized = applied_carrier(
        &mut owner.authority,
        report.status,
        deadline,
        fresh,
        |authority, observed, passed| {
            assert_eq!(
                observed, 3,
                "replay must use the refreshed frontier, not H2"
            );
            assert_eq!(passed, deadline);
            let mut replay = authority.decode_checkpoint(&original.checkpoint)?;
            retain_carrier_progress(directory, signed, &mut replay, observed, native)
        },
    )
    .unwrap();
    assert_eq!(observations.get(), prior_observations + 1);
    assert_eq!(
        finalized,
        Some(expected),
        "the same post-Applied decision must retain the exact native carrier"
    );
    assert_eq!(
        owner
            .authority
            .retained_finality(directory, signed)
            .unwrap(),
        Some(expected)
    );
    assert_eq!(
        directory.read("authorization.nrt", 64 * 1024).unwrap(),
        authorization
    );
    assert_eq!(signed.encode_wire_v1().unwrap(), wire);
    assert!(
        !directory
            .path()
            .join("transaction/submission.json")
            .exists(),
        "read-only replay does not acquire another dispatch authorization"
    );
}
