//! Test-only public Mint113 artifacts from the actual current mathematical producer.
//!
//! Exports public parameters, verifying keys, proofs and exact reconstruction inputs.
//! No proving key, private witness, opening, recovery seed or signing key is written.
//! These artifacts confer no signed release, device, funding or finalized-State authority.

use super::*;
use ff::PrimeField as _;
use norito::{Decode, Encode, NoritoSchema};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write as _,
    os::unix::fs::{
        DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
    },
    path::{Path, PathBuf},
};

use super::super::{
    deferred_parent::{kagemusha_protocol_structure_digest_v1, native_parent_protocol_digest_v1},
    native_backend::{
        read_ep_ordinary_mint_authorization_vk, read_eq_ordinary_mint_authorization_vk,
    },
    ordinary_issuer_config::{
        ORDINARY_ISSUER_SLOTS, OrdinaryIssuerProfileV1, OrdinaryIssuerTableV1,
    },
};

type ExportResult<T> = Result<T, Box<dyn std::error::Error>>;
const OUTPUT_ENV: &str = "IROHA_TEST_ORDINARY_MINT_PUBLIC_ARTIFACT_DIR";

/// Public inputs sufficient to reconstruct the exact protocol from its processed VK.
/// This is a canonical test artifact, not a serialized PlonkProtocol or release manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:test:ordinary-mint-public-protocol-inputs:v1")]
struct ProtocolInputs {
    version: u16,
    parity: u8,
    k: u64,
    advice_per_phase: Vec<u64>,
    fixed: u64,
    lookup_advice_per_phase: Vec<u64>,
    lookup_bits: Option<u64>,
    instance_columns: u64,
    provider_root: [u8; 32],
    issuer_slots: Vec<([u8; 32], [u8; 65])>,
    protocol_digest: [u8; 32],
    protocol_structure_digest: [u8; 32],
}

impl ProtocolInputs {
    fn base(&self) -> ExportResult<BaseCircuitParams> {
        if self.version != 1
            || self.k != u64::from(KAGEMUSHA_HALO2_K_V1)
            || !matches!(self.parity, 1 | 2)
            || self.instance_columns != 1
            || self.provider_root == [0; 32]
            || self.issuer_slots.len() != ORDINARY_ISSUER_SLOTS
        {
            return Err("invalid public Mint protocol inputs".into());
        }
        Ok(BaseCircuitParams {
            k: self.k.try_into()?,
            num_advice_per_phase: self
                .advice_per_phase
                .iter()
                .copied()
                .map(usize::try_from)
                .collect::<Result<_, _>>()?,
            num_fixed: self.fixed.try_into()?,
            num_lookup_advice_per_phase: self
                .lookup_advice_per_phase
                .iter()
                .copied()
                .map(usize::try_from)
                .collect::<Result<_, _>>()?,
            lookup_bits: self.lookup_bits.map(usize::try_from).transpose()?,
            num_instance_columns: self.instance_columns.try_into()?,
        })
    }

    fn issuers(&self) -> ExportResult<OrdinaryIssuerTableV1> {
        if self.issuer_slots.len() != ORDINARY_ISSUER_SLOTS {
            return Err("public Mint issuer slot count differs".into());
        }
        let mut table = OrdinaryIssuerTableV1::default();
        for (slot, (profile_id, issuer_sec1)) in table.slots.iter_mut().zip(&self.issuer_slots) {
            *slot = OrdinaryIssuerProfileV1 {
                profile_id: *profile_id,
                issuer_sec1: *issuer_sec1,
            };
        }
        Ok(table)
    }
}

fn output_root(path: &Path) -> ExportResult<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if !path.is_absolute()
        || path.canonicalize()? != path
        || !metadata.is_dir()
        || metadata.permissions().mode() & 0o7777 != 0o700
        || metadata.uid() != rustix::process::geteuid().as_raw()
    {
        return Err("public Mint output must be a canonical owner-only directory".into());
    }
    Ok(path.to_path_buf())
}

fn write_public(directory: &Path, name: &str, bytes: &[u8]) -> ExportResult<norito::json::Value> {
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        return Err("invalid public artifact name".into());
    }
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if fs::read(&path)? != bytes {
        return Err("public artifact readback differs".into());
    }
    Ok(
        norito::json!({"file": name, "bytes": (bytes.len() as u64), "sha256": (hex::encode(Sha256::digest(bytes)))}),
    )
}

