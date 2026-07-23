//! Generate or check the generated syscall list section in `docs/syscalls.md`.
//! Usage:
//!   cargo run -p ivm --bin gen_syscalls_doc -- --write
//!   cargo run -p ivm --bin gen_syscalls_doc -- --check

use std::{
    fs,
    path::{Path, PathBuf},
};

const BEGIN: &str = "<!-- BEGIN GENERATED SYSCALLS -->";
const END: &str = "<!-- END GENERATED SYSCALLS -->";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Write,
    Check,
}

fn parse_mode(args: impl IntoIterator<Item = String>) -> Result<Mode, String> {
    let mut mode = None;
    for arg in args {
        let requested = match arg.as_str() {
            "--write" => Mode::Write,
            "--check" => Mode::Check,
            _ => {
                return Err(format!(
                    "unknown argument `{arg}`; usage: --write or --check"
                ));
            }
        };
        if mode.replace(requested).is_some() {
            return Err("select exactly one of --write or --check".to_owned());
        }
    }
    mode.ok_or_else(|| "usage: --write or --check".to_owned())
}

fn guess_defaults(n: u32) -> (String, String, String) {
    let name = ivm::syscalls::syscall_name(n).unwrap_or("");
    // Baseline defaults
    let mut args = String::from("-");
    let mut ret = String::from("-");
    let mut gas = String::from("-");

    let up = name;
    // Heuristics for common patterns; conservative and non-binding
    if up.contains("ZK_VERIFY_BATCH") || n == 0x68 {
        args = "r10=&NoritoBytes(Vec<OpenVerifyEnvelope>)".into();
        ret = "r10=ptr (&NoritoBytes(Vec<u8> statuses)), r11=status:u64".into();
        gas = "G_verify + bytes".into();
    } else if matches!(n, 0x60..=0x63) {
        args = "r10=&NoritoBytes(OpenVerifyEnvelope)".into();
        ret = "u64=0/1".into();
        gas = "G_verify_proof + bytes".into();
    } else if up.contains("VRF_VERIFY_BATCH") || n == 0x67 {
        args = "r10=&NoritoBytes(VrfVerifyBatchRequest)".into();
        ret = "r10=ptr (&NoritoBytes(Vec<[u8;32]>)), r11=status:u64, r12=fail_index?:u64".into();
        gas = "G_verify + bytes".into();
    } else if up.contains("VRF_VERIFY") || n == 0x66 {
        args = "r10=&NoritoBytes(VrfVerifyRequest)".into();
        ret = "r10=ptr (&Blob(32-byte output)), r11=status:u64".into();
        gas = "G_verify + bytes".into();
    } else if up.contains("VERIFY_PROOF") || n == 0xF6 {
        args = "r10=&NoritoBytes(OpenVerifyEnvelope)".into();
        ret = "r10=0/1, r11=status:u64".into();
        gas = "G_verify_proof + bytes".into();
    } else if up.contains("ROOTS_GET") || n == 0x64 {
        args = "r10=&NoritoBytes(RootsGetRequest)".into();
        ret = "host-owned ptr (&NoritoBytes)".into();
        gas = "G_roots_get + bytes".into();
    } else if up.contains("VOTE_GET_TALLY") || n == 0x65 {
        args = "r10=&NoritoBytes(VoteGetTallyRequest)".into();
        ret = "host-owned ptr (&NoritoBytes)".into();
        gas = "G_vote_get + bytes".into();
    } else if up.contains("DECODE_INT") || n == 0x53 {
        args = "r10=&NoritoBytes(Norito-framed i64)".into();
        ret = "r10=i64".into();
        gas = "G_numeric + bytes".into();
    } else if up.contains("ENCODE_INT") || n == 0x55 {
        args = "r10=value:i64".into();
        ret = "r10=ptr (&NoritoBytes(Norito-framed i64))".into();
        gas = "G_numeric + bytes".into();
    } else if up.contains("BUILD_PATH_KEY_NORITO") || n == 0x56 {
        args = "r10=&Name(base), r11=&NoritoBytes(key)".into();
        ret = "r10=ptr (&Name)".into();
        gas = "G_path + bytes".into();
    } else if up.contains("JSON_ENCODE") || n == 0x57 {
        args = "r10=&Json".into();
        ret = "ptr (&NoritoBytes)".into();
        gas = "G_json_encode + bytes".into();
    } else if up.contains("JSON_DECODE") || n == 0x58 {
        args = "r10=&NoritoBytes(JSON bytes)".into();
        ret = "ptr (&Json)".into();
        gas = "G_json_decode + bytes".into();
    } else if up.contains("JSON_GET_") {
        args = "r10=&Json(object), r11=&Name(key)".into();
        ret = "r10=Option<T> sum handle".into();
        gas = "G_json_get + input bytes + active payload + sum allocation".into();
    } else if up.contains("JSON_OBJECT") || n == 0x81 {
        args = "-".into();
        ret = "ptr (&Json({}))".into();
        gas = "G_json_object + bytes".into();
    } else if up.contains("JSON_SET_I64") || n == 0x82 {
        args = "r10=&Json(object), r11=&Name(key), r12=value:i64".into();
        ret = "ptr (&Json)".into();
        gas = "G_json_set + bytes".into();
    } else if up.contains("JSON_SET_ACCOUNT_ID") || n == 0x83 {
        args = "r10=&Json(object), r11=&Name(key), r12=&AccountId".into();
        ret = "ptr (&Json)".into();
        gas = "G_json_set + bytes".into();
    } else if up.contains("SCHEMA_ENCODE") || n == 0x59 {
        args = "r10=&Name(schema), r11=&Json".into();
        ret = "ptr (&NoritoBytes)".into();
        gas = "G_schema + bytes".into();
    } else if up.contains("SCHEMA_DECODE") || n == 0x5A {
        args = "r10=&Name(schema), r11=&NoritoBytes".into();
        ret = "ptr (&Json)".into();
        gas = "G_schema + bytes".into();
    } else if up.contains("SCHEMA_INFO") || n == 0x5B {
        args = "r10=&Name(schema)".into();
        ret = "ptr (&Json{\"id\":...,\"version\":...})".into();
        gas = "G_schema + bytes".into();
    } else if up.contains("PRIVATE_NUMERIC_VALCOM") || n == 0xF8 {
        args = "r10=private:&Int|&Decimal|&Quantity(value), r11=private:&Int|&Decimal|&Quantity(blind)".into();
        ret = "r10=public:&Int(full compressed Pedersen point)".into();
        gas = "G_private_numeric_valcom".into();
    } else if up.contains("GET_ACCOUNT_BALANCE") || n == 0xF9 {
        args = "r10=&AccountId, r11=&AssetDefinitionId".into();
        ret = "ptr (&Quantity)".into();
        gas = "G_get_bal".into();
    } else if up.contains("NAME_DECODE") || n == 0x5C {
        args = "r10=&NoritoBytes(Name)".into();
        ret = "ptr (&Name)".into();
        gas = "G_name_decode + bytes".into();
    } else if up.contains("POINTER_TO_NORITO") || n == 0x5D {
        args = "r10=&PointerType<T>".into();
        ret = "ptr (&NoritoBytes(TLV envelope))".into();
        gas = "G_pointer + bytes".into();
    } else if up.contains("POINTER_FROM_NORITO") || n == 0x5E {
        args = "r10=&NoritoBytes(TLV envelope), r11=expected?:u16".into();
        ret = "ptr (&PointerType<T>)".into();
        gas = "G_pointer + bytes".into();
    } else if up.contains("TLV_EQ") || n == 0x5F {
        args = "r10=&Tlv, r11=&Tlv".into();
        ret = "r10=1/0".into();
        gas = "G_tlv_eq + bytes".into();
    } else if up.contains("TLV_LEN") || n == 0x77 {
        args = "r10=&Tlv".into();
        ret = "r10=payload_len:u64".into();
        gas = "G_tlv_len + bytes".into();
    } else if up.contains("VRF_EPOCH_SEED") || n == 0x7E {
        args = "r10=&NoritoBytes(VrfEpochSeedRequest)".into();
        ret = "r10=ptr (&NoritoBytes(VrfEpochSeedResponse)), r11=status:u64".into();
        gas = "G_vote_get + bytes".into();
    } else if up.starts_with("INT_") || up.starts_with("DECIMAL_") || up.starts_with("QUANTITY_") {
        // Numeric ABI rows are mandatory in `spec/syscalls.toml`; this branch
        // is only the diagnostic starting point printed when one is missing.
        // Keep it aligned with the first-release pointer-backed staged family
        // rather than suggesting the retired scalar `NUMERIC_*` protocol.
        let value_type = if up.starts_with("INT_") {
            "Int"
        } else if up.starts_with("DECIMAL_") {
            "Decimal"
        } else {
            "Quantity"
        };
        gas = "G_numeric_staged".into();
        if up.contains("FROM_INT") {
            args = "r10=&Int".into();
            ret = format!("r10=&{value_type}");
        } else if up.contains("TO_INT") {
            args = format!("r10=&{value_type}");
            ret = "r10=&Int-or-zero, r11=NumericFaultV1-or-zero".into();
        } else if up.contains("NEG") {
            args = format!("r10=&{value_type}");
            ret = format!("r10=&{value_type}-or-zero, r11=NumericFaultV1-or-zero");
        } else if up.contains("EQ")
            || up.contains("NE")
            || up.contains("LT")
            || up.contains("LE")
            || up.contains("GT")
            || up.contains("GE")
        {
            args = format!("r10=&{value_type}, r11=&{value_type}");
            ret = "r10=u64(0/1)".into();
        } else {
            args = format!("r10=&{value_type}, r11=&{value_type}");
            ret = format!("r10=&{value_type}-or-zero, r11=NumericFaultV1-or-zero");
        }
    } else if up.contains("PROVE_EXECUTION") || n == 0xF4 {
        ret = "r10=ptr (&NoritoBytes(ExecutionProof)), r11=status:u64".into();
        gas = "G_prove".into();
    } else if up.contains("SM4_GCM_SEAL") || n == 0x92 {
        args = "r10=&Blob(key16), r11=&Blob(nonce12), r12=&Blob(aad)?, r13=&Blob(plaintext)".into();
        ret = "r10=ptr (&Blob(ciphertext || tag16))".into();
        gas = "G_sm4 + bytes".into();
    } else if up.contains("SM4_GCM_OPEN") || n == 0x93 {
        args =
            "r10=&Blob(key16), r11=&Blob(nonce12), r12=&Blob(aad)?, r13=&Blob(ciphertext || tag16)"
                .into();
        ret = "r10=ptr (&Blob(plaintext)) or 0".into();
        gas = "G_sm4 + bytes".into();
    } else if up.contains("SM4_CCM_SEAL") || n == 0x94 {
        args =
            "r10=&Blob(key16), r11=&Blob(nonce[7..13]), r12=&Blob(aad)?, r13=&Blob(plaintext), r14=tag_len:u64"
                .into();
        ret = "r10=ptr (&Blob(ciphertext || tag))".into();
        gas = "G_sm4 + bytes".into();
    } else if up.contains("SM4_CCM_OPEN") || n == 0x95 {
        args =
            "r10=&Blob(key16), r11=&Blob(nonce[7..13]), r12=&Blob(aad)?, r13=&Blob(ciphertext || tag), r14=tag_len:u64"
                .into();
        ret = "r10=ptr (&Blob(plaintext)) or 0".into();
        gas = "G_sm4 + bytes".into();
    } else if up.contains("VERIFY_SIGNATURE") || n == 0xFC {
        args = "r10=&Blob(message), r11=&Blob(signature), r12=&Blob(pubkey), r13=scheme:u8".into();
        ret = "r10=0/1".into();
        gas = "G_verify_sig + bytes".into();
    } else if up.contains("VERIFY_DS_PROOF") || n == 0xB3 {
        args = "r10=&DataSpaceId, r11=&ProofBlob or 0".into();
        ret = "u64=0/1".into();
        gas = "G_verify + bytes".into();
    } else if up.contains("VERIFY") {
        ret = "u64=0/1".into();
        gas = "G_verify".into();
    } else if up.contains("ALLOC") || n == 0xF0 {
        args = "r10=bytes:u64".into();
        ret = "ptr (r10)".into();
        gas = "G_alloc + bytes".into();
    } else if up.contains("GROW_HEAP") || n == 0xF5 {
        args = "r10=bytes:u64".into();
        ret = "u64=new_limit".into();
        gas = "G_grow_heap per page".into();
    } else if up.contains("GET_PRIVATE_INPUT") || n == 0xFD {
        args = "r10=index:u64, r11=PrivateInputKindV1".into();
        ret = "r10=private:&Int|&Decimal|&Quantity".into();
        gas = "G_get_priv".into();
    } else if up.contains("GET_PUBLIC_INPUT") || n == 0xF1 {
        args = "r10=&Name".into();
        ret = "ptr (r10)".into();
        gas = "G_get_pub".into();
    } else if up.contains("GET_AUTHORITY") || n == 0xA4 {
        ret = "host-owned ptr (&AccountId)".into();
        gas = "G_get_auth + bytes".into();
    } else if up.contains("RESOLVE_ACCOUNT_ALIAS") || n == 0xA7 {
        args = "r10=&Blob(alias literal)".into();
        ret = "host-owned ptr (&AccountId)".into();
        gas = "G_alias_resolve".into();
    } else if up.contains("CURRENT_TIME_MS") || n == 0xA8 {
        ret = "r10=unix_time_ms:u64".into();
        gas = "G_sysvar".into();
    } else if up.contains("STATE_GET") || n == 0x50 {
        args = "r10=&Name".into();
        ret = "r10=ptr (&NoritoBytes) or 0".into();
        gas = "G_state_get + bytes".into();
    } else if up.contains("STATE_SET") || n == 0x51 {
        args = "r10=&Name, r11=&NoritoBytes".into();
        ret = "u64=0".into();
        gas = "G_state_set + bytes".into();
    } else if up.contains("STATE_DEL") || n == 0x52 {
        args = "r10=&Name".into();
        ret = "u64=0".into();
        gas = "G_state_del".into();
    } else if up.contains("INPUT_PUBLISH_TLV") || n == 0xE0 {
        args = "r10=&Blob(TLV)".into();
        ret = "ptr (r10)".into();
        gas = "G_input_publish + bytes".into();
    } else if up.contains("MERKLE_PATH") || n == 0xF7 {
        args = "r10=addr:u64, r11=out:u64, r12=root_out?:u64".into();
        ret = "u64=len".into();
        gas = "G_mpath + len".into();
    } else if up.contains("MERKLE_COMPACT") || n == 0xFA || n == 0xFF {
        args = "r10=addr, r11=out, r12=depth_cap?, r13=root_out?".into();
        ret = "u64=depth".into();
        gas = "G_mpath + depth".into();
    } else if up.contains("QUERY_EXECUTE_NORITO") || n == 0x01_0000 {
        args = "r10=&NoritoBytes(QueryRequest)".into();
        ret = "r10=ptr (&NoritoBytes(QueryResponse))".into();
        gas = "G_scq".into();
    } else if up == "CORE_QUERY_GET" || n == 0x01_0001 {
        args = "r10=CoreQueryEntityTagV1:u64, r11=&typed entity id".into();
        ret = "r10=Option<View> sum handle (typed leaf TLVs)".into();
        gas = "G_scq + query items + encoded bytes".into();
    } else if up == "CORE_QUERY_PAGE" || n == 0x01_0002 {
        args = "r10=CoreQueryEntityTagV1:u64, r11=offset:i64 bits, r12=limit:1..=64".into();
        ret = "r10=List<View,64> handle, r11=Option<i64> sum handle".into();
        gas = "G_scq + offset + query items + encoded bytes".into();
    } else if up == "JSON_BUILD" || n == 0x01_004E {
        args = "r10=&NoritoBytes(JsonConstructionSchemaV1), r11=word_table, r12=word_count".into();
        ret = "r10=&Json".into();
        gas = "G_json_build + schema + source + words + elements + encoded bytes".into();
    } else if up.contains("SYSVAR_CHAIN_ID") || n == 0x01_0020 {
        ret = "r10=ptr (&Blob(chain_id)) or 0".into();
        gas = "G_sysvar + bytes".into();
    } else if up.contains("SYSVAR_BLOCK_HEIGHT") || n == 0x01_0021 {
        ret = "r10=height:u64".into();
        gas = "G_sysvar".into();
    } else if up.contains("SYSVAR_BLOCK_TIME_MS") || n == 0x01_0022 {
        ret = "r10=block_time_ms:u64".into();
        gas = "G_sysvar".into();
    } else if up.contains("SYSVAR_AUTHORITY") || n == 0x01_0023 {
        ret = "r10=ptr (&AccountId)".into();
        gas = "G_get_auth + bytes".into();
    } else if up.contains("SYSVAR_CONTRACT_ADDRESS") || n == 0x01_0024 {
        ret = "r10=ptr (&NoritoBytes(ContractAddress)) or 0".into();
        gas = "G_sysvar + bytes".into();
    } else if up.contains("SYSVAR_ENTRYPOINT") || n == 0x01_0025 {
        ret = "r10=ptr (&Blob(entrypoint)) or 0".into();
        gas = "G_sysvar + bytes".into();
    } else if up.contains("DECODE_ARGUMENT_RECORD") || n == 0x01_0026 {
        args = "r10=raw &NoritoBytes(EntrypointArgumentRecordV1) or prepared &NoritoBytes(record binding), r11=&NoritoBytes(EntrypointArgumentSchemaV1)".into();
        ret = "r10=ptr (&Blob(pad:u8 then [u64; word_count]))".into();
        gas = "G_argument_decode + record + schema + complete materialization".into();
    } else if up.contains("NORMALIZE_NORITO_BYTES") || n == 0x01_0028 {
        args = "r10=&Blob or &NoritoBytes (validated public TLV)".into();
        ret = "r10=&NoritoBytes(same payload)".into();
        gas = "G_pointer + bytes".into();
    } else if up.contains("STATE_KEYS") || n == 0x01_0030 {
        args = "r10=&Name(prefix), r11=offset:u64, r12=limit:u64 (0..=64)".into();
        ret = "r10=ptr (&NoritoBytes(Vec<Name>)), r11=total:u64, r12=count:u64".into();
        gas = "G_state_keys + count + bytes".into();
    } else if up.contains("STATE_HAS") || n == 0x01_0031 {
        args = "r10=&Name(path)".into();
        ret = "r10=present:u64".into();
        gas = "G_state_has".into();
    } else if up.contains("STATE_LEN") || n == 0x01_0032 {
        args = "r10=&Name(path)".into();
        ret = "r10=len:u64, r11=found:u64".into();
        gas = "G_state_len + bytes".into();
    } else if up.contains("STATE_COUNT") || n == 0x01_0033 {
        args = "r10=&Name(prefix)".into();
        ret = "r10=total:u64".into();
        gas = "G_state_count + count".into();
    } else if up.contains("STATE_MAP_KEY_AT") || n == 0x01_0034 {
        args = "r10=&NoritoBytes(Vec<Name>), r11=&Name(base), r12=index:u64".into();
        ret = "r10=ptr (&NoritoBytes(canonical key)) or 0".into();
        gas = "G_path + bytes".into();
    } else if up.contains("STATE_VALUE_ENCODE") || n == 0x01_0035 {
        args = "r10=&NoritoBytes(StateValueSchemaV1), r11=&[u64], r12=word_count:u64".into();
        ret = "r10=ptr (&NoritoBytes(StateValueRecordV1))".into();
        gas = "G_state_value + schema + words + pointers + output".into();
    } else if up.contains("STATE_VALUE_DECODE") || n == 0x01_0036 {
        args = "r10=&NoritoBytes(StateValueSchemaV1), r11=&NoritoBytes(StateValueRecordV1)".into();
        ret = "r10=ptr (&Blob(pad:u8 then [u64; word_count]))".into();
        gas = "G_state_value + schema + record + pointers + output".into();
    } else if up.contains("EXECUTE_QUERY") || n == 0xA1 {
        args = "r10=&Json".into();
        ret = "ptr (r10)".into();
        gas = "G_scq".into();
    } else if up.contains("EXECUTE_INSTRUCTION") || n == 0xA0 {
        args = "r10=&Json".into();
        ret = "u64=0".into();
        gas = "G_sci".into();
    } else if up.contains("COMMIT_OUTPUT") || n == 0xFE {
        args = "r10=&Json".into();
        ret = "u64=0".into();
        gas = "G_commit".into();
    } else if up.is_empty() {
        // Unknown number without a name: keep dashes
    } else {
        // Safe default for side-effect syscalls
        ret = "u64=0".into();
    }
    (args, ret, gas)
}

