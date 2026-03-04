//! Iroha-like transaction/data benchmark: sizes and speeds vs SCALE.

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use criterion::Criterion;
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use norito::codec as ncodec;
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use norito::{self, CompressionConfig, NoritoDeserialize, NoritoSerialize};
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use parity_scale_codec::{Decode, Encode};

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct DomainId {
    name: String,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct AccountId {
    name: String,
    domain: DomainId,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct AssetDefinitionId {
    name: String,
    domain: DomainId,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct RegisterAssetDefParams {
    id: AssetDefinitionId,
    precision: u8,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct MintAssetParams {
    id: AssetDefinitionId,
    quantity: u128,
    account: AccountId,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct TransferAssetParams {
    id: AssetDefinitionId,
    src: AccountId,
    dst: AccountId,
    quantity: u128,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct SetKeyValueParams {
    account: AccountId,
    key: String,
    value: String,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
enum Instruction {
    RegisterAssetDef(RegisterAssetDefParams),
    MintAsset(MintAssetParams),
    TransferAsset(TransferAssetParams),
    SetKeyValue(SetKeyValueParams),
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct Signature {
    public_key: [u8; 32],
    signature: [u8; 64],
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct MetadataEntry {
    key: String,
    value: String,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct SignedTransaction {
    creator: AccountId,
    timestamp_ms: u64,
    nonce: u64,
    instructions: Vec<Instruction>,
    metadata: Vec<MetadataEntry>,
    signatures: Vec<Signature>,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn fid(s: &str) -> DomainId {
    DomainId { name: s.into() }
}
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn aid(name: &str, dom: &DomainId) -> AccountId {
    AccountId {
        name: name.into(),
        domain: dom.clone(),
    }
}
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn adid(name: &str, dom: &DomainId) -> AssetDefinitionId {
    AssetDefinitionId {
        name: name.into(),
        domain: dom.clone(),
    }
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn sample_tx(n_instr: usize, n_meta: usize, n_sigs: usize) -> SignedTransaction {
    let dom = fid("wonderland");
    let alice = aid("alice", &dom);
    let bob = aid("bob", &dom);
    let rose = adid("rose", &dom);
    let mut instrs = Vec::new();
    for i in 0..n_instr {
        match i % 4 {
            0 => instrs.push(Instruction::RegisterAssetDef(RegisterAssetDefParams {
                id: rose.clone(),
                precision: 2,
            })),
            1 => instrs.push(Instruction::MintAsset(MintAssetParams {
                id: rose.clone(),
                quantity: 1_000_000,
                account: alice.clone(),
            })),
            2 => instrs.push(Instruction::TransferAsset(TransferAssetParams {
                id: rose.clone(),
                src: alice.clone(),
                dst: bob.clone(),
                quantity: 10,
            })),
            _ => instrs.push(Instruction::SetKeyValue(SetKeyValueParams {
                account: alice.clone(),
                key: format!("k{i}"),
                value: format!("value-{i:04}"),
            })),
        }
    }
    let mut metadata = Vec::new();
    for i in 0..n_meta {
        metadata.push(MetadataEntry {
            key: format!("m{i}"),
            value: format!("v{i}"),
        });
    }
    let mut signatures = Vec::new();
    for i in 0..n_sigs {
        let mut pk = [0u8; 32];
        let mut sig = [0u8; 64];
        pk.iter_mut()
            .enumerate()
            .for_each(|(j, b)| *b = (i as u8).wrapping_add(j as u8));
        sig.iter_mut()
            .enumerate()
            .for_each(|(j, b)| *b = (i as u8).wrapping_mul(3).wrapping_add(j as u8));
        signatures.push(Signature {
            public_key: pk,
            signature: sig,
        });
    }
    SignedTransaction {
        creator: alice,
        timestamp_ms: 1_696_000_000_000,
        nonce: 42,
        instructions: instrs,
        metadata,
        signatures,
    }
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn report_sizes(label: &str, tx: &SignedTransaction) {
    let norito_raw = norito::to_bytes(tx).unwrap();
    let scale_raw = parity_scale_codec::Encode::encode(tx);
    let norito_zstd = norito::to_compressed_bytes(tx, Some(CompressionConfig::default())).unwrap();
    let norito_bare = ncodec::Encode::encode(tx);
    println!(
        "{} sizes: norito_headered={} B, norito_bare={} B, norito+zstd={} B, scale={} B",
        label,
        norito_raw.len(),
        norito_bare.len(),
        norito_zstd.len(),
        scale_raw.len()
    );
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn bench_iroha_like(c: &mut Criterion) {
    // Typical mid-size tx: ~10 instructions, ~3 metadata kvs, ~2 signatures.
    let tx = sample_tx(10, 3, 2);
    report_sizes("tx(10 instr, 3 meta, 2 sigs)", &tx);

    let norito_raw = norito::to_bytes(&tx).unwrap();
    let scale_raw = parity_scale_codec::Encode::encode(&tx);
    let norito_zstd = norito::to_compressed_bytes(&tx, Some(CompressionConfig::default())).unwrap();
    let norito_bare = ncodec::Encode::encode(&tx);

    c.bench_function("iroha_tx_norito_encode", |b| {
        b.iter(|| {
            let bytes = norito::to_bytes(std::hint::black_box(&tx)).unwrap();
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("iroha_tx_norito_decode", |b| {
        b.iter(|| {
            let v: SignedTransaction =
                norito::decode_from_bytes(std::hint::black_box(&norito_raw)).unwrap();
            std::hint::black_box(v)
        })
    });

    c.bench_function("iroha_tx_norito_encode_zstd", |b| {
        b.iter(|| {
            let bytes = norito::to_compressed_bytes(
                std::hint::black_box(&tx),
                Some(CompressionConfig::default()),
            )
            .unwrap();
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("iroha_tx_norito_decode_zstd", |b| {
        b.iter(|| {
            let v: SignedTransaction =
                norito::decode_from_bytes(std::hint::black_box(&norito_zstd)).unwrap();
            std::hint::black_box(v)
        })
    });

    c.bench_function("iroha_tx_scale_encode", |b| {
        b.iter(|| {
            let bytes = parity_scale_codec::Encode::encode(std::hint::black_box(&tx));
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("iroha_tx_scale_decode", |b| {
        b.iter(|| {
            let v = SignedTransaction::decode(&mut &std::hint::black_box(&scale_raw)[..]).unwrap();
            std::hint::black_box(v)
        })
    });

    c.bench_function("iroha_tx_norito_codec_bare_encode", |b| {
        b.iter(|| {
            let bytes = ncodec::Encode::encode(std::hint::black_box(&tx));
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("iroha_tx_norito_codec_bare_decode", |b| {
        b.iter(|| {
            let v: SignedTransaction =
                ncodec::Decode::decode(&mut &std::hint::black_box(&norito_bare)[..]).unwrap();
            std::hint::black_box(v)
        })
    });

    // Also report for a larger tx (more instructions/signatures).
    let tx_large = sample_tx(50, 5, 5);
    report_sizes("tx(50 instr, 5 meta, 5 sigs)", &tx_large);
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_iroha_like(&mut c);
    c.final_summary();
}

#[cfg(not(all(feature = "parity-scale", feature = "bench-internal")))]
fn main() {
    eprintln!("Enable `parity-scale` and `bench-internal` features to run this benchmark.");
}
