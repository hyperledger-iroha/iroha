/// Canonical complete input authenticated by its exact admission certificate.
///
/// Construction requires both certificate quorum verification and the existing
/// exact request/entrypoint/plan/context/journal-claim check. This is not evidence
/// that the local journal owns the body, that State inserted the admission, or
/// that its route/committee is active. Those policies remain separate boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedLaneAdmittedInputV1 {
    input: LaneAdmittedInputV1,
    certificate: ValidatedQueuePlanAdmissionCertificateV1,
}

impl ValidatedLaneAdmittedInputV1 {
    /// Borrow the exact checked complete input for canonical publication.
    #[must_use]
    pub fn input(&self) -> &LaneAdmittedInputV1 {
        &self.input
    }

    /// Consume the token and recover the exact checked complete input.
    #[must_use]
    pub fn into_input(self) -> LaneAdmittedInputV1 {
        self.input
    }

    /// Borrow the authenticated certificate and its existing registry projection.
    #[must_use]
    pub fn certificate(&self) -> &ValidatedQueuePlanAdmissionCertificateV1 {
        &self.certificate
    }

    /// Borrow the exact original outer entrypoint without converting its variant.
    #[must_use]
    pub fn entrypoint(&self) -> &TransactionEntrypoint {
        &self.input.entrypoint
    }
}

/// Decode and authenticate a bounded canonical complete admission input.
///
/// Certificate-only/partial responses keep their existing APIs and cannot create
/// this token. The complete control must fit the existing per-admission carrier
/// cap; a caller must separately establish ingress policy, exact State route
/// authority, carrier admission, and full global/native RS16 size feasibility.
///
/// # Errors
/// Rejects empty/oversized/noncanonical frames, allocation/shape excess, foreign
/// network, insufficient or invalid attestations, or any exact input, signed
/// identity, routing/context, semantic request or journal-claim mismatch.
pub fn decode_and_validate_lane_admitted_input_v1(
    network_id: &NetworkId,
    bytes: &[u8],
) -> Result<ValidatedLaneAdmittedInputV1, String> {
    let max_bytes = iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err("complete lane admitted input is empty or oversized".to_owned());
    }
    let input = LaneAdmittedInputV1::decode_canonical(bytes)
        .map_err(|error| format!("complete lane admitted input cannot be decoded: {error}"))?;
    #[cfg(test)]
    QUEUE_PLAN_AUTHENTICATION_OBSERVER.with(|observer| {
        let callback = observer.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    });
    let certificate = validate_queue_plan_admission_certificate_v1(
        network_id,
        input.certificate.clone(),
        QueuePlanAdmissionCertificateStrengthV1::Quorum,
    )?;
    let routing_plan = input.routing_plan()?;
    validate_queue_plan_binding_for_request(
        &input.certificate.binding,
        network_id,
        &input.entrypoint,
        &routing_plan,
    )?;
    Ok(ValidatedLaneAdmittedInputV1 { input, certificate })
}

/// Count the largest canonical complete input for this exact binding and quorum.
///
/// This is a sizing operation, not authentication or an admission promise. The
/// exact coordinator roster determines every canonical signature length; the
/// largest threshold of these shapes bounds every possible exact quorum. The
/// returned count includes the complete original entrypoint, certificate,
/// canonical frame and nested length/offset prefixes. It excludes the enclosing
/// block and lane proposal wrappers, whose signed DA feasibility is a separate
/// pre-admission requirement.
///
/// # Errors
/// Rejects malformed or mismatched binding/body/plan claims, unrecognized key
/// algorithms, or canonical serialization errors. No placeholder evidence or
/// validated-input token escapes this function.
pub fn maximum_lane_admitted_input_encoded_len_v1(
    entrypoint: &TransactionEntrypoint,
    binding: &QueuePlanAdmissionBindingV1,
) -> Result<usize, String> {
    let sizing_only = maximum_lane_admitted_input_sizing_value_v1(entrypoint, binding)?;
    norito::canonical_frame_len(&sizing_only)
        .map_err(|error| format!("complete QueuePlan input size cannot be encoded: {error}"))
}

