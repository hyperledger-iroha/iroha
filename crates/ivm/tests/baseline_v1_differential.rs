//! Differential replay of the frozen baseline v1 oracle on the baseline engine.
//!
//! Every record of `tests/fixtures/baseline_v1/oracle/oracle.jsonl` was produced by the pre-C10
//! anchor `77600a4840` (`scripts/ivm_baseline_v1/capture/oracle.rs`). This test rebuilds each
//! record with today's [`ivm::IvmEngine::Baseline`] engine, reached only through the explicit
//! [`IvmProfileId::BASELINE_ABI_V1`] identity, and requires every recorded field to be equal:
//! artifact bytes, VM and Core admission verdicts, results, internal error text, fault kinds and
//! their original wire bytes, the canonical fault frame, PC, cycles, gas, the 256-register file,
//! private tags, register and memory roots, output, the complete execution summary, return
//! records, prelude runs, the shared cycle allowance, every dispatched syscall, host state and
//! contract events, and the PC trace of traced cases.
//!
//! Wire values are the data model's restored original `IvmFaultKindV1`, `IvmFaultV1` and
//! `IvmAdmissionError`; the comparison is byte-for-byte against the anchor's encoding.
use std::{
    collections::BTreeMap,
    fs,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use iroha_data_model::{
    executor::{
        IvmAdmissionError,
        fault::{IvmFaultKindV1, IvmFaultPositionV1, IvmFaultV1},
    },
    smart_contract::{ContractAddress, ContractLifecycleControlV1},
};
use iroha_model_base::{name::Name, topology::DataSpaceId};
use ivm::{
    CoreHost, IVM, IVMHost, IvmProfileId, ProgramMetadata, SyscallPolicy, TraceMode, VMError,
    VmCycleBudget,
    host::{AccessLog, DefaultHost},
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    parallel::StateAccessSet,
};
use ivm_abi::baseline as base;
use norito::json::{Map, Number, Value};
use sha2::{Digest as _, Sha256};

/// Signatory of the deterministic fixture account the oracle's ledger hosts run as.
const FIXTURE_SIGNATORY: &str =
    "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774";
/// Network identity of the oracle's contract-fixture address.
const FIXTURE_NETWORK: &str =
    "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
/// Largest byte string the oracle copies verbatim.
const INLINE_BYTES: usize = 96;
/// Syscall events the oracle keeps verbatim per run.
const INLINE_SYSCALL_EVENTS: usize = 16;
/// The profile every case runs under.
const PROFILE: IvmProfileId = IvmProfileId::BASELINE_ABI_V1;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/baseline_v1")
}
fn n(value: u64) -> Value {
    Value::Number(Number::U64(value))
}
fn s(value: impl Into<String>) -> Value {
    Value::String(value.into())
}
fn h(bytes: &[u8]) -> Value {
    Value::String(hex::encode(bytes))
}
fn obj<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn blob(bytes: &[u8]) -> Value {
    if bytes.len() <= INLINE_BYTES {
        obj([("len", n(bytes.len() as u64)), ("hex", h(bytes))])
    } else {
        obj([
            ("len", n(bytes.len() as u64)),
            ("sha256", s(sha256_hex(bytes))),
        ])
    }
}
fn member<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |value, key| value.as_object()?.get(*key))
}
fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    member(value, path)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("`{}` must be a string", path.join(".")))
}
fn number(value: &Value, path: &[&str]) -> u64 {
    member(value, path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("`{}` must be an unsigned integer", path.join(".")))
}
fn hex_bytes(value: &Value, path: &[&str]) -> Vec<u8> {
    hex::decode(text(value, path)).expect("oracle hex")
}

