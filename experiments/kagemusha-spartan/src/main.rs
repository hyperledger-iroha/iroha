//! Byte-gate spike for the 12 KB QR Payment (Path 2, research/DESIGN-100X.md).
//!
//! `cargo run --release -- [tamper|bench|all]` (default `all`).

mod blocks;
mod census;
mod deferral;
mod fs;
mod full;
mod gadgets;
mod grid;
mod ipa;
mod kern;
mod ladders;
mod nonnative;
mod operation;
mod p256;
mod points;
mod prof;
mod r1cs;
mod scalar;
mod sha;
mod sparse;
mod spartan;
mod uniform;
mod verifier;
mod wired;

use std::time::Instant;

use ff::Field;
use iroha_pasta::{Ep as Pallas, Eq as Vesta, PastaCurve};

use crate::fs::SpikeRng;
use crate::ipa::{F, Gens};
use crate::r1cs::{Instance, Shape, Witness};

fn small(label: &'static str, k: usize) -> Shape {
    Shape {
        label,
        sx: 14,
        row_bits: 3,
        col_bits: 10,
        k,
    }
}

/// Codex round 4 shapes (REPORT4.md): Send primary 2^20 constraints and
/// 8 x 65,536; Send secondary 2^16 and 8 x 8,192; Receive doubles the rows.
const SEND_PRIMARY: Shape = Shape {
    label: "send-primary",
    sx: 20,
    row_bits: 3,
    col_bits: 16,
    k: 3,
};
const SEND_SECONDARY: Shape = Shape {
    label: "send-secondary",
    sx: 16,
    row_bits: 3,
    col_bits: 13,
    k: 3,
};
const RECEIVE_PRIMARY: Shape = Shape {
    label: "receive-primary",
    sx: 21,
    row_bits: 4,
    col_bits: 16,
    k: 3,
};
const RECEIVE_SECONDARY: Shape = Shape {
    label: "receive-secondary",
    sx: 17,
    row_bits: 4,
    col_bits: 13,
    k: 3,
};

fn tamper<C: PastaCurve>(curve: &str, shape: Shape, gens: &Gens<C>) {
    let mut rng = SpikeRng::new(shape.label.as_bytes());
    let (inst, wit) = r1cs::sample::<F<C>>(&shape, &mut rng);
    assert!(spartan::relation_holds::<C>(&shape, &inst, &wit));
    let (proof, _) = spartan::prove(&shape, gens, &inst, &wit, &mut rng);
    let bytes = proof.to_bytes();
    assert_eq!(bytes.len(), shape.proof_bytes());
    spartan::verify(&shape, gens, &inst, &bytes).expect("honest proof verifies");

    let elements = bytes.len() / 32;
    for element in 0..elements {
        for (byte, mask) in [(1, 0x10_u8), (17, 0x01)] {
            let mut bad = bytes.clone();
            bad[element * 32 + byte] ^= mask;
            assert!(
                spartan::verify(&shape, gens, &inst, &bad).is_err(),
                "{curve} k={}: element {element} byte {byte} accepted after a flip",
                shape.k
            );
        }
    }
    let truncated = &bytes[..bytes.len() - 32];
    assert!(spartan::verify(&shape, gens, &inst, truncated).is_err());

    let mut io = inst.io.clone();
    io[1] += F::<C>::ONE;
    assert!(
        spartan::verify(&shape, gens, &Instance { io }, &bytes).is_err(),
        "wrong io"
    );

    let qsz = (1 << shape.sw()) / 4;
    for (what, index, delta) in [
        ("product", 2 * qsz + 3, F::<C>::ONE),
        ("bit", 3 * qsz + 5, F::<C>::from(2)),
        (
            "masking product",
            2 * qsz + shape.active_cols() - 1,
            F::<C>::ONE,
        ),
    ] {
        let mut w = wit.w.clone();
        w[index] += delta;
        let bad_wit = Witness { w };
        assert!(
            !spartan::relation_holds::<C>(&shape, &inst, &bad_wit),
            "{what} breaks R1CS"
        );
        let (bad, _) = spartan::prove(&shape, gens, &inst, &bad_wit, &mut rng);
        assert!(
            spartan::verify(&shape, gens, &inst, &bad.to_bytes()).is_err(),
            "{curve}: unsatisfied {what} accepted"
        );
    }
    // Malicious provers on an unsatisfied witness (a bit set to 2).
    let mut w = wit.w.clone();
    w[3 * qsz + 5] += F::<C>::from(2);
    let bad_wit = Witness { w };
    let mut attacks = Vec::new();
    for cheat in [spartan::Cheat::CancelRound, spartan::Cheat::AdaptiveWitness] {
        let forged = spartan::prove_unseparated(&shape, gens, &inst, &bad_wit, &mut rng, cheat);
        let unseparated =
            spartan::verify_unseparated(&shape, gens, &inst, &forged.to_bytes()).is_ok();
        let (proof, _) = spartan::prove_with(&shape, gens, &inst, &bad_wit, &mut rng, cheat);
        assert!(
            spartan::verify(&shape, gens, &inst, &proof.to_bytes()).is_err(),
            "{curve}: {cheat:?} accepted by the separated construction"
        );
        attacks.push(format!(
            "{cheat:?} rejected (unseparated aggregate: {})",
            if unseparated { "ACCEPTED" } else { "rejected" }
        ));
    }
    println!(
        "tamper {curve:6} k={}: honest accepted; {} single-bit flips over {elements} elements, \
         truncation, wrong io, 3 unsatisfied witnesses rejected; {}",
        shape.k,
        2 * elements,
        attacks.join("; ")
    );
}

struct Measured {
    label: &'static str,
    curve: &'static str,
    bytes: usize,
}

fn bench<C: PastaCurve>(curve: &'static str, shape: Shape, gens: &Gens<C>) -> Measured {
    let mut rng = SpikeRng::new(shape.label.as_bytes());
    let clock = Instant::now();
    let (inst, wit) = r1cs::sample::<F<C>>(&shape, &mut rng);
    let sample_s = clock.elapsed().as_secs_f64();
    let (proof, tm) = spartan::prove(&shape, gens, &inst, &wit, &mut rng);
    let bytes = proof.to_bytes();
    assert_eq!(bytes.len(), shape.proof_bytes());
    let clock = Instant::now();
    spartan::verify(&shape, gens, &inst, &bytes).expect("verifies");
    let verify_s = clock.elapsed().as_secs_f64();
    let (ob, ib) = (shape.outer_blocks(), shape.inner_blocks());
    println!(
        "{:18} {curve:6} 2^{} constraints, {} x {} rows ({} active cols, tail {}), \
         rounds {:?}/{:?}: {} B | prove {:.3} s = commit {:.3} + outer {:.3} + inner {:.3} \
         + open {:.3} | verify {:.3} s (witness sample {:.3} s)",
        shape.label,
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        shape.tail_len(),
        ob,
        ib,
        bytes.len(),
        tm.total(),
        tm.commit,
        tm.outer,
        tm.inner,
        tm.open,
        verify_s,
        sample_s,
    );
    Measured {
        label: shape.label,
        curve,
        bytes: bytes.len(),
    }
}

fn bytes_by_k() {
    println!("\nexact proof bytes by variables per committed round (k):");
    println!(
        "{:18} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "shape", "k=1", "k=2", "k=3", "k=4", "k=5"
    );
    for shape in [
        SEND_PRIMARY,
        SEND_SECONDARY,
        RECEIVE_PRIMARY,
        RECEIVE_SECONDARY,
    ] {
        let row: Vec<String> = (1..=5)
            .map(|k| {
                let s = Shape { k, ..shape };
                if s.tail_len() < s.cols() {
                    s.proof_bytes().to_string()
                } else {
                    "-".into()
                }
            })
            .collect();
        println!(
            "{:18} {:>6} {:>6} {:>6} {:>6} {:>6}",
            shape.label, row[0], row[1], row[2], row[3], row[4]
        );
    }
}

fn pair_table(measured: &[Measured]) {
    let find = |label: &str| measured.iter().find(|m| m.label == label).map(|m| m.bytes);
    // Codex round 4 lines this spike does not change: 64 B header/binding per
    // proof; one inherited generator claim per field (point + one challenge
    // per IPA round), full-width or 128-bit challenges.
    let inherited = |rounds: usize, challenge: usize| 32 + rounds * challenge;
    println!(
        "\npair totals (proof bytes measured; header and inherited claim as counted in REPORT4.md):"
    );
    for (pair, primary, secondary, limit, codex_plain) in [
        ("Send", "send-primary", "send-secondary", 8_458, 8_416),
        (
            "Receive",
            "receive-primary",
            "receive-secondary",
            9_600,
            9_120,
        ),
    ] {
        let (Some(p), Some(s)) = (find(primary), find(secondary)) else {
            continue;
        };
        let full = p + s + 2 * 64 + inherited(16, 32) + inherited(13, 32);
        let half = p + s + 2 * 64 + inherited(16, 16) + inherited(13, 16);
        println!(
            "{pair:8} ZK pair {full} B (128-bit inherited challenges: {half} B) vs limit {limit} B \
             -> spare {} B ({} B); Codex non-ZK plain count {codex_plain} B",
            limit as i64 - full as i64,
            limit as i64 - half as i64,
        );
    }
    let _ = measured.iter().map(|m| m.curve).count();
}

/// The first 2^13 Vesta generators (a prefix of the 2^16 set).
fn pallas_free_gens(full: &Gens<Vesta>) -> Gens<Vesta> {
    full.prefix(1 << 13)
}

fn census_shape(label: &'static str, constraints: usize, witness: usize, col_bits: usize) -> Shape {
    let sx = constraints.next_power_of_two().trailing_zeros() as usize;
    let mut row_bits = 0;
    loop {
        let shape = Shape {
            label,
            sx,
            row_bits,
            col_bits,
            k: 3,
        };
        if shape.rows() * shape.active_cols() >= witness {
            return Shape {
                sx: sx.max(shape.sw()),
                ..shape
            };
        }
        row_bits += 1;
    }
}

struct FixedPoint {
    p_send: census::ProofShape,
    s_send: census::ProofShape,
    p_recv: census::ProofShape,
    s_recv: census::ProofShape,
    send_p: usize,
    send_s: usize,
    recv_p: usize,
    recv_s: usize,
    send_pair: usize,
    recv_pair: usize,
    iterations: usize,
}

/// App reference (REPORT4.md census of the whole primary relation: fresh
/// values plus helpers), added to the measured recursion overhead.
const APP_SEND: usize = 428_000 + 131_072;
const APP_RECEIVE: usize = 680_000 + 196_608;

fn fixed_point(
    p_cols: usize,
    s_cols: usize,
    binding: census::Binding,
    fp: &gadgets::GadgetCosts,
    fq: &gadgets::GadgetCosts,
) -> FixedPoint {
    fixed_point_with(p_cols, s_cols, binding, census::Inheritance::Joint, fp, fq)
}

fn fixed_point_with(
    p_cols: usize,
    s_cols: usize,
    binding: census::Binding,
    inheritance: census::Inheritance,
    fp: &gadgets::GadgetCosts,
    fq: &gadgets::GadgetCosts,
) -> FixedPoint {
    use census::{Inheritance, ProofShape, operation};
    let joint = inheritance == Inheritance::Joint;
    let proof = |label: &'static str, c: usize, w: usize, cols: usize, sources: usize| ProofShape {
        shape: census_shape(label, c, w, cols),
        inherited: if joint { sources } else { 0 },
        exported: if joint { sources } else { 1 },
    };
    let mut p_send = proof("P-send", 1 << 20, 1 << 19, p_cols, 1);
    let mut s_send = proof("S-send", 1 << 16, 1 << 16, s_cols, 1);
    let mut p_recv = proof("P-recv", 1 << 21, 1 << 20, p_cols, 2);
    let mut s_recv = proof("S-recv", 1 << 17, 1 << 17, s_cols, 2);
    for iterations in 1..=16 {
        let send = [
            operation(&[(p_send, s_send)], fp, fq, binding, inheritance),
            operation(&[(p_recv, s_recv)], fp, fq, binding, inheritance),
        ];
        let recv = [
            operation(
                &[(p_send, s_send), (p_send, s_send)],
                fp,
                fq,
                binding,
                inheritance,
            ),
            operation(
                &[(p_recv, s_recv), (p_send, s_send)],
                fp,
                fq,
                binding,
                inheritance,
            ),
        ];
        let send_p = send.iter().map(|o| o.primary).max().unwrap();
        let send_s = send.iter().map(|o| o.secondary).max().unwrap();
        let recv_p = recv.iter().map(|o| o.primary).max().unwrap();
        let recv_s = recv.iter().map(|o| o.secondary).max().unwrap();
        let next = (
            proof("P-send", send_p + APP_SEND, send_p + APP_SEND, p_cols, 1),
            proof("S-send", send_s, send_s, s_cols, 1),
            proof(
                "P-recv",
                recv_p + APP_RECEIVE,
                recv_p + APP_RECEIVE,
                p_cols,
                2,
            ),
            proof("S-recv", recv_s, recv_s, s_cols, 2),
        );
        let key = |p: &ProofShape| (p.shape.sx, p.shape.row_bits);
        let stable = key(&next.0) == key(&p_send)
            && key(&next.1) == key(&s_send)
            && key(&next.2) == key(&p_recv)
            && key(&next.3) == key(&s_recv);
        (p_send, s_send, p_recv, s_recv) = next;
        if stable || iterations == 16 {
            let claim = |p: &ProofShape| p.exported * (32 + 16 * p.shape.col_bits);
            let send_bytes = send.iter().map(|o| o.transmitted_bytes).max().unwrap();
            let recv_bytes = recv.iter().map(|o| o.transmitted_bytes).max().unwrap();
            let send_pair = p_send.shape.proof_bytes()
                + s_send.shape.proof_bytes()
                + 128
                + claim(&p_send)
                + claim(&s_send)
                + send_bytes;
            let recv_pair = p_recv.shape.proof_bytes()
                + s_recv.shape.proof_bytes()
                + 128
                + claim(&p_recv)
                + claim(&s_recv)
                + recv_bytes;
            return FixedPoint {
                p_send,
                s_send,
                p_recv,
                s_recv,
                send_p,
                send_s,
                recv_p,
                recv_s,
                send_pair,
                recv_pair,
                iterations,
            };
        }
    }
    unreachable!()
}

/// Pixel 6 prover-core model fitted to PIXEL.md (8 threads): outer sumcheck
/// 0.26 us/constraint, inner 0.2 us per z entry, commitments 0.68 us/entry
/// (rows of >= 65,536) to 1.1 us/entry (8,192), IPA 0.17 s + 18.3 us/column.
fn pixel_seconds(shape: &Shape) -> f64 {
    let constraints = (1_u64 << shape.sx) as f64;
    let z = (1_u64 << shape.sz()) as f64;
    let entries = (shape.rows() * shape.active_cols()) as f64;
    let per_entry = if shape.col_bits >= 16 {
        0.68e-6
    } else if shape.col_bits >= 14 {
        0.85e-6
    } else {
        1.1e-6
    };
    let ipa = 0.17 + 18.3e-6 * shape.cols() as f64;
    0.26e-6 * constraints + 0.2e-6 * z + per_entry * entries + ipa
}

fn run_census() {
    use census::{Binding, verify_cost};
    let fp = gadgets::measure::<Pallas>(1);
    let fq = gadgets::measure::<Vesta>(2);
    println!(
        "measured R1CS gadget costs (constraints; Fp circuit = Pallas points, Fq circuit = Vesta points):"
    );
    for (name, a, b) in [
        (
            "endoscale (128-bit challenge, variable base)",
            fp.endoscale_128,
            fq.endoscale_128,
        ),
        ("variable-base 255-bit mul", fp.var_mul_255, fq.var_mul_255),
        (
            "fixed-base 255-bit mul (2-bit windows)",
            fp.fixed_mul_255,
            fq.fixed_mul_255,
        ),
        ("incomplete point addition", fp.point_add, fq.point_add),
        ("on-curve check", fp.on_curve, fq.on_curve),
        ("RP57 permutation", fp.poseidon, fq.poseidon),
        (
            "canonical squeeze decomposition (2 challenges)",
            fp.squeeze_bits,
            fq.squeeze_bits,
        ),
    ] {
        println!("  {name:48} Fp {a:>6}  Fq {b:>6}");
    }
    println!(
        "\nfixed points (P cols / S cols, binding): circuit sizes are recursion overhead; P adds the app \
         reference {APP_SEND} (Send) / {APP_RECEIVE} (Receive)"
    );
    println!(
        "{:>7} {:>6} {:9} | {:>8} {:>8} {:>8} {:>8} | {:>16} {:>16} {:>16} {:>16} | {:>6} {:>6} | {:>5} {:>5}",
        "P cols",
        "S cols",
        "binding",
        "Send CP",
        "Send CS",
        "Recv CP",
        "Recv CS",
        "P-send",
        "S-send",
        "P-recv",
        "S-recv",
        "SendB",
        "RecvB",
        "Send s",
        "Recv s"
    );
    let mut best: Option<(f64, String)> = None;
    for p_cols in [14, 15, 16, 17] {
        for s_cols in [12, 13, 14] {
            for binding in [Binding::Rlc, Binding::Transmit] {
                let f = fixed_point(p_cols, s_cols, binding, &fp, &fq);
                let shape = |p: &census::ProofShape| {
                    format!("2^{} {}x2^{}", p.shape.sx, p.shape.rows(), p.shape.col_bits)
                };
                let send_s = pixel_seconds(&f.p_send.shape) + pixel_seconds(&f.s_send.shape);
                let recv_s = pixel_seconds(&f.p_recv.shape) + pixel_seconds(&f.s_recv.shape);
                let line = format!(
                    "{:>7} {:>6} {:9} | {:>8} {:>8} {:>8} {:>8} | {:>16} {:>16} {:>16} {:>16} | {:>6} {:>6} | {:>5.2} {:>5.2}{}",
                    1 << p_cols,
                    1 << s_cols,
                    format!("{binding:?}"),
                    f.send_p,
                    f.send_s,
                    f.recv_p,
                    f.recv_s,
                    shape(&f.p_send),
                    shape(&f.s_send),
                    shape(&f.p_recv),
                    shape(&f.s_recv),
                    f.send_pair,
                    f.recv_pair,
                    send_s,
                    recv_s,
                    if f.iterations == 16 {
                        " (not stable)"
                    } else {
                        ""
                    }
                );
                println!("{line}");
                if f.send_pair <= 8_458 && f.recv_pair <= 9_600 {
                    let score = send_s.max(recv_s);
                    if best.as_ref().is_none_or(|(b, _)| score < *b) {
                        best = Some((score, line));
                    }
                }
            }
        }
    }
    if let Some((_, line)) = best {
        println!("\nfastest shape within both byte limits (by slower operation):\n{line}");
    }
    // Inherited-claim re-randomization (owner decision): private masked
    // restructuring per curve versus folding in clear.
    println!(
        "\ninherited claims: fold in clear (Joint) vs private masked restructuring (Restructure), Rlc binding:"
    );
    for (p_cols, s_cols) in [(15, 13), (16, 13), (16, 14)] {
        for inheritance in [census::Inheritance::Joint, census::Inheritance::Restructure] {
            let f = fixed_point_with(p_cols, s_cols, Binding::Rlc, inheritance, &fp, &fq);
            let shape = |p: &census::ProofShape| {
                format!("2^{} {}x2^{}", p.shape.sx, p.shape.rows(), p.shape.col_bits)
            };
            let core_send = pixel_seconds(&f.p_send.shape) + pixel_seconds(&f.s_send.shape);
            let core_recv = pixel_seconds(&f.p_recv.shape) + pixel_seconds(&f.s_recv.shape);
            // Restructuring prover: one IPA per curve plus a full-width mask
            // commitment (2.05 us/point on the Pixel, PIXEL-pasta kernels).
            let extra = |cols: usize| 0.17 + 18.3e-6 * cols as f64 + 2.05e-6 * cols as f64;
            let restructure = if inheritance == census::Inheritance::Restructure {
                extra(1 << p_cols) + extra(1 << s_cols)
            } else {
                0.0
            };
            println!(
                "  P {:>6} / S {:>6} {:11} | Send CP {:>7} CS {:>7} | Recv CP {:>7} CS {:>7} | {} {} {} {} | pairs {} / {} B | \
                 Pixel core Send {:.2} s, Receive {:.2} s (restructuring IPAs {:.2} s included)",
                1 << p_cols,
                1 << s_cols,
                format!("{inheritance:?}"),
                f.send_p,
                f.send_s,
                f.recv_p,
                f.recv_s,
                shape(&f.p_send),
                shape(&f.s_send),
                shape(&f.p_recv),
                shape(&f.s_recv),
                f.send_pair,
                f.recv_pair,
                core_send + restructure,
                core_recv + restructure,
                restructure
            );
        }
    }

    let f = fixed_point(16, 13, Binding::Rlc, &fp, &fq);
    let vp = verify_cost(&f.p_send, &fq);
    let vs = verify_cost(&f.s_send, &fp);
    println!(
        "\nat P 65,536 / S 8,192 (Rlc): one P-send verified = transcript {} + group {} in C_S, scalar {} in C_P, \
         {} shared values; one S-send verified = transcript {} + group {} in C_P, scalar {} in C_S, {} shared",
        vp.transcript,
        vp.group,
        vp.scalar,
        vp.shared,
        vs.transcript,
        vs.group,
        vs.scalar,
        vs.shared
    );
}

/// Pixel 6 commitment cost per witness entry (microseconds, 8 threads) by
/// width class and row length. Anchors: b04a57's 2^18 single-MSM kernel
/// (1-bit 0.118, 8-bit 0.187, 16-bit 0.29 us) scaled by a row-length factor
/// fitted to the spike's measured mix (64/16/80/1-bit rows: 0.72 us at
/// 65,536 columns, 1.13 us at 8,192); full width 2.04 us at 65,536 (measured),
/// scaled to shorter rows by iroha_pasta's MSM planner model; widths between
/// 16 and 255 bits interpolated linearly. A model until `commit-classes` runs
/// on the phone.
fn commit_us(bits: u16, cols: usize) -> f64 {
    match std::env::var("ZKS_COMMIT").as_deref() {
        Ok("model") => commit_us_model(bits, cols),
        Ok("rowloop") => commit_us_measured(bits, cols),
        _ => commit_us_lanes(bits, cols),
    }
}

/// Row length for the commit-cost lookup: lane commits split a proof with
/// fewer than 16 rows into column pieces so that 16 pieces share the 8 lanes,
/// as in the measured 16-row batches (b04a57).
fn commit_cols(shape: &Shape) -> usize {
    match std::env::var("ZKS_COMMIT").as_deref() {
        Ok("model") | Ok("rowloop") => shape.cols(),
        _ => shape.cols() / (16 / shape.rows().max(1)).max(1),
    }
}

/// IPA prover seconds. Default: b04a57's Pixel 6 `ipa-split` medians with
/// endoscalar challenges u = a zeta + b (as the spike draws them) after the
/// GLV rounding fix and the MSM planner work (iroha 8ca1643b3f; mean of two
/// runs' medians, 32.5 C): L/R MSMs + generator fold + rest, plus 0.01 s for
/// the RP57 transcript (~100 permutations) and the final Schnorr step, which
/// the split leaves out (an allowance, not measured). `ZKS_IPA=fullwidth`
/// gives the earlier full-width-challenge table, `ZKS_IPA=model` the fit.
fn ipa_seconds(cols: usize) -> f64 {
    let model = |c: usize| 0.17 + 18.3e-6 * c as f64;
    let which = std::env::var("ZKS_IPA");
    if which.as_deref() == Ok("model") {
        return model(cols);
    }
    const FULLWIDTH: [(usize, f64); 4] = [
        (8_192, 0.283),
        (16_384, 0.493),
        (32_768, 0.855),
        (65_536, 1.473),
    ];
    const ENDO_GLV: [(usize, f64); 4] = [
        (8_192, 0.226),
        (16_384, 0.342),
        (32_768, 0.535),
        (65_536, 0.939),
    ];
    let table = if which.as_deref() == Ok("fullwidth") {
        &FULLWIDTH
    } else {
        &ENDO_GLV
    };
    let (lo, hi) = (table[0].1, table[3].1);
    match table.iter().find(|(c, _)| *c == cols) {
        Some((_, seconds)) => *seconds,
        None if cols < 8_192 => model(cols) * lo / model(8_192),
        None => model(cols) * hi / model(65_536),
    }
}

