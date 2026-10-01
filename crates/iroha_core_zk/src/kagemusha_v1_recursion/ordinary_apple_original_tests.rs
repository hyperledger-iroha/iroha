//! Whole original-CBOR/release/SHA components; no installed/native or financial authority.
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::kagemusha::{KAGEMUSHA_HALO2_K_V1, app_attest_release_extensions_digest};

#[derive(Clone)]
struct OriginalCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for OriginalCircuit<F> {
    type Config = (BaseConfig<F>, PastaSha256ConfigV1);
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;
    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }
    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
        }
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1usize << KAGEMUSHA_HALO2_K_V1) - 9);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!()
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.0, layouter.namespace(|| "same original base"))?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1usize << KAGEMUSHA_HALO2_K_V1) - 9,
        )
    }
}
fn encoded(major: u8, bytes: &[u8]) -> Vec<u8> {
    let mut v = if bytes.len() < 24 {
        vec![(major << 5) | bytes.len() as u8]
    } else {
        vec![(major << 5) | 24, bytes.len() as u8]
    };
    v.extend_from_slice(bytes);
    v
}
fn original(version: Option<&str>, cat_first: bool, auth_first: bool) -> (Vec<u8>, [u8; 32]) {
    let mut auth = vec![12; 32];
    auth.push(if version.is_some() { 0xc0 } else { 0x40 });
    auth.extend_from_slice(&7u32.to_be_bytes());
    let release = version.map_or([13; 32], |v| {
        app_attest_release_extensions_digest(2, v).unwrap()
    });
    if let Some(version) = version {
        let mut cat = encoded(3, b"validationCategory");
        cat.extend(encoded(2, &2u32.to_le_bytes()));
        let mut ver = encoded(3, b"bundleVersion");
        ver.extend(encoded(3, version.as_bytes()));
        auth.push(0xa2);
        if cat_first {
            auth.extend(cat);
            auth.extend(ver);
        } else {
            auth.extend(ver);
            auth.extend(cat);
        }
    }
    let mut a = encoded(3, b"authenticatorData");
    a.extend(encoded(2, &auth));
    let mut s = encoded(3, b"signature");
    s.extend(encoded(2, &[0x30, 6, 2, 1, 1, 2, 1, 1]));
    let mut raw = vec![0xa2];
    if auth_first {
        raw.extend(a);
        raw.extend(s);
    } else {
        raw.extend(s);
        raw.extend(a);
    }
    (raw, release)
}
fn check<F: KagemushaPoseidonFieldV1>(raw: &[u8], release: [u8; 32], mutation: Option<u8>) -> bool {
    let parts = kagemusha_ordinary_apple_original_parts_v1(raw, release).unwrap();
    let mut header: [u8; 37] = parts.authenticator_data[..37].try_into().unwrap();
    let mut der = parts.signature_der.to_vec();
    let mut release_cells = release;
    match mutation {
        Some(0) => release_cells[0] ^= 1,
        Some(1) => header[36] ^= 1,
        Some(2) => der[4] ^= 1,
        _ => {}
    }
    let k = KAGEMUSHA_HALO2_K_V1 as usize;
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(k)
        .use_lookup_bits(k - 1)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let header = std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(header[i]))));
    let release_cells =
        std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(release_cells[i]))));
    let der_cells = std::array::from_fn(|i| {
        let b = ctx.load_witness(F::from(u64::from(der.get(i).copied().unwrap_or(0))));
        PastaSha256ByteV1::range_checked(ctx, &range, b)
    });
    let len = ctx.load_witness(F::from(der.len() as u64));
    let der = P256CanonicalDerV1 {
        bytes: der_cells,
        len,
    };
    constrain_original_apple_assertion_stream_v1(
        &mut builder,
        &mut jobs,
        raw,
        &header,
        release,
        &release_cells,
        &der,
    )
    .unwrap();
    builder.assigned_instances = vec![vec![]];
    builder.calculate_params(Some(9));
    MockProver::run(k as u32, &OriginalCircuit { builder, jobs }, vec![vec![]])
        .expect("bounded original component synthesizes at selected release k")
        .verify()
        .is_ok()
}
#[test]
fn exact_release_original_stream_accepts_both_orders_and_full_utf8_bound_in_both_parities() {
    let maximum = "é".repeat(64);
    for version in [None, Some("1"), Some(maximum.as_str())] {
        for cat in [false, true] {
            for auth in [false, true] {
                let (raw, release) = original(version, cat, auth);
                let eq = check::<Fp>(&raw, release, None);
                let ep = check::<Fq>(&raw, release, None);
                assert_eq!(eq, ep);
                assert!(eq);
            }
        }
    }
}
#[test]
fn assigned_release_header_or_der_substitution_rejects_independently_in_both_parities() {
    let (raw, release) = original(Some("1.2.3"), true, false);
    for mutation in [0, 1, 2] {
        let eq = check::<Fp>(&raw, release, Some(mutation));
        let ep = check::<Fq>(&raw, release, Some(mutation));
        assert_eq!(eq, ep);
        assert!(!eq);
    }
}
fn utf8_check<F: KagemushaPoseidonFieldV1>(bytes: &[u8]) -> bool {
    let k = KAGEMUSHA_HALO2_K_V1 as usize;
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(k)
        .use_lookup_bits(k - 1)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let len = ctx.load_witness(F::from(bytes.len() as u64));
    let cells = (0..VERSION_CAP)
        .map(|i| {
            let b = ctx.load_witness(F::from(u64::from(bytes.get(i).copied().unwrap_or(0))));
            PastaSha256ByteV1::range_checked(ctx, &range, b)
        })
        .collect();
    let stream = KagemushaBoundedByteStreamV1::constrain(ctx, &range, cells, len).unwrap();
    let present = ctx.load_witness(F::ONE);
    constrain_utf8_version(ctx, &range, &stream, present);
    builder.assigned_instances = vec![vec![]];
    builder.calculate_params(Some(9));
    MockProver::run(k as u32, &builder, vec![vec![]])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn version_utf8_relation_rejects_nul_overlong_surrogate_out_of_range_and_truncated_originals() {
    for bytes in ["1.é.𐀀".as_bytes(), b"version".as_slice()] {
        let eq = utf8_check::<Fp>(bytes);
        let ep = utf8_check::<Fq>(bytes);
        assert_eq!(eq, ep);
        assert!(eq);
    }
    for bytes in [
        vec![],
        vec![0],
        vec![0x80],
        vec![0xc0, 0x80],
        vec![0xe0, 0x80, 0x80],
        vec![0xed, 0xa0, 0x80],
        vec![0xf0, 0x80, 0x80, 0x80],
        vec![0xf4, 0x90, 0x80, 0x80],
        vec![0xe2, 0x82],
    ] {
        let eq = utf8_check::<Fp>(&bytes);
        let ep = utf8_check::<Fq>(&bytes);
        assert_eq!(eq, ep);
        assert!(!eq);
    }
}
