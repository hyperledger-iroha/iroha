#[derive(Debug)]
struct ToriiFanoutDecodeBudget {
    retained_bytes: usize,
    max_retained_bytes: usize,
}

impl ToriiFanoutDecodeBudget {
    fn new(max_retained_bytes: usize) -> Self {
        Self {
            retained_bytes: 0,
            max_retained_bytes: max_retained_bytes.max(1),
        }
    }

    fn remaining(&self) -> Result<usize, Response> {
        let remaining = self
            .max_retained_bytes
            .checked_sub(self.retained_bytes)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::BAD_GATEWAY,
                    "route_unavailable",
                    "cross-dataspace response decode accounting is inconsistent",
                )
            })?;
        if remaining == 0 {
            return Err(torii_proxy_error_response(
                StatusCode::BAD_GATEWAY,
                "route_unavailable",
                format!(
                    "cross-dataspace response exceeds the configured {}-byte aggregate decode budget",
                    self.max_retained_bytes
                ),
            ));
        }
        Ok(remaining)
    }

    fn charge(&mut self, bytes: usize) -> Result<(), Response> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_retained_bytes)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::BAD_GATEWAY,
                    "route_unavailable",
                    format!(
                        "cross-dataspace response exceeds the configured {}-byte aggregate decode budget",
                        self.max_retained_bytes
                    ),
                )
            })?;
        Ok(())
    }
}

/// Fixed logical allowance for coordinator metadata and response framing.
const QUERY_FANOUT_BASE_OVERHEAD_BYTES: usize = 64 * 1024;
/// Conservative logical charge for one route while the deduplicating
/// `BTreeMap` and final route `Vec` overlap.
const QUERY_FANOUT_ROUTE_OVERHEAD_BYTES: usize = 1024;
/// Core's protocol-derived bound for one allocation-free identity source row.
const QUERY_FANOUT_SOURCE_OVERHEAD_BYTES: usize =
    iroha_core::smartcontracts::isi::query::CANONICAL_QUERY_PREBOUNDED_SOURCE_BYTES as usize;
/// Fixed scratch used while validating the largest protocol-admitted public
/// key. Exact public-key sizing is allocation-free, but validation still owns
/// this bounded temporary beside the request representation.
const QUERY_FANOUT_PUBLIC_KEY_VALIDATION_SCRATCH_BYTES: usize =
    iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES;
/// Conservative number of simultaneously represented validator identities
/// while Core's bounded manifest result is transformed into a retry candidate.
const QUERY_FANOUT_CANDIDATE_IDENTITY_REPRESENTATIONS: usize = 3;
/// Per-candidate allowance for `Vec`/tree nodes, enum/string handles, allocator
/// bookkeeping, and retry metadata beyond the bounded identity and URL bytes.
const QUERY_FANOUT_CANDIDATE_CONTAINER_OVERHEAD_BYTES: usize = 4 * 1024;
/// Query-sized representations simultaneously live while an authoritative
/// HTTP bridge serializes one attempt: coordinator source, retry template,
/// attempt value, payload destination, and two nested derived-codec spills.
const QUERY_FANOUT_HTTP_SIGNED_REQUEST_REPRESENTATIONS: usize = 6;
/// P2P adds the outer `NetworkMessage` derived-codec spill before the exact
/// actor-wire byte lease takes ownership.
const QUERY_FANOUT_P2P_SIGNED_REQUEST_REPRESENTATIONS: usize = 7;
/// Worst protocol transport representation count used by admission.
const QUERY_FANOUT_SIGNED_REQUEST_REPRESENTATIONS: usize =
    QUERY_FANOUT_P2P_SIGNED_REQUEST_REPRESENTATIONS;
/// One independently measured canonical verified-request representation.
const QUERY_FANOUT_VERIFIED_REQUEST_REPRESENTATIONS: usize = 1;
/// Equal high-water phases in the coordinator working set.
///
/// The sixth unit covers the singular producer corridor while a previously
/// successful route remains retained: request decode, retained first output,
/// an owned persisted/source projection, the current decoded output, and two
/// canonical encoding units can coexist. Routes still execute sequentially.
const QUERY_FANOUT_PHASE_COUNT: usize = 6;
/// Conservative pre-body units: 7Q + E + 6P.
const QUERY_FANOUT_PREBODY_UNITS: usize = QUERY_FANOUT_SIGNED_REQUEST_REPRESENTATIONS
    + QUERY_FANOUT_VERIFIED_REQUEST_REPRESENTATIONS
    + QUERY_FANOUT_PHASE_COUNT;

fn checked_sum<const N: usize>(terms: [usize; N]) -> Option<usize> {
    terms.into_iter().try_fold(0_usize, usize::checked_add)
}