fn spec_entry(
    map: &std::collections::BTreeMap<u32, (String, String, String)>,
    n: u32,
) -> (String, String, String) {
    map.get(&n).cloned().unwrap_or_else(|| {
        let (args, ret, gas) = guess_defaults(n);
        panic!(
            "ABI syscall 0x{n:06X} has no explicit spec row; suggested starting point: args={args:?}, ret={ret:?}, gas={gas:?}"
        )
    })
}

fn split_gas_terms(gas_raw: &str) -> Vec<&str> {
    let mut terms = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0_u32;
    for (idx, ch) in gas_raw.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '+' if paren_depth == 0 => {
                terms.push(&gas_raw[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    terms.push(&gas_raw[start..]);
    terms
}

fn rewrite_gas_tokens(gas_raw: &str) -> (String, Vec<String>) {
    let mut out_gas = String::new();
    let mut gas_keys = Vec::new();
    for (i, part) in split_gas_terms(gas_raw)
        .into_iter()
        .map(|s| s.trim())
        .enumerate()
    {
        if i > 0 {
            out_gas.push_str(" + ");
        }
        if let Some(tok) = part.split_whitespace().next()
            && tok.starts_with('G')
        {
            gas_keys.push(tok.to_string());
            out_gas.push_str(&format!("asset:gas/{tok}@ivm.core/v2"));
            let tail = part.strip_prefix(tok).unwrap_or("").trim();
            if !tail.is_empty() {
                out_gas.push(' ');
                out_gas.push_str(tail);
            }
            continue;
        }
        out_gas.push_str(part);
    }
    (out_gas, gas_keys)
}

fn unescape_basic_toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn escape_rust_string_literal(value: &str) -> String {
    value.escape_default().to_string()
}

fn render_table(map: &std::collections::BTreeMap<u32, (String, String, String)>) -> String {
    let mut nums: Vec<u32> = ivm::syscalls::abi_syscall_list().to_vec();
    nums.sort_unstable();
    let mut out = String::new();
    out.push_str("| Number | Name | Args | Return | Gas |\n");
    out.push_str("|---|---|---|---|---|\n");
    for n in nums {
        let name = ivm::syscalls::syscall_name(n).unwrap_or("");
        let (args, ret, gas_raw) = spec_entry(map, n);
        let (gas, _) = rewrite_gas_tokens(&gas_raw);
        out.push_str(&format!(
            "| 0x{n:02X} | {name} | {args} | {ret} | {gas} |\n"
        ));
    }
    out
}

fn render_docs(text: &str, table: &str) -> Result<String, String> {
    let replacement = format!("{BEGIN}\n{table}{END}\n");
    let begin = text.find(BEGIN);
    let end = text.find(END);
    match (begin, end) {
        (Some(begin), Some(end)) if begin < end => {
            if text[begin + BEGIN.len()..].contains(BEGIN) || text[end + END.len()..].contains(END)
            {
                return Err("syscalls.md contains duplicate generated-section markers".to_owned());
            }
            let mut replace_end = end + END.len();
            if text[replace_end..].starts_with('\n') {
                replace_end += 1;
            }
            let mut rendered =
                String::with_capacity(text.len() - (replace_end - begin) + replacement.len());
            rendered.push_str(&text[..begin]);
            rendered.push_str(&replacement);
            rendered.push_str(&text[replace_end..]);
            Ok(rendered)
        }
        (None, None) => {
            let mut rendered = text.to_owned();
            if !rendered.is_empty() {
                if !rendered.ends_with('\n') {
                    rendered.push('\n');
                }
                rendered.push('\n');
            }
            rendered.push_str(&replacement);
            Ok(rendered)
        }
        _ => Err("syscalls.md contains malformed generated-section markers".to_owned()),
    }
}

fn sync_generated_file(
    path: &Path,
    expected: &str,
    mode: Mode,
    regenerate_command: &str,
) -> Result<bool, String> {
    let current = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    if current.as_deref() == Some(expected.as_bytes()) {
        return Ok(false);
    }
    match mode {
        Mode::Check => Err(format!(
            "{} is out of date; run: {regenerate_command}",
            path.display()
        )),
        Mode::Write => {
            fs::write(path, expected)
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
            Ok(true)
        }
    }
}

fn main() {
    let mode = match parse_mode(std::env::args().skip(1)) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest_dir).join("docs/syscalls.md");
    let text = fs::read_to_string(&path).expect("read syscalls.md");

    // The explicit spec is the canonical source for ABI signatures and gas
    // documentation. Heuristic defaults are diagnostic suggestions only.
    let spec_path = PathBuf::from(manifest_dir).join("spec/syscalls.toml");
    let spec = fs::read_to_string(&spec_path).expect("read canonical syscall spec");
    let mut map: std::collections::BTreeMap<u32, (String, String, String)> = Default::default();
    {
        let s = spec.as_str();
        // Very small, ad-hoc parser: supports arrays of tables [[syscall]] with number,args,ret,gas
        // number accepted as hex string (e.g., "0xA4") or decimal.
        let mut cur: Option<(u32, String, String, String)> = None;
        for line in s.lines() {
            let t = line.trim();
            if t.starts_with("[[syscall]]") {
                if let Some((n, a, r, g)) = cur.take() {
                    assert!(
                        map.insert(n, (a, r, g)).is_none(),
                        "duplicate syscall spec row for 0x{n:06X}"
                    );
                }
                cur = Some((0, String::new(), String::new(), String::new()));
                continue;
            }
            if let Some(eq) = t.find('=') {
                let key = t[..eq].trim();
                let mut val = t[eq + 1..].trim().to_owned();
                if let Some(v) = val.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                    val = unescape_basic_toml_string(v);
                }
                if let Some(tuple) = cur.as_mut() {
                    match key {
                        "number" => {
                            let n = if let Some(hex) = val.strip_prefix("0x") {
                                u32::from_str_radix(hex, 16).unwrap_or(0)
                            } else {
                                val.parse::<u32>().unwrap_or(0)
                            };
                            tuple.0 = n;
                        }
                        "args" => tuple.1 = val.clone(),
                        "ret" => tuple.2 = val.clone(),
                        "gas" => tuple.3 = val.clone(),
                        _ => {}
                    }
                }
            }
        }
        if let Some((n, a, r, g)) = cur.take()
            && (n != 0 || !a.is_empty() || !r.is_empty() || !g.is_empty())
        {
            assert!(
                map.insert(n, (a, r, g)).is_none(),
                "duplicate syscall spec row for 0x{n:06X}"
            );
        }
    }

    let allowed = ivm::syscalls::abi_syscall_list()
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let specified = map
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let missing = allowed.difference(&specified).copied().collect::<Vec<_>>();
    let extra = specified.difference(&allowed).copied().collect::<Vec<_>>();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "canonical syscall spec must exactly cover ABI v1; missing={missing:#X?}, extra={extra:#X?}"
    );
    for (&number, (args, ret, gas)) in &map {
        assert!(
            !args.is_empty() && !ret.is_empty() && !gas.is_empty(),
            "syscall spec row 0x{number:06X} must define non-empty args, ret, and gas"
        );
    }

    // Generate the expected code and gas assets for the `ivm_abi` crate, which
    // owns the syscall renderer used by docs/tests. Rendering is deliberately
    // side-effect free; publication happens only after every output is ready.
    let (generated_docs_code, generated_gas_code) = {
        let mut buf = String::new();
        buf.push_str("// @generated by gen_syscalls_doc.rs; do not edit manually\n");
        buf.push_str("#[rustfmt::skip]\n");
        buf.push_str("#[allow(dead_code)]\n");
        buf.push_str("pub static DOCS: &[crate::syscalls::SyscallDoc] = &[\n");
        let mut nums: Vec<u32> = ivm::syscalls::abi_syscall_list().to_vec();
        nums.sort_unstable();
        let mut gas_keys: std::collections::BTreeSet<String> = Default::default();
        for n in nums {
            let (args, ret, gas_raw) = spec_entry(&map, n);
            let (out_gas, discovered_gas_keys) = rewrite_gas_tokens(&gas_raw);
            for gas_key in discovered_gas_keys {
                gas_keys.insert(gas_key);
            }
            let args = escape_rust_string_literal(&args);
            let ret = escape_rust_string_literal(&ret);
            let out_gas = escape_rust_string_literal(&out_gas);
            buf.push_str(&format!(
                "    crate::syscalls::SyscallDoc {{ number: {n}, args: \"{args}\", ret: \"{ret}\", gas: \"{out_gas}\" }},\n"
            ));
        }
        buf.push_str("];\n");

        // Generate gas_spec.rs
        let mut gbuf = String::new();
        gbuf.push_str("// @generated by gen_syscalls_doc.rs; do not edit manually\n");
        gbuf.push_str("#[rustfmt::skip]\n");
        gbuf.push_str("#[derive(Clone, Copy)]\n");
        gbuf.push_str("pub struct GasAsset { pub key: &'static str, pub asset_id: &'static str, pub unit: &'static str, pub version: &'static str, pub group: &'static str }\n");
        gbuf.push_str("#[rustfmt::skip]\n");
        gbuf.push_str("pub static GAS_ASSETS: &[GasAsset] = &[\n");
        for k in gas_keys.iter() {
            let asset_id = format!("asset:gas/{k}@ivm.core/v2");
            gbuf.push_str(&format!(
                "    GasAsset {{ key: \"{k}\", asset_id: \"{asset_id}\", unit: \"gas\", version: \"v1\", group: \"syscall\" }},\n"
            ));
        }
        gbuf.push_str("];\n");

        // Lint prose gas tokens in docs against generated gas assets
        let mut prose_tokens: std::collections::BTreeSet<String> = Default::default();
        // Parse from prose lines that mention "Gas:" and extract tokens like G_name
        for line in text.lines() {
            if let Some(ix) = line.find("Gas:") {
                let tail = &line[ix + 4..];
                for seg in tail.split(&['+', '|', '—'][..]) {
                    let s = seg.trim();
                    if let Some(pos) = s.find('G') {
                        let token = &s[pos..];
                        let mut end = token.len();
                        for (i, ch) in token.chars().enumerate() {
                            if !(ch == '_' || ch.is_ascii_alphanumeric()) {
                                end = i;
                                break;
                            }
                        }
                        if end > 1 {
                            prose_tokens.insert(token[..end].to_string());
                        }
                    }
                }
            }
        }
        let code_tokens: std::collections::BTreeSet<String> = gas_keys.clone();
        // Suppress warnings for known experimental/excluded tokens
        // Add future experimental gas tokens here to avoid noisy lints until prose/spec catch up.
        let suppress: std::collections::BTreeSet<String> = [
            "G_verify_sig",
            // Often generated but not always mentioned explicitly in prose sections:
            "G_alloc",
            "G_get_pub",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let prose_minus_code: Vec<_> = prose_tokens
            .difference(&code_tokens)
            .filter(|t| !suppress.contains(*t))
            .cloned()
            .collect();
        let code_minus_prose: Vec<_> = code_tokens
            .difference(&prose_tokens)
            .filter(|t| !suppress.contains(*t))
            .cloned()
            .collect();
        if !prose_minus_code.is_empty() {
            eprintln!(
                "[gen_syscalls_doc] Warning: prose Gas tokens not in generated assets: {}",
                prose_minus_code.join(", ")
            );
        }
        if !code_minus_prose.is_empty() {
            eprintln!(
                "[gen_syscalls_doc] Warning: generated Gas tokens not mentioned in prose: {}",
                code_minus_prose.join(", ")
            );
        }
        (buf, gbuf)
    };

    // Render the documentation from the same canonical specification as the
    // Rust assets so check mode can compare all outputs without mutating any.
    let table = render_table(&map);
    let rendered_docs = match render_docs(&text, &table) {
        Ok(rendered) => rendered,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let abi_src_dir = PathBuf::from(manifest_dir).join("../ivm_abi/src");
    let code_path = abi_src_dir.join("syscalls_doc_gen.rs");
    let gas_code_path = abi_src_dir.join("gas_spec.rs");
    let regenerate_command = "cargo run -p ivm --bin gen_syscalls_doc -- --write";
    let outputs = [
        (&code_path, generated_docs_code.as_str()),
        (&gas_code_path, generated_gas_code.as_str()),
        (&path, rendered_docs.as_str()),
    ];
    let mut failures = Vec::new();
    for (output_path, expected) in outputs {
        match sync_generated_file(output_path, expected, mode, regenerate_command) {
            Ok(true) => eprintln!("updated: {}", output_path.display()),
            Ok(false) => {}
            Err(error) => failures.push(error),
        }
    }
    if !failures.is_empty() {
        for failure in failures {
            eprintln!("{failure}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        BEGIN, END, Mode, parse_mode, render_docs, rewrite_gas_tokens, sync_generated_file,
    };

    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

    fn temp_file(name: &str) -> std::path::PathBuf {
        let serial = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ivm-gen-syscalls-doc-{}-{serial}-{name}",
            std::process::id()
        ))
    }

    #[test]
    fn rewrite_gas_tokens_preserves_parenthesized_plus_text() {
        let (rendered, keys) =
            rewrite_gas_tokens("G_soracloud + request bytes (+ response bytes under host)");

        assert_eq!(
            rendered,
            "asset:gas/G_soracloud@ivm.core/v2 + request bytes (+ response bytes under host)"
        );
        assert_eq!(keys, ["G_soracloud"]);
    }

    #[test]
    fn command_mode_is_explicit_and_unambiguous() {
        assert_eq!(parse_mode(["--check".to_owned()]), Ok(Mode::Check));
        assert_eq!(parse_mode(["--write".to_owned()]), Ok(Mode::Write));
        assert!(parse_mode(Vec::new()).is_err());
        assert!(parse_mode(["--check".to_owned(), "--write".to_owned()]).is_err());
        assert!(parse_mode(["--no-code".to_owned()]).is_err());
    }

    #[test]
    fn document_rendering_is_idempotent() {
        let table = "| Number |\n|---|\n";
        let stale = format!("prose\n\n{BEGIN}\nstale\n{END}\n\ntail\n");
        let rendered = render_docs(&stale, table).expect("render generated section");
        assert_eq!(
            render_docs(&rendered, table).expect("render generated section again"),
            rendered
        );

        let without_markers = "prose\n";
        let appended = render_docs(without_markers, table).expect("append generated section");
        assert_eq!(
            render_docs(&appended, table).expect("render appended section again"),
            appended
        );

        assert!(render_docs(&format!("{BEGIN}\nunterminated\n"), table).is_err());
        assert!(
            render_docs(
                &format!("{BEGIN}\none\n{END}\n{BEGIN}\ntwo\n{END}\n"),
                table,
            )
            .is_err()
        );
    }

    #[test]
    fn check_is_nonmutating_and_write_is_idempotent() {
        let path = temp_file("asset.rs");
        fs::write(&path, "stale\n").expect("create stale generated asset");
        let before = fs::read(&path).expect("read stale generated asset");

        let error = sync_generated_file(&path, "current\n", Mode::Check, "generator --write")
            .expect_err("check must reject stale output");
        assert!(error.contains("generator --write"));
        assert_eq!(
            fs::read(&path).expect("read after check"),
            before,
            "check mode must not mutate stale output"
        );

        assert!(
            sync_generated_file(&path, "current\n", Mode::Write, "generator --write")
                .expect("publish generated output")
        );
        assert_eq!(
            fs::read_to_string(&path).expect("read published output"),
            "current\n"
        );
        assert!(
            !sync_generated_file(&path, "current\n", Mode::Write, "generator --write")
                .expect("repeat publication")
        );

        fs::remove_file(path).expect("remove temporary generated asset");

        let missing_path = temp_file("missing.rs");
        sync_generated_file(
            &missing_path,
            "generated\n",
            Mode::Check,
            "generator --write",
        )
        .expect_err("check must reject a missing output");
        assert!(
            !missing_path.exists(),
            "check mode must not create a missing output"
        );
        assert!(
            sync_generated_file(
                &missing_path,
                "generated\n",
                Mode::Write,
                "generator --write",
            )
            .expect("create missing generated output")
        );
        fs::remove_file(missing_path).expect("remove temporary generated asset");
    }
}
