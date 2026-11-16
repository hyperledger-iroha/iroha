//! Integrity of the frozen IVM baseline v1 fixtures in `tests/fixtures/baseline_v1`.
//!
//! The fixtures were recorded on the pre-C10 anchor commit by
//! `scripts/ivm_baseline_v1/capture.py` (`specs/runtime/ivm_baseline_and_c10_profiles.md`,
//! phase 1). These tests check the files themselves: every manifest hash, that the descriptor
//! bytes hash to the installed baseline's identities, and that the execution oracle is complete
//! for the artifact corpus. They deliberately never ask today's interpreter or today's
//! `SyscallPolicy::AbiV1` descriptor for an answer: the baseline engine that must reproduce the
//! oracle arrives in a later phase.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};

/// The pre-C10 anchor commit (`c5511ebfc4~1`).
const ANCHOR_COMMIT: &str = "77600a4840bd5f37f167c46b11b9f5ab73c97ae2";
/// `ABI_V1_HASH_GOLDEN` at the anchor.
const ABI_V1_HASH: &str = "7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7";
/// `ivm::gas::schedule_hash()` at the anchor.
const GAS_SCHEDULE_HASH: &str = "af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f";
/// HostProfile `abi.v1` in the anchor's `specs/runtime_baseline_catalog.json`.
const HOST_PROFILE_ID: &str = "d50c38a8d0d0a229f54a43f9f7568fd45561774cb58f40668eb703ae79611b99";
/// Baseline selection in the anchor's `specs/runtime_baseline_catalog.json`.
const BASELINE_SELECTION: &str = "56c2328657d63a48e18c7d1bac5ae8f2406e6e71249a630ece7b504d0f9cd491";
/// `iroha_data_model::runtime::definition::DEFINITION_ID_DOMAIN` at the anchor.
const DEFINITION_ID_DOMAIN: &[u8] = b"iroha:runtime:definition:v1\0";
/// Size of the baseline program header (C10 uses 48 bytes).
const BASELINE_HEADER_SIZE: usize = 49;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/baseline_v1")
}
fn read(relative: &str) -> Vec<u8> {
    let path = fixture_root().join(relative);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}
fn json(relative: &str) -> Value {
    norito::json::from_slice(&read(relative))
        .unwrap_or_else(|error| panic!("{relative} is not JSON: {error}"))
}
fn object<'a>(value: &'a Value, what: &str) -> &'a Map {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{what} must be an object"))
}
fn field<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |value, key| {
        object(value, key)
            .get(*key)
            .unwrap_or_else(|| panic!("missing `{}`", path.join(".")))
    })
}
fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    field(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("`{}` must be a string", path.join(".")))
}
fn number(value: &Value, path: &[&str]) -> u64 {
    field(value, path)
        .as_u64()
        .unwrap_or_else(|| panic!("`{}` must be an unsigned integer", path.join(".")))
}
fn array<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    field(value, path)
        .as_array()
        .unwrap_or_else(|| panic!("`{}` must be an array", path.join(".")))
}
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

