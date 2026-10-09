//! Compiled leaf inventory metadata shared with the authenticated native profile.
//!
//! This transcript records source classes and semantic endpoints, not qualification.
//! Every concrete source, merge, composition and history key must still pass the
//! complete graph's original-artifact importer before proof production is admitted.

use super::{Error, Program, source_layout};
use crate::finality::{
    aggregate,
    bls::BlsLeafPlan,
    load_source, result_scan,
    schedule::{context_hash, source},
};

const PROGRAMS: [Program; 6] = [
    Program::Bls,
    Program::Aggregation,
    Program::Result,
    Program::Schedule,
    Program::Context,
    Program::Load,
];

#[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.finality.compiled_leaf_program.v1")]
struct Record {
    program: u64,
    semantic_end: u32,
    // One entry per installed proof leaf, named by its first equivalent position.
    classes: Vec<u32>,
}

#[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.finality.compiled_leaf_programs.v1")]
struct Policy {
    version: u16,
    programs: Vec<Record>,
}

fn record(kind: Program) -> Result<Record, Error> {
    let (program, semantic_end) = match kind {
        Program::Bls => (BlsLeafPlan::PROGRAM_ID, BlsLeafPlan::LENGTH),
        Program::Aggregation => (aggregate::PROGRAM_ID, aggregate::PROGRAM_LENGTH),
        Program::Result => (
            result_scan::RESULT_SCAN_PROGRAM,
            result_scan::RESULT_SCAN_LEAVES,
        ),
        Program::Schedule => (source::PROGRAM_ID, source::PROGRAM_LENGTH),
        Program::Context => (context_hash::PROGRAM_ID, context_hash::PROGRAM_LENGTH),
        Program::Load => (load_source::PROGRAM_ID, load_source::PROGRAM_LENGTH),
    };
    // Deliberately use the same factories as installation and offline compilation.
    // Constructing their witnessless sources is cheap and grants no VK authority;
    // a second class table could silently diverge from the concrete owners.
    let classes = (0..source_layout::length(kind))
        .map(|position| match kind {
            Program::Bls => source_layout::bls(position).map(|(class, _)| class),
            Program::Aggregation => source_layout::aggregation(position).map(|(class, _)| class),
            Program::Result => source_layout::result(position).map(|(class, _)| class),
            Program::Schedule => source_layout::schedule(position).map(|(class, _)| class),
            Program::Context => source_layout::context(position).map(|(class, _)| class),
            Program::Load => source_layout::load(position).map(|(class, _)| class),
        })
        .collect::<Result<_, _>>()?;
    Ok(Record {
        program,
        semantic_end,
        classes,
    })
}

/// Canonical metadata for all six compiled finality leaf programs, in fixed order.
/// Installed proof counts are the class-array lengths; semantic endpoints remain
/// distinct for batched programs. This does not encode the composition graph or
/// replace strict reconstruction of its exact sources and child-key identities.
/// # Errors
/// A compiled source position is invalid or canonical Norito encoding fails.
pub fn compiled_leaf_schedule_transcript() -> Result<Vec<u8>, Error> {
    let programs = PROGRAMS.into_iter().map(record).collect::<Result<_, _>>()?;
    norito::encode_canonical(&Policy {
        version: 1,
        programs,
    })
    .map_err(|_| Error::Artifact)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn canonical_policy_matches_all_actual_factories_and_semantic_endpoints() {
        let bytes = compiled_leaf_schedule_transcript().unwrap();
        let decoded: Policy = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.programs.len(), PROGRAMS.len());
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
        let expected = [
            (542, 1_084),
            (33, 33),
            (258, 515),
            (43, 43),
            (1_281, 2_561),
            (35, 35),
        ];
        let mut unique_classes = 0;
        for ((kind, actual), (installed, semantic)) in
            PROGRAMS.into_iter().zip(&decoded.programs).zip(expected)
        {
            assert_eq!(*actual, record(kind).unwrap());
            assert_eq!(actual.classes.len(), installed);
            assert_eq!(actual.semantic_end, semantic);
            for (position, class) in actual.classes.iter().copied().enumerate() {
                let class = usize::try_from(class).unwrap();
                assert!(class <= position);
                assert_eq!(usize::try_from(actual.classes[class]).unwrap(), class);
            }
            unique_classes += actual.classes.iter().collect::<BTreeSet<_>>().len();
        }
        assert_eq!(unique_classes, 598);
        assert!(bytes.len() < 16 * 1024);
    }

    #[test]
    fn installed_batch_counts_cannot_substitute_semantic_endpoints() {
        for kind in [Program::Bls, Program::Result, Program::Context] {
            let source = record(kind).unwrap();
            let exact = norito::encode_canonical(&source).unwrap();
            let mut changed = source;
            changed.semantic_end = u32::try_from(changed.classes.len()).unwrap();
            assert_ne!(norito::encode_canonical(&changed).unwrap(), exact);
            changed = record(kind).unwrap();
            // Mutating a class identity must change the signed transcript.
            changed.classes[0] = u32::MAX;
            assert_ne!(norito::encode_canonical(&changed).unwrap(), exact);
        }
    }
}
