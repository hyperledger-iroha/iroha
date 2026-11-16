//! Case model, recording host and runner of the baseline v1 capture.
//!
//! Every record is produced by the anchor's own `ivm` crate: the interpreter, the loader, the
//! `DefaultHost`/`CoreHost`/`WsvHost` test hosts and the fault projection. The Core admission
//! projection comes from `anchor_core_admission`, generated from the anchor's `iroha_core` text.

use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use iroha_data_model::{
    executor::{IvmAdmissionError, fault::IvmFaultPositionV1},
    smart_contract::{ContractAddress, ContractLifecycleControlV1},
};
use iroha_model_base::{name::Name, topology::DataSpaceId};
use ivm::{
    CoreHost, IVM, IVMHost, ProgramMetadata, SyscallPolicy, TraceMode, VMError, VmCycleBudget,
    host::{AccessLog, DefaultHost},
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    parallel::StateAccessSet,
};
use norito::json::{Map, Number, Value};
use sha2::{Digest as _, Sha256};

use crate::anchor_core_admission as core_admission;

/// Signatory of the deterministic fixture account used by the anchor's own mock-host tests.
pub const FIXTURE_SIGNATORY: &str =
    "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774";
/// Network identity used by the anchor's mock contract-call tests.
pub const FIXTURE_NETWORK: &str =
    "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
/// Largest byte string copied verbatim into a record; longer ones are recorded by SHA-256.
const INLINE_BYTES: usize = 96;
/// Syscall events kept verbatim per run; the digest in `syscall_trace` covers all of them.
const INLINE_SYSCALL_EVENTS: usize = 16;

/// JSON unsigned integer.
pub fn n(value: u64) -> Value {
    Value::Number(Number::U64(value))
}
/// JSON string.
pub fn s(value: impl Into<String>) -> Value {
    Value::String(value.into())
}
/// JSON lower-case hex string.
pub fn h(bytes: &[u8]) -> Value {
    Value::String(hex::encode(bytes))
}
/// JSON object from literal fields.
pub fn obj<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
/// A byte string: its length with either the bytes or, above [`INLINE_BYTES`], their SHA-256.
pub fn blob(bytes: &[u8]) -> Value {
    if bytes.len() <= INLINE_BYTES {
        obj([("len", n(bytes.len() as u64)), ("hex", h(bytes))])
    } else {
        obj([
            ("len", n(bytes.len() as u64)),
            ("sha256", s(sha256_hex(bytes))),
        ])
    }
}
/// SHA-256 of `bytes` as lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
/// Pointer-ABI TLV envelope, as the anchor's tests assemble it.
pub fn tlv(pointer_type: ivm::PointerType, payload: &[u8]) -> Vec<u8> {
    ivm::pointer_abi::encode_tlv(pointer_type, payload).expect("capture TLV fits the envelope")
}

/// One setup operation applied after the artifact is loaded and before the run.
#[derive(Clone, Debug)]
pub enum Step {
    /// `vm.set_register(index, value)`.
    SetRegister { index: usize, value: u64 },
    /// `vm.registers.set_tag(index, true)`: a private operand (ZK mode only).
    TagRegister { index: usize },
    /// `vm.store_bytes(addr, bytes)`.
    StoreBytes { addr: u64, bytes: Vec<u8> },
    /// `vm.alloc_input_tlv(tlv)`, then `vm.set_register(register, pointer)`.
    AllocInputTlv { tlv: Vec<u8>, register: usize },
    /// `vm.select_entrypoint(name)`.
    SelectEntrypoint { name: String },
    /// `vm.set_max_cycles(max)`.
    SetMaxCycles { max: u64 },
}

/// Which of the anchor's test hosts serves syscalls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    /// `ivm::host::DefaultHost::new()`.
    Default,
    /// `ivm::CoreHost::new()`.
    Core,
    /// `WsvHost::new_with_subject(MockWorldStateView::new(), fixture account)`.
    Wsv,
    /// As `Wsv`, with the artifact installed as a contract fixture and its runtime context bound.
    WsvContract,
}
impl HostKind {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Core => "core",
            Self::Wsv => "wsv",
            Self::WsvContract => "wsv_contract",
        }
    }
}