const BLAKE2B_IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];
const BLAKE2B_SIGMA: [[usize; 16]; 10] = [
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
fn blake2b_mix(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}
fn blake2b_compress(state: &mut [u64; 8], block: &[u8; 128], counter: u128, last: bool) {
    let mut m = [0_u64; 16];
    for (word, chunk) in m.iter_mut().zip(block.chunks_exact(8)) {
        *word = u64::from_le_bytes(chunk.try_into().expect("eight-byte chunk"));
    }
    let mut v = [0_u64; 16];
    v[..8].copy_from_slice(state);
    v[8..].copy_from_slice(&BLAKE2B_IV);
    v[12] ^= counter as u64;
    v[13] ^= (counter >> 64) as u64;
    if last {
        v[14] = !v[14];
    }
    for round in 0..12 {
        let s = &BLAKE2B_SIGMA[round % 10];
        blake2b_mix(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        blake2b_mix(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        blake2b_mix(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        blake2b_mix(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        blake2b_mix(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        blake2b_mix(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        blake2b_mix(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        blake2b_mix(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }
    for index in 0..8 {
        state[index] ^= v[index] ^ v[index + 8];
    }
}
/// Unkeyed BLAKE2b with a 32-byte digest (RFC 7693), implemented here so the check owes
/// nothing to the code under test.
fn blake2b_256(chunks: &[&[u8]]) -> [u8; 32] {
    let data = chunks.concat();
    let mut state = BLAKE2B_IV;
    state[0] ^= 0x0101_0000 ^ 32;
    let mut offset = 0;
    while data.len() - offset > 128 {
        let block: &[u8; 128] = data[offset..offset + 128].try_into().expect("full block");
        offset += 128;
        blake2b_compress(&mut state, block, offset as u128, false);
    }
    let mut block = [0_u8; 128];
    block[..data.len() - offset].copy_from_slice(&data[offset..]);
    blake2b_compress(&mut state, &block, data.len() as u128, true);
    let mut out = [0_u8; 32];
    for (bytes, word) in out.chunks_exact_mut(8).zip(state) {
        bytes.copy_from_slice(&word.to_le_bytes());
    }
    out
}
/// The anchor's `iroha_crypto::Hash::new`: BLAKE2b-256 with the least significant bit of the
/// final byte set.
fn iroha_hash_v1(chunks: &[&[u8]]) -> [u8; 32] {
    let mut hash = blake2b_256(chunks);
    hash[31] |= 1;
    hash
}

#[test]
fn local_hash_is_blake2b_256_and_matches_the_production_hash() {
    assert_eq!(
        hex::encode(blake2b_256(&[b"abc"])),
        "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319"
    );
    // The hash function itself did not change with C10; only what it is applied to did.
    let long = (0..1000_u32).map(|index| index as u8).collect::<Vec<_>>();
    for input in [
        &b""[..],
        b"abc",
        &long[..128],
        &long[..129],
        &long[..256],
        &long,
    ] {
        assert_eq!(
            iroha_hash_v1(&[input]),
            *iroha_crypto::Hash::new(input).as_ref(),
            "{} bytes",
            input.len()
        );
    }
    assert_eq!(
        iroha_hash_v1(&[&long[..300], &long[300..]]),
        iroha_hash_v1(&[&long])
    );
}

fn manifest_files() -> BTreeMap<String, (u64, String)> {
    let manifest = json("manifest.json");
    array(&manifest, &["files"])
        .iter()
        .map(|entry| {
            (
                text(entry, &["path"]).to_owned(),
                (
                    number(entry, &["bytes"]),
                    text(entry, &["sha256"]).to_owned(),
                ),
            )
        })
        .collect()
}
fn files_on_disk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            files_on_disk(&path, root, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("path below the fixture root");
            out.insert(
                relative
                    .components()
                    .map(|component| component.as_os_str().to_str().expect("UTF-8 fixture path"))
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}

#[test]
fn manifest_names_the_anchor_and_hashes_every_fixture_file() {
    let manifest = json("manifest.json");
    assert_eq!(number(&manifest, &["format"]), 1);
    assert_eq!(text(&manifest, &["anchor_commit"]), ANCHOR_COMMIT);
    assert_eq!(text(&manifest, &["frozen", "abi_hash"]), ABI_V1_HASH);
    assert_eq!(
        text(&manifest, &["frozen", "gas_schedule_hash"]),
        GAS_SCHEDULE_HASH
    );
    assert_eq!(
        text(&manifest, &["frozen", "host_profile_id"]),
        HOST_PROFILE_ID
    );
    assert_eq!(
        text(&manifest, &["frozen", "selection"]),
        BASELINE_SELECTION
    );

    let listed = manifest_files();
    let root = fixture_root();
    let mut present = BTreeSet::new();
    files_on_disk(&root, &root, &mut present);
    present.remove("manifest.json");
    assert_eq!(
        present,
        listed.keys().cloned().collect::<BTreeSet<_>>(),
        "the manifest must list exactly the files of the fixture directory"
    );
    for (path, (bytes, sha256)) in &listed {
        let content = read(path);
        assert_eq!(content.len() as u64, *bytes, "size of {path}");
        assert_eq!(&sha256_hex(&content), sha256, "SHA-256 of {path}");
    }
}

#[test]
fn descriptor_bytes_hash_to_the_installed_baseline_identities() {
    let frozen = json("descriptors/frozen_values.json");
    assert_eq!(text(&frozen, &["anchor_commit"]), ANCHOR_COMMIT);

    let abi_descriptor = read("descriptors/abi_v1.descriptor.bin");
    assert_eq!(hex::encode(iroha_hash_v1(&[&abi_descriptor])), ABI_V1_HASH);
    assert_eq!(text(&frozen, &["abi_v1", "abi_hash"]), ABI_V1_HASH);
    assert!(
        abi_descriptor.starts_with(b"IVM_ABI_V1_FULL_SURFACE\0")
            || abi_descriptor
                .windows(24)
                .any(|window| window == b"IVM_ABI_V1_FULL_SURFACE\0"),
        "the ABI descriptor carries its surface domain"
    );
    assert_eq!(
        field(&frozen, &["abi_v1_runtime_lifecycle", "exists_at_anchor"]).as_bool(),
        Some(false),
        "the anchor has a single syscall policy"
    );

    let gas_descriptor = read("descriptors/gas_schedule_v1.descriptor.bin");
    assert_eq!(
        hex::encode(iroha_hash_v1(&[&gas_descriptor])),
        GAS_SCHEDULE_HASH
    );
    assert_eq!(
        text(&frozen, &["gas_schedule_v1", "schedule_hash"]),
        GAS_SCHEDULE_HASH
    );

    // HostProfile `abi.v1`: the definition id is the hash of the domain, the format version
    // and kind tag, the little-endian descriptor length and the canonical descriptor bytes.
    let host_profile = read("descriptors/host_profile_abi_v1.descriptor.bin");
    let preimage = field(&frozen, &["host_profile_abi_v1", "definition_id_preimage"]);
    assert_eq!(
        hex::decode(text(preimage, &["domain_hex"])).expect("domain hex"),
        DEFINITION_ID_DOMAIN
    );
    let version = u8::try_from(number(preimage, &["format_version"])).expect("format version");
    let kind = u8::try_from(number(preimage, &["kind_tag"])).expect("kind tag");
    let id = iroha_hash_v1(&[
        DEFINITION_ID_DOMAIN,
        &[version, kind],
        &(host_profile.len() as u64).to_le_bytes(),
        &host_profile,
    ]);
    assert_eq!(hex::encode(id), HOST_PROFILE_ID);
    assert_eq!(
        text(&frozen, &["host_profile_abi_v1", "definition_id"]),
        HOST_PROFILE_ID
    );
    assert_eq!(
        text(&frozen, &["host_profile_abi_v1", "descriptor_hex"]),
        hex::encode(&host_profile)
    );
    // The descriptor binds exactly ABI version 1, the ABI hash and the gas schedule hash: its
    // payload ends with the little-endian `u16` version and the two length-prefixed hashes.
    let mut bound = vec![1, 0, 32];
    bound.extend(hex::decode(ABI_V1_HASH).expect("ABI hash hex"));
    bound.push(32);
    bound.extend(hex::decode(GAS_SCHEDULE_HASH).expect("gas schedule hash hex"));
    assert!(
        host_profile.ends_with(&bound),
        "the HostProfile descriptor ends with abi_version, abi_hash and metering"
    );
    assert_eq!(number(&frozen, &["host_profile_abi_v1", "abi_version"]), 1);

    // The anchor's catalog pins the same selection and HostProfile.
    let catalog = json("descriptors/runtime_baseline_catalog.json");
    assert_eq!(text(&catalog, &["selection"]), BASELINE_SELECTION);
    let host_profiles: Vec<_> = array(&catalog, &["definitions"])
        .iter()
        .filter(|definition| text(definition, &["kind"]) == "HostProfile")
        .collect();
    assert_eq!(host_profiles.len(), 1);
    assert_eq!(text(host_profiles[0], &["name"]), "abi.v1");
    assert_eq!(text(host_profiles[0], &["id"]), HOST_PROFILE_ID);
}

#[test]
fn frozen_header_opcode_and_wire_tables_are_the_pre_c10_ones() {
    let frozen = json("descriptors/frozen_values.json");
    assert_eq!(
        number(&frozen, &["header", "size"]),
        BASELINE_HEADER_SIZE as u64
    );
    let header = hex::decode(text(&frozen, &["header", "default_header_hex"])).expect("header hex");
    assert_eq!(header.len(), BASELINE_HEADER_SIZE);
    assert_eq!(&header[..4], b"IVM\0");
    assert_eq!(hex::encode(&header[17..]), ABI_V1_HASH);
    assert_eq!(number(&frozen, &["header", "vector_length_max"]), 64);

    // Opcode families C10 removed are part of the baseline instruction set.
    let opcodes = array(&frozen, &["opcodes"]);
    let valid: BTreeMap<&str, &str> = opcodes
        .iter()
        .filter(|opcode| field(opcode, &["valid"]).as_bool() == Some(true))
        .map(|opcode| (text(opcode, &["name"]), text(opcode, &["family"])))
        .collect();
    for name in [
        "VADD32",
        "VADD64",
        "VAND",
        "VXOR",
        "VOR",
        "VROT32",
        "SETVL",
        "PARBEGIN",
        "PAREND",
        "SHA256BLOCK",
        "SHA3BLOCK",
        "POSEIDON2",
        "POSEIDON6",
        "AESENC",
        "AESDEC",
        "BLAKE2S",
        "ED25519VERIFY",
        "ED25519BATCHVERIFY",
        "ECDSAVERIFY",
        "DILITHIUMVERIFY",
        "ASSERT",
        "ASSERT_EQ",
        "FADD",
        "FSUB",
        "FMUL",
        "FINV",
        "ASSERT_RANGE",
        "LOAD128",
        "STORE128",
    ] {
        assert!(valid.contains_key(name), "{name} is a baseline opcode");
    }
    assert_eq!(valid.len(), 90, "valid baseline opcodes");
    assert!(
        opcodes
            .iter()
            .filter(|opcode| field(opcode, &["valid"]).as_bool() == Some(true))
            .all(|opcode| field(opcode, &["gas"]).as_u64().is_some()),
        "every valid baseline opcode has a scheduled gas cost"
    );

    // Hardware-probe syscall numbers were never part of the surface.
    let syscalls: BTreeSet<u64> = array(&frozen, &["syscalls"])
        .iter()
        .map(|syscall| number(syscall, &["number"]))
        .collect();
    assert!(!syscalls.contains(&0xF2) && !syscalls.contains(&0xF3));
    assert!(!array(&frozen, &["pointer_types"]).is_empty());

    // Original wire tags of the frozen payload enums.
    let tags = |name: &str| -> BTreeMap<String, u64> {
        array(&frozen, &["wire_tags", name])
            .iter()
            .map(|entry| {
                (
                    text(entry, &["variant"]).to_owned(),
                    number(entry, &["wire_tag"]),
                )
            })
            .collect()
    };
    let admission = tags("IvmAdmissionError");
    assert_eq!(admission.len(), 18);
    assert_eq!(admission["VectorLengthTooLarge"], 5);
    assert_eq!(admission["MaxCyclesExceedsUpperBound"], 6);
    assert_eq!(admission["MaxCyclesExceedsFuel"], 7);
    assert_eq!(admission["GenericSyscallNotAllowed"], 17);
    assert!(!admission.contains_key("MaxCyclesWithoutZk"));
    let faults = tags("IvmFaultKindV1");
    assert_eq!(faults.len(), 30);
    assert_eq!(faults["AssertionFailed"], 12);
    assert_eq!(faults["InvalidVectorLength"], 15);
    assert_eq!(faults["MissingHalt"], 16);
    assert_eq!(faults["VectorExtensionDisabled"], 17);
    assert_eq!(faults["ZkExtensionDisabled"], 18);
    assert_eq!(faults["CallDepthExceeded"], 29);
    assert!(!faults.contains_key("ControlFlowViolation"));
    let numeric = tags("NumericFaultV1");
    assert_eq!(
        numeric.values().copied().collect::<BTreeSet<_>>(),
        (1..=13).collect::<BTreeSet<_>>()
    );
    assert!(!numeric.contains_key("RegisterIntegerOverflow"));
}

fn oracle_records() -> Vec<Value> {
    let text = String::from_utf8(read("oracle/oracle.jsonl")).expect("the oracle is UTF-8");
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            norito::json::from_str(line)
                .unwrap_or_else(|error| panic!("oracle line {}: {error}", index + 1))
        })
        .collect()
}

#[test]
fn oracle_parses_and_is_complete_for_the_artifact_corpus() {
    let listed = manifest_files();
    let records = oracle_records();
    let corpus = json("oracle/corpus.json");
    assert_eq!(text(&corpus, &["anchor_commit"]), ANCHOR_COMMIT);
    assert_eq!(number(&corpus, &["cases"]), records.len() as u64);
    assert!(
        array(&corpus, &["problems"]).is_empty(),
        "the capture recorded no nondeterminism or panic"
    );

    let mut ids = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for record in &records {
        let id = text(record, &["id"]);
        assert!(ids.insert(id.to_owned()), "duplicate oracle case {id}");
        let artifact = text(record, &["artifact"]);
        let (_, sha256) = listed
            .get(artifact)
            .unwrap_or_else(|| panic!("{id}: {artifact} is not in the manifest"));
        assert_eq!(text(record, &["artifact_sha256"]), sha256, "{id}");
        referenced.insert(artifact.to_owned());

        // Setup: everything a replay needs.
        let gas_limit = number(record, &["setup", "gas_limit"]);
        assert!(
            ["default", "core", "wsv", "wsv_contract"].contains(&text(record, &["setup", "host"])),
            "{id}: host"
        );
        for step in array(record, &["setup", "steps"]) {
            assert!(
                [
                    "set_register",
                    "tag_register",
                    "store_bytes",
                    "alloc_input_tlv",
                    "select_entrypoint",
                    "set_max_cycles",
                ]
                .contains(&text(step, &["op"])),
                "{id}: setup step"
            );
        }
        number(record, &["setup", "policy", "max_cycles_upper_bound"]);
        number(record, &["setup", "policy", "fuel"]);

        // Admission: the interpreter's static verdict and the Core projection with its tag.
        let vm_admitted = field(record, &["admission", "vm", "ok"]).as_bool() == Some(true);
        if !vm_admitted {
            text(record, &["admission", "vm", "error", "debug"]);
        }
        if field(record, &["admission", "core", "ok"]).as_bool() == Some(true) {
            assert!(
                vm_admitted,
                "{id}: Core admits only what the interpreter loads"
            );
            number(record, &["admission", "core", "effective_cycles"]);
        } else {
            let tag = number(record, &["admission", "core", "error", "wire_tag"]);
            assert!(tag <= 17, "{id}: IvmAdmissionError tag");
            let bare = hex::decode(text(record, &["admission", "core", "error", "bare_hex"]))
                .expect("admission error hex");
            assert_eq!(
                u64::from(u32::from_le_bytes(bare[..4].try_into().expect("tag bytes"))),
                tag,
                "{id}: the bare encoding starts with the wire tag"
            );
        }

        // Run: a refusal to load, or the complete final state.
        let run = field(record, &["run"]);
        if object(run, "run").contains_key("not_run") {
            text(run, &["error", "debug"]);
            assert!(record.as_object().expect("record")["trace"].is_null());
            continue;
        }
        assert!(vm_admitted, "{id}: only admitted artifacts run");
        assert!(
            !object(run, "run").contains_key("panic"),
            "{id}: the anchor did not panic"
        );
        if field(run, &["result", "ok"]).as_bool() != Some(true) {
            text(run, &["result", "error", "debug"]);
            let kind = field(run, &["result", "fault_kind"]);
            if !kind.is_null() {
                assert!(
                    number(kind, &["wire_tag"]) <= 29,
                    "{id}: IvmFaultKindV1 tag"
                );
                text(kind, &["bare_hex"]);
            }
        }
        number(run, &["pc"]);
        number(run, &["cycles"]);
        let gas_remaining = number(run, &["gas_remaining"]);
        assert!(gas_remaining <= gas_limit, "{id}: gas accounting");
        assert_eq!(
            number(run, &["summary", "gas_used"]),
            gas_limit - gas_remaining,
            "{id}: summary gas"
        );
        for register in array(run, &["registers"]) {
            let pair = register.as_array().expect("register pair");
            assert!(
                pair.len() == 2 && pair[0].as_u64().is_some_and(|index| index < 256),
                "{id}: the baseline register file has 256 registers"
            );
        }
        for commitment in [
            &["register_root"][..],
            &["summary", "code_hash"],
            &["summary", "final_register_root"],
            &["summary", "final_memory_root"],
            &["summary", "output_hash"],
        ] {
            assert_eq!(text(run, commitment).len(), 64, "{id}: {commitment:?}");
        }
        assert_eq!(
            text(run, &["register_root"]),
            text(run, &["summary", "final_register_root"]),
            "{id}"
        );
        for call in array(run, &["syscalls"]) {
            number(call, &["number"]);
            if text(call, &["event"]) == "call" {
                assert_eq!(array(call, &["args"]).len(), 8, "{id}: syscall arguments");
                assert_eq!(array(call, &["ret"]).len(), 8, "{id}: syscall results");
            }
        }
        let trace = field(record, &["trace"]);
        if object(record, "record").contains_key("trace_note") {
            // A program that reads its own trace logs is recorded without the traced run.
            assert!(trace.is_null(), "{id}: untraced case");
        } else {
            number(trace, &["pc_count"]);
            assert_eq!(text(trace, &["pc_sha256"]).len(), 64, "{id}");
        }
        assert!(
            !object(record, "record").contains_key("trace_divergence"),
            "{id}: PC tracing does not change the result"
        );
    }

    // Every artifact has at least one oracle case, and admitted ones carry the baseline header.
    let artifacts: BTreeSet<String> = listed
        .keys()
        .filter(|path| path.starts_with("artifacts/"))
        .cloned()
        .collect();
    assert_eq!(referenced, artifacts, "artifacts without an oracle case");
    assert_eq!(number(&corpus, &["artifacts"]), artifacts.len() as u64);
    for record in &records {
        if field(record, &["admission", "vm", "ok"]).as_bool() == Some(true) {
            let bytes = read(text(record, &["artifact"]));
            assert!(bytes.len() >= BASELINE_HEADER_SIZE);
            assert_eq!(&bytes[..6], b"IVM\0\x01\x01");
            assert_eq!(bytes[16], 1, "ABI version");
            assert_eq!(hex::encode(&bytes[17..BASELINE_HEADER_SIZE]), ABI_V1_HASH);
        }
    }

    // Every Kotodama source is kept with the artifact the anchor compiler produced for it, or
    // with the diagnostic it was refused with.
    for source in array(&corpus, &["kotodama_sources"]) {
        let path = text(source, &["source"]);
        assert_eq!(
            text(source, &["source_sha256"]),
            listed[path].1,
            "source {path}"
        );
        if object(source, "source").contains_key("artifact") {
            let artifact = text(source, &["artifact"]);
            assert_eq!(text(source, &["artifact_sha256"]), listed[artifact].1);
            assert!(referenced.contains(artifact));
        } else {
            text(source, &["compile_error"]);
        }
    }
}

#[test]
fn corpus_reaches_every_baseline_opcode_and_the_changed_fault_kinds() {
    let corpus = json("oracle/corpus.json");
    assert!(
        array(&corpus, &["valid_opcodes_never_executed"]).is_empty(),
        "every valid baseline opcode executes in at least one oracle case: {:?}",
        array(&corpus, &["valid_opcodes_never_executed"])
    );
    let fault_kinds = object(field(&corpus, &["fault_kinds"]), "fault_kinds");
    for kind in [
        "OutOfGas",
        "MemoryAccessViolation",
        "MisalignedAccess",
        "InvalidOpcode",
        "UnknownSyscall",
        "AssertionFailed",
        "ExceededMaxCycles",
        "InvalidVectorLength",
        "MissingHalt",
        "VectorExtensionDisabled",
        "ZkExtensionDisabled",
        "PrivacyViolation",
        "RegisterOutOfBounds",
    ] {
        assert!(
            fault_kinds.contains_key(kind),
            "no oracle case faults with {kind}"
        );
    }
    let admission = object(field(&corpus, &["core_admission"]), "core_admission");
    for refusal in [
        "MissingMaxCycles",
        "UnsupportedVersion",
        "UnsupportedFeatureBits",
        "UnsupportedAbiVersion",
        "VectorLengthTooLarge",
        "MaxCyclesExceedsUpperBound",
        "MaxCyclesExceedsFuel",
        "BytecodeDecodingFailed",
        "ArtifactAbiHashMismatch",
        "GenericSyscallNotAllowed",
    ] {
        assert!(
            admission.contains_key(refusal),
            "no oracle case is refused with {refusal}"
        );
    }
    let hosts = object(field(&corpus, &["cases_by_host"]), "cases_by_host");
    for host in ["default", "core", "wsv", "wsv_contract"] {
        assert!(
            hosts.contains_key(host),
            "no oracle case uses the {host} host"
        );
    }
}
