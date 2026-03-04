//! Analysis helpers for Kotodama contracts.
//!
//! This module exposes higher-level analysis passes that extend the basic
//! syntactic linting performed by `crate::lint`. Static checks operate on
//! the parsed AST and typed program, whereas bytecode checks inspect compiled
//! `.to` artifacts. Fuzz harnesses execute simplified interpreters or VM runs
//! to exercise contracts and surface potential runtime failures.

use std::fmt;

pub mod bytecode;
pub mod fuzz;
pub mod source;

/// Severity level associated with an [`AnalysisFinding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisSeverity {
    Warning,
    Info,
}

impl fmt::Display for AnalysisSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisSeverity::Warning => write!(f, "warning"),
            AnalysisSeverity::Info => write!(f, "info"),
        }
    }
}

/// Category of analysis that produced a [`AnalysisFinding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisCategory {
    StaticSource,
    BytecodeStatic,
    Fuzz,
    BytecodeFuzz,
}

impl fmt::Display for AnalysisCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            AnalysisCategory::StaticSource => "static",
            AnalysisCategory::BytecodeStatic => "bytecode",
            AnalysisCategory::Fuzz => "fuzz",
            AnalysisCategory::BytecodeFuzz => "bytecode-fuzz",
        };
        write!(f, "{label}")
    }
}

/// Single finding produced by one of the analysis passes.
#[derive(Debug, Clone)]
pub struct AnalysisFinding {
    pub code: &'static str,
    pub severity: AnalysisSeverity,
    pub category: AnalysisCategory,
    pub message: String,
}

impl AnalysisFinding {
    pub fn warning(
        category: AnalysisCategory,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            code,
            severity: AnalysisSeverity::Warning,
            message: message.into(),
        }
    }

    pub fn info(
        category: AnalysisCategory,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            code,
            severity: AnalysisSeverity::Info,
            message: message.into(),
        }
    }
}

/// Deterministic pseudo-random number generator used by fuzz harnesses to avoid
/// introducing external dependencies.
#[derive(Clone)]
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    /// Create a new RNG seeded with `seed`. Zero seeds are allowed but will be
    /// tweaked to avoid the all-zero state.
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state }
    }

    /// Next 64-bit word using xorshift64*.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Next 32-bit word.
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() & 0xFFFF_FFFF) as u32
    }

    /// Next boolean.
    pub fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }
}
