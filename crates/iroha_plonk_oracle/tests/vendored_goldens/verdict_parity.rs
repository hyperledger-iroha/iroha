//! Verifier verdict parity on structure-aware tamper corpora (oracle builds
//! only).
//!
//! For each family and curve, at the smallest golden `k` (sigma k = 6, wide
//! k = 8), a corpus of at least [`MIN_CORPUS`] inputs is derived from the
//! golden proof of seed 42 (byte-identical to the vendored proof, checked
//! here) and its instance column:
//!
//! - controls: the golden proof and the seed-43 golden proof (both accept);
//! - per 32-byte message: a low-bit flip, a high-bit flip, all zeros (the
//!   identity encoding or the zero scalar) and all `0xff` (non-canonical);
//! - swaps of neighbouring messages, truncations and trailing bytes;
//! - instance changes: a changed, zeroed or negated value, an appended zero
//!   or one, an empty column, a second column, no column;
//! - seeded random single- and multi-byte corruptions.
//!
//! The native oracle verifier (`verify_full_oracle`, vendored
//! `transcript_repr` injected) must return the vendored verdict on every
//! input. The only allowed differences are the registered stricter
//! rejections in [`DEVIATIONS`] (`deviation_registry`, linked to spec section
//! 14 by `DEV-xx` id): the vendored verifier accepts, the native one rejects
//! with the registered typed reason, and every registered entry must occur in
//! every Blake2b corpus. The native verifier never accepts an input the
//! vendored verifier rejects, and never panics.
//!
//! The same corpora are built a second time on the KAGEMUSHA path (Poseidon
//! transcript, `FoldedGenerator` suffix; `kagemusha_parity`), against the
//! vendored augmented verifier of `iroha_core_zk`. There only DEV-04 occurs:
//! the augmented verifier requires the raw transcript to be consumed
//! exactly, so trailing bytes are rejected on both sides.

use std::panic::{AssertUnwindSafe, catch_unwind};

use halo2_axiom::halo2curves::ff::Field;
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    keys::ProvingKey,
    verifier::{VerifyError, verify_full_oracle},
};
use iroha_plonk_oracle::convert::{CurveBridge, Pallas, Vesta, native_scalars};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;

use crate::{
    cases::{Family, Setup, cases_for, setup},
    deviation_registry::{Class, DEVIATIONS},
    golden_proof_bytes::digest_hex,
    kagemusha_parity::{kagemusha_keys, prove_native_kagemusha},
    proof_parity::prove_native,
};

/// The smallest corpus per family and curve.
pub const MIN_CORPUS: usize = 200;
/// Message size of every proof element (compressed points and scalars).
const MESSAGE: usize = 32;

/// One corpus input.
struct Input<F> {
    label: String,
    class: Class,
    instances: Vec<Vec<F>>,
    proof: Vec<u8>,
    /// Whether both verifiers must accept (a control).
    control: bool,
}

