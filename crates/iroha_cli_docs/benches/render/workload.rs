//! Shared benchmark workload; external comparison harnesses use the same tree and sampling.

use std::{hint::black_box, time::Instant};

use clap::{Arg, ArgAction, Command};

/// One renderer taking the same borrowed fixture and making its own fresh input clone.
#[derive(Clone, Copy)]
pub(crate) struct Renderer {
    /// Label printed with measurements.
    pub(crate) name: &'static str,
    /// Return documentation for a fresh clone of the supplied tree.
    pub(crate) render: fn(&Command) -> String,
}

fn name(value: String) -> &'static str {
    // Clap's default feature set uses static names. These small fixed fixture names live for
    // this process and are created once, before timing starts.
    Box::leak(value.into_boxed_str())
}

/// Construct the deterministic fixture used by timing and external allocation comparisons.
pub(crate) fn tree(leaves: usize) -> Command {
    let mut root = Command::new("operator").about("Inspect and manage local service state.");
    for group in 0..10 {
        let mut branch =
            Command::new(name(format!("group{group}"))).about("A family of service operations.");
        for leaf in 0..leaves / 10 {
            branch = branch.subcommand(
                Command::new(name(format!("operation{leaf}")))
                    .about("Validate the configured target and report its state.")
                    .arg(Arg::new("target").required(true).help("Target identity"))
                    .arg(
                        Arg::new("output").short('o').long("output").default_value("json")
                            .value_parser(["json", "text"]).help("Output format"),
                    )
                    .arg(
                        Arg::new("timeout").long("timeout").default_value("30s")
                            .long_help("Maximum operation time.\n\nA timeout returns a diagnostic without changing state."),
                    )
                    .arg(
                        Arg::new("endpoint").short('e').long("endpoint").visible_alias("url")
                            .help("Service endpoint"),
                    )
                    .arg(
                        Arg::new("verbose").short('v').long("verbose").action(ArgAction::SetTrue)
                            .help("Include detailed diagnostics"),
                    )
                    .arg(Arg::new("labels").long("labels").num_args(1..).help("Labels to select")),
            );
        }
        root = root.subcommand(branch);
    }
    root
}

fn elapsed(tree: &Command, render: fn(&Command) -> String, iterations: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(render(black_box(tree)));
    }
    start.elapsed().as_secs_f64() * 1_000_000.0 / f64::from(iterations)
}

fn benchmark_mode(arguments: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    let mut benchmark = false;
    for argument in arguments {
        match argument.as_ref() {
            "--test" => return false,
            "--bench" => benchmark = true,
            _ => {}
        }
    }
    benchmark
}

/// Measure both fixture sizes, verifying parity when supplied more than one renderer.
///
/// Each renderer must clone its input. Two-renderer comparisons alternate execution order
/// between batches. Each invocation includes the clone and returned `String` allocation.
pub(crate) fn run(renderers: &[Renderer]) {
    assert!(!renderers.is_empty(), "at least one renderer is required");
    // Cargo's all-target test runner can invoke harness-free benchmarks without arguments.
    // Only an explicit --bench starts the full workload; --test always keeps the smoke mode.
    let smoke = !benchmark_mode(std::env::args().skip(1));
    let batches = if smoke { 1 } else { 9 };
    for leaves in [100, 1000] {
        let tree = tree(leaves);
        let expected = (renderers[0].render)(&tree);
        for renderer in &renderers[1..] {
            assert_eq!(
                (renderer.render)(&tree),
                expected,
                "{} produced different documentation",
                renderer.name,
            );
        }
        let iterations = if smoke {
            1
        } else if leaves == 100 {
            20
        } else {
            3
        };
        let mut samples = vec![Vec::with_capacity(batches); renderers.len()];
        for batch in 0..batches {
            for slot in 0..renderers.len() {
                let index = if batch % 2 == 0 {
                    slot
                } else {
                    renderers.len() - slot - 1
                };
                samples[index].push(elapsed(&tree, renderers[index].render, iterations));
            }
        }
        println!(
            "{leaves} leaves, {} output bytes, {batches} batches of {iterations} renders",
            expected.len()
        );
        for (renderer, samples) in renderers.iter().zip(&mut samples) {
            samples.sort_by(f64::total_cmp);
            println!(
                "{}: median {:.1} us; samples_us={samples:?}",
                renderer.name,
                samples[samples.len() / 2],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn full_sampling_requires_explicit_bench_and_test_always_wins() {
        assert!(!super::benchmark_mode(std::iter::empty::<&str>()));
        assert!(!super::benchmark_mode(["--test"]));
        assert!(!super::benchmark_mode(["--unknown"]));
        assert!(super::benchmark_mode(["--bench"]));
        assert!(!super::benchmark_mode(["--bench", "--test"]));
        assert!(!super::benchmark_mode(["--test", "--bench"]));
    }
}