/// Fixed memory that can be reached while an ingress reservation owns route
/// classification and the protocol-bounded route catalogue.
fn query_ingress_fixed_overhead_bytes() -> Option<usize> {
    let route_catalogue = QUERY_FANOUT_ROUTE_OVERHEAD_BYTES
        .checked_mul(iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES)?;
    checked_sum([
        QUERY_FANOUT_BASE_OVERHEAD_BYTES,
        route_catalogue,
        QUERY_FANOUT_PUBLIC_KEY_VALIDATION_SCRATCH_BYTES,
    ])
}

/// Maximum candidate-list high water for one sequentially resolved route.
///
/// Core admits at most 256 manifest validators. Until the routing-only Core
/// seam can return a single representation, account for three simultaneous
/// identity/key payloads, one bounded manifest URL, and explicit container
/// slack for every selected status. This bound is intentionally independent
/// of the total online/world peer population and is not multiplied by lanes:
/// candidate resolution is sequential.
fn query_fanout_candidate_snapshot_overhead_bytes() -> Option<usize> {
    let identities = iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES
        .checked_mul(QUERY_FANOUT_CANDIDATE_IDENTITY_REPRESENTATIONS)?;
    let per_candidate = checked_sum([
        identities,
        iroha_core::governance::manifest::MANIFEST_SOURCE_MAX_VALUE_BYTES_V1,
        QUERY_FANOUT_CANDIDATE_CONTAINER_OVERHEAD_BYTES,
    ])?;
    per_candidate.checked_mul(iroha_core::governance::manifest::LANE_MANIFEST_MAX_VALIDATORS_V1)
}

/// Complete fixed envelope for one admitted fanout.
fn query_fanout_fixed_overhead_bytes() -> Option<usize> {
    checked_sum([
        query_ingress_fixed_overhead_bytes()?,
        QUERY_FANOUT_SOURCE_OVERHEAD_BYTES,
        query_fanout_candidate_snapshot_overhead_bytes()?,
    ])
}

/// Deterministic phase geometry for one signed-query fanout.
///
/// This is logical admission accounting, not an estimate of allocator internals.
/// The request and fixed allowance remain live for the operation. The remaining
/// bytes are divided into six equal parts so that every high-water phase fits
/// in one working-set reservation:
///
/// - request decode + outer accumulator + a local Core accumulator + two
///   candidate-sized encoder transients (direct frame and nested serialization
///   scratch); the 1 KiB protocol-bounded source row is charged in the fixed
///   allowance above;
/// - request decode + outer accumulator + the accumulated raw route body +
///   one concurrently held transport chunk + decoded route output;
/// - request decode + outer accumulator + decoded route output + direct
///   candidate frame + nested serialization scratch;
/// - final decoded output + the exact direct-streamed iterable body + one
///   protocol-bounded identifier item's nested serialization scratch; and
/// - response body + proxy snapshot copy; and
/// - retained first singular output + the current source/output corridor +
///   two canonical scratch units. This is a sequential comparison phase, not
///   a route-count multiplier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueryFanoutMemoryEnvelope {
    working_set_bytes: usize,
    request_bytes: usize,
    request_decode_allocated_bytes: usize,
    accumulator_retained_bytes: usize,
    route_body_bytes: usize,
    decode_allocated_bytes: usize,
    candidate_allocation_bytes: usize,
    candidate_encoded_bytes: usize,
    final_body_bytes: usize,
}