/// Builds the corpus of one golden setup.
fn corpus<B: CurveBridge>(setup: &Setup<B>, proof: &[u8], other: &[u8]) -> Vec<Input<B::VScalar>> {
    let instances = setup.vendored_instances();
    let messages = proof.len() / MESSAGE;
    assert_eq!(proof.len() % MESSAGE, 0, "proofs are whole messages");
    let mut inputs = Vec::new();
    let mut push =
        |label: String, class: Class, instances: Vec<Vec<B::VScalar>>, proof: Vec<u8>| {
            inputs.push(Input {
                label,
                class,
                instances,
                proof,
                control: false,
            });
        };
    let with_message = |index: usize, edit: &dyn Fn(&mut [u8])| {
        let mut tampered = proof.to_vec();
        edit(&mut tampered[index * MESSAGE..(index + 1) * MESSAGE]);
        tampered
    };
    for index in 0..messages {
        push(
            format!("message {index}: low bit flipped"),
            Class::Ordinary,
            instances.clone(),
            with_message(index, &|message| message[0] ^= 1),
        );
        push(
            format!("message {index}: high bit flipped"),
            Class::Ordinary,
            instances.clone(),
            with_message(index, &|message| message[MESSAGE - 1] ^= 0x40),
        );
        push(
            format!("message {index}: zeroed"),
            Class::Ordinary,
            instances.clone(),
            with_message(index, &|message| message.fill(0)),
        );
        push(
            format!("message {index}: all ones"),
            Class::Ordinary,
            instances.clone(),
            with_message(index, &|message| message.fill(0xff)),
        );
    }
    for index in (0..messages.saturating_sub(1)).step_by(5) {
        let mut swapped = proof.to_vec();
        let (left, right) = swapped.split_at_mut((index + 1) * MESSAGE);
        left[index * MESSAGE..].swap_with_slice(&mut right[..MESSAGE]);
        if swapped != proof {
            push(
                format!("messages {index} and {} swapped", index + 1),
                Class::Ordinary,
                instances.clone(),
                swapped,
            );
        }
    }
    for (label, length) in [
        ("truncated by one byte", proof.len() - 1),
        ("truncated by one message", proof.len() - MESSAGE),
        ("truncated to half", proof.len() / 2),
        ("empty proof", 0),
    ] {
        push(
            label.to_owned(),
            Class::Ordinary,
            instances.clone(),
            proof[..length].to_vec(),
        );
    }
    let last = proof[proof.len() - MESSAGE..].to_vec();
    for (label, suffix) in [
        ("one trailing zero byte", vec![0_u8]),
        ("one trailing 0xff byte", vec![0xff]),
        ("one trailing zero message", vec![0; MESSAGE]),
        ("the last message repeated", last),
    ] {
        let mut extended = proof.to_vec();
        extended.extend_from_slice(&suffix);
        push(
            label.to_owned(),
            Class::TrailingBytes,
            instances.clone(),
            extended,
        );
    }
    let value = instances[0][0];
    let one = <B::VScalar as Field>::ONE;
    let zero = <B::VScalar as Field>::ZERO;
    let mut instance_cases = vec![
        ("instance value plus one", vec![vec![value + one]]),
        (
            "instance column with an appended one",
            vec![vec![value, one]],
        ),
        ("empty instance column", vec![Vec::new()]),
        ("two instance columns", vec![vec![value], vec![value]]),
        ("no instance column", Vec::new()),
    ];
    if value != zero {
        instance_cases.push(("instance value zeroed", vec![vec![zero]]));
        instance_cases.push(("instance value negated", vec![vec![-value]]));
    }
    for (label, tampered) in instance_cases {
        push(label.to_owned(), Class::Ordinary, tampered, proof.to_vec());
    }
    for padding in [1, 2] {
        let mut padded = vec![value];
        padded.extend(std::iter::repeat_n(zero, padding));
        push(
            format!("instance column padded with {padding} zero(s)"),
            Class::InstancePadding,
            vec![padded],
            proof.to_vec(),
        );
    }
    let mut spliced = other.to_vec();
    spliced[..MESSAGE].copy_from_slice(&proof[..MESSAGE]);
    if spliced != other {
        push(
            "seed-43 proof with the seed-42 first message".to_owned(),
            Class::Ordinary,
            instances.clone(),
            spliced,
        );
    }
    let mut rng = ChaCha20Rng::from_seed([0x7a; 32]);
    let below = |rng: &mut ChaCha20Rng, bound: usize| {
        usize::try_from(rng.next_u64() % u64::try_from(bound).expect("bound fits u64"))
            .expect("below a usize bound")
    };
    for round in 0..96 {
        let mut tampered = proof.to_vec();
        let bytes = if round < 64 {
            1
        } else {
            1 + below(&mut rng, 8)
        };
        for _ in 0..bytes {
            let offset = below(&mut rng, proof.len());
            let mask = u8::try_from(1 + below(&mut rng, 255)).expect("a nonzero byte");
            tampered[offset] ^= mask;
        }
        push(
            format!("seeded corruption {round} ({bytes} byte(s))"),
            Class::Ordinary,
            instances.clone(),
            tampered,
        );
    }
    for (label, bytes) in [("seed-42 golden", proof), ("seed-43 golden", other)] {
        inputs.push(Input {
            label: label.to_owned(),
            class: Class::Ordinary,
            instances: instances.clone(),
            proof: bytes.to_vec(),
            control: true,
        });
    }
    inputs
}