#[allow(dead_code)]
fn ipa_seconds_previous(cols: usize) -> f64 {
    let model = |c: usize| 0.17 + 18.3e-6 * c as f64;
    const MEASURED: [(usize, f64); 4] = [
        (8_192, 0.283),
        (16_384, 0.493),
        (32_768, 0.855),
        (65_536, 1.473),
    ];
    match MEASURED.iter().find(|(c, _)| *c == cols) {
        Some((_, seconds)) => *seconds,
        None if cols < 8_192 => model(cols) * 0.283 / model(8_192),
        None => model(cols) * 1.473 / model(65_536),
    }
}

/// Pixel 6 lane commits (b04a57, PIXEL.md "Lane commits", 16-row batches, one
/// single-threaded plan per row); a narrow class takes the cheaper of its own
/// plan and a 16-bit plan. The default since 2026-11-08: `ZKS_COMMIT=rowloop`
/// gives the per-row loop's calibration, `ZKS_COMMIT=model` gate 1c's model.
fn commit_us_lanes(bits: u16, cols: usize) -> f64 {
    const BITS: [f64; 9] = [1.0, 11.0, 13.0, 14.0, 16.0, 20.0, 88.0, 180.0, 255.0];
    const TABLE: [(usize, [f64; 9]); 4] = [
        (
            8_192,
            [
                0.153, 0.269, 0.269, 0.277, 0.274, 0.289, 0.955, 1.326, 2.405,
            ],
        ),
        (
            16_384,
            [
                0.094, 0.144, 0.235, 0.236, 0.224, 0.246, 0.775, 1.110, 2.143,
            ],
        ),
        (
            32_768,
            [
                0.080, 0.113, 0.222, 0.227, 0.199, 0.184, 0.743, 1.006, 1.978,
            ],
        ),
        (
            65_536,
            [
                0.085, 0.105, 0.127, 0.222, 0.202, 0.186, 0.680, 0.971, 1.916,
            ],
        ),
    ];
    let row = |c: usize| -> f64 {
        let costs = TABLE
            .iter()
            .find(|(n, _)| *n == c)
            .map(|(_, v)| v)
            .expect("measured width");
        let b = (bits as f64).clamp(1.0, 255.0);
        for i in 0..BITS.len() - 1 {
            if b <= BITS[i + 1] {
                return costs[i]
                    + (costs[i + 1] - costs[i]) * (b - BITS[i]) / (BITS[i + 1] - BITS[i]);
            }
        }
        costs[BITS.len() - 1]
    };
    if cols < 8_192 {
        row(8_192) * commit_us_model(bits, cols) / commit_us_model(bits, 8_192)
    } else {
        row(cols.min(65_536))
    }
}

/// Pixel 6 calibration (b04a57, PIXEL.md "Commit-cost calibration"): medians
/// of three `commit-classes` runs, 8 threads, µs per entry; linear in bits
/// between measured widths; below 8,192 columns scaled by the model's ratio.
/// The default since 2026-11-08; `ZKS_COMMIT=model` reproduces gate 1c's model.
fn commit_us_measured(bits: u16, cols: usize) -> f64 {
    const BITS: [f64; 10] = [1.0, 8.0, 13.0, 16.0, 32.0, 64.0, 88.0, 128.0, 180.0, 255.0];
    const TABLE: [(usize, [f64; 10]); 4] = [
        (
            8_192,
            [
                0.209, 0.401, 0.755, 0.607, 1.057, 1.624, 2.204, 2.519, 3.383, 4.473,
            ],
        ),
        (
            16_384,
            [
                0.208, 0.446, 1.119, 0.574, 1.242, 2.239, 1.822, 1.930, 1.893, 3.759,
            ],
        ),
        (
            32_768,
            [
                0.228, 0.474, 0.848, 0.463, 0.700, 1.364, 1.308, 1.980, 1.429, 2.399,
            ],
        ),
        (
            65_536,
            [
                0.161, 0.344, 0.468, 0.371, 0.536, 1.112, 1.199, 1.535, 1.550, 2.196,
            ],
        ),
    ];
    let row = |c: usize| -> f64 {
        let costs = TABLE
            .iter()
            .find(|(n, _)| *n == c)
            .map(|(_, v)| v)
            .expect("measured width");
        let b = (bits as f64).clamp(1.0, 255.0);
        for i in 0..BITS.len() - 1 {
            if b <= BITS[i + 1] {
                return costs[i]
                    + (costs[i + 1] - costs[i]) * (b - BITS[i]) / (BITS[i + 1] - BITS[i]);
            }
        }
        costs[BITS.len() - 1]
    };
    if cols < 8_192 {
        row(8_192) * commit_us_model(bits, cols) / commit_us_model(bits, 8_192)
    } else {
        row(cols.min(65_536))
    }
}

fn commit_us_model(bits: u16, cols: usize) -> f64 {
    let lg = (cols as f64).log2();
    let factor = 1.685 + (16.0 - lg) / 3.0 * (2.854 - 1.685);
    let planner = match cols {
        c if c <= 8_192 => 24.58 / 18.65,
        c if c <= 16_384 => 22.04 / 18.65,
        c if c <= 32_768 => 20.34 / 18.65,
        _ => 1.0,
    };
    let full = 2.04 * planner;
    let kernel = |b: f64| -> f64 {
        let anchors = [(1.0, 0.118), (8.0, 0.187), (16.0, 0.29)];
        if b <= 1.0 {
            return 0.118;
        }
        for w in anchors.windows(2) {
            let ((b0, c0), (b1, c1)) = (w[0], w[1]);
            if b <= b1 {
                return c0 + (c1 - c0) * (b - b0) / (b1 - b0);
            }
        }
        0.29
    };
    let b = bits as f64;
    if b <= 16.0 {
        (kernel(b) * factor).min(full)
    } else if b >= 255.0 {
        full
    } else {
        let c16 = 0.29 * factor;
        c16 + (full - c16) * (b - 16.0) / 239.0
    }
}

/// The same model, by phase: outer, inner, commitments by class group
/// (full width, 1-bit, narrow chunks, other), density, IPA.
fn pixel_breakdown(
    shape: &Shape,
    widths: &std::collections::BTreeMap<u16, usize>,
    rows: usize,
    nnz: usize,
) -> String {
    let constraints = (1_u64 << shape.sx) as f64;
    let z = (1_u64 << shape.sz()) as f64;
    let part = |f: &dyn Fn(u16) -> bool| -> f64 {
        widths
            .iter()
            .filter(|(w, _)| f(**w))
            .map(|(w, n)| *n as f64 * commit_us(*w, commit_cols(shape)))
            .sum::<f64>()
            * 1e-6
    };
    let full = part(&|w| w >= 128);
    let bits = part(&|w| w <= 1);
    let narrow = part(&|w| (2..=20).contains(&w));
    let other = part(&|w| (21..128).contains(&w));
    let density = 0.03e-6 * (nnz as f64 - 3.0 * rows as f64).max(0.0);
    let ipa = ipa_seconds(shape.cols());
    let total = 0.26e-6 * constraints + 0.2e-6 * z + full + bits + narrow + other + density + ipa;
    format!(
        "outer {:.2} inner {:.2} commit {:.2} (full {:.2}, 1-bit {:.2}, chunks {:.2}, other {:.2}) density {:.2} IPA {:.2} = {:.2} s",
        0.26e-6 * constraints,
        0.2e-6 * z,
        full + bits + narrow + other,
        full,
        bits,
        narrow,
        other,
        density,
        ipa,
        total
    )
}

/// Prover-core model of a uniform proof with the review lane's kernels,
/// calibrated on b04a57's Pixel 6 run of 2026-11-10 08:51Z (stage A's real
/// Receive pair, `ebb983-review/runs/pixel6-real-receive-kern-summary.txt`):
/// - outer rounds, folds, eq and commits: 77.7 ms at 2^19 constraints (P),
///   44 ms at 2^18 (S): 0.155 us per constraint slot;
/// - inner rounds, folds, eq, commits and support: 69 ms (P) and 40 ms (S)
///   with the public half skipped: 0.14 us per witness entry of the half,
///   halved again when at most half the witness rows hold cells (the corner
///   support then drops the top row bit);
/// - the uniform `A z, B z, C z` and `M(r_x, .)`: the spike's Mac kernels
///   (3.2 and 8.7 ms for P-recv's 402,075 rows) times the Pixel/Mac ratios of
///   stage A's sparse ones (7.3x and 9x): 0.058 and 0.19 us per row;
/// - commits: the lane table times 0.78 (fixed-base tables, Pixel A/B
///   2026-11-09: commits per pair 1.238 -> 0.970 s);
/// - IPA: the table times 0.76 (p.ipa 0.26 s at 16,384 columns, 0.1725 s at
///   8,192, against 0.342 and 0.226).
///
/// Returns the breakdown and the total, and the total if at most half the
/// rows held cells.
fn pixel_kern(
    shape: &Shape,
    widths: &std::collections::BTreeMap<u16, usize>,
    rows: usize,
    used_rows: usize,
) -> String {
    let half = (1_u64 << (shape.sz() - 1)) as f64;
    let outer = 0.155e-6 * (1_u64 << shape.sx) as f64;
    let abc = 0.058e-6 * rows as f64;
    let inner_at = |used: usize| -> f64 {
        let mut entries = half;
        let mut r = shape.rows();
        while r > 1 && used <= r / 2 {
            entries /= 2.0;
            r /= 2;
        }
        0.14e-6 * entries
    };
    let mtable = 0.19e-6 * rows as f64;
    let commit = 0.78
        * widths
            .iter()
            .map(|(w, n)| *n as f64 * commit_us(*w, commit_cols(shape)))
            .sum::<f64>()
        * 1e-6;
    let ipa = 0.76 * ipa_seconds(shape.cols());
    let (inner, inner_half) = (inner_at(used_rows), inner_at(shape.rows() / 2));
    let total = outer + abc + inner + mtable + commit + ipa;
    format!(
        "outer {:.2} (abc {:.2}) inner {:.2} (m_table {:.2}) commit {:.2} IPA {:.2} = {:.2} s ({} of {} rows used; at most {} rows: inner {:.2}, total {:.2} s)",
        outer + abc,
        abc,
        inner + mtable,
        mtable,
        commit,
        ipa,
        total,
        used_rows,
        shape.rows(),
        shape.rows() / 2,
        inner_half + mtable,
        total - inner + inner_half
    )
}

/// Prover-core model for a proof whose witness has the given width classes
/// (real entries only: zero padding costs nothing to commit).
fn pixel_seconds_widths(
    shape: &Shape,
    widths: &std::collections::BTreeMap<u16, usize>,
    rows: usize,
    nnz: usize,
) -> f64 {
    let constraints = (1_u64 << shape.sx) as f64;
    let z = (1_u64 << shape.sz()) as f64;
    let commit: f64 = widths
        .iter()
        .map(|(w, n)| *n as f64 * commit_us(*w, commit_cols(shape)))
        .sum::<f64>()
        * 1e-6;
    // The fitted outer/inner rates include the spike's ~3 nonzeros per row.
    let density = 0.03e-6 * (nnz as f64 - 3.0 * rows as f64).max(0.0);
    let ipa = ipa_seconds(shape.cols());
    0.26e-6 * constraints + 0.2e-6 * z + commit + density + ipa
}

/// Dedicated rows holding the per-wallet table limbs (6 per entry).
fn table_rows(entries: usize, p_cols: usize) -> usize {
    (6 * entries).div_ceil(1 << p_cols)
}

/// Secondary-side comparison of a verified P-proof's table-row commitment
/// with four 128-bit limbs: two coordinate decompositions with range checks.
const TABLE_ROW_COMPARE: usize = 600;

fn fp_poseidon(fp: &gadgets::GadgetCosts) -> usize {
    fp.poseidon
}

struct OpFixedPoint {
    /// Recursion-overhead witness classes (one-bit, narrow) of the largest
    /// Send / Receive orders.
    send_p_bits: usize,
    send_p_narrow: usize,
    send_s_bits: usize,
    recv_p_bits: usize,
    recv_p_narrow: usize,
    recv_s_bits: usize,
    p_send: census::ProofShape,
    s_send: census::ProofShape,
    p_recv: census::ProofShape,
    s_recv: census::ProofShape,
    send_p: usize,
    send_s: usize,
    recv_p: usize,
    recv_s: usize,
    send_pair: usize,
    recv_pair: usize,
    iterations: usize,
}

/// Fixed point with the operation census (constraints and witness of the
/// Send / Receive relations including their lookup argument) in place of
/// REPORT4's reference, inherited claims folded in clear (owner, 2026-11-08).
fn fixed_point_op(
    p_cols: usize,
    s_cols: usize,
    fp: &gadgets::GadgetCosts,
    fq: &gadgets::GadgetCosts,
    send: &operation::WithLookups,
    recv: &operation::WithLookups,
    step: census::StepCost,
    start_low: bool,
) -> OpFixedPoint {
    let tables = recv.wallet_entries > 0;
    use census::{Binding, Inheritance, ProofShape};
    // Per-wallet comb tables (lever 1): each state absorbs the table-row
    // commitment as four 128-bit limbs (two permutations); each verified
    // P-proof's table row is compared, in the secondary, with those limbs
    // carried across by the binding (four shared values).
    let operation = |pairs: &[(ProofShape, ProofShape)], fp, fq, binding, inh| {
        let mut o = census::operation_with(pairs, fp, fq, binding, inh, step);
        if tables {
            o.primary += 2 * fp_poseidon(fp) + pairs.len() * 4 * step.rows;
            o.primary_narrow += pairs.len() * 4 * step.narrow;
            o.secondary += pairs.len() * (TABLE_ROW_COMPARE + 4);
        }
        o
    };
    let binding = Binding::Rlc;
    let inh = Inheritance::Joint;
    let proof = |label: &'static str, c: usize, w: usize, cols: usize, sources: usize| ProofShape {
        shape: census_shape(label, c, w, cols),
        inherited: sources,
        exported: sources,
    };
    let wsum = |o: &operation::WithLookups| o.witness.values().sum::<usize>();
    // Gate 1c started from above (2^19 / 2^20 primaries), which finds the
    // largest stable shapes; starting from small shapes finds the least
    // stable (self-consistent) ones, since every size grows with the
    // predecessor's shape.
    let (ps, ss, pr, sr) = if start_low {
        (16, 15, 16, 15)
    } else {
        (19, 17, 20, 18)
    };
    let mut p_send = proof("P-send", 1 << ps, 1 << ps, p_cols, 1);
    let mut s_send = proof("S-send", 1 << ss, 1 << ss, s_cols, 1);
    let mut p_recv = proof("P-recv", 1 << pr, 1 << pr, p_cols, 2);
    let mut s_recv = proof("S-recv", 1 << sr, 1 << sr, s_cols, 2);
    for iterations in 1..=16 {
        let sends = [
            operation(&[(p_send, s_send)], fp, fq, binding, inh),
            operation(&[(p_recv, s_recv)], fp, fq, binding, inh),
        ];
        let recvs = [
            operation(&[(p_send, s_send), (p_send, s_send)], fp, fq, binding, inh),
            operation(&[(p_recv, s_recv), (p_send, s_send)], fp, fq, binding, inh),
        ];
        let send_p = sends.iter().map(|o| o.primary).max().unwrap();
        let send_s = sends.iter().map(|o| o.secondary).max().unwrap();
        let recv_p = recvs.iter().map(|o| o.primary).max().unwrap();
        let recv_s = recvs.iter().map(|o| o.secondary).max().unwrap();
        let pick = |v: &[census::OperationCost; 2],
                    by: fn(&census::OperationCost) -> usize,
                    f: fn(&census::OperationCost) -> usize| {
            f(v.iter().max_by_key(|o| by(o)).unwrap())
        };
        let (send_p_bits, send_p_narrow, send_s_bits) = (
            pick(&sends, |o| o.primary, |o| o.primary_bits),
            pick(&sends, |o| o.primary, |o| o.primary_narrow),
            pick(&sends, |o| o.secondary, |o| o.secondary_bits),
        );
        let (recv_p_bits, recv_p_narrow, recv_s_bits) = (
            pick(&recvs, |o| o.primary, |o| o.primary_bits),
            pick(&recvs, |o| o.primary, |o| o.primary_narrow),
            pick(&recvs, |o| o.secondary, |o| o.secondary_bits),
        );
        // Binding chunks are witness entries beyond the constraint count; one
        // spare row lets the two witness phases (main rows, then LogUp
        // helpers after alpha) each occupy whole rows.
        let slack = (1usize << p_cols)
            * (1 + table_rows(recv.wallet_entries.max(send.wallet_entries), p_cols));
        let next = (
            proof(
                "P-send",
                send_p + send.constraints,
                send_p + send_p_narrow + wsum(send) + slack,
                p_cols,
                1,
            ),
            proof("S-send", send_s, send_s, s_cols, 1),
            proof(
                "P-recv",
                recv_p + recv.constraints,
                recv_p + recv_p_narrow + wsum(recv) + slack,
                p_cols,
                2,
            ),
            proof("S-recv", recv_s, recv_s, s_cols, 2),
        );
        let key = |p: &ProofShape| (p.shape.sx, p.shape.row_bits);
        let stable = key(&next.0) == key(&p_send)
            && key(&next.1) == key(&s_send)
            && key(&next.2) == key(&p_recv)
            && key(&next.3) == key(&s_recv);
        (p_send, s_send, p_recv, s_recv) = next;
        if stable || iterations == 16 {
            let claim = |p: &ProofShape| p.exported * (32 + 16 * p.shape.col_bits);
            let send_pair = p_send.shape.proof_bytes()
                + s_send.shape.proof_bytes()
                + 128
                + claim(&p_send)
                + claim(&s_send);
            let recv_pair = p_recv.shape.proof_bytes()
                + s_recv.shape.proof_bytes()
                + 128
                + claim(&p_recv)
                + claim(&s_recv);
            return OpFixedPoint {
                send_p_bits,
                send_p_narrow,
                send_s_bits,
                recv_p_bits,
                recv_p_narrow,
                recv_s_bits,
                p_send,
                s_send,
                p_recv,
                s_recv,
                send_p,
                send_s,
                recv_p,
                recv_s,
                send_pair,
                recv_pair,
                iterations,
            };
        }
    }
    unreachable!()
}

/// Fixed points for one census method (gate 1c's signatures, or lever 1's
/// comb tables), for the ordinary and the largest variants.
fn print_op_fixed_points(
    method: &str,
    best: &std::collections::BTreeMap<String, (f64, usize, usize, operation::Cost)>,
    fp: &gadgets::GadgetCosts,
    fq: &gadgets::GadgetCosts,
) -> Vec<(String, f64, f64, usize, usize)> {
    let mut out = Vec::new();
    for (title, send_label, recv_label) in [
        ("ordinary", "Send", "Receive"),
        ("largest variants", "Send+quota", "Receive+renewed"),
        (
            "renewal, fixed issuer tables",
            "Send+quota",
            "Receive+renewed/fixed-issuer",
        ),
        (
            "renewal, issuer key a circuit constant",
            "Send+quota",
            "Receive+renewed/issuer-constant",
        ),
        (
            "continuity, window of 4 in core",
            "Send+quota",
            "Receive+renewed/window-core",
        ),
        (
            "continuity, depth-16 tree, root in core",
            "Send+quota",
            "Receive+renewed/tree-core",
        ),
        (
            "continuity, depth-16 tree, root in rest",
            "Send+quota",
            "Receive+renewed/tree-rest",
        ),
    ] {
        let key = |l: &str| format!("{method}/{l}");
        let send = best[&key(send_label)].3.with_lookups();
        let recv = best[&key(recv_label)].3.with_lookups();
        // Binding step at the primary's chunk width (shares its range table).
        let chunk_w = best[&key(recv_label)].1 as u16;
        let bstep = operation::binding_step(chunk_w as usize);
        let bl = bstep.lookups();
        let step = census::StepCost {
            rows: bstep.rows() + bl,
            full: bstep
                .widths
                .iter()
                .filter(|(w, _)| **w > 16)
                .map(|(_, n)| n)
                .sum::<usize>()
                + bl,
            narrow: bstep
                .widths
                .iter()
                .filter(|(w, _)| **w <= 16)
                .map(|(_, n)| n)
                .sum::<usize>(),
        };
        println!(
            "\n[{method}] fixed points ({title}: {send_label} / {recv_label}), joint claims, Rlc binding \
             (binding step {} constraints){}:",
            step.rows,
            if recv.wallet_entries > 0 {
                format!(
                    ", per-wallet table rows bound ({} entries)",
                    recv.wallet_entries
                )
            } else {
                String::new()
            }
        );
        println!(
            "{:>7} {:>6} | {:>8} {:>8} {:>8} {:>8} | {:>16} {:>16} {:>16} {:>16} | {:>6} {:>6} | {:>6} {:>6}",
            "P cols",
            "S cols",
            "Send CP",
            "Send CS",
            "Recv CP",
            "Recv CS",
            "P-send",
            "S-send",
            "P-recv",
            "S-recv",
            "SendB",
            "RecvB",
            "Send s",
            "Recv s"
        );
        let mut top: Option<(f64, f64, f64, String)> = None;
        for p_cols in [13, 14, 15, 16] {
            for s_cols in [12, 13, 14] {
                let high = fixed_point_op(p_cols, s_cols, fp, fq, &send, &recv, step, false);
                let low = fixed_point_op(p_cols, s_cols, fp, fq, &send, &recv, step, true);
                let size = |f: &OpFixedPoint| {
                    (
                        f.p_recv.shape.sx + f.s_recv.shape.sx,
                        f.p_send.shape.sx + f.s_send.shape.sx,
                        f.recv_pair,
                    )
                };
                let lower = size(&low) < size(&high);
                let f = if lower { low } else { high };
                let shape = |p: &census::ProofShape| {
                    format!("2^{} {}x2^{}", p.shape.sx, p.shape.rows(), p.shape.col_bits)
                };
                let secondary = |n: usize, bits: usize| {
                    let mut w = std::collections::BTreeMap::new();
                    w.insert(255u16, n - bits.min(n));
                    w.insert(1u16, bits.min(n));
                    w
                };
                let merged =
                    |op: &operation::WithLookups, over: usize, bits: usize, narrow: usize| {
                        let mut w = op.witness.clone();
                        *w.entry(255).or_insert(0) += over - bits.min(over);
                        *w.entry(1).or_insert(0) += bits.min(over);
                        *w.entry(chunk_w).or_insert(0) += narrow;
                        w
                    };
                let ps = merged(&send, f.send_p, f.send_p_bits, f.send_p_narrow);
                let pr = merged(&recv, f.recv_p, f.recv_p_bits, f.recv_p_narrow);
                let ss = secondary(f.send_s, f.send_s_bits);
                let sr = secondary(f.recv_s, f.recv_s_bits);
                let t_send = pixel_seconds_widths(
                    &f.p_send.shape,
                    &ps,
                    send.constraints + f.send_p,
                    send.nnz + 4 * f.send_p,
                ) + pixel_seconds_widths(&f.s_send.shape, &ss, f.send_s, 4 * f.send_s);
                let t_recv = pixel_seconds_widths(
                    &f.p_recv.shape,
                    &pr,
                    recv.constraints + f.recv_p,
                    recv.nnz + 4 * f.recv_p,
                ) + pixel_seconds_widths(&f.s_recv.shape, &sr, f.recv_s, 4 * f.recv_s);
                let line = format!(
                    "{:>7} {:>6} | {:>8} {:>8} {:>8} {:>8} | {:>16} {:>16} {:>16} {:>16} | {:>6} {:>6} | {:>6.2} {:>6.2}{}",
                    1 << p_cols,
                    1 << s_cols,
                    f.send_p,
                    f.send_s,
                    f.recv_p,
                    f.recv_s,
                    shape(&f.p_send),
                    shape(&f.s_send),
                    shape(&f.p_recv),
                    shape(&f.s_recv),
                    f.send_pair,
                    f.recv_pair,
                    t_send,
                    t_recv,
                    if f.iterations == 16 {
                        " (not stable)"
                    } else if lower {
                        " (low start)"
                    } else {
                        ""
                    }
                );
                println!("{line}");
                let cap = |p: &census::ProofShape| p.shape.rows() * p.shape.active_cols();
                println!(
                    "{:>15}   P-recv constraints {} witness {} (capacity {}), P-send constraints {} witness {} (capacity {})",
                    "",
                    recv.constraints + f.recv_p,
                    f.recv_p
                        + f.recv_p_narrow
                        + recv.witness.values().sum::<usize>()
                        + (1usize << p_cols) * (1 + table_rows(recv.wallet_entries, p_cols)),
                    cap(&f.p_recv),
                    send.constraints + f.send_p,
                    f.send_p
                        + f.send_p_narrow
                        + send.witness.values().sum::<usize>()
                        + (1usize << p_cols) * (1 + table_rows(send.wallet_entries, p_cols)),
                    cap(&f.p_send)
                );
                if f.send_pair <= 8_458 && f.recv_pair <= 9_600 {
                    let score = t_send.max(t_recv);
                    if top.as_ref().is_none_or(|(b, _, _, _)| score < *b) {
                        let detail = format!(
                            "  P-send: {}\n  S-send: {}\n  P-recv: {}\n  S-recv: {}",
                            pixel_breakdown(
                                &f.p_send.shape,
                                &ps,
                                send.constraints + f.send_p,
                                send.nnz + 4 * f.send_p
                            ),
                            pixel_breakdown(&f.s_send.shape, &ss, f.send_s, 4 * f.send_s),
                            pixel_breakdown(
                                &f.p_recv.shape,
                                &pr,
                                recv.constraints + f.recv_p,
                                recv.nnz + 4 * f.recv_p
                            ),
                            pixel_breakdown(&f.s_recv.shape, &sr, f.recv_s, 4 * f.recv_s),
                        );
                        top = Some((score, t_send, t_recv, format!("{line}\n{detail}")));
                        let _ = (f.send_pair, f.recv_pair);
                    }
                }
            }
        }
        if let Some((_, ts, tr, line)) = top {
            println!("[{method}] fastest within both byte limits ({title}):\n{line}");
            out.push((title.to_string(), ts, tr, 0, 0));
        }
    }
    out
}