/// Node and governance limits of the anchor's Core admission.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// `pipeline.ivm_max_cycles_upper_bound`.
    pub upper_bound: u64,
    /// `SmartContractParameters::fuel`.
    pub fuel: u64,
}
impl Policy {
    /// The anchor's defaults.
    pub fn anchor_default() -> Self {
        Self {
            upper_bound: core_admission::IVM_MAX_CYCLES_UPPER_BOUND,
            fuel: iroha_data_model::parameter::system::Parameters::default()
                .smart_contract()
                .fuel()
                .get(),
        }
    }
}

/// Gas limit of the run.
#[derive(Clone, Copy, Debug)]
pub enum Gas {
    /// A fixed limit.
    Explicit(u64),
    /// The anchor Core's header-to-gas conversion, `gas_limit_for_cycles(effective cycles)`,
    /// when Core admits the artifact; `u64::MAX` when it does not.
    FromHeader,
}

/// One oracle case: an artifact, its setup and how to run it.
#[derive(Clone, Debug)]
pub struct Case {
    /// Stable identifier, unique in the corpus.
    pub id: String,
    /// Artifact path relative to the fixture root.
    pub artifact: String,
    /// Complete program image.
    pub program: Vec<u8>,
    /// What the case is meant to cover (free-form tags such as `opcode:ADD`).
    pub covers: Vec<String>,
    /// Gas limit.
    pub gas: Gas,
    /// Host.
    pub host: HostKind,
    /// Public inputs served by the host (name, TLV envelope).
    pub public_inputs: Vec<(String, Vec<u8>)>,
    /// Canonical JSON the argument record was encoded from, when the case passes arguments.
    pub arguments_json: Option<String>,
    /// Entrypoints run to completion first, in the same VM and host (for example `hajimari`);
    /// after each one the VM is `reset()` and its gas limit restored, as the anchor's
    /// initialise-then-run tests do.
    pub prelude: Vec<String>,
    /// Setup operations, applied after the prelude.
    pub steps: Vec<Step>,
    /// Core admission limits.
    pub policy: Policy,
    /// Shared cycle allowance (`VmCycleBudget`) the prelude and the run draw from, as the anchor
    /// Core binds one allowance to a signed source; `None` runs with `run_with_host`.
    pub cycle_budget: Option<u64>,
    /// Whether the case also runs with `TraceMode::PcOnly`. A program that observes its own
    /// trace (`SYSCALL_EXECUTION_SUMMARY`) legitimately behaves differently under tracing.
    pub traced: bool,
}
impl Case {
    /// Entrypoint the case selects, if any.
    pub fn entrypoint(&self) -> Option<&str> {
        self.steps.iter().find_map(|step| match step {
            Step::SelectEntrypoint { name } => Some(name.as_str()),
            _ => None,
        })
    }
}

fn step_json(step: &Step) -> Value {
    match step {
        Step::SetRegister { index, value } => obj([
            ("op", s("set_register")),
            ("index", n(*index as u64)),
            ("value", n(*value)),
        ]),
        Step::TagRegister { index } => {
            obj([("op", s("tag_register")), ("index", n(*index as u64))])
        }
        Step::StoreBytes { addr, bytes } => obj([
            ("op", s("store_bytes")),
            ("addr", n(*addr)),
            ("hex", h(bytes)),
        ]),
        Step::AllocInputTlv { tlv, register } => obj([
            ("op", s("alloc_input_tlv")),
            ("hex", h(tlv)),
            ("register", n(*register as u64)),
        ]),
        Step::SelectEntrypoint { name } => {
            obj([("op", s("select_entrypoint")), ("name", s(name.clone()))])
        }
        Step::SetMaxCycles { max } => obj([("op", s("set_max_cycles")), ("max", n(*max))]),
    }
}

fn vm_error_json(error: &VMError) -> Value {
    obj([
        ("debug", s(format!("{error:?}"))),
        ("display", s(error.to_string())),
    ])
}