impl QueryFanoutMemoryEnvelope {
    fn for_request_lengths(
        working_set_bytes: usize,
        signed_query_bytes: usize,
        verified_request_bytes: usize,
    ) -> Result<Self, Response> {
        let admission_unit = Self::body_admission_unit(working_set_bytes)?;
        if signed_query_bytes > admission_unit || verified_request_bytes > admission_unit {
            return Err(torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                format!(
                    "signed-query fanout request frames ({signed_query_bytes} signed, {verified_request_bytes} verified) exceed the admitted {admission_unit}-byte per-frame limit"
                ),
            ));
        }
        // Routes execute sequentially, so the transport wrapper depth does not
        // multiply by route or candidate count. The current retry template and
        // derived codecs nevertheless overlap seven query-sized
        // representations before the P2P actor lease owns the wire bytes (six
        // for HTTP, plus the outer NetworkMessage spill for P2P). Charge that
        // proven worst case together with the independently measured verified
        // request instead of assuming either representation is shorter.
        let request_bytes = Self::retained_request_bytes(
            signed_query_bytes,
            verified_request_bytes,
        )
        .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    "signed-query fanout request retention accounting overflows the platform address space",
                )
            })?;
        Self::new(working_set_bytes, request_bytes)
    }

    fn retained_request_bytes(
        signed_query_bytes: usize,
        verified_request_bytes: usize,
    ) -> Option<usize> {
        signed_query_bytes
            .checked_mul(QUERY_FANOUT_SIGNED_REQUEST_REPRESENTATIONS)?
            .checked_add(
                verified_request_bytes
                    .checked_mul(QUERY_FANOUT_VERIFIED_REQUEST_REPRESENTATIONS)?,
            )
    }

    fn for_body_admission(working_set_bytes: usize) -> Result<Self, Response> {
        let phase_bytes = Self::body_admission_unit(working_set_bytes)?;
        Self::with_phase_bytes(working_set_bytes, 0, phase_bytes)
    }

    fn body_admission_unit(working_set_bytes: usize) -> Result<usize, Response> {
        // Before decoding, the exact verified-request frame is unknown. Reserve
        // seven signed-query-sized transport representations, one verified
        // frame, and six high-water phases up front. Once Q and E are
        // measured, the exact envelope can only make a phase larger, never
        // smaller.
        let fixed_overhead = query_fanout_fixed_overhead_bytes().ok_or_else(|| {
            torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "signed-query fanout fixed-overhead accounting overflows the platform address space",
            )
        })?;
        working_set_bytes
            .checked_sub(fixed_overhead)
            .map(|remaining| remaining / QUERY_FANOUT_PREBODY_UNITS)
            .filter(|unit| *unit > 1)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    format!(
                        "signed-query fanout requires more than {fixed_overhead} bytes of working-set capacity"
                    ),
                )
            })
    }

    fn new(working_set_bytes: usize, request_bytes: usize) -> Result<Self, Response> {
        let fixed_overhead = query_fanout_fixed_overhead_bytes().ok_or_else(|| {
            torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "signed-query fanout fixed-overhead accounting overflows the platform address space",
            )
        })?;
        let phase_bytes = working_set_bytes
            .checked_sub(request_bytes)
            .and_then(|remaining| remaining.checked_sub(fixed_overhead))
            .map(|remaining| remaining / QUERY_FANOUT_PHASE_COUNT)
            .filter(|phase| *phase > 0)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    format!(
                        "signed-query fanout request requires {request_bytes} bytes inside a {working_set_bytes}-byte working-set reservation"
                    ),
                )
            })?;
        Self::with_phase_bytes(working_set_bytes, request_bytes, phase_bytes)
    }

    fn with_phase_bytes(
        working_set_bytes: usize,
        request_bytes: usize,
        phase_bytes: usize,
    ) -> Result<Self, Response> {
        let retained_item_overhead = usize::try_from(
            iroha_core::smartcontracts::isi::query::CANONICAL_QUERY_RETAINED_ITEM_OVERHEAD_BYTES,
        )
        .map_err(|_| {
            torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "canonical candidate overhead does not fit the platform address space",
            )
        })?;
        let candidate_encoded_bytes = phase_bytes
            .checked_sub(retained_item_overhead)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    "signed-query fanout phase cannot admit one canonical candidate",
                )
            })?;
        let candidate_allocation_bytes = u64::try_from(candidate_encoded_bytes)
            .ok()
            .and_then(
                iroha_core::smartcontracts::isi::query::canonical_query_candidate_allocation_bytes,
            )
            .and_then(|bytes| usize::try_from(bytes).ok())
            .filter(|bytes| *bytes <= phase_bytes)
            .ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    "canonical candidate allocation exceeds its fanout phase",
                )
            })?;
        let final_body_bytes = phase_bytes.checked_mul(2).ok_or_else(|| {
            torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "signed-query fanout response budget overflows the platform address space",
            )
        })?;
        let envelope = Self {
            working_set_bytes,
            request_bytes,
            request_decode_allocated_bytes: phase_bytes,
            accumulator_retained_bytes: phase_bytes,
            route_body_bytes: phase_bytes,
            decode_allocated_bytes: phase_bytes,
            candidate_allocation_bytes,
            candidate_encoded_bytes,
            final_body_bytes,
        };
        if envelope.phases_fit() {
            Ok(envelope)
        } else {
            Err(torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "signed-query fanout phase accounting exceeds its working-set reservation",
            ))
        }
    }

    fn phases_fit(self) -> bool {
        let Some(fixed_overhead) = query_fanout_fixed_overhead_bytes() else {
            return false;
        };
        let Some(fixed) = self.request_bytes.checked_add(fixed_overhead) else {
            return false;
        };
        let Some(local_scan) = checked_sum([
            self.request_decode_allocated_bytes,
            self.accumulator_retained_bytes,
            self.accumulator_retained_bytes,
            self.candidate_allocation_bytes,
            self.candidate_encoded_bytes,
        ]) else {
            return false;
        };
        let Some(remote_decode) = checked_sum([
            self.request_decode_allocated_bytes,
            self.accumulator_retained_bytes,
            self.route_body_bytes,
            self.route_body_bytes,
            self.decode_allocated_bytes,
        ]) else {
            return false;
        };
        let Some(merge_candidate) = checked_sum([
            self.request_decode_allocated_bytes,
            self.accumulator_retained_bytes,
            self.decode_allocated_bytes,
            self.candidate_allocation_bytes,
            self.candidate_encoded_bytes,
        ]) else {
            return false;
        };
        let Some(final_encode) = checked_sum([
            self.decode_allocated_bytes,
            self.final_body_bytes,
            self.candidate_encoded_bytes,
        ]) else {
            return false;
        };
        let Some(proxy_copy) = self.final_body_bytes.checked_mul(2) else {
            return false;
        };
        let Some(singular_compare) = checked_sum([
            self.request_decode_allocated_bytes,
            self.decode_allocated_bytes,
            self.decode_allocated_bytes,
            self.decode_allocated_bytes,
            self.candidate_allocation_bytes,
            self.candidate_encoded_bytes,
        ]) else {
            return false;
        };
        let peak = local_scan
            .max(remote_decode)
            .max(merge_candidate)
            .max(final_encode)
            .max(proxy_copy)
            .max(singular_compare);
        fixed
            .checked_add(peak)
            .is_some_and(|total| total <= self.working_set_bytes)
    }

    fn execution_budget(self) -> iroha_core::smartcontracts::isi::query::QueryExecutionBudget {
        let max_units = self
            .working_set_bytes
            .checked_sub(self.request_bytes)
            .and_then(|remaining| {
                remaining.checked_sub(
                    query_fanout_fixed_overhead_bytes()
                        .expect("validated fanout fixed overhead fits usize"),
                )
            })
            .expect("validated fanout envelope has a non-negative execution budget");
        iroha_core::smartcontracts::isi::query::QueryExecutionBudget::from_weighted_limit(
            u64::try_from(max_units).unwrap_or(u64::MAX),
            1,
            1,
        )
    }

    fn canonical_output_limits(
        self,
        max_items: u64,
    ) -> iroha_core::smartcontracts::isi::query::CanonicalQueryOutputLimits {
        iroha_core::smartcontracts::isi::query::CanonicalQueryOutputLimits::new(
            max_items,
            iroha_core::smartcontracts::isi::query::CANONICAL_QUERY_PREBOUNDED_SOURCE_BYTES,
            u64::try_from(self.candidate_encoded_bytes).unwrap_or(u64::MAX),
            u64::try_from(self.accumulator_retained_bytes).unwrap_or(u64::MAX),
            u64::try_from(self.decode_allocated_bytes).unwrap_or(u64::MAX),
        )
    }

    fn singular_output_limits(
        self,
    ) -> iroha_core::smartcontracts::isi::query::SingularQueryOutputLimits {
        iroha_core::smartcontracts::isi::query::SingularQueryOutputLimits::new(
            u64::try_from(self.candidate_encoded_bytes).unwrap_or(u64::MAX),
            u64::try_from(self.decode_allocated_bytes).unwrap_or(u64::MAX),
        )
    }

    fn request_decode_limits(self, encoded_len: usize) -> Result<norito::DecodeLimits, Response> {
        // The coordinator retains the verified request while one local route
        // owns a decoded clone. Each representation receives half of the one
        // request-decode phase, so their overlap cannot silently double D.
        Self::decode_limits_for(encoded_len, self.request_decode_allocated_bytes / 2)
    }

    fn query_scope_limits(self) -> QueryScopeMemoryLimits {
        // The verified request keeps half of D. Scope classification may own a
        // nested decoded query and one extracted routing identifier at once,
        // so each receives one quarter of D and the combined request phase
        // remains bounded.
        QueryScopeMemoryLimits {
            decode_allocated_bytes: self.request_decode_allocated_bytes / 4,
            canonical_encoded_bytes: self.candidate_encoded_bytes,
        }
    }

    fn response_decode_limits(self, encoded_len: usize) -> Result<norito::DecodeLimits, Response> {
        Self::decode_limits_for(encoded_len, self.decode_allocated_bytes)
    }

    fn decode_limits_for(
        encoded_len: usize,
        max_allocated_bytes: usize,
    ) -> Result<norito::DecodeLimits, Response> {
        let element_limit = encoded_len.checked_mul(8).ok_or_else(|| {
            torii_proxy_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "query_capacity_exceeded",
                "signed-query decode element limit overflows the platform address space",
            )
        })?;
        Ok(norito::DecodeLimits::new(
            element_limit,
            encoded_len,
            element_limit,
            max_allocated_bytes,
            norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
        ))
    }
}