/// Original wire tag of an enum value: the little-endian `u32` its bare encoding starts with.
fn wire_tag(value: &impl norito::codec::Encode) -> u32 {
    let bare = norito::codec::Encode::encode(value);
    u32::from_le_bytes(bare[..4].try_into().expect("an enum encodes its tag first"))
}
/// Name, original wire tag, bare encoding, JSON and debug text of a baseline fault kind.
fn fault_kind_json(kind: IvmFaultKindV1) -> Value {
    let debug = format!("{kind:?}");
    let variant = debug
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_owned();
    let inner = match kind {
        IvmFaultKindV1::Numeric(code) => norito::json::to_value(&code).expect("numeric fault JSON"),
        IvmFaultKindV1::PointerAbi(code) => {
            norito::json::to_value(&code).expect("pointer fault JSON")
        }
        _ => Value::Null,
    };
    obj([
        ("variant", s(variant.clone())),
        ("wire_tag", n(u64::from(wire_tag(&kind)))),
        ("bare_hex", h(&norito::codec::Encode::encode(&kind))),
        ("json", obj([("kind", s(variant)), ("value", inner)])),
        ("debug", s(debug)),
    ])
}
/// The same record for a baseline admission rejection.
fn admission_error_json(error: &IvmAdmissionError) -> Value {
    let debug = format!("{error:?}");
    let variant = debug
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_owned();
    let content = match error {
        IvmAdmissionError::MissingMaxCycles
        | IvmAdmissionError::ManifestCodeHashMissing
        | IvmAdmissionError::ManifestAbiHashMissing
        | IvmAdmissionError::ManifestMalformed => Ok(Value::Null),
        IvmAdmissionError::UnsupportedVersion(info) => norito::json::to_value(info),
        IvmAdmissionError::UnsupportedFeatureBits(value)
        | IvmAdmissionError::UnsupportedAbiVersion(value)
        | IvmAdmissionError::AbiVersionNotActive(value) => Ok(n(u64::from(*value))),
        IvmAdmissionError::VectorLengthTooLarge(info) => Ok(obj([
            ("vector_length", n(u64::from(info.vector_length))),
            ("max_allowed", n(u64::from(info.max_allowed))),
        ])),
        IvmAdmissionError::MaxCyclesExceedsUpperBound(info) => norito::json::to_value(info),
        IvmAdmissionError::MaxCyclesExceedsFuel(info) => Ok(obj([
            ("max_cycles", n(info.max_cycles)),
            ("fuel_limit", n(info.fuel_limit)),
        ])),
        IvmAdmissionError::DecodedInstructionCountExceeded(info) => norito::json::to_value(info),
        IvmAdmissionError::DecodedCodeSizeExceeded(info) => norito::json::to_value(info),
        IvmAdmissionError::BytecodeDecodingFailed(message) => Ok(s(message.clone())),
        IvmAdmissionError::ManifestCodeHashMismatch(info) => norito::json::to_value(info),
        IvmAdmissionError::ManifestAbiHashMismatch(info) => norito::json::to_value(info),
        IvmAdmissionError::ArtifactAbiHashMismatch(info) => norito::json::to_value(info),
        IvmAdmissionError::GenericSyscallNotAllowed(syscall) => Ok(n(u64::from(*syscall))),
    }
    .expect("admission payload JSON");
    obj([
        ("variant", s(variant.clone())),
        ("wire_tag", n(u64::from(wire_tag(error)))),
        ("bare_hex", h(&norito::codec::Encode::encode(error))),
        ("json", obj([("kind", s(variant)), ("content", content)])),
        ("debug", s(debug)),
    ])
}
fn vm_error_json(error: &VMError) -> Value {
    obj([
        ("debug", s(format!("{error:?}"))),
        ("display", s(error.to_string())),
    ])
}

