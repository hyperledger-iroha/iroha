//! The one public-only record schema shared by native and SDK emitters.
//!
//! A [`MeasurementRecord`] is Norito-encoded for retention and has a Norito
//! JSON view for scripts and SDK emitters in other languages. Both forms carry
//! exactly the same fields. Every field is a label, an identity string, an
//! enumerated word, a boolean or an unsigned integer: there is no byte-string
//! field, and every text field is bounded by a closed grammar that the report
//! checks. That prevents attaching witness bytes, key material or a hidden
//! program by accident. It is not a sandbox: the fields are public, so code
//! that deliberately writes a secret as text into a hand-built record is not
//! stopped here, only reported by [`MeasurementRecord::findings`] when the
//! text leaves the grammar.
//!
//! Every quantitative value sits in a section whose `classification` states
//! whether it was measured, is a node-local scheduling parameter, or was
//! declared by the caller as a projection, an engineering target or a
//! deterministic consensus bound. Structural numbers (phase indices and the
//! process identifier) identify things and carry no classification.

use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    json::{self, Map, Value},
};

use crate::text;

/// Schema identity written into every record.
pub const RECORD_SCHEMA_V1: &str = "iroha.measurement.record.v1";
/// Schema identity written into every harness hand-off context.
pub const CONTEXT_SCHEMA_V1: &str = "iroha.measurement.context.v1";
/// Environment variable through which `scripts/zk_resource_harness.py` hands
/// its output directory to a test or diagnostic adapter.
///
/// This crate never reads the environment. Only test and diagnostic adapters
/// read this variable and pass the directory to an explicit sink; it selects
/// where a diagnostic record is written and nothing else.
pub const HARNESS_OUTPUT_DIR_ENV: &str = "IROHA_MEASUREMENT_OUTPUT_DIR";
/// File name of the hand-off context inside the harness output directory.
pub const HARNESS_CONTEXT_FILE: &str = "context.json";

macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$variant_meta:meta])* $variant:ident => $text:literal, )+ }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, NoritoSerialize, NoritoDeserialize)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// Every variant in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant, )+];

            /// Stable spelling used by the JSON view.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }

            /// Parse the stable JSON spelling.
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $( $text => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }
    };
}

text_enum! {
    /// What kind of statement a reported number makes.
    Classification {
        /// Extrapolated or modelled, not observed.
        Projection => "projection",
        /// A performance goal chosen by engineers; it permits nothing.
        EngineeringTarget => "engineering_target",
        /// A limit every validator enforces identically from protocol State.
        DeterministicConsensusBound => "deterministic_consensus_bound",
        /// A node-local limit or parameter that cannot change validity.
        LocalScheduling => "local_scheduling",
        /// Observed on the named hardware during this run.
        Measured => "measured",
    }
}

text_enum! {
    /// The flow whose root phase the tree describes.
    FlowKind {
        /// Proof construction, including any mandatory self-check.
        Proof => "proof",
        /// Independent verification of public bytes.
        Verification => "verification",
        /// Another native operation such as encryption or state commitment.
        Native => "native",
        /// A flow driven through an SDK.
        Sdk => "sdk",
    }
}

text_enum! {
    /// Whether caches were populated before the measured root started.
    CachePolicy {
        /// No reusable cache of this workload existed when the root started.
        Cold => "cold",
        /// Caches from an earlier identical run were present.
        Warm => "warm",
    }
}

text_enum! {
    /// Device thermal pressure as far as the platform exposes it.
    ThermalState {
        /// No thermal pressure.
        Nominal => "nominal",
        /// Moderate pressure; performance may begin to drop.
        Fair => "fair",
        /// Heavy pressure; performance is being reduced.
        Serious => "serious",
        /// The device is throttling hard or about to stop.
        Critical => "critical",
        /// The platform exposes no thermal state to this process.
        Unavailable => "unavailable",
    }
}

text_enum! {
    /// How the measured root ended.
    RunOutcome {
        /// The session was finished and no failure was recorded.
        Succeeded => "succeeded",
        /// The session was finished after at least one recorded failure.
        Failed => "failed",
        /// The session was dropped without being finished (an error return).
        Abandoned => "abandoned",
        /// The session was dropped while its thread was unwinding from a panic.
        Unwound => "unwound",
    }
}

text_enum! {
    /// What a named byte counter measures.
    ByteKind {
        /// Encrypted payload bytes.
        Ciphertext => "ciphertext",
        /// Public or evaluation key bytes.
        Key => "key",
        /// Encoded proof bytes.
        Proof => "proof",
        /// Encoded transaction bytes.
        Transaction => "transaction",
        /// Encoded public statement or input bytes.
        PublicInput => "public_input",
        /// Another public encoded size.
        Other => "other",
    }
}

text_enum! {
    /// Unit of a caller-declared quantity.
    Unit {
        /// Nanoseconds.
        Nanoseconds => "nanoseconds",
        /// Milliseconds.
        Milliseconds => "milliseconds",
        /// Seconds.
        Seconds => "seconds",
        /// Bytes.
        Bytes => "bytes",
        /// A dimensionless count.
        Count => "count",
    }
}

text_enum! {
    /// Where an allocation observation comes from.
    AllocationSourceKind {
        /// A scoped counter driven by explicit calls in instrumented code.
        ScopedCounter => "scoped_counter",
        /// An existing `iroha_allocation::AllocationBudget` read at boundaries.
        AllocationBudget => "allocation_budget",
    }
}

text_enum! {
    /// How the peak resident set size was obtained.
    PeakRssSource {
        /// The kernel's lifetime high-water mark for this process.
        KernelLifetimeHighWater => "kernel_lifetime_high_water",
        /// The platform exposes no peak to this process.
        Unavailable => "unavailable",
    }
}

text_enum! {
    /// What establishes a caller-declared quantity.
    ProvenanceKind {
        /// A constant fixed by the protocol source.
        ProtocolConstant => "protocol_constant",
        /// A value read from committed protocol State.
        CommittedState => "committed_state",
        /// A node-local configuration value.
        LocalConfiguration => "local_configuration",
        /// A limit the operating system enforces on this process.
        OperatingSystem => "operating_system",
        /// A target stated by the delivery plan or a specification.
        PlanTarget => "plan_target",
        /// A model or extrapolation.
        ModelProjection => "model_projection",
    }
}

/// A record or hand-off context could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// The JSON text does not parse.
    Json(String),
    /// The Norito bytes do not decode as this schema.
    Norito(String),
    /// A value has the wrong JSON type or range.
    Shape {
        /// JSON path of the offending value.
        path: String,
        /// The type the schema requires there.
        expected: &'static str,
    },
    /// A required key is absent.
    MissingKey {
        /// JSON path of the object.
        path: String,
        /// The absent key.
        key: &'static str,
    },
    /// A key outside the schema is present.
    UnknownKey {
        /// JSON path of the object.
        path: String,
        /// The unexpected key.
        key: String,
    },
    /// An enumerated word is not one of the schema's spellings.
    UnknownText {
        /// JSON path of the value.
        path: String,
        /// The unexpected word.
        text: String,
    },
}

impl core::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json(reason) => write!(f, "measurement JSON does not parse: {reason}"),
            Self::Norito(reason) => write!(f, "measurement Norito bytes do not decode: {reason}"),
            Self::Shape { path, expected } => write!(f, "{path}: expected {expected}"),
            Self::MissingKey { path, key } => write!(f, "{path}: missing key {key}"),
            Self::UnknownKey { path, key } => write!(f, "{path}: unknown key {key}"),
            Self::UnknownText { path, text } => write!(f, "{path}: unknown word {text}"),
        }
    }
}

impl std::error::Error for SchemaError {}

/// Reader over one JSON object with an exact key set.
struct Reader<'a> {
    map: &'a Map,
    path: &'a str,
}

impl<'a> Reader<'a> {
    fn open(
        value: &'a Value,
        path: &'a str,
        keys: &'static [&'static str],
    ) -> Result<Self, SchemaError> {
        let Value::Object(map) = value else {
            return Err(SchemaError::Shape {
                path: path.to_owned(),
                expected: "object",
            });
        };
        if let Some(key) = map.keys().find(|key| !keys.contains(&key.as_str())) {
            return Err(SchemaError::UnknownKey {
                path: path.to_owned(),
                key: key.clone(),
            });
        }
        if let Some(key) = keys.iter().find(|key| !map.contains_key(**key)) {
            return Err(SchemaError::MissingKey {
                path: path.to_owned(),
                key,
            });
        }
        Ok(Self { map, path })
    }

    fn at(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }

    fn value(&self, key: &'static str) -> Result<&'a Value, SchemaError> {
        self.map.get(key).ok_or_else(|| SchemaError::MissingKey {
            path: self.path.to_owned(),
            key,
        })
    }

    fn u64(&self, key: &'static str) -> Result<u64, SchemaError> {
        unsigned(self.value(key)?, &self.at(key))
    }

    fn u32(&self, key: &'static str) -> Result<u32, SchemaError> {
        u32::try_from(self.u64(key)?).map_err(|_| SchemaError::Shape {
            path: self.at(key),
            expected: "unsigned 32-bit integer",
        })
    }

    fn optional_u64(&self, key: &'static str) -> Result<Option<u64>, SchemaError> {
        match self.value(key)? {
            Value::Null => Ok(None),
            value => unsigned(value, &self.at(key)).map(Some),
        }
    }

    fn optional_u32(&self, key: &'static str) -> Result<Option<u32>, SchemaError> {
        self.optional_u64(key)?
            .map(|value| {
                u32::try_from(value).map_err(|_| SchemaError::Shape {
                    path: self.at(key),
                    expected: "unsigned 32-bit integer or null",
                })
            })
            .transpose()
    }

    fn text(&self, key: &'static str) -> Result<String, SchemaError> {
        match self.value(key)? {
            Value::String(text) => Ok(text.clone()),
            _ => Err(SchemaError::Shape {
                path: self.at(key),
                expected: "string",
            }),
        }
    }

    fn optional_text(&self, key: &'static str) -> Result<Option<String>, SchemaError> {
        match self.value(key)? {
            Value::Null => Ok(None),
            Value::String(text) => Ok(Some(text.clone())),
            _ => Err(SchemaError::Shape {
                path: self.at(key),
                expected: "string or null",
            }),
        }
    }

    fn boolean(&self, key: &'static str) -> Result<bool, SchemaError> {
        match self.value(key)? {
            Value::Bool(value) => Ok(*value),
            _ => Err(SchemaError::Shape {
                path: self.at(key),
                expected: "boolean",
            }),
        }
    }

    fn word<T>(
        &self,
        key: &'static str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Result<T, SchemaError> {
        let text = self.text(key)?;
        let parsed = parse(&text);
        parsed.ok_or_else(|| SchemaError::UnknownText {
            path: self.at(key),
            text,
        })
    }

    fn list<T>(
        &self,
        key: &'static str,
        item: impl Fn(&Value, &str) -> Result<T, SchemaError>,
    ) -> Result<Vec<T>, SchemaError> {
        let Value::Array(values) = self.value(key)? else {
            return Err(SchemaError::Shape {
                path: self.at(key),
                expected: "array",
            });
        };
        values
            .iter()
            .enumerate()
            .map(|(index, value)| item(value, &format!("{}.{key}[{index}]", self.path)))
            .collect()
    }
}