fn run_operation() {
    use operation::{comb_components, components, operations, summary};
    let fp = gadgets::measure::<Pallas>(1);
    let fq = gadgets::measure::<Vesta>(2);
    let poseidon_rows = fp.poseidon;
    census::MATRIX_MLE.store(8_000, std::sync::atomic::Ordering::Relaxed);
    let synth = std::env::var("ZKS_VERIFIER").as_deref() == Ok("synth");
    census::SYNTHESIZED.store(synth, std::sync::atomic::Ordering::Relaxed);
    println!(
        "verifier model: {}; commits: {}; IPA: {}",
        if synth {
            "gate 1d synthesized (verifier.rs)"
        } else {
            "gate 1b census formula"
        },
        std::env::var("ZKS_COMMIT").unwrap_or_else(|_| "Pixel lanes".into()),
        std::env::var("ZKS_IPA")
            .unwrap_or_else(|_| "Pixel measured, endoscalar challenges + GLV fix".into())
    );
    println!(
        "operation census: RP57 = {poseidon_rows} rows (measured); range-chunk width swept; matrix-MLE budget 8,000 per \
         verified proof; copy lookups {} per non-native check, {} per permutation",
        operation::COPY_PER_MUL_CHECK,
        operation::COPY_PER_PERMUTATION
    );
    println!(
        "methods: \"gate1c\" = every non-root signature with a variable key (R' recovery, half-size scalars); \
         \"comb\" = lever 1: own-key signatures over per-wallet comb tables (m bases), root over fixed comb tables"
    );
    // (method/label) -> (own-relation core, chunk, comb bases, cost)
    let mut best: std::collections::BTreeMap<String, (f64, usize, usize, operation::Cost)> =
        Default::default();
    let mut rank = |key: String, cost: operation::Cost, chunk: usize, m: usize| -> f64 {
        let wl = cost.with_lookups();
        // Rank by this relation's own prover-core share at 32,768 columns:
        // rows, z entries, commitments, density.
        let shape = census_shape("op", wl.constraints, wl.witness.values().sum(), 15);
        let t =
            pixel_seconds_widths(&shape, &wl.witness, wl.constraints, wl.nnz) - ipa_seconds(32_768);
        let e = best.entry(key).or_insert((f64::MAX, 0, 0, cost.clone()));
        if t < e.0 {
            *e = (t, chunk, m, cost);
        }
        t
    };
    // SHA-256 gadgets: bit-level (gate 1c) and spread lookups (lever 5, "+lsha").
    for chunk in [10, 11, 12, 13, 14, 15, 16] {
        for lsha in [false, true] {
            operation::set_sha_lookup(lsha);
            let suffix = if lsha { "+lsha" } else { "" };
            let clock = Instant::now();
            let comp = components(chunk);
            let cc = comb_components(&comp, 16);
            println!(
                "\nchunk {chunk} bits, SHA-256 {} (synthesized in {:.1} s):\n  SHA-256 of a 32-byte digest (canonical bits + \
                 compression): {}\n  ECDSA P-256, variable key: {}\n  comb m=16 ECDSA own key: {}\n  signed message, own key \
                 (comb): {}",
                if lsha { "spread lookups" } else { "bit-level" },
                clock.elapsed().as_secs_f64(),
                summary(&comp.sha_only),
                summary(&comp.ecdsa_only),
                summary(&cc.ecdsa_own_only),
                summary(&cc.signed_own),
            );
            for op in operations() {
                let cost = op.cost(&comp, None, poseidon_rows);
                let t = rank(
                    format!("gate1c{suffix}/{}", op.label),
                    cost.clone(),
                    chunk,
                    0,
                );
                println!(
                    "  {:16} gate1c{suffix} {} | own-relation core {:.2} s",
                    op.label,
                    summary(&cost),
                    t
                );
                let cost = op.cost(&comp, Some(&cc), poseidon_rows);
                let t = rank(
                    format!("comb16{suffix}/{}", op.label),
                    cost.clone(),
                    chunk,
                    16,
                );
                println!(
                    "  {:16} comb16{suffix} {} | own-relation core {:.2} s",
                    op.label,
                    summary(&cost),
                    t
                );
            }
        }
    }
    operation::set_sha_lookup(false);
    println!("\nbest chunk width per relation:");
    for (key, (t, chunk, m, cost)) in &best {
        let wl = cost.with_lookups();
        println!(
            "  {key:22} chunk {chunk} m {m}: constraints {} witness {} lookups {} table {} nnz {} checks {} | own-relation core {t:.2} s",
            wl.constraints,
            wl.witness.values().sum::<usize>(),
            wl.lookups,
            wl.table_entries,
            wl.nnz,
            cost.mul_checks
        );
        let widths: Vec<String> = wl
            .witness
            .iter()
            .map(|(w, n)| format!("{w}b:{n}"))
            .collect();
        println!("      width classes: {}", widths.join(" "));
    }
    let chunk = best["comb16/Receive"].1;
    let b = operation::bootstrap_cost(chunk, 16);
    println!(
        "Bootstrap (one time) for comb m=16 at chunk {chunk}: {} | checks {}",
        summary(&b),
        b.mul_checks
    );
    let methods = ["gate1c", "gate1c+lsha", "comb16", "comb16+lsha"];
    let results: Vec<_> = methods
        .iter()
        .map(|m| (*m, print_op_fixed_points(m, &best, &fp, &fq)))
        .collect();
    println!(
        "\ncomparison (fastest within both byte limits, modelled Pixel prover core, Send / Receive):"
    );
    for i in 0..results[0].1.len() {
        let mut line = format!("  {:16}:", results[0].1[i].0);
        for (m, r) in &results {
            line += &format!(" | {m} {:.2} / {:.2} s", r[i].1, r[i].2);
        }
        println!("{line}");
    }
}

/// Verifier synthesis (gate 1d): real proofs at the fixed-point shapes,
/// both sides synthesized and checked, against census::verify_cost.
fn run_verifier() {
    use census::{ProofShape, verify_cost};
    census::MATRIX_MLE.store(8_000, std::sync::atomic::Ordering::Relaxed);
    let fp = gadgets::measure::<Pallas>(1);
    let fq = gadgets::measure::<Vesta>(2);
    let shape = |label: &'static str, sx, row_bits, col_bits| Shape {
        label,
        sx,
        row_bits,
        col_bits,
        k: 3,
    };
    let gens_v14 = Gens::<Vesta>::new(1 << 14);
    let gens_v15 = Gens::<Vesta>::new(1 << 15);
    let gens_p14 = Gens::<Pallas>::new(1 << 14);
    let gens_p13 = Gens::<Pallas>::new(1 << 13);
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    println!(
        "verifier synthesis: real proofs (RP57 sponge transcript, 126-bit challenges, endoscalars for point \
         multipliers, inherited claims folded in clear), every constraint checked, every intermediate equal to the \
         native verifier; census formula with matrix-MLE budget 8,000"
    );
    let points_census = |v: &census::Split| v.transcript + v.group;
    for (method, proofs) in [
        (
            "comb16 fixed point (P 16,384 / S 16,384)",
            vec![
                ("P-send", shape("P-send", 19, 5, 14), true, 1usize),
                ("S-send", shape("S-send", 17, 3, 14), false, 1),
                ("P-recv", shape("P-recv", 19, 5, 14), true, 2),
                ("S-recv", shape("S-recv", 18, 4, 14), false, 2),
            ],
        ),
        (
            "lean fixed point, comb16 + lookup SHA (P 16,384 / S 8,192)",
            vec![
                ("P-send", shape("P-send", 19, 5, 14), true, 1usize),
                ("S-send", shape("S-send", 17, 4, 13), false, 1),
                ("P-recv", shape("P-recv", 19, 5, 14), true, 2),
                ("S-recv", shape("S-recv", 18, 5, 13), false, 2),
            ],
        ),
    ] {
        println!("\n{method}:");
        for (label, sh, vesta, inherited) in proofs {
            let io_len = census::STATEMENT_IO + inherited * (2 + sh.col_bits);
            let clock = Instant::now();
            let syn = if vesta {
                let gens = if sh.col_bits == 15 {
                    &gens_v15
                } else {
                    &gens_v14
                };
                verifier::synthesize::<Vesta>(sh, gens, inherited, io_len, label.as_bytes())
            } else {
                let gens = if sh.col_bits == 13 {
                    &gens_p13
                } else {
                    &gens_p14
                };
                verifier::synthesize::<Pallas>(sh, gens, inherited, io_len, label.as_bytes())
            };
            let ps = ProofShape {
                shape: sh,
                inherited,
                exported: inherited,
            };
            let cen = verify_cost(&ps, if vesta { &fq } else { &fp });
            let toy = syn
                .scalar
                .counts
                .get("matrix MLE (toy)")
                .copied()
                .unwrap_or(0);
            let scalar_budget = syn.scalar.total - toy
                + census::MATRIX_MLE.load(std::sync::atomic::Ordering::Relaxed);
            let fmt = |c: &std::collections::BTreeMap<&str, usize>| {
                c.iter()
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!(
                "  {label} 2^{} {}x2^{} ({inherited} inherited, io {io_len}; {:.1} s):",
                sh.sx,
                sh.rows(),
                sh.col_bits,
                clock.elapsed().as_secs_f64()
            );
            println!(
                "    points side ({}): synthesized {} [{}] | census {} (transcript {}, group {}) | {:+.1}%",
                if vesta { "C_S" } else { "C_P" },
                syn.points.total,
                fmt(&syn.points.counts),
                points_census(&cen),
                cen.transcript,
                cen.group,
                100.0 * (syn.points.total as f64 / points_census(&cen) as f64 - 1.0)
            );
            println!(
                "      permutations {} squeezes {} (lookup-checked {}) endoscales {} adds {} one-bit entries {} (census bits {})",
                syn.points.permutations,
                syn.points.squeezes,
                syn.points.lookup_squeezes,
                syn.points.endoscales,
                syn.points.adds,
                syn.points.bits,
                cen.bits_points
            );
            println!(
                "    scalar side ({}): synthesized {} [{}] -> with the 8,000 matrix budget instead of the toy's {toy}: {} | census {} | {:+.1}%",
                if vesta { "C_P" } else { "C_S" },
                syn.scalar.total,
                fmt(&syn.scalar.counts),
                scalar_budget,
                cen.scalar,
                100.0 * (scalar_budget as f64 / cen.scalar as f64 - 1.0)
            );
            println!(
                "    shared 128-bit values: synthesized {} ({} challenges, {} raw) | census {}",
                syn.shared, syn.challenges, syn.raw_challenges, cen.shared
            );
            let model = census::verify_cost_synth(&ps, vesta);
            println!(
                "    gate 1d formula: points {} (synthesized {}), scalar {} (synthesized {}), shared {} (synthesized {})",
                model.transcript + model.group,
                syn.points.total,
                model.scalar,
                scalar_budget,
                model.shared,
                syn.shared
            );
        }
    }
}

/// The cross-circuit binding (gate 1f) at the fixed point's shapes: a linked
/// pair verified in-circuit (link replay included, every cell mutation-tested),
/// then the binding's R chains over that verification's shared values with
/// rho from the next linked pair, both equal to the R both transcripts absorbed.
fn run_binding() {
    use ff::{Field, PrimeField};
    use spartan::Link;
    use verifier::{
        Fq, MUTATE, PairSpec, instance, prove_pair, rlc_chain_native, rlc_value, synthesize_proof,
        verify_pair,
    };
    census::MATRIX_MLE.store(8_000, std::sync::atomic::Ordering::Relaxed);
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let sp = Shape {
        label: "P-recv",
        sx: 19,
        row_bits: 5,
        col_bits: 14,
        k: 3,
    };
    let ss = Shape {
        label: "S-recv",
        sx: 18,
        row_bits: 5,
        col_bits: 13,
        k: 3,
    };
    let gv = Gens::<Vesta>::new(1 << sp.col_bits);
    let gp = Gens::<Pallas>::new(1 << ss.col_bits);
    let io_p = census::STATEMENT_IO + 2 * (2 + sp.col_bits);
    let io_s = census::STATEMENT_IO + 2 * (2 + ss.col_bits);
    let spec = |seed: &[u8]| {
        let (inst_p, wit_p, claims_p) =
            instance::<Vesta>(sp, &gv, 2, io_p, &[seed, b"/p"].concat());
        let (inst_s, wit_s, claims_s) =
            instance::<Pallas>(ss, &gp, 2, io_s, &[seed, b"/s"].concat());
        PairSpec {
            shape_p: sp,
            gens_p: &gv,
            inst_p,
            wit_p,
            claims_p,
            shape_s: ss,
            gens_s: &gp,
            inst_s,
            wit_s,
            claims_s,
        }
    };
    // Pair 0: linked; its in-circuit verification gives the shared values.
    let a = spec(b"binding-0");
    let noise: Vec<u128> = (0..8)
        .map(|i| 0x0123_4567_89ab_cdef_u128.wrapping_mul(i + 3))
        .collect();
    let pa = prove_pair(&a, b"0", &|rho| {
        rlc_value(rho, Fq::from(5), &noise).to_repr()
    });
    verify_pair(&a, &pa.p, &pa.s, pa.r).expect("pair 0 verifies");
    MUTATE.store(true, std::sync::atomic::Ordering::Relaxed);
    let clock = Instant::now();
    let r0 = pa.r;
    let fixed = move |_: &[u8; 32]| r0;
    let vp = synthesize_proof(
        &sp,
        &gv,
        &a.inst_p,
        &pa.p,
        &a.claims_p,
        &Link::First {
            rho: pa.rho,
            r: pa.r,
        },
    );
    let vs = synthesize_proof(
        &ss,
        &gp,
        &a.inst_s,
        &pa.s,
        &a.claims_s,
        &Link::Second {
            d: pa.d,
            rlc: &fixed,
        },
    );
    MUTATE.store(false, std::sync::atomic::Ordering::Relaxed);
    println!(
        "\npair 0 verified in-circuit with the link replay, every cell mutation-tested ({:.1} s):",
        clock.elapsed().as_secs_f64()
    );
    for (label, side) in [
        ("P points (C_S)", &vp.points),
        ("P scalar (C_P)", &vp.scalar),
        ("S points (C_P)", &vs.points),
        ("S scalar (C_S)", &vs.scalar),
    ] {
        println!(
            "  {label}: {} constraints, free cells {:?}",
            side.total,
            side.free_cells.as_ref().unwrap()
        );
    }
    // The same proofs' verification without the link (fresh unlinked proofs
    // of the same instances), for the link's own cost.
    let up = spartan::prove_with_claims(
        &sp,
        &gv,
        &a.inst_p,
        &a.wit_p,
        &mut fs::SpikeRng::new(b"u/p"),
        &a.claims_p,
    );
    let us = spartan::prove_with_claims(
        &ss,
        &gp,
        &a.inst_s,
        &a.wit_s,
        &mut fs::SpikeRng::new(b"u/s"),
        &a.claims_s,
    );
    let wp = synthesize_proof(&sp, &gv, &a.inst_p, &up, &a.claims_p, &Link::None);
    let ws = synthesize_proof(&ss, &gp, &a.inst_s, &us, &a.claims_s, &Link::None);
    println!(
        "  link replay: P points {} -> {} (+{}), S points {} -> {} (+{}); scalar sides +{} / +{}; shared {} + {} -> {} + {}",
        wp.points.total,
        vp.points.total,
        vp.points.total as i64 - wp.points.total as i64,
        ws.points.total,
        vs.points.total,
        vs.points.total as i64 - ws.points.total as i64,
        vp.scalar.total as i64 - wp.scalar.total as i64,
        vs.scalar.total as i64 - ws.scalar.total as i64,
        wp.shared,
        ws.shared,
        vp.shared,
        vs.shared
    );
    // Pair 1 binds pair 0's verification: R over those shared values.
    let values: Vec<u128> = vp.values.iter().chain(&vs.values).copied().collect();
    assert_eq!(
        values.len(),
        vp.shared + vs.shared,
        "shared list matches the count"
    );
    let b = spec(b"binding-1");
    let mu = Fq::from(0x6d61_736b_u64).square();
    let pb = prove_pair(&b, b"1", &|rho| rlc_value(rho, mu, &values).to_repr());
    verify_pair(&b, &pb.p, &pb.s, pb.r).expect("pair 1 verifies");
    let (native, rn) = rlc_chain_native(&pb.rho, mu, &values);
    assert_eq!(rn.to_repr(), pb.r, "C_S chain gives the absorbed R");
    let mu_big = num_bigint::BigUint::from_bytes_le(&mu.to_repr());
    let rho_big = num_bigint::BigUint::from_bytes_le(&pb.rho);
    for chunk in [11usize, 13] {
        let (cost, rp) = operation::rlc_chain(chunk, &rho_big, &mu_big, &values);
        assert_eq!(
            rp,
            num_bigint::BigUint::from_bytes_le(&pb.r),
            "C_P chain gives the absorbed R"
        );
        let rows = cost.rows() + cost.lookups();
        println!(
            "\nbinding R chain over {} shared values (pair 1's rho, mask), chunk {chunk}: C_P {} rows + {} lookups = {} \
             constraints ({:.1} per value; model: binding step per value), C_S {} rows",
            values.len(),
            cost.rows(),
            cost.lookups(),
            rows,
            rows as f64 / values.len() as f64,
            native.total
        );
        let bstep = operation::binding_step(chunk);
        println!(
            "  measured single binding step at chunk {chunk}: {} constraints",
            bstep.rows() + bstep.lookups()
        );
    }
    println!("  both chains equal the R that pair 1's linked transcripts absorbed");
}