/// Name, wire tag, bare Norito encoding and JSON of one enum value whose bare layout starts
/// with its little-endian `u32` discriminant.
pub fn wire_json<T>(value: &T) -> Value
where
    T: norito::codec::Encode + norito::json::JsonSerialize + core::fmt::Debug,
{
    let bare = norito::codec::Encode::encode(value);
    let tag = u32::from_le_bytes(
        bare.get(..4)
            .and_then(|head| head.try_into().ok())
            .expect("a Norito enum starts with its u32 discriminant"),
    );
    let debug = format!("{value:?}");
    let variant = debug
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_owned();
    obj([
        ("variant", s(variant)),
        ("wire_tag", n(u64::from(tag))),
        ("bare_hex", h(&bare)),
        (
            "json",
            norito::json::to_value(value).expect("Norito JSON value"),
        ),
        ("debug", s(debug)),
    ])
}

/// The anchor Core's admission verdict for `program`: the effective cycle limit or the exact
/// `IvmAdmissionError`. The local ZK backend check is state-dependent and not modelled.
pub fn core_admit(program: &[u8], policy: Policy) -> Result<NonZeroU64, IvmAdmissionError> {
    if let Err(error) = IVM::validate_program(program) {
        return Err(core_admission::admission_reason_from_vm_error(error));
    }
    let metadata = ProgramMetadata::parse(program)
        .map_err(core_admission::admission_reason_from_vm_error)?
        .metadata;
    // The order of `validate_prepared_ivm_execution_policy_with_availability` at the anchor:
    // header policy, governed cycle limits, then the node ceiling again.
    let upper_bound =
        NonZeroU64::new(policy.upper_bound).expect("capture policy upper bound is positive");
    let fuel = NonZeroU64::new(policy.fuel).expect("capture policy fuel is positive");
    core_admission::validate_header_policy(&metadata)?;
    let cycles = core_admission::validate_cycle_limits(&metadata, upper_bound, fuel)?;
    core_admission::validate_cycle_ceiling(&metadata, upper_bound)?;
    Ok(cycles)
}

#[derive(Clone, Default)]
struct Trace(Arc<Mutex<Vec<Value>>>);
impl Trace {
    fn push(&self, value: Value) {
        self.0.lock().expect("capture trace lock").push(value);
    }
    fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.0.lock().expect("capture trace lock"))
    }
}

/// Host wrapper recording every syscall the interpreter dispatches to the wrapped anchor host.
struct Recorder<H> {
    inner: H,
    trace: Trace,
}
impl<H> Recorder<H> {
    fn new(inner: H) -> Self {
        Self {
            inner,
            trace: Trace::default(),
        }
    }
}

fn args_json(registers: &[u64; 256]) -> Value {
    Value::Array(registers[10..=17].iter().map(|value| n(*value)).collect())
}

fn pointer_args_json(vm: &IVM, registers: &[u64; 256]) -> Value {
    let mut out = Vec::new();
    for index in 10..=17 {
        let Ok(tlv) = vm.validate_tlv(registers[index]) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("register".to_owned(), n(index as u64));
        entry.insert("type_id".to_owned(), n(u64::from(tlv.type_id as u16)));
        entry.insert("payload".to_owned(), blob(tlv.payload));
        out.push(Value::Object(entry));
    }
    Value::Array(out)
}

fn gas_result_json(result: &Result<u64, VMError>) -> Value {
    match result {
        Ok(gas) => obj([("ok", n(*gas))]),
        Err(error) => obj([("err", s(format!("{error:?}")))]),
    }
}

