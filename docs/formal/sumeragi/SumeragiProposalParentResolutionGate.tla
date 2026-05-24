---- MODULE SumeragiProposalParentResolutionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal parent resolution and inline frontier
backup transport seeding.

This slice models `resolve_prev_block_for_proposal(...)` and
`should_seed_frontier_backup_transport(...)`, plus the immediate proposal
assembly behavior that defers when no parent is available for heights above
genesis. Kura-backed previous blocks take precedence over pending parents.
Pending fallback is allowed only when the pending block is keyed by the
highest-QC subject and has height exactly one below the proposal height. A
previous-height `usize` conversion overflow skips the Kura lookup but still
permits a matching pending-parent fallback.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  parent_source,
  \* @type: Bool;
  defer_parent_missing,
  \* @type: Bool;
  kura_lookup_attempted,
  \* @type: Bool;
  pending_fallback_checked,
  \* @type: Bool;
  overflow_logged,
  \* @type: Bool;
  seed_backup_transport,
  \* @type: Bool;
  use_rbc_transport

\* @type: <<Str, Str, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, parent_source, defer_parent_missing,
  kura_lookup_attempted, pending_fallback_checked, overflow_logged,
  seed_backup_transport, use_rbc_transport>>

ParentCases == {
  "height_zero",
  "height_one",
  "height_one_pending_ignored",
  "kura_parent",
  "kura_preferred_over_pending",
  "parent_missing_no_pending",
  "pending_parent",
  "pending_wrong_hash",
  "pending_wrong_height",
  "usize_overflow_pending_parent",
  "usize_overflow_no_pending"
}

TransportCases == {
  "transport_no_da_inline_backup",
  "transport_da_inline_backup",
  "transport_da_inline_no_backup",
  "transport_da_rbc_primary_backup",
  "transport_da_rbc_primary_no_backup",
  "transport_no_da_rbc_primary"
}

Cases == ParentCases \union TransportCases

NoParentNoDeferCases == {
  "height_zero",
  "height_one",
  "height_one_pending_ignored"
}

KuraParentCases == {"kura_parent", "kura_preferred_over_pending"}
PendingParentCases == {"pending_parent", "usize_overflow_pending_parent"}
MissingParentDeferCases == {
  "parent_missing_no_pending",
  "pending_wrong_hash",
  "pending_wrong_height",
  "usize_overflow_no_pending"
}

NonzeroHeightParentCases == ParentCases \ {"height_zero"}
PendingFallbackCandidateCases == ParentCases
  \ {"height_zero", "height_one", "height_one_pending_ignored", "kura_parent",
     "kura_preferred_over_pending"}
OverflowCases == {"usize_overflow_pending_parent", "usize_overflow_no_pending"}

SeedAllowedCases == {"transport_da_inline_backup"}
RbcTransportCases == {
  "transport_da_inline_backup",
  "transport_da_rbc_primary_backup",
  "transport_da_rbc_primary_no_backup"
}

SpecParentSource(c) ==
  IF c \in KuraParentCases THEN "kura"
  ELSE IF c \in PendingParentCases THEN "pending"
  ELSE "none"

SpecDeferParentMissing(c) == c \in MissingParentDeferCases

SpecKuraLookupAttempted(c) ==
  c \in NonzeroHeightParentCases /\ c \notin OverflowCases

SpecPendingFallbackChecked(c) == c \in PendingFallbackCandidateCases

SpecOverflowLogged(c) == c \in OverflowCases

SpecSeedBackupTransport(c) == c \in SeedAllowedCases

SpecUseRbcTransport(c) == c \in RbcTransportCases