/// Decode an exact unsigned integer; floats and negative numbers are rejected.
fn unsigned(value: &Value, path: &str) -> Result<u64, SchemaError> {
    match value {
        Value::Number(json::Number::U64(number)) => Ok(*number),
        Value::Number(json::Number::I64(number)) => {
            u64::try_from(*number).map_err(|_| SchemaError::Shape {
                path: path.to_owned(),
                expected: "unsigned 64-bit integer",
            })
        }
        _ => Err(SchemaError::Shape {
            path: path.to_owned(),
            expected: "unsigned 64-bit integer",
        }),
    }
}

fn object<const N: usize>(pairs: [(&'static str, Value); N]) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn optional_number(value: Option<u64>) -> Value {
    value.map_or(Value::Null, Value::from)
}

fn optional_string(value: Option<&String>) -> Value {
    value.map_or(Value::Null, |text| Value::String(text.clone()))
}

fn word(text: &'static str) -> Value {
    Value::String(text.to_owned())
}

/// Identity facts the harness knows before the workload starts.
///
/// `scripts/zk_resource_harness.py` writes this as `context.json` in its
/// output directory; a test or diagnostic adapter reads it and combines it
/// with its own workload label, flow and emitter into a [`RunIdentity`].
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RunContext {
    /// Full Git object name of the source commit.
    pub source_commit: String,
    /// Whether the working tree differed from that commit.
    pub source_dirty: bool,
    /// SHA-256 of the working-tree difference when dirty, otherwise `None`.
    pub source_dirty_digest: Option<String>,
    /// Identity of the measured artifact, for example its SHA-256.
    pub artifact: String,
    /// Identity of the semantic and build profile.
    pub profile: String,
    /// Identity of the configuration in force.
    pub config: String,
    /// Name of the reference hardware record, asserted by the operator. The
    /// harness report carries the observed host facts beside it.
    pub hardware: String,
    /// Cache policy asserted by the operator who started the run. Neither the
    /// harness nor this crate establishes or verifies it.
    pub cache_policy: CachePolicy,
}

const CONTEXT_KEYS: &[&str] = &[
    "artifact",
    "cache_policy",
    "config",
    "hardware",
    "profile",
    "schema",
    "source_commit",
    "source_dirty",
    "source_dirty_digest",
];

impl RunContext {
    /// A context for a local run that no harness bound to source or hardware.
    ///
    /// Records built from it are retained but fail identity completeness.
    pub fn unbound() -> Self {
        Self {
            source_commit: text::UNBOUND.to_owned(),
            source_dirty: true,
            source_dirty_digest: None,
            artifact: text::UNBOUND.to_owned(),
            profile: text::UNBOUND.to_owned(),
            config: text::UNBOUND.to_owned(),
            hardware: text::UNBOUND.to_owned(),
            cache_policy: CachePolicy::Warm,
        }
    }

    /// JSON view written by the harness.
    pub fn to_json_value(&self) -> Value {
        object([
            ("artifact", Value::String(self.artifact.clone())),
            ("cache_policy", word(self.cache_policy.as_str())),
            ("config", Value::String(self.config.clone())),
            ("hardware", Value::String(self.hardware.clone())),
            ("profile", Value::String(self.profile.clone())),
            ("schema", word(CONTEXT_SCHEMA_V1)),
            ("source_commit", Value::String(self.source_commit.clone())),
            ("source_dirty", Value::Bool(self.source_dirty)),
            (
                "source_dirty_digest",
                optional_string(self.source_dirty_digest.as_ref()),
            ),
        ])
    }

    /// Decode the JSON view written by the harness.
    ///
    /// # Errors
    /// Returns the first shape, key or word that differs from the schema.
    pub fn from_json_value(value: &Value) -> Result<Self, SchemaError> {
        let reader = Reader::open(value, "context", CONTEXT_KEYS)?;
        let schema = reader.text("schema")?;
        if schema != CONTEXT_SCHEMA_V1 {
            return Err(SchemaError::UnknownText {
                path: reader.at("schema"),
                text: schema,
            });
        }
        Ok(Self {
            source_commit: reader.text("source_commit")?,
            source_dirty: reader.boolean("source_dirty")?,
            source_dirty_digest: reader.optional_text("source_dirty_digest")?,
            artifact: reader.text("artifact")?,
            profile: reader.text("profile")?,
            config: reader.text("config")?,
            hardware: reader.text("hardware")?,
            cache_policy: reader.word("cache_policy", CachePolicy::parse)?,
        })
    }

    /// Decode JSON text written by the harness.
    ///
    /// # Errors
    /// Returns a parse error or the first difference from the schema.
    pub fn from_json_view(text: &str) -> Result<Self, SchemaError> {
        let value =
            json::parse_value(text).map_err(|error| SchemaError::Json(error.to_string()))?;
        Self::from_json_value(&value)
    }

    /// Compact canonical JSON text of this context.
    pub fn to_json_view(&self) -> String {
        render(&self.to_json_value())
    }
}

/// Exact identity of one measured run.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RunIdentity {
    /// Label of the measured workload and shape.
    pub workload: String,
    /// Which flow the root phase covers.
    pub flow: FlowKind,
    /// Label of the adapter that emitted the record.
    pub emitter: String,
    /// Source, artifact, profile, configuration, hardware and cache identity.
    pub context: RunContext,
}

const IDENTITY_KEYS: &[&str] = &[
    "artifact",
    "cache_policy",
    "config",
    "emitter",
    "flow",
    "hardware",
    "profile",
    "source_commit",
    "source_dirty",
    "source_dirty_digest",
    "workload",
];

impl RunIdentity {
    /// Combine a harness context with the adapter's workload, flow and emitter.
    pub fn new(
        context: RunContext,
        workload: &'static str,
        flow: FlowKind,
        emitter: &'static str,
    ) -> Self {
        Self {
            workload: text::public_or_invalid(workload).0.to_owned(),
            flow,
            emitter: text::public_or_invalid(emitter).0.to_owned(),
            context,
        }
    }

    fn to_json_value(&self) -> Value {
        object([
            ("artifact", Value::String(self.context.artifact.clone())),
            ("cache_policy", word(self.context.cache_policy.as_str())),
            ("config", Value::String(self.context.config.clone())),
            ("emitter", Value::String(self.emitter.clone())),
            ("flow", word(self.flow.as_str())),
            ("hardware", Value::String(self.context.hardware.clone())),
            ("profile", Value::String(self.context.profile.clone())),
            (
                "source_commit",
                Value::String(self.context.source_commit.clone()),
            ),
            ("source_dirty", Value::Bool(self.context.source_dirty)),
            (
                "source_dirty_digest",
                optional_string(self.context.source_dirty_digest.as_ref()),
            ),
            ("workload", Value::String(self.workload.clone())),
        ])
    }

    fn from_json_value(value: &Value, path: &str) -> Result<Self, SchemaError> {
        let reader = Reader::open(value, path, IDENTITY_KEYS)?;
        Ok(Self {
            workload: reader.text("workload")?,
            flow: reader.word("flow", FlowKind::parse)?,
            emitter: reader.text("emitter")?,
            context: RunContext {
                source_commit: reader.text("source_commit")?,
                source_dirty: reader.boolean("source_dirty")?,
                source_dirty_digest: reader.optional_text("source_dirty_digest")?,
                artifact: reader.text("artifact")?,
                profile: reader.text("profile")?,
                config: reader.text("config")?,
                hardware: reader.text("hardware")?,
                cache_policy: reader.word("cache_policy", CachePolicy::parse)?,
            },
        })
    }
}

/// One aggregated phase: every call of `label` under the same parent phase.
///
/// Wall time is taken from one monotonic clock sampled in event order.
/// `wall_inclusive_ns` is the sum of this phase's call durations.
/// `wall_exclusive_ns` is that sum minus, for each call, the exact measure of
/// the union of its direct children's intervals, so parallel children are
/// never subtracted twice and the value is never negative.
///
/// `thread_cpu_*` is the CPU clock of the thread that opened the call;
/// exclusive subtracts only children opened on that same thread.
/// `process_cpu_window_*` is the process CPU clock over the same windows as
/// the wall accounting. It is process-window accounting: work of unrelated
/// threads in the window is included, so it is not CPU uniquely attributable
/// to the phase.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PhaseNode {
    /// Index of the parent phase; `None` only for the root at index zero.
    pub parent: Option<u32>,
    /// Static public label of the phase.
    pub label: String,
    /// Number of calls opened.
    pub calls: u64,
    /// Calls explicitly completed.
    pub completed: u64,
    /// Calls closed without completion (error return, unwind or truncation).
    pub interrupted: u64,
    /// Interrupted calls closed while their thread was unwinding.
    pub unwound: u64,
    /// Interrupted calls closed by an enclosing phase or the session ending.
    pub truncated: u64,
    /// Sum of call durations.
    pub wall_inclusive_ns: u64,
    /// Inclusive wall time not covered by any direct child.
    pub wall_exclusive_ns: u64,
    /// Opening-thread CPU time over the calls.
    pub thread_cpu_inclusive_ns: u64,
    /// Opening-thread CPU time not spent in same-thread children.
    pub thread_cpu_exclusive_ns: u64,
    /// Process CPU clock advance over the calls.
    pub process_cpu_window_inclusive_ns: u64,
    /// Process CPU clock advance while no direct child was open.
    pub process_cpu_window_exclusive_ns: u64,
    /// Most direct children open at once during any call.
    pub peak_concurrent_children: u32,
    /// Most distinct threads with an open phase, sampled at this phase's boundaries.
    pub active_threads_max: u32,
    /// One-minute system load times 1000 when the phase was first entered.
    pub load_milli_first_enter: u64,
    /// One-minute system load times 1000 when the phase last exited.
    pub load_milli_last_exit: u64,
    /// Highest one-minute system load times 1000 seen at its boundaries.
    pub load_milli_max: u64,
    /// Allocations reported while this phase was the innermost open phase.
    pub allocations: u64,
    /// Bytes of those allocations.
    pub allocated_bytes: u64,
    /// Highest live-buffer total observed at events inside this phase's subtree.
    pub live_bytes_high_water: u64,
}

const PHASE_NODE_KEYS: &[&str] = &[
    "active_threads_max",
    "allocated_bytes",
    "allocations",
    "calls",
    "completed",
    "interrupted",
    "label",
    "live_bytes_high_water",
    "load_milli_first_enter",
    "load_milli_last_exit",
    "load_milli_max",
    "parent",
    "peak_concurrent_children",
    "process_cpu_window_exclusive_ns",
    "process_cpu_window_inclusive_ns",
    "thread_cpu_exclusive_ns",
    "thread_cpu_inclusive_ns",
    "truncated",
    "unwound",
    "wall_exclusive_ns",
    "wall_inclusive_ns",
];

