//! One exact fixed leaf/class schedule shared by installation and offline compilation.

use super::{Error, Program, circuit};
use crate::finality::{
    aggregate::{self, AggregateLeafCircuit},
    bls::{BlsBatchCircuit, BlsBatchPlan},
    load_source::{self, LoadSourceCircuit, LoadSourcePlan},
    result_scan::{self, ResultScanBatchCircuit, ResultScanBatchPlan},
    schedule::{
        context_hash::{self, ContextHashBatchCircuit, ContextHashBatchPlan},
        source::{self, ScheduleSourceCircuit, ScheduleSourceStage},
    },
};

// Installed proof leaves, not semantic instruction endpoints. In particular,
// Context has 1,281 batches ending at 2,561; BLS has 542 pairs ending at 1,084;
// Result has 258 batches ending at 515.
pub(in crate::finality) const fn length(program: Program) -> u32 {
    match program {
        Program::Bls => BlsBatchPlan::LENGTH,
        Program::Aggregation => aggregate::PROGRAM_LENGTH,
        Program::Result => result_scan::RESULT_BATCH_LENGTH,
        Program::Schedule => source::PROGRAM_LENGTH,
        Program::Context => context_hash::BATCH_LENGTH,
        Program::Load => load_source::PROGRAM_LENGTH,
    }
}

fn bounded(program: Program, position: u32) -> Result<(), Error> {
    if position < length(program) {
        Ok(())
    } else {
        Err(Error::Artifact)
    }
}

pub(in crate::finality) fn bls(i: u32) -> Result<(u32, BlsBatchCircuit), Error> {
    bounded(Program::Bls, i)?;
    Ok((
        i,
        circuit(BlsBatchCircuit::for_source(
            BlsBatchPlan::at(i).ok_or(Error::Artifact)?,
        ))?,
    ))
}

pub(in crate::finality) fn aggregation(i: u32) -> Result<(u32, AggregateLeafCircuit), Error> {
    bounded(Program::Aggregation, i)?;
    Ok((i, circuit(AggregateLeafCircuit::for_source(i))?))
}

pub(in crate::finality) fn result(i: u32) -> Result<(u32, ResultScanBatchCircuit), Error> {
    bounded(Program::Result, i)?;
    let plan = ResultScanBatchPlan::at(i).ok_or(Error::Artifact)?;
    let class = match plan {
        ResultScanBatchPlan::StartAbsorb => 0,
        ResultScanBatchPlan::AbsorbPair => 1,
        ResultScanBatchPlan::Finish => length(Program::Result) - 1,
    };
    Ok((class, circuit(ResultScanBatchCircuit::for_source(plan))?))
}

pub(in crate::finality) fn schedule(i: u32) -> Result<(u32, ScheduleSourceCircuit), Error> {
    bounded(Program::Schedule, i)?;
    Ok((
        if (10..=40).contains(&i) { 10 } else { i },
        ScheduleSourceCircuit::for_source(ScheduleSourceStage::at(i).ok_or(Error::Artifact)?),
    ))
}

pub(in crate::finality) fn context(i: u32) -> Result<(u32, ContextHashBatchCircuit), Error> {
    bounded(Program::Context, i)?;
    let plan = ContextHashBatchPlan::at(i).ok_or(Error::Artifact)?;
    let class = match plan {
        ContextHashBatchPlan::CrcPair => 0,
        ContextHashBatchPlan::BlakePair => context_hash::CRC_LEAVES / 2,
        ContextHashBatchPlan::BlakeTail => context_hash::BATCH_LENGTH - 1,
    };
    Ok((class, ContextHashBatchCircuit::for_source(plan)))
}

