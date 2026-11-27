//! Recursive public evidence schemas retain every lifecycle phase and fixed cryptographic extent.

use crate::{evidence::*, tle::*};
use iroha_crypto::timed_ovn::{G2Bytes, GtBytes};
use iroha_schema::{IntoSchema, Metadata};

#[test]
fn lifecycle_schema_contains_every_persisted_phase_and_cryptographic_extent() {
    let schema = TimedOvnLifecycleStateV1::schema();
    let Some(Metadata::Enum(phases)) = schema.get::<TimedOvnLifecycleStateV1>() else {
        panic!("the persisted lifecycle has a complete enum schema")
    };
    assert_eq!(
        phases
            .variants
            .iter()
            .map(|phase| (phase.tag.as_str(), phase.discriminant))
            .collect::<Vec<_>>(),
        [
            ("Registered", 0),
            ("RegistrationClosed", 1),
            ("SurvivorsFrozen", 2),
            ("CorpusOpen", 3),
            ("Sealed", 4),
            ("Released", 5),
        ],
    );
    macro_rules! retained {
        ($($ty:ty),+ $(,)?) => {$(assert!(schema.contains_key::<$ty>(), stringify!($ty));)+};
    }
    retained!(
        TimedOvnRegistrationOpenStateV1,
        TimedOvnRegistrationClosedStateV1,
        TimedOvnSurvivorsFrozenStateV1,
        TimedOvnCorpusOpenStateV1,
        TimedOvnCorpusAccumulatorV1,
        TimedOvnEvidenceStateV1,
        TimedOvnReleasedEvidenceV1,
        TimedOvnSessionPublicV1,
        TimedOvnReleaseIdentityPublicV1,
        TimedOvnAggregateTranscriptV1,
        TimedOvnPublicTallyV1,
        TleFinalReleaseSignatureV1,
        iroha_data_model::parliament_casting::ParliamentTimedOvnRegistrationCorpusCommitmentV1,
    );
    let Some(Metadata::Array(g2)) = schema.get::<G2Bytes>() else {
        panic!("G2 byte extent is structural")
    };
    let Some(Metadata::Array(gt)) = schema.get::<GtBytes>() else {
        panic!("target-group byte extent is structural")
    };
    assert_eq!(g2.len, core::mem::size_of::<G2Bytes>() as u128);
    assert_eq!(gt.len, core::mem::size_of::<GtBytes>() as u128);
    assert_ne!(g2.len, gt.len);
}

#[test]
fn key_session_schema_keeps_dealer_proofs_shares_and_lifecycle_fields() {
    let public = TleKeySessionPublicStateV1::schema();
    assert!(public.contains_key::<TleAdaptiveDealerCommitmentV1>());
    assert!(public.contains_key::<TleAdaptivePublicShareV1>());
    let Some(Metadata::Struct(dealer)) = public.get::<TleAdaptiveDealerCommitmentV1>() else {
        panic!("public dealer proof has complete fields")
    };
    assert_eq!(
        dealer
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "dealer_index",
            "coefficient_commitments",
            "constant_pok_commitment",
            "constant_pok_response",
        ],
    );
    let lifecycle = TleKeySessionLifecycleV1::schema();
    let Some(Metadata::Struct(fields)) = lifecycle.get::<TleKeySessionLifecycleV1>() else {
        panic!("lifecycle bounds have complete fields")
    };
    assert!(fields.declarations.iter().any(|field| {
        field.name == "selection_closed_at_height"
            && field.ty == core::any::TypeId::of::<Option<u64>>()
    }));
    assert!(lifecycle.contains_key::<Option<u64>>());
}
