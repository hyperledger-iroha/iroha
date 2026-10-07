//! Native schedule source intervals, bounded stage capacity and hostile openings.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes(j: &norito::json::Value, key: &str) -> Vec<u8> {
    hex(j.get(key).unwrap().as_str().unwrap())
}
fn fixtures() -> norito::json::Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_npos_schedule_v1.json");
    norito::json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn prepare(case: usize, block: usize, authorized: bool) -> Vec<ScheduleSourceCircuit> {
    let fixtures = fixtures();
    let case = &fixtures.get("cases").unwrap().as_array().unwrap()[case];
    let block = &case.get("blocks").unwrap().as_array().unwrap()[block];
    let schedule = block.get("schedule").unwrap();
    let boundary = schedule.get("boundary").unwrap();
    let target = if authorized && !matches!(boundary, norito::json::Value::Null) {
        boundary.get("successor").unwrap()
    } else {
        schedule.get("current").unwrap()
    };
    let id = bytes(target, "context_id_hex").try_into().unwrap();
    let leaves =
        prepare_schedule_source(bytes(block, "result_preimage_hex"), authorized, id).unwrap();
    let input = leaves[0].input();
    assert_eq!(input.epoch_hash.context_id, id);
    assert_eq!(
        u64::from(input.projection.members),
        case.get("seats").unwrap().as_u64().unwrap()
    );
    assert_eq!(
        input.projection.epoch,
        target.get("epoch").unwrap().as_u64().unwrap()
    );
    assert_eq!(
        input.projection.first,
        target.get("first_height").unwrap().as_u64().unwrap()
    );
    assert_eq!(
        input.projection.last,
        target.get("last_height").unwrap().as_u64().unwrap()
    );
    assert_eq!(
        input.projection.network,
        bytes(target, "network_hex").as_slice()
    );
    assert_eq!(
        input.projection.seed,
        bytes(target, "leader_seed_hex").as_slice()
    );
    let keys: Vec<[u8; 48]> = target
        .get("members")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|m| bytes(m, "public_key_compressed_hex").try_into().unwrap())
        .collect();
    assert_eq!(
        input.roster_root,
        crate::finality::roster::key_tree_native(&keys).unwrap().0
    );
    leaves
}
fn check(c: &ScheduleSourceCircuit) -> bool {
    check_circuit(c, 16, &c.instances().unwrap(), CheckMode::Strict)
        .unwrap()
        .is_satisfied()
}
#[test]
fn native_schedule_proposals_bind_exact4_and31_member_epoch_and_slot_fixtures() {
    for case in 0..2 {
        for block in 1..4 {
            for authorized in [false, true] {
                let leaves = prepare(case, block, authorized);
                let input = leaves[0].input();
                assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
                assert_eq!(
                    leaves[0].endpoints()[4],
                    boundary_digest_native(input, false)
                );
                assert_eq!(
                    leaves.last().unwrap().endpoints()[5],
                    boundary_digest_native(input, true)
                );
                for pair in leaves.windows(2) {
                    assert_eq!(pair[0].endpoints()[1], pair[1].endpoints()[1]);
                    assert_eq!(pair[0].endpoints()[3], pair[1].endpoints()[2]);
                    assert_eq!(pair[0].endpoints()[5], pair[1].endpoints()[4]);
                }
            }
        }
    }
}
#[test]
fn all_native_parser_source_classes_fit_k16_for31_member_authorized_boundary() {
    let leaves = prepare(1, 2, true);
    for index in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 40, 41, 42] {
        assert!(
            check(&leaves[index]),
            "source class {:?} at {index}",
            leaves[index].stage
        );
    }
    // Covers the inactive roster path with the identical member layout.
    let four = prepare(0, 2, false);
    assert!(check(&four[40]));
}
#[test]
fn schedule_source_rejects_substituted_spans_metadata_roster_and_progress() {
    let leaves = prepare(0, 2, true);
    let mut span = leaves[2].clone();
    span.before[BODY] += Fp::ONE;
    assert!(!check(&span));
    let mut membership = leaves[10].clone();
    let mut frame = membership.frame.to_vec();
    frame[membership.input.epoch_hash.payload_start as usize + 100] ^= 1;
    membership.frame = frame.into();
    assert!(!check(&membership));
    let mut count = leaves[3].clone();
    count.input.projection.members = 7;
    assert!(!check(&count));
    let mut height = leaves[0].clone();
    height.input.height += 1;
    assert!(!check(&height));
    let mut epoch = leaves[5].clone();
    epoch.input.projection.epoch += 1;
    assert!(!check(&epoch));
    let mut slot = leaves[8].clone();
    slot.input.projection.slots[0].parameters[0] += 1;
    assert!(!check(&slot));
    let mut key = leaves[41].clone();
    key.before[ROSTER + 1] += Fp::ONE;
    assert!(!check(&key));
    let mut cursor = leaves[10].clone();
    cursor.cursor = 11;
    assert!(!check(&cursor));
    let mut state = leaves[0].clone();
    state.before[ROSTER] = Fp::ONE;
    assert!(!check(&state));
    let mut target = leaves[1].clone();
    target.input.authorized = false;
    assert!(!check(&target));
}
#[test]
fn schedule_member_source_has_one_layout_across_seats_and_unknown_witnesses() {
    let leaves = prepare(1, 2, true);
    let first = &leaves[10];
    let known = synthesize(first, 16, Some(&first.instances().unwrap())).unwrap();
    let unknown = synthesize(&first.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    let metadata = synthesize(
        &ScheduleSourceCircuit::for_source(ScheduleSourceStage::Member),
        16,
        None,
    )
    .unwrap();
    assert_eq!(known.tables.fixed(), metadata.tables.fixed());
    assert_eq!(known.tables.permutation(), metadata.tables.permutation());
    let last = &leaves[40];
    let last = synthesize(last, 16, Some(&last.instances().unwrap())).unwrap();
    assert_eq!(known.tables.fixed(), last.tables.fixed());
    assert_eq!(known.tables.selectors(), last.tables.selectors());
    assert_eq!(known.tables.permutation(), last.tables.permutation());
    let rows = known
        .tables
        .advice_assigned()
        .iter()
        .map(|col| col.iter().rposition(|b| *b).map_or(0, |i| i + 1))
        .max()
        .unwrap_or(0);
    let cells: usize = known
        .tables
        .advice_assigned()
        .iter()
        .map(|col| col.iter().filter(|b| **b).count())
        .sum();
    eprintln!("native schedule member source: {rows} advice rows, {cells} assigned cells");
}
#[test]
#[ignore = "full source qualification across both committee sizes and selected roles"]
fn all43_native_parser_transitions_accept_complete4_and31_member_boundary_programs() {
    for case in 0..2 {
        for authorized in [false, true] {
            for leaf in prepare(case, 2, authorized) {
                assert!(
                    check(&leaf),
                    "case={case} authorized={authorized} cursor={}",
                    leaf.cursor
                );
            }
        }
    }
}

#[derive(Clone)]
struct BindingCircuit {
    input: ScheduleSourceInput,
    known: bool,
}
#[derive(Clone, Debug)]
struct BindingConfig {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl Circuit<Fp> for BindingCircuit {
    type Config = BindingConfig;
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
        let public = meta.instance_column(59);
        meta.enable_equality(public);
        BindingConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let values = layouter.assign_region(
            || "complete native schedule source boundaries",
            |mut region| {
                let input = if self.known {
                    Value::known(self.input)
                } else {
                    Value::unknown()
                };
                let binding = ScheduleSourceBinding::assign(&mut chip, &mut region, &input)?;
                let mut words = binding
                    .complete_parser_endpoints(&mut chip, &mut region)?
                    .words()
                    .to_vec();
                words.extend(
                    binding
                        .complete_epoch_hash_endpoints(&mut chip, &mut region)?
                        .words(),
                );
                words.extend([
                    binding.tape().root().clone(),
                    binding.tape().frame_len().word().clone(),
                    binding.height().word().clone(),
                    binding.roster_root().clone(),
                    binding.members().clone(),
                    binding.faults().clone(),
                    binding.epoch().clone(),
                    binding.first_height().clone(),
                    binding.last_height().clone(),
                    binding.authorized().word().clone(),
                    binding.checksum().clone(),
                    binding.payload_start().clone(),
                    binding.payload_len().clone(),
                ]);
                words.extend_from_slice(binding.context_id());
                words.extend([
                    binding.slot(0).unwrap().pending().clone(),
                    binding.slot(1).unwrap().pending().clone(),
                ]);
                Ok(words)
            },
        )?;
        for (i, word) in values.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn binding_public(input: &ScheduleSourceInput) -> Vec<Vec<Fp>> {
    let mut words = vec![
        Fp::from(PROGRAM_ID),
        input.digest(),
        Fp::ZERO,
        Fp::from(u64::from(PROGRAM_LENGTH)),
        boundary_digest_native(input, false),
        boundary_digest_native(input, true),
        Fp::from(context_hash::PROGRAM_ID),
        input.epoch_hash.digest(),
        Fp::ZERO,
        Fp::from(u64::from(context_hash::PROGRAM_LENGTH)),
        context_hash::boundary_digest_native(&input.epoch_hash, false),
        context_hash::boundary_digest_native(&input.epoch_hash, true),
    ];
    words.extend([
        input.epoch_hash.tape_root,
        Fp::from(u64::from(input.epoch_hash.result_len)),
        Fp::from(input.height),
        input.roster_root,
        Fp::from(u64::from(input.projection.members)),
        Fp::from(u64::from(input.projection.faults)),
        Fp::from(input.projection.epoch),
        Fp::from(input.projection.first),
        Fp::from(input.projection.last),
        Fp::from(u64::from(input.authorized)),
        Fp::from(input.epoch_hash.checksum),
        Fp::from(u64::from(input.epoch_hash.payload_start)),
        Fp::from(u64::from(input.epoch_hash.payload_len)),
    ]);
    words.extend(input.epoch_hash.context_id.map(|b| Fp::from(u64::from(b))));
    words.extend(
        input
            .projection
            .slots
            .map(|s| Fp::from(u64::from(s.pending))),
    );
    vec![words]
}
#[test]
fn typed_source_binding_links_exact_complete_intervals_and_original_claim_cells() {
    let leaves = prepare(1, 2, true);
    let circuit = BindingCircuit {
        input: *leaves[0].input(),
        known: true,
    };
    let public = binding_public(&circuit.input);
    assert!(
        check_circuit(&circuit, 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    for index in [3, 9, 12, 13, 16, 18, 25, 58] {
        let mut forged = public.clone();
        forged[0][index] += Fp::ONE;
        assert!(
            !check_circuit(&circuit, 16, &forged, CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            "binding word {index}"
        );
    }
    let mut truncated = public.clone();
    truncated[0][9] = Fp::from(u64::from(circuit.input.epoch_hash.crc_steps()));
    assert!(
        !check_circuit(&circuit, 16, &truncated, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&circuit, 16, Some(&public)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}
