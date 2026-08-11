// Operator-authenticated application webhook registry contract.

fn webhook_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/webhooks".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "Webhooks",
                "List webhooks.",
                "List registered webhooks with a replay-resistant operator signature.",
                "#/components/schemas/JsonValue",
                operator_signature_header_parameters(),
            );
            let post_op = json_post_operation(
                "Webhooks",
                "Create a webhook.",
                "Create a webhook subscription with a replay-resistant operator signature over the exact body.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                operator_signature_header_parameters(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/webhooks/{id}".to_owned(),
        Value::Object(json_delete_operation(
            "Webhooks",
            "Delete a webhook.",
            "Delete a webhook subscription with a replay-resistant operator signature.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Webhook identifier.")]
                .into_iter()
                .chain(operator_signature_header_parameters())
                .collect(),
        )),
    );
    paths
}
