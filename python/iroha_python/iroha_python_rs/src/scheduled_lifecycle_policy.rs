//! Additional caller-pinned PoC rollout policy; never a replacement for finality
//! or the authenticated native runtime selection. Omission leaves the future
//! committed-runtime corridor available; an explicit null height means never.

use super::*;

const MAX_POLICY_BYTES: usize = 4096;
// Include the endpoint's field names and canonical NetworkId/Hash strings.
// Each decoded byte may use a six-byte JSON Unicode escape, plus two quotes.
const MAX_POLICY_DECODED_STRING_BYTES: usize = 128;
const MAX_POLICY_ENCODED_STRING_BYTES: usize = 6 * MAX_POLICY_DECODED_STRING_BYTES + 2;

#[derive(norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ActivationPolicy {
    network_id: NetworkId,
    v2_activation_height: Option<u64>,
    committed_height: u64,
    active: bool,
    consensus_config_fingerprint: Hash,
}

impl ActivationPolicy {
    pub(super) fn parse(input: Option<&str>, network: NetworkId) -> PyResult<Option<Self>> {
        input
            .map(|input| {
                if input.is_empty() || input.len() > MAX_POLICY_BYTES {
                    return Err(refusal("lifecycle activation policy exceeds its bounds"));
                }
                json::preflight_slice(
                    input.as_bytes(),
                    json::JsonPreflightLimits::new(
                        MAX_POLICY_BYTES,
                        64,
                        MAX_POLICY_ENCODED_STRING_BYTES,
                        MAX_POLICY_DECODED_STRING_BYTES,
                        1024,
                        16,
                        16,
                        32,
                        64,
                        4,
                    ),
                )
                .map_err(refusal)?;
                let value: json::Value = json::from_json(input).map_err(refusal)?;
                let fields = [
                    "network_id",
                    "v2_activation_height",
                    "committed_height",
                    "active",
                    "consensus_config_fingerprint",
                ];
                if !value.as_object().is_some_and(|object| {
                    object.len() == fields.len()
                        && fields.iter().all(|name| object.contains_key(*name))
                }) {
                    return Err(refusal(
                        "lifecycle activation policy needs exactly its five endpoint fields",
                    ));
                }
                let policy: Self = json::from_json(input).map_err(refusal)?;
                if policy.network_id != network {
                    return Err(refusal(
                        "lifecycle activation policy belongs to another network",
                    ));
                }
                if policy.v2_activation_height.is_some_and(|height| height < 2) {
                    return Err(refusal(
                        "lifecycle activation policy height must be at least 2",
                    ));
                }
                if policy.active
                    != policy
                        .v2_activation_height
                        .is_some_and(|height| policy.committed_height >= height)
                {
                    return Err(refusal(
                        "lifecycle activation policy active status contradicts its heights",
                    ));
                }
                // Decode as the canonical Hash type, even though identity pinning
                // across reads belongs to the caller retaining this exact policy.
                let _ = policy.consensus_config_fingerprint;
                Ok(policy)
            })
            .transpose()
    }

    pub(super) fn require_tip(&self, height: u64) -> PyResult<()> {
        if self.committed_height < height {
            return Err(refusal(
                "lifecycle activation policy predates the certified proof tip",
            ));
        }
        Ok(())
    }

