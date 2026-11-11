//! Thread-CPU-time profile of the binary codec on representative values.
//!
//! Wall-clock benchmarks are unreliable on a loaded machine. This example
//! reports per-operation thread CPU time, which preemption does not inflate.
//! Run with `cargo run --release -p norito --example codec_cpu_profile`.
//! Pass `loop <case>` to spin one case for an external sampling profiler.
//!
//! `decode` is the ordinary framed decoder; `canonical` is `decode_canonical`,
//! which also proves the bytes are the value's exact canonical encoding.
use std::{collections::BTreeMap, hint::black_box};

use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    core::{Archived, DeserializePayload, Encoder, Error, SerializePayload},
};

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Flat")]
struct Flat {
    id: u64,
    name: String,
    values: Vec<u32>,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Leaf")]
struct Leaf {
    account: [u8; 32],
    asset: String,
    amount: u128,
    memo: Option<String>,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Action")]
enum Action {
    Transfer { from: Leaf, to: Leaf },
    Mint(Leaf),
    Note(String),
    Noop,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Payload")]
struct Payload {
    chain: String,
    authority: [u8; 32],
    created_ms: u64,
    ttl_ms: Option<u64>,
    nonce: Option<u32>,
    actions: Vec<Action>,
    metadata: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Signed")]
struct Signed {
    signature: Vec<u8>,
    payload: Payload,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Block")]
struct Block {
    height: u64,
    previous: [u8; 32],
    transactions: Vec<Signed>,
}

/// A value carried as a nested frame, the way `InstructionBox` carries every
/// instruction: the pair `(wire_id, frame)`, with the frame stored as a raw
/// byte vector.
#[derive(Clone, PartialEq, Debug)]
struct Framed(Leaf);

const WIRE_ID: &str = "norito.example.transfer";

impl SerializePayload for Framed {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        norito::core::write_frame_with_prefix(&self.0, writer, |writer, frame_len| {
            let name_field = norito::core::len_prefix_len(WIRE_ID.len()) + WIRE_ID.len();
            norito::core::write_len(writer, name_field as u64)?;
            norito::core::write_len(writer, WIRE_ID.len() as u64)?;
            writer.write_all(WIRE_ID.as_bytes())?;
            norito::core::write_len(writer, (8 + frame_len) as u64)?;
            norito::core::write_seq_len(writer, frame_len as u64)?;
            Ok(())
        })
    }
}

impl<'a> DeserializePayload<'a> for Framed {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("framed value")
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        let bytes = norito::core::payload_slice_from_ptr(std::ptr::from_ref(archived).cast())?;
        let (name_field, used) = norito::core::read_len_from_slice(bytes)?;
        let mut offset = used + name_field;
        let (frame_field, used) = norito::core::read_len_from_slice(&bytes[offset..])?;
        offset += used;
        let field = bytes
            .get(offset..offset + frame_field)
            .ok_or(Error::LengthMismatch)?;
        let (frame_len, used) = norito::core::read_seq_len_slice(field)?;
        let frame = field
            .get(used..used + frame_len)
            .ok_or(Error::LengthMismatch)?;
        let leaf = norito::decode_from_bytes::<Leaf>(frame)?;
        norito::core::note_payload_access(bytes, offset + frame_field);
        Ok(Self(leaf))
    }
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.Transaction")]
struct Transaction {
    authority: [u8; 32],
    instructions: Vec<Framed>,
    nonce: u32,
}

#[derive(Clone, PartialEq, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "norito.example.profile.TxBlock")]
struct TxBlock {
    height: u64,
    transactions: Vec<Transaction>,
}

fn tx_block() -> TxBlock {
    TxBlock {
        height: 9,
        transactions: (0..32_u8)
            .map(|seed| Transaction {
                authority: [seed; 32],
                instructions: (0..4).map(|offset| Framed(leaf(seed + offset))).collect(),
                nonce: u32::from(seed),
            })
            .collect(),
    }
}

fn flat() -> Flat {
    Flat {
        id: 42,
        name: "benchmark".repeat(10),
        values: (0..100).collect(),
    }
}

fn leaf(seed: u8) -> Leaf {
    Leaf {
        account: [seed; 32],
        asset: format!("asset{seed}#domain"),
        amount: u128::from(seed) * 1_000_000_007,
        memo: seed.is_multiple_of(2).then(|| format!("memo {seed}")),
    }
}

fn signed(seed: u8) -> Signed {
    Signed {
        signature: vec![seed; 64],
        payload: Payload {
            chain: "00000000-0000-0000-0000-000000000000".to_owned(),
            authority: [seed; 32],
            created_ms: 1_700_000_000_000 + u64::from(seed),
            ttl_ms: Some(100_000),
            nonce: Some(u32::from(seed)),
            actions: vec![
                Action::Transfer {
                    from: leaf(seed),
                    to: leaf(seed.wrapping_add(1)),
                },
                Action::Mint(leaf(seed.wrapping_add(2))),
                Action::Note("settlement".to_owned()),
                Action::Noop,
            ],
            metadata: (0..4)
                .map(|index| (format!("key{index}"), format!("value{index}")))
                .collect(),
        },
    }
}

fn block() -> Block {
    Block {
        height: 7,
        previous: [9; 32],
        transactions: (0..32).map(signed).collect(),
    }
}

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

unsafe extern "C" {
    fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
}

#[cfg(target_os = "macos")]
const CLOCK_THREAD_CPUTIME_ID: i32 = 16;
#[cfg(not(target_os = "macos"))]
const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

fn thread_cpu_nanos() -> u128 {
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: `time` is a valid, writable `timespec` for the duration of the call.
    let status = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &raw mut time) };
    assert_eq!(status, 0, "thread CPU clock is available");
    u128::try_from(time.seconds).expect("non-negative seconds") * 1_000_000_000
        + u128::try_from(time.nanoseconds).expect("non-negative nanoseconds")
}

/// Best per-iteration thread CPU time across several batches.
fn measure(mut operation: impl FnMut()) -> f64 {
    for _ in 0..200 {
        operation();
    }
    let mut iterations = 64_u32;
    loop {
        let start = thread_cpu_nanos();
        for _ in 0..iterations {
            operation();
        }
        if thread_cpu_nanos() - start > 20_000_000 || iterations >= 1 << 22 {
            break;
        }
        iterations *= 2;
    }
    let mut best = f64::INFINITY;
    for _ in 0..9 {
        let start = thread_cpu_nanos();
        for _ in 0..iterations {
            operation();
        }
        let elapsed = (thread_cpu_nanos() - start) as f64 / f64::from(iterations);
        best = best.min(elapsed);
    }
    best
}

fn report<T>(label: &str, value: &T)
where
    T: NoritoSerialize + PartialEq + std::fmt::Debug + for<'de> NoritoDeserialize<'de>,
{
    let bytes = norito::to_bytes(value).expect("encode");
    let decoded: T = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(&decoded, value, "{label} roundtrip");
    let encode = measure(|| {
        black_box(norito::to_bytes(black_box(value)).expect("encode"));
    });
    let decode = measure(|| {
        black_box(norito::decode_from_bytes::<T>(black_box(&bytes)).expect("decode"));
    });
    let canonical_bytes = norito::encode_canonical(value).expect("canonical encode");
    let canonical = measure(|| {
        black_box(norito::decode_canonical::<T>(black_box(&canonical_bytes)).expect("canonical"));
    });
    let size = bytes.len();
    println!(
        "{label:<8} bytes={size:<6} encode={encode:>9.1} ns ({:>5.2} ns/B)  decode={decode:>9.1} ns ({:>5.2} ns/B)  canonical={canonical:>9.1} ns ({:>5.2} ns/B)",
        encode / size as f64,
        decode / size as f64,
        canonical / size as f64,
    );
}

fn spin<T>(value: &T, decode: bool) -> !
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let bytes = norito::to_bytes(value).expect("encode");
    loop {
        if decode {
            black_box(norito::decode_from_bytes::<T>(black_box(&bytes)).expect("decode"));
        } else {
            black_box(norito::to_bytes(black_box(value)).expect("encode"));
        }
    }
}

fn spin_canonical<T>(value: &T) -> !
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let bytes = norito::encode_canonical(value).expect("encode");
    loop {
        black_box(norito::decode_canonical::<T>(black_box(&bytes)).expect("canonical"));
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [mode, case] = arguments.as_slice()
        && mode == "loop"
    {
        match case.as_str() {
            "flat-encode" => spin(&flat(), false),
            "flat-decode" => spin(&flat(), true),
            "block-encode" => spin(&block(), false),
            "block-decode" => spin(&block(), true),
            "tx-encode" => spin(&tx_block(), false),
            "tx-decode" => spin(&tx_block(), true),
            "tx-canonical" => spin_canonical(&tx_block()),
            other => panic!("unknown case {other}"),
        }
    }
    report("flat", &flat());
    report("leaf", &leaf(3));
    report("signed", &signed(5));
    report("block", &block());
    report("txblock", &tx_block());
}
