//! Retired instructions, peak memory and wall/CPU time of native suffix verification over 50
//! distinct genuine Receives, in ledger mode and in Native mode (with the strict circuit
//! predicate assignments). Each mode uses a freshly loaded verifier pack, so the parameter
//! owner's in-process memo of accepted proofs never answers a measured verification.
//! Run this test alone: the instruction counter is process wide.
use super::*;

const MEASURED: usize = 50;

#[repr(C)]
struct RusageInfoV4 {
    uuid: [u8; 16],
    fields: [u64; 35],
}

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RusageInfoV4) -> i32;
    fn getpid() -> i32;
}

/// (retired instructions, lifetime peak physical footprint bytes, user+system CPU ns).
fn rusage() -> (u64, u64, u64) {
    #[cfg(target_vendor = "apple")]
    {
        let mut info = RusageInfoV4 {
            uuid: [0; 16],
            fields: [0; 35],
        };
        // SAFETY: `info` is a writable `rusage_info_v4` (flavor 4) of this process.
        if unsafe { proc_pid_rusage(getpid(), 4, &mut info) } == 0 {
            // ri_user_time and ri_system_time are mach absolute-time units (1 ns on arm64).
            return (
                info.fields[29],
                info.fields[28],
                info.fields[0] + info.fields[1],
            );
        }
    }
    (0, 0, 0)
}

struct Interval {
    label: &'static str,
    instructions: u64,
    peak: u64,
    cpu_ns: u64,
    wall_ms: u128,
}

fn measured(label: &'static str, run: impl FnOnce()) -> Interval {
    let (i0, _, c0) = rusage();
    let started = Instant::now();
    run();
    let wall_ms = started.elapsed().as_millis();
    let (i1, peak, c1) = rusage();
    Interval {
        label,
        instructions: i1 - i0,
        peak,
        cpu_ns: c1 - c0,
        wall_ms,
    }
}

fn fresh_pack(corpus: &Corpus) -> InstalledVerifierPackV1 {
    let catalog = corpus.root.join("catalog");
    let installation_original = bounded(&catalog.join("engineering-installation.norito"), 1024);
    let (scheme_id, manifest_digest): ([u8; 32], [u8; 32]) =
        decode(&installation_original).unwrap();
    InstalledVerifierPackV1::load(
        &bounded(
            &catalog.join("engineering-verifier-pack.norito"),
            VERIFIER_PACK_MAX_BYTES_V1,
        ),
        InstallationV1 {
            scheme_id,
            manifest_digest,
        },
    )
    .unwrap()
}

fn verify_all(verifier: &SuffixVerifierV1<'_>, corpus: &Corpus, steps: &[StepOriginals]) {
    let count = steps.len();
    let certificates = corpus.certificates();
    let anchor = verifier
        .verify_anchor(corpus.anchor_input(&certificates), None)
        .unwrap();
    let mut cursor = verifier.cursor_at_anchor(&anchor).unwrap();
    for step in steps {
        cursor.verify_step(verifier, step.input(), None).unwrap();
    }
    cursor.settle(verifier, None).unwrap();
    let tip = cursor.into_tip().unwrap();
    assert_eq!(tip.steps(), (count as u32, count as u32, 0));
    assert_eq!(
        tip.point().0,
        corpus.state(count as u128),
        "the verified tip is the genuine state"
    );
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a printed estimate; every reported count is far below 2^52"
)]
fn report(interval: &Interval, count: usize) {
    // Projection conventions, not device measurements: the cost model's equal-throughput
    // rate implied by the owner-supplied 45-65 s per k16 stage over the historical 4.625 T
    // instructions per Receive head (231,250 G for 50 heads in 27,000-39,000 s).
    let low = 231_250.0 / 39_000.0;
    let high = 231_250.0 / 27_000.0;
    let giga = interval.instructions as f64 / 1e9;
    eprintln!(
        "SUFFIX_MEASURE mode={} receives={count} retired_instructions={} per_receive_g={:.3} peak_footprint_mib={:.1} cpu_s={:.3} wall_ms={} projected_g99_s_equal_throughput={:.1}-{:.1} projected_g99_s_at_2gips_one_core={:.1}",
        interval.label,
        interval.instructions,
        giga / count as f64,
        interval.peak as f64 / f64::from(1 << 20),
        interval.cpu_ns as f64 / 1e9,
        interval.wall_ms,
        giga / high,
        giga / low,
        giga / 2.0,
    );
}

pub(super) fn run(corpus: &Corpus) {
    let budget = MemoryBudget::new(128 << 20);
    let steps = corpus.steps(MEASURED as u128);
    let payers: BTreeSet<_> = steps
        .iter()
        .map(|step| step.payer_credential.clone())
        .collect();
    assert_eq!(payers.len(), MEASURED, "50 distinct genuine payers");
    let ledger_pack = fresh_pack(corpus);
    let ledger = SuffixVerifierV1::ledger(ledger_pack.verifier(), budget);
    let interval = measured("ledger", || verify_all(&ledger, corpus, &steps));
    report(&interval, MEASURED);
    let batched_pack = fresh_pack(corpus);
    let batched = SuffixVerifierV1::ledger(batched_pack.verifier(), budget).batched();
    let interval = measured("ledger_batched", || verify_all(&batched, corpus, &steps));
    report(&interval, MEASURED);
    // Native mode adds three strict k16 circuit assignments per Receive; the optional count
    // (default 50) bounds that run on a loaded host.
    if let Some(count) = std::env::var_os("KAGEMUSHA_SUFFIX_MEASURE_NATIVE") {
        let count = count
            .to_str()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=MEASURED).contains(value))
            .unwrap_or(MEASURED);
        let (sources, _originals) = super::continuation::qualify(corpus);
        let native_pack = fresh_pack(corpus);
        let native = SuffixVerifierV1::native(&native_pack, &sources, budget)
            .unwrap()
            .batched();
        let interval = measured("native_strict_predicates_batched", || {
            verify_all(&native, corpus, &steps[..count]);
        });
        report(&interval, count);
    }
}