/// Allocation and canonical-encoding ceilings for routing-scope inspection.
///
/// These limits always come from the reservation currently held by the
/// caller. In particular, HTTP ingress cannot borrow the larger fanout limits
/// before it has successfully promoted to a fanout reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueryScopeMemoryLimits {
    decode_allocated_bytes: usize,
    canonical_encoded_bytes: usize,
}

impl QueryScopeMemoryLimits {
    fn decode_limits(self, encoded_len: usize) -> Result<norito::DecodeLimits, Response> {
        QueryFanoutMemoryEnvelope::decode_limits_for(encoded_len, self.decode_allocated_bytes)
    }
}

/// Fraction of the configured query memory pool reserved for bounded HTTP
/// ingress. The remaining three quarters back complete fanout working sets.
const QUERY_INGRESS_POOL_DIVISOR: usize = 4;
/// Independent signed-query bodies that may be read concurrently.
const QUERY_INGRESS_SLOT_COUNT: usize = 4;
/// Maximum simultaneous variable-size ingress representations during bounded
/// routing-scope classification: decoded signed query, its canonical frame, a
/// nested decoded scope query, that query's canonical-check frame, and one
/// derived-codec scratch representation.
const QUERY_INGRESS_PHASE_UNITS: usize = 5;

/// Complete memory reservation for one signed-query body before its route is
/// known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueryIngressMemoryEnvelope {
    slot_bytes: usize,
    body_bytes: usize,
    decode_allocated_bytes: usize,
    canonical_encoded_bytes: usize,
    scope_decode_allocated_bytes: usize,
    scope_canonical_encoded_bytes: usize,
}

