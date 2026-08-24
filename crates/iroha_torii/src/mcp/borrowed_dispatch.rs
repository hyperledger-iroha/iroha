// Borrowed MCP request projections used to validate and encode source-sized
// bodies without recursively cloning the decoded JSON value tree.
#[derive(Debug)]
enum BorrowedMcpJson<'a> {
    Value(&'a Value),
    Object(BorrowedMcpJsonObject<'a>),
}

#[derive(Debug)]
struct BorrowedMcpJsonObject<'a> {
    entries: Vec<(&'a str, BorrowedMcpJson<'a>)>,
}

impl<'a> BorrowedMcpJsonObject<'a> {
    fn try_with_capacity(capacity: usize, context: &str) -> Result<Self, String> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| format!("failed to reserve {context}"))?;
        Ok(Self { entries })
    }

    fn insert_value(&mut self, key: &'a str, value: &'a Value) {
        self.entries.push((key, BorrowedMcpJson::Value(value)));
    }

    fn insert_object(&mut self, key: &'a str, value: Self) {
        self.entries.push((key, BorrowedMcpJson::Object(value)));
    }

    fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(candidate, _)| *candidate == key)
    }

    fn get(&self, key: &str) -> Option<&'a Value> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .and_then(|(_, value)| match value {
                BorrowedMcpJson::Value(value) => Some(*value),
                BorrowedMcpJson::Object(_) => None,
            })
    }

    fn sorted(mut self) -> Self {
        self.entries
            .sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        self
    }
}

impl<'a> BorrowedMcpJson<'a> {
    fn get(&self, key: &str) -> Option<&'a Value> {
        match self {
            Self::Value(value) => value.as_object().and_then(|object| object.get(key)),
            Self::Object(object) => object.get(key),
        }
    }
}

impl FastJsonWrite for BorrowedMcpJsonObject<'_> {
    fn write_json(&self, output: &mut String) {
        output.push('{');
        for (index, (key, value)) in self.entries.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            json::write_json_string(key, output);
            output.push(':');
            value.write_json(output);
        }
        output.push('}');
    }

    fn write_json_to(&self, output: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        output.begin_container()?;
        output.push('{')?;
        for (index, (key, value)) in self.entries.iter().enumerate() {
            if index != 0 {
                output.push(',')?;
            }
            json::write_json_string_to(key, output)?;
            output.push(':')?;
            value.write_json_to(output)?;
        }
        output.push('}')?;
        output.end_container();
        Ok(())
    }
}

impl FastJsonWrite for BorrowedMcpJson<'_> {
    fn write_json(&self, output: &mut String) {
        match self {
            Self::Value(value) => FastJsonWrite::write_json(*value, output),
            Self::Object(object) => object.write_json(output),
        }
    }

    fn write_json_to(&self, output: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        match self {
            Self::Value(value) => FastJsonWrite::write_json_to(*value, output),
            Self::Object(object) => object.write_json_to(output),
        }
    }
}

fn encode_mcp_json_body<T: FastJsonWrite>(value: &T, context: &str) -> Result<Vec<u8>, String> {
    json::to_json_bounded_boxed(value, usize::MAX)
        .map(|bytes| bytes.into_vec())
        .map_err(|error| format!("{context}: {error}"))
}

fn decode_base64_any(input: &str, invalid_message: &str) -> Result<Vec<u8>, String> {
    let remainder = input.len() % 4;
    if remainder == 1 {
        return Err(invalid_message.to_owned());
    }
    let padding = if input.ends_with("==") {
        2
    } else if input.ends_with('=') {
        1
    } else {
        0
    };
    if padding != 0 && remainder != 0 {
        return Err(invalid_message.to_owned());
    }
    let decoded_len = input
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| {
            bytes.checked_add(match remainder {
                2 => 1,
                3 => 2,
                _ => 0,
            })
        })
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or_else(|| invalid_message.to_owned())?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(decoded_len)
        .map_err(|_| "failed to reserve decoded base64 payload".to_owned())?;
    decoded.resize(decoded_len, 0);
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(written) = engine.decode_slice(input.as_bytes(), &mut decoded)
            && written == decoded_len
        {
            return Ok(decoded);
        }
    }
    Err(invalid_message.to_owned())
}

