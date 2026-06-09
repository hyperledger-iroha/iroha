---- MODULE SumeragiHandshakeGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi p2p handshake admission.

This slice pins `HandshakeGate::local(...)` and
`HandshakeGate::validate_peer(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ValidateCases == {
  "all_match",
  "chain_mismatch",
  "mode_mismatch",
  "proto_mismatch",
  "fingerprint_mismatch",
  "chain_mode_mismatch",
  "mode_proto_mismatch",
  "proto_fingerprint_mismatch",
  "all_mismatch"
}

LocalFields == {"chain", "mode", "proto", "fingerprint"}

SpecValidate(c) ==
  CASE c = "all_match" -> "ok"
    [] c \in {"chain_mismatch", "chain_mode_mismatch", "all_mismatch"} ->
         "chain_error"
    [] c \in {"mode_mismatch", "mode_proto_mismatch"} -> "mode_error"
    [] c = "proto_mismatch" -> "proto_error"
    [] c = "proto_fingerprint_mismatch" -> "proto_error"
    [] OTHER -> "fingerprint_error"

ActualValidate(c) ==
  CASE Bug = "validate_rejects_match"
       /\ c = "all_match" -> "fingerprint_error"
    [] Bug = "validate_accepts_chain_mismatch"
       /\ c = "chain_mismatch" -> "ok"
    [] Bug = "validate_accepts_mode_mismatch"
       /\ c = "mode_mismatch" -> "ok"
    [] Bug = "validate_accepts_proto_mismatch"
       /\ c = "proto_mismatch" -> "ok"
    [] Bug = "validate_accepts_fingerprint_mismatch"
       /\ c = "fingerprint_mismatch" -> "ok"
    [] Bug = "validate_chain_priority_uses_mode"
       /\ c = "chain_mode_mismatch" -> "mode_error"
    [] Bug = "validate_mode_priority_uses_proto"
       /\ c = "mode_proto_mismatch" -> "proto_error"
    [] Bug = "validate_proto_priority_uses_fingerprint"
       /\ c = "proto_fingerprint_mismatch" -> "fingerprint_error"
    [] Bug = "validate_all_mismatch_uses_fingerprint"
       /\ c = "all_mismatch" -> "fingerprint_error"
    [] OTHER -> SpecValidate(c)

SpecLocal(field) ==
  CASE field = "chain" -> "local_chain"
    [] field = "mode" -> "local_mode"
    [] field = "proto" -> "proto_v1"
    [] OTHER -> "local_fingerprint"

ActualLocal(field) ==
  CASE Bug = "local_uses_peer_chain"
       /\ field = "chain" -> "peer_chain"
    [] Bug = "local_uses_peer_mode"
       /\ field = "mode" -> "peer_mode"
    [] Bug = "local_proto_zero"
       /\ field = "proto" -> "proto_zero"
    [] Bug = "local_fingerprint_zero"
       /\ field = "fingerprint" -> "fingerprint_zero"
    [] OTHER -> SpecLocal(field)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "validate_rejects_match",
       "validate_accepts_chain_mismatch",
       "validate_accepts_mode_mismatch",
       "validate_accepts_proto_mismatch",
       "validate_accepts_fingerprint_mismatch",
       "validate_chain_priority_uses_mode",
       "validate_mode_priority_uses_proto",
       "validate_proto_priority_uses_fingerprint",
       "validate_all_mismatch_uses_fingerprint",
       "local_uses_peer_chain",
       "local_uses_peer_mode",
       "local_proto_zero",
       "local_fingerprint_zero"
     }
  /\ checked = 0

HandshakeMatchesSpec ==
  /\ \A c \in ValidateCases:
       ActualValidate(c) = SpecValidate(c)
  /\ \A field \in LocalFields:
       ActualLocal(field) = SpecLocal(field)

SafetyFast ==
  HandshakeMatchesSpec

BugValidateRejectsMatch ==
  ActualValidate("all_match") = SpecValidate("all_match")

BugValidateAcceptsChainMismatch ==
  ActualValidate("chain_mismatch") = SpecValidate("chain_mismatch")

BugValidateAcceptsModeMismatch ==
  ActualValidate("mode_mismatch") = SpecValidate("mode_mismatch")

BugValidateAcceptsProtoMismatch ==
  ActualValidate("proto_mismatch") = SpecValidate("proto_mismatch")

BugValidateAcceptsFingerprintMismatch ==
  ActualValidate("fingerprint_mismatch") = SpecValidate("fingerprint_mismatch")

BugValidateChainPriorityUsesMode ==
  ActualValidate("chain_mode_mismatch") = SpecValidate("chain_mode_mismatch")

BugValidateModePriorityUsesProto ==
  ActualValidate("mode_proto_mismatch") = SpecValidate("mode_proto_mismatch")

BugValidateProtoPriorityUsesFingerprint ==
  ActualValidate("proto_fingerprint_mismatch") =
    SpecValidate("proto_fingerprint_mismatch")

BugValidateAllMismatchUsesFingerprint ==
  ActualValidate("all_mismatch") = SpecValidate("all_mismatch")

BugLocalUsesPeerChain ==
  ActualLocal("chain") = SpecLocal("chain")

BugLocalUsesPeerMode ==
  ActualLocal("mode") = SpecLocal("mode")

BugLocalProtoZero ==
  ActualLocal("proto") = SpecLocal("proto")

BugLocalFingerprintZero ==
  ActualLocal("fingerprint") = SpecLocal("fingerprint")

====
