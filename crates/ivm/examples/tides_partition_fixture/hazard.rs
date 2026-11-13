//! Existing-ISA IVM hazard damage core, not a complete native area transition.
//!
//! Bounded eight-owner caps, materials, cargo loss and linked-episode accounting
//! are executed by bytecode. Independent Rust only initializes and checks words.
//! Geographic exposure, state I/O, beacon, country grace and funding are excluded.

use ivm::{IVM, Memory, ProgramMetadata, encoding::wide as enc, instruction::wide as op};
use norito::{json, json::Value};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

const HEADER: usize = 40;
const STRIDE: usize = 14;
const HP: usize = 0;
const MAX_HP: usize = 1;
const SHELTERED: usize = 2;
const KIND: usize = 3;
const MATERIAL_NUM: usize = 4;
const MATERIAL_DEN: usize = 5;
const EPISODE_DAMAGE: usize = 6;
const XOR: usize = 7;
const OWNER: usize = 8;
const CARGO: usize = 9;
const LOST_CARGO: usize = 10;
const EPISODE_MAX_HP: usize = 11;
const DAMAGE: usize = 12;
const ID: usize = 13;
const BASE: u64 = Memory::HEAP_START + 4096;
const GAS: u64 = 1_680_000;

#[derive(Default)]
struct Assembler {
    code: Vec<u32>,
    labels: BTreeMap<&'static str, usize>,
    fixups: Vec<(usize, u8, u8, u8, &'static str, bool)>,
}
impl Assembler {
    fn rr(&mut self, opcode: u8, dst: u8, a: u8, b: u8) {
        self.code.push(enc::encode_rr(opcode, dst, a, b));
    }
    fn ri(&mut self, dst: u8, a: u8, imm: i8) {
        self.code
            .push(enc::encode_ri(op::arithmetic::ADDI, dst, a, imm));
    }
    fn load(&mut self, dst: u8, base: u8, offset: i8) {
        self.code
            .push(enc::encode_load(op::memory::LOAD64, dst, base, offset));
    }
    fn store(&mut self, base: u8, src: u8, offset: i8) {
        self.code
            .push(enc::encode_store(op::memory::STORE64, base, src, offset));
    }
    fn label(&mut self, name: &'static str) {
        assert!(self.labels.insert(name, self.code.len()).is_none());
    }
    fn branch(&mut self, opcode: u8, a: u8, b: u8, name: &'static str) {
        self.fixups
            .push((self.code.len(), opcode, a, b, name, false));
        self.code.push(0);
    }
    fn jump(&mut self, name: &'static str) {
        self.fixups
            .push((self.code.len(), op::control::JAL, 0, 0, name, true));
        self.code.push(0);
    }
    fn finish(mut self) -> Vec<u8> {
        self.code.push(enc::encode_halt());
        for (at, opcode, a, b, name, jump) in self.fixups {
            let delta = self.labels[name] as isize - at as isize;
            self.code[at] = if jump {
                enc::encode_jump(opcode, 0, i16::try_from(delta).expect("jump bound"))
            } else {
                enc::encode_branch(opcode, a, b, i8::try_from(delta).expect("branch bound"))
            };
        }
        let mut bytes = ProgramMetadata {
            max_cycles: 1_000_000,
            ..ProgramMetadata::default()
        }
        .encode();
        for word in self.code {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

fn program() -> Vec<u8> {
    use op::{arithmetic as ar, control as ctl};
    let mut a = Assembler::default();
    // Header: N,event,last-event,episode,last-episode,event-damage,unit-deaths,
    // building-deaths, then eight owner groups (units,buildings,killed U,killed B).
    a.load(2, 1, 0);
    a.load(3, 1, 8);
    a.load(4, 1, 16);
    a.branch(ctl::BNE, 3, 4, "begin");
    a.jump("finish");
    a.label("begin");
    a.ri(24, 0, 4);
    a.ri(28, 0, 80);
    a.rr(ar::MUL, 28, 28, 24);
    a.rr(ar::ADD, 28, 28, 1); // Entity base = input + 320 bytes.
    a.ri(29, 0, 112);
    a.ri(30, 1, 64); // Owner bucket base.
    a.ri(31, 0, 32); // Bytes per owner.
    a.load(3, 1, 24);
    a.load(4, 1, 32);
    a.branch(ctl::BNE, 3, 4, "reset_episode");
    a.jump("start_damage");
    a.label("reset_episode");
    a.rr(ar::ADD, 3, 30, 0);
    a.ri(4, 0, 0);
    a.ri(7, 0, 8);
    a.label("reset_owner");
    for offset in [0, 8, 16, 24] {
        a.store(3, 0, offset);
    }
    a.rr(ar::ADD, 3, 3, 31);
    a.ri(4, 4, 1);
    a.branch(ctl::BLTU, 4, 7, "reset_owner");
    a.rr(ar::ADD, 3, 28, 0);
    a.ri(4, 0, 0);
    a.ri(7, 0, 1);
    a.label("count");
    a.load(8, 3, 8);
    a.store(3, 8, 88);
    a.store(3, 0, 48);
    a.load(8, 3, 0);
    a.branch(ctl::BEQ, 8, 0, "count_next");
    a.load(9, 3, 64);
    a.rr(ar::MUL, 9, 9, 31);
    a.rr(ar::ADD, 9, 9, 30);
    a.load(10, 3, 24);
    a.branch(ctl::BNE, 10, 0, "count_building");
    a.load(11, 9, 0);
    a.ri(11, 11, 1);
    a.store(9, 11, 0);
    a.jump("count_next");
    a.label("count_building");
    a.branch(ctl::BNE, 10, 7, "count_next");
    a.load(11, 9, 8);
    a.ri(11, 11, 1);
    a.store(9, 11, 8);
    a.label("count_next");
    a.rr(ar::ADD, 3, 3, 29);
    a.ri(4, 4, 1);
    a.branch(ctl::BLTU, 4, 2, "count");
    a.load(3, 1, 24);
    a.store(1, 3, 32);
    a.label("start_damage");
    a.rr(ar::ADD, 3, 28, 0);
    a.ri(4, 0, 0);
    a.ri(20, 0, 0);
    a.ri(21, 0, 0);
    a.ri(22, 0, 0);
    a.ri(23, 0, 100);
    a.rr(ar::MUL, 23, 23, 23); // 10000 basis point denominator.
    a.ri(25, 0, 2);
    a.ri(26, 0, 10);
    a.label("damage");
    a.store(3, 0, 96);
    a.load(8, 3, 0);
    a.branch(ctl::BEQ, 8, 0, "next");
    a.load(9, 3, 8);
    a.load(10, 3, 16);
    a.ri(11, 0, 100);
    a.rr(ar::MUL, 11, 11, 26); // Exposed 1000 bp.
    a.branch(ctl::BEQ, 10, 0, "exposed");
    a.ri(12, 0, 5);
    a.rr(ar::DIVU, 11, 11, 12); // Sheltered 200 bp.
    a.label("exposed");
    a.load(12, 3, 32);
    a.rr(ar::MUL, 11, 11, 12);
    a.rr(ar::MUL, 11, 11, 9);
    a.load(12, 3, 40);
    a.rr(ar::MUL, 12, 12, 23);
    a.rr(ar::DIVU, 11, 11, 12); // Material-adjusted desired damage.
    a.load(12, 3, 88);
    a.ri(13, 0, 3);
    a.rr(ar::MUL, 12, 12, 13);
    a.rr(ar::DIVU, 12, 12, 26); // Episode-wide 30% ceiling.
    a.load(14, 3, 48);
    a.rr(ar::SUB, 12, 12, 14); // Input validation binds used <= ceiling.
    a.rr(ar::MIN, 11, 11, 12);
    a.rr(ar::MIN, 11, 11, 8);
    a.rr(ar::SUB, 12, 8, 11);
    a.load(13, 3, 24);
    a.branch(ctl::BNE, 13, 25, "mortal");
    a.ri(14, 9, 3);
    a.rr(ar::DIVU, 14, 14, 24);
    a.rr(ar::MIN, 14, 14, 8);
    a.rr(ar::MAX, 12, 12, 14); // Non-healing ceil25% Hall floor.
    a.jump("write");
    a.label("mortal");
    a.branch(ctl::BNE, 12, 0, "write");
    a.load(15, 3, 64);
    a.rr(ar::MUL, 15, 15, 31);
    a.rr(ar::ADD, 15, 15, 30);
    a.branch(ctl::BNE, 13, 0, "building_death");
    a.load(16, 15, 0);
    a.ri(17, 0, 5);
    a.rr(ar::DIVU, 16, 16, 17);
    a.load(17, 15, 16);
    a.branch(ctl::BGEU, 17, 16, "survive_quota");
    a.ri(17, 17, 1);
    a.store(15, 17, 16);
    a.ri(20, 20, 1);
    a.jump("destroy_cargo");
    a.label("building_death");
    a.load(16, 15, 8);
    a.rr(ar::DIVU, 16, 16, 26);
    a.load(17, 15, 24);
    a.branch(ctl::BGEU, 17, 16, "survive_quota");
    a.ri(17, 17, 1);
    a.store(15, 17, 24);
    a.ri(21, 21, 1);
    a.label("destroy_cargo");
    a.load(16, 3, 72);
    a.load(17, 3, 80);
    a.rr(ar::ADD, 17, 17, 16);
    a.store(3, 0, 72);
    a.store(3, 17, 80);
    a.jump("write");
    a.label("survive_quota");
    a.ri(12, 0, 1);
    a.label("write");
    a.rr(ar::SUB, 11, 8, 12);
    a.store(3, 12, 0);
    a.store(3, 11, 96);
    a.load(14, 3, 48);
    a.rr(ar::ADD, 14, 14, 11);
    a.store(3, 14, 48);
    a.rr(ar::ADD, 22, 22, 11);
    a.label("next");
    a.rr(ar::ADD, 3, 3, 29);
    a.ri(4, 4, 1);
    a.branch(ctl::BLTU, 4, 2, "damage");
    a.store(1, 22, 40);
    a.store(1, 20, 48);
    a.store(1, 21, 56);
    a.load(3, 1, 8);
    a.store(1, 3, 16);
    a.label("finish");
    a.finish()
}

fn validate(words: &[u64]) -> Result<(), &'static str> {
    let Some(&n) = words.first() else {
        return Err("missing header");
    };
    if !(1..=256).contains(&n) || words.len() != HEADER + n as usize * STRIDE {
        return Err("entity bound");
    }
    if words[1] < words[2] || words[3] < words[4] {
        return Err("event order");
    }
    for (i, e) in words[HEADER..].chunks_exact(STRIDE).enumerate() {
        if e[MAX_HP] == 0
            || e[MAX_HP] > 1_000_000
            || e[HP] > e[MAX_HP]
            || e[SHELTERED] > 1
            || e[KIND] > 2
            || e[OWNER] > 7
            || e[CARGO] > 1000
            || e[LOST_CARGO] > 1_000_000
            || ![(3, 2), (1, 1), (1, 2), (3, 4)].contains(&(e[MATERIAL_NUM], e[MATERIAL_DEN]))
            || e[ID] != i as u64
            || e[EPISODE_MAX_HP] > 1_000_000
            || e[EPISODE_DAMAGE] > e[EPISODE_MAX_HP] * 3 / 10
        {
            return Err("entity domain");
        }
    }
    for group in words[8..HEADER].chunks_exact(4) {
        if group[0] > n || group[1] > n || group[2] > group[0] / 5 || group[3] > group[1] / 10 {
            return Err("owner cap domain");
        }
    }
    Ok(())
}

fn oracle(words: &[u64]) -> Vec<u64> {
    validate(words).expect("bounded hazard input");
    if words[1] == words[2] {
        return words.to_vec();
    }
    let mut out = words.to_vec();
    if out[3] != out[4] {
        out[8..HEADER].fill(0);
        for i in 0..out[0] as usize {
            let b = HEADER + i * STRIDE;
            out[b + EPISODE_MAX_HP] = out[b + MAX_HP];
            out[b + EPISODE_DAMAGE] = 0;
            let k = out[b + KIND] as usize;
            if out[b + HP] > 0 && k < 2 {
                let group = 8 + out[b + OWNER] as usize * 4 + k;
                out[group] += 1;
            }
        }
        out[4] = out[3];
    }
    out[5..8].fill(0);
    for i in 0..out[0] as usize {
        let b = HEADER + i * STRIDE;
        let hp = out[b + HP];
        let bps = if out[b + SHELTERED] == 1 { 200 } else { 1000 };
        let desired =
            out[b + MAX_HP] * bps * out[b + MATERIAL_NUM] / (10000 * out[b + MATERIAL_DEN]);
        let available = out[b + EPISODE_MAX_HP] * 3 / 10 - out[b + EPISODE_DAMAGE];
        let mut next = hp - hp.min(desired).min(available);
        let kind = out[b + KIND] as usize;
        if kind == 2 {
            next = next.max(hp.min(out[b + MAX_HP].div_ceil(4)));
        }
        if hp > 0 && next == 0 && kind < 2 {
            let g = 8 + out[b + OWNER] as usize * 4;
            let limit = out[g + kind] / if kind == 0 { 5 } else { 10 };
            if out[g + 2 + kind] < limit {
                out[g + 2 + kind] += 1;
                out[6 + kind] += 1;
                out[b + LOST_CARGO] += out[b + CARGO];
                out[b + CARGO] = 0;
            } else {
                next = 1;
            }
        }
        out[b + DAMAGE] = hp - next;
        out[b + HP] = next;
        out[b + EPISODE_DAMAGE] += hp - next;
        out[5] += hp - next;
    }
    out[2] = out[1];
    out
}

fn scene(sheltered: bool, weakened: bool) -> Vec<u64> {
    let mut words = vec![0; HEADER];
    words[0] = 256;
    words[1] = 1;
    words[3] = 1;
    for i in 0..256 {
        let kind = if i < 192 {
            0
        } else if i < 248 {
            1
        } else {
            2
        };
        let hp = if weakened && i % 3 == 0 { 10 } else { 1000 };
        let (num, den) = [(3, 2), (1, 1), (1, 2), (3, 4)][i as usize % 4];
        // Fourteen ordinary buildings for each of four owners allow one real
        // building death per owner. Seven buildings per owner would round every
        // 10% death quota down to zero and miss the building destruction path.
        let owner = if kind == 1 { i % 4 } else { i % 8 };
        let cargo = match kind {
            0 => 4,
            1 => 2,
            _ => 0,
        };
        words.extend_from_slice(&[
            hp,
            1000,
            u64::from(sheltered),
            kind,
            num,
            den,
            0,
            10000 + i,
            owner,
            cargo,
            0,
            0,
            0,
            i,
        ]);
    }
    words
}
fn setup(words: &[u64]) -> IVM {
    validate(words).expect("bounded fixture");
    let mut vm = IVM::new(GAS);
    vm.set_zk_trace_enabled(false);
    vm.load_program(&program()).expect("valid bytecode");
    vm.set_register(1, BASE);
    for (i, w) in words.iter().enumerate() {
        vm.memory
            .store_u64(BASE + (i * 8) as u64, *w)
            .expect("input");
    }
    black_box(vm.memory.root());
    vm
}
fn check(vm: &IVM, expected: &[u64]) {
    for (i, w) in expected.iter().enumerate() {
        assert_eq!(
            vm.memory.load_u64(BASE + (i * 8) as u64).expect("output"),
            *w,
            "hazard word {i}"
        );
    }
}
/// Measure actual IVM damage core, excluding durable I/O and whole-area scheduling.
pub fn benchmark() -> Value {
    let mut results = Vec::new();
    for (name, sheltered, weakened, linked) in [
        ("exposed", false, false, false),
        ("sheltered", true, false, false),
        ("weakened_owner_caps", false, true, false),
        ("linked_episode_ceiling", false, false, true),
    ] {
        let mut input = scene(sheltered, weakened);
        if linked {
            for id in 1..=3 {
                input[1] = id;
                input = oracle(&input);
            }
            input[1] = 4;
        }
        let expected = oracle(&input);
        assert!(
            input[HEADER..]
                .chunks_exact(STRIDE)
                .zip(expected[HEADER..].chunks_exact(STRIDE))
                .all(|(before, after)| {
                    (before[XOR], before[OWNER]) == (after[XOR], after[OWNER])
                }),
            "hazard damage must leave every XOR and ownership sentinel unchanged"
        );
        if weakened {
            assert_eq!(expected[6], 32, "four unit deaths per each of eight owners");
            assert_eq!(expected[7], 4, "one building death per each of four owners");
            assert_eq!(
                expected[HEADER..]
                    .chunks_exact(STRIDE)
                    .map(|entity| entity[LOST_CARGO])
                    .sum::<u64>(),
                136,
                "32*4 unit cargo plus 4*2 building cargo"
            );
        }
        let mut vm = setup(&input);
        let template = vm.try_runtime_template().expect("captured hazard runtime");
        let mut walls = Vec::new();
        let mut rooted = Vec::new();
        let mut gas = 0;
        let mut cycles = 0;
        for _ in 0..30 {
            vm.reset_from_runtime_template(&template)
                .expect("warm reset");
            let start = Instant::now();
            vm.run().expect("hazard execution");
            walls.push(start.elapsed().as_nanos() as u64);
            black_box(vm.memory.root());
            rooted.push(start.elapsed().as_nanos() as u64);
            gas = GAS - vm.gas_remaining;
            cycles = vm.get_cycle_count();
            check(&vm, &expected);
        }
        walls.sort_unstable();
        rooted.sort_unstable();
        let lost: u64 = expected[HEADER..]
            .chunks_exact(STRIDE)
            .map(|e| e[LOST_CARGO])
            .sum();
        results.push(json!({"name":name,"entities":256,"owners":8,"samples":30,"gas":gas,"cycles":cycles,
            "warm_run_p50_ns":(walls[15]),"warm_run_p95_ns":(walls[28]),"warm_run_root_p50_ns":(rooted[15]),"warm_run_root_p95_ns":(rooted[28]),
            "hp_damage":(expected[5]),"units_destroyed":(expected[6]),"buildings_destroyed":(expected[7]),"cargo_destroyed":lost,
            "all_words_oracle_verified":(expected.len()),"xor_and_owner_unchanged":true}));
    }
    json!({"scope":"existing-ISA damage core; eight-owner quotas, materials, cargo loss and linked-episode counters; excludes durable state I/O, geography, beacon, funding, country grace, policy/scheduler, proof and full native area admission",
      "parameters":"provisional 10%/2% exposure; material 3/2,1,1/2,3/4; cumulative 30% HP; per-owner 20% unit/10% building deaths; non-healing ceil25% Hall floor; canonical IDs",
      "population":"192 units across eight owners; 56 ordinary buildings across four owners; eight Halls across eight owners; building cargo is a bounded destruction counter, not the complete storage economy",
      "python_differences":"fixed 10000-bp event intensity; entity-ID quota order rather than the Python model's hash(episode,id) order; Rust checks arithmetic independently, not full Python transition parity",
      "scenarios":(Value::Array(results))})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(input: &[u64]) -> Vec<u64> {
        let expected = oracle(input);
        let mut vm = setup(input);
        vm.run().expect("IVM hazard");
        check(&vm, &expected);
        expected
    }
    #[test]
    fn all_256_entities_match_independent_oracle_with_materials() {
        for s in [false, true] {
            for w in [false, true] {
                run(&scene(s, w));
            }
        }
    }
    #[test]
    fn shelter_reduces_damage_fivefold_and_no_value_moves() {
        let input = scene(false, false);
        let after = run(&input);
        let safe = run(&scene(true, false));
        assert_eq!(after[5], safe[5] * 5);
        for (a, b) in input[HEADER..]
            .chunks_exact(STRIDE)
            .zip(after[HEADER..].chunks_exact(STRIDE))
        {
            assert_eq!((a[XOR], a[OWNER]), (b[XOR], b[OWNER]));
        }
    }
    #[test]
    fn linked_episodes_cannot_repeat_caps_or_cargo_losses() {
        let mut x = scene(false, true);
        for id in 1..=8 {
            x[1] = id;
            x = run(&x);
            for e in x[HEADER..].chunks_exact(STRIDE) {
                assert!(e[EPISODE_DAMAGE] <= e[MAX_HP] * 3 / 10);
                let initial = match e[KIND] {
                    0 => 4,
                    1 => 2,
                    _ => 0,
                };
                assert_eq!(e[CARGO] + e[LOST_CARGO], initial);
            }
            for g in x[8..HEADER].chunks_exact(4) {
                assert!(g[2] <= g[0] / 5);
                assert!(g[3] <= g[1] / 10);
            }
        }
    }
    #[test]
    fn weakened_scene_executes_each_building_death_and_cargo_path() {
        let input = scene(false, true);
        let after = run(&input);
        assert_eq!((after[6], after[7]), (32, 4));
        let mut building_cargo_lost = 0;
        for (before, entity) in input[HEADER..]
            .chunks_exact(STRIDE)
            .zip(after[HEADER..].chunks_exact(STRIDE))
        {
            assert_eq!(entity[CARGO] + entity[LOST_CARGO], before[CARGO]);
            if entity[KIND] == 1 && entity[HP] == 0 {
                assert_eq!((entity[CARGO], entity[LOST_CARGO]), (0, 2));
                building_cargo_lost += entity[LOST_CARGO];
            }
        }
        assert_eq!(building_cargo_lost, 8);
        for owner in 0..8 {
            let group = &after[8 + owner * 4..12 + owner * 4];
            assert_eq!((group[0], group[2]), (24, 4));
            assert_eq!(
                (group[1], group[3]),
                if owner < 4 { (14, 1) } else { (0, 0) }
            );
        }
    }
    #[test]
    fn one_owner_cannot_borrow_another_owners_death_budget() {
        let mut x = scene(false, true);
        for e in x[HEADER..].chunks_exact_mut(STRIDE) {
            e[OWNER] = 0;
        }
        x[HEADER + OWNER] = 1;
        let after = run(&x);
        assert_eq!(after[HEADER + HP], 1);
        assert_eq!(after[8 + 4 + 2], 0);
    }
    #[test]
    fn replay_idempotent_and_next_episode_resets() {
        let once = run(&scene(false, true));
        assert_eq!(run(&once), once);
        let mut next = once;
        next[1] += 1;
        next[3] += 1;
        run(&next);
    }
    #[test]
    fn tiny_hall_never_dies_or_heals() {
        for maxhp in 1..=10 {
            for hp in 0..=maxhp {
                let mut x = vec![0; HEADER];
                x[0] = 1;
                x[1] = 1;
                x[3] = 1;
                x.extend_from_slice(&[hp, maxhp, 0, 2, 3, 2, 0, 4, 0, 0, 0, 0, 0, 0]);
                let y = run(&x);
                assert!(y[HEADER + HP] <= hp);
                if hp > 0 {
                    assert!(y[HEADER + HP] > 0);
                }
            }
        }
    }
    #[test]
    fn invalid_domains_fail_before_execution() {
        let mut x = scene(false, false);
        x[HEADER + OWNER] = 8;
        assert!(validate(&x).is_err());
        x[HEADER + OWNER] = 0;
        x[HEADER + MATERIAL_DEN] = 0;
        assert!(validate(&x).is_err());
    }
}
