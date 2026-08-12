// Closed OpenAPI authentication contract for exact-account runtime/governance endpoints.

fn secure_runtime_governance_account_paths(paths: &mut Map) {
    for (path, authenticated_methods) in [
        ("/v1/zk/verify-batch", &["post"][..]),
        ("/v1/zk/ivm/derive", &["post"][..]),
        ("/v1/zk/attachments", &["get", "post"][..]),
        ("/v1/zk/attachments/{id}", &["get", "delete"][..]),
        ("/v1/zk/attachments/count", &["get"][..]),
        ("/v1/zk/roots", &["post"][..]),
        ("/v1/zk/merkle-path", &["post"][..]),
        ("/v1/zk/vote/tally", &["post"][..]),
        ("/v1/runtime/abi/active", &["get"][..]),
        ("/v1/runtime/metrics", &["get"][..]),
        ("/v1/node/capabilities", &["get"][..]),
        ("/v1/privacy/capabilities", &["get"][..]),
        ("/v1/node/query/projection/checkpoint", &["get"][..]),
        ("/v1/ministry/agenda/proposals/draft", &["post"][..]),
        ("/v1/ministry/agenda/proposals/{proposal_id}", &["get"][..]),
        ("/v1/gov/proposals/deploy-contract", &["post"][..]),
        ("/v1/gov/proposals/sccp-route-governance", &["post"][..]),
        ("/v1/gov/capabilities", &["get"][..]),
        ("/v1/gov/citizens/draft", &["post"][..]),
        ("/v1/validation-fee/policy/current/proof", &["post"][..]),
        ("/v1/validation-fee/proposals", &["get"][..]),
        ("/v1/validation-fee/proposals/draft", &["post"][..]),
        ("/v1/validation-fee/proposals/{proposal_id}", &["get"][..]),
        (
            "/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft",
            &["post"][..],
        ),
        ("/v1/gov/proposals/{id}", &["get"][..]),
        ("/v1/gov/locks/{rid}", &["get"][..]),
        ("/v1/gov/referenda/{id}", &["get"][..]),
        ("/v1/gov/tally/{id}", &["get"][..]),
        ("/v1/gov/protected-namespaces", &["get"][..]),
        ("/v1/gov/unlocks/stats", &["get"][..]),
        ("/v1/gov/contracts/{contract_address}", &["get"][..]),
        ("/v1/gov/enact", &["post"][..]),
        ("/v1/gov/council/current", &["get"][..]),
        ("/v1/gov/citizens", &["get"][..]),
        ("/v1/gov/citizens/{account_id}", &["get"][..]),
    ] {
        let Some(Value::Object(methods)) = paths.get_mut(path) else {
            continue;
        };
        for method in authenticated_methods {
            let Some(operation) = methods.get_mut(*method).and_then(Value::as_object_mut) else {
                continue;
            };
            let parameters = operation
                .entry("parameters".to_owned())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(parameters) = parameters {
                for parameter in canonical_request_auth_header_parameters() {
                    let name = parameter.get("name").and_then(Value::as_str);
                    if !parameters
                        .iter()
                        .any(|existing| existing.get("name").and_then(Value::as_str) == name)
                    {
                        parameters.push(parameter);
                    }
                }
            }
            insert_canonical_request_auth_contract(operation);
            insert_private_no_store_response_contract(operation);
        }
    }
}

#[cfg(test)]
mod zk_attachment_auth_tests {
    use super::*;

    #[test]
    fn account_operations_publish_exact_auth_and_private_responses() {
        let document = generate_spec();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .expect("OpenAPI paths");
        for (path, method_names) in [
            ("/v1/zk/verify-batch", &["post"][..]),
            ("/v1/zk/ivm/derive", &["post"][..]),
            ("/v1/zk/attachments", &["get", "post"][..]),
            ("/v1/zk/attachments/{id}", &["get", "delete"][..]),
            ("/v1/zk/attachments/count", &["get"][..]),
            ("/v1/zk/roots", &["post"][..]),
            ("/v1/zk/merkle-path", &["post"][..]),
            ("/v1/zk/vote/tally", &["post"][..]),
            ("/v1/runtime/abi/active", &["get"][..]),
            ("/v1/runtime/metrics", &["get"][..]),
            ("/v1/node/capabilities", &["get"][..]),
            ("/v1/privacy/capabilities", &["get"][..]),
            ("/v1/node/query/projection/checkpoint", &["get"][..]),
            ("/v1/ministry/agenda/proposals/draft", &["post"][..]),
            ("/v1/ministry/agenda/proposals/{proposal_id}", &["get"][..]),
            ("/v1/gov/proposals/deploy-contract", &["post"][..]),
            ("/v1/gov/proposals/sccp-route-governance", &["post"][..]),
            ("/v1/gov/capabilities", &["get"][..]),
            ("/v1/gov/citizens/draft", &["post"][..]),
            ("/v1/validation-fee/policy/current/proof", &["post"][..]),
            ("/v1/validation-fee/proposals", &["get"][..]),
            ("/v1/validation-fee/proposals/draft", &["post"][..]),
            ("/v1/validation-fee/proposals/{proposal_id}", &["get"][..]),
            (
                "/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft",
                &["post"][..],
            ),
            ("/v1/gov/proposals/{id}", &["get"][..]),
            ("/v1/gov/locks/{rid}", &["get"][..]),
            ("/v1/gov/referenda/{id}", &["get"][..]),
            ("/v1/gov/tally/{id}", &["get"][..]),
            ("/v1/gov/protected-namespaces", &["get"][..]),
            ("/v1/gov/unlocks/stats", &["get"][..]),
            ("/v1/gov/contracts/{contract_address}", &["get"][..]),
            ("/v1/gov/enact", &["post"][..]),
            ("/v1/gov/council/current", &["get"][..]),
            ("/v1/gov/citizens", &["get"][..]),
            ("/v1/gov/citizens/{account_id}", &["get"][..]),
        ] {
            let path_item = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("attachment path");
            for method in method_names {
                let operation = path_item
                    .get(*method)
                    .and_then(Value::as_object)
                    .expect("attachment operation");
                assert!(operation.contains_key("security"), "{method} {path}");
                let parameters = operation
                    .get("parameters")
                    .and_then(Value::as_array)
                    .expect("canonical header parameters");
                assert_eq!(
                    parameters
                        .iter()
                        .filter(|parameter| {
                            parameter.get("name").and_then(Value::as_str) == Some("X-Iroha-Account")
                        })
                        .count(),
                    1,
                    "{method} {path} must publish one account header"
                );
                let responses = operation
                    .get("responses")
                    .and_then(Value::as_object)
                    .expect("attachment responses");
                assert!(responses.values().all(|response| {
                    response
                        .get("headers")
                        .and_then(Value::as_object)
                        .is_some_and(|headers| headers.contains_key("Cache-Control"))
                }));
            }
        }

        for (path, method) in [
            ("/v1/runtime/abi/hash", "get"),
            ("/v1/gov/finalize", "post"),
            ("/v1/gov/protected-namespaces", "post"),
        ] {
            let operation = paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|methods| methods.get(method))
                .and_then(Value::as_object)
                .expect("deliberately non-account operation");
            assert!(
                !operation
                    .get("parameters")
                    .and_then(Value::as_array)
                    .is_some_and(|parameters| parameters.iter().any(|parameter| {
                        parameter.get("name").and_then(Value::as_str) == Some("X-Iroha-Account")
                    })),
                "{method} {path} must retain its non-account admission contract"
            );
        }
    }
}
