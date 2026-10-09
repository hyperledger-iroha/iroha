//! Terminal binding tests. Proposed endpoints never replace either source proof.

use super::*;
use crate::finality::{
    load_source::prepare_load_source, schedule::source::prepare_schedule_source,
};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_gadgets::bytes::tape::{BytesChip, BytesConfig};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct Link {
    anchor: HistoryAnchor,
    input: ReceiptFinalityInput,
    sources: [[Fp; 6]; 2],
    known: bool,
}
impl Link {
    fn value<T: Copy>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
        } else {
            Value::unknown()
        }
    }
    fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl Circuit<Fp> for Link {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).unwrap();
        let first = meta.advice_column();
        let second = meta.advice_column();
        let bytes = BytesConfig::configure(meta, first, second);
        let public = meta.instance_column(2);
        meta.enable_equality(public);
        Config {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let mut bytes = BytesChip::new(config.bytes);
        bytes.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "exact terminal receipt linkage",
            |mut region| {
                let sources = [
                    self.words(&mut chip, &mut region, self.sources[0])?,
                    self.words(&mut chip, &mut region, self.sources[1])?,
                ];
                let sources = [
                    SourceEndpoints::from_words(&mut chip, &mut region, &sources[0])?,
                    SourceEndpoints::from_words(&mut chip, &mut region, &sources[1])?,
                ];
                let anchor = chip
                    .uint()
                    .glue()
                    .constant(&mut region, self.anchor.digest())?;
                let state = HistoryStateCells::assign(
                    &mut chip,
                    &mut region,
                    &self.value(self.input.terminal),
                    &anchor,
                )?;
                let input = self.input.receipt.map(|b| self.value(b));
                let run = bytes.run(
                    &mut region,
                    &input,
                    &LoadReceiptCells::primary_segments(),
                    &LoadReceiptCells::secondary_segments(),
                )?;
                let (mut uint, hash) = chip.uint_and_hasher()?;
                let receipt = LoadReceiptCells::from_run(&mut uint, hash, &mut region, &run)?;
                let [root] = self.words(&mut chip, &mut region, [self.input.load.result_root])?;
                let length = chip.uint().assign::<32>(
                    &mut region,
                    self.value(u128::from(self.input.load.result_frame_len)),
                )?;
                let tape = ResultTape::new(&mut chip.uint(), &mut region, &root, &length)?;
                let event_root = self.words(
                    &mut chip,
                    &mut region,
                    self.input.load.event_root.map(|b| Fp::from(u64::from(b))),
                )?;
                let count = chip.uint().assign::<64>(
                    &mut region,
                    self.value(u128::from(self.input.load.event_count)),
                )?;
                let index = chip.uint().assign::<32>(
                    &mut region,
                    self.value(u128::from(self.input.load.event_index)),
                )?;
                let history_key = chip.uint().glue().constant(&mut region, Fp::from(77))?;
                let linked = ReceiptFinalityLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &anchor,
                    &history_key,
                    [&sources[0], &sources[1]],
                    &state,
                    ReceiptInclusionCells {
                        tape: &tape,
                        receipt: &receipt,
                        event_root: &event_root,
                        count: &count,
                        index: &index,
                    },
                )?;
                Ok([linked.digest().clone(), linked.receipt_digest().clone()])
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn fixture() -> Link {
    let json: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let bytes = |name: &str| hex(json.get(name).unwrap().as_str().unwrap());
    let frame = bytes("result_preimage_hex");
    let receipt = bytes("receipt_transcript_hex").try_into().unwrap();
    let message = bytes("commit_vote_preimage_hex");
    let id = core::array::from_fn(|i| message[53 + i]);
    let parser = prepare_schedule_source(frame.clone(), false, id).unwrap();
    let schedule = parser[0].input();
    // This is a fixed policy proposal for a link-only test. No signature on
    // genesis, complete history proof, or monetary authority is claimed here.
    let anchor = HistoryAnchor {
        network: schedule.projection.network,
        instance: core::array::from_fn(|i| message[13 + i]),
        initial_context: id,
        initial_epoch: schedule.projection.epoch,
        parameters: schedule.projection.slots[0].parameters,
    };
    let event_root = core::array::from_fn(|i| frame[220 + i]);
    let leaves = prepare_load_source(&frame, &receipt, event_root, 1, 0, &[[0; 32]; 32]).unwrap();
    let load = *leaves[0].context();
    let mut terminal = HistoryState::genesis(&anchor);
    terminal.next_height = load.receipt.height + 1;
    terminal.result = core::array::from_fn(|i| message[133 + i]);
    terminal.tape_root = load.result_root;
    terminal.frame_len = load.result_frame_len;
    let input = ReceiptFinalityInput {
        terminal,
        load,
        receipt,
    };
    Link {
        sources: input.source_endpoints(&anchor, Fp::from(77)),
        anchor,
        input,
        known: true,
    }
}
fn public(circuit: &Link) -> Vec<Vec<Fp>> {
    vec![vec![
        circuit.input.digest(&circuit.anchor),
        circuit.input.receipt_digest(),
    ]]
}
fn accepts(circuit: &Link) -> bool {
    check_circuit(circuit, 16, &public(circuit), CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
}

#[test]
fn native_receipt_digest_matches_both_terminal_sources_and_fixed_layout() {
    let honest = fixture();
    assert!(accepts(&honest));
    assert_eq!(
        honest.input.receipt_digest(),
        honest.input.load.receipt.digest
    );
    let known = synthesize(&honest, 16, None).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}

#[test]
fn complete_history_and_all_load_stages_are_mandatory() {
    let honest = fixture();
    for side in 0..2 {
        for field in 0..6 {
            let mut changed = honest.clone();
            changed.sources[side][field] += Fp::ONE;
            assert!(!accepts(&changed), "source {side} endpoint {field}");
        }
    }
}

#[test]
fn terminal_height_tape_and_nonempty_result_cannot_be_substituted() {
    let honest = fixture();
    for mutation in 0..8 {
        let mut changed = honest.clone();
        match mutation {
            0 => changed.input.terminal.next_height += 1,
            1 => changed.input.terminal.tape_root += Fp::ONE,
            2 => changed.input.terminal.frame_len += 1,
            3 => changed.input.terminal = HistoryState::genesis(&changed.anchor),
            4 => {
                changed.input.terminal.frame_len = 0;
                changed.input.load.result_frame_len = 0;
            }
            5 => changed.input.load.event_count = 0,
            6 => changed.input.load.event_index = 1,
            _ => {
                changed.input.receipt[242..250].copy_from_slice(&u64::MAX.to_le_bytes());
                changed.input.load.receipt.height = u64::MAX;
                changed.input.load.receipt.digest = changed.input.receipt_digest();
                changed.input.terminal.next_height = u64::MAX;
            }
        }
        changed.sources = changed
            .input
            .source_endpoints(&changed.anchor, Fp::from(77));
        assert!(!accepts(&changed), "terminal geometry {mutation}");
    }
}

#[test]
fn every_original_receipt_term_is_bound_to_the_complete_load_source() {
    let honest = fixture();
    for offset in [0, 2, 34, 66, 98, 130, 146, 162, 178, 210, 242, 250] {
        let mut changed = honest.clone();
        changed.input.receipt[offset] ^= 1;
        assert!(!accepts(&changed), "receipt field at {offset}");
    }
    let mut changed = honest.clone();
    changed.input.load.receipt.amount += 1;
    changed.sources = changed
        .input
        .source_endpoints(&changed.anchor, Fp::from(77));
    assert!(
        !accepts(&changed),
        "a host receipt projection is not the original tape"
    );
    let mut changed = honest.clone();
    changed.input.load.event_root[0] ^= 1;
    assert!(
        !accepts(&changed),
        "changed event root cannot reuse the complete child"
    );
}

#[test]
fn selected_anchor_and_complete_terminal_state_are_immutable() {
    let honest = fixture();
    for mutation in 0..4 {
        let mut changed = honest.clone();
        match mutation {
            0 => changed.anchor.network[0] ^= 1,
            1 => changed.input.terminal.current.epoch += 1,
            2 => changed.input.terminal.following.parameters[0] += 1,
            _ => changed.input.terminal.result[0] ^= 1,
        }
        assert!(!accepts(&changed), "complete history state {mutation}");
    }
    let mut changed = honest.clone();
    changed.anchor.parameters[0] += 1;
    changed.sources = changed
        .input
        .source_endpoints(&changed.anchor, Fp::from(77));
    assert_ne!(
        changed.input.digest(&changed.anchor),
        honest.input.digest(&honest.anchor)
    );
    assert!(
        !check_circuit(&changed, 16, &public(&honest), CheckMode::Strict)
            .is_ok_and(|r| r.is_satisfied()),
        "a different installed root cannot retain the terminal identity"
    );
}
