--------------------------- MODULE ZkVerifierAdmission ---------------------------
EXTENDS FiniteSets, TLC

\* This model covers verifier admission and proof-binding control invariants.
\* It intentionally abstracts algebraic soundness. The mutation constants below
\* model individual classes of fail-open implementation bugs.

CONSTANTS
    BUG_MISSING_VK_ACCEPTS,
    BUG_WRONG_VK_HASH_ACCEPTS,
    BUG_CIRCUIT_MISMATCH_ACCEPTS,
    BUG_OMIT_STARK_DOMAIN_BINDING,
    BUG_DISABLED_BACKEND_ACCEPTS,
    BUG_DIAGNOSTIC_ENDPOINT_PROMOTION,
    BUG_FASTPQ_CLAIM_MISMATCH_ACCEPTS,
    BUG_REPLAY_ACCEPTS

VARIABLES accepted, diagnostic

vars == <<accepted, diagnostic>>

Proofs ==
    {
        "good_halo2",
        "good_stark",
        "good_zk_ace",
        "good_fastpq",
        "missing_vk",
        "wrong_vk_hash",
        "circuit_mismatch",
        "stark_domain_swap",
        "disabled_backend",
        "trusted_setup_backend",
        "developer_only_backend",
        "oversized_proof",
        "decode_only",
        "diagnostic_only",
        "fastpq_claim_mismatch",
        "zk_ace_replay"
    }

HasActiveVk(p) ==
    p \in (Proofs \ {"missing_vk"})

VkHashBound(p) ==
    p \in (Proofs \ {"wrong_vk_hash"})

CircuitBound(p) ==
    p \in (Proofs \ {"circuit_mismatch"})

SchemaAndPublicInputsBound(p) ==
    p \in (Proofs \ {"circuit_mismatch", "wrong_vk_hash"})

StarkDomainBound(p) ==
    p \in (Proofs \ {"stark_domain_swap"})

BackendAllowed(p) ==
    p \notin {"disabled_backend", "trusted_setup_backend", "developer_only_backend"}

WithinSizeLimits(p) ==
    p # "oversized_proof"

CryptographicVerifierRan(p) ==
    p # "decode_only"

LedgerPath(p) ==
    p # "diagnostic_only"

FastpqClaimBound(p) ==
    p # "fastpq_claim_mismatch"

ZkAceReplayFresh(p) ==
    p # "zk_ace_replay"

ValidLedgerProof(p) ==
    /\ p \in Proofs
    /\ LedgerPath(p)
    /\ HasActiveVk(p)
    /\ VkHashBound(p)
    /\ CircuitBound(p)
    /\ SchemaAndPublicInputsBound(p)
    /\ StarkDomainBound(p)
    /\ BackendAllowed(p)
    /\ WithinSizeLimits(p)
    /\ CryptographicVerifierRan(p)
    /\ FastpqClaimBound(p)
    /\ ZkAceReplayFresh(p)

AdmissionAllows(p) ==
    \/ ValidLedgerProof(p)
    \/ /\ BUG_MISSING_VK_ACCEPTS
       /\ p = "missing_vk"
    \/ /\ BUG_WRONG_VK_HASH_ACCEPTS
       /\ p = "wrong_vk_hash"
    \/ /\ BUG_CIRCUIT_MISMATCH_ACCEPTS
       /\ p = "circuit_mismatch"
    \/ /\ BUG_OMIT_STARK_DOMAIN_BINDING
       /\ p = "stark_domain_swap"
    \/ /\ BUG_DISABLED_BACKEND_ACCEPTS
       /\ p = "disabled_backend"
    \/ /\ BUG_DIAGNOSTIC_ENDPOINT_PROMOTION
       /\ p = "diagnostic_only"
    \/ /\ BUG_FASTPQ_CLAIM_MISMATCH_ACCEPTS
       /\ p = "fastpq_claim_mismatch"
    \/ /\ BUG_REPLAY_ACCEPTS
       /\ p = "zk_ace_replay"

Init ==
    /\ accepted = {}
    /\ diagnostic = {}

LedgerAdmit(p) ==
    /\ p \in Proofs
    /\ AdmissionAllows(p)
    /\ accepted' = accepted \cup {p}
    /\ diagnostic' = diagnostic

DiagnosticVerify(p) ==
    /\ p = "diagnostic_only"
    /\ diagnostic' = diagnostic \cup {p}
    /\ accepted' =
        IF BUG_DIAGNOSTIC_ENDPOINT_PROMOTION
        THEN accepted \cup {p}
        ELSE accepted

Next ==
    \E p \in Proofs:
        \/ LedgerAdmit(p)
        \/ DiagnosticVerify(p)

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ accepted \subseteq Proofs
    /\ diagnostic \subseteq Proofs

LedgerBindingInvariant ==
    \A p \in accepted: ValidLedgerProof(p)

DiagnosticBoundaryInvariant ==
    "diagnostic_only" \notin accepted

ZkAceReplayInvariant ==
    "zk_ace_replay" \notin accepted

FastpqClaimInvariant ==
    "fastpq_claim_mismatch" \notin accepted

=============================================================================