/// The baseline header policy of the anchor Core, in its admission order.
fn core_admit(
    program: &[u8],
    upper_bound: u64,
    fuel: u64,
) -> Result<NonZeroU64, IvmAdmissionError> {
    if let Err(error) = IVM::validate_program_for_profile(program, PROFILE) {
        return Err(base::admission_reason(error));
    }
    let header = ProgramMetadata::parse_header_for_profile(program, PROFILE)
        .map_err(base::admission_reason)?;
    let vector_length = header.vector_length();
    let metadata = header
        .parse_sections()
        .map_err(base::admission_reason)?
        .metadata;
    let upper_bound = NonZeroU64::new(upper_bound).expect("positive upper bound");
    let fuel = NonZeroU64::new(fuel).expect("positive fuel");
    base::validate_header_policy(&metadata, vector_length)?;
    let cycles = base::validate_cycle_limits(&metadata, upper_bound, fuel)?;
    base::validate_cycle_ceiling(&metadata, upper_bound)?;
    Ok(cycles)
}
fn gas_limit_for_cycles(cycles: NonZeroU64) -> u64 {
    cycles
        .get()
        .saturating_mul(ivm::gas::baseline::max_instruction_cost())
}

fn admission_json(program: &[u8], upper_bound: u64, fuel: u64) -> Value {
    let vm = match IVM::validate_program_for_profile(program, PROFILE) {
        Ok(()) => obj([("ok", Value::Bool(true))]),
        Err(error) => obj([("ok", Value::Bool(false)), ("error", vm_error_json(&error))]),
    };
    let core = match core_admit(program, upper_bound, fuel) {
        Ok(cycles) => obj([
            ("ok", Value::Bool(true)),
            ("effective_cycles", n(cycles.get())),
            ("gas_limit_for_cycles", n(gas_limit_for_cycles(cycles))),
        ]),
        Err(error) => obj([
            ("ok", Value::Bool(false)),
            ("display", s(error.to_string())),
            ("error", admission_error_json(&error)),
        ]),
    };
    obj([("vm", vm), ("core", core)])
}

