// Complete memory accounting for application-API routed reads.

#[derive(Clone, Copy, Debug)]
struct ToriiRoutedReadDecodePlan {
    limits: norito::DecodeLimits,
    canonical_limit_bytes: usize,
}

#[derive(Debug)]
struct ToriiBoundedNoritoPayload<T> {
    value: T,
    canonical_bytes: Vec<u8>,
}

/// Logical ledger backed by one existing query-fanout working-set permit.
///
/// Route bodies are decoded sequentially. Norito's measured decode scope
/// records every allocation request made by JSON or binary decoders, so a
/// successful value can be retained against the aggregate phase without a
/// guessed per-element allowance or a per-route fair split. Candidate encodings
/// and the final response use their existing independent envelope phases.
#[derive(Clone, Copy, Debug)]
struct ToriiRoutedReadMemoryBudget {
    envelope: QueryFanoutMemoryEnvelope,
    configured_body_limit_bytes: usize,
    retained_decoded_bytes: usize,
    retained_canonical_bytes: usize,
    merge_allocated_bytes: usize,
}

impl ToriiRoutedReadMemoryBudget {
    fn new(working_set_bytes: usize, configured_body_limit_bytes: usize) -> Result<Self, Response> {
        Ok(Self {
            envelope: QueryFanoutMemoryEnvelope::for_body_admission(working_set_bytes)?,
            configured_body_limit_bytes,
            retained_decoded_bytes: 0,
            retained_canonical_bytes: 0,
            merge_allocated_bytes: 0,
        })
    }

    /// Bound the complete routed request retained across transport retries.
    fn admit_request_bytes(self, bytes: usize) -> Result<(), Response> {
        torii_routed_read_ensure(
            "request representation",
            bytes,
            self.envelope.route_body_bytes,
        )
    }

    fn route_body_limit(self) -> usize {
        self.configured_body_limit_bytes
            .min(self.envelope.route_body_bytes)
    }

    fn final_body_limit(self) -> usize {
        self.configured_body_limit_bytes
            .min(self.envelope.final_body_bytes)
    }

