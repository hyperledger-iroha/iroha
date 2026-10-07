//! Exact history linkage against captured native schedules; these tests do not
//! substitute field openings for the mandatory original child proofs.

use super::*;
use crate::finality::schedule::source::prepare_schedule_source;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

#[derive(Clone)]
struct Link {
    anchor: HistoryAnchor,
    input: HistoryStepInput,
    sources: [[Fp; 6]; 2],
    known: bool,
}
impl Link {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn public(&self) -> Vec<Vec<Fp>> {
        let anchor = self.anchor.digest();
        vec![vec![
            anchor,
            self.input.before.digest(anchor),
            self.input.after.digest(anchor),
        ]]
    }
}
impl Circuit<Fp> for Link {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier =
            iroha_plonk_recursion::verifier::VerifierConfig::configure_serialized_foreign_tagged(
                meta, 3,
            )
            .unwrap();
        let public = meta.instance_column(3);
        meta.enable_equality(public);
        SourcePairConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "original history source linkage",
            |mut region| {
                let anchor = HistoryAnchorCells::assign(&mut chip, &mut region, &self.anchor)?;
                let mut sources = Vec::new();
                for native in self.sources {
                    let words: [Word<Fp>; 6] = chip
                        .uint()
                        .glue()
                        .witnesses(&mut region, &native.map(|v| self.value(v)))?
                        .try_into()
                        .map_err(|_| Error::Synthesis)?;
                    sources.push(SourceEndpoints::from_words(&mut chip, &mut region, &words)?);
                }
                let linked = HistoryStepCells::constrain(
                    &mut chip,
                    &mut region,
                    [&sources[0], &sources[1]],
                    &anchor,
                    &self.value(self.input),
                )?;
                Ok([
                    anchor.digest().clone(),
                    linked.before().digest().clone(),
                    linked.after().digest().clone(),
                ])
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes(j: &norito::json::Value, key: &str) -> Vec<u8> {
    hex(j.get(key).unwrap().as_str().unwrap())
}
fn fixtures(case: usize) -> Vec<Link> {
    let j: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../../fixtures/kagemusha/ordinary_load_npos_schedule_v1.json"
    ))
    .unwrap();
    let case = &j.get("cases").unwrap().as_array().unwrap()[case];
    let blocks = case.get("blocks").unwrap().as_array().unwrap();
    let genesis = blocks[0].get("schedule").unwrap();
    let current = genesis.get("current").unwrap();
    let params = bytes(
        genesis.get("successor_slots").unwrap().as_array().unwrap()[0]
            .get("params")
            .unwrap(),
        "payload_hex",
    );
    let parameters = [(1, 8), (10, 8), (19, 8), (28, 8), (37, 4), (42, 8)].map(|(at, len)| {
        params[at..at + len]
            .iter()
            .rev()
            .fold(0_u64, |v, b| v * 256 + u64::from(*b))
    });
    // Reproduce the native Global instance preimage from captured genesis
    // network and chain identity (kind0, index0). These field-only tests still
    // do not verify a signed genesis or either source proof.
    let network: [u8; 32] = bytes(case, "network_hex").try_into().unwrap();
    let mut instance_preimage = b"sumeragi/instance".to_vec();
    instance_preimage.extend_from_slice(&network);
    instance_preimage.extend_from_slice(case.get("chain_id").unwrap().as_str().unwrap().as_bytes());
    instance_preimage.extend_from_slice(&[0; 5]);
    let instance = <[u8; 32]>::from(iroha_crypto::Hash::new(instance_preimage));
    let expected = if case.get("seats").unwrap().as_u64().unwrap() == 4 {
        "b4ebc81a87ff9a9a95ddc026946acbf68057fa614cb1b28687d8a80e7f1d4c4b"
    } else {
        "ba5246293a2990a09bcc1ee1cf5007b71612fa6f623629446a449638d8188443"
    };
    assert_eq!(instance.as_slice(), hex(expected));
    let anchor = HistoryAnchor {
        network,
        instance,
        initial_context: bytes(current, "context_id_hex").try_into().unwrap(),
        initial_epoch: current.get("epoch").unwrap().as_u64().unwrap(),
        parameters,
    };
    let mut before = HistoryState::genesis(&anchor);
    let mut out = Vec::new();
    for block in blocks.iter().skip(1) {
        let schedule = block.get("schedule").unwrap();
        let current = schedule.get("current").unwrap();
        let boundary = schedule.get("boundary").unwrap();
        let target = if matches!(boundary, norito::json::Value::Null) {
            current
        } else {
            boundary.get("successor").unwrap()
        };
        let id = bytes(target, "context_id_hex").try_into().unwrap();
        let source =
            prepare_schedule_source(bytes(block, "result_preimage_hex"), true, id).unwrap();
        let authorized = *source[0].input();
        let scheduled = ScheduledResultStatement {
            network: anchor.network,
            instance: anchor.instance,
            epoch: current.get("epoch").unwrap().as_u64().unwrap(),
            height: block.get("height").unwrap().as_u64().unwrap(),
            context: bytes(current, "context_id_hex").try_into().unwrap(),
            result: bytes(block, "result_hash_hex").try_into().unwrap(),
            tape_root: authorized.epoch_hash.tape_root,
            frame_len: authorized.epoch_hash.result_len,
        };
        let mut input = HistoryStepInput {
            before,
            after: before,
            scheduled,
            authorized,
        };
        input.after = input.expected_after().unwrap();
        input.validate(&anchor).unwrap();
        before = input.after;
        out.push(Link {
            anchor,
            sources: input.source_endpoints(),
            input,
            known: true,
        });
    }
    out
}
fn accepts(c: &Link) -> bool {
    check_circuit(c, 16, &c.public(), CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
}

#[test]
fn captured4_and31_member_preboundary_boundary_successor_history_links_match() {
    for case in 0..2 {
        let links = fixtures(case);
        assert_eq!(links.len(), 3);
        for (i, link) in links.iter().enumerate() {
            assert!(accepts(link), "case {case} block {}", i + 2);
            assert_eq!(link.input.after.next_height, i as u64 + 3);
            if i != 0 {
                assert_eq!(links[i - 1].input.after, link.input.before);
            }
        }
        assert!(links[0].input.after.following.pending);
        assert!(links[1].input.authorized.projection.boundary_present);
        assert!(links[1].input.before.following.pending);
        assert!(!links[1].input.after.current.pending);
        assert!(!links[1].input.after.following.pending);
        assert_ne!(
            links[1].input.before.current.context,
            links[1].input.after.current.context
        );
    }
}
#[test]
fn exact_source_anchor_authority_lag_two_and_boundary_substitutions_fail() {
    let links = fixtures(0);
    let boundary = &links[1];
    for fault in 0..20 {
        let mut c = boundary.clone();
        match fault {
            0 => c.input.scheduled.height += 1,
            1 => c.input.before.current.context[0] ^= 1,
            2 => c.input.before.current.epoch += 1,
            3 => c.input.authorized.height += 1,
            4 => c.input.authorized.authorized = false,
            5 => c.input.scheduled.network[0] ^= 1,
            6 => c.input.scheduled.instance[0] ^= 1,
            7 => c.input.authorized.epoch_hash.tape_root += Fp::ONE,
            8 => c.input.after.result[0] ^= 1,
            9 => c.input.after.tape_root += Fp::ONE,
            10 => c.input.after.frame_len += 1,
            11 => c.input.after.next_height += 1,
            12 => {
                c.input.after.current.parameters[0] += 1;
                c.input.authorized.projection.slots[0].parameters[0] += 1;
            }
            13 => c.input.before.following = c.input.before.current,
            14 => c.input.before.following.boundary_height += 1,
            15 => c.input.before.following.predecessor[0] ^= 1,
            16 => {
                c.input.before.following.predecessor[0] ^= 1;
                c.input.authorized.projection.boundary_ids[0][0] ^= 1;
            }
            17 => c.input.authorized.projection.boundary_present = false,
            18 => c.input.after.following.parameters[5] += 1,
            19 => c.input.authorized.projection.network[0] ^= 1,
            _ => unreachable!(),
        }
        // Recompute child statement claims to test actual history semantics,
        // rather than merely failing an unchanged context hash.
        c.sources = c.input.source_endpoints();
        assert!(c.input.validate(&c.anchor).is_err(), "native fault {fault}");
        assert!(!accepts(&c), "constrained fault {fault}");
    }
    let mut continuation = links[2].clone();
    continuation.input.authorized.projection.epoch += 1;
    continuation.input.after.current.epoch += 1;
    continuation.input.after.following.epoch += 1;
    continuation.sources = continuation.input.source_endpoints();
    assert!(continuation.input.validate(&continuation.anchor).is_err());
    assert!(
        !accepts(&continuation),
        "nonboundary must preserve the full promised slot"
    );
    for child in 0..2 {
        for field in 0..6 {
            let mut c = boundary.clone();
            c.sources[child][field] += Fp::ONE;
            assert!(!accepts(&c), "source {child} endpoint {field}");
        }
    }
}
#[test]
fn full_u64_heights_and_fixed_policy_shape_are_preserved() {
    let mut c = fixtures(0).remove(2);
    // Arithmetic/linkage qualification above u32; no new native certificate or
    // source proof at this synthetic height is asserted by this test.
    let height = u64::from(u32::MAX) + 91;
    c.input.before.next_height = height;
    c.input.scheduled.height = height;
    c.input.authorized.height = height;
    c.input.after.next_height = height + 1;
    c.sources = c.input.source_endpoints();
    c.input.validate(&c.anchor).unwrap();
    assert!(accepts(&c));
    let known = synthesize(&c, 16, Some(&c.public())).unwrap();
    let erased = c.without_witnesses();
    assert_eq!(erased.anchor, c.anchor);
    let unknown = synthesize(&erased, 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    c.input.before.next_height = u64::MAX;
    c.input.scheduled.height = u64::MAX;
    c.input.authorized.height = u64::MAX;
    c.input.after.next_height = 0;
    c.sources = c.input.source_endpoints();
    assert!(c.input.expected_after().is_err());
    assert!(c.input.validate(&c.anchor).is_err());
    assert!(!accepts(&c));
}