#[derive(Clone, Default)]
struct Trace(Arc<Mutex<Vec<Value>>>);
impl Trace {
    fn push(&self, value: Value) {
        self.0.lock().expect("trace lock").push(value);
    }
    fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.0.lock().expect("trace lock"))
    }
}
/// The oracle's recording host wrapper.
struct Recorder<H> {
    inner: H,
    trace: Trace,
}
fn public_args(vm: &dyn ivm::host::SyscallContext) -> Result<[u64; 8], VMError> {
    let mut words = [0; 8];
    for (index, word) in words.iter_mut().enumerate() {
        *word = vm.public_register(index + 10)?;
    }
    Ok(words)
}
fn args_json(registers: &[u64; 8]) -> Value {
    Value::Array(registers.iter().map(|value| n(*value)).collect())
}
fn pointer_args_json(vm: &dyn ivm::host::SyscallContext, registers: &[u64; 8]) -> Value {
    let mut out = Vec::new();
    for (index, pointer) in registers.iter().enumerate() {
        let Ok(tlv) = vm.public_tlv(*pointer) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("register".to_owned(), n((index + 10) as u64));
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
    fn prepare_syscall(
        &self,
        number: u32,
        vm: ivm::host::SyscallQuote<'_>,
    ) -> Result<u64, VMError> {
        let result = self.inner.prepare_syscall(number, vm);
        self.trace.push(obj([
            ("event", s("prepare")),
            ("number", n(u64::from(number))),
            ("quote", gas_result_json(&result)),
        ]));
        result
    }
    fn syscall(
        &mut self,
        number: u32,
        vm: &mut dyn ivm::host::SyscallContext,
    ) -> Result<u64, VMError> {
        let before = public_args(vm)?;
        let gas_before = vm.quote().remaining_gas();
        let pointers = pointer_args_json(vm, &before);
        let result = self.inner.syscall(number, vm);
        let after = public_args(vm)?;
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
            ("gas_after", n(vm.quote().remaining_gas())),
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

fn fixture_account() -> AccountId {
    AccountId::new(FIXTURE_SIGNATORY.parse().expect("fixture public key"))
}
fn public_inputs(setup: &Value) -> BTreeMap<Name, Vec<u8>> {
    member(setup, &["public_inputs"])
        .and_then(Value::as_array)
        .expect("public inputs")
        .iter()
        .map(|input| {
            (
                text(input, &["name"]).parse::<Name>().expect("input name"),
                hex_bytes(input, &["tlv_hex"]),
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

fn apply_steps(vm: &mut IVM, steps: &[Value]) -> Result<(), (usize, Box<VMError>)> {
    for (index, step) in steps.iter().enumerate() {
        let applied = match text(step, &["op"]) {
            "set_register" => {
                vm.set_register(
                    usize::try_from(number(step, &["index"])).expect("register"),
                    number(step, &["value"]),
                );
                Ok(())
            }
            "tag_register" => {
                vm.registers.set_tag(
                    usize::try_from(number(step, &["index"])).expect("register"),
                    true,
                );
                Ok(())
            }
            "store_bytes" => vm.store_bytes(number(step, &["addr"]), &hex_bytes(step, &["hex"])),
            "alloc_input_tlv" => {
                let register = usize::try_from(number(step, &["register"])).expect("register");
                vm.alloc_input_tlv(&hex_bytes(step, &["hex"]))
                    .map(|pointer| vm.set_register(register, pointer))
            }
            "select_entrypoint" => vm.select_entrypoint(text(step, &["name"])),
            "set_max_cycles" => {
                vm.set_max_cycles(number(step, &["max"]));
                Ok(())
            }
            other => panic!("unknown setup step {other}"),
        };
        applied.map_err(|error| (index, Box::new(error)))?;
    }
    Ok(())
}

fn summary_json(vm: &mut IVM) -> Value {
    let summary = vm
        .baseline_execution_summary()
        .expect("summary memory root fits the test host");
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
        ProgramMetadata::parse_for_profile(program, PROFILE)
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
                let rendered =
                    match ivm::value_record::decode_entrypoint_return_record(&schema, &bytes) {
                        Ok(record) => {
                            ivm::value_record::render_entrypoint_return_record(&schema, &record)
                        }
                        Err(error) => Err(error),
                    };
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

/// The anchor's `IvmFaultV1` canonical frame: the original bare bytes under the unchanged
/// `IvmFaultV1` schema identity and the canonical layout flags.
fn canonical_fault_frame(fault: &IvmFaultV1) -> Vec<u8> {
    let bare = norito::codec::Encode::encode(fault);
    norito::core::frame_bare_with_header_flags::<IvmFaultV1>(
        &bare,
        norito::core::default_encode_flags(),
    )
    .expect("canonical fault frame")
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
        base::fault_kind(error).map_or(Value::Null, fault_kind_json),
    );
    out.insert(
        "fault".to_owned(),
        vm.execution_fault(error, IvmFaultPositionV1::ReturnValidation)
            .map_or(Value::Null, |fault| {
                let mut json = Map::new();
                json.insert(
                    "kind".to_owned(),
                    member(&fault_kind_json(fault.kind), &["json"])
                        .cloned()
                        .expect("kind JSON"),
                );
                json.insert(
                    "site".to_owned(),
                    norito::json::to_value(&fault.site).expect("site JSON"),
                );
                obj([
                    ("json", Value::Object(json)),
                    ("bare_hex", h(&norito::codec::Encode::encode(&fault))),
                    ("canonical_hex", h(&canonical_fault_frame(&fault))),
                    ("display", s(fault.to_string())),
                ])
            }),
    );
    Value::Object(out)
}

fn state_json(vm: &mut IVM) -> Value {
    let registers = vm.registers.baseline_snapshot();
    let tags = vm.registers.baseline_snapshot_tags();
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

fn pc_trace_json(vm: &IVM) -> Value {
    let pcs = vm.trace_pcs();
    let mut digest = Sha256::new();
    let mut opcodes = BTreeMap::<u8, u64>::new();
    for pc in pcs {
        digest.update(pc.to_le_bytes());
        if let Ok(word) = ivm::decode(&vm.memory, *pc) {
            *opcodes
                .entry(base::instruction::wide::opcode(word))
                .or_default() += 1;
        }
    }
    let histogram = opcodes
        .iter()
        .map(|(opcode, count)| (format!("{opcode:02x}"), n(*count)))
        .collect::<Map>();
    obj([
        ("pc_count", n(pcs.len() as u64)),
        ("pc_sha256", s(hex::encode(digest.finalize()))),
        ("executed_opcodes", Value::Object(histogram)),
    ])
}

fn not_loaded(stage: &str, error: &VMError) -> Value {
    obj([
        ("not_run", s(stage)),
        ("error", vm_error_json(error)),
        (
            "fault_kind",
            base::fault_kind(error).map_or(Value::Null, fault_kind_json),
        ),
    ])
}

/// One replayed case: the `run` record and, for a PC-traced run, its trace summary.
struct Replay {
    run: Value,
    trace: Option<Value>,
}

fn loaded_run<H: IVMHost + 'static>(
    record: &Value,
    vm: &mut IVM,
    host: H,
    trace_mode: TraceMode,
    post: impl FnOnce(&mut H) -> Vec<(&'static str, Value)>,
) -> Replay {
    let setup = member(record, &["setup"]).expect("setup");
    let gas_limit = number(setup, &["gas_limit"]);
    let mut recorder = Recorder {
        inner: host,
        trace: Trace::default(),
    };
    let budget = member(setup, &["cycle_budget"])
        .and_then(Value::as_u64)
        .map(|limit| VmCycleBudget::new(NonZeroU64::new(limit).expect("positive budget")));
    let mut prelude = Vec::new();
    for name in member(setup, &["prelude"])
        .and_then(Value::as_array)
        .expect("prelude")
    {
        let name = name.as_str().expect("prelude entrypoint");
        if let Err(error) = vm.select_entrypoint(name) {
            return Replay {
                run: not_loaded(&format!("prelude select_entrypoint({name})"), &error),
                trace: None,
            };
        }
        let outcome = run_recorded(vm, &mut recorder, budget.as_ref());
        let events = recorder.trace.take().len();
        prelude.push(obj([
            ("entrypoint", s(name)),
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
            return Replay {
                run: not_loaded(&format!("prelude reset after {name}"), &error),
                trace: None,
            };
        }
        vm.set_gas_limit(gas_limit);
    }
    let steps = member(setup, &["steps"])
        .and_then(Value::as_array)
        .expect("steps");
    if let Err((index, error)) = apply_steps(vm, steps) {
        return Replay {
            run: not_loaded(&format!("setup step {index}"), &error),
            trace: None,
        };
    }
    if trace_mode != TraceMode::Off {
        vm.set_trace_mode(trace_mode);
    }
    let outcome = run_recorded(vm, &mut recorder, budget.as_ref());
    let syscalls = recorder.trace.take();
    let mut host = recorder.inner;
    let mut out = Map::new();
    match &outcome {
        Ok(result) => {
            out.insert("result".to_owned(), result_json(vm, result));
        }
        Err(message) => {
            out.insert("panic".to_owned(), s(message.clone()));
        }
    }
    let Value::Object(state) = state_json(vm) else {
        unreachable!("state_json builds an object");
    };
    out.extend(state);
    let entrypoint = steps.iter().find_map(|step| {
        (text(step, &["op"]) == "select_entrypoint").then(|| text(step, &["name"]))
    });
    let program = fs::read(fixture_root().join(text(record, &["artifact"]))).expect("artifact");
    out.insert(
        "return".to_owned(),
        match &outcome {
            Ok(Ok(())) => return_json(vm, &program, entrypoint),
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
    let trace = (trace_mode == TraceMode::PcOnly).then(|| pc_trace_json(vm));
    Replay {
        run: Value::Object(out),
        trace,
    }
}

fn core_state(host: &mut CoreHost) -> Vec<(&'static str, Value)> {
    let state = host
        .state_paths()
        .into_iter()
        .map(|path| {
            let value = host.state_bytes(&path).unwrap_or_default();
            obj([("path", s(path)), ("value", blob(&value))])
        })
        .collect();
    vec![("host_state", Value::Array(state))]
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
                    blob(&norito::encode_canonical(event.payload()).expect("event payload")),
                ),
            ])
        })
        .collect();
    vec![("events", Value::Array(events))]
}

/// Replay one oracle case on a fresh baseline VM, as the capture's `run_case` did.
fn replay(record: &Value, program: &[u8], trace_mode: TraceMode) -> Replay {
    let setup = member(record, &["setup"]).expect("setup");
    let gas_limit = number(setup, &["gas_limit"]);
    let mut vm = IVM::deterministic_builder(gas_limit)
        .suppress_startup_banner()
        .build();
    let host_kind = text(setup, &["host"]);
    if host_kind == "wsv_contract" {
        let account = fixture_account();
        let mut host = WsvHost::new_with_subject(MockWorldStateView::new(), account.clone())
            .with_public_inputs(public_inputs(setup));
        let hash = ivm::contract_code_hash(program);
        let address = ContractAddress::derive(
            &FIXTURE_NETWORK.parse().expect("fixture network"),
            &account,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("fixture contract address");
        let mut lifecycle = ContractLifecycleControlV1::direct(account.clone());
        lifecycle.active_code_hash = Some(hash);
        lifecycle.retained_code_hash = Some(hash);
        if let Err(error) = host.install_contract_fixture_for_profile(
            address.clone(),
            program.to_vec(),
            lifecycle,
            None,
            PROFILE,
        ) {
            return Replay {
                run: not_loaded("install_contract_fixture", &error),
                trace: None,
            };
        }
        let prepared = host
            .contract_fixture(&address)
            .expect("installed contract fixture")
            .clone();
        if let Err(error) = vm.load_prepared(&prepared) {
            return Replay {
                run: not_loaded("load_prepared", &error),
                trace: None,
            };
        }
        let entrypoint = member(setup, &["steps"])
            .and_then(Value::as_array)
            .and_then(|steps| {
                steps.iter().find_map(|step| {
                    (text(step, &["op"]) == "select_entrypoint").then(|| text(step, &["name"]))
                })
            })
            .expect("a contract-fixture case selects an entrypoint")
            .to_owned();
        host.bind_contract_runtime_context(account, address, entrypoint)
            .expect("bind fixture runtime context");
        return loaded_run(record, &mut vm, host, trace_mode, wsv_events);
    }
    if let Err(error) = vm.load_program_for_profile(program, PROFILE) {
        return Replay {
            run: not_loaded("load_program", &error),
            trace: None,
        };
    }
    match host_kind {
        "default" => loaded_run(
            record,
            &mut vm,
            DefaultHost::new().with_public_inputs(public_inputs(setup)),
            trace_mode,
            |_| Vec::new(),
        ),
        "core" => loaded_run(record, &mut vm, CoreHost::new(), trace_mode, core_state),
        "wsv" => loaded_run(
            record,
            &mut vm,
            WsvHost::new_with_subject(MockWorldStateView::new(), fixture_account())
                .with_public_inputs(public_inputs(setup)),
            trace_mode,
            wsv_events,
        ),
        other => panic!("unknown host {other}"),
    }
}

/// Every path at which `expected` and `actual` differ, with both values.
fn differences(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left
                .keys()
                .chain(right.keys().filter(|key| !left.contains_key(*key)))
            {
                let next = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => differences(&next, left, right, out),
                    (left, right) => {
                        out.push(format!("{next}: oracle {left:?} != baseline {right:?}"))
                    }
                }
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                differences(&format!("{path}[{index}]"), left, right, out);
            }
        }
        _ if expected != actual => {
            let render = |value: &Value| {
                let text = norito::json::to_string(value).unwrap_or_default();
                if text.len() > 300 {
                    format!("{}…", &text[..300])
                } else {
                    text
                }
            };
            out.push(format!(
                "{path}: oracle {} != baseline {}",
                render(expected),
                render(actual)
            ));
        }
        _ => {}
    }
}

fn oracle_records() -> Vec<Value> {
    let path = fixture_root().join("oracle/oracle.jsonl");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .lines()
        .map(|line| norito::json::from_str(line).expect("oracle record JSON"))
        .collect()
}

#[test]
fn baseline_engine_reproduces_every_oracle_case_exactly() {
    ivm::set_banner_enabled(false);
    let records = oracle_records();
    assert_eq!(records.len(), 499, "the phase-1 corpus has 499 cases");
    let mut mismatched = BTreeMap::<String, Vec<String>>::new();
    let mut by_field = BTreeMap::<String, usize>::new();
    for record in &records {
        let id = text(record, &["id"]).to_owned();
        let program = fs::read(fixture_root().join(text(record, &["artifact"]))).expect("artifact");
        let mut diffs = Vec::new();
        if sha256_hex(&program) != text(record, &["artifact_sha256"]) {
            diffs.push("artifact_sha256".to_owned());
        }
        let setup = member(record, &["setup"]).expect("setup");
        let upper_bound = number(setup, &["policy", "max_cycles_upper_bound"]);
        let fuel = number(setup, &["policy", "fuel"]);
        differences(
            "admission",
            member(record, &["admission"]).expect("admission"),
            &admission_json(&program, upper_bound, fuel),
            &mut diffs,
        );
        if text(setup, &["gas_limit_source"]) == "header_cycles" {
            let expected =
                core_admit(&program, upper_bound, fuel).map_or(u64::MAX, gas_limit_for_cycles);
            if expected != number(setup, &["gas_limit"]) {
                diffs.push(format!(
                    "setup.gas_limit: oracle {} != baseline header-to-gas {expected}",
                    number(setup, &["gas_limit"])
                ));
            }
        }
        let plain = replay(record, &program, TraceMode::Off);
        differences(
            "run",
            member(record, &["run"]).expect("run"),
            &plain.run,
            &mut diffs,
        );
        let expected_trace = member(record, &["trace"]).expect("trace");
        if !matches!(expected_trace, Value::Null) {
            let traced = replay(record, &program, TraceMode::PcOnly);
            differences(
                "trace",
                expected_trace,
                traced.trace.as_ref().unwrap_or(&Value::Null),
                &mut diffs,
            );
        }
        if !diffs.is_empty() {
            for diff in &diffs {
                let field = diff.split([':', '[']).next().unwrap_or_default().to_owned();
                *by_field.entry(field).or_default() += 1;
            }
            mismatched.insert(id, diffs);
        }
    }
    if !mismatched.is_empty() {
        let mut report = format!(
            "{} of {} oracle cases differ on the baseline engine\nby field: {by_field:#?}\n",
            mismatched.len(),
            records.len()
        );
        for (id, diffs) in mismatched.iter().take(40) {
            report.push_str(&format!("--- {id}\n"));
            for diff in diffs.iter().take(8) {
                report.push_str(&format!("    {diff}\n"));
            }
        }
        panic!("{report}");
    }
}

#[test]
fn baseline_engine_descriptors_are_the_frozen_fixture_bytes() {
    let root = fixture_root().join("descriptors");
    let abi = fs::read(root.join("abi_v1.descriptor.bin")).expect("ABI descriptor fixture");
    let schedule =
        fs::read(root.join("gas_schedule_v1.descriptor.bin")).expect("gas schedule fixture");
    let engine_abi =
        ivm_abi::syscalls::baseline_abi_surface_descriptor().expect("baseline ABI descriptor");
    assert_eq!(engine_abi.len(), abi.len(), "ABI descriptor length");
    assert!(engine_abi == abi.as_slice(), "ABI descriptor bytes");
    assert_eq!(
        hex::encode(iroha_crypto::Hash::new(engine_abi).as_ref()),
        "7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7"
    );
    assert_eq!(IvmProfileId::BASELINE_ABI_V1.abi_hash(), base::abi_hash());
    let engine_schedule = ivm::gas::baseline::schedule_descriptor();
    assert_eq!(engine_schedule.len(), schedule.len(), "schedule length");
    assert!(engine_schedule == schedule, "gas schedule descriptor bytes");
    assert_eq!(
        hex::encode(ivm::gas::schedule_hash_for(ivm::IvmEngine::Baseline).as_ref()),
        "af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f"
    );
}