// The only constructor of worst-quorum placeholder bytes remains private. It
// creates no authenticated token; public callers receive byte counts only.
fn maximum_lane_admitted_input_sizing_value_v1(
    entrypoint: &TransactionEntrypoint,
    binding: &QueuePlanAdmissionBindingV1,
) -> Result<LaneAdmittedInputV1, String> {
    binding.validate_structure()?;
    let plan = binding.routing_plan()?;
    validate_queue_plan_binding_for_transaction_and_plan(binding, entrypoint, &plan)?;
    let coordinator = binding
        .admission_context
        .route_incarnations
        .first()
        .ok_or_else(|| "QueuePlan input sizing requires a coordinator".to_owned())?;
    let threshold = usize::from(coordinator.durability_threshold);
    let mut shapes = coordinator
        .validator_set
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let length = peer
                .public_key()
                .try_algorithm()
                .map_err(|error| format!("QueuePlan sizing key algorithm is invalid: {error}"))?
                .signature_payload_len();
            let index = u16::try_from(index)
                .map_err(|_| "QueuePlan sizing validator index exceeds u16".to_owned())?;
            Ok((length, index))
        })
        .collect::<Result<Vec<_>, String>>()?;
    shapes.sort_unstable_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    shapes.truncate(threshold);
    shapes.sort_unstable_by_key(|(_, index)| *index);
    // Canonical V1 uses uncompressed frames, fixed scalar widths and monotone
    // sequence framing. Signature contents do not affect length. Selecting the
    // threshold largest payload lengths therefore bounds all signer subsets,
    // including heterogeneous algorithms. These bytes are never evidence: the
    // private sizing scope exposes neither placeholder bytes nor an authenticated token.
    let attestations = shapes
        .into_iter()
        .map(
            |(length, validator_index)| QueuePlanAdmissionAttestationV1 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                validator_index,
                signature: Signature::from_bytes(&vec![0xa5; length]),
            },
        )
        .collect();
    let sizing_only = LaneAdmittedInputV1 {
        entrypoint: entrypoint.clone(),
        certificate: QueuePlanAdmissionCertificateV1 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
            binding: binding.clone(),
            attestations,
        },
    };
    Ok(sizing_only)
}

/// Largest encoded bodies and actual publication frames for one admitted input.
///
/// This is arithmetic only: no field grants route, transport, finality or voting
/// authority, and no placeholder bytes escape the sizing implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneAdmittedInputEnvelopeSizeBoundsV1 {
    /// Canonical complete-input frame, bounded by the per-control protocol cap.
    pub complete_input_bytes: usize,
    /// Full canonical `LaneInputPayloadV1`, including all distinct route slots.
    pub native_payload_bytes: usize,
    /// Distinct routes, counting a coordinator/participant overlap once.
    pub native_route_slots: usize,
    /// Plaintext direct Relay/Data frame for Torii's publication NetworkMessage.
    pub publication_plaintext_bytes: usize,
    /// Encrypted publication plus stream prefix, charged by P2P's queue owner.
    pub publication_queue_bytes: usize,
    /// Plaintext direct Relay/Data frame for validator-to-leader republication.
    pub republication_plaintext_bytes: usize,
    /// Encrypted republication plus stream prefix, charged by P2P's queue owner.
    pub republication_queue_bytes: usize,
}