/// The real Receive circuit (stage A): P-recv = the own relation recorded from
/// the census gadgets + the recursion (scalar halves of the two incoming
/// P-proofs, point halves of the two incoming S-proofs, replayed from real
/// linked pairs) + the matrix-MLE budget + the binding chain; S-recv = the
/// mirror halves + its native chain. Both assembled with LogUp, proved, verified.
///
/// `ZKS_ROWSTATS=1`: instead of proving, describe each first-phase row's
/// values (zeros, small and full-width entries, repeats) and time the
/// first-phase commit on the real rows against rows of random values at the
/// same positions and bounds, interleaved: repeated values send bucket
/// additions to the projective overflow path, random ones rarely do.
fn row_stats<C: iroha_pasta::PastaCurve>(
    g: &sparse::General<ipa::F<C>>,
    shape: &Shape,
    gens: &Gens<C>,
    wit: &r1cs::Witness<ipa::F<C>>,
    rng: &mut SpikeRng,
) {
    use ff::{Field, PrimeField};
    let (cols, active) = (shape.cols(), shape.active_cols());
    let bits_of = |x: &ipa::F<C>| -> usize {
        let r = x.to_repr();
        (0..256)
            .rev()
            .find(|i| (r[i / 8] >> (i % 8)) & 1 == 1)
            .map_or(0, |i| i + 1)
    };
    for r in 0..g.p1_rows {
        let row = &wit.w[r * cols..r * cols + active];
        let mut counts: std::collections::HashMap<[u8; 32], usize> = Default::default();
        let (mut nonzero, mut small, mut full) = (0usize, 0usize, 0usize);
        for x in row {
            if bool::from(x.is_zero()) {
                continue;
            }
            nonzero += 1;
            let b = bits_of(x);
            if b <= 16 {
                small += 1;
            }
            if b >= 250 {
                full += 1;
            }
            *counts.entry(x.to_repr()).or_insert(0) += 1;
        }
        let max_repeat = counts.values().copied().max().unwrap_or(0);
        let in_repeats: usize = counts.values().filter(|c| **c >= 8).sum();
        println!(
            "  row {r:>2} bound {:>3}: nonzero {nonzero:>5}, small (<=16 bits) {small:>5}, full (>=250 bits) {full:>5}, distinct {:>5}, most repeated value x{max_repeat:<5} entries repeated >=8 times {in_repeats:>5}",
            g.bounds[r],
            counts.len()
        );
    }
    let two128 = ipa::F::<C>::from_u128(u128::MAX) + ipa::F::<C>::ONE;
    let mut randomized = wit.w.clone();
    for r in 0..g.p1_rows {
        let bound = g.bounds[r];
        for x in &mut randomized[r * cols..r * cols + active] {
            if bool::from(x.is_zero()) {
                continue;
            }
            *x = if bound >= 255 {
                rng.scalar()
            } else if bound <= 128 {
                rng.below(bound as u32)
            } else {
                rng.below::<ipa::F<C>>(bound as u32 - 128) * two128 + rng.below::<ipa::F<C>>(128)
            };
            if bool::from(x.is_zero()) {
                *x = ipa::F::<C>::ONE;
            }
        }
    }
    let blinds: Vec<ipa::F<C>> = (0..shape.rows()).map(|_| rng.scalar()).collect();
    let (mut real, mut random) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        for (w, out) in [(&wit.w, &mut real), (&randomized, &mut random)] {
            let clock = Instant::now();
            let rows =
                spartan::commit_rows_lanes(gens, shape, w, 0..g.p1_rows, &g.bounds, &blinds, false);
            out.push(clock.elapsed().as_secs_f64());
            assert_eq!(rows.len(), g.p1_rows);
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    println!(
        "  first-phase commit ({} rows), medians of 5 interleaved: real values {:.4} s, random values at the same positions and bounds {:.4} s",
        g.p1_rows,
        median(&mut real),
        median(&mut random)
    );
}

/// Stage A2 step 2: P-recv's elliptic-curve ladders as uniform arrays: 123
/// endoscalings (126 bits), 2 variable-base and 6 fixed-base multiplications
/// (4 over U, 2 over H), proved and verified, against the same rows as a
/// sparse assembly.
fn run_uniform_ladders() {
    use ff::Field;
    use iroha_pasta::{Ep as Pallas, Fq};
    type Fb = iroha_pasta::Fp;
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let host = if cfg!(target_os = "android") {
        "Pixel"
    } else {
        "Mac"
    };
    let mut rng = SpikeRng::new(b"uniform-ladders");
    let mut point = |rng: &mut SpikeRng| <Pallas as group::Group>::generator() * rng.scalar::<Fq>();
    let mut bits = |rng: &mut SpikeRng, n: usize| -> Vec<bool> {
        (0..n).map(|_| rng.below::<Fb>(1) == Fb::ONE).collect()
    };
    let clock = Instant::now();
    let endo_in: Vec<(Pallas, Vec<bool>)> = (0..123)
        .map(|_| (point(&mut rng), bits(&mut rng, 126)))
        .collect();
    let var_in: Vec<(Pallas, Vec<bool>)> = (0..2)
        .map(|_| (point(&mut rng), bits(&mut rng, 255)))
        .collect();
    let (u, h) = (point(&mut rng), point(&mut rng));
    let windowed = std::env::var("ZKS_WINDOWED").is_ok();
    let fixed_u: Vec<Vec<bool>> = (0..4).map(|_| bits(&mut rng, 254)).collect();
    let fixed_h: Vec<Vec<bool>> = (0..2).map(|_| bits(&mut rng, 254)).collect();
    let mut arrays = Vec::new();
    arrays.extend(ladders::endoscale::<Pallas>(0, 0, 7, &endo_in).arrays);
    arrays.extend(ladders::var_mul::<Pallas>(3, 1, 1, &var_in).arrays);
    let (pi, counts) = if windowed {
        // Stage A's windowed tables, one family per base (ZKS_WINDOWED=1).
        arrays.extend(ladders::fixed_mul::<Pallas>(7, 2, 2, &u, &fixed_u).arrays);
        arrays.extend(ladders::fixed_mul::<Pallas>(11, 3, 1, &h, &fixed_h).arrays);
        (vec![7, 1, 2, 1, 1], vec![123, 2, 4, 2, 2])
    } else if std::env::var("ZKS_SEPARATE").is_ok() {
        // Fixed bases as constant columns, in their own family of six.
        let inputs: Vec<(Pallas, Vec<bool>)> = fixed_u
            .iter()
            .map(|b| (u, b.clone()))
            .chain(fixed_h.iter().map(|b| (h, b.clone())))
            .collect();
        arrays.extend(ladders::fixed_base_mul::<Pallas>(7, 2, 3, &inputs).arrays);
        (vec![7, 1, 3, 1], vec![123, 2, 6, 2])
    } else {
        // Default: variable and fixed bases share one family (selector column).
        arrays.truncate(3);
        let inputs: Vec<(Pallas, Vec<bool>, bool)> = var_in
            .iter()
            .map(|(p, b)| (*p, b.clone(), false))
            .chain(fixed_u.iter().map(|b| (u, b.clone(), true)))
            .chain(fixed_h.iter().map(|b| (h, b.clone(), true)))
            .map(|(p, mut b, f)| {
                // One bit count per family: a zero top bit leaves k unchanged.
                b.resize(255, false);
                (p, b, f)
            })
            .collect();
        arrays.extend(ladders::mixed_mul::<Pallas>(3, 1, 3, &inputs).arrays);
        (vec![7, 3, 1], vec![123, 8, 2])
    };
    let mask_family = pi.len() - 1;
    let ma: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
    let mb: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
    let mc: Vec<Fb> = ma.iter().zip(&mb).map(|(x, y)| *x * y).collect();
    arrays.push(uniform::Array {
        name: "mask",
        family: mask_family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 3],
        rows: vec![[uniform::own(0, 1), uniform::own(1, 1), uniform::own(2, 1)]],
        values: vec![ma, mb, mc],
    });
    let (c, inst, wit) = uniform::UniformCircuit::build(
        "uniform-ladders",
        arrays,
        pi,
        counts,
        Vec::new(),
        14,
        8,
        b"uniform-ladders",
    );
    let shape = c.shape;
    let (cells, rows) = c.sizes();
    let plan = plan_note(&c);
    let (terms, work) = c.template_size();
    println!(
        "P-recv ladders as uniform arrays (built in {:.2} s): {cells} cells, {rows} constraint rows, 2^{} constraints, witness {} rows of {} (active {}, {} constant rows in the key), proof {} B",
        clock.elapsed().as_secs_f64(),
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        r1cs::Circuit::fixed_rows(&c, &shape),
        shape.proof_bytes() - 32 * r1cs::Circuit::fixed_rows(&c, &shape)
    );
    for (a, arr) in c.arrays.iter().enumerate() {
        let (t, wk) = c.array_size(a);
        println!(
            "  array {a:>2} {:<12} {:>4} ladders x {:>3} steps, {:>2} cells, {:>2} template rows, {t:>3} terms, MLE work {wk}",
            arr.name,
            c.counts[arr.family],
            arr.r_count,
            arr.cells(),
            arr.rows.len()
        );
    }
    println!(
        "  template terms {terms}; verifier matrix-MLE work {work} products of two slots (closed forms where the step pattern allows), independent of the ladder counts"
    );
    println!("  {plan}; one evaluation {:.6} s", plan_call_time(&c, 50));
    let gens = Gens::<Vesta>::new(shape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &c);
        let bytes = proof.to_bytes();
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&shape, &gens, &inst, &bytes, &c);
        println!(
            "  {host} uniform prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; verify {:.3} s: {:?}; proof {} B; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            clock.elapsed().as_secs_f64(),
            ok.map(|_| "accepted"),
            bytes.len(),
            mem_note("after it")
        );
    }
    let mut bad = wit.w.clone();
    let target = bad
        .iter()
        .enumerate()
        .filter(|(_, v)| !bool::from(v.is_zero()) && **v != Fb::ONE)
        .map(|(i, _)| i)
        .nth(cells / 3)
        .unwrap();
    bad[target] += Fb::ONE;
    let (proof, _) = spartan::prove_circuit(
        &shape,
        &gens,
        &inst,
        &r1cs::Witness { w: bad },
        &mut rng,
        &c,
    );
    println!(
        "  changed cell at {target}: {:?}",
        spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).map(|_| "accepted")
    );
    let clock = Instant::now();
    let asm = c.to_assembly();
    let (g, gshape, ginst, gwit) =
        sparse::General::build(asm, "sparse-ladders", 14, 8, b"sparse-ladders");
    println!(
        "sparse assembly of the same rows ({:.2} s): {}",
        clock.elapsed().as_secs_f64(),
        sparse::describe(&g, &gshape)
    );
    let ggens = Gens::<Vesta>::new(gshape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&gshape, &ggens, &ginst, &gwit, &mut rng, &g);
        let bytes = proof.to_bytes();
        *sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&gshape, &ggens, &ginst, &bytes, &g);
        let v = clock.elapsed().as_secs_f64();
        let vm = sparse::LAST_BUILD.lock().unwrap().1;
        println!(
            "  {host} sparse prove {}: core {:.3} s = commit {:.3} + outer {:.3} + inner {:.3} + open {:.3}; matrices {:.3} s; verify {:.3} s + matrices {vm:.3} s: {:?}; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.outer,
            tm.inner,
            tm.open,
            tm.matrices,
            v - vm,
            ok.map(|_| "accepted"),
            mem_note("after it")
        );
    }
    println!(
        "  reference (block census v4, P-recv): the gadgets use 7 rows per endoscale step, 6 per var_mul step and 4 per fixed_mul window, with the bits' booleanity and recomposition in separate blocks"
    );
}

/// Stage A2 step 3: the own-key comb of `ZKS_COMB_SIGS` (default 2)
/// signatures (m = 16, 16 steps each) as uniform arrays: canonical
/// non-native checks, value and selection arrays, against the same rows as a
/// sparse assembly.
fn run_uniform_comb() {
    use ff::Field;
    type Fp = iroha_pasta::Fp;
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let sigs: usize = std::env::var("ZKS_COMB_SIGS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .max(1);
    let host = if cfg!(target_os = "android") {
        "Pixel"
    } else {
        "Mac"
    };
    let pi = sigs.next_power_of_two().trailing_zeros() as usize;
    let mut rng = SpikeRng::new(b"uniform-comb");
    let clock = Instant::now();
    let inputs = nonnative::sample_inputs(sigs);
    let comb = nonnative::comb(0, 0, pi, 1, &inputs);
    let (n1, n4, nx) = comb.check_counts;
    let (s1, s4) = (comb.shapes.0.clone(), comb.shapes.1.clone());
    let nonnative::Comb { arrays, tables, .. } = comb;
    let (c, inst, wit) = uniform::UniformCircuit::build(
        "uniform-comb",
        arrays,
        vec![pi, 0],
        vec![sigs, 1],
        tables,
        14,
        8,
        b"uniform-comb",
    );
    let shape = c.shape;
    let (cells, rows) = c.sizes();
    let plan = plan_note(&c);
    let (terms, work) = c.template_size();
    println!(
        "Own-key comb, {sigs} signatures x {} steps, as uniform arrays (built in {:.2} s): {cells} cells, {rows} constraint rows, 2^{} constraints, witness {} rows of {} (active {}, {} constant rows in the key), proof {} B",
        nonnative::STEPS,
        clock.elapsed().as_secs_f64(),
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        r1cs::Circuit::fixed_rows(&c, &shape),
        shape.proof_bytes() - 32 * r1cs::Circuit::fixed_rows(&c, &shape)
    );
    println!(
        "  canonical checks: one-product {} cells, {} range lookups; four-product {} cells, {} range lookups; {n1} + {n4} checks per step, {nx} one-product checks per signature",
        s1.cells,
        s1.lookups(),
        s4.cells,
        s4.lookups()
    );
    let steps = sigs * nonnative::STEPS;
    let (mut step_rows, mut step_cells, mut step_lookups) = (0usize, 0usize, 0usize);
    for (a, arr) in c.arrays.iter().enumerate() {
        let (t, wk) = c.array_size(a);
        let inst_n = c.instances(a);
        println!(
            "  array {a:>2} {:<22} {:>5} instances, {:>3} cells, {:>3} template rows ({:>3} lookups), {t:>4} terms, MLE work {wk}",
            arr.name,
            inst_n,
            c.placed_cells(a),
            arr.rows.len(),
            arr.lookups.len()
        );
        if arr.family == 0 {
            step_rows += arr.rows.len() * inst_n;
            step_cells += c.placed_cells(a) * inst_n;
            step_lookups += arr.lookups.len() * inst_n;
        }
    }
    println!(
        "  per comb step: {:.0} rows (with one row per lookup), {:.0} cells (with one helper per lookup), {:.0} lookups; stage A's comb step: 222 rows + 1,344 range lookups (+ 4 table lookups)",
        step_rows as f64 / steps as f64,
        step_cells as f64 / steps as f64,
        step_lookups as f64 / steps as f64
    );
    println!(
        "  template terms {terms}; verifier matrix-MLE work {work} products of two slots, independent of the signature count"
    );
    println!("  {plan}");
    {
        // Native cost of the relation kernels on this circuit.
        let ch: Vec<Fp> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = r1cs::Circuit::instantiate(&c, &shape, &ch, Some(&mut w));
        let z = r1cs::z_vector(&shape, &inst, &r1cs::Witness { w });
        let _ = rel.abc(&shape, &z);
        let (abc_min, abc_med) = warm_times(5, || {
            let _ = rel.abc(&shape, &z);
        });
        let rx: Vec<Fp> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fp> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let ex = crate::grid::eq_table(&rx);
        let clock = Instant::now();
        let _ = rel.m_table(&shape, &ex, Fp::ONE, Fp::ONE, Fp::ONE);
        let t_m_first = clock.elapsed().as_secs_f64();
        let (m_min, m_med) = warm_times(5, || {
            let _ = rel.m_table(&shape, &ex, Fp::ONE, Fp::ONE, Fp::ONE);
        });
        println!("  m_table first call (builds the circuit's readers and plan) {t_m_first:.4} s");
        let clock = Instant::now();
        let _ = rel.matrix_evals(&shape, &rx, &ry);
        let t_first = clock.elapsed().as_secs_f64();
        let t_e = plan_call_time(&c, 50);
        println!(
            "  native kernels (warm, min / median of 5): abc {abc_min:.4} / {abc_med:.4} s, m_table {m_min:.4} / {m_med:.4} s, matrix evaluation {t_e:.6} s (first call in a proof {t_first:.6} s)"
        );
    }
    let gens = Gens::<Vesta>::new(shape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &c);
        let bytes = proof.to_bytes();
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&shape, &gens, &inst, &bytes, &c);
        println!(
            "  {host} uniform prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; verify {:.3} s: {:?}; proof {} B; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            clock.elapsed().as_secs_f64(),
            ok.map(|_| "accepted"),
            bytes.len(),
            mem_note("after it")
        );
    }
    let mut bad = wit.w.clone();
    let target = c.cell_index(0, 1, 5);
    bad[target] += Fp::ONE;
    let (proof, _) = spartan::prove_circuit(
        &shape,
        &gens,
        &inst,
        &r1cs::Witness { w: bad },
        &mut rng,
        &c,
    );
    println!(
        "  changed value limb at {target}: {:?}",
        spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).map(|_| "accepted")
    );
    if std::env::var("ZKS_NO_SPARSE").is_ok() {
        return;
    }
    let clock = Instant::now();
    let asm = c.to_assembly();
    let (g, gshape, ginst, gwit) =
        sparse::General::build(asm, "sparse-comb", 14, 8, b"sparse-comb");
    println!(
        "sparse assembly of the same rows ({:.2} s): {}",
        clock.elapsed().as_secs_f64(),
        sparse::describe(&g, &gshape)
    );
    let ggens = Gens::<Vesta>::new(gshape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&gshape, &ggens, &ginst, &gwit, &mut rng, &g);
        let bytes = proof.to_bytes();
        *sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&gshape, &ggens, &ginst, &bytes, &g);
        let v = clock.elapsed().as_secs_f64();
        let vm = sparse::LAST_BUILD.lock().unwrap().1;
        println!(
            "  {host} sparse prove {}: core {:.3} s = commit {:.3} + outer {:.3} + inner {:.3} + open {:.3}; matrices {:.3} s; verify {:.3} s + matrices {vm:.3} s: {:?}; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.outer,
            tm.inner,
            tm.open,
            tm.matrices,
            v - vm,
            ok.map(|_| "accepted"),
            mem_note("after it")
        );
    }
}

/// Proves a uniform circuit `repeats` times, verifies each proof, refuses one
/// changed cell, then proves the same rows as a sparse assembly.
/// The minimum and the median of `n` timed calls of `f`.
fn warm_times(n: usize, mut f: impl FnMut()) -> (f64, f64) {
    let mut times: Vec<f64> = (0..n)
        .map(|_| {
            let clock = Instant::now();
            f();
            clock.elapsed().as_secs_f64()
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).expect("finite times"));
    (times[0], times[n / 2])
}

/// A short tag of a proof's bytes (std's SipHash with fixed keys), to compare
/// two builds' proofs of the same seeds byte for byte.
fn proof_tag(bytes: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn prove_uniform_and_sparse(
    c: &uniform::UniformCircuit<iroha_pasta::Fp>,
    inst: &Instance<iroha_pasta::Fp>,
    wit: &Witness<iroha_pasta::Fp>,
    bad_cell: usize,
    label: &'static str,
    repeats: usize,
) {
    let host = if cfg!(target_os = "android") {
        "Pixel"
    } else {
        "Mac"
    };
    let shape = c.shape;
    let mut rng = SpikeRng::new(label.as_bytes());
    {
        let ch: Vec<iroha_pasta::Fp> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = r1cs::Circuit::instantiate(c, &shape, &ch, Some(&mut w));
        let z = r1cs::z_vector(&shape, inst, &r1cs::Witness { w });
        // One warm-up call each (it builds the gather plan, the readers and the
        // M(r_x, .) plan), then five timed calls: on a loaded machine one call
        // can take twice another's time.
        let _ = rel.abc(&shape, &z);
        let (abc_min, abc_med) = warm_times(5, || {
            let _ = rel.abc(&shape, &z);
        });
        let ex = crate::grid::eq_table(
            &(0..shape.sx)
                .map(|_| rng.scalar())
                .collect::<Vec<iroha_pasta::Fp>>(),
        );
        let one = iroha_pasta::Fp::ONE;
        let _ = rel.m_table(&shape, &ex, one, one, one);
        let (m_min, m_med) = warm_times(5, || {
            let _ = rel.m_table(&shape, &ex, one, one, one);
        });
        println!(
            "  native kernels (warm, min / median of 5): abc {abc_min:.4} / {abc_med:.4} s, m_table {m_min:.4} / {m_med:.4} s, matrix evaluation {:.6} s",
            plan_call_time(c, 50)
        );
    }
    let gens = Gens::<Vesta>::new(shape.cols());
    // Commitment tables are per-shape setup, built before the timed proofs.
    let table_build = spartan::warm_table(&gens, &shape);
    println!(
        "  commit tables built in {table_build:.3} s; {}",
        mem_note("after the tables")
    );
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&shape, &gens, inst, wit, &mut rng, c);
        let bytes = proof.to_bytes();
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&shape, &gens, inst, &bytes, c);
        println!(
            "  {host} uniform prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; verify {:.3} s: {:?}; proof {} B ({}); {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            clock.elapsed().as_secs_f64(),
            ok.map(|_| "accepted"),
            bytes.len(),
            proof_tag(&bytes),
            mem_note("after it")
        );
    }
    let mut bad = wit.w.clone();
    bad[bad_cell] += iroha_pasta::Fp::ONE;
    let (proof, _) =
        spartan::prove_circuit(&shape, &gens, inst, &r1cs::Witness { w: bad }, &mut rng, c);
    println!(
        "  changed cell at {bad_cell}: {:?}",
        spartan::verify_circuit(&shape, &gens, inst, &proof.to_bytes(), c).map(|_| "accepted")
    );
    if std::env::var("ZKS_NO_SPARSE").is_ok() {
        return;
    }
    let clock = Instant::now();
    let asm = c.to_assembly();
    let (g, gshape, ginst, gwit) = sparse::General::build(asm, label, 14, 8, label.as_bytes());
    println!(
        "sparse assembly of the same rows ({:.2} s): {}",
        clock.elapsed().as_secs_f64(),
        sparse::describe(&g, &gshape)
    );
    let ggens = Gens::<Vesta>::new(gshape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&gshape, &ggens, &ginst, &gwit, &mut rng, &g);
        let bytes = proof.to_bytes();
        *sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&gshape, &ggens, &ginst, &bytes, &g);
        let v = clock.elapsed().as_secs_f64();
        let vm = sparse::LAST_BUILD.lock().unwrap().1;
        println!(
            "  {host} sparse prove {}: core {:.3} s = commit {:.3} + outer {:.3} + inner {:.3} + open {:.3}; matrices {:.3} s; verify {:.3} s + matrices {vm:.3} s: {:?}; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.outer,
            tm.inner,
            tm.open,
            tm.matrices,
            v - vm,
            ok.map(|_| "accepted"),
            mem_note("after it")
        );
    }
}

/// Header line, per-array sizes and the compiled evaluation of a uniform
/// circuit; returns (template rows, lookups, cells without helpers) summed
/// over the arrays of family `fam`.
fn describe_uniform(
    c: &uniform::UniformCircuit<iroha_pasta::Fp>,
    title: &str,
    built: f64,
    fam: usize,
) -> (usize, usize, usize) {
    let shape = c.shape;
    let (cells, rows) = c.sizes();
    let plan = plan_note(c);
    let (terms, work) = c.template_size();
    println!(
        "{title} as uniform arrays (built in {built:.2} s): {cells} cells, {rows} constraint rows, 2^{} constraints, witness {} rows of {} (active {}, {} constant rows in the key), proof {} B",
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        r1cs::Circuit::fixed_rows(c, &shape),
        shape.proof_bytes() - 32 * r1cs::Circuit::fixed_rows(c, &shape)
    );
    let (mut trows, mut tlookups, mut tcells) = (0, 0, 0);
    for (a, arr) in c.arrays.iter().enumerate() {
        let (t, wk) = c.array_size(a);
        let inst_n = c.instances(a);
        println!(
            "  array {a:>2} {:<22} {:>5} instances, {:>3} cells, {:>3} template rows ({:>3} lookups), {t:>4} terms, MLE work {wk}",
            arr.name,
            inst_n,
            c.placed_cells(a),
            arr.rows.len(),
            arr.lookups.len()
        );
        if arr.family == fam {
            trows += (arr.rows.len() - arr.lookups.len()) * inst_n;
            tlookups += arr.lookups.len() * inst_n;
            tcells += (c.placed_cells(a) - arr.lookups.len()) * inst_n;
        }
    }
    println!("  template terms {terms}; verifier matrix-MLE work {work} products of two slots");
    println!("  {plan}");
    (trows, tlookups, tcells)
}

/// Stage A2 step 4: `ZKS_SHA_BLOCKS` (default 2: P-recv's two own-key
/// signatures) SHA-256 blocks as uniform arrays.
fn run_uniform_sha() {
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let blocks: usize = std::env::var("ZKS_SHA_BLOCKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .max(1);
    let pi = blocks.next_power_of_two().trailing_zeros() as usize;
    let clock = Instant::now();
    let messages = sha::sample_messages(blocks);
    let sha::Sha { arrays, tables, .. } = sha::sha(0, 0, pi, 1, &messages);
    let (c, inst, wit) = uniform::UniformCircuit::build(
        "uniform-sha",
        arrays,
        vec![pi, 0],
        vec![blocks, 1],
        tables,
        14,
        8,
        b"uniform-sha",
    );
    let (rows, lookups, cells) = describe_uniform(
        &c,
        &format!("{blocks} SHA-256 blocks"),
        clock.elapsed().as_secs_f64(),
        0,
    );
    println!(
        "  per block: {} rows other than lookups, {} lookups, {} cells without helpers; stage A's lookup SHA-256 block: 2,709 rows + 3,392 lookups (canonical bits apart)",
        rows / blocks,
        lookups / blocks,
        cells / blocks
    );
    let bad = c.cell_index(sha::A_ROUND, 1, 30);
    prove_uniform_and_sparse(&c, &inst, &wit, bad, "uniform-sha", repeats);
}

/// Stage A2 step 5: one P-recv proof's scalar-side verification (k = 3,
/// base-4 slots) as uniform arrays, from a real proof with two inherited
/// claims: rows hold and every output equals the native verifier's (checked
/// while building), then proved and verified, beside the non-uniform
/// synthesis of the same proof.
fn run_uniform_scalar() {
    use spartan::Link;
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let shape = Shape {
        label: "P-recv",
        sx: 19,
        row_bits: 5,
        col_bits: 14,
        k: 3,
    };
    let inherited = 2;
    let io_len = census::STATEMENT_IO + inherited * (2 + shape.col_bits);
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let gens = Gens::<Vesta>::new(shape.cols());
    let clock = Instant::now();
    let (inst, wit, claims) =
        verifier::instance::<Vesta>(shape, &gens, inherited, io_len, b"uniform-scalar");
    let proof = spartan::prove_with_claims(
        &shape,
        &gens,
        &inst,
        &wit,
        &mut SpikeRng::new(b"uniform-scalar/prove"),
        &claims,
    );
    let tr = verifier::trace(&shape, &gens, &inst, &proof, &claims, &Link::None);
    println!(
        "P-recv-shaped proof (2^{} x 2^{}, {inherited} inherited claims, io {io_len}) proved in {:.2} s",
        shape.sx,
        shape.col_bits,
        clock.elapsed().as_secs_f64()
    );
    let syn = verifier::synthesize_proof(&shape, &gens, &inst, &proof, &claims, &Link::None);
    let sec = |name: &str| syn.scalar.counts.get(name).copied().unwrap_or(0);
    let covered = [
        "challenge powers",
        "relation",
        "closed form",
        "io eval",
        "schnorr scalars",
    ];
    let covered_rows: usize = covered.iter().map(|n| sec(n)).sum();
    println!(
        "non-uniform scalar side: {} rows [{}]; the arrays cover {} of them ({}); challenge values (endoscalars from bits), the io digest (permutation family) and the matrix MLE stay outside",
        syn.scalar.total,
        syn.scalar
            .counts
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", "),
        covered_rows,
        covered.join(", ")
    );
    let clock = Instant::now();
    let side = scalar::scalar_side::<Vesta>(
        0,
        0,
        &shape,
        fs::lean_schedule(),
        &inst,
        &proof,
        &claims,
        &tr,
    );
    let (openings, q_pad, side_slots) = (side.openings, side.q_pad, side.slot_instances);
    let checked: Vec<&str> = side.expected.iter().map(|(n, _, _, _, _)| *n).collect();
    let (c, uinst, uwit) = uniform::UniformCircuit::build(
        "uniform-scalar",
        side.arrays,
        vec![0],
        vec![1],
        Vec::new(),
        12,
        4,
        b"uniform-scalar",
    );
    let (rows, _, cells) = describe_uniform(
        &c,
        "One P-recv proof's scalar side",
        clock.elapsed().as_secs_f64(),
        0,
    );
    println!(
        "  {rows} template-row instances, {cells} cells over {openings} openings padded to {q_pad}; equal to the native verifier: {}",
        checked.join(", ")
    );
    let bad = c.cell_index(scalar::S_TERMS, scalar::FGG, (side_slots - 3) * q_pad + 1);
    prove_uniform_and_sparse(&c, &uinst, &uwit, bad, "uniform-scalar", repeats);
}