    pub(super) fn require_row(&self, height: u64, with_lifecycle: bool) -> PyResult<()> {
        let expected = self
            .v2_activation_height
            .is_some_and(|activation| height >= activation);
        if expected != with_lifecycle {
            return Err(refusal(
                "certified output lifecycle version contradicts the configured activation height",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_policy_json(network: NetworkId, fingerprint: Hash) -> String {
        json::to_json(&norito::json!({
            "network_id": network,
            "v2_activation_height": 10_u64,
            "committed_height": 20_u64,
            "active": true,
            "consensus_config_fingerprint": fingerprint,
        }))
        .unwrap()
    }

    fn escaped_ascii_json_string(canonical: &str) -> String {
        use std::fmt::Write as _;

        assert!(canonical.starts_with('"') && canonical.ends_with('"'));
        assert!(canonical.is_ascii());
        let mut escaped = String::from("\"");
        for byte in &canonical.as_bytes()[1..canonical.len() - 1] {
            write!(&mut escaped, "\\u{byte:04x}").unwrap();
        }
        escaped.push('"');
        escaped
    }

    #[test]
    fn lifecycle_policy_accepts_canonical_and_escaped_network_hash_strings() {
        Python::initialize();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network")));
        let fingerprint = Hash::new(b"config");
        let input = active_policy_json(network, fingerprint);
        let network_json = json::to_json(&network).unwrap();
        let fingerprint_json = json::to_json(&fingerprint).unwrap();
        let escaped_network = escaped_ascii_json_string(&network_json);
        let escaped_fingerprint = escaped_ascii_json_string(&fingerprint_json);
        assert!(escaped_fingerprint.len() > MAX_POLICY_DECODED_STRING_BYTES);
        for encoded in [
            input.clone(),
            input.replace(&fingerprint_json, &escaped_fingerprint),
            input.replace(&network_json, &escaped_network),
            input
                .replace(&fingerprint_json, &escaped_fingerprint)
                .replace(&network_json, &escaped_network),
        ] {
            let policy = ActivationPolicy::parse(Some(&encoded), network)
                .unwrap()
                .unwrap();
            assert_eq!(policy.network_id, network);
            assert_eq!(policy.consensus_config_fingerprint, fingerprint);
            assert_eq!(policy.v2_activation_height, Some(10));
            policy.require_tip(20).unwrap();
            policy.require_row(9, false).unwrap();
            policy.require_row(10, true).unwrap();
        }
    }

    #[test]
    fn lifecycle_policy_oversized_string_is_refused_by_lexical_guard() {
        Python::initialize();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network")));
        let fingerprint = Hash::new(b"config");
        let input = active_policy_json(network, fingerprint);
        let oversized = json::to_json(&"a".repeat(MAX_POLICY_DECODED_STRING_BYTES + 1)).unwrap();
        let canonical = json::to_json(&fingerprint).unwrap();
        for replacement in [oversized.clone(), escaped_ascii_json_string(&oversized)] {
            let encoded = input.replace(&canonical, &replacement);
            assert!(encoded.len() < MAX_POLICY_BYTES);
            let error = ActivationPolicy::parse(Some(&encoded), network)
                .err()
                .expect("oversized decoded fingerprint must fail before typed decoding")
                .to_string();
            assert!(error.contains("DecodedStringBytes"), "{error}");
            assert!(error.contains("129 exceeds limit 128"), "{error}");
        }
    }

    #[test]
    fn lifecycle_policy_raw_body_limit_accepts_exact_bound_and_refuses_one_more() {
        Python::initialize();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network")));
        let mut input = active_policy_json(network, Hash::new(b"config"));
        input.push_str(&" ".repeat(MAX_POLICY_BYTES - input.len()));
        assert_eq!(input.len(), MAX_POLICY_BYTES);
        assert!(
            ActivationPolicy::parse(Some(&input), network)
                .unwrap()
                .is_some()
        );
        input.push(' ');
        let error = ActivationPolicy::parse(Some(&input), network)
            .err()
            .expect("oversized body must fail before JSON preflight or owned decoding")
            .to_string();
        assert!(error.contains("policy exceeds its bounds"), "{error}");
    }

    #[test]
    fn lifecycle_policy_boundary_includes_all_row_kinds_and_explicit_never() {
        Python::initialize();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network")));
        for height in [None, Some(10)] {
            let active = height.is_some();
            let fingerprint = Hash::new(b"config");
            let input = json::to_json(&norito::json!({"network_id": network,
                "v2_activation_height": height, "committed_height": 20_u64,
                "active": active, "consensus_config_fingerprint": fingerprint}))
            .unwrap();
            let policy = ActivationPolicy::parse(Some(&input), network)
                .unwrap()
                .unwrap();
            // require_row uses only the trace-presence accessor common to all
            // three typed row families; no Network-only dispatch can skip one.
            for cut in [1, 9, 10, 11, 20] {
                let expected = height.is_some_and(|start| cut >= start);
                policy.require_row(cut, expected).unwrap();
                assert!(
                    policy
                        .require_row(cut, !expected)
                        .unwrap_err()
                        .to_string()
                        .contains("version contradicts")
                );
            }
            let mut value: json::Value = json::from_json(&input).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert("active".into(), json::Value::Bool(height.is_none()));
            assert!(
                ActivationPolicy::parse(Some(&json::to_json(&value).unwrap()), network).is_err()
            );
            value
                .as_object_mut()
                .unwrap()
                .insert("unknown".into(), json::Value::Bool(true));
            assert!(
                ActivationPolicy::parse(Some(&json::to_json(&value).unwrap()), network).is_err()
            );
        }
        assert!(ActivationPolicy::parse(None, network).unwrap().is_none());
    }
}
