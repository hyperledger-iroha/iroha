/// Remove retired source occurrences from semantic work already owned by the
/// lane adapter. A malformed effect which never carried its required route is
/// left intact so normal strict validation rejects it.
fn retain_active_owned_reply_routes(effect: &mut V2LaneWorkEffect) -> bool {
    retain_active_owned_reply_routes_with_snapshot_hook(effect, || {})
}
#[cfg(test)]
fn retain_active_owned_reply_routes_after_snapshot<AfterSnapshot>(
    effect: &mut V2LaneWorkEffect,
    after_snapshot: AfterSnapshot,
) -> bool
where
    AfterSnapshot: FnOnce(),
{
    retain_active_owned_reply_routes_with_snapshot_hook(effect, after_snapshot)
}
fn retain_active_owned_reply_routes_with_snapshot_hook<AfterSnapshot>(
    effect: &mut V2LaneWorkEffect,
    after_snapshot: AfterSnapshot,
) -> bool
where
    AfterSnapshot: FnOnce(),
{
    if let V2LaneWorkEffect::PostDurableLaneCertificate {
        reply_routes,
        ingress_ownership,
        ..
    } = effect
    {
        let Some(routes) = reply_routes.as_mut() else {
            return true;
        };
        let Some(ownership) = ingress_ownership.as_mut() else {
            return true;
        };
        if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {
            return true;
        }
        let (retained_routes, receipt) = routes.retain_active_with_receipt();
        after_snapshot();
        let Some(projected_routes) = ownership.project_retained_reply_routes(receipt) else {
            // Preserve malformed pre-existing ownership for strict dispatch;
            // ordinary retirement cannot reach this branch because the exact
            // route snapshot is projected without another liveness read.
            return true;
        };
        *routes = projected_routes;
        return retained_routes != 0;
    }
    let reply_routes = match effect {
        V2LaneWorkEffect::PostNativeAmx {
            reply_routes,
            message: NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_),
            ..
        } => reply_routes,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            reply_routes,
            message,
            ..
        } if matches!(
            message.as_ref(),
            CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_)
        ) =>
        {
            reply_routes
        }
        V2LaneWorkEffect::PostLaneBlock { .. }
        | V2LaneWorkEffect::PostDurableLaneCertificate { .. }
        | V2LaneWorkEffect::PostNativeAmx { .. }
        | V2LaneWorkEffect::PostLaneDrainVote { .. }
        | V2LaneWorkEffect::BroadcastMerge(_)
        | V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { .. }
        | V2LaneWorkEffect::PostCertifiedMergeSidecar { .. } => return true,
    };
    let Some(routes) = reply_routes.as_mut() else {
        return true;
    };
    let before = routes.clone();
    let (retained, receipt) = routes.retain_active_with_receipt();
    let Some(projected) = receipt.into_output(&before) else {
        // Preserve the operation's mutated value for normal strict dispatch;
        // this branch is unreachable for a module-minted receipt and exists
        // only to fail closed if its exact-history contract is broken.
        return true;
    };
    *routes = projected;
    retained != 0
}