/// Stage A2 step 7: P-recv in the uniform layout (own relation, both incoming
/// pairs' verifiers, the binding chain, the matrix-MLE filler), proved and
/// verified, against the same rows as a sparse assembly.
fn run_uniform_full() {
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let clock = Instant::now();
    let (gens, pairs) = full::receive_pairs();
    println!(
        "incoming pairs proved and traced in {:.1} s",
        clock.elapsed().as_secs_f64()
    );
    let clock = Instant::now();
    let (w, idx) = full::p_recv(&gens, &pairs);
    let (sites_e, sites_i) = w.copy_sites();
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (c, inst, wit) = uniform::UniformCircuit::build_with_copies(
        "P-recv-uniform",
        arrays,
        pi,
        counts,
        tables,
        copies,
        14,
        64,
        b"p-recv-uniform",
    );
    for sc in &idx.scalars {
        for (name, a, k, i, v) in &sc.expected {
            assert!(
                wit.w[c.cell_index(*a, *k, *i)] == *v,
                "{name} equals the native verifier's"
            );
        }
    }
    describe_uniform(&c, "P-recv", clock.elapsed().as_secs_f64(), usize::MAX);
    let (entries, reads) = c.copy_counts();
    println!(
        "  copies: {sites_e} export sites with {entries} entries, {sites_i} read sites with {reads} reads; {} permutations in the pool; {} shared values; stage A P-recv: 355,637 constraints in 2^19, witness 23 + 9 rows",
        idx.own.pool.perms, idx.values,
    );
    let late_p = {
        let g = Gens::<Vesta>::new(c.shape.cols());
        let (proof, _) = spartan::prove_circuit(
            &c.shape,
            &g,
            &inst,
            &wit,
            &mut SpikeRng::new(b"p-recv late"),
            &c,
        );
        assert!(
            spartan::verify_circuit(&c.shape, &g, &inst, &proof.to_bytes(), &c).is_ok(),
            "P-recv proof with late io"
        );
        proof.late
    };
    deferral_note(&c, &inst, &wit, &idx.defer, &gens.defer_p, &late_p);
    let (r1, r2, r3) = c.regions();
    let mut by_region = [0usize; 3];
    for (a, arr) in c.arrays.iter().enumerate() {
        let n = c.instances(a);
        for k in 0..arr.cells() {
            if c.cell_placed(a, k) {
                by_region[c.cell_region(a, k)] += n;
            }
        }
    }
    println!(
        "  witness regions: first phase {r1} rows ({} cells), second phase {r2} rows ({} cells), constant {r3} rows ({} cells)",
        by_region[0], by_region[1], by_region[2]
    );
    {
        let rel = r1cs::Circuit::instantiate(
            &c,
            &c.shape,
            &[iroha_pasta::Fp::from(5u64), iroha_pasta::Fp::from(7u64)],
            None,
        );
        let nnz: usize = rel.nnz().iter().sum();
        let widths = c.width_classes(&wit.w);
        println!(
            "  Pixel model, this layout: {}",
            pixel_breakdown(&c.shape, &widths, c.sizes().1, nnz)
        );
        let (u1, u2, u3) = c.regions();
        println!(
            "  Pixel model, kernels of 11-10: {}",
            pixel_kern(&c.shape, &widths, c.sizes().1, u1 + u2 + u3)
        );
    }
    let bad = c.cell_index(idx.points.adds, 2, 5);
    prove_uniform_and_sparse(&c, &inst, &wit, bad, "P-recv-uniform", repeats);
    drop((c, inst, wit));
    // S-recv.
    let clock = Instant::now();
    let (w, sidx) = full::s_recv(&gens, &pairs);
    let (sites_e, sites_i) = w.copy_sites();
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (c, inst, wit) = uniform::UniformCircuit::build_with_copies(
        "S-recv-uniform",
        arrays,
        pi,
        counts,
        tables,
        copies,
        13,
        62,
        b"s-recv-uniform",
    );
    for sc in &sidx.scalars {
        for (name, a, k, i, v) in &sc.expected {
            assert!(
                wit.w[c.cell_index(*a, *k, *i)] == *v,
                "{name} equals the native verifier's"
            );
        }
    }
    describe_uniform_q(&c, "S-recv", clock.elapsed().as_secs_f64());
    let (entries, reads) = c.copy_counts();
    let (r1, r2, r3) = c.regions();
    println!(
        "  copies: {sites_e} export sites with {entries} entries, {sites_i} read sites with {reads} reads; {} permutations in the pool; {} shared values; witness regions {r1} + {r2} + {r3} rows; stage A S-recv: 185,751 constraints in 2^18, witness 26 + 6 rows of 8192",
        sidx.perms, sidx.values
    );
    let late_s = {
        let g = Gens::<Pallas>::new(c.shape.cols());
        let (proof, _) = spartan::prove_circuit(
            &c.shape,
            &g,
            &inst,
            &wit,
            &mut SpikeRng::new(b"s-recv late"),
            &c,
        );
        assert!(
            spartan::verify_circuit(&c.shape, &g, &inst, &proof.to_bytes(), &c).is_ok(),
            "S-recv proof with late io"
        );
        proof.late
    };
    deferral_note(&c, &inst, &wit, &sidx.defer, &gens.defer_s, &late_s);
    {
        let rel = r1cs::Circuit::instantiate(
            &c,
            &c.shape,
            &[full::Fq::from(5u64), full::Fq::from(7u64)],
            None,
        );
        let nnz: usize = rel.nnz().iter().sum();
        let widths = c.width_classes(&wit.w);
        println!(
            "  Pixel model, this layout: {}",
            pixel_breakdown(&c.shape, &widths, c.sizes().1, nnz)
        );
        println!(
            "  Pixel model, kernels of 11-10: {}",
            pixel_kern(&c.shape, &widths, c.sizes().1, r1 + r2 + r3)
        );
    }
    prove_uniform_q(&c, &inst, &wit, repeats, &gens.p);
}

/// The deferred matrix claims: the fold's size, and its folded claim checked
/// natively in a witness filled at challenges (5, 7), against the native fold.
fn deferral_note<S: fs::Scalar>(
    c: &uniform::UniformCircuit<S>,
    inst: &Instance<S>,
    wit: &Witness<S>,
    d: &full::DeferIdx<S>,
    poly: &deferral::MatPoly<S>,
    proof_late: &[S],
) {
    let ch = [S::from(5u64), S::from(7u64)];
    let mut w = wit.w.clone();
    let _ = r1cs::Circuit::instantiate(c, &c.shape, &ch, Some(&mut w));
    let out = deferral::read_output(c, &d.fold, &w);
    assert!(
        out == deferral::fold_output(&d.claims, &d.q, ch[1]),
        "the circuit's fold is the native fold"
    );
    let holds = poly.eval(&out.point) == out.value;
    let cells: usize = [d.fold.coords, d.fold.q, d.fold.values]
        .iter()
        .map(|a| c.instances(*a) * c.placed_cells(*a))
        .sum();
    // The ties to the sources: a fresh claim's value (glue M1 + M2 + M3), one
    // of its r_x digits, an accumulator coordinate (an io leaf); each off by
    // one breaks a row.
    let broken = |a: usize, k: usize, i: usize| -> bool {
        let mut b = w.clone();
        b[c.cell_index(a, k, i)] += S::ONE;
        let rel = r1cs::Circuit::instantiate(c, &c.shape, &ch, None);
        let io = spartan::io_with_late(
            &inst.io,
            &r1cs::Circuit::late_slots(c),
            &r1cs::Circuit::late_values(c, &w),
        );
        let z = r1cs::z_vector(&c.shape, &Instance { io }, &Witness { w: b });
        let (az, bz, cz) = rel.abc(&c.shape, &z);
        (0..1usize << c.shape.sx).any(|x| az[x] * bz[x] != cz[x])
    };
    let refused = [
        broken(d.fold.values, deferral::V_V[1], 0),
        broken(d.fold.coords, deferral::C_P[1], 2),
        broken(d.fold.coords, deferral::C_P[0], 5),
    ];
    println!(
        "  deferred matrix claims: 4 claims of {} coordinates folded by q of degree {} ({cells} cells); folded claim at (5, 7) {}; changed fresh value / r_x digit / accumulator coordinate refused: {refused:?}",
        d.fold.n,
        d.fold.degree,
        if holds {
            "holds natively"
        } else {
            "FAILS natively"
        }
    );
    assert!(
        holds && refused.iter().all(|r| *r),
        "the folded claim holds and its ties hold"
    );
    // A real proof's late io: its public accumulator, a true claim.
    let pub_acc = deferral::claim_of(proof_late);
    let pub_holds = poly.eval(&pub_acc.point) == pub_acc.value;
    println!(
        "  the proof's late io ({} values) is its accumulator; {}",
        proof_late.len(),
        if pub_holds {
            "it holds natively"
        } else {
            "it FAILS natively"
        }
    );
    assert!(pub_holds, "the proof's public accumulator holds");
    // The prover's native part: q from one batched matrix evaluation per kind
    // at kind_degree + 1 curve points. At the fixed point the kinds are
    // uniform circuits like this one; its compiled evaluation stands in for
    // both.
    let per_eval = plan_call_time(c, 20);
    let m = 3 * (c.shape.sx + c.shape.sz() + c.challenge_degrees().0 + c.challenge_degrees().1) + 1;
    let mut rng = SpikeRng::new(b"deferral batched");
    let pts: Vec<(Vec<S>, Vec<S>, Vec<S>)> = (0..m)
        .map(|_| {
            (
                (0..2).map(|_| rng.scalar()).collect(),
                (0..c.shape.sx).map(|_| rng.scalar()).collect(),
                (0..c.shape.sz()).map(|_| rng.scalar()).collect(),
            )
        })
        .collect();
    let refs: Vec<(&[S], &[S], &[S])> = pts
        .iter()
        .map(|(ch, rx, ry)| (&ch[..], &rx[..], &ry[..]))
        .collect();
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("a one-thread pool");
    let timed = |f: &dyn Fn()| {
        let clock = Instant::now();
        f();
        clock.elapsed().as_secs_f64()
    };
    let t1 = (0..3)
        .map(|_| timed(&|| drop(one.install(|| c.evals_many(&refs)))))
        .fold(f64::MAX, f64::min);
    let tn = (0..3)
        .map(|_| timed(&|| drop(c.evals_many(&refs))))
        .fold(f64::MAX, f64::min);
    println!(
        "  deferral prover work at the fixed point, 2 kinds like this circuit: {} points each (kind degree + 1, of q's {}); one at a time {:.1} ms, batched {:.1} ms on one thread, {:.1} ms over {} threads (here)",
        m,
        d.fold.degree + 1,
        2.0 * m as f64 * per_eval * 1e3,
        2.0 * t1 * 1e3,
        2.0 * tn * 1e3,
        rayon::current_num_threads()
    );
}

/// Native q on one hop's polynomial and claims, in interleaved rounds (the
/// shared Mac's load moves): one point at a time (the form before batching)
/// against batched lanes of 4 and 8 points on one thread and over every
/// core; medians, and in how many rounds each beat the pointwise run. Every
/// run's q equals the pointwise q.
fn q_bench<S: fs::Scalar>(
    name: &str,
    poly: &deferral::MatPoly<S>,
    claims: &[deferral::Claim<S>; 4],
) {
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("a one-thread pool");
    let q0 = deferral::restrict_pointwise(poly, claims);
    let variants: [(&str, usize, bool); 4] = [
        ("4 lanes, 1 thread", 4, false),
        ("8 lanes, 1 thread", 8, false),
        ("4 lanes, all threads", 4, true),
        ("8 lanes, all threads", 8, true),
    ];
    let rounds = 7;
    let (mut base, mut times) = (Vec::new(), vec![Vec::new(); variants.len()]);
    for _ in 0..rounds {
        let clock = Instant::now();
        let q = deferral::restrict_pointwise(poly, claims);
        base.push(clock.elapsed().as_secs_f64());
        assert!(q == q0, "the pointwise q repeats");
        for (v, &(_, lanes, all)) in variants.iter().enumerate() {
            uniform::set_q_lanes(lanes);
            let clock = Instant::now();
            let q = if all {
                deferral::restrict(poly, claims)
            } else {
                one.install(|| deferral::restrict(poly, claims))
            };
            times[v].push(clock.elapsed().as_secs_f64());
            assert!(q == q0, "the batched q is the pointwise q");
        }
    }
    uniform::set_q_lanes(0);
    let median = |v: &[f64]| {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s[s.len() / 2]
    };
    let mb = median(&base);
    let parts: Vec<String> = variants
        .iter()
        .zip(&times)
        .map(|((label, ..), t)| {
            let wins = t.iter().zip(&base).filter(|(a, b)| a < b).count();
            format!(
                "{label} {:.1} ms ({:.1}x, faster in {wins}/{rounds})",
                median(t) * 1e3,
                mb / median(t)
            )
        })
        .collect();
    println!(
        "  native q, {name}: {} points x {} kinds, {rounds} interleaved rounds, medians ({} threads): one point at a time {:.1} ms; {}",
        poly.degree() + 1,
        poly.kinds.len(),
        rayon::current_num_threads(),
        mb * 1e3,
        parts.join("; ")
    );
}

/// A uniform circuit from a wiring at another circuit's layout (`Layout`).
macro_rules! uniform_at {
    ($label:expr, $w:expr, $col_bits:expr, $io:expr, $seed:expr, $layout:expr) => {{
        let (arrays, pi, counts, tables, copies) = $w.finish();
        uniform::UniformCircuit::build_with_copies_at(
            $label,
            arrays,
            pi,
            counts,
            tables,
            copies,
            $col_bits,
            $io,
            $seed,
            Some($layout),
        )
    }};
}

/// A uniform circuit from a wiring, on rows of `2^col_bits` columns.
macro_rules! uniform_of {
    ($label:expr, $w:expr, $col_bits:expr, $io:expr, $seed:expr) => {{
        let (arrays, pi, counts, tables, copies) = $w.finish();
        uniform::UniformCircuit::build_with_copies(
            $label, arrays, pi, counts, tables, copies, $col_bits, $io, $seed,
        )
    }};
}

/// One hop circuit's line: size, layout, fold, and the Pixel model.
fn describe_hop_circuit<S: fs::Scalar>(
    name: &str,
    c: &uniform::UniformCircuit<S>,
    wit: &Witness<S>,
    d: &full::DeferIdx<S>,
) {
    let shape = c.shape;
    let ((cells, rows), (terms, work)) = (c.sizes(), c.template_size());
    let regions = c.regions();
    let pl = c.link_phase_rows();
    println!(
        "  {name}: 2^{} constraints ({rows} rows), witness {} rows of {} ({} + {pl} + {} + {} used: first phase, link phase, second phase, fixed), {cells} cells; template terms {terms}, matrix-MLE products {work}; fold: {} coordinates, q of degree {} (computed in {:.0} ms)",
        shape.sx,
        shape.rows(),
        shape.cols(),
        regions.0,
        regions.1,
        regions.2,
        d.fold.n,
        d.fold.degree,
        d.q_secs * 1e3
    );
    println!(
        "  Pixel model, kernels of 11-10: {name} {}",
        pixel_kern(
            &shape,
            &c.width_classes(&wit.w),
            rows,
            regions.0 + pl + regions.1 + regions.2
        )
    );
}

/// Each split family once: its instances as the dense 2^k block plus the
/// remainder family, and its arrays; with ZKS_SPLIT=off, the families the
/// split would take (2^k < n < 1.5 2^k, 2^k >= 8).
fn split_families_note<G>(c: &uniform::UniformCircuit<G>) -> String {
    let mut pairs: Vec<(usize, Option<usize>, Vec<&str>)> = Vec::new();
    for rest in c.arrays.iter().filter(|a| a.name.ends_with("(rest)")) {
        let name = rest.name.trim_end_matches(" (rest)");
        let orig = c
            .arrays
            .iter()
            .find(|a| a.name == name)
            .expect("a split array's original");
        match pairs
            .iter_mut()
            .find(|(f, g, _)| (*f, *g) == (orig.family, Some(rest.family)))
        {
            Some((_, _, names)) => names.push(name),
            None => pairs.push((orig.family, Some(rest.family), vec![name])),
        }
    }
    for (f, &n) in c.counts.iter().enumerate() {
        let taken = pairs.iter().any(|(a, b, _)| *a == f || *b == Some(f));
        if taken || n < 9 || n.is_power_of_two() {
            continue;
        }
        let k = n.ilog2();
        if 2 * (n - (1usize << k)) < (1usize << k) {
            pairs.push((
                f,
                None,
                c.arrays
                    .iter()
                    .filter(|a| a.family == f)
                    .map(|a| a.name)
                    .collect(),
            ));
        }
    }
    let v: Vec<String> = pairs
        .iter()
        .map(|(f, g, names)| match g {
            Some(g) => format!(
                "{} = {} + {} ({})",
                c.counts[*f] + c.counts[*g],
                c.counts[*f],
                c.counts[*g],
                names.join(", ")
            ),
            None => format!("{} unsplit ({})", c.counts[*f], names.join(", ")),
        })
        .collect();
    v.join("; ")
}

