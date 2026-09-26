# F02 pending penalty backing admission, 2026-09-24

Scope: the existing `optimizations` checkout. Parent penalty planning now counts
due committed evidence through one borrowed State view, acquires the exact
`ChargedBuffer` layout from the State-owned evidence preparation pool, and only
then constructs its stake index. The original charge stays with the fixed
pending metadata backing through ordered scratch derivation and refunds when
that backing is dropped. In-place ordering uses an allocation-free unstable
sort over unique evidence keys. The retained evidence rows remain unchanged
when local capacity refuses the plan; no consensus-invalid transaction or
block result is inferred from that refusal.

The maximum pending metadata backing has 124 entries, the committed evidence
record bound. The concrete entry layout is the transparent Core wrapper around
the configuration-owned `(Hash, u64, Option<(ValidatorIndex, PeerId)>)` layout.
The configured minimum funds one maximum prune-key backing plus one maximum
pending metadata backing; the default funds eight of each, preserving the
existing eight concurrent prune plans. User parsing and State installation
enforce the same minimum, and the actual default and canonical config fixture
carry the expanded pool. This is process-local resource admission, not a
consensus parameter or a change to evidence or block wire layouts.

Follower NPoS action derivation now preserves typed
`BlockValidationError::EvidencePreparation` instead of treating pool exhaustion
as invalid NPoS effects. The live Apply classifier already maps a capacity
refusal to its original pool-release wait and retains the body. Proposer
derivation maps the same typed failure to a local candidate preparation error;
the original proposal owner waits on that same release before retrying. A
permanent local allocator or configuration failure remains a local
qualification error and cannot be signed as candidate evidence.

Focused cases cover exact layout and config minimum/default, one-byte capacity
refusal with unchanged committed evidence and same-State retry after original
release, original charge lifetime, follower and proposer typed error mapping,
and proposal-owner wait retirement. The coordinated offline, locked Config and
Core builds completed successfully on this frozen source. The Config
`nexus_evidence_preparation_pool_admits_one_plan_and_defaults_to_eight` and
`minimal_config_snapshot` selectors each passed 1/1. The Core
`pending_penalty_backing_refusal_preserves_source_and_retries_after_original_release`,
`parent_penalty_plan_retains_only_due_evidence_metadata`,
`funded_prune_scan_preserves_terminal_order_and_original_pool_release`,
`configured_evidence_preparation_pool_preserves_identity_and_refuses_live_replacement`,
and `penalty_derivation_capacity_is_local_validation_not_invalid_effects`
selectors each passed 1/1; the two `penalty_preparation_` proposer selectors
passed 2/2. The adjacent F03 canonical worker selectors passed 2/2 in that
same Core build. These are focused local results, not full workspace or
multi-validator qualification.

This is deliberately a fixed backing slice. Cloned `PeerId` key bytes,
`PublicLaneStakeIndex` B-trees and share vectors, validator locator maps,
adaptive `Quantity` values, scratch `StateBlock`, and emitted action vectors
still need complete reservation and lifetime ownership. The wider F02 and
first-release resource gates remain open.