/// Called only after the genuine producer's positive and adversarial controls finish.
pub(super) fn export_from_environment(
    pair: &ordinary_mint_generation::GeneratedOrdinaryMintPairV1,
    provider_root: [u8; 32],
    issuers: &OrdinaryIssuerTableV1,
    apple: bool,
) -> ExportResult<()> {
    let Some(path) = std::env::var_os(OUTPUT_ENV) else {
        return Ok(());
    };
    let root = output_root(Path::new(&path))?;
    let directory = root.join(if apple { "apple" } else { "android" });
    DirBuilder::new().mode(0o700).create(&directory)?;
    let mut files = Vec::new();
    macro_rules! export_parity {
        ($value:expr, $field:ty, $curve:ty, $parity:ident, $tag:literal, $prefix:literal, $read_vk:ident, $verify:ident, $accumulator:ident, $decide:ident) => {{
            let generated = $value;
            let protocol = &generated.protocol;
            let inputs = ProtocolInputs {
                version: 1,
                parity: $tag,
                k: generated.base_params.k as u64,
                advice_per_phase: generated
                    .base_params
                    .num_advice_per_phase
                    .iter()
                    .map(|n| *n as u64)
                    .collect(),
                fixed: generated.base_params.num_fixed as u64,
                lookup_advice_per_phase: generated
                    .base_params
                    .num_lookup_advice_per_phase
                    .iter()
                    .map(|n| *n as u64)
                    .collect(),
                lookup_bits: generated.base_params.lookup_bits.map(|n| n as u64),
                instance_columns: generated.base_params.num_instance_columns as u64,
                provider_root,
                issuer_slots: issuers
                    .slots
                    .iter()
                    .map(|r| (r.profile_id, r.issuer_sec1))
                    .collect(),
                protocol_digest: generated.protocol_digest,
                protocol_structure_digest: kagemusha_protocol_structure_digest_v1(
                    protocol,
                    KagemushaPastaParityV1::$parity,
                )?,
            };
            let columns: Vec<[u8; 32]> = generated
                .instances
                .iter()
                .map(|v| v.to_repr().as_ref().try_into().expect("Pasta scalar width"))
                .collect();
            assert_eq!(columns.len(), 113);
            files.push(write_public(
                &directory,
                concat!($prefix, ".params"),
                &generated.parameters,
            )?);
            files.push(write_public(
                &directory,
                concat!($prefix, ".vk"),
                &generated.verifying_key,
            )?);
            files.push(write_public(
                &directory,
                concat!($prefix, ".proof"),
                &generated.proof,
            )?);
            files.push(write_public(
                &directory,
                concat!($prefix, ".instances.norito"),
                &norito::encode_canonical(&columns)?,
            )?);
            files.push(write_public(
                &directory,
                concat!($prefix, ".protocol-inputs.norito"),
                &norito::encode_canonical(&inputs)?,
            )?);

            // Consume only the exported public bytes for the replay. Recompile the current
            // protocol from the actual processed VK and its canonical public configuration.
            let encoded = fs::read(directory.join(concat!($prefix, ".protocol-inputs.norito")))?;
            let decoded: ProtocolInputs = norito::decode_canonical(&encoded)?;
            assert_eq!(decoded, inputs);
            let table = decoded.issuers()?;
            let vk = $read_vk(
                &fs::read(directory.join(concat!($prefix, ".vk")))?,
                decoded.base()?,
                decoded.provider_root,
                &table,
            )?;
            let encoded_parameters = fs::read(directory.join(concat!($prefix, ".params")))?;
            let mut reader = Cursor::new(&encoded_parameters);
            let parameters = ParamsIPA::<$curve>::read(&mut reader)?;
            assert_eq!(reader.position() as usize, encoded_parameters.len());
            assert_eq!(parameters.k(), KAGEMUSHA_HALO2_K_V1);
            let rebuilt = compile(
                &parameters,
                &vk,
                snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![113]),
            );
            assert_eq!(
                native_parent_protocol_digest_v1(&rebuilt, KagemushaPastaParityV1::$parity)?,
                decoded.protocol_digest
            );
            assert_eq!(
                kagemusha_protocol_structure_digest_v1(&rebuilt, KagemushaPastaParityV1::$parity)?,
                decoded.protocol_structure_digest
            );
            let encoded: Vec<[u8; 32]> = norito::decode_canonical(&fs::read(
                directory.join(concat!($prefix, ".instances.norito")),
            )?)?;
            let column: Vec<$field> = encoded
                .into_iter()
                .map(|bytes| {
                    Option::<$field>::from(<$field>::from_repr(bytes))
                        .expect("exported canonical Pasta scalar")
                })
                .collect();
            assert_eq!(column.len(), 113);
            let proof = fs::read(directory.join(concat!($prefix, ".proof")))?;
            let accepted = |proof: &[u8], column: &[$field]| {
                $verify(&parameters, &rebuilt, proof, column)
                    .ok()
                    .and_then(|a| $accumulator::from_native(&a).ok())
                    .is_some_and(|a| $decide(&parameters, &a).is_ok())
            };
            assert!(accepted(&proof, &column));
            let mut changed = column.clone();
            changed[0] += <$field>::ONE;
            assert!(!accepted(&proof, &changed));
            let mut changed = proof.clone();
            changed[0] ^= 1;
            assert!(!accepted(&changed, &column));
        }};
    }
    export_parity!(
        &pair.eq,
        Fp,
        EqAffine,
        Eq,
        1,
        "eq",
        read_eq_ordinary_mint_authorization_vk,
        verify_eq_succinct_protocol,
        KagemushaEqAccumulatorV1,
        decide_kagemusha_eq_accumulator_v1
    );
    export_parity!(
        &pair.ep,
        Fq,
        EpAffine,
        Ep,
        2,
        "ep",
        read_ep_ordinary_mint_authorization_vk,
        verify_ep_succinct_protocol,
        KagemushaEpAccumulatorV1,
        decide_kagemusha_ep_accumulator_v1
    );
    let receipt = norito::json!({
        "version": 1,
        "relation": "ordinary-MintAuthorization113",
        "platform": (if apple { "apple" } else { "android" }),
        "files": files,
        "exported_public_replay_passed": true,
        "original_adversarial_controls_passed": true,
        "mathematical_fixture_only": true,
        "source_qualification": false,
        "release_qualification": false,
        "device_qualification": false,
        "finalized_state_qualification": false
    });
    write_public(
        &directory,
        "receipt.json",
        &norito::json::to_vec_pretty(&receipt)?,
    )?;
    eprintln!(
        "IROHA_TEST_ORDINARY_MINT_PUBLIC_ARTIFACTS_V1 {}",
        directory.display()
    );
    Ok(())
}