/// Where two builds' templates differ, by array (at most `max`): sizes,
/// rows or lookups; empty when every array's template is the same (then any
/// difference lies in the copy argument or the placement).
fn template_differences<S: fs::Scalar>(
    a: &uniform::UniformCircuit<S>,
    b: &uniform::UniformCircuit<S>,
    max: usize,
) -> Vec<String> {
    let same_lin = |l: &[uniform::Term<S>], m: &[uniform::Term<S>]| {
        l.len() == m.len()
            && l.iter()
                .zip(m)
                .all(|(s, t)| s.src == t.src && s.coef == t.coef)
    };
    let mut out = Vec::new();
    if a.arrays.len() != b.arrays.len() {
        out.push(format!(
            "array count {} vs {}",
            a.arrays.len(),
            b.arrays.len()
        ));
    }
    for (i, (x, y)) in a.arrays.iter().zip(&b.arrays).enumerate() {
        let meta = x.name == y.name
            && x.family == y.family
            && x.rho == y.rho
            && x.r_count == y.r_count
            && x.fixed == y.fixed
            && x.widths == y.widths;
        let rows = x.rows.len() == y.rows.len()
            && x.rows
                .iter()
                .zip(&y.rows)
                .all(|(r, s)| (0..3).all(|k| same_lin(&r[k], &s[k])));
        let lookups = x.lookups.len() == y.lookups.len()
            && x.lookups.iter().zip(&y.lookups).all(|((t, l), (u, m))| {
                t == u && l.len() == m.len() && l.iter().zip(m).all(|(p, q)| same_lin(p, q))
            });
        if !(meta && rows && lookups) {
            out.push(format!(
                "array {i} '{}'{}{}{}",
                x.name,
                if meta { "" } else { " sizes" },
                if rows { "" } else { " rows" },
                if lookups { "" } else { " lookups" }
            ));
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

/// A circuit against the same circuit of the previous hop: shape, sizes,
/// and the matrices at a random point.
fn compare_hop_circuit<S: fs::Scalar>(
    name: &str,
    before: &uniform::UniformCircuit<S>,
    now: &uniform::UniformCircuit<S>,
    rng: &mut SpikeRng,
) -> String {
    let same_shape = before.shape.sx == now.shape.sx
        && before.shape.row_bits == now.shape.row_bits
        && before.shape.col_bits == now.shape.col_bits;
    let same_sizes = before.sizes() == now.sizes() && before.template_size() == now.template_size();
    let equal = same_shape && {
        let rx: Vec<S> = (0..now.shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<S> = (0..now.shape.sz()).map(|_| rng.scalar()).collect();
        let ch: Vec<S> = (0..2).map(|_| rng.scalar()).collect();
        before.evals(&rx, &ry, &ch) == now.evals(&rx, &ry, &ch)
    };
    format!(
        "{name} shape {}, sizes {}, matrices {}",
        if same_shape { "same" } else { "differs" },
        if same_sizes { "same" } else { "differ" },
        if equal { "EQUAL" } else { "differ" }
    )
}

/// Whether changing each cell `(array, cell, instance)` by one breaks a row
/// of `c` (its rows at random second-phase challenges, the honest late io).
/// Panics if the honest witness itself breaks one.
fn row_breaks<S: fs::Scalar>(
    c: &uniform::UniformCircuit<S>,
    inst: &Instance<S>,
    wit: &Witness<S>,
    cells: &[(usize, usize, usize)],
) -> Vec<bool> {
    let mut rng = SpikeRng::new(b"row breaks");
    let ch: Vec<S> = (0..r1cs::Circuit::phase2_challenges(c))
        .map(|_| rng.scalar())
        .collect();
    let mut w = wit.w.clone();
    // The link phase at a rho below 2^128, ending at the circuit's own R.
    let mut rho = [0u8; 32];
    rho[..16].copy_from_slice(&0x1234_5678_9abc_def1_0fed_cba9_8765_4321_u128.to_le_bytes());
    r1cs::Circuit::fill_link(c, &rho, &mut w);
    let r = r1cs::Circuit::binding_r(c, &w).unwrap_or([0; 32]);
    let link_io = r1cs::Circuit::link_io(c, &rho, &r);
    let rel = r1cs::Circuit::instantiate(c, &c.shape, &ch, Some(&mut w));
    let io = spartan::io_with_late(
        &r1cs::io_with_link(&inst.io, &link_io),
        &r1cs::Circuit::late_slots(c),
        &r1cs::Circuit::late_values(c, &w),
    );
    let fails = |w: Vec<S>| {
        let z = r1cs::z_vector(&c.shape, &Instance { io: io.clone() }, &Witness { w });
        let (az, bz, cz) = rel.abc(&c.shape, &z);
        (0..1usize << c.shape.sx).any(|x| az[x] * bz[x] != cz[x])
    };
    assert!(!fails(w.clone()), "{}: the honest witness holds", c.label);
    cells
        .iter()
        .map(|&(a, k, i)| {
            let mut b = w.clone();
            b[c.cell_index(a, k, i)] += S::ONE;
            fails(b)
        })
        .collect()
}

/// A proved and verified linked pair.
struct LinkedPair {
    pp: spartan::Proof<Vesta>,
    ps: spartan::Proof<Pallas>,
    d: [u8; 32],
    rho: [u8; 32],
    r: [u8; 32],
    ok: bool,
    /// Each side's verdict: S, then P.
    sides: (bool, bool),
    secs: f64,
}

/// Proves a uniform pair, linked (P's phase-1 digest d, S with d giving rho
/// and R, P again with rho and R), and verifies both proofs. `seed` names
/// the pair's randomness ("fixed point" keeps the Receive pair's original
/// seeds, so its proofs are byte-identical to earlier runs).
#[allow(clippy::too_many_arguments)]
fn prove_linked_pair(
    seed: &str,
    hop: usize,
    cp: &uniform::UniformCircuit<iroha_pasta::Fp>,
    ip: &Instance<iroha_pasta::Fp>,
    wp: &Witness<iroha_pasta::Fp>,
    cs: &uniform::UniformCircuit<verifier::Fq>,
    is: &Instance<verifier::Fq>,
    ws: &Witness<verifier::Fq>,
) -> LinkedPair {
    use ff::PrimeField;
    use spartan::Link;
    use verifier::Fq;
    let clock = Instant::now();
    let gp = Gens::<Vesta>::new(cp.shape.cols());
    let gs = Gens::<Pallas>::new(cs.shape.cols());
    let rlc = |rho: &[u8; 32]| verifier::rlc_value(rho, Fq::from(3), &[1, 2, 3]).to_repr();
    let rng_p = SpikeRng::new(&[format!("{seed} P/").as_bytes(), &[hop as u8]].concat());
    let mut rng_s = SpikeRng::new(&[format!("{seed} S/").as_bytes(), &[hop as u8]].concat());
    let (_, d, _) = spartan::prove_linked_circuit(
        &cp.shape,
        &gp,
        ip,
        wp,
        &mut rng_p.clone(),
        &Link::First {
            rho: [0; 32],
            r: [0; 32],
        },
        cp,
    );
    let (ps, rho, r) = spartan::prove_linked_circuit(
        &cs.shape,
        &gs,
        is,
        ws,
        &mut rng_s,
        &Link::Second { d, rlc: &rlc },
        cs,
    );
    let (pp, d2, _) = spartan::prove_linked_circuit(
        &cp.shape,
        &gp,
        ip,
        wp,
        &mut rng_p.clone(),
        &Link::First { rho, r },
        cp,
    );
    assert_eq!(d, d2, "same rows, same digest");
    let secs = clock.elapsed().as_secs_f64();
    let fixed_r = move |_: &[u8; 32]| r;
    let vs = spartan::verify_linked_circuit(
        &cs.shape,
        &gs,
        is,
        &ps.to_bytes(),
        &Link::Second { d, rlc: &fixed_r },
        cs,
    );
    let vp = spartan::verify_linked_circuit(
        &cp.shape,
        &gp,
        ip,
        &pp.to_bytes(),
        &Link::First { rho, r },
        cp,
    );
    let sides = (
        vs.as_ref().is_ok_and(|x| *x == rho),
        vp.as_ref().is_ok_and(|x| *x == d),
    );
    LinkedPair {
        pp,
        ps,
        d,
        rho,
        r,
        ok: sides.0 && sides.1,
        sides,
        secs,
    }
}

/// One Bootstrap circuit's line: size, layout and the Pixel model (it has no
/// fold).
fn describe_boot_circuit<S: fs::Scalar>(
    name: &str,
    c: &uniform::UniformCircuit<S>,
    wit: &Witness<S>,
) {
    let shape = c.shape;
    let ((cells, rows), (terms, work)) = (c.sizes(), c.template_size());
    let regions = c.regions();
    let pl = c.link_phase_rows();
    println!(
        "  {name}: 2^{} constraints ({rows} rows), witness {} rows of {} ({} + {pl} + {} + {} used: first phase, link phase, second phase, fixed), {cells} cells; template terms {terms}, matrix-MLE products {work}; no fold (the empty claim as its accumulator)",
        shape.sx,
        shape.rows(),
        shape.cols(),
        regions.0,
        regions.1,
        regions.2,
    );
    println!(
        "  Pixel model, kernels of 11-10: {name} {}",
        pixel_kern(
            &shape,
            &c.width_classes(&wit.w),
            rows,
            regions.0 + pl + regions.1 + regions.2
        )
    );
}

/// A proved linked pair on its circuits as a uniform pair of kind `kind`.
fn linked_uniform(
    cp: &std::sync::Arc<uniform::UniformCircuit<iroha_pasta::Fp>>,
    ip: Instance<iroha_pasta::Fp>,
    cs: &std::sync::Arc<uniform::UniformCircuit<verifier::Fq>>,
    is: Instance<verifier::Fq>,
    l: LinkedPair,
    kind: usize,
) -> full::UniformPair {
    full::UniformPair {
        cp: cp.clone(),
        ip,
        pp: l.pp,
        cs: cs.clone(),
        is,
        ps: l.ps,
        d: l.d,
        rho: l.rho,
        r: l.r,
        kind,
    }
}

/// A linked pair's line: proving time, verification, sizes, and whether its
/// accumulators hold natively on `gens`' polynomials (returned).
fn report_pair(label: &str, l: &LinkedPair, gens: &full::PairGens) -> (bool, bool) {
    let (ap, as_) = (
        deferral::claim_of(&l.pp.late),
        deferral::claim_of(&l.ps.late),
    );
    let (hp, hs) = (
        gens.defer_p.eval(&ap.point) == ap.value,
        gens.defer_s.eval(&as_.point) == as_.value,
    );
    println!(
        "  {label} pair proved in {:.1} s (P twice, S once); linked verification {}; proofs {} + {} B; accumulators hold natively: P {hp}, S {hs}",
        l.secs,
        if l.ok { "accepted" } else { "FAILED" },
        l.pp.to_bytes().len(),
        l.ps.to_bytes().len()
    );
    assert!(l.ok, "the {label} pair verifies");
    (hp, hs)
}

/// Bootstrap with the wallet's key tables, measured once: P-boot without and
/// with them, each at its natural layout of 2^14-column rows (rows by phase,
/// cells, constraint rows, the matrix program, the Pixel model), against what
/// Receive's layout `layout` leaves Bootstrap.
fn key_tables_fit(gens: &full::PairGens, layout: uniform::Layout) {
    let clock = Instant::now();
    let measure = |tables: bool| {
        let (w, idx) = full::p_boot(gens, tables);
        let (c, _, wit) = uniform_of!("P-uniform", w, 14, 64, b"p-boot-uniform");
        let (cells, rows) = c.sizes();
        let (r, pl) = (c.regions(), c.link_phase_rows());
        let model = pixel_kern(
            &c.shape,
            &c.width_classes(&wit.w),
            rows,
            r.0 + pl + r.1 + r.2,
        );
        let work = c.template_size().1;
        (
            c.shape.sx,
            c.shape.rows(),
            r,
            pl,
            cells,
            rows,
            work,
            model,
            idx.key_tables,
        )
    };
    let (sx0, rows0, r0, pl0, cells0, crows0, work0, model0, _) = measure(false);
    let (sx1, rows1, r1, pl1, cells1, crows1, work1, model1, kt) = measure(true);
    let [dbl, add, c1, c2, c4] = kt.expect("the tables were derived").1;
    let free2 = (1usize << layout.row_bits) - layout.p1 - layout.pl - layout.fixed;
    let fits = sx1 <= layout.sx
        && r1.0 <= layout.p1
        && pl1 <= layout.pl
        && r1.1 <= free2
        && r1.2 <= layout.fixed;
    println!(
        "  Bootstrap's key tables, measured once ({:.1} s): {dbl} doublings and {add} additions; {c1} one-product, {c2} two-product and {c4} four-product checks",
        clock.elapsed().as_secs_f64()
    );
    println!(
        "    P-boot without them: 2^{sx0} constraints ({crows0} rows), {} + {pl0} + {} + {} of {rows0} witness rows, {cells0} cells, matrix-MLE products {work0}; Pixel model {model0}",
        r0.0, r0.1, r0.2
    );
    println!(
        "    P-boot with them: 2^{sx1} constraints ({crows1} rows), {} + {pl1} + {} + {} of {rows1} witness rows, {cells1} cells (+{}), matrix-MLE products {work1}; Pixel model {model1}",
        r1.0,
        r1.1,
        r1.2,
        cells1 - cells0
    );
    println!(
        "    Receive's layout leaves Bootstrap 2^{} constraints and at most {} + {} + {free2} + {} rows (first, link, second phase, fixed): with the tables it {}",
        layout.sx,
        layout.p1,
        layout.pl,
        layout.fixed,
        if fits { "FITS" } else { "does NOT fit" }
    );
}

/// P-boot at P-recv's layout `layout`, without the wallet's key tables:
/// they do not fit it (`key_tables_fit`). `ZKS_BOOT_TABLES=on` tries them
/// first and falls back, saying by how much they miss (the build at a target
/// layout panics when a circuit does not fit; this catches it and builds the
/// natural layout to measure).
#[allow(clippy::type_complexity)]
fn p_boot_at(
    gens: &full::PairGens,
    layout: uniform::Layout,
) -> (
    uniform::UniformCircuit<iroha_pasta::Fp>,
    Instance<iroha_pasta::Fp>,
    Witness<iroha_pasta::Fp>,
    full::BootIdx,
) {
    let want = std::env::var("ZKS_BOOT_TABLES").is_ok_and(|v| v == "on");
    if want {
        let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (w, idx) = full::p_boot(gens, true);
            let (c, i, wb) = uniform_at!("P-uniform", w, 14, 64, b"p-boot-uniform", layout);
            (c, i, wb, idx)
        }));
        match built {
            Ok(b) => return b,
            Err(_) => {
                let (w, _) = full::p_boot(gens, true);
                let (c, _, _) = uniform_of!("P-uniform", w, 14, 64, b"p-boot-uniform");
                let (r, pl) = (c.regions(), c.link_phase_rows());
                println!(
                    "  P-boot with the key tables does NOT fit P-recv's layout (2^{} constraints, {} rows: {} + {} + second phase + {} fixed): on its own it needs 2^{} constraints and {} + {pl} + {} + {} of {} rows; built without the tables",
                    layout.sx,
                    1usize << layout.row_bits,
                    layout.p1,
                    layout.pl,
                    layout.fixed,
                    c.shape.sx,
                    r.0,
                    r.1,
                    r.2,
                    c.shape.rows()
                );
            }
        }
    }
    let (w, idx) = full::p_boot(gens, false);
    let (c, i, wb) = uniform_at!("P-uniform", w, 14, 64, b"p-boot-uniform", layout);
    (c, i, wb, idx)
}

/// The recursion's fixed point over the three kinds (`ZKS_HOPS`, default 4).
/// Each hop builds the uniform Receive pair (P-recv, S-recv) over the
/// previous hop's Receive pair (the predecessor) and Send pair (the payer's),
/// the uniform Send pair (P-send, S-send) over the previous hop's Receive
/// pair (the predecessor), and the uniform Bootstrap pair (P-boot, S-boot),
/// which verifies no proof; Send and Bootstrap are built at the Receive
/// pair's layout. It proves and verifies the three linked pairs, checks their
/// accumulators natively, and compares each circuit's shape, sizes and
/// matrices with the previous hop's. The accumulators' polynomials have three
/// kinds, the previous hop's Receive, Send and Bootstrap circuits. Hop 1
/// verifies toy pairs.
///
/// From hop 2 on, the predecessor slot takes any kind (its kind bits, read
/// from the proof's io, select the key and the matrices). The hop chain
/// starts from toy pairs, so its accumulators are claims on earlier hops'
/// polynomials and do not hold, and no flag restarts it. At the last hop the
/// runner starts two wallets' chains where production does, at Bootstrap
/// pairs, and proves every order on the hop's circuits (`wallet_chains`).
/// ZKS_DETERMINISM=1 builds each Send and Bootstrap circuit twice and
/// compares the builds.
fn run_fixed_point() {
    use std::sync::Arc;
    use verifier::Fq;
    let hops: usize = std::env::var("ZKS_HOPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4)
        .max(1);
    let determinism = std::env::var("ZKS_DETERMINISM").is_ok();
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let clock = Instant::now();
    let (mut gens, mut pairs) = full::receive_pairs();
    println!(
        "toy incoming pairs proved and traced in {:.1} s",
        clock.elapsed().as_secs_f64()
    );
    // The previous hop's circuits: Receive, Send, Bootstrap (P, S).
    #[allow(clippy::type_complexity)]
    let mut prev: Option<
        [(
            Arc<uniform::UniformCircuit<iroha_pasta::Fp>>,
            Arc<uniform::UniformCircuit<Fq>>,
        ); 3],
    > = None;
    for hop in 1..=hops {
        let clock = Instant::now();
        let (w, idx) = full::p_recv(&gens, &pairs);
        let (cp, ip, wp) = uniform_of!("P-uniform", w, 14, 64, b"p-recv-uniform");
        let (w, sidx) = full::s_recv(&gens, &pairs);
        let (cs, is, ws) = uniform_of!("S-uniform", w, 13, 62, b"s-recv-uniform");
        let (pl, sl) = (cp.layout(), cs.layout());
        let send_p = |gens: &full::PairGens, pred: &full::TracedPair| {
            let (w, idx) = full::p_send(gens, pred);
            let (c, i, wt) = uniform_at!("P-uniform", w, 14, 64, b"p-send-uniform", pl);
            (c, i, wt, idx)
        };
        let send_s = |gens: &full::PairGens, pred: &full::TracedPair| {
            let (w, idx) = full::s_send(gens, pred);
            let (c, i, wu) = uniform_at!("S-uniform", w, 13, 62, b"s-send-uniform", sl);
            (c, i, wu, idx)
        };
        let boot_p = |gens: &full::PairGens| p_boot_at(gens, pl);
        let boot_s = |gens: &full::PairGens| {
            let (w, idx) = full::s_boot(gens);
            let (c, i, wv) = uniform_at!("S-uniform", w, 13, 62, b"s-boot-uniform", sl);
            (c, i, wv, idx)
        };
        let (ct, it, wt, idx_t) = send_p(&gens, &pairs[0]);
        let (cu, iu, wu, sidx_t) = send_s(&gens, &pairs[0]);
        let (cb, ib, wb, bidx) = boot_p(&gens);
        let (cv, iv, wv, sbidx) = boot_s(&gens);
        if determinism {
            let mut rng = SpikeRng::new(b"determinism");
            let (ct2, ..) = send_p(&gens, &pairs[0]);
            let (cu2, ..) = send_s(&gens, &pairs[0]);
            let (cb2, ..) = boot_p(&gens);
            let (cv2, ..) = boot_s(&gens);
            println!(
                "  determinism: {}; {}; {}; {}",
                compare_hop_circuit("P-send rebuilt", &ct, &ct2, &mut rng),
                compare_hop_circuit("S-send rebuilt", &cu, &cu2, &mut rng),
                compare_hop_circuit("P-boot rebuilt", &cb, &cb2, &mut rng),
                compare_hop_circuit("S-boot rebuilt", &cv, &cv2, &mut rng)
            );
        }
        let incoming = if hop == 1 {
            "toy pairs"
        } else {
            "the previous hop's uniform Receive and Send pairs"
        };
        let any = if gens.keys_p.is_some() {
            "; the predecessor slot takes any kind (Receive, Send, Bootstrap)"
        } else {
            ""
        };
        println!(
            "hop {hop}: Receive verifies {incoming} (predecessor, payer), Send the predecessor, Bootstrap nothing{any}; circuits built in {:.1} s",
            clock.elapsed().as_secs_f64()
        );
        describe_hop_circuit("P-recv", &cp, &wp, &idx.defer);
        describe_hop_circuit("S-recv", &cs, &ws, &sidx.defer);
        describe_hop_circuit("P-send", &ct, &wt, &idx_t.defer);
        describe_hop_circuit("S-send", &cu, &wu, &sidx_t.defer);
        describe_boot_circuit("P-boot", &cb, &wb);
        describe_boot_circuit("S-boot", &cv, &wv);
        if hop == 1
            && let Some(own) = &bidx.own
        {
            println!(
                "  P-boot's own relation (Bootstrap at the census's counts, a stand-in): {} permutations in its pool, {} signatures, {} insertions",
                own.pool.perms,
                own.digest_perms.len(),
                own.insertion_values.map_or(0, |a| cb.arrays[a].r_count)
            );
        }
        match bidx.key_tables {
            Some((_, [dbl, add, c1, c2, c4])) => println!(
                "  P-boot derives the wallet's comb tables (m = 16, 2 x 256 entries) from its key: {dbl} doublings, {add} additions; checks: {c1} one-product, {c2} two-product, {c4} four-product"
            ),
            None => println!("  P-boot without the key tables"),
        }
        println!(
            "  split families: P-recv [{}]; S-recv [{}]; P-send [{}]; S-send [{}]; P-boot [{}]; S-boot [{}]",
            split_families_note(&cp),
            split_families_note(&cs),
            split_families_note(&ct),
            split_families_note(&cu),
            split_families_note(&cb),
            split_families_note(&cv)
        );
        if let Some([(pr, sr), (pt, st), (pb, sb)]) = &prev {
            let mut rng = SpikeRng::new(b"fixed point compare");
            println!(
                "  against hop {}: {}; {}; {}; {}; {}; {}",
                hop - 1,
                compare_hop_circuit("P-recv", pr, &cp, &mut rng),
                compare_hop_circuit("S-recv", sr, &cs, &mut rng),
                compare_hop_circuit("P-send", pt, &ct, &mut rng),
                compare_hop_circuit("S-send", st, &cu, &mut rng),
                compare_hop_circuit("P-boot", pb, &cb, &mut rng),
                compare_hop_circuit("S-boot", sb, &cv, &mut rng)
            );
        }
        let recv = prove_linked_pair("fixed point", hop, &cp, &ip, &wp, &cs, &is, &ws);
        let send = prove_linked_pair("fixed point send", hop, &ct, &it, &wt, &cu, &iu, &wu);
        let boot = prove_linked_pair("fixed point boot", hop, &cb, &ib, &wb, &cv, &iv, &wv);
        report_pair("Receive", &recv, &gens);
        report_pair("Send", &send, &gens);
        let boot_holds = report_pair("Bootstrap", &boot, &gens);
        assert_eq!(
            boot_holds,
            (true, true),
            "a Bootstrap pair's accumulators (the empty claim) hold on any hop's polynomials"
        );
        if hop == hops {
            // The pair binding refuses swapped partners: P-recv linked with
            // S-send proves S-send's chain ending at its R, which P-recv's
            // chain over the Receive pair's shared values does not reach.
            let swapped = prove_linked_pair("swapped", hop, &cp, &ip, &wp, &cu, &iu, &wu);
            println!(
                "  partners swapped (P-recv linked with S-send): S verifies {}, P verifies {}",
                swapped.sides.0, swapped.sides.1
            );
            assert!(
                swapped.sides == (true, false),
                "P refuses a partner whose chain ends at another R"
            );
            // Bootstrap's pins: its kind bits and its accumulator (the empty
            // claim's c0, c1 and value) each break a row when changed.
            let n_p = gens.defer_p.n();
            let n_s = gens.defer_s.n();
            let p_breaks = row_breaks(
                &cb,
                &ib,
                &wb,
                &[
                    (bidx.kind_pin, 0, 0),
                    (bidx.kind_pin, 0, 1),
                    (bidx.empty, 0, 0),
                    (bidx.empty, 0, n_p - 1),
                    (bidx.empty, 0, n_p),
                ],
            );
            let s_breaks = row_breaks(
                &cv,
                &iv,
                &wv,
                &[
                    (sbidx.kind_pin, 0, 0),
                    (sbidx.kind_pin, 0, 1),
                    (sbidx.empty, 0, 0),
                    (sbidx.empty, 0, n_s - 1),
                    (sbidx.empty, 0, n_s),
                ],
            );
            println!(
                "  Bootstrap's kind bits (c0, c1) and empty claim (c0, c1, value), changed, break a row: P-boot {p_breaks:?}, S-boot {s_breaks:?}"
            );
            assert!(
                p_breaks.iter().chain(&s_breaks).all(|b| *b),
                "Bootstrap's kind and accumulator are pinned"
            );
            key_tables_fit(&gens, cp.layout());
            {
                fn tape<S: fs::Scalar>(c: &uniform::UniformCircuit<S>) -> String {
                    let ((muls, scales, ops), (regs, slots)) = (c.plan_stats(), c.plan_registers());
                    format!(
                        "{muls} products, {scales} by constants, {ops} instructions; {regs} registers for {slots} slots"
                    )
                }
                println!(
                    "  compiled matrix programs (the kinds' uniform circuits): P-recv {}; S-recv {}; P-send {}; S-send {}; P-boot {}; S-boot {}",
                    tape(&cp),
                    tape(&cs),
                    tape(&ct),
                    tape(&cu),
                    tape(&cb),
                    tape(&cv)
                );
            }
            q_bench("P-recv", &gens.defer_p, &idx.defer.claims);
            q_bench("S-recv", &gens.defer_s, &sidx.defer.claims);
            q_bench("P-send", &gens.defer_p, &idx_t.defer.claims);
            q_bench("S-send", &gens.defer_s, &sidx_t.defer.claims);
            if gens.keys_p.is_some() {
                let hop_c = [
                    (Arc::new(cp), Arc::new(cs)),
                    (Arc::new(ct), Arc::new(cu)),
                    (Arc::new(cb), Arc::new(cv)),
                ];
                let boot_a =
                    linked_uniform(&hop_c[2].0, ib, &hop_c[2].1, iv, boot, full::BOOTSTRAP);
                wallet_chains(hop, &gens, &hop_c, boot_a);
            }
            break;
        }
        let (cp, cs, ct, cu, cb, cv) = (
            Arc::new(cp),
            Arc::new(cs),
            Arc::new(ct),
            Arc::new(cu),
            Arc::new(cb),
            Arc::new(cv),
        );
        let clock = Instant::now();
        let recv_u = linked_uniform(&cp, ip, &cs, is, recv, full::RECEIVE);
        let send_u = linked_uniform(&ct, it, &cu, iu, send, full::SEND);
        let boot_u = linked_uniform(&cb, ib, &cv, iv, boot, full::BOOTSTRAP);
        prev = Some([(cp, cs), (ct, cu), (cb, cv)]);
        gens = full::next_gens(&recv_u, Some(&send_u), Some(&boot_u));
        pairs = vec![
            full::trace_uniform(recv_u, &gens),
            full::trace_uniform(send_u, &gens),
        ];
        println!(
            "  next hop's pairs traced in {:.1} s",
            clock.elapsed().as_secs_f64()
        );
    }
}

/// Two wallets' chains on the last hop's circuits, started where production
/// starts them, at Bootstrap pairs: B's Send after Bootstrap (S1), A's
/// Receive after Bootstrap paid by S1 (R1), A's Send after Receive (S2) and
/// Send after Send (S3), B's Receive after Send paid by S2 (R2) and Receive
/// after Receive paid by S3 (R3). Each operation's circuits are rebuilt over
/// its inputs and must be the hop's (its slot takes any kind); every pair
/// verifies and its accumulators hold natively. Over a Bootstrap
/// predecessor, the predecessor's kind bits (their shared copies and the
/// fresh claim's c0, c1) each break a row when changed.
fn wallet_chains(
    hop: usize,
    gens: &full::PairGens,
    hop_c: &[(
        std::sync::Arc<uniform::UniformCircuit<iroha_pasta::Fp>>,
        std::sync::Arc<uniform::UniformCircuit<verifier::Fq>>,
    ); 3],
    boot_a: full::UniformPair,
) {
    use std::sync::Arc;
    let (pl, sl) = (hop_c[0].0.layout(), hop_c[0].1.layout());
    let mut rng = SpikeRng::new(b"wallet chains compare");
    let mut same = |name: &str,
                    kind: usize,
                    cp: &uniform::UniformCircuit<iroha_pasta::Fp>,
                    cs: &uniform::UniformCircuit<verifier::Fq>| {
        let (a, b) = (
            compare_hop_circuit(&format!("P {name}"), &hop_c[kind].0, cp, &mut rng),
            compare_hop_circuit(&format!("S {name}"), &hop_c[kind].1, cs, &mut rng),
        );
        if !(a.ends_with("EQUAL") && b.ends_with("EQUAL")) {
            for (side, d) in [
                ("P", template_differences(&hop_c[kind].0, cp, 6)),
                ("S", template_differences(&hop_c[kind].1, cs, 6)),
            ] {
                if !d.is_empty() {
                    println!("    {name}: {side} templates differ in: {}", d.join("; "));
                }
            }
        }
        assert!(
            a.ends_with("EQUAL") && b.ends_with("EQUAL"),
            "{name}: the hop's circuits ({a}; {b})"
        );
    };
    println!("  wallet chains from Bootstrap pairs, on the hop's circuits:");
    // Wallet B's Bootstrap: the hop's circuits, its own randomness.
    let clock = Instant::now();
    let (cb, ib, wb, _) = p_boot_at(gens, pl);
    let (w, _) = full::s_boot(gens);
    let (cv, iv, wv) = uniform_at!("S-uniform", w, 13, 62, b"s-boot-uniform", sl);
    same("Bootstrap (B)", 2, &cb, &cv);
    let l = prove_linked_pair("wallet B boot", hop, &cb, &ib, &wb, &cv, &iv, &wv);
    println!(
        "    Bootstrap (B): built and proved in {:.1} s",
        clock.elapsed().as_secs_f64()
    );
    let boot_b = linked_uniform(&Arc::new(cb), ib, &Arc::new(cv), iv, l, full::BOOTSTRAP);
    let tr = |u: &full::UniformPair| full::trace_uniform(u.clone(), gens);
    // One operation over traced inputs (Receive: predecessor, payer; Send:
    // predecessor): rebuilt, compared with the hop's, proved, checked.
    #[allow(clippy::type_complexity)]
    let mut op = |label: &str, kind: usize, inputs: &[full::TracedPair], seed: &str| {
        let clock = Instant::now();
        let ((cp, ip, wp), (cs, is, ws), kind_at, coords) = if kind == full::RECEIVE {
            let (w, idx) = full::p_recv(gens, inputs);
            let p = uniform_of!("P-uniform", w, 14, 64, b"p-recv-uniform");
            let (w, sidx) = full::s_recv(gens, inputs);
            let s = uniform_of!("S-uniform", w, 13, 62, b"s-recv-uniform");
            (
                p,
                s,
                (idx.shared, sidx.shared, idx.kind_at, sidx.kind_at),
                (idx.defer.fold.coords, sidx.defer.fold.coords),
            )
        } else {
            let (w, idx) = full::p_send(gens, &inputs[0]);
            let p = uniform_at!("P-uniform", w, 14, 64, b"p-send-uniform", pl);
            let (w, sidx) = full::s_send(gens, &inputs[0]);
            let s = uniform_at!("S-uniform", w, 13, 62, b"s-send-uniform", sl);
            (
                p,
                s,
                (idx.shared, sidx.shared, idx.kind_at, sidx.kind_at),
                (idx.defer.fold.coords, sidx.defer.fold.coords),
            )
        };
        same(label, kind, &cp, &cs);
        if inputs[0].kind == full::BOOTSTRAP {
            // Over a Bootstrap predecessor: its kind bits' shared copies and
            // the fresh claim's selector coordinates (c0 first, c1 last) are
            // pinned.
            let (n_p, n_s) = (gens.defer_p.n(), gens.defer_s.n());
            let (sh_p, sh_s, at_p, at_s) = kind_at;
            let p_breaks = row_breaks(
                &cp,
                &ip,
                &wp,
                &[
                    (sh_p, 0, at_p),
                    (sh_p, 0, at_p + 1),
                    (coords.0, deferral::C_P[1], 0),
                    (coords.0, deferral::C_P[1], n_p - 1),
                ],
            );
            let s_breaks = row_breaks(
                &cs,
                &is,
                &ws,
                &[
                    (sh_s, 0, at_s),
                    (sh_s, 0, at_s + 1),
                    (coords.1, deferral::C_P[1], 0),
                    (coords.1, deferral::C_P[1], n_s - 1),
                ],
            );
            println!(
                "    {label}: the Bootstrap predecessor's kind bits, changed (shared c0, c1; fresh claim's c0, c1), break a row: P {p_breaks:?}, S {s_breaks:?}"
            );
            assert!(
                p_breaks.iter().chain(&s_breaks).all(|b| *b),
                "the predecessor's kind bits are pinned"
            );
        }
        let l = prove_linked_pair(seed, hop, &cp, &ip, &wp, &cs, &is, &ws);
        let holds = report_pair(&format!("    {label}:"), &l, gens);
        assert_eq!(holds, (true, true), "{label}: the accumulators hold");
        println!(
            "      (built, checked and proved in {:.1} s)",
            clock.elapsed().as_secs_f64()
        );
        linked_uniform(&Arc::new(cp), ip, &Arc::new(cs), is, l, kind)
    };
    let s1 = op(
        "Send after Bootstrap (B)",
        full::SEND,
        &[tr(&boot_b)],
        "chain S1",
    );
    drop(boot_b);
    let r1 = op(
        "Receive after Bootstrap (A, paid by S1)",
        full::RECEIVE,
        &[tr(&boot_a), tr(&s1)],
        "chain R1",
    );
    drop(boot_a);
    let s2 = op("Send after Receive (A)", full::SEND, &[tr(&r1)], "chain S2");
    drop(r1);
    let s3 = op("Send after Send (A)", full::SEND, &[tr(&s2)], "chain S3");
    let r2 = op(
        "Receive after Send (B, paid by S2)",
        full::RECEIVE,
        &[tr(&s1), tr(&s2)],
        "chain R2",
    );
    drop((s1, s2));
    let _r3 = op(
        "Receive after Receive (B, paid by S3)",
        full::RECEIVE,
        &[tr(&r2), tr(&s3)],
        "chain R3",
    );
    println!("  every order from a Bootstrap start verifies, and every accumulator holds");
}

/// The header line and compiled evaluation of an Fq uniform circuit.
fn describe_uniform_q(c: &uniform::UniformCircuit<full::Fq>, title: &str, built: f64) {
    let shape = c.shape;
    let (cells, rows) = c.sizes();
    let (terms, work) = c.template_size();
    println!(
        "{title} as uniform arrays (built in {built:.2} s): {cells} cells, {rows} constraint rows, 2^{} constraints, witness {} rows of {} (active {}, {} constant rows in the key)",
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        r1cs::Circuit::fixed_rows(c, &shape)
    );
    println!("  template terms {terms}; verifier matrix-MLE work {work} products of two slots");
}

/// Proves and verifies an Fq uniform circuit (Pallas commitments).
fn prove_uniform_q(
    c: &uniform::UniformCircuit<full::Fq>,
    inst: &Instance<full::Fq>,
    wit: &Witness<full::Fq>,
    repeats: usize,
    _gp: &Gens<Vesta>,
) {
    let host = if cfg!(target_os = "android") {
        "Pixel"
    } else {
        "Mac"
    };
    let shape = c.shape;
    let gens = Gens::<Pallas>::new(shape.cols());
    let table_build = spartan::warm_table(&gens, &shape);
    println!(
        "  commit tables built in {table_build:.3} s; {}",
        mem_note("after the tables")
    );
    let mut rng = SpikeRng::new(b"s-recv-uniform prove");
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&shape, &gens, inst, wit, &mut rng, c);
        let bytes = proof.to_bytes();
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&shape, &gens, inst, &bytes, c);
        println!(
            "  {host} uniform prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; verify {:.3} s: {:?}; proof {} B ({}); {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            clock.elapsed().as_secs_f64(),
            ok.map(|_| "accepted"),
            bytes.len(),
            proof_tag(&bytes),
            mem_note("after it")
        );
    }
}