ActualParentSource(c) ==
  IF Bug = "return_parent_at_height_zero" /\ c = "height_zero" THEN "kura"
  ELSE IF Bug = "return_parent_at_height_one"
      /\ c \in {"height_one", "height_one_pending_ignored"} THEN "pending"
  ELSE IF Bug = "skip_kura_parent" /\ c \in KuraParentCases THEN "none"
  ELSE IF Bug = "pending_overrides_kura"
      /\ c = "kura_preferred_over_pending" THEN "pending"
  ELSE IF Bug = "skip_pending_parent" /\ c \in PendingParentCases THEN "none"
  ELSE IF Bug = "pending_wrong_hash_accepted" /\ c = "pending_wrong_hash" THEN "pending"
  ELSE IF Bug = "pending_wrong_height_accepted" /\ c = "pending_wrong_height" THEN "pending"
  ELSE IF Bug = "overflow_blocks_pending_fallback"
      /\ c = "usize_overflow_pending_parent" THEN "none"
  ELSE SpecParentSource(c)

ActualDeferParentMissing(c) ==
  \/ /\ SpecDeferParentMissing(c)
     /\ Bug # "skip_defer_on_missing_parent"
  \/ /\ c \in (KuraParentCases \union PendingParentCases)
     /\ Bug = "defer_when_parent_found"

ActualKuraLookupAttempted(c) ==
  \/ /\ SpecKuraLookupAttempted(c)
     /\ Bug # "skip_kura_lookup_nonzero"
  \/ /\ c = "height_zero"
     /\ Bug = "lookup_kura_height_zero"

ActualPendingFallbackChecked(c) ==
  \/ /\ SpecPendingFallbackChecked(c)
     /\ Bug # "overflow_blocks_pending_fallback"
  \/ /\ c = "height_one_pending_ignored"
     /\ Bug = "height_one_checks_pending"

ActualOverflowLogged(c) ==
  /\ SpecOverflowLogged(c)
  /\ Bug # "skip_overflow_log"

ActualSeedBackupTransport(c) ==
  \/ /\ SpecSeedBackupTransport(c)
     /\ Bug # "skip_seed_all_enabled"
  \/ /\ c = "transport_no_da_inline_backup"
     /\ Bug = "seed_without_da"
  \/ /\ c = "transport_da_rbc_primary_backup"
     /\ Bug = "seed_without_inline"
  \/ /\ c = "transport_da_inline_no_backup"
     /\ Bug = "seed_without_backup"

ActualUseRbcTransport(c) ==
  \/ /\ SpecUseRbcTransport(c)
     /\ ~(Bug = "skip_rbc_primary"
          /\ c \in {"transport_da_rbc_primary_backup",
                    "transport_da_rbc_primary_no_backup"})
     /\ ~(Bug = "skip_rbc_backup" /\ c = "transport_da_inline_backup")
  \/ /\ c = "transport_no_da_rbc_primary"
     /\ Bug = "rbc_without_da"
  \/ /\ c = "transport_da_inline_no_backup"
     /\ Bug = "rbc_inline_without_backup"

Init ==
  /\ candidate = "none"
  /\ parent_source = "none"
  /\ defer_parent_missing = FALSE
  /\ kura_lookup_attempted = FALSE
  /\ pending_fallback_checked = FALSE
  /\ overflow_logged = FALSE
  /\ seed_backup_transport = FALSE
  /\ use_rbc_transport = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ parent_source' = ActualParentSource(c)
  /\ defer_parent_missing' = ActualDeferParentMissing(c)
  /\ kura_lookup_attempted' = ActualKuraLookupAttempted(c)
  /\ pending_fallback_checked' = ActualPendingFallbackChecked(c)
  /\ overflow_logged' = ActualOverflowLogged(c)
  /\ seed_backup_transport' = ActualSeedBackupTransport(c)
  /\ use_rbc_transport' = ActualUseRbcTransport(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ parent_source \in {"none", "kura", "pending"}
  /\ defer_parent_missing \in BOOLEAN
  /\ kura_lookup_attempted \in BOOLEAN
  /\ pending_fallback_checked \in BOOLEAN
  /\ overflow_logged \in BOOLEAN
  /\ seed_backup_transport \in BOOLEAN
  /\ use_rbc_transport \in BOOLEAN