/// The native verdict under `pk` (a panic fails the test).
fn verify_native<B: CurveBridge>(
    setup: &Setup<B>,
    pk: &ProvingKey<B::Native>,
    instances: &[Vec<B::VScalar>],
    proof: &[u8],
) -> Result<(), VerifyError> {
    let native: Vec<Vec<_>> = instances.iter().map(|c| native_scalars::<B>(c)).collect();
    catch_unwind(AssertUnwindSafe(|| {
        verify_full_oracle(
            &setup.params,
            pk.binding(),
            pk.vk(),
            &native,
            proof,
            MemoryBudget::DEFAULT,
            setup.transcript_repr,
        )
    }))
    .unwrap_or_else(|_| panic!("the native verifier panicked"))
}

/// The name of an error variant (for the summary).
fn variant(error: &VerifyError) -> String {
    let text = format!("{error:?}");
    text.split(['(', ' ', '{'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Checks every verdict of a corpus: equal verdicts, or a registered
/// stricter native rejection; every id in `required` must occur.
fn check_corpus<F: Sync>(
    label: &str,
    inputs: &[Input<F>],
    vendored: impl Fn(&[Vec<F>], &[u8]) -> Result<(), String> + Sync,
    native: impl Fn(&[Vec<F>], &[u8]) -> Result<(), VerifyError> + Sync,
    required: &[&str],
) {
    assert!(
        inputs.len() >= MIN_CORPUS,
        "{label}: corpus of {} inputs",
        inputs.len()
    );
    let verdicts: Vec<_> = inputs
        .par_iter()
        .map(|input| {
            (
                vendored(&input.instances, &input.proof),
                native(&input.instances, &input.proof),
            )
        })
        .collect();

    let mut failures = Vec::new();
    let mut agreements = std::collections::BTreeMap::<String, usize>::new();
    let mut deviations = std::collections::BTreeMap::<&str, usize>::new();
    let mut accepted = 0;
    for (input, (vendored, native)) in inputs.iter().zip(&verdicts) {
        let context = format!("{label}: {}", input.label);
        if input.control && (vendored.is_err() || native.is_err()) {
            failures.push(format!(
                "{context}: control rejected (vendored {vendored:?}, native {native:?})"
            ));
            continue;
        }
        match (vendored, native) {
            (Ok(()), Ok(())) => accepted += 1,
            (Err(_), Err(error)) => *agreements.entry(variant(error)).or_default() += 1,
            (Err(error), Ok(())) => failures.push(format!(
                "{context}: the native verifier accepts what the vendored rejects ({error})"
            )),
            (Ok(()), Err(error)) => {
                match DEVIATIONS
                    .iter()
                    .find(|d| d.class == input.class && (d.native_rejection)(error))
                {
                    Some(deviation) => *deviations.entry(deviation.id).or_default() += 1,
                    None => failures.push(format!(
                        "{context}: unregistered stricter rejection {error:?}"
                    )),
                }
            }
        }
    }
    for id in required {
        if !deviations.contains_key(id) {
            failures.push(format!("{label}: registered deviation {id} never occurred"));
        }
    }
    println!(
        "VERDICTS {label}: {} inputs, {accepted} accepted by both, rejected by both {agreements:?}, \
         registered stricter rejections {deviations:?}",
        inputs.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Runs the Blake2b corpus of `family` over curve `B` (the golden path):
/// every registered deviation must occur.
fn verdict_parity<B: CurveBridge>(family: Family, k: u32) {
    let setup = setup::<B>(family, k);
    let cases = cases_for(family, B::NAME, k);
    let proof = prove_native(&setup, cases[0].seed_bytes());
    let other = prove_native(&setup, cases[1].seed_bytes());
    assert_eq!(digest_hex(&proof), cases[0].sha256, "{}", cases[0].name);
    assert_eq!(digest_hex(&other), cases[1].sha256, "{}", cases[1].name);
    let inputs = corpus(&setup, &proof, &other);
    let required: Vec<&str> = DEVIATIONS.iter().map(|d| d.id).collect();
    check_corpus(
        &format!("{}/{}/k{k}", family.label(), B::NAME),
        &inputs,
        |instances, proof| setup.verify_vendored(instances, proof),
        |instances, proof| verify_native(&setup, &setup.pk, instances, proof),
        &required,
    );
}

/// Runs the KAGEMUSHA-path corpus (Poseidon transcript, folded-generator
/// suffix) of `family` over curve `B`, against the vendored augmented
/// verifier. Only DEV-04 occurs: the augmented verifier already requires
/// the raw transcript to be consumed exactly, so trailing bytes (DEV-05)
/// are rejected on both sides.
fn kagemusha_verdict_parity<B: CurveBridge>(family: Family, k: u32) {
    let setup = setup::<B>(family, k);
    let keys = kagemusha_keys(&setup);
    let cases = cases_for(family, B::NAME, k);
    let proof = prove_native_kagemusha(&setup, &keys, cases[0].seed_bytes());
    let other = prove_native_kagemusha(&setup, &keys, cases[1].seed_bytes());
    assert_eq!(proof, setup.prove_vendored_kagemusha(cases[0].seed_bytes()));
    let inputs = corpus(&setup, &proof, &other);
    check_corpus(
        &format!("{}/{}/k{k} KAGEMUSHA", family.label(), B::NAME),
        &inputs,
        |instances, proof| setup.verify_vendored_kagemusha(instances, proof),
        |instances, proof| verify_native(&setup, &keys.pk, instances, proof),
        &["DEV-04"],
    );
}

#[test]
fn sigma_eq_tamper_verdicts_match() {
    verdict_parity::<Vesta>(Family::Sigma, 6);
}

#[test]
fn sigma_ep_tamper_verdicts_match() {
    verdict_parity::<Pallas>(Family::Sigma, 6);
}

#[test]
fn wide_eq_tamper_verdicts_match() {
    verdict_parity::<Vesta>(Family::Wide, 8);
}

#[test]
fn wide_ep_tamper_verdicts_match() {
    verdict_parity::<Pallas>(Family::Wide, 8);
}

#[test]
fn sigma_eq_kagemusha_tamper_verdicts_match() {
    kagemusha_verdict_parity::<Vesta>(Family::Sigma, 6);
}

#[test]
fn sigma_ep_kagemusha_tamper_verdicts_match() {
    kagemusha_verdict_parity::<Pallas>(Family::Sigma, 6);
}

#[test]
fn wide_eq_kagemusha_tamper_verdicts_match() {
    kagemusha_verdict_parity::<Vesta>(Family::Wide, 8);
}

#[test]
fn wide_ep_kagemusha_tamper_verdicts_match() {
    kagemusha_verdict_parity::<Pallas>(Family::Wide, 8);
}

#[test]
fn variant_names_the_error() {
    assert_eq!(
        variant(&VerifyError::DegenerateChallenge),
        "DegenerateChallenge"
    );
    assert_eq!(
        variant(&VerifyError::ProofLength {
            expected: 1,
            actual: 2
        }),
        "ProofLength"
    );
}