impl<H: IVMHost + 'static> IVMHost for Recorder<H> {
    fn prepared_entrypoint_arguments(&self) -> Option<ivm::PreparedArgumentRecord> {
        self.inner.prepared_entrypoint_arguments()
    }
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError> {
        let result = self.inner.prepare_syscall(number, vm);
        self.trace.push(obj([
            ("event", s("prepare")),
            ("number", n(u64::from(number))),
            ("quote", gas_result_json(&result)),
        ]));
        result
    }
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let before = vm.registers.snapshot();
        let gas_before = vm.remaining_gas();
        let pointers = pointer_args_json(vm, &before);
        let result = self.inner.syscall(number, vm);
        let after = vm.registers.snapshot();
        self.trace.push(obj([
            ("event", s("call")),
            ("number", n(u64::from(number))),
            (
                "name",
                ivm::syscalls::syscall_name(number).map_or(Value::Null, s),
            ),
            ("args", args_json(&before)),
            ("pointer_args", pointers),
            ("result", gas_result_json(&result)),
            ("ret", args_json(&after)),
            ("gas_before", n(gas_before)),
            ("gas_after", n(vm.remaining_gas())),
        ]));
        result
    }
    fn allows_syscall(&self, policy: SyscallPolicy, number: u32) -> bool {
        self.inner.allows_syscall(policy, number)
    }
    fn as_any(&mut self) -> &mut dyn std::any::Any
    where
        Self: 'static,
    {
        self.inner.as_any()
    }
    fn begin_tx(&mut self, declared: &StateAccessSet) -> Result<(), VMError> {
        self.inner.begin_tx(declared)
    }
    fn finish_tx(&mut self) -> Result<AccessLog, VMError> {
        self.inner.finish_tx()
    }
    fn checkpoint(&self) -> Option<Box<dyn std::any::Any + Send>> {
        self.inner.checkpoint()
    }
    fn restore(&mut self, snapshot: &dyn std::any::Any) -> Result<(), VMError> {
        self.inner.restore(snapshot)
    }
    fn access_logging_supported(&self) -> bool {
        self.inner.access_logging_supported()
    }
}

/// The fixture account every ledger-host case runs as.
pub fn fixture_account() -> AccountId {
    AccountId::new(
        FIXTURE_SIGNATORY
            .parse()
            .expect("fixture public key literal"),
    )
}

fn public_inputs(case: &Case) -> BTreeMap<Name, Vec<u8>> {
    case.public_inputs
        .iter()
        .map(|(name, tlv)| {
            (
                name.parse::<Name>().expect("capture public input name"),
                tlv.clone(),
            )
        })
        .collect()
}

fn run_recorded<H: IVMHost + 'static>(
    vm: &mut IVM,
    recorder: &mut Recorder<H>,
    budget: Option<&VmCycleBudget>,
) -> Result<Result<(), VMError>, String> {
    catch_unwind(AssertUnwindSafe(|| match budget {
        Some(budget) => vm.run_with_host_and_cycle_budget(recorder, budget),
        None => vm.run_with_host(recorder),
    }))
    .map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_else(|| "non-string panic payload".to_owned())
    })
}

fn apply_steps(vm: &mut IVM, steps: &[Step]) -> Result<(), (usize, VMError)> {
    for (index, step) in steps.iter().enumerate() {
        let applied = match step {
            Step::SetRegister { index, value } => {
                vm.set_register(*index, *value);
                Ok(())
            }
            Step::TagRegister { index } => {
                vm.registers.set_tag(*index, true);
                Ok(())
            }
            Step::StoreBytes { addr, bytes } => vm.store_bytes(*addr, bytes),
            Step::AllocInputTlv { tlv, register } => vm.alloc_input_tlv(tlv).map(|pointer| {
                vm.set_register(*register, pointer);
            }),
            Step::SelectEntrypoint { name } => vm.select_entrypoint(name),
            Step::SetMaxCycles { max } => {
                vm.set_max_cycles(*max);
                Ok(())
            }
        };
        applied.map_err(|error| (index, error))?;
    }
    Ok(())
}

fn summary_json(vm: &mut IVM) -> Value {
    let summary = vm.execution_summary();
    obj([
        ("version", n(u64::from(summary.version))),
        ("code_hash", h(&summary.code_hash)),
        ("final_register_root", h(&summary.final_register_root)),
        ("final_memory_root", h(&summary.final_memory_root)),
        ("output_hash", h(&summary.output_hash)),
        ("pc_trace_hash", h(&summary.pc_trace_hash)),
        ("delta_trace_hash", h(&summary.delta_trace_hash)),
        ("register_trace_hash", h(&summary.register_trace_hash)),
        ("constraint_hash", h(&summary.constraint_hash)),
        ("memory_log_hash", h(&summary.memory_log_hash)),
        ("register_log_hash", h(&summary.register_log_hash)),
        ("step_log_hash", h(&summary.step_log_hash)),
        ("cycles", n(summary.cycles)),
        ("max_cycles", n(summary.max_cycles)),
        ("gas_used", n(summary.gas_used)),
        ("gas_remaining", n(summary.gas_remaining)),
        ("pc_trace_len", n(summary.pc_trace_len)),
        ("delta_trace_len", n(summary.delta_trace_len)),
        ("register_trace_len", n(summary.register_trace_len)),
        ("constraint_len", n(summary.constraint_len)),
        ("memory_log_len", n(summary.memory_log_len)),
        ("register_log_len", n(summary.register_log_len)),
        ("step_log_len", n(summary.step_log_len)),
        ("zk_mode", Value::Bool(summary.zk_mode)),
        ("halted", Value::Bool(summary.halted)),
        ("constraint_failed", Value::Bool(summary.constraint_failed)),
    ])
}