    fn canonical_remaining(self) -> Result<usize, Response> {
        let remaining = self
            .envelope
            .candidate_encoded_bytes
            .checked_sub(self.retained_canonical_bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        if remaining == 0 {
            return Err(torii_routed_read_capacity_response(
                "candidate encodings",
                1,
                0,
            ));
        }
        Ok(remaining)
    }

    fn decode_plan(self, raw_body_bytes: usize) -> Result<ToriiRoutedReadDecodePlan, Response> {
        torii_routed_read_ensure("raw route body", raw_body_bytes, self.route_body_limit())?;
        let phase_bytes = self.envelope.decode_allocated_bytes;
        if phase_bytes == 0 {
            return Err(torii_routed_read_capacity_response(
                "decode allocations",
                1,
                0,
            ));
        }
        Ok(ToriiRoutedReadDecodePlan {
            // Core charges at least one allocation byte for every sequence
            // element. Using the physical phase for all four dimensions keeps
            // compressed expansion bounded without inventing a route cap.
            limits: norito::DecodeLimits::new(
                phase_bytes,
                phase_bytes,
                phase_bytes,
                phase_bytes,
                norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
            ),
            canonical_limit_bytes: self.canonical_remaining()?,
        })
    }

    fn json_profile(
        self,
        body: &[u8],
        plan: ToriiRoutedReadDecodePlan,
    ) -> Result<norito::json::JsonPreflightProfile, Response> {
        norito::json::preflight_slice(
            body,
            norito::json::JsonPreflightLimits::from_decode_limits(
                self.route_body_limit(),
                plan.limits,
            ),
        )
        .map_err(torii_routed_read_json_preflight_response)
    }

    fn retain_decode_usage(
        &mut self,
        usage: norito::core::DecodeAllocationUsage,
    ) -> Result<(), Response> {
        let decoded_bytes = usage.total_allocated_bytes();
        torii_routed_read_ensure(
            "current decode allocations",
            decoded_bytes,
            self.envelope.decode_allocated_bytes,
        )?;
        self.admit_retained_allocation(decoded_bytes)
    }

    fn admit_retained_allocation(&mut self, bytes: usize) -> Result<(), Response> {
        let retained = self
            .retained_decoded_bytes
            .checked_add(bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        torii_routed_read_ensure(
            "retained decoded values and collection storage",
            retained,
            self.envelope.accumulator_retained_bytes,
        )?;
        self.retained_decoded_bytes = retained;
        Ok(())
    }

    /// Allocate a retained payload vector inside the accumulator phase.
    fn try_retained_vec<T>(&mut self, capacity: usize) -> Result<Vec<T>, Response> {
        let mut values = Vec::new();
        self.ensure_retained_vec_capacity(&mut values, capacity)?;
        Ok(values)
    }

    /// Grow a retained payload vector only after its requested Rust-layout
    /// capacity is admitted. The old allocation during `realloc` fits in the
    /// independent transient phase because it is no larger than the admitted
    /// accumulator allocation.
    fn ensure_retained_vec_capacity<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), Response> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or_else(torii_routed_read_accounting_response)?;
        if required <= values.capacity() {
            return Ok(());
        }
        let element_bytes = core::mem::size_of::<T>();
        let old_bytes = values
            .capacity()
            .checked_mul(element_bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        let requested_bytes = required
            .checked_mul(element_bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        let requested_extra = requested_bytes
            .checked_sub(old_bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        self.admit_retained_allocation(requested_extra)?;
        values
            .try_reserve_exact(required - values.len())
            .map_err(|_| {
                torii_routed_read_allocation_response("retained payload vector", requested_bytes)
            })?;
        let allocated_bytes = values
            .capacity()
            .checked_mul(element_bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        if let Some(extra_bytes) = allocated_bytes.checked_sub(requested_bytes)
            && extra_bytes != 0
        {
            self.admit_retained_allocation(extra_bytes)?;
        }
        Ok(())
    }

    fn push_retained<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Response> {
        self.ensure_retained_vec_capacity(values, 1)?;
        values.push(value);
        Ok(())
    }

    fn retain_canonical_bytes(&mut self, bytes: usize) -> Result<(), Response> {
        let retained = self
            .retained_canonical_bytes
            .checked_add(bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        torii_routed_read_ensure(
            "retained candidate encodings",
            retained,
            self.envelope.candidate_encoded_bytes,
        )?;
        self.retained_canonical_bytes = retained;
        Ok(())
    }

    /// Start a JSON merge after its decode-time canonical probes were dropped.
    fn begin_json_merge(&mut self) {
        self.retained_canonical_bytes = 0;
        self.merge_allocated_bytes = 0;
    }

    /// Start a typed merge whose canonical frames remain owned by payloads.
    fn begin_typed_merge(&mut self) {
        self.merge_allocated_bytes = 0;
    }

    fn admit_merge_allocation(&mut self, bytes: usize) -> Result<(), Response> {
        let total = self
            .merge_allocated_bytes
            .checked_add(bytes)
            .ok_or_else(torii_routed_read_accounting_response)?;
        torii_routed_read_ensure(
            "merge allocations",
            total,
            self.envelope.candidate_allocation_bytes,
        )?;
        self.merge_allocated_bytes = total;
        Ok(())
    }

    /// Pre-admit standard-library B-tree nodes used by a merge index.
    fn admit_merge_btree<K, V>(&mut self, maps: usize, entries: usize) -> Result<(), Response> {
        let bytes = norito::core::owned_btree_maps_allocation_bytes::<K, V>(maps, entries)
            .map_err(|_| torii_routed_read_accounting_response())?;
        self.admit_merge_allocation(bytes)
    }

    /// Allocate a merge vector only after its complete Rust-layout request is
    /// admitted by the existing candidate-allocation phase.
    fn try_merge_vec<T>(&mut self, capacity: usize) -> Result<Vec<T>, Response> {
        let bytes = capacity
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(torii_routed_read_accounting_response)?;
        self.admit_merge_allocation(bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| torii_routed_read_allocation_response("merge vector", bytes))?;
        let allocated_bytes = values
            .capacity()
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(torii_routed_read_accounting_response)?;
        if let Some(extra_bytes) = allocated_bytes.checked_sub(bytes)
            && extra_bytes != 0
        {
            self.admit_merge_allocation(extra_bytes)?;
        }
        Ok(values)
    }

    /// Encode one transient canonical key inside the space left beside keys
    /// that the merge has actually retained. Callers commit its byte length
    /// only if they keep the returned allocation.
    fn canonical_json_candidate(&self, value: &Value) -> Result<Vec<u8>, Response> {
        let canonical = norito::json::to_json_bounded(value, self.canonical_remaining()?)
            .map_err(|_| torii_routed_read_json_encode_response())?;
        Ok(canonical.into_bytes())
    }

    fn json_response<T: norito::json::JsonSerialize + ?Sized>(
        self,
        value: &T,
    ) -> Result<Response, Response> {
        let body = norito::json::to_json_bounded(value, self.final_body_limit())
            .map_err(|_| torii_routed_read_json_encode_response())?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(Body::from(body))
            .expect("build preflighted routed-read JSON response"))
    }

    /// Verify that native `Value` parser charges cover every owned graph node.
    fn verify_json_value_usage(
        self,
        profile: norito::json::JsonPreflightProfile,
        usage: norito::core::DecodeAllocationUsage,
    ) -> Result<(), Response> {
        let graph_bytes = torii_routed_read_json_value_graph_bytes(profile)?;
        torii_routed_read_ensure(
            "decoded JSON graph",
            graph_bytes,
            self.envelope.decode_allocated_bytes,
        )?;
        if usage.total_allocated_bytes() < graph_bytes {
            return Err(torii_proxy_error_response(
                StatusCode::BAD_GATEWAY,
                "route_unavailable",
                "proxied JSON decoder did not account for its complete owned value graph",
            ));
        }
        Ok(())
    }
}

/// Heap bytes reachable from a native JSON `Value` after parsing.
///
/// Strings and arrays use the parser's exact-reserve requests. The lexical
/// profile sums Norito core's checked node-count bound separately for every
/// object, so empty and differently sized objects cannot inflate one another's
/// topology charge. The parser separately charges any allocator capacity
/// returned above an exact-reserve request.
fn torii_routed_read_json_value_graph_bytes(
    profile: norito::json::JsonPreflightProfile,
) -> Result<usize, Response> {
    let array_bytes = profile
        .array_entries()
        .checked_mul(core::mem::size_of::<Value>())
        .ok_or_else(torii_routed_read_accounting_response)?;
    let object_nodes = profile.object_btree_node_upper_bound();
    let object_bytes = norito::core::owned_btree_maps_allocation_bytes::<String, Value>(
        object_nodes,
        object_nodes,
    )
    .map_err(|_| torii_routed_read_accounting_response())?;
    profile
        .string_capacity_bytes()
        .checked_add(array_bytes)
        .and_then(|bytes| bytes.checked_add(object_bytes))
        .ok_or_else(torii_routed_read_accounting_response)
}

fn torii_routed_read_request_bytes(
    path_args: &[String],
    query_string: Option<&str>,
    body: &[u8],
) -> Result<usize, Response> {
    path_args
        .iter()
        .try_fold(body.len(), |bytes, arg| bytes.checked_add(arg.len()))
        .and_then(|bytes| bytes.checked_add(query_string.map_or(0, str::len)))
        .ok_or_else(torii_routed_read_accounting_response)
}

fn torii_routed_read_ensure(
    phase: &'static str,
    attempted: usize,
    limit: usize,
) -> Result<(), Response> {
    if attempted > limit {
        return Err(torii_routed_read_capacity_response(phase, attempted, limit));
    }
    Ok(())
}

fn torii_routed_read_capacity_response(
    phase: &'static str,
    attempted: usize,
    limit: usize,
) -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        format!(
            "proxied application response exceeded its admitted {phase} bound (attempted {attempted}, limit {limit})"
        ),
    )
}

fn torii_routed_read_accounting_response() -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied application response memory accounting overflowed",
    )
}

