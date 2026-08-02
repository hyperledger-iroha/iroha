// Signed-transaction Norito RPC fixture regressions share the parent module.
#[cfg(test)]
mod norito_rpc_fixture_tests {
    use super::*;
    use crate::account::address::ChainDiscriminantGuard;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use iroha_crypto::Hash;
    use norito::{
        core::DecodeFromSlice,
        json::{self, Value},
    };
    use std::{collections::BTreeSet, fs, path::PathBuf};

    fn manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("norito_rpc")
            .join("transaction_fixtures.manifest.json")
    }

    fn compact_hash_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("norito_rpc")
            .join("iroha_compact_hash_vector.properties")
    }

    fn compact_hash_fixture() -> std::collections::BTreeMap<String, String> {
        let path = compact_hash_fixture_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        parse_compact_hash_fixture(&raw)
    }

    fn parse_compact_hash_fixture(raw: &str) -> std::collections::BTreeMap<String, String> {
        const EXPECTED_KEYS: [&str; 10] = [
            "schema.version",
            "source.fixture",
            "versioned.bytes",
            "versioned.sha256",
            "bare.bytes",
            "compact.length.hex",
            "canonical.prefix.hex",
            "canonical.hash",
            "payload.prehash",
            "versioned.base64",
        ];
        let mut properties = std::collections::BTreeMap::new();
        for line in raw
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed compact hash fixture line: {line}"));
            assert!(
                !key.is_empty() && !value.is_empty(),
                "malformed compact hash fixture line: {line}"
            );
            assert!(
                properties
                    .insert(key.to_owned(), value.to_owned())
                    .is_none(),
                "duplicate compact hash fixture key: {key}"
            );
        }
        let actual_keys: BTreeSet<&str> = properties.keys().map(String::as_str).collect();
        let expected_keys: BTreeSet<&str> = EXPECTED_KEYS.into_iter().collect();
        assert_eq!(
            actual_keys, expected_keys,
            "compact hash fixture keys must match the required set"
        );
        let versioned_base64 = properties
            .get("versioned.base64")
            .expect("required versioned.base64 property");
        let versioned = BASE64
            .decode(versioned_base64)
            .expect("versioned.base64 must be valid canonical base64");
        assert_eq!(
            BASE64.encode(versioned),
            *versioned_base64,
            "versioned.base64 must be canonical"
        );
        properties
    }

    fn require_object<'a>(value: &'a Value, context: &str) -> &'a json::Map {
        value
            .as_object()
            .unwrap_or_else(|| panic!("{context} must be a JSON object"))
    }

    fn require_array<'a>(value: &'a Value, context: &str) -> &'a Vec<Value> {
        value
            .as_array()
            .unwrap_or_else(|| panic!("{context} must be a JSON array"))
    }

    fn require_str<'a>(map: &'a json::Map, key: &str, context: &str) -> &'a str {
        map.get(key)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{context}: missing {key} string"))
    }

    fn require_u64(map: &json::Map, key: &str, context: &str) -> u64 {
        map.get(key)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("{context}: missing {key} integer"))
    }

    fn optional_u64(map: &json::Map, key: &str, context: &str) -> Option<u64> {
        match map.get(key) {
            Some(Value::Null) | None => None,
            Some(Value::Number(number)) => number
                .as_u64()
                .or_else(|| panic!("{context}: {key} must be an integer or null")),
            Some(_) => panic!("{context}: {key} must be an integer or null"),
        }
    }

    fn authority_prefix(authority: &str) -> Option<u16> {
        if authority.starts_with("sora") {
            return Some(0x02F1);
        }
        if authority.starts_with("test") {
            return Some(0x0171);
        }
        if authority.starts_with("dev") {
            return Some(0x0000);
        }
        authority
            .strip_prefix('n')
            .and_then(|rest| {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    None
                } else {
                    Some(digits)
                }
            })
            .and_then(|digits| digits.parse::<u16>().ok())
    }

    #[allow(
        clippy::too_many_lines,
        clippy::explicit_iter_loop,
        clippy::collapsible_if,
        clippy::collapsible_match
    )]
    #[test]
    fn norito_rpc_fixture_manifest_roundtrips() {
        let path = manifest_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
        let manifest: Value =
            json::from_str(&raw).unwrap_or_else(|err| panic!("manifest JSON: {err}"));
        let manifest_obj = require_object(&manifest, "manifest");
        let fixtures = manifest_obj.get("fixtures").map_or_else(
            || panic!("manifest missing fixtures array"),
            |value| require_array(value, "manifest.fixtures"),
        );

        let mut names = BTreeSet::new();
        let mut encoded_files = BTreeSet::new();
        let mut payload_hashes = BTreeSet::new();
        let mut payload_bytes_values = BTreeSet::new();
        let mut signed_hashes = BTreeSet::new();
        let mut signed_bytes_values = BTreeSet::new();
        for fixture in fixtures {
            let entry = require_object(fixture, "fixture");
            let name = require_str(entry, "name", "fixture");
            let encoded_file = require_str(entry, "encoded_file", name);
            let payload_base64 = require_str(entry, "payload_base64", name);
            let signed_base64 = require_str(entry, "signed_base64", name);
            let payload_hash = require_str(entry, "payload_hash", name);
            let signed_hash = require_str(entry, "signed_hash", name);
            assert!(names.insert(name), "duplicate fixture name: {name}");
            assert!(
                encoded_files.insert(encoded_file),
                "duplicate fixture encoded_file: {encoded_file}"
            );
            assert!(
                payload_hashes.insert(payload_hash),
                "duplicate fixture payload_hash: {payload_hash}"
            );
            assert!(
                signed_hashes.insert(signed_hash),
                "duplicate fixture signed_hash: {signed_hash}"
            );
            let encoded_len = require_u64(entry, "encoded_len", name);
            let signed_len = require_u64(entry, "signed_len", name);
            let chain = require_str(entry, "chain", name);
            let authority = require_str(entry, "authority", name);
            let _chain_guard = authority_prefix(authority).map(ChainDiscriminantGuard::enter);
            let creation_time_ms = require_u64(entry, "creation_time_ms", name);
            let time_to_live_ms = optional_u64(entry, "time_to_live_ms", name);
            let nonce = optional_u64(entry, "nonce", name);

            let payload_bytes = BASE64
                .decode(payload_base64.as_bytes())
                .unwrap_or_else(|err| panic!("{name}: invalid payload_base64: {err}"));
            let signed_bytes = BASE64
                .decode(signed_base64.as_bytes())
                .unwrap_or_else(|err| panic!("{name}: invalid signed_base64: {err}"));
            assert_eq!(
                BASE64.encode(&payload_bytes),
                payload_base64,
                "{name}: payload_base64 must be canonical"
            );
            assert_eq!(
                BASE64.encode(&signed_bytes),
                signed_base64,
                "{name}: signed_base64 must be canonical"
            );
            assert!(
                payload_bytes_values.insert(payload_bytes.clone()),
                "duplicate fixture payload bytes: {name}"
            );
            assert!(
                signed_bytes_values.insert(signed_bytes.clone()),
                "duplicate fixture signed bytes: {name}"
            );
            assert_eq!(
                payload_bytes.len() as u64,
                encoded_len,
                "{name}: encoded_len mismatch"
            );
            assert_eq!(
                signed_bytes.len() as u64,
                signed_len,
                "{name}: signed_len mismatch"
            );

            let computed_payload_hash = Hash::new(&payload_bytes).to_string();
            assert_eq!(
                computed_payload_hash, payload_hash,
                "{name}: payload_hash mismatch"
            );

            let (signed_tx, used) = SignedTransaction::decode_from_slice(&signed_bytes)
                .unwrap_or_else(|err| panic!("{name}: signed transaction decode failed: {err}"));
            assert_eq!(
                used,
                signed_bytes.len(),
                "{name}: signed transaction has trailing bytes"
            );
            assert_eq!(
                signed_tx.hash_as_entrypoint().to_string(),
                signed_hash,
                "{name}: signed_hash mismatch"
            );
            assert_eq!(signed_tx.chain().as_str(), chain, "{name}: chain mismatch");
            let expected_authority = AccountId::parse_encoded(authority).map_or_else(
                |err| panic!("{name}: authority parse failed: {err}"),
                crate::account::ParsedAccountId::into_account_id,
            );
            assert_eq!(
                signed_tx.authority().to_string(),
                expected_authority.to_string(),
                "{name}: authority mismatch"
            );
            let creation_ms = u64::try_from(signed_tx.creation_time().as_millis())
                .expect("creation_time_ms fits u64");
            assert_eq!(
                creation_ms, creation_time_ms,
                "{name}: creation_time_ms mismatch"
            );
            let ttl_ms = signed_tx
                .time_to_live()
                .map(|ttl| u64::try_from(ttl.as_millis()).expect("time_to_live_ms fits u64"));
            assert_eq!(ttl_ms, time_to_live_ms, "{name}: time_to_live_ms mismatch");
            assert_eq!(
                signed_tx.nonce().map(NonZeroU32::get).map(u64::from),
                nonce,
                "{name}: nonce mismatch"
            );

            let signed_payload_bytes = norito::codec::encode_adaptive(signed_tx.payload());
            if signed_payload_bytes != payload_bytes {
                fn first_diff(left: &[u8], right: &[u8]) -> Option<(usize, u8, u8)> {
                    let shared_len = left.len().min(right.len());
                    for idx in 0..shared_len {
                        if left[idx] != right[idx] {
                            return Some((idx, left[idx], right[idx]));
                        }
                    }
                    None
                }

                let payload_from_fixture: TransactionPayload = {
                    let _guard = norito::core::PayloadCtxGuard::enter(&payload_bytes);
                    let mut cursor = std::io::Cursor::new(&payload_bytes);
                    let decoded: TransactionPayload = norito::codec::Decode::decode(&mut cursor)
                        .unwrap_or_else(|err| {
                            panic!("{name}: decode payload fixture bytes (bare): {err}")
                        });
                    let used =
                        usize::try_from(cursor.position()).expect("cursor.position fits usize");
                    assert_eq!(
                        used,
                        payload_bytes.len(),
                        "{name}: payload fixture contains trailing bytes"
                    );
                    decoded
                };

                let payload_equal = &payload_from_fixture == signed_tx.payload();
                let diff = first_diff(&signed_payload_bytes, &payload_bytes);
                let mut has_invalid_instruction = false;
                let mut register_role_stats: Option<(usize, usize)> = None;
                let mut instruction_count: Option<usize> = None;
                let mut instruction_types: Vec<&'static str> = Vec::new();
                if let Executable::Instructions(instrs) = signed_tx.instructions() {
                    instruction_count = Some(instrs.len());
                    for instr in instrs.iter() {
                        if instruction_types.len() < 16 {
                            instruction_types.push(crate::isi::Instruction::id(&**instr));
                        }
                        if instr
                            .as_any()
                            .downcast_ref::<crate::isi::InvalidInstruction>()
                            .is_some()
                        {
                            has_invalid_instruction = true;
                        }
                        if let Some(register) = instr
                            .as_any()
                            .downcast_ref::<crate::isi::Register<crate::role::Role>>()
                        {
                            let perms = register.object.inner.permissions.len();
                            let epochs = register.object.inner.permission_epochs.len();
                            register_role_stats = Some((perms, epochs));
                        }
                        if let Some(register_box) =
                            instr.as_any().downcast_ref::<crate::isi::RegisterBox>()
                        {
                            if let crate::isi::RegisterBox::Role(register) = register_box {
                                let perms = register.object.inner.permissions.len();
                                let epochs = register.object.inner.permission_epochs.len();
                                register_role_stats = Some((perms, epochs));
                            }
                        }
                    }
                }

                panic!(
                    "{name}: payload bytes mismatch after decode+re-encode (len encoded={}, len fixture={}, first_diff={diff:?}, payload_equal={payload_equal}, has_invalid_instruction={has_invalid_instruction}, register_role_stats={register_role_stats:?}, instruction_count={instruction_count:?}, instruction_types={instruction_types:?})",
                    signed_payload_bytes.len(),
                    payload_bytes.len(),
                );
            }

            let signed_reencoded = norito::codec::encode_adaptive(&signed_tx);
            assert_eq!(
                signed_reencoded, signed_bytes,
                "{name}: signed bytes mismatch after re-encode"
            );
        }
    }

    #[test]
    fn compact_hash_fixture_rejects_duplicate_property_keys() {
        let raw =
            fs::read_to_string(compact_hash_fixture_path()).expect("read compact hash fixture");
        let duplicated = format!("{raw}\ncanonical.hash=duplicate\n");
        let panic = std::panic::catch_unwind(|| parse_compact_hash_fixture(&duplicated));
        assert!(
            panic.is_err(),
            "duplicate compact fixture keys must fail closed"
        );
    }

    #[test]
    fn compact_external_entrypoint_golden_matches_native_hash_and_rejects_alias_encodings() {
        use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
        use sha2::Digest as _;
        let fixture = compact_hash_fixture();
        assert_eq!(fixture["schema.version"], "2");
        assert_eq!(fixture["source.fixture"], "transfer_asset");
        let versioned = BASE64
            .decode(fixture["versioned.base64"].as_bytes())
            .expect("compact hash fixture must contain valid base64");
        assert_eq!(
            versioned.len(),
            fixture["versioned.bytes"].parse::<usize>().unwrap()
        );
        assert_eq!(versioned.first(), Some(&1));
        assert_eq!(
            hex::encode(sha2::Sha256::digest(&versioned)),
            fixture["versioned.sha256"]
        );

        let transaction = SignedTransaction::decode_all_versioned(&versioned)
            .expect("compact hash fixture must decode as an exact versioned transaction");
        assert_eq!(transaction.encode_versioned(), versioned);
        assert_eq!(
            hex::encode(HashOf::new(transaction.payload()).as_ref()),
            fixture["payload.prehash"],
            "decoded payload prehash must match the shared signer golden"
        );
        let bare = norito::codec::encode_adaptive(&transaction);
        assert_eq!(bare.len(), fixture["bare.bytes"].parse::<usize>().unwrap());
        assert_eq!(bare, versioned[1..]);

        let payload = transaction.payload().encode();
        let mut canonical = 0_u32.to_le_bytes().to_vec();
        norito::core::write_len_to_vec(&mut canonical, payload.len() as u64);
        canonical.extend_from_slice(&payload);
        let entrypoint = TransactionEntrypoint::External(transaction);
        let expected_prefix = hex::decode(&fixture["canonical.prefix.hex"]).unwrap();
        assert!(canonical.starts_with(&expected_prefix));
        assert_eq!(
            hex::encode(iroha_crypto::Hash::new(&canonical).as_ref()),
            fixture["canonical.hash"],
            "the payload-only External identity preimage must match the shared golden"
        );
        assert_eq!(
            hex::encode(entrypoint.hash().as_ref()),
            fixture["canonical.hash"],
            "Rust entrypoint hash must match the shared Android/browser golden"
        );

        let mut overlong_signed = Vec::with_capacity(versioned.len() + 1);
        overlong_signed.extend_from_slice(&versioned[..2]);
        assert_eq!(versioned[1..3], [0x8a, 0x01]);
        overlong_signed.extend_from_slice(&[0x81, 0x00]);
        overlong_signed.extend_from_slice(&versioned[3..]);
        SignedTransaction::decode_all_versioned(&overlong_signed)
            .expect_err("overlong signed-transaction field length must be rejected");

        assert_eq!(
            expected_prefix.len(),
            6,
            "the shared fixture must exercise a two-byte External COMPACT_LEN"
        );
        assert_eq!(
            &canonical[..expected_prefix.len()],
            expected_prefix.as_slice()
        );
        let first_length_index = expected_prefix.len() - 2;
        assert_ne!(
            canonical[first_length_index] & 0x80,
            0,
            "the first External length byte must continue"
        );
        let terminal_index = expected_prefix.len() - 1;
        let terminal = canonical[terminal_index];
        assert_eq!(
            terminal & 0x80,
            0,
            "the second External length byte must terminate"
        );
        let mut overlong_entrypoint = Vec::with_capacity(canonical.len() + 1);
        overlong_entrypoint.extend_from_slice(&canonical[..terminal_index]);
        overlong_entrypoint.extend_from_slice(&[terminal | 0x80, 0x00]);
        overlong_entrypoint.extend_from_slice(&canonical[terminal_index + 1..]);
        assert_ne!(
            iroha_crypto::Hash::new(&overlong_entrypoint).as_ref(),
            entrypoint.hash().as_ref(),
            "an overlong External identity length must not alias the canonical hash"
        );

        let mut fixed_width_entrypoint = Vec::with_capacity(canonical.len() + 6);
        fixed_width_entrypoint.extend_from_slice(&canonical[..4]);
        fixed_width_entrypoint.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        fixed_width_entrypoint.extend_from_slice(&payload);
        assert_ne!(
            iroha_crypto::Hash::new(&fixed_width_entrypoint).as_ref(),
            entrypoint.hash().as_ref(),
            "fixed-u64 External identity length must not alias canonical COMPACT_LEN bytes"
        );

        let wire_entrypoint = norito::codec::encode_adaptive(&entrypoint);
        assert_ne!(
            wire_entrypoint, canonical,
            "the authorization-bearing entrypoint wire is distinct from its identity preimage"
        );
        assert_eq!(
            norito::codec::decode_adaptive::<TransactionEntrypoint>(&wire_entrypoint)
                .expect("canonical authorization-bearing entrypoint wire"),
            entrypoint
        );
    }
}