fn return_json(vm: &IVM, program: &[u8], entrypoint: Option<&str>) -> Value {
    let Ok(count) = vm.call_result_word_count() else {
        return Value::Null;
    };
    let words = (0..count)
        .map(|index| match vm.public_call_result_word(index) {
            Ok(word) => n(word),
            Err(error) => obj([("err", s(format!("{error:?}")))]),
        })
        .collect();
    let mut out = Map::new();
    out.insert("word_count".to_owned(), n(count as u64));
    out.insert("words".to_owned(), Value::Array(words));
    let schema = entrypoint.and_then(|name| {
        ProgramMetadata::parse(program)
            .ok()?
            .contract_interface?
            .entrypoints
            .into_iter()
            .find(|entry| entry.name == name)?
            .return_schema
    });
    if let Some(schema) = schema {
        match ivm::value_record::encode_entrypoint_return_record_bytes(vm, &schema) {
            Ok(bytes) => {
                out.insert("record".to_owned(), blob(&bytes));
                let rendered = ivm::value_record::decode_entrypoint_return_record(&schema, &bytes)
                    .and_then(|record| {
                        ivm::value_record::render_entrypoint_return_record(&schema, &record)
                    });
                match rendered {
                    Ok(value) => {
                        out.insert("rendered".to_owned(), value);
                    }
                    Err(error) => {
                        out.insert("render_error".to_owned(), s(format!("{error:?}")));
                    }
                }
            }
            Err(error) => {
                out.insert("record_error".to_owned(), s(format!("{error:?}")));
            }
        }
    }
    Value::Object(out)
}

fn result_json(vm: &IVM, result: &Result<(), VMError>) -> Value {
    let error = match result {
        Ok(()) => return obj([("ok", Value::Bool(true))]),
        Err(error) => error,
    };
    let mut out = Map::new();
    out.insert("ok".to_owned(), Value::Bool(false));
    out.insert("error".to_owned(), vm_error_json(error));
    out.insert(
        "unmetered_debug".to_owned(),
        s(format!("{:?}", error.as_unmetered())),
    );
    out.insert(
        "metered_gas".to_owned(),
        error.metered_gas().map_or(Value::Null, n),
    );
    out.insert(
        "execution_deferral".to_owned(),
        error
            .execution_deferral()
            .map_or(Value::Null, |reason| s(format!("{reason:?}"))),
    );
    out.insert(
        "fault_kind".to_owned(),
        error
            .fault_kind()
            .map_or(Value::Null, |kind| wire_json(&kind)),
    );
    // The position argument is the one the anchor Core passes
    // (`map_vm_error_with_context_to_validation`); a trap snapshot overrides it.
    out.insert(
        "fault".to_owned(),
        vm.execution_fault(error, IvmFaultPositionV1::ReturnValidation)
            .map_or(Value::Null, |fault| {
                obj([
                    (
                        "json",
                        norito::json::to_value(&fault).expect("fault JSON value"),
                    ),
                    ("bare_hex", h(&norito::codec::Encode::encode(&fault))),
                    (
                        "canonical_hex",
                        h(&norito::encode_canonical(&fault).expect("canonical fault frame")),
                    ),
                    ("display", s(fault.to_string())),
                ])
            }),
    );
    Value::Object(out)
}

