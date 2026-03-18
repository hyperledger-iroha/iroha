//! Regression test: complex struct with Vec and enum fields roundtrips under
//! hybrid packed-struct layout without panicking or length mismatches.
use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, NoritoSerialize, from_bytes, to_bytes};

#[derive(Clone, Debug, PartialEq, Default, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct DomainId {
    name: String,
}

#[derive(Clone, Debug, PartialEq, Default, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct AccountId {
    name: String,
    domain: DomainId,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct AssetDefinitionId {
    name: String,
    domain: DomainId,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct RegisterAssetDef {
    id: AssetDefinitionId,
    precision: u8,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct MintAsset {
    id: AssetDefinitionId,
    quantity: u128,
    account: AccountId,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct TransferAsset {
    id: AssetDefinitionId,
    src: AccountId,
    dst: AccountId,
    quantity: u128,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct SetKeyValue {
    account: AccountId,
    key: String,
    value: String,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
enum Instruction {
    RegisterAssetDef(RegisterAssetDef),
    MintAsset(MintAsset),
    TransferAsset(TransferAsset),
    SetKeyValue(SetKeyValue),
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct Signature {
    public_key: [u8; 32],
    signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct MetadataEntry {
    key: String,
    value: String,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct SignedTransaction {
    creator: AccountId,
    timestamp_ms: u64,
    nonce: u64,
    instructions: Vec<Instruction>,
    metadata: Vec<MetadataEntry>,
    signatures: Vec<Signature>,
}

// Provide safe slice-based decoders for nested types used by derive expansions.
impl<'a> norito::core::DecodeFromSlice<'a> for DomainId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        use std::alloc::{Layout, alloc, dealloc};
        let _g = norito::core::PayloadCtxGuard::enter(bytes);
        let layout =
            Layout::from_size_align(bytes.len(), core::mem::align_of::<norito::Archived<Self>>())
                .map_err(|_| norito::Error::LengthMismatch)?;
        let tmp = unsafe { alloc(layout) };
        if tmp.is_null() {
            return Err(norito::Error::LengthMismatch);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), tmp, bytes.len());
        }
        let archived = unsafe { &*(tmp as *const norito::Archived<Self>) };
        let v = <Self as NoritoDeserialize>::deserialize(archived);
        unsafe {
            dealloc(tmp, layout);
        }
        Ok((v, bytes.len()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AccountId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        use std::alloc::{Layout, alloc, dealloc};
        let _g = norito::core::PayloadCtxGuard::enter(bytes);
        let layout =
            Layout::from_size_align(bytes.len(), core::mem::align_of::<norito::Archived<Self>>())
                .map_err(|_| norito::Error::LengthMismatch)?;
        let tmp = unsafe { alloc(layout) };
        if tmp.is_null() {
            return Err(norito::Error::LengthMismatch);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), tmp, bytes.len());
        }
        let archived = unsafe { &*(tmp as *const norito::Archived<Self>) };
        let v = <Self as NoritoDeserialize>::deserialize(archived);
        unsafe {
            dealloc(tmp, layout);
        }
        Ok((v, bytes.len()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AssetDefinitionId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        use std::alloc::{Layout, alloc, dealloc};
        let _g = norito::core::PayloadCtxGuard::enter(bytes);
        let layout =
            Layout::from_size_align(bytes.len(), core::mem::align_of::<norito::Archived<Self>>())
                .map_err(|_| norito::Error::LengthMismatch)?;
        let tmp = unsafe { alloc(layout) };
        if tmp.is_null() {
            return Err(norito::Error::LengthMismatch);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), tmp, bytes.len());
        }
        let archived = unsafe { &*(tmp as *const norito::Archived<Self>) };
        let v = <Self as NoritoDeserialize>::deserialize(archived);
        unsafe {
            dealloc(tmp, layout);
        }
        Ok((v, bytes.len()))
    }
}

fn sample_tx(n_instr: usize, n_meta: usize, n_sigs: usize) -> SignedTransaction {
    let dom = DomainId {
        name: "wonderland".into(),
    };
    let alice = AccountId {
        name: "alice".into(),
        domain: dom.clone(),
    };
    let bob = AccountId {
        name: "bob".into(),
        domain: dom.clone(),
    };
    let rose = AssetDefinitionId {
        name: "rose".into(),
        domain: dom,
    };
    let mut instrs = Vec::new();
    for i in 0..n_instr {
        match i % 4 {
            0 => instrs.push(Instruction::RegisterAssetDef(RegisterAssetDef {
                id: rose.clone(),
                precision: 2,
            })),
            1 => instrs.push(Instruction::MintAsset(MintAsset {
                id: rose.clone(),
                quantity: 1_000_000,
                account: alice.clone(),
            })),
            2 => instrs.push(Instruction::TransferAsset(TransferAsset {
                id: rose.clone(),
                src: alice.clone(),
                dst: bob.clone(),
                quantity: 10,
            })),
            _ => instrs.push(Instruction::SetKeyValue(SetKeyValue {
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

#[test]
fn tx_like_roundtrip() {
    // A modestly sized transaction exercising nested structs, enums, and Vec fields.
    let tx = sample_tx(8, 3, 2);
    let bytes = to_bytes(&tx).expect("encode");
    let archived = from_bytes::<SignedTransaction>(&bytes).expect("from_bytes");
    let out: SignedTransaction = <SignedTransaction as NoritoDeserialize>::deserialize(archived);
    assert_eq!(tx, out);
}