/// Count actual native-input and P2P publication wrappers before a promise.
///
/// Reuses the same exact worst-quorum signature shapes as
/// [`maximum_lane_admitted_input_encoded_len_v1`]. Native descriptor identities
/// and heights have fixed canonical widths; their sizing-only values grant no
/// authority. Slots derive from distinct binding routes, never local FIFO or
/// current context guesses. The P2P owners count the real NetworkMessage variants,
/// direct relay, signature, Data frame, encryption and stream prefix.
///
/// Callers must separately enforce signed global/native RS16 geometry, the
/// **entire** global candidate, transport topic caps and outbound reservations.
/// These lengths are not a complete admission-feasibility decision. This function
/// intentionally does not change the existing Torii preacceptance guard.
///
/// # Errors
/// Rejects inconsistent input/binding claims, an input above the complete-control
/// cap, conflicting route incarnations, malformed bounded slots, codec errors or
/// transport length overflow. No invalid evidence or authenticated token escapes.
pub fn maximum_lane_admitted_input_envelope_sizes_v1(
    entrypoint: &TransactionEntrypoint,
    binding: &QueuePlanAdmissionBindingV1,
) -> Result<LaneAdmittedInputEnvelopeSizeBoundsV1, String> {
    use iroha_data_model::block::{
        lane_consensus::QueuePlanAdmissionPriorityV1,
        lane_input::{
            LANE_INPUT_VERSION_V1, LaneInputDescriptorV1, LaneInputPayloadV1, LaneInputRouteSlotV1,
        },
    };
    use std::{collections::BTreeMap, sync::Arc};

    let sizing_only = maximum_lane_admitted_input_sizing_value_v1(entrypoint, binding)?;
    let complete_input_bytes = norito::canonical_frame_len(&sizing_only)
        .map_err(|error| format!("complete QueuePlan input size cannot be encoded: {error}"))?;
    if complete_input_bytes > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES {
        return Err(
            "complete QueuePlan input exceeds its per-control cap before envelope sizing"
                .to_owned(),
        );
    }
    // Bound allocation before materializing the one canonical Vec<u8> field
    // used by both actual publication variants. No second codec is introduced.
    let control_bytes = norito::encode_canonical(&sizing_only)
        .map_err(|error| format!("complete QueuePlan sizing frame cannot be encoded: {error}"))?;
    let mut routes = BTreeMap::new();
    for bound in &binding.admission_context.route_incarnations {
        let route = bound.leg.route;
        if let Some(previous) =
            routes.insert((route.lane_id, route.dataspace_id), bound.lane_incarnation)
            && previous != bound.lane_incarnation
        {
            return Err("QueuePlan sizing route has conflicting incarnations".to_owned());
        }
    }
    let slots = routes
        .into_iter()
        .map(
            |((lane_id, dataspace_id), lane_incarnation)| LaneInputRouteSlotV1 {
                route: crate::queue::RoutingDecision::new(lane_id, dataspace_id),
                lane_incarnation,
                instance_id: Hash::new(b"non-authorizing native sizing instance"),
                lane_height: 1,
            },
        )
        .collect::<Vec<_>>();
    let native_route_slots = slots.len();
    let native_sizing_only = LaneInputPayloadV1 {
        descriptor: LaneInputDescriptorV1 {
            version: LANE_INPUT_VERSION_V1,
            admission_priority: QueuePlanAdmissionPriorityV1::new(1, 0)
                .map_err(|error| format!("native sizing position is invalid: {error}"))?,
            admission_carrier_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"non-authorizing native sizing carrier",
            )),
            admitted_input_hash: Hash::new(&control_bytes),
            slots,
        },
        input: sizing_only,
    };
    native_sizing_only.descriptor.validate_structure()?;
    let native_payload_bytes = norito::canonical_frame_len(&native_sizing_only)
        .map_err(|error| format!("native input size cannot be encoded: {error}"))?;

    let publication = crate::NetworkMessage::QueuePlanAdmissionPublication(Arc::new(
        QueuePlanAdmissionPublicationV1 {
            schema_version: QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1,
            certificate: control_bytes.clone(),
        },
    ));
    let republication =
        crate::NetworkMessage::QueuePlanAdmissionCertificate(Arc::new(control_bytes));
    let (publication_plaintext_bytes, publication_queue_bytes) =
        queue_plan_direct_frame_sizes_v1(&publication)?;
    let (republication_plaintext_bytes, republication_queue_bytes) =
        queue_plan_direct_frame_sizes_v1(&republication)?;
    Ok(LaneAdmittedInputEnvelopeSizeBoundsV1 {
        complete_input_bytes,
        native_payload_bytes,
        native_route_slots,
        publication_plaintext_bytes,
        publication_queue_bytes,
        republication_plaintext_bytes,
        republication_queue_bytes,
    })
}

// Serialize the real application payload, then use the transport owner's exact
// direct-node framing. P2P's authenticated node identities are BLS-normal; the
// helper owns those key/signature widths and rejects arithmetic overflow.
fn queue_plan_direct_frame_sizes_v1(
    message: &crate::NetworkMessage,
) -> Result<(usize, usize), String> {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload_bytes = norito::core::encoded_payload_len(message)
        .map_err(|error| format!("QueuePlan NetworkMessage cannot be sized: {error}"))?;
    let plaintext = iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<
        crate::NetworkMessage,
    >(payload_bytes);
    let queued = iroha_p2p::frame_queue_charge(plaintext)
        .ok_or_else(|| "QueuePlan transport frame size overflows".to_owned())?;
    Ok((plaintext, queued))
}
