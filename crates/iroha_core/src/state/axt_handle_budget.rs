// AXT handle-family budget and permanent-counter helpers live together so the
// state root retains a compact, reviewable production surface.
pub(crate) fn retain_until_slot_for_handle(
    handle: &AxtHandleFragment,
    nexus: &iroha_config::parameters::actual::Nexus,
    current_slot: u64,
) -> u64 {
    let expiry_slot = ivm::axt::expiry_slot_with_skew(
        handle.handle.expiry_slot,
        nexus.axt.slot_length_ms,
        nexus.axt.max_clock_skew_ms,
        handle.handle.max_clock_skew_ms,
    );
    let retention_cap = current_slot.saturating_add(nexus.axt.replay_retention_slots.get());
    expiry_slot.max(retention_cap)
}

fn resolve_axt_handle_budget_amount(
    record: &AxtEnvelopeRecord,
    fragment: &AxtHandleFragment,
) -> Result<Quantity, Error> {
    let selected_proof = fragment.proof.as_ref().or_else(|| {
        record
            .proofs
            .iter()
            .find(|proof| proof.dsid == fragment.intent.asset_dsid)
            .map(|proof| &proof.proof)
    });
    let resolved = ivm::axt::resolve_handle_amount_components(
        fragment.intent.asset_dsid,
        fragment.intent.op.amount.as_ref(),
        selected_proof.map(|proof| proof.payload.as_slice()),
    )
    .map_err(|error| {
        Error::InvariantViolation(
            format!("committed AXT handle amount cannot be resolved: {error:?}").into(),
        )
    })?;
    let fragment_amount_matches = fragment.intent.op.amount.as_ref().map_or_else(
        || fragment.amount.is_none(),
        |amount| fragment.amount.as_ref() == Some(amount),
    );
    if !fragment_amount_matches || fragment.amount_commitment != resolved.amount_commitment {
        return Err(Error::InvariantViolation(
            "committed AXT handle amount fields do not match the selected proof statement".into(),
        ));
    }
    Ok(resolved.amount)
}

fn advance_axt_policy_for_handle(
    mut policy: AxtPolicyEntry,
    handle: &AxtHandleFragment,
    current_slot: u64,
) -> Result<AxtPolicyEntry, Error> {
    let dsid = handle.intent.asset_dsid;
    if handle.handle.manifest_view_root != policy.manifest_root {
        return Err(Error::InvariantViolation(
            format!(
                "AXT handle for dataspace {} does not match the committed manifest root",
                dsid.as_u64()
            )
            .into(),
        ));
    }
    if handle.handle.target_lane != policy.target_lane {
        return Err(Error::InvariantViolation(
            format!(
                "AXT handle for dataspace {} targets lane {}, expected {}",
                dsid.as_u64(),
                handle.handle.target_lane,
                policy.target_lane
            )
            .into(),
        ));
    }
    policy.next_handle_counter =
        iroha_data_model::nexus::next_axt_handle_sub_nonce(&policy, &handle.handle).map_err(
            |error| {
                Error::InvariantViolation(
                    format!(
                        "AXT handle for dataspace {} violates the committed sequence: {error}",
                        dsid.as_u64()
                    )
                    .into(),
                )
            },
        )?;
    policy.current_slot = current_slot;
    Ok(policy)
}

fn axt_policy_identity_matches(left: &AxtPolicyEntry, right: &AxtPolicyEntry) -> bool {
    left.manifest_root == right.manifest_root && left.target_lane == right.target_lane
}

fn axt_policy_is_active(policy: &AxtPolicyEntry) -> bool {
    policy.manifest_root != [0; 32]
}

fn axt_policy_identity_changed(
    previous: Option<&AxtPolicyEntry>,
    next: Option<&AxtPolicyEntry>,
) -> bool {
    match (previous, next) {
        (Some(previous), Some(next)) => !axt_policy_identity_matches(previous, next),
        (Some(_), None) | (None, Some(_)) => true,
        (None, None) => false,
    }
}

pub(crate) fn axt_counter_after_block_boundary(
    previous_policy: Option<&AxtPolicyEntry>,
    next_policy: Option<&AxtPolicyEntry>,
    minimum_generation: u64,
    authorization_identity_changed: bool,
    counter_before_block: Option<AxtHandleCounterRecord>,
    current_counter: Option<AxtHandleCounterRecord>,
) -> Result<Option<AxtHandleCounterRecord>, AxtHandleCounterError> {
    let previous_was_active = previous_policy.is_some_and(axt_policy_is_active);
    let next_is_active = next_policy.is_some_and(axt_policy_is_active);
    let mut counter = current_counter
        .or(counter_before_block)
        .or_else(|| {
            previous_policy.and_then(|policy| {
                (policy.next_handle_counter != 0)
                    .then_some((policy.next_handle_counter, policy.active_handle_era))
                    .and_then(|(next, generation)| {
                        AxtHandleCounterRecord::try_from_parts(next, generation).ok()
                    })
            })
        })
        .or_else(|| {
            next_policy
                .filter(|policy| axt_policy_is_active(policy))
                .map(|_| AxtHandleCounterRecord::initial(minimum_generation))
        });
    let revokes_authority = previous_was_active
        || (counter_before_block.is_some() && !previous_was_active && next_is_active);
    let raises_generation_floor = counter
        .as_ref()
        .is_some_and(|counter| minimum_generation > counter.authorization_generation());
    if revokes_authority && (authorization_identity_changed || raises_generation_floor) {
        counter
            .as_mut()
            .expect("an authorized AXT policy must retain its permanent counter")
            .try_revoke_for_policy_transition(minimum_generation)?;
    }
    Ok(counter)
}

pub(crate) fn axt_policy_generation_minimum(
    world: &(impl WorldReadOnly + ?Sized),
    dataspace: DataSpaceId,
    policy: Option<&AxtPolicyEntry>,
) -> u64 {
    let Some(policy) = policy.filter(|policy| axt_policy_is_active(policy)) else {
        return 0;
    };
    world
        .space_directory_manifests()
        .iter()
        .filter_map(|(_, set)| set.get(&dataspace))
        .filter(|record| {
            record.is_active() && record.manifest_hash.as_ref() == policy.manifest_root.as_slice()
        })
        .map(|record| {
            record
                .lifecycle
                .activated_epoch
                .unwrap_or(record.manifest.activation_epoch)
        })
        .max()
        .unwrap_or(policy.active_handle_era)
}

fn axt_lane_map_from_lane_config(lane_config: &LaneConfig) -> BTreeMap<DataSpaceId, LaneId> {
    let mut lane_for_dataspace = BTreeMap::new();
    for entry in lane_config.entries() {
        lane_for_dataspace
            .entry(entry.dataspace_id)
            .or_insert(entry.lane_id);
    }
    lane_for_dataspace
}
