//! Native selection of a later generation's fifth program (ArchiveSent), and
//! the signed four-kind v4 generation as the fixed control. Nothing here
//! builds, signs or selects a generation: the signed v4 originals are read
//! only, and the five-kind geometry is computed over their signed dimensions.

use super::*;
use crate::deferral::{SelectorError, corner_kind, kind_corner};
use group::{Curve, Group};
use iroha_pasta::{Fp, Fq};

fn project(kind: usize, bits: [Fp; 2]) -> std::result::Result<carrier::ProjectedInputs, String> {
    carrier::ProjectedInputs::from_authenticated_fields(
        Fp::from(101),
        Fp::from(102),
        Fp::from(103),
        [Fp::from(7); crate::bootstrap_context::FIELDS],
        [Fp::from(9); crate::bootstrap_context::LINEAGE_FIELDS],
        crate::spartan::limbs_of_point::<Eq>(&Eq::generator().to_affine()),
        kind,
        bits,
    )
}

fn words(io: &[Fp]) -> [Fp; 3] {
    [io[full::KIND_IO], io[full::KIND_IO + 1], io[full::KIND_C2_IO]]
}

#[test]
fn archive_sent_projection_and_mode_name_only_its_own_corner() {
    let zero = [Fp::ZERO; 2];
    for kind in 0..4 {
        // The four signed kinds: c2 stays their zero unused input.
        let p = project(kind, zero).unwrap();
        assert_eq!(p.primary().io[full::KIND_C2_IO], Fp::ZERO);
        assert_eq!(p.secondary().io[full::KIND_C2_IO], Fq::ZERO);
        assert_eq!(corner_kind(4, words(&p.primary().io)), Ok(kind));
        assert_eq!(corner_kind(5, words(&p.primary().io)), Ok(kind));
    }
    let archive = project(full::ARCHIVE_SENT, zero).unwrap();
    let io = archive.primary().io.clone();
    assert_eq!(words(&io), [Fp::ZERO, Fp::ZERO, Fp::ONE]);
    assert_eq!(
        archive.secondary().io[full::KIND_IO..full::KIND_IO + 2],
        [Fq::ZERO, Fq::ZERO]
    );
    assert_eq!(archive.secondary().io[full::KIND_C2_IO], Fq::ONE);
    assert_eq!(io[crate::control::MODE_IO..crate::control::MODE_IO + 2], zero);
    assert_eq!(corner_kind(5, words(&io)), Ok(full::ARCHIVE_SENT));
    // Uninstalled under the signed four-kind generation.
    assert_eq!(corner_kind(4, words(&io)), Err(SelectorError::Uninstalled));
    assert_eq!(kind_corner(4, full::ARCHIVE_SENT), Err(SelectorError::Uninstalled));
    // No ordinary mode word selects ArchiveSent, and it has no mode.
    assert_eq!(crate::control::Mode::for_program(full::ARCHIVE_SENT, zero), Ok(None));
    for bits in [[Fp::ONE, Fp::ZERO], [Fp::ZERO, Fp::ONE], [Fp::ONE, Fp::ONE]] {
        assert!(project(full::ARCHIVE_SENT, bits).is_err());
        assert!(crate::control::Mode::for_program(full::ARCHIVE_SENT, bits).is_err());
    }
    // Out of range.
    for kind in [5, 7, 8, usize::MAX] {
        assert!(project(kind, zero).is_err(), "kind {kind}");
    }
    // Wrong kind: no four-kind projection is ArchiveSent's profile.
    for kind in 0..4 {
        assert_ne!(project(kind, zero).unwrap().primary().io, io);
    }
}

