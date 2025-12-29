#![allow(clippy::manual_div_ceil)]
use norito::{NoritoDeserialize, NoritoSerialize, from_bytes, to_bytes};

#[derive(
    Clone, Debug, PartialEq, Default, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
struct DomainId {
    name: String,
}
#[derive(
    Clone, Debug, PartialEq, Default, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
struct AccountId {
    name: String,
    domain: DomainId,
}
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct AssetDefinitionId {
    name: String,
    domain: DomainId,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct RegisterAssetDefInstr {
    asset: AssetDefinitionId,
    precision: u8,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct MintAssetInstr {
    asset: AssetDefinitionId,
    amount: u128,
    destination: AccountId,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct TransferAssetInstr {
    asset: AssetDefinitionId,
    source: AccountId,
    destination: AccountId,
    amount: u128,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct SetKeyValueInstr {
    account: AccountId,
    key: String,
    value: String,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
enum Instruction {
    RegisterAssetDef(RegisterAssetDefInstr),
    MintAsset(MintAssetInstr),
    TransferAsset(TransferAssetInstr),
    SetKeyValue(SetKeyValueInstr),
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct Signature {
    public_key: [u8; 32],
    signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct SignedTransaction {
    creator: AccountId,
    timestamp_ms: u64,
    nonce: u64,
    instructions: Vec<Instruction>,
    metadata: Vec<MetadataEntry>,
    signatures: Vec<Signature>,
}

#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct MetadataEntry {
    key: String,
    value: String,
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

fn sample() -> SignedTransaction {
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
    for i in 0..10 {
        match i % 4 {
            0 => instrs.push(Instruction::RegisterAssetDef(RegisterAssetDefInstr {
                asset: rose.clone(),
                precision: 2,
            })),
            1 => instrs.push(Instruction::MintAsset(MintAssetInstr {
                asset: rose.clone(),
                amount: 1_000,
                destination: alice.clone(),
            })),
            2 => instrs.push(Instruction::TransferAsset(TransferAssetInstr {
                asset: rose.clone(),
                source: alice.clone(),
                destination: bob.clone(),
                amount: 10,
            })),
            _ => instrs.push(Instruction::SetKeyValue(SetKeyValueInstr {
                account: alice.clone(),
                key: format!("k{i}"),
                value: format!("v{i}"),
            })),
        }
    }
    let mut metadata = Vec::new();
    for i in 0..3 {
        metadata.push(MetadataEntry {
            key: format!("m{i}"),
            value: format!("v{i}"),
        });
    }
    let mut signatures = Vec::new();
    for i in 0..2 {
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
fn iroha_like_roundtrip() {
    let tx = sample();
    let bytes = to_bytes(&tx).expect("encode");
    let archived = from_bytes::<SignedTransaction>(&bytes).expect("from_bytes");
    let got: SignedTransaction = <SignedTransaction as NoritoDeserialize>::deserialize(archived);
    assert_eq!(tx, got);
}

#[test]
fn signature_roundtrip() {
    let mut pk = [0u8; 32];
    let mut sig = [0u8; 64];
    pk.iter_mut().enumerate().for_each(|(i, b)| *b = i as u8);
    sig.iter_mut()
        .enumerate()
        .for_each(|(i, b)| *b = (i as u8).wrapping_mul(7));
    let s = Signature {
        public_key: pk,
        signature: sig,
    };
    let bytes = to_bytes(&s).expect("encode");
    let archived = from_bytes::<Signature>(&bytes).expect("from_bytes");
    let got: Signature = <Signature as NoritoDeserialize>::deserialize(archived);
    assert_eq!(s, got);
}