fn state_json(vm: &mut IVM) -> Value {
    let registers = vm.registers.snapshot();
    let tags = vm.registers.snapshot_tags();
    obj([
        ("pc", n(vm.pc)),
        ("cycles", n(vm.get_cycle_count())),
        ("gas_remaining", n(vm.remaining_gas())),
        ("vector_length", n(vm.vector_length() as u64)),
        (
            "registers",
            Value::Array(
                registers
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| **value != 0)
                    .map(|(index, value)| Value::Array(vec![n(index as u64), n(*value)]))
                    .collect(),
            ),
        ),
        (
            "private_registers",
            Value::Array(
                tags.iter()
                    .enumerate()
                    .filter(|(_, tag)| **tag)
                    .map(|(index, _)| n(index as u64))
                    .collect(),
            ),
        ),
        ("register_root", h(&vm.register_root())),
        ("output", blob(vm.read_output_used())),
        ("summary", summary_json(vm)),
    ])
}

fn pc_trace_json(vm: &IVM) -> (Value, BTreeMap<u8, u64>) {
    let pcs = vm.trace_pcs();
    let mut digest = Sha256::new();
    let mut opcodes = BTreeMap::<u8, u64>::new();
    for pc in pcs {
        digest.update(pc.to_le_bytes());
        if let Ok(word) = ivm::decode(&vm.memory, *pc) {
            *opcodes
                .entry(ivm::instruction::wide::opcode(word))
                .or_default() += 1;
        }
    }
    let histogram = opcodes
        .iter()
        .map(|(opcode, count)| (format!("{opcode:02x}"), n(*count)))
        .collect::<Map>();
    (
        obj([
            ("pc_count", n(pcs.len() as u64)),
            ("pc_sha256", s(hex::encode(digest.finalize()))),
            ("executed_opcodes", Value::Object(histogram)),
        ]),
        opcodes,
    )
}

/// Result of running one case once.
pub struct Outcome {
    /// The `run` member of the oracle record (`null` when the artifact did not load).
    pub run: Value,
    /// Executed-instruction trace summary; only for a `TraceMode::PcOnly` run that executed.
    pub trace: Option<Value>,
    /// Executed opcode counts of a traced run.
    pub opcodes: BTreeMap<u8, u64>,
    /// Whether the interpreter or host panicked.
    pub panicked: bool,
}

