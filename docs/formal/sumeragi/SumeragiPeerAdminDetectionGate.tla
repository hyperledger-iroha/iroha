---- MODULE SumeragiPeerAdminDetectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi peer-admin transaction detection.

This slice captures the contract of `is_peer_admin_instruction(...)` and
`Actor::is_peer_admin_transaction(...)`:
- instruction IDs are matched case-insensitively,
- any ID containing `registerpeer` or `unregisterpeer` is admin-sensitive,
- unrelated IDs, reversed words, and empty IDs are not admin-sensitive,
- only external signed transactions are inspected,
- only executable instruction batches can be admin-sensitive, and
- a batch is admin-sensitive when any instruction in the batch is admin.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ExactRegister == "exact_register"
ExactUnregister == "exact_unregister"
MixedCaseRegister == "mixed_case_register"
MixedCaseUnregister == "mixed_case_unregister"
PrefixedRegister == "prefixed_register"
SuffixedUnregister == "suffixed_unregister"
ReversedPeerRegister == "reversed_peer_register"
RegisterDomain == "register_domain"
OtherInstruction == "other_instruction"
EmptyInstructionId == "empty_instruction_id"

InstructionCases == {
  ExactRegister,
  ExactUnregister,
  MixedCaseRegister,
  MixedCaseUnregister,
  PrefixedRegister,
  SuffixedUnregister,
  ReversedPeerRegister,
  RegisterDomain,
  OtherInstruction,
  EmptyInstructionId
}

SpecInstructionAdmin(c) ==
  c \in {
    ExactRegister,
    ExactUnregister,
    MixedCaseRegister,
    MixedCaseUnregister,
    PrefixedRegister,
    SuffixedUnregister
  }

ActualInstructionAdmin(c) ==
  CASE Bug = "reject_exact_register"
       /\ c = ExactRegister -> FALSE
    [] Bug = "reject_exact_unregister"
       /\ c = ExactUnregister -> FALSE
    [] Bug = "skip_lowercase"
       /\ c \in {MixedCaseRegister, MixedCaseUnregister} -> FALSE
    [] Bug = "skip_substring"
       /\ c \in {PrefixedRegister, SuffixedUnregister} -> FALSE
    [] Bug = "accept_reversed_peer_register"
       /\ c = ReversedPeerRegister -> TRUE
    [] Bug = "accept_register_domain"
       /\ c = RegisterDomain -> TRUE
    [] Bug = "accept_empty_id"
       /\ c = EmptyInstructionId -> TRUE
    [] OTHER -> SpecInstructionAdmin(c)

NoExternal == "no_external"
NonInstructionExecutable == "non_instruction_executable"
EmptyBatch == "empty_batch"
OneNonAdmin == "one_non_admin"
OneRegisterAdmin == "one_register_admin"
OneUnregisterAdmin == "one_unregister_admin"
OneMixedCaseAdmin == "one_mixed_case_admin"
OneSubstringAdmin == "one_substring_admin"
AdminFirstMixed == "admin_first_mixed"
AdminSecondMixed == "admin_second_mixed"
TwoNonAdmin == "two_non_admin"

TransactionCases == {
  NoExternal,
  NonInstructionExecutable,
  EmptyBatch,
  OneNonAdmin,
  OneRegisterAdmin,
  OneUnregisterAdmin,
  OneMixedCaseAdmin,
  OneSubstringAdmin,
  AdminFirstMixed,
  AdminSecondMixed,
  TwoNonAdmin
}

SpecTransactionAdmin(t) ==
  t \in {
    OneRegisterAdmin,
    OneUnregisterAdmin,
    OneMixedCaseAdmin,
    OneSubstringAdmin,
    AdminFirstMixed,
    AdminSecondMixed
  }

ActualTransactionAdmin(t) ==
  CASE Bug = "accept_non_external"
       /\ t = NoExternal -> TRUE
    [] Bug = "accept_non_instruction_executable"
       /\ t = NonInstructionExecutable -> TRUE
    [] Bug = "accept_empty_batch"
       /\ t = EmptyBatch -> TRUE
    [] Bug = "reject_single_register_admin"
       /\ t = OneRegisterAdmin -> FALSE
    [] Bug = "reject_single_unregister_admin"
       /\ t = OneUnregisterAdmin -> FALSE
    [] Bug = "skip_casefold_in_batch"
       /\ t = OneMixedCaseAdmin -> FALSE
    [] Bug = "skip_substring_in_batch"
       /\ t = OneSubstringAdmin -> FALSE
    [] Bug = "require_all_instructions_admin"
       /\ t \in {AdminFirstMixed, AdminSecondMixed} -> FALSE
    [] Bug = "check_only_first_instruction"
       /\ t = AdminSecondMixed -> FALSE
    [] Bug = "check_only_last_instruction"
       /\ t = AdminFirstMixed -> FALSE
    [] Bug = "accept_non_admin_batch"
       /\ t = TwoNonAdmin -> TRUE
    [] OTHER -> SpecTransactionAdmin(t)

Bugs == {
  "none",
  "reject_exact_register",
  "reject_exact_unregister",
  "skip_lowercase",
  "skip_substring",
  "accept_reversed_peer_register",
  "accept_register_domain",
  "accept_empty_id",
  "accept_non_external",
  "accept_non_instruction_executable",
  "accept_empty_batch",
  "reject_single_register_admin",
  "reject_single_unregister_admin",
  "skip_casefold_in_batch",
  "skip_substring_in_batch",
  "require_all_instructions_admin",
  "check_only_first_instruction",
  "check_only_last_instruction",
  "accept_non_admin_batch"
}

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs
  /\ \A c \in InstructionCases:
       /\ SpecInstructionAdmin(c) \in BOOLEAN
       /\ ActualInstructionAdmin(c) \in BOOLEAN
  /\ \A t \in TransactionCases:
       /\ SpecTransactionAdmin(t) \in BOOLEAN
       /\ ActualTransactionAdmin(t) \in BOOLEAN

PeerAdminDetectionCoreSafety ==
  /\ \A c \in InstructionCases:
       ActualInstructionAdmin(c) = SpecInstructionAdmin(c)
  /\ \A t \in TransactionCases:
       ActualTransactionAdmin(t) = SpecTransactionAdmin(t)

NoBugInvariant == PeerAdminDetectionCoreSafety

SafetyFast == PeerAdminDetectionCoreSafety

PeerAdminDetectionExactness ==
  /\ PeerAdminDetectionCoreSafety
PeerAdminDetectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PeerAdminDetectionExactness

BugRejectExactRegister == NoBugInvariant
BugRejectExactUnregister == NoBugInvariant
BugSkipLowercase == NoBugInvariant
BugSkipSubstring == NoBugInvariant
BugAcceptReversedPeerRegister == NoBugInvariant
BugAcceptRegisterDomain == NoBugInvariant
BugAcceptEmptyId == NoBugInvariant
BugAcceptNonExternal == NoBugInvariant
BugAcceptNonInstructionExecutable == NoBugInvariant
BugAcceptEmptyBatch == NoBugInvariant
BugRejectSingleRegisterAdmin == NoBugInvariant
BugRejectSingleUnregisterAdmin == NoBugInvariant
BugSkipCasefoldInBatch == NoBugInvariant
BugSkipSubstringInBatch == NoBugInvariant
BugRequireAllInstructionsAdmin == NoBugInvariant
BugCheckOnlyFirstInstruction == NoBugInvariant
BugCheckOnlyLastInstruction == NoBugInvariant
BugAcceptNonAdminBatch == NoBugInvariant

====