/// Complete memory admitted while one authenticated peer delivers an internal
/// Torii proxy request over HTTP.
///
/// A dedicated single-slot bridge lane holds this complete reservation before
/// polling the body. The high-water phases cover the signed raw frame plus the
/// decoded request, and then the moved decoded envelope, one shared outbound
/// frame, and two derived-codec scratch representations. Candidate attempts
/// share those representations instead of cloning the request per candidate.
/// One fixed 64 KiB retryable diagnostic may remain while the next sequential
/// authority is attempted; larger retry bodies are dropped before proceeding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ToriiProxyHttpIngressEnvelope {
    working_set_bytes: usize,
    body_bytes: usize,
    decode_allocated_bytes: usize,
    forwarded_request_bytes: usize,
    forwarding_transient_bytes: usize,
}

impl ToriiProxyHttpIngressEnvelope {
    fn from_max_content_bytes(max_content_bytes: usize) -> Option<Self> {
        let fixed_overhead = query_fanout_fixed_overhead_bytes()?
            .checked_add(TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1)?;
        if max_content_bytes == 0 {
            return None;
        }
        // The middleware owns the raw body only through decoding. After that,
        // the request is moved (never deep-cloned) through dispatch and one
        // bounded shared frame is reused by every transport attempt. Two
        // derived-codec scratch representations can overlap that frame.
        let variable_bytes = max_content_bytes.checked_mul(4)?;
        let working_set_bytes = fixed_overhead.checked_add(variable_bytes)?;
        let envelope = Self {
            working_set_bytes,
            body_bytes: max_content_bytes,
            decode_allocated_bytes: max_content_bytes,
            forwarded_request_bytes: max_content_bytes,
            forwarding_transient_bytes: max_content_bytes,
        };
        envelope.phases_fit().then_some(envelope)
    }

    fn decode_limits(self) -> Result<norito::DecodeLimits, Response> {
        QueryFanoutMemoryEnvelope::decode_limits_for(self.body_bytes, self.decode_allocated_bytes)
    }

    fn phases_fit(self) -> bool {
        let decode_peak = self.body_bytes.checked_add(self.decode_allocated_bytes);
        let forwarding_peak = checked_sum([
            self.forwarded_request_bytes,
            self.forwarding_transient_bytes,
            self.forwarding_transient_bytes,
            self.forwarding_transient_bytes,
        ]);
        decode_peak
            .into_iter()
            .chain(forwarding_peak)
            .max()
            .and_then(|peak| {
                peak.checked_add(
                    query_fanout_fixed_overhead_bytes()?
                        .checked_add(TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1)?,
                )
            })
            .is_some_and(|total| total <= self.working_set_bytes)
    }
}

