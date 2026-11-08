//! Test-only first-execution diagnostics. No timing result confers proof or ledger authority.
use std::time::Instant;
#[path = "suffix_performance_cpu.rs"]
mod cpu;
use cpu::cpu_ns;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Usage {
    pub proof_calls: u32,
    pub proof_bytes: u64,
    pub max_calls_per_tx: u32,
    pub max_calls_per_block: u32,
    pub max_proof_bytes_block: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    label: &'static str,
    height: u64,
    ordinal: u128,
    steps: usize,
    cpu: Option<u128>,
    wall: Instant,
}
impl Sample {
    pub(super) fn start(label: &'static str, height: u64, ordinal: u128, steps: usize) -> Self {
        Self {
            label,
            height,
            ordinal,
            steps,
            cpu: cpu_ns(),
            wall: Instant::now(),
        }
    }
    pub(super) fn finish(self, success: bool, usage: &Usage) {
        let wall_ns = self.wall.elapsed().as_nanos();
        let cpu = self
            .cpu
            .zip(cpu_ns())
            .map(|(before, after)| after.saturating_sub(before));
        let cpu = cpu.map_or_else(|| "unavailable".to_owned(), |ns| ns.to_string());
        eprintln!(
            "SUFFIX_PERF label={} height={} ordinal={} steps={} success={} wall_ns={} cpu_ns={} proof_calls={} proof_bytes={} max_calls_per_tx={} max_calls_per_block={} max_proof_bytes_block={} mount_scope=production_dispatch",
            self.label,
            self.height,
            self.ordinal,
            self.steps,
            success,
            wall_ns,
            cpu,
            usage.proof_calls,
            usage.proof_bytes,
            usage.max_calls_per_tx,
            usage.max_calls_per_block,
            usage.max_proof_bytes_block,
        );
    }
}
