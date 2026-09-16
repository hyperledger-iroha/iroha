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
    // Unlike the certificate-only decoder, the complete entrypoint may contain
    // a byte payload larger than a public-key payload. Bound every sequence by
    // the actual control cap, with the same four-times allocation/depth budget.
    let limits = norito::DecodeLimits::new(
        max_bytes,
        max_bytes,
        max_bytes,
        max_bytes.saturating_mul(4),
        64,
    );
    let input = norito::decode_canonical_with_limits::<LaneAdmittedInputV1>(bytes, limits)
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
    // including heterogeneous algorithms. These bytes are never evidence: only
    // the counting serializer sees them, and neither bytes nor a token escape.
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
    norito::canonical_frame_len(&sizing_only)
        .map_err(|error| format!("complete QueuePlan input size cannot be encoded: {error}"))
}