#[test]
fn public_artifact_writer_requires_private_canonical_directory_and_never_overwrites() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().canonicalize().unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(output_root(&path).unwrap(), path);
    write_public(&path, "sample", b"public mathematical bytes").unwrap();
    assert!(write_public(&path, "sample", b"changed").is_err());
    assert_eq!(
        fs::read(path.join("sample")).unwrap(),
        b"public mathematical bytes"
    );
    assert!(write_public(&path, "../outside", b"none").is_err());
    let alias = path.join("alias");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(output_root(&alias).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(output_root(&path).is_err());
}

#[test]
fn public_protocol_inputs_roundtrip_preserves_configuration_and_rejects_wrong_shape() {
    let value = ProtocolInputs {
        version: 1,
        parity: 1,
        k: u64::from(KAGEMUSHA_HALO2_K_V1),
        advice_per_phase: vec![3, 2],
        fixed: 1,
        lookup_advice_per_phase: vec![2],
        lookup_bits: Some(8),
        instance_columns: 1,
        provider_root: [3; 32],
        issuer_slots: vec![([4; 32], [5; 65]); ORDINARY_ISSUER_SLOTS],
        protocol_digest: [6; 32],
        protocol_structure_digest: [7; 32],
    };
    let bytes = norito::encode_canonical(&value).unwrap();
    let decoded: ProtocolInputs = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, value);
    let base = decoded.base().unwrap();
    assert_eq!(base.num_advice_per_phase, [3, 2]);
    assert_eq!(base.num_lookup_advice_per_phase, [2]);
    assert_eq!(base.lookup_bits, Some(8));
    assert_eq!(decoded.issuers().unwrap().slots[63].issuer_sec1, [5; 65]);
    let mut changed = decoded.clone();
    changed.k += 1;
    assert!(changed.base().is_err());
    let mut changed = decoded.clone();
    changed.issuer_slots.pop();
    assert!(changed.issuers().is_err());
    assert!(changed.base().is_err());
    let mut changed = decoded;
    changed.provider_root = [0; 32];
    assert!(changed.base().is_err());
}