impl PhaseNode {
    fn to_json_value(&self) -> Value {
        object([
            (
                "active_threads_max",
                Value::from(u64::from(self.active_threads_max)),
            ),
            ("allocated_bytes", Value::from(self.allocated_bytes)),
            ("allocations", Value::from(self.allocations)),
            ("calls", Value::from(self.calls)),
            ("completed", Value::from(self.completed)),
            ("interrupted", Value::from(self.interrupted)),
            ("label", Value::String(self.label.clone())),
            (
                "live_bytes_high_water",
                Value::from(self.live_bytes_high_water),
            ),
            (
                "load_milli_first_enter",
                Value::from(self.load_milli_first_enter),
            ),
            (
                "load_milli_last_exit",
                Value::from(self.load_milli_last_exit),
            ),
            ("load_milli_max", Value::from(self.load_milli_max)),
            ("parent", optional_number(self.parent.map(u64::from))),
            (
                "peak_concurrent_children",
                Value::from(u64::from(self.peak_concurrent_children)),
            ),
            (
                "process_cpu_window_exclusive_ns",
                Value::from(self.process_cpu_window_exclusive_ns),
            ),
            (
                "process_cpu_window_inclusive_ns",
                Value::from(self.process_cpu_window_inclusive_ns),
            ),
            (
                "thread_cpu_exclusive_ns",
                Value::from(self.thread_cpu_exclusive_ns),
            ),
            (
                "thread_cpu_inclusive_ns",
                Value::from(self.thread_cpu_inclusive_ns),
            ),
            ("truncated", Value::from(self.truncated)),
            ("unwound", Value::from(self.unwound)),
            ("wall_exclusive_ns", Value::from(self.wall_exclusive_ns)),
            ("wall_inclusive_ns", Value::from(self.wall_inclusive_ns)),
        ])
    }

    fn from_json_value(value: &Value, path: &str) -> Result<Self, SchemaError> {
        let reader = Reader::open(value, path, PHASE_NODE_KEYS)?;
        Ok(Self {
            parent: reader.optional_u32("parent")?,
            label: reader.text("label")?,
            calls: reader.u64("calls")?,
            completed: reader.u64("completed")?,
            interrupted: reader.u64("interrupted")?,
            unwound: reader.u64("unwound")?,
            truncated: reader.u64("truncated")?,
            wall_inclusive_ns: reader.u64("wall_inclusive_ns")?,
            wall_exclusive_ns: reader.u64("wall_exclusive_ns")?,
            thread_cpu_inclusive_ns: reader.u64("thread_cpu_inclusive_ns")?,
            thread_cpu_exclusive_ns: reader.u64("thread_cpu_exclusive_ns")?,
            process_cpu_window_inclusive_ns: reader.u64("process_cpu_window_inclusive_ns")?,
            process_cpu_window_exclusive_ns: reader.u64("process_cpu_window_exclusive_ns")?,
            peak_concurrent_children: reader.u32("peak_concurrent_children")?,
            active_threads_max: reader.u32("active_threads_max")?,
            load_milli_first_enter: reader.u64("load_milli_first_enter")?,
            load_milli_last_exit: reader.u64("load_milli_last_exit")?,
            load_milli_max: reader.u64("load_milli_max")?,
            allocations: reader.u64("allocations")?,
            allocated_bytes: reader.u64("allocated_bytes")?,
            live_bytes_high_water: reader.u64("live_bytes_high_water")?,
        })
    }
}

/// The complete phase tree as a flat table in first-entry order.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PhaseTree {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Phases; index zero is the root and every parent precedes its children.
    pub nodes: Vec<PhaseNode>,
}

const PHASE_TREE_KEYS: &[&str] = &["classification", "nodes"];

/// One raw failure, retained exactly as the instrumented code reported it.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RawFailure {
    /// Index of the innermost phase open on the reporting thread.
    pub phase: u32,
    /// Static label of the failing stage.
    pub stage: String,
    /// Static label of the failure; never a formatted error payload.
    pub code: String,
}

const RAW_FAILURE_KEYS: &[&str] = &["code", "phase", "stage"];

/// Every failure reported during the run, in report order.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct FailureLog {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Retained failures.
    pub entries: Vec<RawFailure>,
    /// Failures that exceeded the retention bound; nonzero rejects the report.
    pub dropped: u64,
}

const FAILURE_LOG_KEYS: &[&str] = &["classification", "dropped", "entries"];

/// A named public size observed inside one phase.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ByteCounter {
    /// Index of the innermost phase open on the reporting thread.
    pub phase: u32,
    /// What the bytes are.
    pub kind: ByteKind,
    /// Static public label of the object.
    pub label: String,
    /// Number of reports.
    pub count: u64,
    /// Sum of the reported sizes.
    pub total_bytes: u64,
    /// Smallest reported size.
    pub min_bytes: u64,
    /// Largest reported size.
    pub max_bytes: u64,
}

const BYTE_COUNTER_KEYS: &[&str] = &[
    "count",
    "kind",
    "label",
    "max_bytes",
    "min_bytes",
    "phase",
    "total_bytes",
];

/// All named byte counters.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ByteCounters {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Counters in first-report order.
    pub entries: Vec<ByteCounter>,
}

const BYTE_COUNTERS_KEYS: &[&str] = &["classification", "entries"];

/// A named public count of completed work observed inside one phase.
///
/// Work counters carry the non-byte quantities of a flow: transform calls,
/// columns, rows, butterflies, constraints. Like every other field they are
/// public geometry, never witness-dependent positions or values.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct WorkCounter {
    /// Index of the innermost phase open on the reporting thread.
    pub phase: u32,
    /// Static public label of the counted work.
    pub label: String,
    /// Number of reports.
    pub count: u64,
    /// Sum of the reported units.
    pub total_units: u64,
    /// Smallest reported number of units.
    pub min_units: u64,
    /// Largest reported number of units.
    pub max_units: u64,
}

const WORK_COUNTER_KEYS: &[&str] = &[
    "count",
    "label",
    "max_units",
    "min_units",
    "phase",
    "total_units",
];

/// All named work counters.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct WorkCounters {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Counters in first-report order.
    pub entries: Vec<WorkCounter>,
}

const WORK_COUNTERS_KEYS: &[&str] = &["classification", "entries"];

/// Allocation accounting from one scoped counter or one allocation budget.
///
/// For an allocation budget the counts stay zero: `iroha_allocation` accounts
/// requested layout bytes, not calls. `live_bytes` is then the budget's
/// reserved bytes when the session ended and `live_bytes_high_water` its peak.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AllocationSource {
    /// Static public label of the source.
    pub label: String,
    /// Which accounting produced the numbers.
    pub kind: AllocationSourceKind,
    /// Allocations reported.
    pub allocations: u64,
    /// Bytes of those allocations.
    pub allocated_bytes: u64,
    /// Releases reported.
    pub frees: u64,
    /// Bytes of those releases.
    pub freed_bytes: u64,
    /// Bytes still live when the session ended.
    pub live_bytes: u64,
    /// Highest live bytes.
    pub live_bytes_high_water: u64,
    /// Buffers still live when the session ended.
    pub live_buffers: u64,
    /// Highest number of live buffers.
    pub live_buffers_high_water: u64,
}

const ALLOCATION_SOURCE_KEYS: &[&str] = &[
    "allocated_bytes",
    "allocations",
    "freed_bytes",
    "frees",
    "kind",
    "label",
    "live_buffers",
    "live_buffers_high_water",
    "live_bytes",
    "live_bytes_high_water",
];

/// Allocation and live-buffer observations.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AllocationObservation {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Sources in registration order.
    pub sources: Vec<AllocationSource>,
}

const ALLOCATION_OBSERVATION_KEYS: &[&str] = &["classification", "sources"];

/// Process-level observations taken by the emitting process itself.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ProcessObservation {
    /// Always [`Classification::Measured`].
    pub classification: Classification,
    /// Operating-system process identifier (structural, not a measurement).
    pub pid: u32,
    /// Logical processors available to the process.
    pub logical_cpus: u32,
    /// User CPU time consumed by the process during the session.
    pub cpu_user_ns: u64,
    /// System CPU time consumed by the process during the session.
    pub cpu_system_ns: u64,
    /// Peak resident set size when the session started.
    pub peak_rss_bytes_at_begin: u64,
    /// Peak resident set size when the session ended.
    pub peak_rss_bytes: u64,
    /// How the peak was obtained.
    pub peak_rss_source: PeakRssSource,
    /// One-minute system load times 1000 when the session started.
    pub load_milli_begin: u64,
    /// One-minute system load times 1000 when the session ended.
    pub load_milli_finish: u64,
    /// Thermal state when the session started.
    pub thermal_begin: ThermalState,
    /// Thermal state when the session ended.
    pub thermal_finish: ThermalState,
    /// Public label of the platform interface that supplied the thermal state.
    pub thermal_source: String,
}

const PROCESS_OBSERVATION_KEYS: &[&str] = &[
    "classification",
    "cpu_system_ns",
    "cpu_user_ns",
    "load_milli_begin",
    "load_milli_finish",
    "logical_cpus",
    "peak_rss_bytes",
    "peak_rss_bytes_at_begin",
    "peak_rss_source",
    "pid",
    "thermal_begin",
    "thermal_finish",
    "thermal_source",
];

/// A worker count declared for one phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PhaseWorkers {
    /// Index of the phase.
    pub phase: u32,
    /// Declared workers while it ran.
    pub workers: u32,
}

const PHASE_WORKERS_KEYS: &[&str] = &["phase", "workers"];

/// Caller-declared worker counts: node-local scheduling, never validity.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct Scheduling {
    /// Always [`Classification::LocalScheduling`].
    pub classification: Classification,
    /// Workers declared for the whole session.
    pub workers: u32,
    /// Public label of what set that worker count.
    pub workers_provenance: String,
    /// Per-phase overrides in declaration order.
    pub phase_workers: Vec<PhaseWorkers>,
}

const SCHEDULING_KEYS: &[&str] = &[
    "classification",
    "phase_workers",
    "workers",
    "workers_provenance",
];

/// The address-space limit actually in force for the emitting process.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AddressSpaceObservation {
    /// Always [`Classification::LocalScheduling`].
    pub classification: Classification,
    /// Public label of the platform interface that reported the limit.
    pub source: String,
    /// Soft limit in bytes; `None` when unlimited or unavailable.
    pub soft_limit_bytes: Option<u64>,
    /// Hard limit in bytes; `None` when unlimited or unavailable.
    pub hard_limit_bytes: Option<u64>,
    /// Whether a finite soft limit was in force.
    pub enforced: bool,
}

const ADDRESS_SPACE_KEYS: &[&str] = &[
    "classification",
    "enforced",
    "hard_limit_bytes",
    "soft_limit_bytes",
    "source",
];