fn loaded_run<H: IVMHost + 'static>(
    case: &Case,
    vm: &mut IVM,
    host: H,
    trace_mode: TraceMode,
    post: impl FnOnce(&mut H) -> Vec<(&'static str, Value)>,
) -> Outcome {
    let mut recorder = Recorder::new(host);
    let budget = case.cycle_budget.map(|limit| {
        VmCycleBudget::new(NonZeroU64::new(limit).expect("capture cycle budget is positive"))
    });
    let mut prelude = Vec::new();
    for name in &case.prelude {
        if let Err(error) = vm.select_entrypoint(name) {
            return not_loaded(&format!("prelude select_entrypoint({name})"), &error);
        }
        let outcome = run_recorded(vm, &mut recorder, budget.as_ref());
        let events = recorder.trace.take().len();
        prelude.push(obj([
            ("entrypoint", s(name.clone())),
            (
                "result",
                match &outcome {
                    Ok(result) => result_json(vm, result),
                    Err(message) => obj([("panic", s(message.clone()))]),
                },
            ),
            ("cycles", n(vm.get_cycle_count())),
            ("gas_remaining", n(vm.remaining_gas())),
            ("syscall_events", n(events as u64)),
        ]));
        if let Err(error) = vm.reset() {
            return not_loaded(&format!("prelude reset after {name}"), &error);
        }
        vm.set_gas_limit(gas_limit(case));
    }
    if let Err((index, error)) = apply_steps(vm, &case.steps) {
        return not_loaded(&format!("setup step {index}"), &error);
    }
    if trace_mode != TraceMode::Off {
        vm.set_trace_mode(trace_mode);
    }
    let outcome = run_recorded(vm, &mut recorder, budget.as_ref());
    let syscalls = recorder.trace.take();
    let mut host = recorder.inner;
    let mut out = Map::new();
    let panicked = match &outcome {
        Ok(result) => {
            out.insert("result".to_owned(), result_json(vm, result));
            false
        }
        Err(message) => {
            out.insert("panic".to_owned(), s(message.clone()));
            true
        }
    };
    let Value::Object(state) = state_json(vm) else {
        unreachable!("state_json builds an object");
    };
    out.extend(state);
    out.insert(
        "return".to_owned(),
        match &outcome {
            Ok(Ok(())) => return_json(vm, &case.program, case.entrypoint()),
            _ => Value::Null,
        },
    );
    out.insert("prelude".to_owned(), Value::Array(prelude));
    out.insert(
        "cycle_budget".to_owned(),
        budget.as_ref().map_or(Value::Null, |budget| {
            obj([
                ("limit", n(budget.limit())),
                ("consumed", n(budget.consumed())),
                ("remaining", n(budget.remaining())),
                ("is_open", Value::Bool(budget.is_open())),
                ("exhausted", Value::Bool(budget.exhausted())),
            ])
        }),
    );
    // The digest covers every event (one compact JSON document per line); the first
    // `INLINE_SYSCALL_EVENTS` are also kept verbatim.
    let mut digest = Sha256::new();
    for event in &syscalls {
        digest.update(norito::json::to_string(event).expect("syscall event JSON"));
        digest.update(b"\n");
    }
    out.insert(
        "syscall_trace".to_owned(),
        obj([
            ("events", n(syscalls.len() as u64)),
            ("sha256", s(hex::encode(digest.finalize()))),
        ]),
    );
    let mut syscalls = syscalls;
    syscalls.truncate(INLINE_SYSCALL_EVENTS);
    out.insert("syscalls".to_owned(), Value::Array(syscalls));
    for (key, value) in post(&mut host) {
        out.insert(key.to_owned(), value);
    }
    let (trace, opcodes) = if trace_mode == TraceMode::PcOnly {
        let (trace, opcodes) = pc_trace_json(vm);
        (Some(trace), opcodes)
    } else {
        (None, BTreeMap::new())
    };
    Outcome {
        run: Value::Object(out),
        trace,
        opcodes,
        panicked,
    }
}

fn not_loaded(stage: &str, error: &VMError) -> Outcome {
    Outcome {
        run: obj([
            ("not_run", s(stage)),
            ("error", vm_error_json(error)),
            (
                "fault_kind",
                error
                    .fault_kind()
                    .map_or(Value::Null, |kind| wire_json(&kind)),
            ),
        ]),
        trace: None,
        opcodes: BTreeMap::new(),
        panicked: false,
    }
}

/// Gas limit the case runs with.
pub fn gas_limit(case: &Case) -> u64 {
    match case.gas {
        Gas::Explicit(limit) => limit,
        Gas::FromHeader => core_admit(&case.program, case.policy)
            .map_or(u64::MAX, core_admission::gas_limit_for_cycles),
    }
}

