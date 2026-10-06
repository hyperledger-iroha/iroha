//! Minimal compact lengths, fixed witness shape and enclosing-bound checks.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_gadgets::{
    GlueConfig, LimbBits, RunningSumChip, RunningSumConfig, tamper::undetected_tampers,
};

#[derive(Clone)]
struct LengthCircuit {
    bytes: [u64; 3],
    start: u128,
    end: u128,
    known: bool,
}

#[derive(Clone)]
struct Config {
    glue: GlueConfig,
    range: RunningSumConfig,
    public: Column<Instance>,
}

fn witness<T>(known: bool, value: T) -> Value<T> {
    if known {
        Value::known(value)
    } else {
        Value::unknown()
    }
}

impl Circuit<Fp> for LengthCircuit {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let columns = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, columns, constants);
        let column = meta.advice_column();
        let range =
            RunningSumConfig::configure(meta, column, LimbBits::new(4).expect("nibble table"));
        let public = meta.instance_column(8);
        meta.enable_equality(public);
        Config {
            glue,
            range,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let words = layouter.assign_region(
            || "result compact length",
            |mut region| {
                let bytes = glue.witnesses(
                    &mut region,
                    &self.bytes.map(|byte| witness(self.known, Fp::from(byte))),
                )?;
                let bytes: [Word<Fp>; 3] = bytes.try_into().map_err(|_| Error::Synthesis)?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let start = uint.assign::<32>(&mut region, witness(self.known, self.start))?;
                let end = uint.assign::<32>(&mut region, witness(self.known, self.end))?;
                let length = CompactResultLength::from_window(&mut uint, &mut region, &bytes)?;
                let next = length.end_from(&mut uint, &mut region, &start, &end)?;
                Ok(vec![
                    bytes[0].clone(),
                    bytes[1].clone(),
                    bytes[2].clone(),
                    start.word().clone(),
                    end.word().clone(),
                    length.value().word().clone(),
                    length.encoded_bytes().word().clone(),
                    next.word().clone(),
                ])
            },
        )?;
        for (i, word) in words.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

fn case(bytes: [u64; 3], value: u64, width: u64) -> (LengthCircuit, Vec<Fp>) {
    let circuit = LengthCircuit {
        bytes,
        start: 40,
        end: 65_536,
        known: true,
    };
    let public = bytes
        .into_iter()
        .chain([40, 65_536, value, width, 40 + value + width])
        .map(Fp::from)
        .collect();
    (circuit, public)
}

#[test]
fn compact_result_lengths_are_canonical_and_ignore_only_inactive_lookahead() {
    for (bytes, value, width) in [
        ([0, 255, 255], 0, 1),
        ([127, 128, 255], 127, 1),
        ([128, 1, 255], 128, 2),
        ([255, 127, 255], 16_383, 2),
        ([128, 128, 1], 16_384, 3),
    ] {
        let (circuit, public) = case(bytes, value, width);
        assert!(
            check_circuit(&circuit, 9, &[public], CheckMode::Strict)
                .expect("canonical length")
                .is_satisfied()
        );
    }
    for bytes in [
        [128, 0, 255],
        [129, 128, 0],
        [128, 128, 128],
        [129, 128, 4],
        [256, 0, 0],
    ] {
        let (circuit, public) = case(bytes, 0, 1);
        assert!(
            !check_circuit(&circuit, 9, &[public], CheckMode::Strict)
                .expect("malformed length")
                .is_satisfied()
        );
    }
}

#[test]
fn compact_length_prefix_and_value_are_fully_bound_to_the_source_window() {
    let (circuit, public) = case([172, 2, 32], 300, 2);
    assert!(
        undetected_tampers(&circuit, 9, core::slice::from_ref(&public))
            .expect("all cells")
            .is_empty()
    );
    let known = synthesize(&circuit, 9, Some(core::slice::from_ref(&public))).expect("known");
    let unknown = synthesize(&circuit.without_witnesses(), 9, None).expect("unknown");
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}

#[test]
fn length_cannot_skip_past_its_enclosing_payload_or_wrap_cursor() {
    let (mut circuit, mut public) = case([172, 2, 32], 300, 2);
    circuit.end = 341;
    public[4] = Fp::from(341);
    assert!(
        !check_circuit(&circuit, 9, &[public], CheckMode::Strict)
            .expect("field overrun")
            .is_satisfied()
    );
    let (mut circuit, mut public) = case([172, 2, 32], 300, 2);
    circuit.start = u128::from(u32::MAX);
    circuit.end = u128::from(u32::MAX);
    public[3] = Fp::from(u64::from(u32::MAX));
    public[4] = public[3];
    public[7] = Fp::from(301);
    assert!(
        !check_circuit(&circuit, 9, &[public], CheckMode::Strict)
            .expect("cursor overflow")
            .is_satisfied()
    );
}

#[derive(Clone)]
struct StreamCircuit {
    frame: Vec<u8>,
    declared: u128,
    domain_tamper: bool,
    padding_tamper: bool,
    omit_last: bool,
    append_block: bool,
    known: bool,
}

#[derive(Clone)]
struct StreamConfig {
    glue: GlueConfig,
    range: RunningSumConfig,
    blake: iroha_plonk_gadgets::blake2b::Blake2bConfig,
    public: Column<Instance>,
}

fn stream_config(meta: &mut ConstraintSystem<Fp>, public_len: usize) -> StreamConfig {
    let advice = core::array::from_fn(|_| meta.advice_column());
    let constants = meta.fixed_column();
    let glue = GlueConfig::configure(meta, advice, constants);
    let advice = meta.advice_column();
    let range = RunningSumConfig::configure(meta, advice, LimbBits::new(8).expect("byte table"));
    let advice = core::array::from_fn(|_| meta.advice_column());
    let blake = iroha_plonk_gadgets::blake2b::Blake2bConfig::configure(meta, advice, constants);
    let public = meta.instance_column(public_len);
    meta.enable_equality(public);
    StreamConfig {
        glue,
        range,
        blake,
        public,
    }
}

impl Circuit<Fp> for StreamCircuit {
    type Config = StreamConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StreamConfig {
        stream_config(meta, 33)
    }
    fn synthesize(
        &self,
        config: StreamConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let output = layouter.assign_region(
            || "bounded result hash stream",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let len = uint.assign::<32>(&mut region, witness(self.known, self.declared))?;
                let mut stream = ResultHashStream::start(&mut uint, &mut blake, &mut region, &len)?;
                let mut transcript = RESULT_TAG.to_vec();
                transcript.extend_from_slice(&self.frame);
                if self.domain_tamper {
                    transcript[0] ^= 1;
                }
                let blocks = transcript.len().div_ceil(128);
                transcript.resize(blocks * 128, 0);
                if self.padding_tamper {
                    transcript[blocks * 128 - 1] = 1;
                }
                let steps = blocks - usize::from(self.omit_last) + usize::from(self.append_block);
                transcript.resize(steps.max(blocks) * 128, 0);
                for block in transcript.chunks_exact(128).take(steps) {
                    let values: Vec<_> = block
                        .iter()
                        .map(|byte| witness(self.known, Fp::from(u64::from(*byte))))
                        .collect();
                    let words = uint
                        .glue()
                        .witnesses(&mut region, &values)?
                        .try_into()
                        .map_err(|_| Error::Synthesis)?;
                    stream = stream.absorb(&mut uint, &mut blake, &mut region, &words)?;
                }
                let digest = stream.finish(&mut blake, &mut region)?;
                let mut out = vec![len.word().clone()];
                out.extend_from_slice(digest.bytes());
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

fn stream_case(len: usize, digest: &str) -> (StreamCircuit, Vec<Fp>) {
    let circuit = StreamCircuit {
        frame: (0..len).map(|i| (i % 256) as u8).collect(),
        declared: len as u128,
        domain_tamper: false,
        padding_tamper: false,
        omit_last: false,
        append_block: false,
        known: true,
    };
    let mut public = vec![Fp::from(len as u64)];
    public.extend(digest.as_bytes().chunks_exact(2).map(|pair| {
        Fp::from(u64::from(
            u8::from_str_radix(core::str::from_utf8(pair).expect("hex"), 16).expect("byte"),
        ))
    }));
    (circuit, public)
}

#[test]
fn result_domain_hash_stream_matches_independent_blake_at_block_boundaries() {
    // These are synthetic byte frames testing the hash schedule only, not
    // native result decoding. Oracle: hashlib.blake2b(TAG+frame,digest_size=32),
    // then output[31]|=1. Full canonical native result fixtures are separate.
    for (len, digest) in [
        (
            0,
            "7b9de28f8be3ca2e7ca5605ab3fc9b9873424642aaaa968c2050aedd276c2ecb",
        ),
        (
            1,
            "f768b3cf4ab52bdb0d0cba74798820c1aec6d6b409bea8d504783cbd7b97bb4f",
        ),
        (
            104,
            "133613b7ee4159a9709487b2f032aea6913cbf298b5a2d095e7f9e66c6d0b8e5",
        ),
        (
            105,
            "ca9e5fd789b5991db36cacbbf933474cc737b386992ef12c1960a7e0a7d106c9",
        ),
        (
            232,
            "5ecfabfd8d9568251b558ebcbb15abab4106a655309582c129840fb83e6da6c9",
        ),
    ] {
        let (circuit, public) = stream_case(len, digest);
        let k = if len <= 104 { 14 } else { 15 };
        assert!(
            check_circuit(&circuit, k, &[public], CheckMode::Strict)
                .expect("bounded hash stream")
                .is_satisfied(),
            "frame length {len}"
        );
    }
}

#[test]
fn result_stream_rejects_changed_domain_nonzero_padding_wrong_length_and_extra_blocks() {
    let (circuit, public) = stream_case(
        1,
        "f768b3cf4ab52bdb0d0cba74798820c1aec6d6b409bea8d504783cbd7b97bb4f",
    );
    for attack in 0..6 {
        let mut forged = circuit.clone();
        let mut expected = public.clone();
        match attack {
            0 => forged.domain_tamper = true,
            1 => forged.padding_tamper = true,
            2 => forged.omit_last = true,
            3 => forged.append_block = true,
            4 => {
                forged.declared = 2;
                expected[0] = Fp::from(2);
            }
            _ => {
                forged.declared = u128::from(MAX_RESULT_BYTES) + 1;
                expected[0] = Fp::from(u64::from(MAX_RESULT_BYTES) + 1);
            }
        }
        assert!(
            !check_circuit(&forged, 15, &[expected], CheckMode::Strict)
                .expect("stream attack layout")
                .is_satisfied(),
            "attack {attack}"
        );
    }
    let known =
        synthesize(&circuit, 14, Some(core::slice::from_ref(&public))).expect("known stream");
    let unknown = synthesize(&circuit.without_witnesses(), 14, None).expect("unknown stream");
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}

// Integer RFC compression oracle; the final digest is independently pinned
// by the production-native fixture. This is test code, not a proof input.
const IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

fn native_compress(mut h: [u64; 8], block: &[u8; 128], counter: u128, last: bool) -> [u64; 8] {
    fn g(v: &mut [u64; 16], [a, b, c, d]: [usize; 4], x: u64, y: u64) {
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
        v[d] = (v[d] ^ v[a]).rotate_right(32);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(24);
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
        v[d] = (v[d] ^ v[a]).rotate_right(16);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(63);
    }
    let message: [u64; 16] = core::array::from_fn(|i| {
        u64::from_le_bytes(block[i * 8..i * 8 + 8].try_into().expect("word"))
    });
    let mut v = [0; 16];
    v[..8].copy_from_slice(&h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= counter as u64;
    v[13] ^= (counter >> 64) as u64;
    if last {
        v[14] = !v[14];
    }
    for r in 0..12 {
        let s = SIGMA[r % 10];
        g(&mut v, [0, 4, 8, 12], message[s[0]], message[s[1]]);
        g(&mut v, [1, 5, 9, 13], message[s[2]], message[s[3]]);
        g(&mut v, [2, 6, 10, 14], message[s[4]], message[s[5]]);
        g(&mut v, [3, 7, 11, 15], message[s[6]], message[s[7]]);
        g(&mut v, [0, 5, 10, 15], message[s[8]], message[s[9]]);
        g(&mut v, [1, 6, 11, 12], message[s[10]], message[s[11]]);
        g(&mut v, [2, 7, 8, 13], message[s[12]], message[s[13]]);
        g(&mut v, [3, 4, 9, 14], message[s[14]], message[s[15]]);
    }
    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
    h
}

fn native_fixture() -> norito::json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    norito::json::from_str(&std::fs::read_to_string(path).expect("native result fixture")).unwrap()
}

fn fixture_bytes(fixture: &norito::json::Value, name: &str) -> Vec<u8> {
    let hex = fixture
        .get(name)
        .and_then(norito::json::Value::as_str)
        .unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn initial_words() -> [u64; 8] {
    let mut words = IV;
    words[0] ^= 0x0101_0020;
    words
}

#[derive(Clone)]
struct PrefixCircuit {
    frame: Vec<u8>,
    hash: bool,
    finish_early: bool,
    known: bool,
}

impl Circuit<Fp> for PrefixCircuit {
    type Config = StreamConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        stream_config(meta, 43)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let output = layouter.assign_region(
            || "native result prefix",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let len =
                    uint.assign::<32>(&mut region, witness(self.known, self.frame.len() as u128))?;
                let mut transcript = RESULT_TAG.to_vec();
                transcript.extend_from_slice(&self.frame);
                let values: Vec<_> = transcript[..384]
                    .iter()
                    .map(|byte| witness(self.known, Fp::from(u64::from(*byte))))
                    .collect();
                let source = uint.glue().witnesses(&mut region, &values)?;
                let (prefix, state) = if self.hash {
                    let blocks: [[Word<Fp>; 128]; 3] = core::array::from_fn(|i| {
                        core::array::from_fn(|j| source[i * 128 + j].clone())
                    });
                    let stream = ResultPrefixStream::start(
                        &mut uint,
                        &mut blake,
                        &mut region,
                        &len,
                        &blocks,
                    )?;
                    if self.finish_early {
                        stream.finish(&mut blake, &mut region)?;
                    }
                    let state = blake.state_words(&mut region, stream.stream.state())?;
                    (
                        stream.prefix,
                        state
                            .into_iter()
                            .map(|word| word.word().clone())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    let prefix = ResultPrefix::parse(
                        &mut uint,
                        &mut region,
                        &len,
                        &source[RESULT_TAG.len()..],
                    )?;
                    let zero = uint.glue().constant(&mut region, Fp::ZERO)?;
                    (prefix, vec![zero; 8])
                };
                let mut out = vec![len.word().clone(), prefix.height.word().clone()];
                out.extend_from_slice(&prefix.event_root);
                out.push(prefix.event_count.word().clone());
                out.extend(state);
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

fn prefix_public(frame: &[u8], hash: bool) -> Vec<Vec<Fp>> {
    let fixture = native_fixture();
    let mut output = vec![Fp::from(frame.len() as u64), Fp::from(2)];
    output.extend(
        fixture_bytes(&fixture, "event_commitment_root_hex")
            .into_iter()
            .map(|byte| Fp::from(u64::from(byte))),
    );
    output.push(Fp::ONE);
    let mut state = [0; 8];
    if hash {
        state = initial_words();
        let mut transcript = RESULT_TAG.to_vec();
        transcript.extend_from_slice(frame);
        for (i, block) in transcript[..384].chunks_exact(128).enumerate() {
            state = native_compress(
                state,
                block.try_into().unwrap(),
                ((i + 1) * 128) as u128,
                false,
            );
        }
    }
    output.extend(state.into_iter().map(Fp::from));
    vec![output]
}

#[test]
fn native_result_prefix_is_pinned_to_exact_codec_and_original_hashed_blocks() {
    let fixture = native_fixture();
    let frame = fixture_bytes(&fixture, "result_preimage_hex");
    assert_eq!(frame.len(), 10_242);
    assert_eq!(
        fixture
            .get("result_alignment")
            .and_then(norito::json::Value::as_u64),
        Some(8)
    );
    assert_eq!(
        fixture_bytes(&fixture, "result_codec_identity_hex"),
        RESULT_CODEC_ID
    );
    let execution = fixture_bytes(&fixture, "execution_commitment_frame_hex");
    assert_eq!(&frame[51..393], &execution[40..]);
    let circuit = PrefixCircuit {
        frame,
        hash: true,
        finish_early: false,
        known: true,
    };
    let public = prefix_public(&circuit.frame, true);
    assert!(
        check_circuit(&circuit, 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&circuit, 16, Some(&public)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    let early = PrefixCircuit {
        finish_early: true,
        ..circuit
    };
    assert!(
        !check_circuit(&early, 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn result_prefix_rejects_changed_framing_lengths_and_detached_event_fields() {
    let frame = fixture_bytes(&native_fixture(), "result_preimage_hex");
    let public = prefix_public(&frame, false);
    let circuit = PrefixCircuit {
        frame,
        hash: false,
        finish_early: false,
        known: true,
    };
    assert!(
        check_circuit(&circuit, 11, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    for offset in [
        0, 4, 5, 6, 21, 22, 23, 30, 39, 40, 41, 49, 50, 51, 84, 117, 150, 183, 216, 217, 218, 219,
        220, 251, 252, 253,
    ] {
        let mut forged = circuit.clone();
        forged.frame[offset] ^= 1;
        assert!(
            !check_circuit(&forged, 11, &public, CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            "byte {offset}"
        );
    }
    for range in [41..49, 253..261] {
        let mut forged = circuit.clone();
        forged.frame[range].fill(0);
        let mut zero_public = public.clone();
        let index = if forged.frame[41] == 0 { 1 } else { 34 };
        zero_public[0][index] = Fp::ZERO;
        assert!(
            !check_circuit(&forged, 11, &zero_public, CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
    }
}

#[derive(Clone)]
struct NativeStepCircuit {
    frame_len: usize,
    processed: usize,
    state: [u64; 8],
    block: [u8; 128],
}

impl Circuit<Fp> for NativeStepCircuit {
    type Config = StreamConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        stream_config(meta, 19)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let output = layouter.assign_region(
            || "native result hash step",
            |mut region| {
                let values = self.state.map(|value| Value::known(Fp::from(value)));
                let state: [Word<Fp>; 8] =
                    glue.witnesses(&mut region, &values)?.try_into().unwrap();
                let mut uint = UintChip::new(&mut glue, &mut range);
                let frame_len =
                    uint.assign::<32>(&mut region, Value::known(self.frame_len as u128))?;
                let processed =
                    uint.assign::<32>(&mut region, Value::known(self.processed as u128))?;
                // Only this test can construct a continuation. Public state/counter
                // outputs are passed unchanged to the next strict-checked step below.
                let stream = if self.processed == 0 {
                    let initial =
                        ResultHashStream::start(&mut uint, &mut blake, &mut region, &frame_len)?;
                    let initial_words = blake.state_words(&mut region, initial.state())?;
                    for (a, b) in state.iter().zip(&initial_words) {
                        GlueChip::assert_equal(&mut region, a, b.word())?;
                    }
                    initial
                } else {
                    ResultHashStream {
                        state: blake.state_from_words(&mut region, &state)?,
                        processed: processed.clone(),
                        total: uint.checked_add_constant(
                            &mut region,
                            &frame_len,
                            RESULT_TAG.len() as u128,
                        )?,
                        frame_len: frame_len.clone(),
                    }
                };
                let values = self
                    .block
                    .map(|byte| Value::known(Fp::from(u64::from(byte))));
                let block: [Word<Fp>; 128] = uint
                    .glue()
                    .witnesses(&mut region, &values)?
                    .try_into()
                    .unwrap();
                let next = stream.absorb(&mut uint, &mut blake, &mut region, &block)?;
                let next_state = blake.state_words(&mut region, next.state())?;
                let mut out = vec![
                    frame_len.word().clone(),
                    processed.word().clone(),
                    next.processed().word().clone(),
                ];
                out.extend(state);
                out.extend(next_state.into_iter().map(|word| word.word().clone()));
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[test]
fn complete_native_result_fixture_hash_matches_all_bounded_stream_steps() {
    let fixture = native_fixture();
    let frame = fixture_bytes(&fixture, "result_preimage_hex");
    let mut transcript = RESULT_TAG.to_vec();
    transcript.extend_from_slice(&frame);
    let total = transcript.len();
    transcript.resize(total.div_ceil(128) * 128, 0);
    let mut state = initial_words();
    for (index, bytes) in transcript.chunks_exact(128).enumerate() {
        let processed = index * 128;
        let next = (processed + 128).min(total);
        let block: [u8; 128] = bytes.try_into().unwrap();
        let expected = native_compress(state, &block, next as u128, next == total);
        let circuit = NativeStepCircuit {
            frame_len: frame.len(),
            processed,
            state,
            block,
        };
        let mut public = vec![
            Fp::from(frame.len() as u64),
            Fp::from(processed as u64),
            Fp::from(next as u64),
        ];
        public.extend(state.into_iter().chain(expected).map(Fp::from));
        assert!(
            check_circuit(&circuit, 14, &[public], CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            "native result block {index}"
        );
        state = expected;
    }
    let mut digest: Vec<_> = state[..4]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    digest[31] |= 1;
    assert_eq!(digest, fixture_bytes(&fixture, "result_hash_hex"));
}