impl QueryIngressMemoryEnvelope {
    fn from_slot_bytes(slot_bytes: usize, max_unit_bytes: usize) -> Option<Self> {
        let unit = (slot_bytes.checked_sub(query_ingress_fixed_overhead_bytes()?)?
            / QUERY_INGRESS_PHASE_UNITS)
            .min(max_unit_bytes);
        if unit == 0 {
            return None;
        }
        let envelope = Self {
            slot_bytes,
            body_bytes: unit,
            decode_allocated_bytes: unit,
            canonical_encoded_bytes: unit,
            scope_decode_allocated_bytes: unit,
            scope_canonical_encoded_bytes: unit,
        };
        envelope.phases_fit().then_some(envelope)
    }

    fn request_decode_limits(
        self,
        provisional_fanout: QueryFanoutMemoryEnvelope,
    ) -> Result<norito::DecodeLimits, Response> {
        // Promotion retains this decoded request while a local route may own
        // one bounded clone. Each side receives at most half of fanout's D
        // phase even when the independent ingress slot could admit more.
        QueryFanoutMemoryEnvelope::decode_limits_for(
            self.body_bytes,
            self.decode_allocated_bytes
                .min(provisional_fanout.request_decode_allocated_bytes / 2),
        )
    }

    /// Decode limits for JSON ingress while two allocation units can coexist.
    ///
    /// Unlike the direct Norito path above, a JSON wrapper can still own its
    /// decoded base64 string buffer while the nested Norito payload is being
    /// reconstructed. The JSON path therefore receives at most two ingress
    /// allocation units for that transient. The independent fanout half-phase
    /// remains an upper bound so ingress cannot borrow memory it will not own
    /// after promotion.
    fn json_request_two_unit_decode_limits(
        self,
        provisional_fanout: QueryFanoutMemoryEnvelope,
    ) -> Result<norito::DecodeLimits, Response> {
        let json_decode_allocated_bytes =
            self.decode_allocated_bytes.checked_mul(2).ok_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "query_capacity_exceeded",
                    "signed-query JSON decode budget overflows the platform address space",
                )
            })?;
        QueryFanoutMemoryEnvelope::decode_limits_for(
            self.body_bytes,
            json_decode_allocated_bytes.min(provisional_fanout.request_decode_allocated_bytes / 2),
        )
    }

    fn query_scope_limits(self) -> QueryScopeMemoryLimits {
        QueryScopeMemoryLimits {
            decode_allocated_bytes: self.scope_decode_allocated_bytes,
            canonical_encoded_bytes: self.scope_canonical_encoded_bytes,
        }
    }

    fn phases_fit(self) -> bool {
        let Some(fixed_overhead) = query_ingress_fixed_overhead_bytes() else {
            return false;
        };
        let Some(body_decode) = checked_sum([self.body_bytes, self.decode_allocated_bytes]) else {
            return false;
        };
        let Some(canonicalize) = checked_sum([
            self.decode_allocated_bytes,
            self.canonical_encoded_bytes,
            self.canonical_encoded_bytes,
        ]) else {
            return false;
        };
        let Some(scope_classification) = checked_sum([
            self.decode_allocated_bytes,
            self.canonical_encoded_bytes,
            self.scope_decode_allocated_bytes,
            self.scope_canonical_encoded_bytes,
            self.scope_canonical_encoded_bytes,
        ]) else {
            return false;
        };
        fixed_overhead
            .checked_add(body_decode.max(canonicalize).max(scope_classification))
            .is_some_and(|bytes| bytes <= self.slot_bytes)
    }
}

#[cfg(test)]
mod query_ingress_json_decode_limit_tests {
    use super::*;

    #[test]
    fn json_base64_and_nested_decode_peak_fits_the_five_unit_ingress_slot() {
        let fixed = query_ingress_fixed_overhead_bytes().expect("fixed ingress overhead fits");
        let unit = 17;
        let slot = fixed + QUERY_INGRESS_PHASE_UNITS * unit;
        let ingress = QueryIngressMemoryEnvelope::from_slot_bytes(slot, usize::MAX)
            .expect("exact five-unit ingress slot fits");
        let provisional = QueryFanoutMemoryEnvelope::for_body_admission(64_000_000)
            .expect("test fanout envelope fits");
        let limits = ingress
            .json_request_two_unit_decode_limits(provisional)
            .expect("two-unit JSON decode limits fit");

        assert_eq!(limits.max_total_allocated_bytes(), 2 * unit);
        let json_decode_peak = fixed
            .checked_add(ingress.body_bytes)
            .and_then(|bytes| bytes.checked_add(limits.max_total_allocated_bytes()))
            .expect("test peak fits usize");
        assert!(json_decode_peak <= ingress.slot_bytes);
        assert!(ingress.phases_fit());
    }
}