/// Stage A2 step 7, part 1: Receive's own relation as uniform arrays (the
/// statement sponge and the insertions in the permutation pool, the signed
/// messages, the small blocks), against stage A's `receive_relation`.
fn run_uniform_own() {
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let cs = operation::receive_relation(11, 16);
    let mut asm = sparse::Assembly::<iroha_pasta::Fp>::new();
    operation::to_assembly(&cs, &mut asm);
    drop(cs);
    let (g, gshape, _, _) = sparse::General::build(asm, "stage-a-own", 14, 8, b"stage-a-own");
    println!("stage A's own relation: {}", sparse::describe(&g, &gshape));
    drop(g);
    let clock = Instant::now();
    let (w, own) = full::own_circuit();
    let (sites_e, sites_i) = w.copy_sites();
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (c, inst, wit) = uniform::UniformCircuit::build_with_copies(
        "uniform-own",
        arrays,
        pi,
        counts,
        tables,
        copies,
        14,
        8,
        b"uniform-own",
    );
    describe_uniform(
        &c,
        "Receive's own relation",
        clock.elapsed().as_secs_f64(),
        usize::MAX,
    );
    let (entries, reads) = c.copy_counts();
    println!(
        "  copies: {sites_e} export sites with {entries} entries, {sites_i} read sites with {reads} reads; {} permutations in the pool",
        own.pool.perms
    );
    let bad = c.cell_index(own.pool.input, full::P_A, own.statement[40]);
    prove_uniform_and_sparse(&c, &inst, &wit, bad, "uniform-own", repeats);
}

/// Stage A2 step 6: the remaining flows wired, each composite also built
/// without its flows to price them. Part 1: the own relation's signed
/// messages (`2^ZKS_PERMS_LOG` permutations, default 2^9; two digests signed:
/// copy into SHA-256's m, z linked into the comb). Part 2: one P-recv proof's
/// scalar side bound to the binding chain over its shared values.
fn run_uniform_wired() {
    use spartan::Link;
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let log: usize = std::env::var("ZKS_PERMS_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let products = |c: &uniform::UniformCircuit<iroha_pasta::Fp>| c.template_size().1;
    // Part 1.
    let w = wired::own_chain(log, 2, false);
    let (c0, _, _) = uniform::UniformCircuit::build_with_copies(
        "uniform-own-unwired",
        w.arrays,
        w.pi,
        w.counts,
        w.tables,
        w.copies,
        14,
        8,
        b"uniform-own-unwired",
    );
    let (unwired, (cells0, rows0)) = (products(&c0), c0.sizes());
    drop(c0);
    let clock = Instant::now();
    let w = wired::own_chain(log, 2, true);
    let (c, inst, wit) = uniform::UniformCircuit::build_with_copies(
        "uniform-wired-own",
        w.arrays,
        w.pi,
        w.counts,
        w.tables,
        w.copies,
        14,
        8,
        b"uniform-wired-own",
    );
    describe_uniform(
        &c,
        &format!(
            "Own relation's signed messages ({} permutations, 2 digests signed, SHA-256, comb)",
            1usize << log
        ),
        clock.elapsed().as_secs_f64(),
        usize::MAX,
    );
    let (entries, reads) = c.copy_counts();
    let (cells, rows) = c.sizes();
    println!(
        "  wiring: {entries} copy entries, {reads} reads; {} products against {unwired} for the same blocks unwired (+{}); {} cells (+{}), {} constraint rows (+{})",
        products(&c),
        products(&c) as i64 - unwired as i64,
        cells,
        cells as i64 - cells0 as i64,
        rows,
        rows as i64 - rows0 as i64
    );
    let bad = c.cell_index(wired::W_SHA + sha::A_OUT, wired::Z_ACC + 1, (1 << 3) | 4);
    prove_uniform_and_sparse(&c, &inst, &wit, bad, "uniform-wired-own", repeats);
    drop((c, inst, wit));
    // Part 2.
    let shape = Shape {
        label: "P-recv",
        sx: 19,
        row_bits: 5,
        col_bits: 14,
        k: 3,
    };
    let inherited = 2;
    let io_len = census::STATEMENT_IO + inherited * (2 + shape.col_bits);
    let gens = Gens::<Vesta>::new(shape.cols());
    let (pinst, pwit, claims) =
        verifier::instance::<Vesta>(shape, &gens, inherited, io_len, b"uniform-wired-scalar");
    let proof = spartan::prove_with_claims(
        &shape,
        &gens,
        &pinst,
        &pwit,
        &mut SpikeRng::new(b"uniform-wired-scalar/prove"),
        &claims,
    );
    let tr = verifier::trace(&shape, &gens, &pinst, &proof, &claims, &Link::None);
    let sw = wired::scalar_wired(&shape, &pinst, &proof, &claims, &tr, false);
    let (c0, _, _) = uniform::UniformCircuit::build_with_copies(
        "uniform-scalar-unwired",
        sw.wired.arrays,
        sw.wired.pi,
        sw.wired.counts,
        sw.wired.tables,
        sw.wired.copies,
        12,
        4,
        b"uniform-scalar-unwired",
    );
    let (unwired, (cells0, rows0)) = (products(&c0), c0.sizes());
    drop(c0);
    let clock = Instant::now();
    let sw = wired::scalar_wired(&shape, &pinst, &proof, &claims, &tr, true);
    let (endo, limbs, shared, inputs) = (sw.endo, sw.limbs, sw.shared, sw.inputs);
    let (c, uinst, uwit) = uniform::UniformCircuit::build_with_copies(
        "uniform-wired-scalar",
        sw.wired.arrays,
        sw.wired.pi,
        sw.wired.counts,
        sw.wired.tables,
        sw.wired.copies,
        12,
        4,
        b"uniform-wired-scalar",
    );
    describe_uniform(
        &c,
        "One P-recv proof's scalar side bound to the binding chain",
        clock.elapsed().as_secs_f64(),
        usize::MAX,
    );
    for (name, a, k, i, v) in &sw.expected {
        assert!(
            uwit.w[c.cell_index(*a, *k, *i)] == *v,
            "{name} equals the native verifier's"
        );
    }
    let (cells, rows) = c.sizes();
    println!(
        "  wiring: {shared} shared values in the chain; {inputs} scalar-side inputs bound through {endo} endo values and {limbs} limb splits; {} products against {unwired} for the scalar side and the chain unwired (+{}); {} cells (+{}), {} constraint rows (+{})",
        products(&c),
        products(&c) as i64 - unwired as i64,
        cells,
        cells as i64 - cells0 as i64,
        rows,
        rows as i64 - rows0 as i64
    );
    let bad = c.cell_index(wired::S_LIMB, wired::L_VAL, 1);
    prove_uniform_and_sparse(&c, &uinst, &uwit, bad, "uniform-wired-scalar", repeats);
    drop((c, uinst, uwit));
    // Part 3: everything so far in one circuit, so the program shares what it
    // can: the own chain, the scalar side bound to the chain, P-recv's ladders.
    let clock = Instant::now();
    let own = wired::own_chain(log, 2, true);
    let mut arrays = own.arrays;
    let (mut pi, mut counts, mut tables) = (own.pi, own.counts, own.tables);
    let sw = wired::scalar_wired(&shape, &pinst, &proof, &claims, &tr, true);
    let (a1, f1) = (arrays.len(), pi.len());
    let mut side = sw.wired.arrays;
    uniform::relocate(&mut side, a1, f1);
    arrays.extend(side);
    pi.extend(sw.wired.pi);
    counts.extend(sw.wired.counts);
    tables.extend(sw.wired.tables.iter().map(|(a, t)| (a + a1, *t)));
    let (a2, f2) = (arrays.len(), pi.len());
    let mut lad = uniform_ladder_arrays();
    uniform::relocate(&mut lad.0, a2, f2);
    arrays.extend(lad.0);
    pi.extend(lad.1);
    counts.extend(lad.2);
    // One table per id (the binding's range table duplicates the comb's).
    let mut seen = std::collections::HashSet::new();
    tables.retain(|(_, t)| seen.insert(*t));
    let (c, cinst, cwit) = uniform::UniformCircuit::build_with_copies(
        "uniform-wired-all",
        arrays,
        pi,
        counts,
        tables,
        own.copies,
        14,
        8,
        b"uniform-wired-all",
    );
    for (name, a, k, i, v) in &sw.expected {
        assert!(
            cwit.w[c.cell_index(a + a1, *k, *i)] == *v,
            "{name} equals the native verifier's"
        );
    }
    describe_uniform(
        &c,
        "Own chain + scalar side bound to the chain + P-recv's ladders, one circuit",
        clock.elapsed().as_secs_f64(),
        usize::MAX,
    );
    let bad = c.cell_index(a1 + wired::S_LIMB, wired::L_VAL, 1);
    prove_uniform_and_sparse(&c, &cinst, &cwit, bad, "uniform-wired-all", repeats);
}

/// P-recv's ladders as `run_uniform_ladders` builds them by default (123
/// endoscalings; two variable-base and six fixed-base multiplications in one
/// family; the mask gates): arrays from index 0, families, counts.
fn uniform_ladder_arrays() -> (Vec<uniform::Array<iroha_pasta::Fp>>, Vec<usize>, Vec<usize>) {
    use iroha_pasta::{Ep as Pallas, Fq};
    type Fb = iroha_pasta::Fp;
    let mut rng = SpikeRng::new(b"uniform-ladders");
    let point = |rng: &mut SpikeRng| <Pallas as group::Group>::generator() * rng.scalar::<Fq>();
    let bits = |rng: &mut SpikeRng, n: usize| -> Vec<bool> {
        (0..n).map(|_| rng.below::<Fb>(1) == Fb::ONE).collect()
    };
    let endo_in: Vec<(Pallas, Vec<bool>)> = (0..123)
        .map(|_| (point(&mut rng), bits(&mut rng, 126)))
        .collect();
    let var_in: Vec<(Pallas, Vec<bool>)> = (0..2)
        .map(|_| (point(&mut rng), bits(&mut rng, 255)))
        .collect();
    let (u, h) = (point(&mut rng), point(&mut rng));
    let fixed_u: Vec<Vec<bool>> = (0..4).map(|_| bits(&mut rng, 254)).collect();
    let fixed_h: Vec<Vec<bool>> = (0..2).map(|_| bits(&mut rng, 254)).collect();
    let mut arrays = Vec::new();
    arrays.extend(ladders::endoscale::<Pallas>(0, 0, 7, &endo_in).arrays);
    let inputs: Vec<(Pallas, Vec<bool>, bool)> = var_in
        .iter()
        .map(|(p, b)| (*p, b.clone(), false))
        .chain(fixed_u.iter().map(|b| (u, b.clone(), true)))
        .chain(fixed_h.iter().map(|b| (h, b.clone(), true)))
        .map(|(p, mut b, f)| {
            b.resize(255, false);
            (p, b, f)
        })
        .collect();
    arrays.extend(ladders::mixed_mul::<Pallas>(3, 1, 3, &inputs).arrays);
    let ma: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
    let mb: Vec<Fb> = vec![rng.scalar(), rng.scalar()];
    let mc: Vec<Fb> = ma.iter().zip(&mb).map(|(x, y)| *x * y).collect();
    arrays.push(uniform::Array {
        name: "mask",
        family: 2,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 3],
        rows: vec![[uniform::own(0, 1), uniform::own(1, 1), uniform::own(2, 1)]],
        values: vec![ma, mb, mc],
    });
    (arrays, vec![7, 3, 1], vec![123, 8, 2])
}

/// Stage A2 step 4: the cross-circuit binding chain over `ZKS_BINDING_VALUES`
/// (default 258, P-recv's shared values) as uniform arrays.
fn run_uniform_binding() {
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let n: usize = std::env::var("ZKS_BINDING_VALUES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(258)
        .max(1);
    let clock = Instant::now();
    let (rho, mu, values) = nonnative::sample_binding(n);
    let nonnative::Binding {
        arrays,
        tables,
        shape: bshape,
        ..
    } = nonnative::binding_chain(0, 0, 1, &rho, &mu, &values);
    let (c, inst, wit) = uniform::UniformCircuit::build(
        "uniform-binding",
        arrays,
        vec![0, 0],
        vec![1, 1],
        tables,
        14,
        8,
        b"uniform-binding",
    );
    let (rows, lookups, cells) = describe_uniform(
        &c,
        &format!("Binding chain over {n} shared values"),
        clock.elapsed().as_secs_f64(),
        0,
    );
    println!(
        "  per value: {:.1} rows other than lookups + {:.1} lookups = {:.1} constraints with LogUp, {:.1} cells without helpers (check {} cells, {} lookups); stage A: 83 constraints per value at chunk 13, 92.4 at chunk 11",
        rows as f64 / n as f64,
        lookups as f64 / n as f64,
        (rows + lookups) as f64 / n as f64,
        cells as f64 / n as f64,
        bshape.cells,
        bshape.lookups()
    );
    let bad = c.cell_index(nonnative::B_VALUE, 1, n / 2);
    prove_uniform_and_sparse(&c, &inst, &wit, bad, "uniform-binding", repeats);
}

/// Stage A2 step 1: `2^ZKS_PERMS_LOG` (default 2^9) RP57 permutations as
/// uniform round arrays, proved and verified, against the same constraints
/// as a sparse assembly (stage A's representation).
fn run_uniform_perm() {
    type Fp = iroha_pasta::Fp;
    let log: usize = std::env::var("ZKS_PERMS_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    let repeats: usize = std::env::var("ZKS_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let host = if cfg!(target_os = "android") {
        "Pixel"
    } else {
        "Mac"
    };
    let clock = Instant::now();
    let (arrays, pi, _) = uniform::rp57::<Fp>(log, b"uniform-perm");
    let counts = pi.iter().map(|p| 1 << p).collect();
    let (c, inst, wit) = uniform::UniformCircuit::build(
        "uniform-perm",
        arrays,
        pi,
        counts,
        Vec::new(),
        14,
        8,
        b"uniform-perm",
    );
    let shape = c.shape;
    let (cells, rows) = c.sizes();
    let plan = plan_note(&c);
    let (terms, work) = c.template_size();
    println!(
        "{} RP57 permutations as uniform round arrays (built in {:.2} s): {cells} cells, {rows} constraint rows ({:.1} per permutation), 2^{} constraints, witness {} rows of {} (active {}), proof {} B",
        1 << log,
        clock.elapsed().as_secs_f64(),
        rows as f64 / (1u64 << log) as f64,
        shape.sx,
        shape.rows(),
        shape.cols(),
        shape.active_cols(),
        shape.proof_bytes()
    );
    for (a, arr) in c.arrays.iter().enumerate() {
        let t: usize = arr
            .rows
            .iter()
            .map(|r| r[0].len() + r[1].len() + r[2].len())
            .sum();
        println!(
            "  array {a} {:<18} instances 2^{} x {} cells, {} template rows, {t} template terms",
            arr.name,
            c.pi[arr.family] + arr.rho,
            arr.cells(),
            arr.rows.len()
        );
    }
    println!(
        "  template terms {terms}; verifier matrix-MLE work {work} products of two slots (A, B and C together), independent of the permutation count"
    );
    println!("  {plan}; one evaluation {:.6} s", plan_call_time(&c, 50));
    let gens = Gens::<Vesta>::new(shape.cols());
    let mut rng = SpikeRng::new(b"uniform-perm");
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &c);
        let bytes = proof.to_bytes();
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&shape, &gens, &inst, &bytes, &c);
        println!(
            "  {host} uniform prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; instantiate {:.4} s; verify {:.3} s: {:?}; proof {} B; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            tm.instantiate,
            clock.elapsed().as_secs_f64(),
            ok.map(|_| "accepted"),
            bytes.len(),
            mem_note("after it")
        );
    }
    // A changed S-box cell in the middle of the arrays is refused.
    let mut bad = wit.w.clone();
    let target = bad
        .iter()
        .enumerate()
        .filter(|(_, v)| !bool::from(ff::Field::is_zero(*v)))
        .map(|(i, _)| i)
        .nth(cells / 2)
        .unwrap();
    bad[target] += Fp::ONE;
    let (proof, _) = spartan::prove_circuit(
        &shape,
        &gens,
        &inst,
        &r1cs::Witness { w: bad },
        &mut rng,
        &c,
    );
    println!(
        "  changed cell at {target}: {:?}",
        spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).map(|_| "accepted")
    );
    // The same constraints, as stage A would prove them.
    let clock = Instant::now();
    let asm = c.to_assembly();
    let (g, gshape, ginst, gwit) =
        sparse::General::build(asm, "sparse-perm", 14, 8, b"sparse-perm");
    println!(
        "sparse assembly of the same rows ({:.2} s): {}",
        clock.elapsed().as_secs_f64(),
        sparse::describe(&g, &gshape)
    );
    let ggens = Gens::<Vesta>::new(gshape.cols());
    for i in 0..repeats {
        let (proof, tm) = spartan::prove_circuit(&gshape, &ggens, &ginst, &gwit, &mut rng, &g);
        let bytes = proof.to_bytes();
        *sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
        let clock = Instant::now();
        let ok = spartan::verify_circuit(&gshape, &ggens, &ginst, &bytes, &g);
        let v = clock.elapsed().as_secs_f64();
        let vm = sparse::LAST_BUILD.lock().unwrap().1;
        println!(
            "  {host} sparse prove {}: core {:.3} s = commit {:.3} [msm {:.3}] + outer {:.3} + inner {:.3} + open {:.3}; matrices {:.3} s; verify {:.3} s + matrices {vm:.3} s: {:?}; proof {} B; {}",
            i + 1,
            tm.core(),
            tm.commit,
            tm.split[1],
            tm.outer,
            tm.inner,
            tm.open,
            tm.matrices,
            v - vm,
            ok.map(|_| "accepted"),
            bytes.len(),
            mem_note("after it")
        );
    }
    println!(
        "  reference: the stage A gadget keeps partial-round lanes as linear combinations: 243 rows and about 6,765 template nonzeros per permutation (block census v4)"
    );
}