/// A value the caller declared instead of measuring.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct DeclaredQuantity {
    /// Never [`Classification::Measured`].
    pub classification: Classification,
    /// Static public label of the quantity.
    pub label: String,
    /// Unit of `value`.
    pub unit: Unit,
    /// The declared value.
    pub value: u64,
    /// What kind of source establishes the value.
    pub provenance_kind: ProvenanceKind,
    /// Static reference to that source, for example a source path and symbol.
    pub provenance: String,
}

const DECLARED_QUANTITY_KEYS: &[&str] = &[
    "classification",
    "label",
    "provenance",
    "provenance_kind",
    "unit",
    "value",
];

/// Events the recorder could not attribute; nonzero overflow rejects the report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RecorderHealth {
    /// Phase entries refused because the phase table was full.
    pub node_overflow_events: u64,
    /// Phase entries refused because too many phases were open.
    pub span_overflow_events: u64,
    /// Counter, declaration or source reports refused because a table was full.
    pub counter_overflow_events: u64,
    /// Guards that closed after their phase or session had already ended.
    pub stale_guard_events: u64,
    /// Entries whose explicit parent had already ended and that fell back to the root.
    pub stale_parent_events: u64,
    /// Labels outside the public grammar that were replaced.
    pub invalid_label_events: u64,
    /// Clock samples that were not monotonic and were clamped.
    pub clock_anomaly_events: u64,
    /// Phases closed by an enclosing phase or the session ending.
    pub forced_closes: u64,
}

const RECORDER_HEALTH_KEYS: &[&str] = &[
    "classification",
    "clock_anomaly_events",
    "counter_overflow_events",
    "forced_closes",
    "invalid_label_events",
    "node_overflow_events",
    "span_overflow_events",
    "stale_guard_events",
    "stale_parent_events",
];

/// One complete public-only record of a measured run.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(
    name = "iroha_measurement::MeasurementRecord",
    frame = "iroha_measurement::MeasurementRecordV1"
)]
pub struct MeasurementRecord {
    /// Always [`RECORD_SCHEMA_V1`].
    pub schema: String,
    /// Exact identity of the run.
    pub identity: RunIdentity,
    /// How the root ended.
    pub outcome: RunOutcome,
    /// Raw failures, never dropped.
    pub failures: FailureLog,
    /// The phase tree.
    pub phase_tree: PhaseTree,
    /// Named ciphertext, key, proof and transaction sizes.
    pub byte_counters: ByteCounters,
    /// Named counts of completed public work: calls, columns, rows.
    pub work_counters: WorkCounters,
    /// Allocation counts, bytes and live-buffer high-water marks.
    pub allocations: AllocationObservation,
    /// Process CPU, peak RSS, load and thermal state.
    pub process: ProcessObservation,
    /// Declared worker counts.
    pub scheduling: Scheduling,
    /// The enforced address-space limit.
    pub address_space: AddressSpaceObservation,
    /// Caller-declared projections, targets, bounds and local limits.
    pub declared: Vec<DeclaredQuantity>,
    /// Recorder self-observation.
    pub recorder: RecorderHealth,
}

const RECORD_KEYS: &[&str] = &[
    "address_space",
    "allocations",
    "byte_counters",
    "declared",
    "failures",
    "identity",
    "outcome",
    "phase_tree",
    "process",
    "recorder",
    "scheduling",
    "schema",
    "work_counters",
];

/// JSON type of every key of every object kind, in the order of its key set.
///
/// `u64` and `u32` are exact unsigned integers, `?` admits `null`,
/// `enum:<name>` is one of the listed words, `object:<kind>` and
/// `list:<kind>` nest another object kind. Emitters and validators in other
/// languages decode the JSON view from this table.
const FIELD_TYPES: &[(&str, &[&str], &[&str])] = &[
    (
        "address_space",
        ADDRESS_SPACE_KEYS,
        &["enum:classification", "bool", "u64?", "u64?", "text"],
    ),
    (
        "allocation_source",
        ALLOCATION_SOURCE_KEYS,
        &[
            "u64",
            "u64",
            "u64",
            "u64",
            "enum:allocation_source_kind",
            "text",
            "u64",
            "u64",
            "u64",
            "u64",
        ],
    ),
    (
        "allocations",
        ALLOCATION_OBSERVATION_KEYS,
        &["enum:classification", "list:allocation_source"],
    ),
    (
        "byte_counter",
        BYTE_COUNTER_KEYS,
        &["u64", "enum:byte_kind", "text", "u64", "u64", "u32", "u64"],
    ),
    (
        "byte_counters",
        BYTE_COUNTERS_KEYS,
        &["enum:classification", "list:byte_counter"],
    ),
    (
        "context",
        CONTEXT_KEYS,
        &[
            "text",
            "enum:cache_policy",
            "text",
            "text",
            "text",
            "text",
            "text",
            "bool",
            "text?",
        ],
    ),
    (
        "declared_quantity",
        DECLARED_QUANTITY_KEYS,
        &[
            "enum:classification",
            "text",
            "text",
            "enum:provenance_kind",
            "enum:unit",
            "u64",
        ],
    ),
    ("failure", RAW_FAILURE_KEYS, &["text", "u32", "text"]),
    (
        "failures",
        FAILURE_LOG_KEYS,
        &["enum:classification", "u64", "list:failure"],
    ),
    (
        "identity",
        IDENTITY_KEYS,
        &[
            "text",
            "enum:cache_policy",
            "text",
            "text",
            "enum:flow",
            "text",
            "text",
            "text",
            "bool",
            "text?",
            "text",
        ],
    ),
    (
        "phase_node",
        PHASE_NODE_KEYS,
        &[
            "u32", "u64", "u64", "u64", "u64", "u64", "text", "u64", "u64", "u64", "u64", "u32?",
            "u32", "u64", "u64", "u64", "u64", "u64", "u64", "u64", "u64",
        ],
    ),
    (
        "phase_tree",
        PHASE_TREE_KEYS,
        &["enum:classification", "list:phase_node"],
    ),
    ("phase_workers", PHASE_WORKERS_KEYS, &["u32", "u32"]),
    (
        "process",
        PROCESS_OBSERVATION_KEYS,
        &[
            "enum:classification",
            "u64",
            "u64",
            "u64",
            "u64",
            "u32",
            "u64",
            "u64",
            "enum:peak_rss_source",
            "u32",
            "enum:thermal_state",
            "enum:thermal_state",
            "text",
        ],
    ),
    (
        "record",
        RECORD_KEYS,
        &[
            "object:address_space",
            "object:allocations",
            "object:byte_counters",
            "list:declared_quantity",
            "object:failures",
            "object:identity",
            "enum:outcome",
            "object:phase_tree",
            "object:process",
            "object:recorder",
            "object:scheduling",
            "text",
            "object:work_counters",
        ],
    ),
    (
        "recorder",
        RECORDER_HEALTH_KEYS,
        &[
            "enum:classification",
            "u64",
            "u64",
            "u64",
            "u64",
            "u64",
            "u64",
            "u64",
            "u64",
        ],
    ),
    (
        "scheduling",
        SCHEDULING_KEYS,
        &["enum:classification", "list:phase_workers", "u32", "text"],
    ),
    (
        "work_counter",
        WORK_COUNTER_KEYS,
        &["u64", "text", "u64", "u64", "u32", "u64"],
    ),
    (
        "work_counters",
        WORK_COUNTERS_KEYS,
        &["enum:classification", "list:work_counter"],
    ),
];

/// Emitter-label prefixes of adapters that write the Norito form of a record
/// beside its JSON view. The harness rejects a JSON view from such an emitter
/// whose Norito sibling is missing; emitters in other languages write the JSON
/// view only.
pub const NORITO_SIBLING_EMITTER_PREFIXES: &[&str] = &["rust."];

/// JSON keys whose numbers identify something and carry no classification.
pub const STRUCTURAL_NUMBER_KEYS: &[&str] = &["parent", "phase", "pid"];

fn classification_word(value: Classification) -> Value {
    word(value.as_str())
}

fn render(value: &Value) -> String {
    // Serializing an owned JSON value cannot fail: every number in the view is
    // an unsigned integer and every string satisfies a closed ASCII grammar or
    // was produced by this module.
    json::to_string(value).unwrap_or_default()
}

