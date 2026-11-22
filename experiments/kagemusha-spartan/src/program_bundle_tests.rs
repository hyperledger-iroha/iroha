use super::*;
use crate::{deferral, uniform::Array};

fn bytes_only() -> ProgramBundleV2 {
    ProgramBundleV2 {
        version: 2,
        primary: std::array::from_fn(|i| vec![i as u8]),
        secondary: std::array::from_fn(|i| vec![(5 + i) as u8]),
    }
}

#[test]
fn successor_bundle_requires_two_exact_five_original_arrays_and_its_own_schema() {
    let mut bundle = bytes_only();
    let frame = bundle.encode().unwrap();
    let decoded = ProgramBundleV2::decode(&frame).unwrap();
    assert_eq!(decoded.primary, bundle.primary);
    assert_eq!(decoded.secondary, bundle.secondary);
    assert_eq!(decoded.encode().unwrap(), frame);
    assert!(ProgramBundle::decode(&frame).is_err());
    let old_frame = ProgramBundle {
        version: 1,
        primary: std::array::from_fn(|_| vec![1]),
        secondary: std::array::from_fn(|_| vec![2]),
    }
    .encode()
    .unwrap();
    assert!(ProgramBundleV2::decode(&old_frame).is_err());
    bundle.version = 1;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    assert!(ProgramBundleV2::decode(&norito::to_bytes(&bundle).unwrap()).is_err());
    bundle.version = 2;
    bundle.secondary[4].clear();
    assert!(ProgramBundleV2::decode(&norito::to_bytes(&bundle).unwrap()).is_err());
    assert!(bundle.encode().is_err());
    assert!(ProgramBundleV2::decode(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(ProgramBundleV2::decode(&trailing).is_err());
}

#[test]
fn successor_bundle_enforces_complete_frame_ceiling_and_stricter_caller_budget() {
    let mut bundle = bytes_only();
    let overhead = bundle.encode().unwrap().len() - 1;
    bundle.secondary[4] = vec![42; MAX_PROGRAM_BUNDLE_BYTES - overhead];
    let mut frame = bundle.encode().unwrap();
    assert_eq!(frame.len(), MAX_PROGRAM_BUNDLE_BYTES);
    assert_eq!(
        ProgramBundleV2::decode(&frame).unwrap().secondary[4],
        bundle.secondary[4]
    );
    let caller = norito::DecodeLimits::new(4096, 4096, 8192, 16384, 16);
    assert!(
        norito::core::with_decode_limits_scope(caller, || ProgramBundleV2::decode(&frame)).is_err()
    );
    bundle.secondary[4].push(0);
    assert!(matches!(bundle.encode(), Err(Error::ResourceLimit)));
    frame.push(0);
    assert!(matches!(
        ProgramBundleV2::decode(&frame),
        Err(Error::ResourceLimit)
    ));
}

/// Small real compiled public tapes, with the exact two late slot sequences.
/// Each kind pins a different fixture value. These describe no monetary action.
pub(in crate::native) fn original<F: Scalar>(
    primary: bool,
    kind: usize,
    coordinates: usize,
    col_bits: usize,
    wallet_row: bool,
) -> (Vec<u8>, [u8; 32]) {
    let mut wiring = full::Wiring::<F>::default();
    let family = wiring.family(0, 1);
    for (count, offset) in [
        (coordinates, full::LATE_IO),
        (3 + col_bits, generator_claim::IO_START),
    ] {
        let claim = deferral::Claim {
            point: vec![F::ZERO; count],
            value: F::from(kind as u64 + 1),
        };
        deferral::expose_constant(&mut wiring, family, &claim, offset);
    }
    if wallet_row {
        let row = wiring.push(Array {
            name: "bundle fixture wallet table",
            family,
            rho: 9,
            r_count: crate::nonnative::WALLET_ROWS,
            fixed: false,
            lookups: vec![],
            rows: vec![[
                crate::uniform::own(0, crate::nonnative::WALLET_ROWS),
                crate::uniform::constant(vec![F::ONE; crate::nonnative::WALLET_ROWS]),
                crate::uniform::own(0, crate::nonnative::WALLET_ROWS),
            ]],
            widths: vec![9, 88, 88, 80, 88, 88, 80],
            values: vec![
                vec![F::ZERO; crate::nonnative::WALLET_ROWS];
                crate::nonnative::WALLET_CELLS
            ],
        });
        wiring.wallet = Some((row, full::TABLE_IO));
    }
    let slots: &[usize] = if primary {
        &[124, 125, 126, 127]
    } else {
        &[124, 126]
    };
    for &slot in slots {
        let a = wiring.push(Array {
            name: "bundle fixture link slot",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![F::ZERO]],
        });
        wiring.io_arrays.push((a, slot));
        wiring.link.io.push((a, slot));
    }
    wiring.link.io_values = Some(Box::new(move |_, _| vec![F::ZERO; slots.len()]));
    let (arrays, families, counts, tables, copies) = wiring.finish();
    let (circuit, _, _) = UniformCircuit::build_with_copies(
        if primary { "P-uniform" } else { "S-uniform" },
        arrays,
        families,
        counts,
        tables,
        copies,
        col_bits,
        64,
        b"five-kind original geometry fixture",
    );
    (circuit.export_program().unwrap(), circuit.program_digest())
}

#[test]
fn successor_preflight_checks_each_pinned_tape_common_layout_and_claim_slots() {
    let p = std::array::from_fn::<_, 5, _>(|i| original::<Fp>(true, i, 55, 13, true));
    let s = std::array::from_fn::<_, 5, _>(|i| original::<Fq>(false, i, 54, 10, false));
    let mut pins_p = p.each_ref().map(|original| original.1);
    let mut pins_s = s.each_ref().map(|original| original.1);
    let mut bundle = ProgramBundleV2 {
        version: 2,
        primary: p.map(|o| o.0),
        secondary: s.map(|o| o.0),
    };
    let dimensions = [[24, 25], [24, 24]];
    let frame = bundle.encode().unwrap();
    let [primary, secondary] =
        ProgramBundleV2::inspect_originals(&frame, &pins_p, &pins_s, dimensions).unwrap();
    assert_eq!((primary.column_bits, primary.folded_claim_words), (13, 56));
    assert_eq!(
        (secondary.column_bits, secondary.folded_claim_words),
        (10, 55)
    );
    eprintln!(
        "unsigned five-kind fixture: {} B; primary {primary:?}; secondary {secondary:?}",
        frame.len()
    );
    // An offline CLI integration run can consume these exact compiled public
    // originals and their independently measured pre-export digests.
    if let Some(dir) = std::env::var_os("KAGEMUSHA_SPARTAN_BUNDLE_FIXTURE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("program-bundle.norito"), &frame).unwrap();
        let pins = pins_p
            .iter()
            .chain(&pins_s)
            .map(|digest| {
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(dir.join("compiler-pins.hex"), pins).unwrap();
    }
    let mut wrong_pin = pins_s;
    wrong_pin.swap(3, 4);
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&frame, &pins_p, &wrong_pin, dimensions),
        Err(Error::AuthorityMismatch("five-kind original digest"))
    ));
    for invalid in [
        [[0, 25], [24, 24]],
        [[30, 30], [24, 24]],
        [[24, 25], [24, 23]],
    ] {
        assert!(ProgramBundleV2::inspect_originals(&frame, &pins_p, &pins_s, invalid).is_err());
    }
    let (missing_wallet, pin) = original::<Fp>(true, 4, 55, 13, false);
    let saved = std::mem::replace(&mut bundle.primary[4], missing_wallet);
    let saved_pin = std::mem::replace(&mut pins_p[4], pin);
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&bundle.encode().unwrap(), &pins_p, &pins_s, dimensions),
        Err(Error::Malformed("five-kind wallet table geometry"))
    ));
    bundle.primary[4] = saved;
    pins_p[4] = saved_pin;
    bundle.secondary[4] = bundle.secondary[0].clone();
    pins_s[4] = pins_s[0];
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&bundle.encode().unwrap(), &pins_p, &pins_s, dimensions),
        Err(Error::Malformed("duplicate five-kind program pin"))
    ));
    (bundle.secondary[4], pins_s[4]) = original::<Fq>(false, 4, 53, 10, false);
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&bundle.encode().unwrap(), &pins_p, &pins_s, dimensions),
        Err(Error::Malformed("five-kind late IO layout"))
    ));
    (bundle.secondary[4], pins_s[4]) = original::<Fq>(false, 4, 54, 11, false);
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&bundle.encode().unwrap(), &pins_p, &pins_s, dimensions),
        Err(Error::Malformed("five-kind common program layout"))
    ));
    bundle.secondary[4] = bundle.primary[4].clone();
    pins_s[4] = pins_p[4];
    assert!(matches!(
        ProgramBundleV2::inspect_originals(&bundle.encode().unwrap(), &pins_p, &pins_s, dimensions),
        Err(Error::Rejected(_))
    ));
}