fn build_query_envelope_body(arguments: &Map) -> Result<BorrowedMcpJson<'_>, String> {
    if let Some(body) = arguments.get("body") {
        body.as_object()
            .ok_or_else(|| "`body` must be an object".to_owned())?;
        return Ok(BorrowedMcpJson::Value(body));
    }
    let mut env =
        BorrowedMcpJsonObject::try_with_capacity(7, "borrowed MCP query-envelope fields")?;
    for key in [
        "query",
        "filter",
        "select",
        "aggregate",
        "sort",
        "fetch_size",
    ] {
        if let Some(value) = arguments.get(key) {
            env.insert_value(key, value);
        }
    }
    if let Some(pagination) = arguments.get("pagination") {
        pagination
            .as_object()
            .ok_or_else(|| "`pagination` must be an object".to_owned())?;
        env.insert_value("pagination", pagination);
    } else {
        let mut pagination =
            BorrowedMcpJsonObject::try_with_capacity(2, "borrowed MCP pagination fields")?;
        if let Some(limit) = arguments.get("limit") {
            pagination.insert_value("limit", limit);
        }
        if let Some(offset) = arguments.get("offset") {
            pagination.insert_value("offset", offset);
        }
        if !pagination.entries.is_empty() {
            env.insert_object("pagination", pagination.sorted());
        }
    }
    Ok(BorrowedMcpJson::Object(env.sorted()))
}

fn build_object_body_or_default(arguments: &Map) -> Result<BorrowedMcpJson<'_>, String> {
    if let Some(body) = arguments.get("body") {
        body.as_object()
            .ok_or_else(|| "`body` must be an object".to_owned())?;
        return Ok(BorrowedMcpJson::Value(body));
    }
    Ok(BorrowedMcpJson::Object(
        BorrowedMcpJsonObject::try_with_capacity(0, "empty MCP body")?,
    ))
}

fn build_required_exact_object_body<'a>(
    arguments: &'a Map,
    allowed_fields: &[&str],
    required_fields: &[&str],
    context: &str,
) -> Result<BorrowedMcpJson<'a>, String> {
    let body = arguments
        .get("body")
        .ok_or_else(|| "`body` is required".to_owned())?;
    let body_object = body
        .as_object()
        .ok_or_else(|| "`body` must be an object".to_owned())?;
    reject_unknown_arguments(body_object, allowed_fields, context)?;
    if let Some(field) = required_fields
        .iter()
        .find(|field| !body_object.contains_key(**field))
    {
        return Err(format!("`{field}` is required for {context}"));
    }
    Ok(BorrowedMcpJson::Value(body))
}

fn build_object_body_or_flat_shortcuts<'a>(
    arguments: &'a Map,
    ignored_keys: &[&str],
) -> Result<BorrowedMcpJson<'a>, String> {
    if let Some(body) = arguments.get("body") {
        body.as_object()
            .ok_or_else(|| "`body` must be an object".to_owned())?;
        return Ok(BorrowedMcpJson::Value(body));
    }
    let field_count = arguments
        .iter()
        .filter(|(key, value)| {
            !ignored_keys.iter().any(|ignored| key == ignored) && !value.is_null()
        })
        .count();
    let mut payload =
        BorrowedMcpJsonObject::try_with_capacity(field_count, "borrowed MCP flat body fields")?;
    for (key, value) in arguments {
        if ignored_keys.iter().any(|ignored| key == ignored) || value.is_null() {
            continue;
        }
        payload.insert_value(key, value);
    }
    if payload.entries.is_empty() {
        return Err("`body` is required (or provide flat top-level fields)".to_owned());
    }
    Ok(BorrowedMcpJson::Object(payload.sorted()))
}

fn build_accounts_onboard_exact_body<'a>(
    arguments: &'a Map,
    allowed_fields: &[&str],
    required_fields: &[&str],
) -> Result<BorrowedMcpJson<'a>, String> {
    if let Some(body) = arguments.get("body") {
        let body_object = body
            .as_object()
            .ok_or_else(|| "`body` must be an object".to_owned())?;
        if let Some(field) = body_object
            .keys()
            .find(|field| !allowed_fields.contains(&field.as_str()))
        {
            return Err(format!(
                "unsupported account onboarding field `{field}`; tokens, keys, and legacy identity fields are forbidden"
            ));
        }
        if let Some(field) = required_fields
            .iter()
            .find(|field| !body_object.contains_key(**field))
        {
            return Err(format!("`{field}` is required"));
        }
        return Ok(BorrowedMcpJson::Value(body));
    }
    let field_count = arguments
        .iter()
        .filter(|(key, value)| !matches!(key.as_str(), "headers" | "accept") && !value.is_null())
        .count();
    let mut payload = BorrowedMcpJsonObject::try_with_capacity(
        field_count,
        "borrowed account-onboarding body fields",
    )?;
    for (key, value) in arguments {
        if matches!(key.as_str(), "headers" | "accept") || value.is_null() {
            continue;
        }
        if !allowed_fields.contains(&key.as_str()) {
            return Err(format!(
                "unsupported account onboarding field `{key}`; tokens, keys, and legacy identity fields are forbidden"
            ));
        }
        payload.insert_value(key, value);
    }
    if let Some(field) = required_fields
        .iter()
        .find(|field| !payload.contains_key(**field))
    {
        return Err(format!("`{field}` is required"));
    }
    Ok(BorrowedMcpJson::Object(payload.sorted()))
}