/// The alternative to a new selector coordinate, an existing program's
/// authenticated mode, has no unambiguous room for ArchiveSent: Control's two
/// mode words admit exactly its three modes (their product is constrained to
/// zero) and feed a statement tag linear in them, whose free corner would be
/// 11, never ArchiveSent's 5; every other program, Receive (the only one with
/// an incoming slot) included, admits only the zero mode. Folding would
/// therefore change an installed relation's public mode semantics.
#[test]
fn archive_sent_cannot_fold_into_an_existing_program_mode() {
    use crate::control::Mode;
    use iroha_data_model::kagemusha::KagemushaWalletOperationKindV1 as Operation;
    let words = |u: u64, r: u64| [Fp::from(u), Fp::from(r)];
    let corners = [(0u64, 0u64), (0, 1), (1, 0), (1, 1)];
    let admitted: Vec<_> = corners
        .into_iter()
        .filter(|&(u, r)| Mode::for_program(full::CONTROL, words(u, r)).is_ok())
        .collect();
    assert_eq!(admitted, vec![(0, 0), (0, 1), (1, 0)], "Control's mode space is full");
    for mode in [Mode::Load, Mode::RefreshTimeAnchor, Mode::Unload] {
        let [u, r] = mode.bits();
        assert_eq!(2 + 5 * r + 4 * u, mode.tag(), "Control's tag is linear in its words");
    }
    assert_eq!(u64::from(Operation::ArchiveSent.tag()), 5);
    assert_ne!(2 + 5 + 4, u64::from(Operation::ArchiveSent.tag()));
    for kind in [full::RECEIVE, full::SEND, full::BOOTSTRAP, full::ARCHIVE_SENT] {
        for (u, r) in [(0, 1), (1, 0), (1, 1)] {
            assert!(Mode::for_program(kind, words(u, r)).is_err(), "kind {kind} has no mode words");
        }
    }
}