/// Run `case` once on a fresh anchor VM.
pub fn run_case(case: &Case, trace_mode: TraceMode) -> Outcome {
    let mut vm = IVM::deterministic_builder(gas_limit(case))
        .suppress_startup_banner()
        .build();
    match case.host {
        HostKind::Default | HostKind::Core | HostKind::Wsv => {
            if let Err(error) = vm.load_program(&case.program) {
                return not_loaded("load_program", &error);
            }
            match case.host {
                HostKind::Default => loaded_run(
                    case,
                    &mut vm,
                    DefaultHost::new().with_public_inputs(public_inputs(case)),
                    trace_mode,
                    |_| Vec::new(),
                ),
                HostKind::Core => loaded_run(case, &mut vm, CoreHost::new(), trace_mode, |host| {
                    let state = host
                        .state_paths()
                        .into_iter()
                        .map(|path| {
                            let value = host.state_bytes(&path).unwrap_or_default();
                            obj([("path", s(path)), ("value", blob(&value))])
                        })
                        .collect();
                    vec![("host_state", Value::Array(state))]
                }),
                _ => loaded_run(
                    case,
                    &mut vm,
                    WsvHost::new_with_subject(MockWorldStateView::new(), fixture_account())
                        .with_public_inputs(public_inputs(case)),
                    trace_mode,
                    wsv_events,
                ),
            }
        }
        HostKind::WsvContract => {
            let account = fixture_account();
            let mut host = WsvHost::new_with_subject(MockWorldStateView::new(), account.clone())
                .with_public_inputs(public_inputs(case));
            let hash = ivm::contract_code_hash(&case.program);
            let address = ContractAddress::derive(
                &FIXTURE_NETWORK.parse().expect("fixture network identity"),
                &account,
                0,
                DataSpaceId::UNIVERSAL,
            )
            .expect("fixture contract address");
            let mut lifecycle = ContractLifecycleControlV1::direct(account.clone());
            lifecycle.active_code_hash = Some(hash);
            lifecycle.retained_code_hash = Some(hash);
            if let Err(error) = host.install_contract_fixture(
                address.clone(),
                case.program.clone(),
                lifecycle,
                None,
            ) {
                return not_loaded("install_contract_fixture", &error);
            }
            let prepared = host
                .contract_fixture(&address)
                .expect("installed contract fixture")
                .clone();
            if let Err(error) = vm.load_prepared(&prepared) {
                return not_loaded("load_prepared", &error);
            }
            assert!(
                case.prelude.is_empty(),
                "a contract-fixture case binds one entrypoint and takes no prelude"
            );
            let entrypoint = case
                .entrypoint()
                .expect("a contract-fixture case selects an entrypoint")
                .to_owned();
            host.bind_contract_runtime_context(account, address, entrypoint)
                .expect("bind fixture contract runtime context");
            loaded_run(case, &mut vm, host, trace_mode, wsv_events)
        }
    }
}

fn wsv_events(host: &mut WsvHost) -> Vec<(&'static str, Value)> {
    let events = host
        .drain_contract_events()
        .iter()
        .map(|event| {
            let contract: &str = event.contract().as_ref();
            let name: &str = event.name().as_ref();
            obj([
                ("contract", s(contract)),
                ("name", s(name)),
                (
                    "payload",
                    blob(
                        &norito::encode_canonical(event.payload())
                            .expect("canonical event payload"),
                    ),
                ),
            ])
        })
        .collect();
    vec![("events", Value::Array(events))]
}

/// The setup half of an oracle record.
pub fn setup_json(case: &Case) -> Value {
    obj([
        ("gas_limit", n(gas_limit(case))),
        (
            "gas_limit_source",
            s(match case.gas {
                Gas::Explicit(_) => "explicit",
                Gas::FromHeader => "header_cycles",
            }),
        ),
        ("host", s(case.host.name())),
        (
            "public_inputs",
            Value::Array(
                case.public_inputs
                    .iter()
                    .map(|(name, tlv)| obj([("name", s(name.clone())), ("tlv_hex", h(tlv))]))
                    .collect(),
            ),
        ),
        (
            "arguments_json",
            case.arguments_json.clone().map_or(Value::Null, s),
        ),
        (
            "prelude",
            Value::Array(case.prelude.iter().cloned().map(s).collect()),
        ),
        (
            "steps",
            Value::Array(case.steps.iter().map(step_json).collect()),
        ),
        (
            "policy",
            obj([
                ("max_cycles_upper_bound", n(case.policy.upper_bound)),
                ("fuel", n(case.policy.fuel)),
            ]),
        ),
        ("cycle_budget", case.cycle_budget.map_or(Value::Null, n)),
    ])
}

/// The admission half of an oracle record.
pub fn admission_json(case: &Case) -> Value {
    let vm = match IVM::validate_program(&case.program) {
        Ok(()) => obj([("ok", Value::Bool(true))]),
        Err(error) => obj([("ok", Value::Bool(false)), ("error", vm_error_json(&error))]),
    };
    let core = match core_admit(&case.program, case.policy) {
        Ok(cycles) => obj([
            ("ok", Value::Bool(true)),
            ("effective_cycles", n(cycles.get())),
            (
                "gas_limit_for_cycles",
                n(core_admission::gas_limit_for_cycles(cycles)),
            ),
        ]),
        Err(error) => obj([
            ("ok", Value::Bool(false)),
            ("display", s(error.to_string())),
            ("error", wire_json(&error)),
        ]),
    };
    obj([("vm", vm), ("core", core)])
}