fn build_accounts_onboard_plan_body(arguments: &Map) -> Result<BorrowedMcpJson<'_>, String> {
    build_accounts_onboard_exact_body(
        arguments,
        &["version", "alias", "account_id", "permissions"],
        &["version", "alias", "account_id"],
    )
}

fn build_accounts_onboard_apply_body(arguments: &Map) -> Result<BorrowedMcpJson<'_>, String> {
    build_accounts_onboard_exact_body(arguments, &["receipt"], &["receipt"])
}

fn build_accounts_faucet_body(arguments: &Map) -> Result<BorrowedMcpJson<'_>, String> {
    if let Some(body) = arguments.get("body") {
        body.as_object()
            .ok_or_else(|| "`body` must be an object".to_owned())?;
        return Ok(BorrowedMcpJson::Value(body));
    }
    let account_id = arguments
        .get("account_id")
        .filter(|value| value.as_str().is_some())
        .ok_or_else(|| "`account_id` is required (or provide `body.account_id`)".to_owned())?;
    let mut payload =
        BorrowedMcpJsonObject::try_with_capacity(1, "borrowed account-faucet body field")?;
    payload.insert_value("account_id", account_id);
    Ok(BorrowedMcpJson::Object(payload))
}

fn require_borrowed_governance_body_string<'a>(
    body: &BorrowedMcpJson<'a>,
    field: &str,
) -> Result<&'a str, String> {
    body.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{field}` must be a string"))
}

fn require_borrowed_governance_selector_body<'a>(
    body: &BorrowedMcpJson<'a>,
    field: &str,
) -> Result<&'a str, String> {
    let value = require_borrowed_governance_body_string(body, field)?;
    require_governance_selector_v1(field, value)?;
    Ok(value)
}

fn require_borrowed_governance_proposal_id_body<'a>(
    body: &BorrowedMcpJson<'a>,
    field: &str,
) -> Result<&'a str, String> {
    let value = require_borrowed_governance_body_string(body, field)?;
    require_governance_proposal_id_v1(field, value)?;
    Ok(value)
}

fn try_begin_form_query(path: &mut String) -> Result<usize, String> {
    let query_start = path
        .len()
        .checked_add(1)
        .ok_or_else(|| "MCP query route length overflow".to_owned())?;
    path.try_reserve_exact(1)
        .map_err(|_| "failed to reserve MCP query route".to_owned())?;
    path.push('?');
    Ok(query_start)
}

fn try_append_form_component(output: &mut String, value: &str) {
    for component in url::form_urlencoded::byte_serialize(value.as_bytes()) {
        output.push_str(component);
    }
}

fn form_component_encoded_len(value: &str) -> Result<usize, String> {
    value.bytes().try_fold(0_usize, |length, byte| {
        let encoded_bytes = if matches!(
            byte,
            b' ' | b'*' | b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z'
        ) {
            1
        } else {
            3
        };
        length
            .checked_add(encoded_bytes)
            .ok_or_else(|| "MCP query route length overflow".to_owned())
    })
}

fn try_append_form_pair(
    output: &mut String,
    query_start: usize,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let separator_bytes = usize::from(output.len() > query_start);
    let key_bytes = form_component_encoded_len(key)?;
    let value_bytes = form_component_encoded_len(value)?;
    let additional = key_bytes
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(value_bytes))
        .and_then(|bytes| bytes.checked_add(separator_bytes))
        .ok_or_else(|| "MCP query route length overflow".to_owned())?;
    output
        .try_reserve_exact(additional)
        .map_err(|_| "failed to reserve MCP query route".to_owned())?;
    if separator_bytes != 0 {
        output.push('&');
    }
    try_append_form_component(output, key);
    output.push('=');
    try_append_form_component(output, value);
    Ok(())
}

fn try_append_percent_encoded_path_component(
    output: &mut String,
    value: &str,
) -> Result<(), String> {
    let additional = percent_encoded_path_component_len(value)?;
    output
        .try_reserve_exact(additional)
        .map_err(|_| "failed to reserve MCP route template".to_owned())?;
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    Ok(())
}

fn percent_encoded_path_component_len(value: &str) -> Result<usize, String> {
    value.bytes().try_fold(0_usize, |length, byte| {
        let encoded_bytes =
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                1
            } else {
                3
            };
        length
            .checked_add(encoded_bytes)
            .ok_or_else(|| "MCP route template length overflow".to_owned())
    })
}

fn append_borrowed_query_pairs<'a>(
    mut path: String,
    pairs: impl IntoIterator<Item = (&'a str, &'a Value)>,
) -> Result<String, String> {
    let mut query_start = None;
    for (key, value) in pairs {
        if value.is_null() {
            continue;
        }
        let rendered;
        let value = if let Some(value) = value.as_str() {
            value
        } else {
            rendered =
                value_to_string(value).ok_or_else(|| format!("invalid query value for `{key}`"))?;
            rendered.as_str()
        };
        let query_start = match query_start {
            Some(query_start) => query_start,
            None => *query_start.insert(try_begin_form_query(&mut path)?),
        };
        try_append_form_pair(&mut path, query_start, key, value)?;
    }
    Ok(path)
}

fn append_query_arguments(
    path: String,
    arguments: &Map,
    ignored_keys: &[&str],
) -> Result<String, String> {
    if let Some(query) = arguments.get("query") {
        let query = query
            .as_object()
            .ok_or_else(|| "`query` must be an object".to_owned())?;
        return append_borrowed_query_pairs(
            path,
            query.iter().map(|(key, value)| (key.as_str(), value)),
        );
    }
    append_borrowed_query_pairs(
        path,
        arguments
            .iter()
            .filter(|(key, _)| !ignored_keys.iter().any(|ignored| key == ignored))
            .map(|(key, value)| (key.as_str(), value)),
    )
}

fn append_named_query_fields(
    path: String,
    arguments: &Map,
    fields: &[&str],
) -> Result<String, String> {
    append_borrowed_query_pairs(
        path,
        fields
            .iter()
            .filter_map(|field| arguments.get(*field).map(|value| (*field, value))),
    )
}

fn append_transaction_status_query(
    mut path: String,
    arguments: &Map,
    transaction_hash: &str,
) -> Result<String, String> {
    let nested_query = arguments
        .get("query")
        .map(|query| {
            query
                .as_object()
                .ok_or_else(|| "`query` must be an object".to_owned())
        })
        .transpose()?;
    let source = nested_query.unwrap_or(arguments);
    let query_start = try_begin_form_query(&mut path)?;
    try_append_form_pair(
        &mut path,
        query_start,
        "hash",
        canonical_transaction_hash(transaction_hash)?,
    )?;
    for (key, value) in source {
        let ignored = if nested_query.is_some() {
            key == "hash"
        } else {
            matches!(key.as_str(), "query" | "headers" | "accept" | "hash")
        };
        if ignored || value.is_null() {
            continue;
        }
        let rendered;
        let value = if let Some(value) = value.as_str() {
            value
        } else {
            rendered =
                value_to_string(value).ok_or_else(|| format!("invalid query value for `{key}`"))?;
            rendered.as_str()
        };
        try_append_form_pair(&mut path, query_start, key, value)?;
    }
    Ok(path)
}

fn transaction_status_poll_route(transaction_hash: &str) -> Result<String, String> {
    const BASE: &str = "/v1/pipeline/transactions/status?hash=";
    const SUFFIX: &str = "&scope=local";
    let transaction_hash = canonical_transaction_hash(transaction_hash)?;
    let capacity = BASE
        .len()
        .checked_add(transaction_hash.len())
        .and_then(|capacity| capacity.checked_add(SUFFIX.len()))
        .ok_or_else(|| "transaction status route length overflow".to_owned())?;
    let mut route = String::new();
    route
        .try_reserve_exact(capacity)
        .map_err(|_| "failed to reserve transaction status route".to_owned())?;
    route.push_str(BASE);
    route.push_str(transaction_hash);
    route.push_str(SUFFIX);
    Ok(route)
}

async fn dispatch_iroha_transaction_status_poll(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    transaction_hash: &str,
    extra_headers: Option<&Value>,
    accept: &str,
) -> Result<Value, String> {
    let route = transaction_status_poll_route(transaction_hash)?;
    dispatch_route_borrowed(
        app,
        inbound_headers,
        Method::GET,
        &route,
        extra_headers,
        Vec::new(),
        None,
        Some(accept),
    )
    .await
}

fn extra_headers_contain_authorization(value: &Value) -> bool {
    value.as_object().is_some_and(|headers| {
        headers.contains_key("Authorization") || headers.contains_key("authorization")
    })
}

fn connect_management_authorization_value(token: &str) -> Result<HeaderValue, String> {
    if token.len() != 43
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("management token must be canonical unpadded base64url".to_owned());
    }
    const PREFIX: &str = "Bearer ";
    let capacity = PREFIX
        .len()
        .checked_add(token.len())
        .ok_or_else(|| "management authorization length overflow".to_owned())?;
    let mut authorization = String::new();
    authorization
        .try_reserve_exact(capacity)
        .map_err(|_| "failed to reserve management authorization header".to_owned())?;
    authorization.push_str(PREFIX);
    authorization.push_str(token);
    HeaderValue::from_str(&authorization)
        .map_err(|error| format!("invalid management authorization header: {error}"))
}