#[test]
#[ignore = "requires KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS with the signed v4 p18-s17 programs"]
fn signed_v4_is_the_fixed_four_kind_control_and_five_kind_geometry_fits() {
    let directory =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let read = |name: &str| std::fs::read(directory.join(name)).unwrap();
    let parameters = crate::native::device_fixture::install(
        &read("program-bundle.norito"),
        &read("parameter-manifest.norito"),
        read("parameter-signature.bin").try_into().unwrap(),
    )
    .unwrap();
    let inner = &parameters.inner;
    let g = &inner.generators;

    // The signed generation is the four-kind profile, unchanged.
    assert_eq!(inner.programs.len(), 4);
    assert_eq!((g.defer_p.kinds.len(), g.defer_s.kinds.len()), (4, 4));
    assert_eq!((g.defer_p.x, g.defer_p.y, g.defer_s.x, g.defer_s.y), (24, 25, 24, 24));
    assert_eq!((g.defer_p.n(), g.defer_s.n()), (54, 53));
    assert_eq!((g.defer_p.degree(), g.defer_s.degree()), (186, 174));
    assert_eq!((g.defer_p.c1_at(), g.defer_p.c2_at()), (Some(53), None));
    assert_eq!((g.defer_s.c1_at(), g.defer_s.c2_at()), (Some(52), None));
    assert_eq!(g.keys_p.as_ref().map(Vec::len), Some(4));
    assert_eq!(g.keys_s.as_ref().map(Vec::len), Some(4));
    let empty = g.defer_p.empty_claim();
    assert_eq!((empty.point[0], empty.point[1], empty.point[53]), (Fp::ONE, Fp::ZERO, Fp::ONE));
    for kind in 0..4 {
        let (b0, b1) = deferral::kind_bits(kind);
        assert_eq!(kind_corner(4, kind), Ok([b0, b1, 0]));
    }

    // Native selection under it refuses ArchiveSent as uninstalled: no
    // trusted profile, no head of that kind (the bound is the installed
    // program count, so kind 3 passes the kind check and fails later).
    assert!(inner.profile(full::ARCHIVE_SENT).is_err());
    assert_eq!(kind_corner(g.defer_p.kinds.len(), full::ARCHIVE_SENT), Err(SelectorError::Uninstalled));
    let head = |kind: u8| {
        super::super::encode(&HeadFrame {
            version: 1,
            kind,
            parameters: *parameters.id(),
            profile: vec![[0; 32]; 64],
            pair: Vec::new(),
            credential: Vec::new(),
            certificate: Vec::new(),
        })
        .unwrap()
    };
    assert!(matches!(
        decode_head(&head(full::ARCHIVE_SENT as u8), &parameters),
        Err(Error::Malformed("canonical head"))
    ));
    let control = decode_head(&head(full::CONTROL as u8), &parameters);
    assert!(control.is_err() && !matches!(control, Err(Error::Malformed("canonical head"))));

    // A five-kind polynomial over the same signed dimensions. The Receive
    // program stands in for ArchiveSent's term: it has exactly the measured
    // Lambda_archive shape (P sx 22, sz 24; S sx 22, sz 22).
    let five_p = deferral::MatPoly::with_min_vec(
        inner
            .programs
            .iter()
            .map(|x| full::uniform_kind(x.primary.clone()))
            .chain([full::uniform_kind(inner.programs[full::RECEIVE].primary.clone())])
            .collect(),
        24,
        25,
    );
    let five_s = deferral::MatPoly::with_min_vec(
        inner
            .programs
            .iter()
            .map(|x| full::uniform_kind(x.secondary.clone()))
            .chain([full::uniform_kind(inner.programs[full::RECEIVE].secondary.clone())])
            .collect(),
        24,
        24,
    );
    assert_eq!(
        (inner.programs[full::RECEIVE].primary.shape.sx, inner.programs[full::RECEIVE].primary.shape.sz()),
        (22, 24)
    );
    assert_eq!((five_p.n(), five_s.n()), (55, 54));
    assert_eq!(
        (five_p.degree(), five_s.degree()),
        (186, 174),
        "the third selector bit keeps the signed fold degrees"
    );
    assert_eq!((five_p.c2_at(), five_s.c2_at()), (Some(54), Some(53)));
    // Recursive layout: each folded claim (n + 1 late words from LATE_IO)
    // stays aligned and below the link's io; q and the coordinate arrays keep
    // their power-of-two sizes.
    for (four, five) in [(g.defer_p.n(), five_p.n()), (g.defer_s.n(), five_s.n())] {
        let late = five + 1;
        assert!(full::LATE_IO + late <= full::LINK_RHO_IO, "late io {late} fits");
        assert_eq!(full::LATE_IO % late.next_power_of_two(), 0, "late io aligned");
        assert_eq!(five.next_power_of_two(), four.next_power_of_two());
        eprintln!(
            "five-kind late io {}..{} (four-kind ..{}), link at {}",
            full::LATE_IO,
            full::LATE_IO + late,
            full::LATE_IO + four + 1,
            full::LINK_RHO_IO
        );
    }
    assert_eq!(
        (five_p.degree() + 1).next_power_of_two(),
        (g.defer_p.degree() + 1).next_power_of_two()
    );
    // At c2 = 0 the five-kind polynomial is the signed four-kind one.
    let mut rng = crate::fs::SpikeRng::new(b"signed four-kind embedding");
    let p4: Vec<Fp> = (0..g.defer_p.n()).map(|_| rng.scalar()).collect();
    let mut p5 = p4.clone();
    p5.push(Fp::ZERO);
    assert_eq!(five_p.eval(&p5), g.defer_p.eval(&p4));
    let s4: Vec<Fq> = (0..g.defer_s.n()).map(|_| rng.scalar()).collect();
    let mut s5 = s4.clone();
    s5.push(Fq::ZERO);
    assert_eq!(five_s.eval(&s5), g.defer_s.eval(&s4));

    // Payment: one more full late word per side of the Send pair.
    let send = inner.profile(full::SEND).unwrap();
    let v4 = payment_wire::admit_send_profile(&send).unwrap();
    let grown = payment_wire::genuine_sizes(v4.pair + 64).unwrap();
    eprintln!(
        "Send Payment: pair {} -> {} B, envelope {} -> {} B (limit 10034), complete IPM1 {} -> {} B",
        v4.pair, grown.pair, v4.envelope, grown.envelope, v4.peer, grown.peer
    );
    assert_eq!(grown.envelope, v4.envelope + 64);
    assert!(grown.envelope <= 10_034, "the grown Send still fits the envelope");
}