/// Split of the configured aggregate query-memory pool into an independent
/// pre-body lane and complete fanout working-set reservations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueryMemoryGeometry {
    ingress: QueryIngressMemoryEnvelope,
    ingress_slots: NonZeroUsize,
    fanout_working_set_bytes: usize,
    fanout_slots: NonZeroUsize,
}

fn query_memory_geometry(
    aggregate_bytes: usize,
    max_content_bytes: usize,
    max_concurrent_fanouts: usize,
) -> Option<QueryMemoryGeometry> {
    let ingress_pool_bytes = aggregate_bytes / QUERY_INGRESS_POOL_DIVISOR;
    let fanout_pool_bytes = aggregate_bytes.checked_sub(ingress_pool_bytes)?;
    // One working set must include the fixed routing/candidate catalogue and
    // every simultaneously live representation of the admitted body. A small
    // listener limit therefore reduces the slot weight (and can increase safe
    // concurrency); it must not make the slot smaller than its fixed state.
    // When the configured body is larger than the aggregate pool can cover,
    // the phase-derived query-body limit shrinks while the general Torii body
    // limit remains unchanged.
    let desired_fanout_working_set = max_content_bytes
        .checked_mul(QUERY_FANOUT_PREBODY_UNITS)
        .and_then(|bytes| bytes.checked_add(query_fanout_fixed_overhead_bytes()?));
    let fanout_working_set_bytes = desired_fanout_working_set
        .map_or(fanout_pool_bytes, |desired| desired.min(fanout_pool_bytes));
    let provisional_fanout =
        QueryFanoutMemoryEnvelope::for_body_admission(fanout_working_set_bytes).ok()?;
    let ingress_slots = NonZeroUsize::new(QUERY_INGRESS_SLOT_COUNT)?;
    let ingress_slot_bytes = ingress_pool_bytes / ingress_slots.get();
    let ingress_unit_cap = max_content_bytes.min(provisional_fanout.route_body_bytes);
    let ingress =
        QueryIngressMemoryEnvelope::from_slot_bytes(ingress_slot_bytes, ingress_unit_cap)?;

    let fanout_slots = query_fanout_slot_count(
        fanout_pool_bytes,
        fanout_working_set_bytes,
        max_concurrent_fanouts,
    )?;

    let ingress_reserved = ingress_slot_bytes.checked_mul(ingress_slots.get())?;
    let fanout_reserved = fanout_working_set_bytes.checked_mul(fanout_slots.get())?;
    ingress_reserved
        .checked_add(fanout_reserved)
        .filter(|reserved| *reserved <= aggregate_bytes)?;

    Some(QueryMemoryGeometry {
        ingress,
        ingress_slots,
        fanout_working_set_bytes,
        fanout_slots,
    })
}

fn query_fanout_slot_count(
    max_retained_bytes: usize,
    working_set_bytes: usize,
    max_concurrent_queries: usize,
) -> Option<NonZeroUsize> {
    let working_set_bytes = NonZeroUsize::new(working_set_bytes)?;
    if max_retained_bytes < working_set_bytes.get() {
        return None;
    }
    NonZeroUsize::new(
        (max_retained_bytes / working_set_bytes.get()).min(max_concurrent_queries.max(1)),
    )
}

async fn acquire_query_ingress_memory(
    app: &SharedAppState,
) -> Result<tokio::sync::OwnedSemaphorePermit, Response> {
    let capacity_response = || {
        torii_proxy_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "query_capacity_exceeded",
            "signed-query ingress memory capacity is exhausted",
        )
    };
    if app.query_queue_timeout.is_zero() {
        return app
            .query_ingress_inflight
            .clone()
            .try_acquire_owned()
            .map_err(|_| capacity_response());
    }
    tokio::time::timeout(
        app.query_queue_timeout,
        app.query_ingress_inflight.clone().acquire_owned(),
    )
    .await
    .map_err(|_| capacity_response())?
    .map_err(|_| capacity_response())
}

#[derive(Clone)]
struct ToriiProxyMemoryReservation(Arc<tokio::sync::OwnedSemaphorePermit>);

impl ToriiProxyMemoryReservation {
    fn new(permit: tokio::sync::OwnedSemaphorePermit) -> Self {
        Self(Arc::new(permit))
    }
}