/// Byte identity for the retained, independently signed generation whose
/// original v1 envelope now shares the bounded codec with the successor.
#[test]
#[ignore = "requires the retained signed v4 engineering generation"]
fn signed_installed_v1_bundle_roundtrips_through_shared_codec_and_keeps_all_eight_pins() {
    use crate::native::{built_in_producer_programs, device_fixture};
    use sha2::Digest;

    let dir =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let orig = std::fs::read(dir.join("program-bundle.norito")).unwrap();
    assert_eq!(orig.len(), 5_663_732);
    let actual: [u8; 32] = sha2::Sha256::digest(&orig).into();
    assert_eq!(
        actual,
        [
            0x32, 0xea, 0xfc, 0xce, 0x2a, 0x43, 0xf7, 0xcc, 0x72, 0x3b, 0x2f, 0xf6, 0x6f, 0x18,
            0x38, 0x08, 0x3d, 0xd1, 0x5f, 0xef, 0xd0, 0x7b, 0x0c, 0xca, 0x7b, 0x0d, 0x6f, 0x28,
            0xb7, 0x17, 0x80, 0xd8
        ]
    );
    let restored = ProgramBundle::decode(&orig).unwrap().encode().unwrap();
    assert!(
        orig == restored,
        "complete signed program wire must remain byte-identical"
    );
    assert!(ProgramBundleV2::decode(&restored).is_err());
    let signature = std::fs::read(dir.join("parameter-signature.bin"))
        .unwrap()
        .try_into()
        .unwrap();
    let manifest = std::fs::read(dir.join("parameter-manifest.norito")).unwrap();
    let installed = device_fixture::install(&restored, &manifest, signature).unwrap();
    assert_eq!(*installed.id(), device_fixture::scheme().relation_id);
    let (primary, secondary) = built_in_producer_programs().unwrap();
    assert_eq!(installed.inner.programs.len(), 4);
    for (kind, pair) in installed.inner.programs.iter().enumerate() {
        assert_eq!(pair.primary.program_digest(), primary[kind]);
        assert_eq!(pair.secondary.program_digest(), secondary[kind]);
    }
    eprintln!(
        "authenticated signed fixture: {} byte-identical original bytes; four installed kinds; eight compiler pins matched",
        restored.len()
    );
}