impl MeasurementRecord {
    /// The JSON view as a value with alphabetically ordered keys.
    pub fn to_json_value(&self) -> Value {
        let failures = object([
            (
                "classification",
                classification_word(self.failures.classification),
            ),
            ("dropped", Value::from(self.failures.dropped)),
            (
                "entries",
                Value::Array(
                    self.failures
                        .entries
                        .iter()
                        .map(|failure| {
                            object([
                                ("code", Value::String(failure.code.clone())),
                                ("phase", Value::from(u64::from(failure.phase))),
                                ("stage", Value::String(failure.stage.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let phase_tree = object([
            (
                "classification",
                classification_word(self.phase_tree.classification),
            ),
            (
                "nodes",
                Value::Array(
                    self.phase_tree
                        .nodes
                        .iter()
                        .map(PhaseNode::to_json_value)
                        .collect(),
                ),
            ),
        ]);
        let byte_counters = object([
            (
                "classification",
                classification_word(self.byte_counters.classification),
            ),
            (
                "entries",
                Value::Array(
                    self.byte_counters
                        .entries
                        .iter()
                        .map(|counter| {
                            object([
                                ("count", Value::from(counter.count)),
                                ("kind", word(counter.kind.as_str())),
                                ("label", Value::String(counter.label.clone())),
                                ("max_bytes", Value::from(counter.max_bytes)),
                                ("min_bytes", Value::from(counter.min_bytes)),
                                ("phase", Value::from(u64::from(counter.phase))),
                                ("total_bytes", Value::from(counter.total_bytes)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let work_counters = object([
            (
                "classification",
                classification_word(self.work_counters.classification),
            ),
            (
                "entries",
                Value::Array(
                    self.work_counters
                        .entries
                        .iter()
                        .map(|counter| {
                            object([
                                ("count", Value::from(counter.count)),
                                ("label", Value::String(counter.label.clone())),
                                ("max_units", Value::from(counter.max_units)),
                                ("min_units", Value::from(counter.min_units)),
                                ("phase", Value::from(u64::from(counter.phase))),
                                ("total_units", Value::from(counter.total_units)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let allocations = object([
            (
                "classification",
                classification_word(self.allocations.classification),
            ),
            (
                "sources",
                Value::Array(
                    self.allocations
                        .sources
                        .iter()
                        .map(|source| {
                            object([
                                ("allocated_bytes", Value::from(source.allocated_bytes)),
                                ("allocations", Value::from(source.allocations)),
                                ("freed_bytes", Value::from(source.freed_bytes)),
                                ("frees", Value::from(source.frees)),
                                ("kind", word(source.kind.as_str())),
                                ("label", Value::String(source.label.clone())),
                                ("live_buffers", Value::from(source.live_buffers)),
                                (
                                    "live_buffers_high_water",
                                    Value::from(source.live_buffers_high_water),
                                ),
                                ("live_bytes", Value::from(source.live_bytes)),
                                (
                                    "live_bytes_high_water",
                                    Value::from(source.live_bytes_high_water),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let process = object([
            (
                "classification",
                classification_word(self.process.classification),
            ),
            ("cpu_system_ns", Value::from(self.process.cpu_system_ns)),
            ("cpu_user_ns", Value::from(self.process.cpu_user_ns)),
            (
                "load_milli_begin",
                Value::from(self.process.load_milli_begin),
            ),
            (
                "load_milli_finish",
                Value::from(self.process.load_milli_finish),
            ),
            (
                "logical_cpus",
                Value::from(u64::from(self.process.logical_cpus)),
            ),
            ("peak_rss_bytes", Value::from(self.process.peak_rss_bytes)),
            (
                "peak_rss_bytes_at_begin",
                Value::from(self.process.peak_rss_bytes_at_begin),
            ),
            (
                "peak_rss_source",
                word(self.process.peak_rss_source.as_str()),
            ),
            ("pid", Value::from(u64::from(self.process.pid))),
            ("thermal_begin", word(self.process.thermal_begin.as_str())),
            ("thermal_finish", word(self.process.thermal_finish.as_str())),
            (
                "thermal_source",
                Value::String(self.process.thermal_source.clone()),
            ),
        ]);
        let scheduling = object([
            (
                "classification",
                classification_word(self.scheduling.classification),
            ),
            (
                "phase_workers",
                Value::Array(
                    self.scheduling
                        .phase_workers
                        .iter()
                        .map(|entry| {
                            object([
                                ("phase", Value::from(u64::from(entry.phase))),
                                ("workers", Value::from(u64::from(entry.workers))),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("workers", Value::from(u64::from(self.scheduling.workers))),
            (
                "workers_provenance",
                Value::String(self.scheduling.workers_provenance.clone()),
            ),
        ]);
        let address_space = object([
            (
                "classification",
                classification_word(self.address_space.classification),
            ),
            ("enforced", Value::Bool(self.address_space.enforced)),
            (
                "hard_limit_bytes",
                optional_number(self.address_space.hard_limit_bytes),
            ),
            (
                "soft_limit_bytes",
                optional_number(self.address_space.soft_limit_bytes),
            ),
            ("source", Value::String(self.address_space.source.clone())),
        ]);
        let declared = Value::Array(
            self.declared
                .iter()
                .map(|quantity| {
                    object([
                        (
                            "classification",
                            classification_word(quantity.classification),
                        ),
                        ("label", Value::String(quantity.label.clone())),
                        ("provenance", Value::String(quantity.provenance.clone())),
                        ("provenance_kind", word(quantity.provenance_kind.as_str())),
                        ("unit", word(quantity.unit.as_str())),
                        ("value", Value::from(quantity.value)),
                    ])
                })
                .collect(),
        );
        let recorder = object([
            (
                "classification",
                classification_word(Classification::Measured),
            ),
            (
                "clock_anomaly_events",
                Value::from(self.recorder.clock_anomaly_events),
            ),
            (
                "counter_overflow_events",
                Value::from(self.recorder.counter_overflow_events),
            ),
            ("forced_closes", Value::from(self.recorder.forced_closes)),
            (
                "invalid_label_events",
                Value::from(self.recorder.invalid_label_events),
            ),
            (
                "node_overflow_events",
                Value::from(self.recorder.node_overflow_events),
            ),
            (
                "span_overflow_events",
                Value::from(self.recorder.span_overflow_events),
            ),
            (
                "stale_guard_events",
                Value::from(self.recorder.stale_guard_events),
            ),
            (
                "stale_parent_events",
                Value::from(self.recorder.stale_parent_events),
            ),
        ]);
        object([
            ("address_space", address_space),
            ("allocations", allocations),
            ("byte_counters", byte_counters),
            ("declared", declared),
            ("failures", failures),
            ("identity", self.identity.to_json_value()),
            ("outcome", word(self.outcome.as_str())),
            ("phase_tree", phase_tree),
            ("process", process),
            ("recorder", recorder),
            ("scheduling", scheduling),
            ("schema", Value::String(self.schema.clone())),
            ("work_counters", work_counters),
        ])
    }

    /// Decode the JSON view.
    ///
    /// Unknown keys, missing keys, floats, negative numbers and unknown
    /// enumerated words are rejected. Semantic checks are in [`crate::report`].
    ///
    /// # Errors
    /// Returns the first difference from the schema.
    pub fn from_json_value(value: &Value) -> Result<Self, SchemaError> {
        let reader = Reader::open(value, "record", RECORD_KEYS)?;

        let failures = Reader::open(
            reader.value("failures")?,
            "record.failures",
            FAILURE_LOG_KEYS,
        )?;
        let failures = FailureLog {
            classification: failures.word("classification", Classification::parse)?,
            entries: failures.list("entries", |value, path| {
                let entry = Reader::open(value, path, RAW_FAILURE_KEYS)?;
                Ok(RawFailure {
                    phase: entry.u32("phase")?,
                    stage: entry.text("stage")?,
                    code: entry.text("code")?,
                })
            })?,
            dropped: failures.u64("dropped")?,
        };

        let tree = Reader::open(
            reader.value("phase_tree")?,
            "record.phase_tree",
            PHASE_TREE_KEYS,
        )?;
        let phase_tree = PhaseTree {
            classification: tree.word("classification", Classification::parse)?,
            nodes: tree.list("nodes", PhaseNode::from_json_value)?,
        };

        let counters = Reader::open(
            reader.value("byte_counters")?,
            "record.byte_counters",
            BYTE_COUNTERS_KEYS,
        )?;
        let byte_counters = ByteCounters {
            classification: counters.word("classification", Classification::parse)?,
            entries: counters.list("entries", |value, path| {
                let entry = Reader::open(value, path, BYTE_COUNTER_KEYS)?;
                Ok(ByteCounter {
                    phase: entry.u32("phase")?,
                    kind: entry.word("kind", ByteKind::parse)?,
                    label: entry.text("label")?,
                    count: entry.u64("count")?,
                    total_bytes: entry.u64("total_bytes")?,
                    min_bytes: entry.u64("min_bytes")?,
                    max_bytes: entry.u64("max_bytes")?,
                })
            })?,
        };

        let work = Reader::open(
            reader.value("work_counters")?,
            "record.work_counters",
            WORK_COUNTERS_KEYS,
        )?;
        let work_counters = WorkCounters {
            classification: work.word("classification", Classification::parse)?,
            entries: work.list("entries", |value, path| {
                let entry = Reader::open(value, path, WORK_COUNTER_KEYS)?;
                Ok(WorkCounter {
                    phase: entry.u32("phase")?,
                    label: entry.text("label")?,
                    count: entry.u64("count")?,
                    total_units: entry.u64("total_units")?,
                    min_units: entry.u64("min_units")?,
                    max_units: entry.u64("max_units")?,
                })
            })?,
        };

        let allocation = Reader::open(
            reader.value("allocations")?,
            "record.allocations",
            ALLOCATION_OBSERVATION_KEYS,
        )?;
        let allocations = AllocationObservation {
            classification: allocation.word("classification", Classification::parse)?,
            sources: allocation.list("sources", |value, path| {
                let entry = Reader::open(value, path, ALLOCATION_SOURCE_KEYS)?;
                Ok(AllocationSource {
                    label: entry.text("label")?,
                    kind: entry.word("kind", AllocationSourceKind::parse)?,
                    allocations: entry.u64("allocations")?,
                    allocated_bytes: entry.u64("allocated_bytes")?,
                    frees: entry.u64("frees")?,
                    freed_bytes: entry.u64("freed_bytes")?,
                    live_bytes: entry.u64("live_bytes")?,
                    live_bytes_high_water: entry.u64("live_bytes_high_water")?,
                    live_buffers: entry.u64("live_buffers")?,
                    live_buffers_high_water: entry.u64("live_buffers_high_water")?,
                })
            })?,
        };

        let process = Reader::open(
            reader.value("process")?,
            "record.process",
            PROCESS_OBSERVATION_KEYS,
        )?;
        let process = ProcessObservation {
            classification: process.word("classification", Classification::parse)?,
            pid: process.u32("pid")?,
            logical_cpus: process.u32("logical_cpus")?,
            cpu_user_ns: process.u64("cpu_user_ns")?,
            cpu_system_ns: process.u64("cpu_system_ns")?,
            peak_rss_bytes_at_begin: process.u64("peak_rss_bytes_at_begin")?,
            peak_rss_bytes: process.u64("peak_rss_bytes")?,
            peak_rss_source: process.word("peak_rss_source", PeakRssSource::parse)?,
            load_milli_begin: process.u64("load_milli_begin")?,
            load_milli_finish: process.u64("load_milli_finish")?,
            thermal_begin: process.word("thermal_begin", ThermalState::parse)?,
            thermal_finish: process.word("thermal_finish", ThermalState::parse)?,
            thermal_source: process.text("thermal_source")?,
        };

        let scheduling = Reader::open(
            reader.value("scheduling")?,
            "record.scheduling",
            SCHEDULING_KEYS,
        )?;
        let scheduling = Scheduling {
            classification: scheduling.word("classification", Classification::parse)?,
            workers: scheduling.u32("workers")?,
            workers_provenance: scheduling.text("workers_provenance")?,
            phase_workers: scheduling.list("phase_workers", |value, path| {
                let entry = Reader::open(value, path, PHASE_WORKERS_KEYS)?;
                Ok(PhaseWorkers {
                    phase: entry.u32("phase")?,
                    workers: entry.u32("workers")?,
                })
            })?,
        };

        let address = Reader::open(
            reader.value("address_space")?,
            "record.address_space",
            ADDRESS_SPACE_KEYS,
        )?;
        let address_space = AddressSpaceObservation {
            classification: address.word("classification", Classification::parse)?,
            source: address.text("source")?,
            soft_limit_bytes: address.optional_u64("soft_limit_bytes")?,
            hard_limit_bytes: address.optional_u64("hard_limit_bytes")?,
            enforced: address.boolean("enforced")?,
        };

        let declared = reader.list("declared", |value, path| {
            let entry = Reader::open(value, path, DECLARED_QUANTITY_KEYS)?;
            Ok(DeclaredQuantity {
                classification: entry.word("classification", Classification::parse)?,
                label: entry.text("label")?,
                unit: entry.word("unit", Unit::parse)?,
                value: entry.u64("value")?,
                provenance_kind: entry.word("provenance_kind", ProvenanceKind::parse)?,
                provenance: entry.text("provenance")?,
            })
        })?;

        let health = Reader::open(
            reader.value("recorder")?,
            "record.recorder",
            RECORDER_HEALTH_KEYS,
        )?;
        let health_classification = health.word("classification", Classification::parse)?;
        if health_classification != Classification::Measured {
            return Err(SchemaError::UnknownText {
                path: health.at("classification"),
                text: health_classification.as_str().to_owned(),
            });
        }
        let recorder = RecorderHealth {
            node_overflow_events: health.u64("node_overflow_events")?,
            span_overflow_events: health.u64("span_overflow_events")?,
            counter_overflow_events: health.u64("counter_overflow_events")?,
            stale_guard_events: health.u64("stale_guard_events")?,
            stale_parent_events: health.u64("stale_parent_events")?,
            invalid_label_events: health.u64("invalid_label_events")?,
            clock_anomaly_events: health.u64("clock_anomaly_events")?,
            forced_closes: health.u64("forced_closes")?,
        };

        Ok(Self {
            schema: reader.text("schema")?,
            identity: RunIdentity::from_json_value(reader.value("identity")?, "record.identity")?,
            outcome: reader.word("outcome", RunOutcome::parse)?,
            failures,
            phase_tree,
            byte_counters,
            work_counters,
            allocations,
            process,
            scheduling,
            address_space,
            declared,
            recorder,
        })
    }

    /// Compact canonical JSON text: keys sorted, no insignificant whitespace.
    pub fn to_json_view(&self) -> String {
        render(&self.to_json_value())
    }

    /// Decode JSON text emitted by any adapter.
    ///
    /// # Errors
    /// Returns a parse error or the first difference from the schema.
    pub fn from_json_view(text: &str) -> Result<Self, SchemaError> {
        let value =
            json::parse_value(text).map_err(|error| SchemaError::Json(error.to_string()))?;
        Self::from_json_value(&value)
    }

    /// Canonical Norito bytes with the frame header that declares the layout.
    ///
    /// # Errors
    /// Returns the codec error when the record cannot be framed.
    pub fn to_norito_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        norito::to_bytes(self).map_err(|error| SchemaError::Norito(error.to_string()))
    }

    /// Decode canonical Norito bytes.
    ///
    /// # Errors
    /// Returns the codec error for malformed, truncated or foreign frames.
    pub fn from_norito_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        norito::decode_from_bytes(bytes).map_err(|error| SchemaError::Norito(error.to_string()))
    }
}

impl json::JsonSerialize for MeasurementRecord {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(&self.to_json_view());
    }
}

impl json::JsonDeserialize for MeasurementRecord {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        Self::from_json_value(&value).map_err(|error| json::Error::Message(error.to_string()))
    }
}

fn words<T: Copy>(all: &[T], spell: impl Fn(T) -> &'static str) -> Value {
    Value::Array(all.iter().map(|value| word(spell(*value))).collect())
}

fn keys(list: &[&'static str]) -> Value {
    Value::Array(list.iter().map(|key| word(key)).collect())
}

/// Machine-readable description of the JSON view, generated from this module.
///
/// `fixtures/record_schema_v1.json` is the tracked copy; a test fails when it
/// drifts. The harness script and SDK emitters in other languages read that
/// file instead of restating key sets and enumerated words.
pub fn schema_descriptor() -> Value {
    object([
        (
            "classification_by_section",
            object([
                (
                    "address_space",
                    word(Classification::LocalScheduling.as_str()),
                ),
                ("allocations", word(Classification::Measured.as_str())),
                ("byte_counters", word(Classification::Measured.as_str())),
                ("failures", word(Classification::Measured.as_str())),
                ("phase_tree", word(Classification::Measured.as_str())),
                ("process", word(Classification::Measured.as_str())),
                ("recorder", word(Classification::Measured.as_str())),
                ("scheduling", word(Classification::LocalScheduling.as_str())),
                ("work_counters", word(Classification::Measured.as_str())),
            ]),
        ),
        ("context_file", word(HARNESS_CONTEXT_FILE)),
        ("context_schema", word(CONTEXT_SCHEMA_V1)),
        (
            "declared_provenance",
            object([
                (
                    "deterministic_consensus_bound",
                    Value::Array(vec![
                        word(ProvenanceKind::ProtocolConstant.as_str()),
                        word(ProvenanceKind::CommittedState.as_str()),
                    ]),
                ),
                (
                    "engineering_target",
                    Value::Array(vec![word(ProvenanceKind::PlanTarget.as_str())]),
                ),
                (
                    "local_scheduling",
                    Value::Array(vec![
                        word(ProvenanceKind::LocalConfiguration.as_str()),
                        word(ProvenanceKind::OperatingSystem.as_str()),
                    ]),
                ),
                (
                    "projection",
                    Value::Array(vec![word(ProvenanceKind::ModelProjection.as_str())]),
                ),
            ]),
        ),
        (
            "enums",
            object([
                (
                    "allocation_source_kind",
                    words(AllocationSourceKind::ALL, AllocationSourceKind::as_str),
                ),
                ("byte_kind", words(ByteKind::ALL, ByteKind::as_str)),
                ("cache_policy", words(CachePolicy::ALL, CachePolicy::as_str)),
                (
                    "classification",
                    words(Classification::ALL, Classification::as_str),
                ),
                ("flow", words(FlowKind::ALL, FlowKind::as_str)),
                ("outcome", words(RunOutcome::ALL, RunOutcome::as_str)),
                (
                    "peak_rss_source",
                    words(PeakRssSource::ALL, PeakRssSource::as_str),
                ),
                (
                    "provenance_kind",
                    words(ProvenanceKind::ALL, ProvenanceKind::as_str),
                ),
                (
                    "thermal_state",
                    words(ThermalState::ALL, ThermalState::as_str),
                ),
                ("unit", words(Unit::ALL, Unit::as_str)),
            ]),
        ),
        ("harness_output_dir_env", word(HARNESS_OUTPUT_DIR_ENV)),
        ("invalid_label", word(text::INVALID_LABEL)),
        ("decode_fixed_classification", keys(&["recorder"])),
        (
            "fields",
            Value::Object(
                FIELD_TYPES
                    .iter()
                    .map(|(kind, names, types)| {
                        (
                            (*kind).to_owned(),
                            Value::Object(
                                names
                                    .iter()
                                    .zip(types.iter())
                                    .map(|(name, kind)| ((*name).to_owned(), word(kind)))
                                    .collect(),
                            ),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "limits",
            object([
                (
                    "identity_max_bytes",
                    Value::from(text::MAX_IDENTITY_BYTES as u64),
                ),
                ("label_max_bytes", Value::from(text::MAX_LABEL_BYTES as u64)),
                (
                    "unattributed_denominator",
                    Value::from(crate::report::UNATTRIBUTED_DENOMINATOR),
                ),
                (
                    "unattributed_numerator",
                    Value::from(crate::report::UNATTRIBUTED_NUMERATOR),
                ),
            ]),
        ),
        (
            "norito_sibling_emitter_prefixes",
            keys(NORITO_SIBLING_EMITTER_PREFIXES),
        ),
        ("record_schema", word(RECORD_SCHEMA_V1)),
        ("structural_number_keys", keys(STRUCTURAL_NUMBER_KEYS)),
        ("unbound", word(text::UNBOUND)),
    ])
}

/// Pretty JSON text of [`schema_descriptor`], as tracked in `fixtures/`.
pub fn schema_descriptor_text() -> String {
    let mut text = json::to_string_pretty(&schema_descriptor()).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::test_support::{bound_context, sample_record};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name)
    }

    fn pretty(value: &Value) -> String {
        let mut text = json::to_string_pretty(value).unwrap();
        text.push('\n');
        text
    }

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        let mut text = String::with_capacity(bytes.len() * 2 + 1);
        for byte in bytes {
            write!(text, "{byte:02x}").unwrap();
        }
        text.push('\n');
        text
    }

    fn at<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
        path.iter().fold(value, |value, key| match value {
            Value::Object(map) => map.get_mut(*key).unwrap(),
            Value::Array(values) => &mut values[key.parse::<usize>().unwrap()],
            _ => panic!("no member {key}"),
        })
    }

    fn decode_after(change: impl FnOnce(&mut Value)) -> Result<MeasurementRecord, SchemaError> {
        let mut view = sample_record().to_json_value();
        change(&mut view);
        MeasurementRecord::from_json_value(&view)
    }

    #[test]
    fn enumerated_words_round_trip_and_are_unique() {
        macro_rules! check {
            ($($name:ident),+) => {$(
                let mut seen = Vec::new();
                for value in $name::ALL {
                    let text = value.as_str();
                    assert_eq!($name::parse(text), Some(*value));
                    assert!(crate::text::is_public_label(text));
                    assert!(!seen.contains(&text), "{text}");
                    seen.push(text);
                }
                assert_eq!($name::parse("no_such_word"), None);
                assert_eq!($name::parse(""), None);
            )+};
        }
        check!(
            Classification,
            FlowKind,
            CachePolicy,
            ThermalState,
            RunOutcome,
            ByteKind,
            Unit,
            AllocationSourceKind,
            PeakRssSource,
            ProvenanceKind
        );
        assert_eq!(
            Classification::ALL
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>(),
            [
                "projection",
                "engineering_target",
                "deterministic_consensus_bound",
                "local_scheduling",
                "measured"
            ]
        );
    }

    #[test]
    fn norito_encoding_round_trips_and_is_deterministic() {
        let record = sample_record();
        let bytes = record.to_norito_bytes().unwrap();
        assert_eq!(bytes, record.clone().to_norito_bytes().unwrap());
        assert_eq!(bytes, sample_record().to_norito_bytes().unwrap());
        assert_eq!(
            MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
            record
        );
        let mut changed = record;
        changed.phase_tree.nodes[4].wall_inclusive_ns += 1;
        assert_ne!(changed.to_norito_bytes().unwrap(), bytes);
    }

    #[test]
    fn norito_decoding_rejects_truncated_foreign_and_corrupted_frames() {
        let bytes = sample_record().to_norito_bytes().unwrap();
        for length in [0, 1, 8, bytes.len() / 2, bytes.len() - 1] {
            assert!(matches!(
                MeasurementRecord::from_norito_bytes(&bytes[..length]),
                Err(SchemaError::Norito(_))
            ));
        }
        let mut corrupted = bytes.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert!(MeasurementRecord::from_norito_bytes(&corrupted).is_err());
        // A frame of a different root type carries a different schema hash.
        let foreign = norito::to_bytes(&vec![1_u64, 2, 3]).unwrap();
        assert!(MeasurementRecord::from_norito_bytes(&foreign).is_err());
        assert!(MeasurementRecord::from_norito_bytes(b"not a norito frame").is_err());
    }

    #[test]
    fn json_view_round_trips_and_is_canonical() {
        let record = sample_record();
        let view = record.to_json_view();
        assert_eq!(view, sample_record().to_json_view());
        assert!(!view.contains(' ') && !view.contains('\n'));
        assert!(view.starts_with("{\"address_space\":{\"classification\":\"local_scheduling\""));
        let decoded = MeasurementRecord::from_json_view(&view).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(decoded.to_json_view(), view);
        // The pretty form used for tracked fixtures decodes to the same record.
        let spaced = pretty(&record.to_json_value());
        assert_eq!(MeasurementRecord::from_json_view(&spaced).unwrap(), record);
        // The Norito JSON traits use the same view.
        assert_eq!(json::to_string(&record).unwrap(), view);
        assert_eq!(json::from_str::<MeasurementRecord>(&view).unwrap(), record);
        assert!(json::from_str::<MeasurementRecord>("{}").is_err());
    }

    #[test]
    fn json_and_norito_forms_carry_the_same_record() {
        let record = sample_record();
        let from_json = MeasurementRecord::from_json_view(&record.to_json_view()).unwrap();
        assert_eq!(
            from_json.to_norito_bytes().unwrap(),
            record.to_norito_bytes().unwrap()
        );
        let from_norito =
            MeasurementRecord::from_norito_bytes(&record.to_norito_bytes().unwrap()).unwrap();
        assert_eq!(from_norito.to_json_view(), record.to_json_view());
    }

    fn keys_of(value: &Value) -> Vec<&str> {
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn written_keys_equal_the_declared_key_sets() {
        let mut record = sample_record();
        record.failures.entries.push(RawFailure {
            phase: 1,
            stage: "commit".into(),
            code: "refused".into(),
        });
        let view = record.to_json_value();
        let member = |path: &[&str]| {
            path.iter().fold(&view, |value, key| match value {
                Value::Object(map) => &map[*key],
                Value::Array(values) => &values[key.parse::<usize>().unwrap()],
                _ => panic!("no member {key}"),
            })
        };
        for (path, expected) in [
            (&[][..], RECORD_KEYS),
            (&["identity"][..], IDENTITY_KEYS),
            (&["failures"][..], FAILURE_LOG_KEYS),
            (&["failures", "entries", "0"][..], RAW_FAILURE_KEYS),
            (&["phase_tree"][..], PHASE_TREE_KEYS),
            (&["phase_tree", "nodes", "0"][..], PHASE_NODE_KEYS),
            (&["byte_counters"][..], BYTE_COUNTERS_KEYS),
            (&["byte_counters", "entries", "0"][..], BYTE_COUNTER_KEYS),
            (&["work_counters"][..], WORK_COUNTERS_KEYS),
            (&["work_counters", "entries", "0"][..], WORK_COUNTER_KEYS),
            (&["allocations"][..], ALLOCATION_OBSERVATION_KEYS),
            (&["allocations", "sources", "0"][..], ALLOCATION_SOURCE_KEYS),
            (&["process"][..], PROCESS_OBSERVATION_KEYS),
            (&["scheduling"][..], SCHEDULING_KEYS),
            (
                &["scheduling", "phase_workers", "0"][..],
                PHASE_WORKERS_KEYS,
            ),
            (&["address_space"][..], ADDRESS_SPACE_KEYS),
            (&["declared", "0"][..], DECLARED_QUANTITY_KEYS),
            (&["recorder"][..], RECORDER_HEALTH_KEYS),
        ] {
            assert_eq!(keys_of(member(path)), expected, "{path:?}");
            let mut sorted = expected.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted, expected, "{path:?} keys are sorted and unique");
        }
        assert_eq!(keys_of(&bound_context().to_json_value()), CONTEXT_KEYS);
        assert_eq!(MeasurementRecord::from_json_value(&view).unwrap(), record);
    }

    #[test]
    fn json_decoding_rejects_unknown_and_missing_keys_in_every_section() {
        for path in [
            &[][..],
            &["identity"][..],
            &["failures"][..],
            &["phase_tree"][..],
            &["phase_tree", "nodes", "2"][..],
            &["byte_counters"][..],
            &["byte_counters", "entries", "1"][..],
            &["work_counters"][..],
            &["work_counters", "entries", "0"][..],
            &["allocations"][..],
            &["allocations", "sources", "0"][..],
            &["process"][..],
            &["scheduling"][..],
            &["scheduling", "phase_workers", "0"][..],
            &["address_space"][..],
            &["declared", "3"][..],
            &["recorder"][..],
        ] {
            let unknown = decode_after(|view| {
                at(view, path)
                    .as_object_mut()
                    .unwrap()
                    .insert("witness".into(), Value::String("secret".into()));
            });
            assert!(
                matches!(&unknown, Err(SchemaError::UnknownKey { key, .. }) if key == "witness"),
                "{path:?}: {unknown:?}"
            );
            let missing = decode_after(|view| {
                let map = at(view, path).as_object_mut().unwrap();
                let first = map.keys().next().unwrap().clone();
                map.remove(&first);
            });
            assert!(
                matches!(missing, Err(SchemaError::MissingKey { .. })),
                "{path:?}: {missing:?}"
            );
        }
    }

    #[test]
    fn json_decoding_rejects_wrong_types_ranges_and_words() {
        let shape = |path: &[&str], value: Value| {
            let result = decode_after(|view| *at(view, path) = value);
            assert!(
                matches!(result, Err(SchemaError::Shape { .. })),
                "{path:?}: {result:?}"
            );
        };
        let nanoseconds = ["phase_tree", "nodes", "0", "wall_inclusive_ns"];
        shape(&nanoseconds, json::parse_value("1.5").unwrap());
        shape(&nanoseconds, json::parse_value("-1").unwrap());
        shape(&nanoseconds, json::parse_value("1e3").unwrap());
        shape(&nanoseconds, Value::String("7".into()));
        shape(&nanoseconds, Value::Null);
        shape(
            &nanoseconds,
            json::parse_value("18446744073709551616").unwrap(),
        );
        shape(
            &["phase_tree", "nodes", "1", "parent"],
            Value::from(u64::from(u32::MAX) + 1),
        );
        shape(
            &["phase_tree", "nodes", "1", "peak_concurrent_children"],
            Value::from(u64::from(u32::MAX) + 1),
        );
        shape(&["phase_tree", "nodes", "1", "label"], Value::from(3_u64));
        shape(&["phase_tree", "nodes"], Value::String("nodes".into()));
        shape(&["phase_tree"], Value::Array(Vec::new()));
        shape(&["identity", "source_dirty"], Value::String("yes".into()));
        shape(&["identity", "source_dirty_digest"], Value::Bool(true));
        shape(&["address_space", "soft_limit_bytes"], Value::Bool(false));
        shape(&["declared"], Value::Null);
        shape(
            &["work_counters", "entries", "0", "total_units"],
            json::parse_value("2.5").unwrap(),
        );
        shape(
            &["work_counters", "entries", "0", "phase"],
            Value::from(u64::from(u32::MAX) + 1),
        );
        shape(&["work_counters", "entries", "0", "label"], Value::Null);
        let word = |path: &[&str]| {
            let result =
                decode_after(|view| *at(view, path) = Value::String("no_such_word".into()));
            assert!(
                matches!(&result, Err(SchemaError::UnknownText { text, .. }) if text == "no_such_word"),
                "{path:?}: {result:?}"
            );
        };
        word(&["outcome"]);
        word(&["identity", "flow"]);
        word(&["identity", "cache_policy"]);
        word(&["phase_tree", "classification"]);
        word(&["byte_counters", "entries", "0", "kind"]);
        word(&["work_counters", "classification"]);
        word(&["allocations", "sources", "0", "kind"]);
        word(&["process", "peak_rss_source"]);
        word(&["process", "thermal_begin"]);
        word(&["declared", "0", "unit"]);
        word(&["declared", "0", "provenance_kind"]);
        // The recorder section is measured by construction; no other word decodes.
        let recorder = decode_after(|view| {
            *at(view, &["recorder", "classification"]) = Value::String("projection".into());
        });
        assert!(matches!(recorder, Err(SchemaError::UnknownText { .. })));
        // The largest exact unsigned value is accepted.
        let maximum = decode_after(|view| *at(view, &nanoseconds) = Value::from(u64::MAX)).unwrap();
        assert_eq!(maximum.phase_tree.nodes[0].wall_inclusive_ns, u64::MAX);
        assert!(matches!(
            MeasurementRecord::from_json_view("{not json"),
            Err(SchemaError::Json(_))
        ));
        assert!(matches!(
            MeasurementRecord::from_json_view("[]"),
            Err(SchemaError::Shape { .. })
        ));
    }

    #[test]
    fn optional_fields_use_null_and_round_trip() {
        let mut record = sample_record();
        record.address_space.soft_limit_bytes = None;
        record.address_space.hard_limit_bytes = None;
        record.address_space.enforced = false;
        record.identity.context.source_dirty = false;
        record.identity.context.source_dirty_digest = None;
        let view = record.to_json_value();
        let address = view.as_object().unwrap()["address_space"]
            .as_object()
            .unwrap();
        assert_eq!(address["soft_limit_bytes"], Value::Null);
        assert_eq!(address["hard_limit_bytes"], Value::Null);
        let nodes = view.as_object().unwrap()["phase_tree"].as_object().unwrap()["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(nodes[0].as_object().unwrap()["parent"], Value::Null);
        assert_eq!(nodes[1].as_object().unwrap()["parent"], Value::from(0_u64));
        assert_eq!(MeasurementRecord::from_json_value(&view).unwrap(), record);
        let bytes = record.to_norito_bytes().unwrap();
        assert_eq!(
            MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
            record
        );
    }

    #[test]
    fn run_context_round_trips_and_checks_its_schema() {
        let context = bound_context();
        let text = context.to_json_view();
        assert_eq!(RunContext::from_json_view(&text).unwrap(), context);
        assert!(text.contains("\"schema\":\"iroha.measurement.context.v1\""));
        let unbound = RunContext::unbound();
        assert_eq!(
            RunContext::from_json_view(&unbound.to_json_view()).unwrap(),
            unbound
        );
        assert_eq!(unbound.source_commit, text::UNBOUND);
        assert!(unbound.source_dirty && unbound.source_dirty_digest.is_none());
        let mut wrong = context.to_json_value();
        *at(&mut wrong, &["schema"]) = Value::String(RECORD_SCHEMA_V1.into());
        assert!(matches!(
            RunContext::from_json_value(&wrong),
            Err(SchemaError::UnknownText { .. })
        ));
        assert!(matches!(
            RunContext::from_json_view("{"),
            Err(SchemaError::Json(_))
        ));
        assert!(matches!(
            RunContext::from_json_view("{}"),
            Err(SchemaError::MissingKey { .. })
        ));
    }

    #[test]
    fn run_identity_replaces_malformed_literals_and_keeps_the_context() {
        let identity =
            RunIdentity::new(bound_context(), "ok.workload", FlowKind::Sdk, "bad emitter");
        assert_eq!(identity.workload, "ok.workload");
        assert_eq!(identity.emitter, text::INVALID_LABEL);
        assert_eq!(identity.flow, FlowKind::Sdk);
        assert_eq!(identity.context, bound_context());
        let view = identity.to_json_value();
        assert_eq!(keys_of(&view), IDENTITY_KEYS);
        assert_eq!(
            RunIdentity::from_json_value(&view, "identity").unwrap(),
            identity
        );
    }

    #[test]
    fn schema_errors_name_the_offending_path() {
        let error = decode_after(|view| {
            *at(view, &["phase_tree", "nodes", "3", "calls"]) = Value::Bool(true);
        })
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "record.phase_tree.nodes[3].calls: expected unsigned 64-bit integer"
        );
        for (error, text) in [
            (
                SchemaError::Json("eof".into()),
                "measurement JSON does not parse: eof",
            ),
            (
                SchemaError::Norito("short".into()),
                "measurement Norito bytes do not decode: short",
            ),
            (
                SchemaError::MissingKey {
                    path: "record".into(),
                    key: "schema",
                },
                "record: missing key schema",
            ),
            (
                SchemaError::UnknownKey {
                    path: "record".into(),
                    key: "witness".into(),
                },
                "record: unknown key witness",
            ),
            (
                SchemaError::UnknownText {
                    path: "record.outcome".into(),
                    text: "maybe".into(),
                },
                "record.outcome: unknown word maybe",
            ),
        ] {
            assert_eq!(error.to_string(), text);
        }
    }

    #[test]
    fn low_level_json_helpers_are_exact() {
        assert_eq!(unsigned(&Value::from(7_u64), "p").unwrap(), 7);
        assert_eq!(unsigned(&json::parse_value("7").unwrap(), "p").unwrap(), 7);
        assert!(unsigned(&json::parse_value("7.0").unwrap(), "p").is_err());
        assert!(unsigned(&Value::Null, "p").is_err());
        assert_eq!(optional_number(None), Value::Null);
        assert_eq!(optional_number(Some(3)), Value::from(3_u64));
        assert_eq!(optional_string(None), Value::Null);
        assert_eq!(
            optional_string(Some(&"a".to_owned())),
            Value::String("a".into())
        );
        assert_eq!(word("measured"), Value::String("measured".into()));
        assert_eq!(
            classification_word(Classification::Projection),
            Value::String("projection".into())
        );
        assert_eq!(
            render(&object([("b", Value::from(1_u64)), ("a", Value::Null)])),
            "{\"a\":null,\"b\":1}"
        );
        assert_eq!(keys(&["x", "y"]), Value::Array(vec![word("x"), word("y")]));
        assert_eq!(
            words(CachePolicy::ALL, CachePolicy::as_str),
            Value::Array(vec![word("cold"), word("warm")])
        );
    }

    #[test]
    fn schema_descriptor_lists_every_key_set_word_and_rule() {
        let descriptor = schema_descriptor();
        let map = descriptor.as_object().unwrap();
        assert_eq!(map["record_schema"], word(RECORD_SCHEMA_V1));
        assert_eq!(map["context_schema"], word(CONTEXT_SCHEMA_V1));
        assert_eq!(
            map["harness_output_dir_env"],
            word("IROHA_MEASUREMENT_OUTPUT_DIR")
        );
        assert_eq!(map["context_file"], word("context.json"));
        assert_eq!(map["unbound"], word("unbound"));
        assert_eq!(map["invalid_label"], word("invalid_label"));
        assert_eq!(map["structural_number_keys"], keys(STRUCTURAL_NUMBER_KEYS));
        let enums = map["enums"].as_object().unwrap();
        assert_eq!(enums.len(), 10);
        assert_eq!(
            enums["classification"],
            words(Classification::ALL, Classification::as_str)
        );
        let fields = map["fields"].as_object().unwrap();
        assert_eq!(fields.len(), 19);
        assert_eq!(keys_of(&fields["phase_node"]), PHASE_NODE_KEYS);
        assert_eq!(keys_of(&fields["work_counter"]), WORK_COUNTER_KEYS);
        assert_eq!(
            fields["work_counter"].as_object().unwrap()["total_units"],
            word("u64")
        );
        assert_eq!(
            map["norito_sibling_emitter_prefixes"],
            Value::Array(vec![word("rust.")])
        );
        assert_eq!(keys_of(&fields["record"]), RECORD_KEYS);
        assert_eq!(
            fields["phase_node"].as_object().unwrap()["parent"],
            word("u32?")
        );
        assert_eq!(map["decode_fixed_classification"], keys(&["recorder"]));
        let limits = map["limits"].as_object().unwrap();
        assert_eq!(limits["unattributed_numerator"], Value::from(1_u64));
        assert_eq!(limits["unattributed_denominator"], Value::from(100_u64));
        assert_eq!(limits["label_max_bytes"], Value::from(96_u64));
        assert_eq!(limits["identity_max_bytes"], Value::from(160_u64));
        let sections = map["classification_by_section"].as_object().unwrap();
        assert_eq!(sections.len(), 9);
        assert_eq!(sections["work_counters"], word("measured"));
        assert_eq!(sections["scheduling"], word("local_scheduling"));
        assert_eq!(sections["phase_tree"], word("measured"));
        let provenance = map["declared_provenance"].as_object().unwrap();
        assert_eq!(
            provenance["deterministic_consensus_bound"],
            Value::Array(vec![word("protocol_constant"), word("committed_state")])
        );
        assert!(!provenance.contains_key("measured"));
        assert_eq!(schema_descriptor_text(), pretty(&descriptor));
    }

    /// Check `value` against the declared type of one object kind, the way an
    /// emitter in another language decodes the JSON view from the descriptor.
    fn conforms(descriptor: &Map, kind: &str, value: &Value, path: &str) -> Result<(), String> {
        let fields = descriptor["fields"].as_object().unwrap()[kind]
            .as_object()
            .unwrap();
        let map = value
            .as_object()
            .ok_or_else(|| format!("{path}: not an object"))?;
        if map.keys().ne(fields.keys()) {
            return Err(format!("{path}: key set differs"));
        }
        for (key, declared) in fields {
            let declared = declared.as_str().unwrap();
            let (value, path) = (&map[key], format!("{path}.{key}"));
            let (base, optional) = declared
                .strip_suffix('?')
                .map_or((declared, false), |base| (base, true));
            if optional && *value == Value::Null {
                continue;
            }
            let accepted = match base.split_once(':') {
                None => match base {
                    "u64" => unsigned(value, &path).is_ok(),
                    "u32" => unsigned(value, &path).is_ok_and(|n| u32::try_from(n).is_ok()),
                    "bool" => matches!(value, Value::Bool(_)),
                    "text" => matches!(value, Value::String(_)),
                    other => return Err(format!("{path}: unknown type {other}")),
                },
                Some(("enum", name)) => {
                    let words = descriptor["enums"].as_object().unwrap()[name]
                        .as_array()
                        .unwrap();
                    words.contains(value)
                }
                Some(("object", inner)) => {
                    conforms(descriptor, inner, value, &path)?;
                    true
                }
                Some(("list", inner)) => {
                    let items = value
                        .as_array()
                        .ok_or_else(|| format!("{path}: not a list"))?;
                    for (index, item) in items.iter().enumerate() {
                        conforms(descriptor, inner, item, &format!("{path}[{index}]"))?;
                    }
                    true
                }
                Some(_) => return Err(format!("{path}: unknown type {declared}")),
            };
            if !accepted {
                return Err(format!("{path}: not {declared}"));
            }
        }
        Ok(())
    }

    #[test]
    fn declared_field_types_describe_exactly_what_is_written_and_read() {
        for (kind, names, types) in FIELD_TYPES {
            assert_eq!(names.len(), types.len(), "{kind}");
        }
        let kinds: Vec<_> = FIELD_TYPES.iter().map(|(kind, _, _)| *kind).collect();
        let mut sorted = kinds.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(kinds, sorted);
        let descriptor = schema_descriptor();
        let descriptor = descriptor.as_object().unwrap();
        let mut record = sample_record();
        record.failures.entries.push(RawFailure {
            phase: 1,
            stage: "commit".into(),
            code: "refused".into(),
        });
        let view = record.to_json_value();
        assert_eq!(conforms(descriptor, "record", &view, "record"), Ok(()));
        assert_eq!(
            conforms(
                descriptor,
                "context",
                &bound_context().to_json_value(),
                "context"
            ),
            Ok(())
        );
        // Optional fields accept null and nothing else of another type.
        let mut cleared = sample_record();
        cleared.address_space.soft_limit_bytes = None;
        cleared.identity.context.source_dirty_digest = None;
        assert_eq!(
            conforms(descriptor, "record", &cleared.to_json_value(), "record"),
            Ok(())
        );
        // The type table rejects what the reader rejects.
        for (path, value, reason) in [
            (
                &["phase_tree", "nodes", "0", "calls"][..],
                Value::Bool(true),
                "record.phase_tree.nodes[0].calls: not u64",
            ),
            (
                &["phase_tree", "nodes", "1", "parent"][..],
                Value::from(u64::from(u32::MAX) + 1),
                "record.phase_tree.nodes[1].parent: not u32?",
            ),
            (
                &["outcome"][..],
                Value::String("maybe".into()),
                "record.outcome: not enum:outcome",
            ),
            (
                &["identity", "source_dirty"][..],
                Value::Null,
                "record.identity.source_dirty: not bool",
            ),
            (
                &["declared"][..],
                Value::Null,
                "record.declared: not a list",
            ),
        ] {
            let mut changed = sample_record().to_json_value();
            *at(&mut changed, path) = value;
            assert_eq!(
                conforms(descriptor, "record", &changed, "record"),
                Err(reason.to_owned())
            );
            assert!(MeasurementRecord::from_json_value(&changed).is_err());
        }
        let mut extra = sample_record().to_json_value();
        at(&mut extra, &["process"])
            .as_object_mut()
            .unwrap()
            .insert("witness".into(), Value::Null);
        assert_eq!(
            conforms(descriptor, "record", &extra, "record"),
            Err("record.process: key set differs".to_owned())
        );
    }

    /// The tracked fixtures other SDK emitters and the harness script read.
    fn fixtures() -> [(&'static str, String); 3] {
        let record = sample_record();
        [
            ("record_schema_v1.json", schema_descriptor_text()),
            (
                "measurement_record_v1.json",
                pretty(&record.to_json_value()),
            ),
            (
                "measurement_record_v1.norito.hex",
                hex(&record.to_norito_bytes().unwrap()),
            ),
        ]
    }

    #[test]
    fn tracked_fixtures_match_the_schema_in_this_source() {
        for (name, expected) in fixtures() {
            let tracked = std::fs::read_to_string(fixture(name)).unwrap_or_default();
            assert!(
                tracked == expected,
                "fixtures/{name} drifted from the schema source; review the change and run \
                 `cargo test -p iroha_measurement --lib -- --ignored regenerate_tracked_fixtures`"
            );
        }
        let golden = std::fs::read_to_string(fixture("measurement_record_v1.json")).unwrap();
        let record = MeasurementRecord::from_json_view(&golden).unwrap();
        assert_eq!(record, sample_record());
        assert_eq!(record.findings(), Vec::new());
    }

    #[test]
    #[ignore = "writes the tracked fixtures after an intentional schema change"]
    fn regenerate_tracked_fixtures() {
        for (name, expected) in fixtures() {
            std::fs::write(fixture(name), expected).unwrap();
        }
    }
}
