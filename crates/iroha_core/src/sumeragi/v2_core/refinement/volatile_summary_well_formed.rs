macro_rules! volatile_summary_well_formed_body {
    ($summary:expr, $validator_count:expr) => {{
        $validator_count > 0u64
            && $validator_count <= u64::MAX / 2u64
            // At most two active phase pools are kept: current Prepare plus
            // either its same-round Commit or a durable same-round Commit
            // retained from an older lock round. A newly durable lock retires
            // the superseded older pool before its Commit signature completes.
            && $summary.vote_pools <= 2u64
            && $summary.vote_entries >= $summary.vote_pools
            && $summary.vote_entries <= $validator_count * 2u64
            // The current timeout pool plus exactly one adjacent future pool
            // are retained so staggered honest validators can form the TC
            // which resynchronizes the pacemaker.
            && $summary.timeout_vote_pools <= 2u64
            && $summary.timeout_vote_entries >= $summary.timeout_vote_pools
            && $summary.timeout_vote_entries <= $validator_count * 2u64
            // At most one locally formed certificate per phase and one TC per
            // retained timeout round.
            && $summary.formed_certificates <= 2u64
            && $summary.formed_timeouts <= 2u64
            // `OutboundControlClass` has seven exhaustive variants.
            && $summary.outbound_control <= 7u64
            // At most one live PrepareQC owns the current body pipeline.
            // Highest and locked add at most two durable references while a
            // strictly newer observation is awaiting its WAL acknowledgement.
            && $summary.pending_prepare <= 1u64
            && $summary.pending_prepare <= $summary.known_prepare
            && $summary.known_prepare <= 3u64
            && $summary.known_prepare - $summary.pending_prepare <= 2u64
            // Body work is sourced by a candidate, a pending certified body,
            // or the sole durable decision.  Two spare identities cover the
            // candidate/decision cases without trusting subject equality.
            && ($summary.body_work <= $summary.pending_prepare
                || $summary.body_work - $summary.pending_prepare <= 2u64)
            // The FIFO plus its sole in-flight element is bounded by durable
            // intents eligible for replay; no unsigned item may be invented.
            && $summary.signature_queue <= $summary.durable_signable_limit
            && (!$summary.awaiting_signature
                || $summary.signature_queue < $summary.durable_signable_limit)
    }};
}