fn run_real_receive() {
    use ff::{Field, PrimeField};
    use operation::{receive_relation, rlc_chain_cs, to_assembly};
    use spartan::Link;
    use verifier::{
        EXPORT, Exported, PairSpec, instance, prove_pair, rlc_chain_native, rlc_value,
        synthesize_proof, take_exports,
    };
    type Fp = iroha_pasta::Fp;
    type Fq = iroha_pasta::Fq;
    fn import<S: fs::Scalar>(asm: &mut sparse::Assembly<S>, e: &Exported<S>) {
        let map = asm.import_vars(&e.vals, &e.widths);
        asm.row_scope.resize(asm.rows.len(), 0);
        for (i, (a, b, c)) in e.rows.iter().enumerate() {
            asm.rows.push((
                sparse::remap(a, &map),
                sparse::remap(b, &map),
                sparse::remap(c, &map),
            ));
            asm.row_scope.push(e.row_scope.get(i).copied().unwrap_or(0));
        }
        asm.filler += e.counted;
    }
    println!(
        "challenge schedule: {}",
        if fs::lean_schedule() {
            "lean (gate 1e)"
        } else {
            "per variable (gate 1d)"
        }
    );
    let census_only = std::env::var("ZKS_BLOCKS").is_ok();
    blocks::ENABLED.store(census_only, std::sync::atomic::Ordering::Relaxed);
    let clock = Instant::now();
    // The own relation (Fp).
    let cs = receive_relation(11, 16);
    let mut asm_p = sparse::Assembly::<Fp>::new();
    to_assembly(&cs, &mut asm_p);
    drop(cs);
    println!("{}", mem_note("after the own relation"));
    let own_rows = asm_p.rows.len() + asm_p.filler;
    // The two incoming pairs: the predecessor Receive and the payer's Send.
    let shape = |label: &'static str, sx, row_bits, col_bits| Shape {
        label,
        sx,
        row_bits,
        col_bits,
        k: 3,
    };
    let gv = Gens::<Vesta>::new(1 << 14);
    let gp = Gens::<Pallas>::new(1 << 13);
    let spec = |sp: Shape, ss: Shape, inh: usize, seed: &[u8]| {
        let io_p = census::STATEMENT_IO + inh * (2 + sp.col_bits);
        let io_s = census::STATEMENT_IO + inh * (2 + ss.col_bits);
        let (inst_p, wit_p, claims_p) =
            instance::<Vesta>(sp, &gv, inh, io_p, &[seed, b"/p"].concat());
        let (inst_s, wit_s, claims_s) =
            instance::<Pallas>(ss, &gp, inh, io_s, &[seed, b"/s"].concat());
        PairSpec {
            shape_p: sp,
            gens_p: &gv,
            inst_p,
            wit_p,
            claims_p,
            shape_s: ss,
            gens_s: &gp,
            inst_s,
            wit_s,
            claims_s,
        }
    };
    let pairs = [
        spec(
            shape("P-recv", 19, 5, 14),
            shape("S-recv", 18, 5, 13),
            2,
            b"pred-receive",
        ),
        spec(
            shape("P-send", 19, 5, 14),
            shape("S-send", 17, 4, 13),
            1,
            b"payer-send",
        ),
    ];
    let mut asm_s = sparse::Assembly::<Fq>::new();
    let mut values: Vec<u128> = Vec::new();
    EXPORT.store(true, std::sync::atomic::Ordering::Relaxed);
    for (i, sp) in pairs.iter().enumerate() {
        let pr = prove_pair(sp, &[b"pair", &[i as u8][..]].concat(), &|rho| {
            rlc_value(rho, Fq::from(3), &[1, 2, 3]).to_repr()
        });
        let r = pr.r;
        let fixed = move |_: &[u8; 32]| r;
        let vp = synthesize_proof(
            &sp.shape_p,
            &gv,
            &sp.inst_p,
            &pr.p,
            &sp.claims_p,
            &Link::First {
                rho: pr.rho,
                r: pr.r,
            },
        );
        let mut ex = take_exports();
        let p_scalar = ex.pop().unwrap().downcast::<Exported<Fp>>().unwrap();
        let p_points = ex.pop().unwrap().downcast::<Exported<Fq>>().unwrap();
        let vs = synthesize_proof(
            &sp.shape_s,
            &gp,
            &sp.inst_s,
            &pr.s,
            &sp.claims_s,
            &Link::Second {
                d: pr.d,
                rlc: &fixed,
            },
        );
        let mut ex = take_exports();
        let s_scalar = ex.pop().unwrap().downcast::<Exported<Fq>>().unwrap();
        let s_points = ex.pop().unwrap().downcast::<Exported<Fp>>().unwrap();
        import(&mut asm_p, &p_scalar);
        import(&mut asm_p, &s_points);
        import(&mut asm_s, &p_points);
        import(&mut asm_s, &s_scalar);
        // The real circuit's matrix MLE, budgeted (the toy's is synthesized).
        asm_p.filler += census::MATRIX_MLE
            .load(std::sync::atomic::Ordering::Relaxed)
            .max(8_000);
        asm_s.filler += 8_000;
        values.extend(vp.values.iter().chain(&vs.values));
        println!("{}", mem_note(&format!("after incoming pair {}", i + 1)));
    }
    EXPORT.store(false, std::sync::atomic::Ordering::Relaxed);
    // The binding chains over every shared value (rho is drawn in phase 2 of
    // the real proofs; a fixed value stands in for the witness).
    let rho = Fq::from(0x0123_4567_u64).square().square();
    let mu = Fq::from(0x0bad_cafe_u64).square();
    let (chain, rp) = rlc_chain_cs(
        operation::OpCs::recording(11),
        &num_bigint::BigUint::from_bytes_le(&rho.to_repr()),
        &num_bigint::BigUint::from_bytes_le(&mu.to_repr()),
        &values,
    );
    to_assembly(&chain, &mut asm_p);
    EXPORT.store(true, std::sync::atomic::Ordering::Relaxed);
    let (_, rn) = rlc_chain_native(&rho.to_repr(), mu, &values);
    let chain_s = take_exports()
        .pop()
        .unwrap()
        .downcast::<Exported<Fq>>()
        .unwrap();
    EXPORT.store(false, std::sync::atomic::Ordering::Relaxed);
    import(&mut asm_s, &chain_s);
    assert_eq!(
        rp,
        num_bigint::BigUint::from_bytes_le(&rn.to_repr()),
        "both chains give one R"
    );
    println!(
        "assembled in {:.1} s: own relation {own_rows} rows; {} shared values bound; {}",
        clock.elapsed().as_secs_f64(),
        values.len(),
        mem_note("assembled")
    );
    if census_only {
        for level in [blocks::Level::Inner, blocks::Level::Outer] {
            println!("P-recv block census\n{}", blocks::census(&asm_p, level));
            println!("S-recv block census\n{}", blocks::census(&asm_s, level));
        }
        return;
    }
    // Prove and verify both; then the Pixel model applied to this exact layout.
    fn measure<C: iroha_pasta::PastaCurve>(
        name: &str,
        g: &sparse::General<ipa::F<C>>,
        shape: &Shape,
        gens: &Gens<C>,
        inst: &r1cs::Instance<ipa::F<C>>,
        wit: &r1cs::Witness<ipa::F<C>>,
        rng: &mut SpikeRng,
        census_model: &str,
    ) {
        use ff::Field;
        use r1cs::Circuit;
        println!("{name}: {}", sparse::describe(g, shape));
        let (cols, active) = (shape.cols(), shape.active_cols());
        // Row-granular width classes: every nonzero entry of a row commits at
        // the row's bound. Second-phase helpers are full width: h is never
        // zero, g is zero exactly when its table entry's multiplicity is.
        let lookups = g.asm.lookups.len();
        let used: Vec<bool> = g
            .mult
            .iter()
            .flatten()
            .map(|m| !bool::from(g.asm.vals[*m as usize].is_zero()))
            .collect();
        let mut nonzero = vec![0usize; shape.rows()];
        for (r, n) in nonzero.iter_mut().enumerate().take(g.p1_rows) {
            *n = wit.w[r * cols..r * cols + active]
                .iter()
                .filter(|x| !bool::from(x.is_zero()))
                .count();
        }
        for (k, p) in g.helpers.iter().enumerate() {
            if k < lookups || used[k - lookups] {
                nonzero[*p as usize / cols] += 1;
            }
        }
        let mut by_bound: std::collections::BTreeMap<usize, (usize, usize)> = Default::default();
        let mut widths: std::collections::BTreeMap<u16, usize> = Default::default();
        for r in 0..shape.rows() {
            let e = by_bound.entry(g.bounds[r]).or_insert((0, 0));
            e.0 += 1;
            e.1 += nonzero[r];
            *widths.entry(g.bounds[r] as u16).or_insert(0) += nonzero[r];
        }
        let clock = Instant::now();
        let rel = g.instantiate(
            shape,
            &[ipa::F::<C>::from(5u64), ipa::F::<C>::from(7u64)],
            None,
        );
        let build = clock.elapsed().as_secs_f64();
        let nnz = rel.nnz();
        drop(rel);
        println!("  rows by bound (bits: rows, nonzero entries): {by_bound:?}");
        println!(
            "  nonzeros A {} B {} C {} = {} ({:.2} per used constraint); matrices alone build in {build:.2} s; {}",
            nnz[0],
            nnz[1],
            nnz[2],
            nnz.iter().sum::<usize>(),
            nnz.iter().sum::<usize>() as f64 / g.asm.constraints() as f64,
            mem_note("after a matrix build")
        );
        if std::env::var("ZKS_ROWSTATS").is_ok() {
            row_stats(g, shape, gens, wit, rng);
            return;
        }
        // Each proof: the per-proof core (commits, helpers, sumchecks, IPA)
        // apart from the matrix build, which a deployed prover preprocesses.
        let repeats: usize = std::env::var("ZKS_REPEAT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1)
            .max(1);
        let host = if cfg!(target_os = "android") {
            "Pixel"
        } else {
            "Mac"
        };
        // Commitment tables are per-shape setup, built before the timed proofs.
        let table_build = spartan::warm_table(gens, shape);
        println!(
            "  commit table: {}, built in {table_build:.3} s; {}",
            if spartan::table_commits() {
                format!("fixed-base, window c{}", spartan::table_window::<C>())
            } else {
                "off (variable-base column MSMs)".to_string()
            },
            mem_note("after the table")
        );
        let mut runs: Vec<(spartan::Timings, f64)> = Vec::new();
        let mut size = 0;
        for i in 0..repeats {
            let (proof, tm) = spartan::prove_circuit(shape, gens, inst, wit, rng, g);
            let bytes = proof.to_bytes();
            size = bytes.len();
            *sparse::LAST_BUILD.lock().unwrap() = (0.0, 0.0);
            let clock = Instant::now();
            let ok = spartan::verify_circuit(shape, gens, inst, &bytes, g);
            let v = clock.elapsed().as_secs_f64();
            let vm = sparse::LAST_BUILD.lock().unwrap().1;
            println!(
                "  {host} prove {}: core {:.3} s = commit {:.3} (phase 1 {:.3}, phase 2 {:.3}) [bound checks {:.3}, msm {:.3}, blind {:.3}] + helpers {:.3} + outer {:.3} + inner {:.3} + open {:.3} (relation {:.3}); matrices {:.3} s; verify {:.3} s + matrices {vm:.3} s: {:?}; {}",
                i + 1,
                tm.core(),
                tm.commit,
                tm.phase1,
                tm.commit - tm.phase1,
                tm.split[0],
                tm.split[1],
                tm.split[2],
                tm.helpers,
                tm.outer,
                tm.inner,
                tm.open,
                tm.relation,
                tm.matrices,
                v - vm,
                ok.map(|_| "accepted"),
                mem_note("after it")
            );
            runs.push((tm, v - vm));
        }
        if repeats > 1 {
            let med = |f: &dyn Fn(&(spartan::Timings, f64)) -> f64| {
                let mut x: Vec<f64> = runs.iter().map(f).collect();
                x.sort_by(|a, b| a.total_cmp(b));
                x[x.len() / 2]
            };
            println!(
                "  {host} medians of {repeats}: core {:.3} s; commit {:.3} (phase 1 {:.3}, phase 2 {:.3}) [bound checks {:.3}, msm {:.3}, blind {:.3}]; helpers {:.3}; outer {:.3}; inner {:.3}; open {:.3} (relation {:.3}); verify {:.3} s",
                med(&|r| r.0.core()),
                med(&|r| r.0.commit),
                med(&|r| r.0.phase1),
                med(&|r| r.0.commit - r.0.phase1),
                med(&|r| r.0.split[0]),
                med(&|r| r.0.split[1]),
                med(&|r| r.0.split[2]),
                med(&|r| r.0.helpers),
                med(&|r| r.0.outer),
                med(&|r| r.0.inner),
                med(&|r| r.0.open),
                med(&|r| r.0.relation),
                med(&|r| r.1)
            );
        }
        println!("  proof {size} B; {}", mem_note("after proving"));
        println!(
            "  Pixel model, this layout : {}",
            pixel_breakdown(shape, &widths, g.asm.constraints(), nnz.iter().sum())
        );
        println!("  Pixel model, census      : {census_model}");
        // Full-size refusals: one full-width circuit cell off by one (not a
        // filler stand-in, mask or multiplicity): early, middle and the last
        // such cell (the binding chain's end).
        if std::env::var("ZKS_REFUSE").is_ok() {
            let boundary = g
                .mult
                .iter()
                .flatten()
                .copied()
                .chain([g.mask[0].0])
                .min()
                .unwrap() as usize;
            let wide: Vec<usize> = (1..boundary).filter(|v| g.asm.widths[*v] == 255).collect();
            for v in [
                wide[wide.len() / 10],
                wide[wide.len() / 2],
                wide[wide.len() - 1],
            ] {
                let mut bad = wit.w.clone();
                let at = g.pos[v] as usize;
                let r = at / cols;
                bad[at] += ipa::F::<C>::ONE;
                let (proof, _) =
                    spartan::prove_circuit(shape, gens, inst, &r1cs::Witness { w: bad }, rng, g);
                let res = spartan::verify_circuit(shape, gens, inst, &proof.to_bytes(), g);
                println!(
                    "  refusal, variable {v} (cell {at}, row {r}) off by one: {}",
                    if res.is_err() { "rejected" } else { "ACCEPTED" }
                );
            }
        }
    }
    let (gp_, shp, instp, witp) = sparse::General::build(asm_p, "P-recv-real", 14, 64, b"real-p");
    let mut rng = SpikeRng::new(b"real-receive-prove");
    measure::<Vesta>(
        "P-recv",
        &gp_,
        &shp,
        &gv,
        &instp,
        &witp,
        &mut rng,
        "outer 0.14 inner 0.21 commit 0.76 (full 0.74, 1-bit 0.00, chunks 0.01, other 0.00) density 0.03 IPA 0.34 = 1.47 s",
    );
    drop((gp_, witp));
    let (gs_, shs, insts, wits) = sparse::General::build(asm_s, "S-recv-real", 13, 62, b"real-s");
    measure::<Pallas>(
        "S-recv",
        &gs_,
        &shs,
        &gp,
        &insts,
        &wits,
        &mut rng,
        "outer 0.07 inner 0.10 commit 0.43 (full 0.43, 1-bit 0.00, chunks -0.00, other -0.00) density 0.01 IPA 0.23 = 0.84 s",
    );
}

/// Resident memory at a stage: peak so far and current on Linux and Android
/// (/proc/self/status), current elsewhere (ps).
/// Compiles a uniform circuit's matrix evaluation (timed) and describes the
/// program.
fn plan_note<S: fs::Scalar>(c: &uniform::UniformCircuit<S>) -> String {
    let clock = Instant::now();
    let (muls, scales, ops) = c.plan_stats();
    format!(
        "compiled matrix evaluation: {ops} instructions, {muls} products of two slots (verifier-circuit constraints), {scales} products by a constant; compiled in {:.3} s",
        clock.elapsed().as_secs_f64()
    )
}

/// Mean time of the compiled matrix evaluation over `calls` points.
fn plan_call_time<S: fs::Scalar>(c: &uniform::UniformCircuit<S>, calls: usize) -> f64 {
    let mut rng = SpikeRng::new(b"plan call time");
    let points: Vec<(Vec<S>, Vec<S>, Vec<S>)> = (0..calls)
        .map(|_| {
            (
                (0..c.shape.sx).map(|_| rng.scalar()).collect(),
                (0..c.shape.sz()).map(|_| rng.scalar()).collect(),
                (0..2).map(|_| rng.scalar()).collect(),
            )
        })
        .collect();
    let clock = Instant::now();
    let mut sink = S::ZERO;
    for (rx, ry, ch) in &points {
        let (a, b, cc) = c.evals(rx, ry, ch);
        sink += a + b + cc;
    }
    let t = clock.elapsed().as_secs_f64() / calls as f64;
    assert!(!bool::from(sink.is_zero()) || calls == 0);
    t
}

fn mem_note(stage: &str) -> String {
    let mb = |kb: &str| {
        kb.trim()
            .parse::<f64>()
            .map(|k| k * 1024.0 / 1e6)
            .unwrap_or(0.0)
    };
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let field = |name: &str| {
            status
                .lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.split_whitespace().nth(1))
                .map(mb)
        };
        return format!(
            "memory {stage}: peak {:.0} MB, now {:.0} MB",
            field("VmHWM:").unwrap_or(0.0),
            field("VmRSS:").unwrap_or(0.0)
        );
    }
    let pid = std::process::id().to_string();
    let now = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .map(|o| mb(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(0.0);
    // macOS: the physical footprint (dirty memory, without pages the
    // allocator has freed but the kernel has not yet reclaimed).
    let footprint = std::process::Command::new("footprint")
        .arg(&pid)
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let rest = text.split("Footprint: ").nth(1)?.to_string();
            let mut it = rest.split_whitespace();
            let value: f64 = it.next()?.parse().ok()?;
            let scale = match it.next()? {
                "KB" => 1e3 * 1.024 / 1e6,
                "MB" => 1.048576,
                "GB" => 1073.741824,
                _ => 1e-6,
            };
            Some(value * scale)
        });
    match footprint {
        Some(f) => format!("memory {stage}: resident {now:.0} MB, footprint {f:.0} MB"),
        None => format!("memory {stage}: resident {now:.0} MB"),
    }
}

/// Per-entry commitment time by width class and row length, through the
/// spike's own commit path (bounded secret MSM per row, 8 rows per shape).
fn run_commit_classes() {
    use iroha_pasta::msm::{
        BoundedSecretScalars, MemoryBudget, SharedMemoryBudget, msm_secret,
        msm_secret_bounded_cancellable,
    };
    println!("rayon threads: {}", rayon::current_num_threads());
    let gens = Gens::<Vesta>::new(1 << 16);
    let mut rng = SpikeRng::new(b"commit-classes");
    for cols in [8_192usize, 16_384, 32_768, 65_536] {
        let mut line = format!("{cols:>6} cols:");
        for bits in [1usize, 8, 13, 16, 32, 64, 88, 128, 180, 255] {
            let rows = 8;
            let values: Vec<Vec<F<Vesta>>> = (0..rows)
                .map(|_| {
                    (0..cols)
                        .map(|_| {
                            if bits >= 255 {
                                rng.scalar()
                            } else {
                                rng.below(bits as u32)
                            }
                        })
                        .collect()
                })
                .collect();
            let shared = SharedMemoryBudget::process_default();
            let clock = Instant::now();
            for row in &values {
                let _ = if bits >= 255 {
                    msm_secret::<Vesta>(row, &gens.g[..cols], MemoryBudget::DEFAULT).unwrap()
                } else {
                    let bounded = BoundedSecretScalars::new(row, bits, None).unwrap();
                    msm_secret_bounded_cancellable::<Vesta>(
                        &bounded,
                        &gens.g[..cols],
                        MemoryBudget::DEFAULT,
                        &shared,
                        None,
                    )
                    .unwrap()
                };
            }
            let us = clock.elapsed().as_secs_f64() * 1e6 / (rows * cols) as f64;
            line += &format!(" {bits}b {us:.3}");
        }
        println!("{line}");
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if mode == "census" {
        run_census();
        return;
    }
    if mode == "operation" {
        run_operation();
        return;
    }
    if mode == "commit-classes" {
        run_commit_classes();
        return;
    }
    if mode == "verifier" {
        run_verifier();
        return;
    }
    if mode == "binding" {
        run_binding();
        return;
    }
    if mode == "real-receive" {
        run_real_receive();
        return;
    }
    if mode == "uniform-perm" {
        run_uniform_perm();
        return;
    }
    if mode == "uniform-ladders" {
        run_uniform_ladders();
        return;
    }
    if mode == "uniform-comb" {
        run_uniform_comb();
        return;
    }
    if mode == "uniform-sha" {
        run_uniform_sha();
        return;
    }
    if mode == "uniform-binding" {
        run_uniform_binding();
        return;
    }
    if mode == "uniform-scalar" {
        run_uniform_scalar();
        return;
    }
    if mode == "uniform-wired" {
        run_uniform_wired();
        return;
    }
    if mode == "uniform-own" {
        run_uniform_own();
        return;
    }
    if mode == "uniform-full" {
        run_uniform_full();
        return;
    }
    if mode == "fixed-point" {
        run_fixed_point();
        return;
    }
    println!("rayon threads: {}", rayon::current_num_threads());
    if mode == "tamper" || mode == "all" {
        let clock = Instant::now();
        let vesta_small = Gens::<Vesta>::new(1 << 10);
        let pallas_small = Gens::<Pallas>::new(1 << 10);
        for (k, label) in [
            (4, "tamper-k4"),
            (3, "tamper-k3"),
            (2, "tamper-k2"),
            (5, "tamper-k5"),
            (1, "tamper-k1"),
        ] {
            let shape = small(label, k);
            if shape.tail_len() >= shape.cols() - r1cs::MASK_COLS {
                continue;
            }
            tamper::<Vesta>("vesta", shape, &vesta_small);
            tamper::<Pallas>("pallas", shape, &pallas_small);
        }
        println!("tamper suite {:.1} s", clock.elapsed().as_secs_f64());
    }
    if mode == "bench" || mode == "all" {
        let clock = Instant::now();
        let vesta = Gens::<Vesta>::new(1 << 16);
        let pallas = Gens::<Pallas>::new(1 << 13);
        println!(
            "generators (hash to curve, 2^16 Vesta + 2^13 Pallas): {:.2} s",
            clock.elapsed().as_secs_f64()
        );
        let mut measured = Vec::new();
        measured.push(bench::<Vesta>("vesta", SEND_PRIMARY, &vesta));
        measured.push(bench::<Pallas>("pallas", SEND_SECONDARY, &pallas));
        measured.push(bench::<Vesta>("vesta", RECEIVE_PRIMARY, &vesta));
        measured.push(bench::<Pallas>("pallas", RECEIVE_SECONDARY, &pallas));
        bytes_by_k();
        pair_table(&measured);

        // Critical-path consult: 8,192-column own IPAs on both fields.
        println!("\nshort-IPA shapes (8,192 columns on both fields):");
        let short = [
            Shape {
                label: "send-primary-64x8k",
                sx: 20,
                row_bits: 6,
                col_bits: 13,
                k: 3,
            },
            Shape {
                label: "send-primary-64x8k-k2",
                sx: 20,
                row_bits: 6,
                col_bits: 13,
                k: 2,
            },
            Shape {
                label: "receive-primary-128x8k",
                sx: 21,
                row_bits: 7,
                col_bits: 13,
                k: 3,
            },
        ];
        let mut short_measured = Vec::new();
        for shape in short {
            short_measured.push(bench::<Vesta>("vesta", shape, &pallas_free_gens(&vesta)));
        }
        let inherited13 = 32 + 13 * 32;
        for (pair, primary, secondary, limit) in [
            ("Send 64x8k", short_measured[0].bytes, 1_600, 8_458),
            ("Send 64x8k k=2", short_measured[1].bytes, 1_600, 8_458),
            ("Receive 128x8k", short_measured[2].bytes, 1_888, 9_600),
        ] {
            let total = primary + secondary + 2 * 64 + 2 * inherited13;
            println!(
                "{pair:16} pair {total} B vs {limit} B -> spare {} B",
                limit as i64 - total as i64
            );
        }
    }
}