pub(in crate::finality) fn load(i: u32) -> Result<(u32, LoadSourceCircuit), Error> {
    bounded(Program::Load, i)?;
    Ok((
        if (1..=32).contains(&i) { 1 } else { i },
        circuit(LoadSourceCircuit::for_source(match i {
            0 => LoadSourcePlan::Start,
            1..=32 => LoadSourcePlan::Path,
            33 => LoadSourcePlan::Prefix,
            34 => LoadSourcePlan::Finish,
            _ => return Err(Error::Artifact),
        }))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capacity<C: iroha_plonk::frontend::Circuit<iroha_pasta::Fp>>(
        program: Program,
        factory: fn(u32) -> Result<(u32, C), Error>,
    ) -> bool {
        use iroha_plonk::frontend::synthesize;
        let mut classes = std::collections::BTreeSet::new();
        let mut failures = 0;
        let mut maximum = (0, 0);
        for position in 0..length(program) {
            let (class, source) = factory(position).unwrap();
            if !classes.insert(class) {
                continue;
            }
            match synthesize(&source, 16, None) {
                Ok(layout) => {
                    let rows = layout
                        .tables
                        .advice_assigned()
                        .iter()
                        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
                        .map(|row| row + 1)
                        .max()
                        .unwrap_or(0);
                    if rows > maximum.1 {
                        maximum = (class, rows);
                    }
                }
                Err(error) => {
                    failures += 1;
                    eprintln!(
                        "FINALITY_SOURCE_CAPACITY_FAILURE program={program:?} class={class} error={error} proof=false"
                    );
                }
            }
        }
        eprintln!(
            "FINALITY_SOURCE_CAPACITY program={program:?} classes={} failures={failures} largest_class={} maximum_advice_rows={} proof=false original_import=false",
            classes.len(),
            maximum.0,
            maximum.1
        );
        failures == 0
    }

    #[test]
    #[ignore = "synthesis of every unique source class, including all 542 BLS pairs; no proofs"]
    fn every_fixed_program_source_class_fits_k16() {
        let results = [
            capacity(Program::Aggregation, aggregation),
            capacity(Program::Bls, bls),
            capacity(Program::Result, result),
            capacity(Program::Schedule, schedule),
            capacity(Program::Context, context),
            capacity(Program::Load, load),
        ];
        assert!(results.into_iter().all(|fits| fits));
    }

    fn check<C>(program: Program, layout: fn(u32) -> Result<(u32, C), Error>) {
        let count = length(program);
        assert!((2..=4096).contains(&count));
        for i in 0..count {
            let (class, _) = layout(i).unwrap();
            assert!(class <= i);
            assert_eq!(layout(class).unwrap().0, class);
        }
        assert!(layout(count).is_err());
        assert!(layout(u32::MAX).is_err());
    }

    fn invariant<C: iroha_plonk::frontend::Circuit<iroha_pasta::Fp>>(
        program: Program,
        factory: fn(u32) -> Result<(u32, C), Error>,
    ) {
        use iroha_plonk::frontend::synthesize;
        let mut classes = std::collections::BTreeMap::<u32, Vec<u32>>::new();
        for i in 0..length(program) {
            classes.entry(factory(i).unwrap().0).or_default().push(i);
        }
        for (class, positions) in classes {
            if positions.len() == 1 {
                assert_eq!(positions, [class], "unique source owns its exact cursor");
                continue;
            }
            let (_, source) = factory(class).unwrap();
            let expected = synthesize(&source, 16, None).unwrap();
            for position in [
                positions[0],
                positions[positions.len() / 2],
                *positions.last().unwrap(),
            ] {
                let (_, source) = factory(position).unwrap();
                let actual = synthesize(&source, 16, None).unwrap();
                assert_eq!(
                    actual.cs, expected.cs,
                    "{program:?} class {class} position {position}"
                );
                assert_eq!(actual.tables.fixed(), expected.tables.fixed());
                assert_eq!(actual.tables.selectors(), expected.tables.selectors());
                assert_eq!(actual.tables.permutation(), expected.tables.permutation());
                assert_eq!(
                    actual.tables.advice_assigned(),
                    expected.tables.advice_assigned()
                );
            }
            eprintln!(
                "FINALITY_SOURCE_CLASS program={program:?} class={class} covered_positions={} layout_samples=first_middle_last",
                positions.len()
            );
        }
    }

    #[test]
    #[ignore = "actual k16 layout equality at every shared class boundary and midpoint"]
    fn all_shared_program_classes_preserve_exact_original_layouts() {
        invariant(Program::Bls, bls);
        invariant(Program::Aggregation, aggregation);
        invariant(Program::Result, result);
        invariant(Program::Schedule, schedule);
        invariant(Program::Context, context);
        invariant(Program::Load, load);
    }

    #[test]
    fn minimal_receipt_graph_counts_every_source_and_wrapper_proof() {
        // InstalledProgram produces one source and one wrapper per leaf and
        // per binary merge. An n-leaf tree has exactly n-1 merges, independent
        // of the semantic instruction width of each leaf.
        let program = |kind| 4 * length(kind) - 2;
        let context = program(Program::Context);
        assert_eq!(context, 5_122);
        // Session::genesis, certificate, certified result, both Schedule
        // compositions, ScheduledResult, HistoryStep, history Append and
        // terminal receipt each produce one source plus one wrapper.
        let fixed_compositions = 2 * (1 + 1 + 1 + 2 + 1 + 1 + 1 + 1);
        let total = program(Program::Bls)
            + program(Program::Aggregation)
            + program(Program::Result)
            + 2 * program(Program::Schedule)
            + 2 * context
            + program(Program::Load)
            + fixed_compositions;
        assert_eq!(fixed_compositions, 18);
        assert_eq!(total, 14_066);
        assert_eq!(total - context, 8_944);
    }

    #[test]
    fn all_six_complete_programs_have_exact_bounded_fixed_classes() {
        check(Program::Bls, bls);
        check(Program::Aggregation, aggregation);
        check(Program::Result, result);
        check(Program::Schedule, schedule);
        check(Program::Context, context);
        check(Program::Load, load);
        assert_eq!(
            result(1).unwrap().0,
            result(length(Program::Result) - 2).unwrap().0
        );
        assert_ne!(result(0).unwrap().0, result(1).unwrap().0);
        assert_eq!(load(1).unwrap().0, load(32).unwrap().0);
        assert_ne!(load(32).unwrap().0, load(33).unwrap().0);
        assert_eq!(schedule(10).unwrap().0, schedule(40).unwrap().0);
        assert_eq!(length(Program::Result), 258);
        assert_eq!(result_scan::RESULT_SCAN_LEAVES, 515);
        assert_eq!(length(Program::Context), 1_281);
        assert_eq!(context_hash::PROGRAM_LENGTH, 2_561);
        assert_ne!(
            context(context_hash::BATCH_LENGTH - 2).unwrap().0,
            context(context_hash::BATCH_LENGTH - 1).unwrap().0
        );
        assert_ne!(
            context(context_hash::CRC_LEAVES / 2 - 1).unwrap().0,
            context(context_hash::CRC_LEAVES / 2).unwrap().0
        );
    }
}