ParentSourceMatchesSpec ==
  candidate = "none" \/ parent_source = SpecParentSource(candidate)

ParentMissingDeferralMatchesSpec ==
  candidate = "none" \/ defer_parent_missing = SpecDeferParentMissing(candidate)

KuraLookupMatchesSpec ==
  candidate = "none" \/ kura_lookup_attempted = SpecKuraLookupAttempted(candidate)

PendingFallbackMatchesSpec ==
  candidate = "none" \/
    pending_fallback_checked = SpecPendingFallbackChecked(candidate)

OverflowLoggingMatchesSpec ==
  candidate = "none" \/ overflow_logged = SpecOverflowLogged(candidate)

BackupSeedMatchesSpec ==
  candidate = "none" \/ seed_backup_transport = SpecSeedBackupTransport(candidate)

RbcTransportMatchesSpec ==
  candidate = "none" \/ use_rbc_transport = SpecUseRbcTransport(candidate)

KuraParentTakesPrecedence ==
  candidate \in KuraParentCases => parent_source = "kura"

PendingParentRequiresMatchingSubjectAndHeight ==
  parent_source = "pending" => candidate \in PendingParentCases

ParentMissingDefersOnlyAboveGenesis ==
  defer_parent_missing => candidate \in MissingParentDeferCases

HeightZeroAndOneDoNotResolveParentOrDefer ==
  candidate \in NoParentNoDeferCases =>
    /\ parent_source = "none"
    /\ ~defer_parent_missing
    /\ ~pending_fallback_checked

MissingParentAboveGenesisDefers ==
  candidate \in MissingParentDeferCases =>
    /\ parent_source = "none"
    /\ defer_parent_missing

OverflowStillAllowsPendingFallback ==
  candidate = "usize_overflow_pending_parent" =>
    /\ overflow_logged
    /\ pending_fallback_checked
    /\ parent_source = "pending"
    /\ ~defer_parent_missing

BackupSeedRequiresDaInlineAndConfig ==
  seed_backup_transport => candidate \in SeedAllowedCases

RbcTransportRequiresDaAndEitherPrimaryOrBackup ==
  use_rbc_transport => candidate \in RbcTransportCases

InlineWithoutBackupDoesNotUseRbc ==
  candidate = "transport_da_inline_no_backup" =>
    /\ ~seed_backup_transport
    /\ ~use_rbc_transport

DaRbcPrimaryUsesRbcWithoutSeedingBackup ==
  candidate \in {"transport_da_rbc_primary_backup",
                 "transport_da_rbc_primary_no_backup"} =>
    /\ ~seed_backup_transport
    /\ use_rbc_transport

DaInlineBackupSeedsAndUsesRbc ==
  candidate = "transport_da_inline_backup" =>
    /\ seed_backup_transport
    /\ use_rbc_transport

Safety ==
  /\ ParentSourceMatchesSpec
  /\ ParentMissingDeferralMatchesSpec
  /\ KuraLookupMatchesSpec
  /\ PendingFallbackMatchesSpec
  /\ OverflowLoggingMatchesSpec
  /\ BackupSeedMatchesSpec
  /\ RbcTransportMatchesSpec
  /\ KuraParentTakesPrecedence
  /\ PendingParentRequiresMatchingSubjectAndHeight
  /\ ParentMissingDefersOnlyAboveGenesis
  /\ HeightZeroAndOneDoNotResolveParentOrDefer
  /\ MissingParentAboveGenesisDefers
  /\ OverflowStillAllowsPendingFallback
  /\ BackupSeedRequiresDaInlineAndConfig
  /\ RbcTransportRequiresDaAndEitherPrimaryOrBackup
  /\ InlineWithoutBackupDoesNotUseRbc
  /\ DaRbcPrimaryUsesRbcWithoutSeedingBackup
  /\ DaInlineBackupSeedsAndUsesRbc

=============================================================================
