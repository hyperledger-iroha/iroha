//! A read-only cross-version *test vector*. Production import accepts only the
//! compact schema. None of the transformed test originals authorizes a scheme.
use super::*;
use crate::native::{ParameterManifest, ProgramBundle};
use iroha_pasta::{Fp, Fq};

fn historical_fields<T>(bytes: &[u8]) -> T
where
    T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>,
{
    let view = norito::core::from_bytes_view(bytes).unwrap();
    assert_eq!(view.flags(), norito::core::V1_LAYOUT_FLAGS);
    let _flags = norito::core::DecodeFlagsGuard::enter(view.flags());
    let (fields, used) =
        norito::core::with_decode_limits(norito::canonical_decode_limits(bytes.len()), || {
            norito::core::decode_field_canonical::<T>(view.as_bytes())
        })
        .unwrap();
    assert_eq!(used, view.as_bytes().len());
    fields
}

#[test]
#[ignore = "requires the historical signed three-mode control program vectors"]
fn compact_eight_actual_programs_match_signed_ssa_and_carrier_budget() {
    let dir = std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_V3_PROGRAMS").unwrap());
    let bytes = std::fs::read(dir.join("native-device/parameter-manifest.norito")).unwrap();
    let digest = iroha_data_model::kagemusha::kagemusha_wallet_poseidon_bytes_v1(
        u64::from_le_bytes(*b"kgwparm1"),
        &bytes,
    );
    assert_eq!(
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "101142c39d85b53c8158e0312d2fad00679f9f9d141af029cfd387b2c32aca35"
    );
    // The fixture's historical payload has the same fields as v4 and a
    // distinct envelope/schema. It is named by the independent complete pin.
    let manifest: ParameterManifest = historical_fields(&bytes);
    assert_eq!(manifest.version, 3);
    let mut bundle = ProgramBundle {
        version: 1,
        primary: std::array::from_fn(|_| Vec::new()),
        secondary: std::array::from_fn(|_| Vec::new()),
    };
    fn check<F: Scalar>(path: std::path::PathBuf, primary: bool, digest: [u8; 32]) -> Vec<u8> {
        let original = std::fs::read(&path).unwrap();
        assert!(UniformCircuit::<F>::import_program(&original, primary).is_err());
        let view = norito::core::from_bytes_view(&original).unwrap();
        assert_eq!(
            view.schema(),
            [
                0x36, 0xb1, 0x78, 0x93, 0x1f, 0xe7, 0xb4, 0x77, 0xba, 0xdd, 0xcb, 0x55, 0xc7, 0x24,
                0xcf, 0xac
            ]
        );
        let mut p: PublicProgram = historical_fields(&original);
        assert_eq!(p.version, 2);
        assert!(p.operations.len().is_multiple_of(12));
        let words = std::mem::take(&mut p.operations);
        let original_ops = || {
            words.chunks_exact(12).map(|r| {
                let [tag, a, b] = std::array::from_fn(|j| {
                    u32::from_le_bytes(r[j * 4..j * 4 + 4].try_into().unwrap())
                });
                match tag {
                    0 => Op::Add(a, b),
                    1 => Op::Sub(a, b),
                    2 => Op::Mul(a, b),
                    3 => Op::Scale(a, b),
                    4 if b == 0 => Op::Const(a),
                    _ => panic!("invalid historical instruction"),
                }
            })
        };
        p.operations = compact::pack(p.inputs, original_ops).unwrap();
        p.version = 3;
        let bytes = compact::encode(&p).unwrap();
        let loaded = UniformCircuit::<F>::import_program(&bytes, primary).unwrap();
        assert!(loaded.plan().slot_instructions().eq(original_ops()));
        assert_eq!(
            loaded.program_digest_with_domain(b"kagemusha-spartan/program/v1\0"),
            digest,
            "independently pinned SSA digest"
        );
        assert_eq!(loaded.export_program().unwrap(), bytes);
        eprintln!(
            "{}: {} SSA instructions, {} original bytes, {} compact bytes",
            path.file_name().unwrap().display(),
            words.len() / 12,
            original.len(),
            bytes.len()
        );
        bytes
    }
    for kind in 0..4 {
        bundle.primary[kind] = check::<Fp>(
            dir.join(format!("research-kind-{kind}-primary.norito")),
            true,
            manifest.primary[kind],
        );
        bundle.secondary[kind] = check::<Fq>(
            dir.join(format!("research-kind-{kind}-secondary.norito")),
            false,
            manifest.secondary[kind],
        );
    }
    let primary =
        UniformCircuit::<Fp>::import_program(&bundle.primary[crate::full::SEND], true).unwrap();
    let secondary =
        UniformCircuit::<Fq>::import_program(&bundle.secondary[crate::full::SEND], false).unwrap();
    let pair = crate::carrier::TrustedProfile::from_parameters(
        manifest.relation,
        primary.shape,
        &primary,
        secondary.shape,
        &secondary,
        true,
    )
    .unwrap();
    crate::payment_wire::check_historical_payment_transport(
        &crate::payment_wire::Profile {
            pair: &pair,
            scheme: manifest.scheme,
            relation: manifest.relation,
        },
        &std::fs::read(dir.join("research-payment.ipm1")).unwrap(),
    );
    let bytes = bundle.encode().unwrap();
    let roundtrip = ProgramBundle::decode(&bytes).unwrap();
    assert_eq!(roundtrip.primary, bundle.primary);
    assert_eq!(roundtrip.secondary, bundle.secondary);
    eprintln!(
        "Complete compact Norito program bundle: {} bytes",
        bytes.len()
    );
    assert!(bytes.len() < 10 * 1024 * 1024);
}