fn torii_routed_read_allocation_response(phase: &'static str, bytes: usize) -> Response {
    torii_proxy_error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "route_unavailable",
        format!("failed to reserve the admitted {bytes}-byte {phase}"),
    )
}

fn torii_routed_read_json_preflight_response(error: norito::json::JsonPreflightError) -> Response {
    if error.resource_kind().is_some() {
        return torii_routed_read_capacity_response(
            "JSON lexical resource",
            error.attempted(),
            error.limit(),
        );
    }
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied JSON response failed bounded lexical validation",
    )
}

/// Consume a hostile parser error without reflecting payload-derived text.
fn torii_routed_read_json_decode_response(_: norito::json::Error) -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied JSON response failed bounded decoding",
    )
}

/// Consume a hostile binary decoder error without reflecting payload bytes.
fn torii_routed_read_norito_decode_response(_: norito::Error) -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied Norito response failed bounded decoding",
    )
}

fn torii_routed_read_body_response() -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied application response body exceeded its admitted bound",
    )
}

fn torii_routed_read_norito_encode_response() -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied Norito response exceeded its bounded canonical representation",
    )
}

fn torii_routed_read_json_encode_response() -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_GATEWAY,
        "route_unavailable",
        "proxied JSON response exceeded its bounded canonical representation",
    )
}

#[cfg(test)]
include!("tests/lib_routed_reads/routed_read_memory_bounds.rs");