async fn acquire_torii_proxy_memory(
    app: &SharedAppState,
) -> Result<ToriiProxyMemoryReservation, Response> {
    let capacity_response = || {
        torii_proxy_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "proxy_capacity_exceeded",
            "Torii proxy memory capacity is exhausted",
        )
    };
    if app.query_queue_timeout.is_zero() {
        return app
            .torii_proxy_memory_inflight
            .clone()
            .try_acquire_owned()
            .map(ToriiProxyMemoryReservation::new)
            .map_err(|_| capacity_response());
    }
    tokio::time::timeout(
        app.query_queue_timeout,
        app.torii_proxy_memory_inflight.clone().acquire_owned(),
    )
    .await
    .map_err(|_| capacity_response())?
    .map(ToriiProxyMemoryReservation::new)
    .map_err(|_| capacity_response())
}

fn try_acquire_torii_proxy_memory(
    app: &SharedAppState,
) -> Result<ToriiProxyMemoryReservation, Response> {
    app.torii_proxy_memory_inflight
        .clone()
        .try_acquire_owned()
        .map(ToriiProxyMemoryReservation::new)
        .map_err(|_| {
            torii_proxy_error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "proxy_capacity_exceeded",
                "Torii proxy memory capacity is exhausted",
            )
        })
}

fn hold_torii_proxy_memory_in_response_body(
    response: Response,
    reservation: ToriiProxyMemoryReservation,
) -> Response {
    use http_body_util::BodyExt as _;

    let (parts, body) = response.into_parts();
    let guarded_body = body.map_frame(move |frame| {
        let _reservation = &reservation;
        frame
    });
    Response::from_parts(parts, Body::new(guarded_body))
}

/// Promote an already-admitted body to a complete fanout working set without
/// waiting while it occupies the independent ingress lane.
///
/// Failing fast here prevents a full fanout queue from pinning every ingress
/// permit and starving ordinary signed-query body decoding.
fn try_acquire_query_fanout_memory(
    app: &SharedAppState,
) -> Result<QueryFanoutMemoryReservation, Response> {
    app.query_fanout_inflight
        .clone()
        .try_acquire_owned()
        .map(QueryFanoutMemoryReservation::new)
        .map_err(|_| {
            torii_proxy_error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "query_capacity_exceeded",
                "cross-dataspace query fanout memory capacity is exhausted",
            )
        })
}

#[cfg(any(feature = "p2p_ws", feature = "connect", feature = "app_api"))]
fn hold_query_fanout_memory_in_response_body(
    response: Response,
    reservation: QueryFanoutMemoryReservation,
) -> Response {
    hold_query_fanout_memory_reservation_in_response_body(response, reservation)
}

/// Cloneable ownership token used to transfer one fanout reservation across
/// response-body and proxy-snapshot representations without reacquiring it.
#[derive(Clone)]
struct QueryFanoutMemoryReservation(Arc<tokio::sync::OwnedSemaphorePermit>);

impl QueryFanoutMemoryReservation {
    fn new(permit: tokio::sync::OwnedSemaphorePermit) -> Self {
        Self(Arc::new(permit))
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect", feature = "app_api"))]
fn hold_query_fanout_memory_reservation_in_response_body(
    mut response: Response,
    reservation: QueryFanoutMemoryReservation,
) -> Response {
    use http_body_util::BodyExt as _;

    response.extensions_mut().insert(reservation.clone());
    let (parts, body) = response.into_parts();
    let guarded_body = body.map_frame(move |frame| {
        let _reservation = &reservation;
        frame
    });
    Response::from_parts(parts, Body::new(guarded_body))
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn take_query_fanout_memory_reservation(
    response: &mut Response,
) -> Option<QueryFanoutMemoryReservation> {
    response
        .extensions_mut()
        .remove::<QueryFanoutMemoryReservation>()
}

fn should_skip_singleton_routed_query_route_error(response: &Response) -> bool {
    response.status() == StatusCode::NOT_FOUND
        || torii_response_has_reject_code(response, "route_unavailable")
}

/// Fixed-size summary of skippable routed-query failures.
///
/// A route response can own a bounded but still large body. Only these two
/// bits survive into the next route; [`Self::record_and_drop`] destroys the
/// response (and therefore its body stream) immediately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SkippedRoutedQueryErrors {
    saw_not_found: bool,
    saw_route_unavailable: bool,
}

impl SkippedRoutedQueryErrors {
    fn record_and_drop(&mut self, response: Response) {
        if torii_response_has_reject_code(&response, "route_unavailable") {
            self.saw_route_unavailable = true;
        } else {
            debug_assert_eq!(response.status(), StatusCode::NOT_FOUND);
            self.saw_not_found = true;
        }
        drop(response);
    }

    fn into_response(self) -> Response {
        if self.saw_not_found {
            return torii_proxy_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "query result was not found in any dataspace",
            );
        }
        if self.saw_route_unavailable {
            return torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no queried dataspace route was available",
            );
        }
        torii_proxy_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "query result was not found in any dataspace",
        )
    }
}
