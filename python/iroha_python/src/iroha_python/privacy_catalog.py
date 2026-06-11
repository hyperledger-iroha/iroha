"""Privacy algorithm catalog metadata for the Python SDK."""

from __future__ import annotations

import copy
import json
import unicodedata
from ipaddress import ip_address
from typing import Any, Mapping
from urllib.parse import unquote, urlsplit, urlunsplit

PRIVACY_CRITERIA = (
    "hide_amount",
    "hide_sender",
    "hide_receiver",
    "hide_asset_type",
    "post_quantum",
)
_ALGORITHM_ID_CHARS = frozenset("abcdefghijklmnopqrstuvwxyz0123456789-_")
_BACKEND_FAMILY_NAME_CHARS = frozenset("abcdefghijklmnopqrstuvwxyz0123456789-")
PRODUCTION_GATE_VERSION = "privacy-production-gate-v1"
PRODUCTION_GATE_REQUIREMENTS = (
    ("real_proving", "real proving engine is not registered"),
    ("real_verification", "real verifier is not registered"),
    ("chain_admission", "chain admission path is not enabled"),
    ("sdk_parity", "cross-SDK parity is incomplete"),
    ("wallet_state", "wallet/state support is incomplete"),
    ("witness_privacy_checks", "witness privacy checks are incomplete"),
    ("deterministic_tests", "deterministic tests are incomplete"),
    ("negative_adversarial_tests", "negative/adversarial tests are incomplete"),
    ("replay_nullifier_tests", "replay/nullifier rejection tests are incomplete"),
    ("fuzzing", "fuzzing gate is incomplete"),
    ("parser_fuzzing", "parser fuzzing gate is incomplete"),
    ("verifier_fuzzing", "verifier fuzzing gate is incomplete"),
    ("performance_gates", "performance gate is incomplete"),
    ("external_audit", "internal cryptographic review signoff is missing"),
)
TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS = (
    "real_proving",
    "real_verification",
    "witness_privacy_checks",
    "verifier_fuzzing",
)
PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE = (
    "implementation stage is not production-hardened"
)
PRODUCTION_GATE_MISSING_PLANNED_SDK = "planned SDK entrypoints remain"
PRODUCTION_GATE_MISSING_DEV_FIXTURE = (
    "dev fixture entrypoints are not production entrypoints"
)
PRODUCTION_GATE_MISSING_ALLOWLIST = (
    "Iroha production allowlist is not enabled for this audited row"
)
PRODUCTION_GATE_SUPPLEMENTAL_MISSING_REASONS = (
    PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE,
    PRODUCTION_GATE_MISSING_PLANNED_SDK,
    PRODUCTION_GATE_MISSING_DEV_FIXTURE,
    PRODUCTION_GATE_MISSING_ALLOWLIST,
)
PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION = (
    "iroha-privacy-production-evidence-registry-v1"
)
PRIVACY_PRODUCTION_REVIEW_ARTIFACT_KEYS = frozenset(
    ("label", "uri", "signature")
)
PRIVACY_PRODUCTION_REVIEW_ARTIFACT_SIGNATURE_PREFIX = "ed25519:"
PRIVACY_PRODUCTION_GATE_ARTIFACT_KEYS = frozenset(("label", "uri"))
PRIVACY_PRODUCTION_RESULT_KEYS = frozenset(("passed", "artifact"))
PRIVACY_PRODUCTION_LOCALNET_ACCEPTANCE_KEYS = frozenset(
    (
        "run_id",
        "target",
        "peer_count",
        "peer_ids",
        "chain_id",
        "smoke_passed",
        "smoke_tx_hash",
        "replay_rejected",
        "replay_rejection_hash",
        "restart_persistence_checked",
        "restart_replay_rejected",
        "restart_replay_rejection_hash",
        "state_recovery_passed",
        "state_recovery_hash",
    )
)
PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES = (
    "rust_core",
    "ffi",
    "python",
    "javascript",
    "java_android",
    "kotlin",
    "swift",
    "csharp",
)
PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS = (
    "types",
    "validation_rules",
    "error_codes",
    "golden_vectors",
)
PRIVACY_PRODUCTION_EVIDENCE_ROW_KEYS = frozenset(
    (
        "version",
        "covered_algorithm_id",
        "chain_id",
        "reviewer_identity",
        "review_artifact",
        "verifier_key_id",
        "proof_family",
        "public_inputs_schema",
        "sdk_entrypoints",
        "sdk_parity_artifacts",
        "required_state",
        "fuzz_results",
        "performance_results",
        "localnet_run_id",
        "localnet_acceptance",
        "gate_evidence",
    )
)
BACKEND_FAMILY_BY_ALGORITHM_ID = {
    "transparent-transfer": "none",
    "shield": "commitment-only",
    "confidential-transfer-v2": "halo2-ipa-pasta",
    "unshield": "halo2-ipa-pasta",
    "asset-hidden-confidential-transfer-v1": "halo2-ipa-pasta",
    "zk-ace-pq-authorization-v0": "stark-fri",
    "anonymous-pgc-k-out-of-n-v1": "anonymous-pgc",
    "verange-transparent-range-v1": "verange",
    "zkat-policy-private-auth-v1": "zkat",
    "zk-ams-recursive-admission-v0": "recursive-anonymous-admission",
    "vega-existing-credential-zk-v0": "vega-existing-credential-zk",
    "silent-threshold-anoncred-v0": "silent-threshold-anoncred",
    "zk-x509-onchain-identity-v0": "zk-x509",
    "jindo-lattice-pcs-zk-v0": "lattice-pcs-sis",
    "sis-hints-anoncred-pq-v0": "sis-with-hints",
    "orchard-halo2-actions-v1": "halo2-ipa-orchard",
    "penumbra-masp-v1": "groth16-bls12-377",
    "monero-fcmp-plus-plus-v1": "fcmp-plus-plus-curve-tree",
    "miden-stark-note-v1": "miden-stark",
    "aztec-private-rollup-v1": "aztec-plonkish-private-kernel",
    "pq-masp-stark-v0": "pq-masp-stark-fri",
}
REQUIRED_PRIVACY_PLAN_ROWS = (
    ("anonymous-pgc-k-out-of-n-v1", "production-hardened", "anonymous-pgc"),
    ("verange-transparent-range-v1", "component", "verange"),
    ("zkat-policy-private-auth-v1", "production-hardened", "zkat"),
    (
        "zk-ams-recursive-admission-v0",
        "production-hardened",
        "recursive-anonymous-admission",
    ),
    ("vega-existing-credential-zk-v0", "production-hardened", "vega-existing-credential-zk"),
    ("silent-threshold-anoncred-v0", "production-hardened", "silent-threshold-anoncred"),
    ("zk-x509-onchain-identity-v0", "production-hardened", "zk-x509"),
    ("jindo-lattice-pcs-zk-v0", "production-hardened", "lattice-pcs-sis"),
    ("sis-hints-anoncred-pq-v0", "production-hardened", "sis-with-hints"),
    ("orchard-halo2-actions-v1", "production-hardened", "halo2-ipa-orchard"),
    ("penumbra-masp-v1", "production-hardened", "groth16-bls12-377"),
    (
        "monero-fcmp-plus-plus-v1",
        "production-hardened",
        "fcmp-plus-plus-curve-tree",
    ),
    ("miden-stark-note-v1", "production-hardened", "miden-stark"),
    (
        "aztec-private-rollup-v1",
        "production-hardened",
        "aztec-plonkish-private-kernel",
    ),
    ("pq-masp-stark-v0", "production-hardened", "pq-masp-stark-fri"),
)
REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("Anonymous PGC k-out-of-n payments v1", "Anonymous PGC", "Account-based anonymous confidential payment target with hidden sender, hidden amount, receiver privacy, and k-out-of-n receiver-set proofs."),
    "verange-transparent-range-v1": ("VeRange transparent range proofs v1", "VeRange", "Verification-efficient transparent range-proof component for confidential amounts, solvency proofs, and numeric credential predicates."),
    "zkat-policy-private-auth-v1": ("zkAt policy-private authorization v1", "zkAt policy auth", "Policy-private blockchain authenticator that hides threshold rules, signer sets, and account authorization logic."),
    "zk-ams-recursive-admission-v0": ("ZK-AMS recursive anonymous admission v0", "ZK-AMS admission", "Research target for recursively aggregated anonymous admission from real-world personhood or eligibility credentials into anonymous on-chain accounts."),
    "vega-existing-credential-zk-v0": ("Vega existing-credential ZK proofs v0", "Vega credentials", "Low-latency zero-knowledge proof target for proving predicates over existing credentials without revealing the full credential."),
    "silent-threshold-anoncred-v0": ("Silent threshold anonymous credentials v0", "Silent threshold cred", "Research target for threshold-issued anonymous credentials with silent setup, issuer hiding, constant-size showings, and dynamic verifier policies."),
    "zk-x509-onchain-identity-v0": ("ZK-X.509 on-chain identity v0", "ZK-X.509 identity", "ZK proof target for X.509 certificate validity, ownership, revocation status, and wallet-address binding."),
    "jindo-lattice-pcs-zk-v0": ("Jindo lattice polynomial commitment ZK v0", "Jindo lattice PCS", "2026 lattice-based polynomial commitment candidate for post-quantum zero-knowledge proof systems."),
    "sis-hints-anoncred-pq-v0": ("SIS-with-hints PQ anonymous credentials v0", "SIS hints anoncred", "PKC 2026 research foundation for lattice/SIS-with-hints anonymous credentials and post-quantum credential proofs."),
    "zk-ace-pq-authorization-v0": ("ZK-ACE post-quantum authorization v0", "ZK-ACE PQ auth", "STARK/FRI-backed source-account authorization for transparent asset transfers."),
    "orchard-halo2-actions-v1": ("Orchard-style Halo2 action bundle v1", "Orchard Halo2", "Zcash Orchard-style action bundle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions."),
    "penumbra-masp-v1": ("Penumbra-style multi-asset shielded pool v1", "Penumbra MASP", "Single multi-asset shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs for private IBC-style assets."),
    "monero-fcmp-plus-plus-v1": ("Monero FCMP++ RingCT-style transfer v1", "FCMP++", "Full-chain membership proof target that replaces small decoy rings with a full-output-set spend proof while retaining hidden amounts and one-time receivers."),
    "miden-stark-note-v1": ("Miden-style STARK private note transaction v1", "Miden STARK", "Client-side STARK-proved account transition using private notes whose data stays off-chain while note hashes/nullifiers anchor correctness."),
    "aztec-private-rollup-v1": ("Aztec-style programmable private transaction v1", "Aztec private", "Programmable private-state transaction using client-side private execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs."),
    "pq-masp-stark-v0": ("Post-quantum MASP STARK v0", "PQ MASP v0", "Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption."),
}
REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": "payment",
    "verange-transparent-range-v1": "proof_backend",
    "zkat-policy-private-auth-v1": "authorization",
    "zk-ams-recursive-admission-v0": "admission",
    "vega-existing-credential-zk-v0": "credential",
    "silent-threshold-anoncred-v0": "credential",
    "zk-x509-onchain-identity-v0": "identity",
    "jindo-lattice-pcs-zk-v0": "proof_backend",
    "sis-hints-anoncred-pq-v0": "credential",
    "zk-ace-pq-authorization-v0": "authorization",
    "orchard-halo2-actions-v1": "payment",
    "penumbra-masp-v1": "payment",
    "monero-fcmp-plus-plus-v1": "payment",
    "miden-stark-note-v1": "payment",
    "aztec-private-rollup-v1": "payment",
    "pq-masp-stark-v0": "payment",
}
REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": "accepted_conference",
    "verange-transparent-range-v1": "accepted_conference",
    "zkat-policy-private-auth-v1": "accepted_conference",
    "zk-ams-recursive-admission-v0": "arxiv_preprint",
    "vega-existing-credential-zk-v0": "technical_report",
    "silent-threshold-anoncred-v0": "technical_report",
    "zk-x509-onchain-identity-v0": "arxiv_preprint",
    "jindo-lattice-pcs-zk-v0": "technical_report",
    "sis-hints-anoncred-pq-v0": "accepted_conference",
    "zk-ace-pq-authorization-v0": "arxiv_preprint",
    "orchard-halo2-actions-v1": "specification",
    "penumbra-masp-v1": "specification",
    "monero-fcmp-plus-plus-v1": "specification",
    "miden-stark-note-v1": "specification",
    "aztec-private-rollup-v1": "specification",
    "pq-masp-stark-v0": "specification",
}
REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("account-based private payments", "multi-receiver confidential transfers", "payment privacy without a note-based shielded pool UX"),
    "verange-transparent-range-v1": ("confidential amount range proofs", "reserve or solvency proofs", "numeric credential predicates"),
    "zkat-policy-private-auth-v1": ("institutional wallet policy privacy", "hidden threshold authorization", "authorization-policy migration without revealing signer topology"),
    "zk-ams-recursive-admission-v0": ("anonymous onboarding", "Sybil-resistant wallet issuance", "credential-gated CBDC pilots"),
    "vega-existing-credential-zk-v0": ("legacy credential bridges", "private eligibility checks", "attribute predicates for wallet enrollment"),
    "silent-threshold-anoncred-v0": ("multi-authority regulated credentials", "issuer-hiding eligibility proofs", "central-bank or supervisor issued wallet credentials"),
    "zk-x509-onchain-identity-v0": ("institutional wallet identity", "legal-entity account binding", "private PKI-based eligibility checks"),
    "jindo-lattice-pcs-zk-v0": ("post-quantum proof-system research", "future PQ verifier backend evaluation", "lattice PCS benchmarking"),
    "sis-hints-anoncred-pq-v0": ("post-quantum anonymous credential research", "future PQ KYC or eligibility proofs", "assumption tracking for lattice credential designs"),
    "zk-ace-pq-authorization-v0": ("post-quantum transaction authorization migration", "identity-private source-account authorization", "authorization envelopes for transparent asset transfers"),
    "orchard-halo2-actions-v1": ("single-asset private transfers", "mature note/nullifier wallet design", "compact client proofs without Groth16 ceremonies"),
    "penumbra-masp-v1": ("multi-asset shielded pools", "IBC-style asset privacy", "asset-id hiding with typed-value notes"),
    "monero-fcmp-plus-plus-v1": ("maximal sender anonymity sets", "decoy-ring replacement research", "account-independent UTXO spend privacy"),
    "miden-stark-note-v1": ("client-side proving", "private programmable note workflows", "parallel account-local transaction execution"),
    "aztec-private-rollup-v1": ("programmable private payments", "hybrid public/private contract workflows", "wallet-side private execution with encrypted note discovery"),
    "pq-masp-stark-v0": ("end-to-end post-quantum privacy target", "long-horizon central-bank pilot research", "strict PQ proof, authorization, and note-encryption experiments"),
}
REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("hide_amount", "hide_sender", "hide_receiver"),
    "verange-transparent-range-v1": ("hide_amount",),
    "zkat-policy-private-auth-v1": (),
    "zk-ams-recursive-admission-v0": (),
    "vega-existing-credential-zk-v0": (),
    "silent-threshold-anoncred-v0": (),
    "zk-x509-onchain-identity-v0": (),
    "jindo-lattice-pcs-zk-v0": (),
    "sis-hints-anoncred-pq-v0": (),
    "zk-ace-pq-authorization-v0": (),
    "orchard-halo2-actions-v1": ("hide_amount", "hide_sender", "hide_receiver"),
    "penumbra-masp-v1": ("hide_amount", "hide_sender", "hide_receiver", "hide_asset_type"),
    "monero-fcmp-plus-plus-v1": ("hide_amount", "hide_sender", "hide_receiver"),
    "miden-stark-note-v1": ("hide_amount", "hide_receiver", "hide_asset_type"),
    "aztec-private-rollup-v1": ("hide_amount", "hide_sender", "hide_receiver"),
    "pq-masp-stark-v0": ("hide_amount", "hide_sender", "hide_receiver", "hide_asset_type", "post_quantum"),
}
REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": "anonymous-pgc-k-out-of-n",
    "verange-transparent-range-v1": "verange-transparent-range",
    "zkat-policy-private-auth-v1": "zkat-policy-private-authenticator",
    "zk-ams-recursive-admission-v0": "recursive-anonymous-admission",
    "vega-existing-credential-zk-v0": "existing-credential-zk",
    "silent-threshold-anoncred-v0": "threshold-anonymous-credentials",
    "zk-x509-onchain-identity-v0": "zkvm-x509-identity",
    "jindo-lattice-pcs-zk-v0": "lattice-polynomial-commitment",
    "sis-hints-anoncred-pq-v0": "lattice-anonymous-credentials",
    "zk-ace-pq-authorization-v0": "stark/fri/sha256-goldilocks",
    "orchard-halo2-actions-v1": "halo2-pasta-action-bundle",
    "penumbra-masp-v1": "groth16-bls12-377-decaf377",
    "monero-fcmp-plus-plus-v1": "fcmp-plus-plus-curve-trees-bulletproofs",
    "miden-stark-note-v1": "stark-vm-note-transaction",
    "aztec-private-rollup-v1": "plonkish-private-kernel-rollup",
    "pq-masp-stark-v0": "stark-fri",
}
REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (
        "anonymity_set_root,tx_digest,balance_commitments,"
        "receiver_set_commitment,receiver_ciphertext_commitments,"
        "receiver_threshold,receiver_count,link_tag,range_commitments,"
        "chain_id,domain_separator"
    ),
    "verange-transparent-range-v1": (
        "commitments,range_parameters,aggregation_count,domain_separator,"
        "payload_digest"
    ),
    "zkat-policy-private-auth-v1": (
        "policy_commitment,tx_digest,account_id,action_class,domain_separator,"
        "policy_epoch"
    ),
    "zk-ams-recursive-admission-v0": (
        "issuer_root,admission_batch_root,admission_nullifiers,"
        "anonymous_account_commitments,recursive_admission_digest,"
        "domain_separator"
    ),
    "vega-existing-credential-zk-v0": (
        "issuer_commitment,credential_schema,predicate_commitment,"
        "subject_binding,expiration_epoch,domain_separator"
    ),
    "silent-threshold-anoncred-v0": (
        "issuer_set_commitment,threshold_policy_hash,"
        "credential_showing_commitment,showing_nullifier,"
        "verifier_policy_hash,domain_separator"
    ),
    "zk-x509-onchain-identity-v0": (
        "ca_root_commitment,certificate_policy_hash,revocation_root,"
        "subject_commitment,address_binding,domain_separator"
    ),
    "jindo-lattice-pcs-zk-v0": (
        "commitment,opening_claim,query_set,parameter_hash,domain_separator"
    ),
    "sis-hints-anoncred-pq-v0": (
        "issuer_commitment,credential_commitment,showing_policy_hash,"
        "parameter_hash,domain_separator"
    ),
    "zk-ace-pq-authorization-v0": (
        "identity_commitment,tx_digest,chain_id,domain_separator,action_class,"
        "replay_nullifier,policy_hash,from,to,asset,amount,verifier_key_id"
    ),
    "orchard-halo2-actions-v1": (
        "anchor,nullifiers,cmx,value_commitments,binding_signature"
    ),
    "penumbra-masp-v1": (
        "state_commitment_anchor,nullifiers,note_commitments,"
        "balance_commitment,asset_id_commitment"
    ),
    "monero-fcmp-plus-plus-v1": (
        "membership_root,key_image_or_link_tag,amount_commitments,"
        "range_commitments,spend_authorization,chain_tag"
    ),
    "miden-stark-note-v1": (
        "account_id,initial_account_commitment,final_account_commitment,"
        "input_note_nullifiers,output_note_hashes,reference_block"
    ),
    "aztec-private-rollup-v1": (
        "note_hashes,nullifiers,encrypted_logs,public_call_requests,"
        "private_kernel_commitment,rollup_state_roots"
    ),
    "pq-masp-stark-v0": (
        "pool_id,asset_set_root,nullifier_set,output_commitments,root,"
        "chain_tag,pq_policy_hash"
    ),
}
REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": "anonymous_pgc_k_out_of_n_v1",
    "verange-transparent-range-v1": "verange_transparent_range_v1",
    "zkat-policy-private-auth-v1": "zkat_policy_private_auth_v1",
    "zk-ams-recursive-admission-v0": "zk_ams_recursive_admission_v0",
    "vega-existing-credential-zk-v0": "vega_existing_credential_zk_v0",
    "silent-threshold-anoncred-v0": "silent_threshold_anoncred_v0",
    "zk-x509-onchain-identity-v0": "zk_x509_onchain_identity_v0",
    "jindo-lattice-pcs-zk-v0": "jindo_lattice_pcs_zk_v0",
    "sis-hints-anoncred-pq-v0": "sis_hints_anoncred_pq_v0",
    "zk-ace-pq-authorization-v0": "zk_ace_pq_authorization_v0",
    "orchard-halo2-actions-v1": "orchard_halo2_action_bundle_v1",
    "penumbra-masp-v1": "penumbra_masp_v1",
    "monero-fcmp-plus-plus-v1": "monero_fcmp_plus_plus_v1",
    "miden-stark-note-v1": "miden_stark_note_v1",
    "aztec-private-rollup-v1": "aztec_private_kernel_v1",
    "pq-masp-stark-v0": "pq_masp_stark_v0",
}
REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (
        "anonymous account commitment",
        "anonymity-set roots",
        "spent link-tag",
        "range-proof",
        "wallet account blinding",
    ),
    "verange-transparent-range-v1": (
        "range-proof verifier parameters",
        "verange verifier",
        "range commitment",
        "dependent payment or credential verifier",
    ),
    "zkat-policy-private-auth-v1": (
        "policy commitment registry",
        "policy epoch state",
        "authorization replay",
        "wallet policy witness",
        "typed zk::submitzkatauthorizedtransaction",
    ),
    "zk-ams-recursive-admission-v0": (
        "issuer root registry",
        "admission nullifier set",
        "anonymous account commitment registry",
        "wallet admission witness",
        "typed zk-ams admission batch instruction",
    ),
    "vega-existing-credential-zk-v0": (
        "credential issuer registry",
        "credential schema registry",
        "revocation or expiration policy",
        "wallet credential predicate witness",
        "typed vega credential proof instruction",
    ),
    "silent-threshold-anoncred-v0": (
        "threshold issuer registry",
        "credential showing nullifier policy",
        "wallet credential showing witness",
        "anonymous credential verifier key registry",
        "typed silent-threshold credential proof instruction",
    ),
    "zk-x509-onchain-identity-v0": (
        "trusted ca root registry",
        "revocation root registry",
        "certificate subject commitment registry",
        "wallet certificate witness",
        "typed zk-x.509 identity proof instruction",
    ),
    "jindo-lattice-pcs-zk-v0": (
        "lattice pcs parameter registry",
        "backend verifier implementation",
        "lattice pcs verifier key registry",
        "dependent circuit integration",
    ),
    "sis-hints-anoncred-pq-v0": (
        "lattice credential parameter registry",
        "credential showing verifier",
        "wallet lattice credential witness",
        "lattice credential verifier key registry",
        "typed sis-with-hints credential proof instruction",
    ),
    "zk-ace-pq-authorization-v0": (
        "registered zk-ace identity commitment",
        "source-account allowlist",
        "authorization policy hash registry",
        "active zk-ace verifier key",
        "chain/domain binding state",
        "transfer digest binding",
        "replay nullifier uniqueness set",
        "identity rotation/revocation registry",
        "stark/fri verifier parameter floors",
        "wallet identity witness",
        "zk::submitzkaceauthorizedtransfer",
    ),
    "orchard-halo2-actions-v1": (
        "orchard note commitment tree",
        "orchard nullifier set",
        "orchard action-bundle verifier key registry",
        "wallet orchard witness",
        "typed orchard action-bundle instruction",
    ),
    "penumbra-masp-v1": (
        "multi-asset state commitment tree",
        "typed nullifier set",
        "groth16 spend/output verifier key registry",
        "wallet asset metadata witness",
        "typed penumbra shielded-pool transaction admission",
    ),
    "monero-fcmp-plus-plus-v1": (
        "full-output-set commitment accumulator",
        "spent link-tag set",
        "fcmp++ verifier key registry",
        "wallet output ownership scan state",
        "typed fcmp++ transfer instruction",
    ),
    "miden-stark-note-v1": (
        "private note hash database",
        "input note nullifier set",
        "account commitment state",
        "stark vm verifier key registry",
        "wallet private note witness",
    ),
    "aztec-private-rollup-v1": (
        "private note-hash tree",
        "nullifier tree",
        "encrypted log delivery store",
        "private-kernel verifier key registry",
        "wallet private execution witness",
        "typed aztec private-rollup transaction instruction",
    ),
    "pq-masp-stark-v0": (
        "pq masp asset-set commitment root",
        "pq nullifier set",
        "ml-kem encrypted note payload store",
        "wallet pq note witness",
        "active pq masp verifier key",
    ),
}
REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = (
    "malformed proof bytes",
    "wrong verifier key",
    "public input mismatch",
)
REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (
        "stale or unknown anonymity-set root",
        "duplicate link tag",
        "receiver-set substitution",
    ),
    "verange-transparent-range-v1": (
        "wrong bit length",
        "commitment substitution",
        "verifier-parameter mismatch",
    ),
    "zkat-policy-private-auth-v1": (
        "policy-root substitution",
        "stale policy epoch",
        "authorization replay",
    ),
    "zk-ams-recursive-admission-v0": (
        "duplicate credential admission",
        "wrong issuer root",
        "batch omission or account commitment substitution",
    ),
    "vega-existing-credential-zk-v0": (
        "expired credential",
        "predicate mismatch",
        "wallet-binding replay",
    ),
    "silent-threshold-anoncred-v0": (
        "insufficient issuer threshold",
        "issuer-set substitution",
        "credential showing replay",
    ),
    "zk-x509-onchain-identity-v0": (
        "expired certificate",
        "revoked certificate",
        "stale revocation root",
    ),
    "jindo-lattice-pcs-zk-v0": (
        "parameter mismatch",
        "opening claim substitution",
        "unsupported query set",
    ),
    "sis-hints-anoncred-pq-v0": (
        "wrong parameter set",
        "issuer parameter substitution",
        "credential showing replay",
    ),
    "zk-ace-pq-authorization-v0": (
        "transaction digest substitution",
        "chain-id or domain-separator mismatch",
        "replayed nullifier",
    ),
    "orchard-halo2-actions-v1": (
        "stale anchor",
        "duplicate nullifier",
        "invalid action-bundle proof",
    ),
    "penumbra-masp-v1": (
        "stale state commitment anchor",
        "duplicate nullifier",
        "asset balance commitment mismatch",
    ),
    "monero-fcmp-plus-plus-v1": (
        "stale membership root",
        "duplicate link tag",
        "amount commitment mismatch",
    ),
    "miden-stark-note-v1": (
        "stale reference block",
        "duplicate input note nullifier",
        "account commitment transition mismatch",
    ),
    "aztec-private-rollup-v1": (
        "stale rollup state root",
        "duplicate nullifier",
        "private-kernel public input mismatch",
    ),
    "pq-masp-stark-v0": (
        "stale asset-set root",
        "duplicate pq nullifier",
        "ml-dsa or ml-kem domain mismatch",
    ),
}
REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("stale or unknown anonymity-set root", "duplicate link tag", "receiver-set substitution", "range commitment mismatch", "authorization envelope mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "verange-transparent-range-v1": ("wrong bit length", "commitment substitution", "verifier-parameter mismatch", "oversized aggregation", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "zkat-policy-private-auth-v1": ("policy-root substitution", "stale policy epoch", "unauthorized signer witness", "transaction digest mismatch", "authorization replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "zk-ams-recursive-admission-v0": ("duplicate credential admission", "wrong issuer root", "batch omission or account commitment substitution", "recursive proof parameter mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "vega-existing-credential-zk-v0": ("expired credential", "wrong issuer", "predicate mismatch", "wallet-binding replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "silent-threshold-anoncred-v0": ("insufficient issuer threshold", "issuer-set substitution", "credential showing replay", "verifier-policy mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "zk-x509-onchain-identity-v0": ("expired certificate", "revoked certificate", "unknown CA root", "wrong wallet address binding", "address-binding replay", "stale revocation root", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "jindo-lattice-pcs-zk-v0": ("parameter mismatch", "opening claim substitution", "unsupported query set", "backend misclassified as production-ready", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "sis-hints-anoncred-pq-v0": ("wrong parameter set", "issuer parameter substitution", "credential showing replay", "overclaiming production readiness from assumption research", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "zk-ace-pq-authorization-v0": ("transaction digest substitution", "chain-id or domain-separator mismatch", "replayed nullifier", "revoked identity commitment", "policy hash mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "orchard-halo2-actions-v1": ("stale anchor", "duplicate nullifier", "invalid action-bundle proof", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "penumbra-masp-v1": ("stale state commitment anchor", "duplicate nullifier", "asset balance commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "monero-fcmp-plus-plus-v1": ("stale membership root", "duplicate link tag", "amount commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "miden-stark-note-v1": ("stale reference block", "duplicate input note nullifier", "account commitment transition mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "aztec-private-rollup-v1": ("stale rollup state root", "duplicate nullifier", "private-kernel public input mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    "pq-masp-stark-v0": ("stale asset-set root", "duplicate PQ nullifier", "ML-DSA or ML-KEM domain mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
}
REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "Production Anonymous PGC proof admission requires a caller-supplied proof envelope bound to the anonymity root, receiver set, link tag, range commitments, chain id, and domain separator.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "verange-transparent-range-v1": ("This range-proof backend component is not a standalone payment protocol.", "Range parameters must be bound to the transaction payload and verifier key.", "Aggregated proof limits must be enforced by validators.", "The SDK dev fixture is non-production and verifies deterministic binding only; production VeRange proving remains unavailable until the hardening gates pass.", "Wallet witnesses and private range inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "zkat-policy-private-auth-v1": ("Hides authorization policy, not payment fields.", "Policy commitments require explicit epoch, replay, and rotation semantics.", "Combining with ZK-ACE requires both proofs to bind the same transaction digest.", "Production zkAt proof admission requires canonical policy commitments, transaction digest binding, account/action/domain binding, verifier-key registration, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "zk-ams-recursive-admission-v0": ("Admission privacy is separate from later payment privacy.", "Duplicate admission prevention depends on issuer-scoped nullifiers.", "Recursive batching must bind every admitted account commitment.", "This production admission component is proof-verifiable but not a standalone payment protocol.", "Production ZK-AMS admission requires canonical issuer roots, admission-nullifier sets, anonymous account commitments, recursive admission digests, domain binding, verifier-key registration, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet admission witnesses and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "vega-existing-credential-zk-v0": ("Credential schema parsing must be deterministic and versioned.", "Proofs must bind to wallet or identity commitments to prevent credential replay.", "Issuer trust, expiration, and revocation semantics require registered policy inputs.", "This production credential component is proof-verifiable but not a standalone payment protocol.", "Production Vega credential admission requires canonical issuer commitments, credential schema, predicate commitment, subject binding, expiration epoch, domain binding, verifier-key registration, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet credential predicate witnesses and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "silent-threshold-anoncred-v0": ("Credential issuance and revocation governance are as important as proof verification.", "Issuer-set commitments require explicit threshold, rotation, and downgrade protections.", "This production credential component is proof-verifiable but not a standalone payment protocol.", "Production silent-threshold credential admission requires canonical issuer-set, threshold-policy, showing-nullifier, verifier-policy, domain binding, verifier-key registration, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet credential showing witnesses and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "zk-x509-onchain-identity-v0": ("Legacy X.509 trust roots are usually not post-quantum.", "Revocation root freshness must be explicit in the public inputs.", "Address binding must prevent proof replay across wallets and chains.", "Production ZK-X.509 identity proofs verify canonical CA-root, certificate-policy, revocation-root, subject-commitment, wallet binding, and domain-separated public inputs while remaining a reusable identity component rather than a standalone payment protocol.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "jindo-lattice-pcs-zk-v0": ("This is a production proof backend component, not a standalone transaction algorithm.", "PQ proof coverage alone does not imply PQ authorization or note encryption.", "Parameter selection and implementation security require independent review.", "Production Jindo lattice proving and verifier adapters require registered LatticePcsSis parameters, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "sis-hints-anoncred-pq-v0": ("This is a production credential proof component, not a standalone wallet protocol.", "PQ credential proof coverage does not make a payment flow end-to-end post-quantum.", "Parameter choices and reduction assumptions need explicit governance.", "Production SIS-with-hints credential proving and verifier adapters require registered SisWithHints parameters, deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographic review.", "Wallet lattice credential witnesses and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "zk-ace-pq-authorization-v0": ("Authorization is only one PQ layer; proof backend and note encryption must also be PQ before a payment flow is end-to-end post-quantum.", "Replay nullifiers must be chain-domain separated and irreversible after acceptance.", "A dev verifier must never be accepted under a production verifier key id.", "Native AIR openings are blinded so sampled rows do not recover identity or replay witness limbs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "orchard-halo2-actions-v1": ("Orchard actions require circuit-compatible note/nullifier semantics and domain-separated action hashes.", "Viewing-key and outgoing-viewing metadata must remain wallet-local.", "Production readiness requires audited Halo2 parameters and note-encryption review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "penumbra-masp-v1": ("Typed asset values must bind asset identifiers to balance commitments.", "Groth16 parameter registration must distinguish spend and output circuits.", "Wallet note plaintexts and position metadata must not be exposed through public APIs.", "Production MASP use requires audited parameter governance and chain-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "monero-fcmp-plus-plus-v1": ("Full-chain membership roots must be canonical and replay protected.", "Link tags/key images must be unique without revealing owned outputs.", "Range-proof and amount-commitment parameters require production verifier review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "miden-stark-note-v1": ("Private note data and off-chain delivery metadata must stay wallet-local.", "Account-local transition proofs must bind initial and final account commitments.", "Reference blocks must prevent replay against stale account state.", "Production Miden note transactions require audited STARK parameters and account-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "aztec-private-rollup-v1": ("Private-kernel proofs must bind note hashes, nullifiers, encrypted logs, and public calls.", "Encrypted log delivery metadata must not leak wallet note ownership.", "Recursive verifier registration must distinguish private-kernel versions and rollup state roots.", "Production private-rollup use requires audited private-kernel parameters and rollup-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
    "pq-masp-stark-v0": ("PQ MASP combines experimental STARK/FRI proving with production PQ authorization and note encryption requirements.", "ML-DSA domains and ML-KEM ciphertext formats must be bound to verifier keys and pool identifiers.", "Post-quantum readiness still requires parameter review, parser fuzzing, and internal cryptographic review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."),
}
REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (
        (
            "Anonymous PGC with k-out-of-n Proofs",
            "https://eprint.iacr.org/2025/884",
        ),
    ),
    "verange-transparent-range-v1": (
        (
            "VeRange: Verification-efficient Zero-knowledge Range Arguments",
            "https://eprint.iacr.org/2025/528",
        ),
    ),
    "zkat-policy-private-auth-v1": (
        (
            "zkAt: Zero-Knowledge Authenticator for Blockchain",
            "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
        ),
    ),
    "zk-ams-recursive-admission-v0": (
        (
            "ZK-AMS recursive anonymous admission",
            "https://arxiv.org/abs/2602.16130",
        ),
    ),
    "vega-existing-credential-zk-v0": (
        (
            "Vega: Low-Latency Zero-Knowledge Proofs over Existing Credentials",
            "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
        ),
    ),
    "silent-threshold-anoncred-v0": (
        (
            "Anonymous Credentials with Issuer-Hiding, Threshold Issuance, and Silent Setup",
            "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html",
        ),
    ),
    "zk-x509-onchain-identity-v0": (
        ("ZK-X.509 on-chain identity", "https://arxiv.org/abs/2603.25190"),
    ),
    "jindo-lattice-pcs-zk-v0": (
        (
            "Jindo lattice-based polynomial commitment",
            "https://eprint.iacr.org.cn/2026/044",
        ),
    ),
    "sis-hints-anoncred-pq-v0": (
        (
            "Tight Reductions for SIS-with-Hints Assumptions with Applications",
            "https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-with-hints-assumptions-with-applications/",
        ),
    ),
    "zk-ace-pq-authorization-v0": (
        (
            "ZK-ACE: Practical Post-Quantum Authorization for Blockchain",
            "https://arxiv.org/abs/2603.07974",
        ),
    ),
    "orchard-halo2-actions-v1": (
        ("ZIP 224 Orchard Shielded Protocol", "https://zips.z.cash/zip-0224"),
        (
            "Zcash Protocol Specification",
            "https://zips.z.cash/protocol/protocol.pdf",
        ),
    ),
    "penumbra-masp-v1": (
        (
            "Penumbra Multi-Asset Shielded Pool",
            "https://protocol.penumbra.zone/main/shielded_pool.html",
        ),
        (
            "Penumbra Cryptographic Primitives",
            "https://protocol.penumbra.zone/main/crypto.html",
        ),
    ),
    "monero-fcmp-plus-plus-v1": (
        (
            "Monero FCMP++ Development",
            "https://web.getmonero.org/2024/04/27/fcmps.html",
        ),
    ),
    "miden-stark-note-v1": (
        (
            "Miden Transaction Model",
            "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
        ),
        ("Miden Notes", "https://docs.miden.xyz/core-concepts/miden-base/note/"),
    ),
    "aztec-private-rollup-v1": (
        (
            "Aztec State Management",
            "https://docs.aztec.network/developers/docs/foundational-topics/state_management",
        ),
        (
            "Aztec Private Kernel Circuit",
            "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
        ),
    ),
    "pq-masp-stark-v0": (
        (
            "NIST Post-Quantum Standards",
            "https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards",
        ),
        ("FIPS 203 ML-KEM", "https://csrc.nist.gov/pubs/fips/203/final"),
        ("FIPS 204 ML-DSA", "https://csrc.nist.gov/pubs/fips/204/final"),
        ("FIPS 205 SLH-DSA", "https://csrc.nist.gov/pubs/fips/205/final"),
    ),
}
REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (
        "buildAnonymousPgcReceiverSet",
        "buildAnonymousPgcAccountCommitmentInstruction",
        "buildAnonymousPgcKOutOfNProofV1",
        "buildAnonymousPgcTransferInstruction",
    ),
    "verange-transparent-range-v1": ("buildRangeCommitment", "buildVeRangeDevProofFixture", "buildVeRangeProofEnvelope", "verifyVeRangeProofLocally"),
    "zkat-policy-private-auth-v1": ("buildZkAtPolicyCommitment", "buildZkAtAuthenticatorEnvelope", "buildZkAtPolicyProofV1", "verifyZkAtPolicyProofV1"),
    "zk-ams-recursive-admission-v0": ("buildZkAmsAdmissionBatch", "buildZkAmsAdmissionProofEnvelope", "buildZkAmsAdmissionBatchProofV0", "verifyZkAmsAdmissionBatchProofV0"),
    "vega-existing-credential-zk-v0": ("buildVegaCredentialPredicateCommitment", "buildVegaCredentialProofEnvelope", "buildVegaCredentialPredicateProofV0", "verifyVegaCredentialPredicateProofV0"),
    "silent-threshold-anoncred-v0": ("buildSilentThresholdCredentialCommitments", "buildSilentThresholdCredentialEnvelope", "buildSilentThresholdCredentialShowingProofV0", "verifySilentThresholdCredentialShowingProofV0"),
    "zk-x509-onchain-identity-v0": ("buildZkX509IdentityCommitments", "buildZkX509IdentityEnvelope", "buildZkX509IdentityProofV0", "verifyZkX509IdentityProofV0"),
    "jindo-lattice-pcs-zk-v0": ("buildJindoLatticePublicInputs", "buildJindoLatticeProofEnvelope", "buildJindoLatticeProofV0", "verifyJindoPolynomialCommitmentV0"),
    "sis-hints-anoncred-pq-v0": ("buildSisHintsCredentialCommitments", "buildSisHintsCredentialEnvelope", "buildSisHintsAnonymousCredentialProofV0", "verifySisHintsAnonymousCredentialProofV0"),
    "zk-ace-pq-authorization-v0": ("buildRegisterZkAceIdentityCommitmentInstruction", "buildRotateZkAceIdentityCommitmentInstruction", "buildRevokeZkAceIdentityCommitmentInstruction", "buildZkAceAuthorizedTransferInstruction", "buildZkAceAuthorizationProofV1"),
    "orchard-halo2-actions-v1": (
        "buildOrchardActionBundleProofV1",
        "buildOrchardActionBundleInstruction",
    ),
    "penumbra-masp-v1": (
        "buildPenumbraSpendProofV1",
        "buildPenumbraOutputProofV1",
        "buildPenumbraShieldedPoolTransaction",
    ),
    "monero-fcmp-plus-plus-v1": (
        "buildFcmpPlusPlusMembershipProofV1",
        "buildFcmpPlusPlusTransferInstruction",
    ),
    "miden-stark-note-v1": (
        "buildMidenStarkTransactionProofV1",
        "buildMidenNoteTransactionInstruction",
    ),
    "aztec-private-rollup-v1": (
        "buildAztecPrivateKernelProofV1",
        "buildAztecPrivateRollupTransactionInstruction",
    ),
    "pq-masp-stark-v0": (
        "buildPqMaspStarkTransferProofV0",
        "buildPqMaspStarkRegisterPoolInstruction",
        "buildPqMaspStarkTransferInstruction",
        "generateMlDsaKeyPair",
        "encapsulateMlKem",
    ),
}
REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": (),
    "verange-transparent-range-v1": ("buildVeRangeProofV1",),
    "zkat-policy-private-auth-v1": (),
    "zk-ams-recursive-admission-v0": (),
    "vega-existing-credential-zk-v0": (),
    "silent-threshold-anoncred-v0": (),
    "zk-x509-onchain-identity-v0": (),
    "jindo-lattice-pcs-zk-v0": (),
    "sis-hints-anoncred-pq-v0": (),
    "zk-ace-pq-authorization-v0": (),
    "orchard-halo2-actions-v1": (),
    "penumbra-masp-v1": (),
    "monero-fcmp-plus-plus-v1": (),
    "miden-stark-note-v1": (),
    "aztec-private-rollup-v1": (),
    "pq-masp-stark-v0": (),
}
REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "verange-transparent-range-v1": {"proof": True, "authorization": False, "note_encryption": False},
    "zkat-policy-private-auth-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "zk-ams-recursive-admission-v0": {"proof": False, "authorization": False, "note_encryption": False},
    "vega-existing-credential-zk-v0": {"proof": False, "authorization": False, "note_encryption": False},
    "silent-threshold-anoncred-v0": {"proof": False, "authorization": False, "note_encryption": False},
    "zk-x509-onchain-identity-v0": {"proof": False, "authorization": False, "note_encryption": False},
    "jindo-lattice-pcs-zk-v0": {"proof": True, "authorization": False, "note_encryption": False},
    "sis-hints-anoncred-pq-v0": {"proof": True, "authorization": False, "note_encryption": False},
    "zk-ace-pq-authorization-v0": {"proof": True, "authorization": True, "note_encryption": False},
    "orchard-halo2-actions-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "penumbra-masp-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "monero-fcmp-plus-plus-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "miden-stark-note-v1": {"proof": True, "authorization": False, "note_encryption": False},
    "aztec-private-rollup-v1": {"proof": False, "authorization": False, "note_encryption": False},
    "pq-masp-stark-v0": {"proof": True, "authorization": True, "note_encryption": True},
}
REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("anonymous account commitment accumulator", "spent link-tag set", "Anonymous PGC verifier", "range-proof component verifier", "typed zk::RegisterAnonymousPgcAccountCommitment instruction", "typed zk::SubmitAnonymousPgcTransfer instruction"),
    "verange-transparent-range-v1": ("VeRange verifier registry entry", "range commitment binding rules", "dependent payment or credential verifier"),
    "zkat-policy-private-auth-v1": ("zkAt policy commitment registry", "zkAt verifier", "account policy epoch state", "account policy replay protection", "typed zk::RegisterZkAtPolicyCommitment instruction", "typed zk::SubmitZkAtAuthorizedTransaction admission"),
    "zk-ams-recursive-admission-v0": ("issuer root registry", "admission nullifier set", "recursive admission verifier", "typed ZK-AMS admission batch instruction"),
    "vega-existing-credential-zk-v0": ("credential schema registry", "issuer registry", "credential predicate verifier", "typed Vega credential proof instruction"),
    "silent-threshold-anoncred-v0": ("threshold issuer registry", "anonymous credential verifier", "credential showing replay policy", "typed silent-threshold credential proof instruction"),
    "zk-x509-onchain-identity-v0": ("trusted CA root registry", "revocation root registry", "ZK-X.509 verifier", "typed ZK-X.509 identity proof instruction"),
    "jindo-lattice-pcs-zk-v0": ("Jindo verifier backend", "lattice PCS parameter registry", "dependent circuit integration"),
    "sis-hints-anoncred-pq-v0": ("lattice anonymous credential verifier", "credential parameter registry", "issuer parameter registry", "typed SIS-with-hints credential proof instruction"),
    "zk-ace-pq-authorization-v0": ("zk::RegisterZkAceIdentityCommitment", "zk::RotateZkAceIdentityCommitment", "zk::RevokeZkAceIdentityCommitment", "zk::SubmitZkAceAuthorizedTransfer", "active stark/fri/sha256-goldilocks ZK-ACE verifier key", "ZK-ACE identity source-account allowlist"),
    "orchard-halo2-actions-v1": ("Orchard note commitment tree", "Orchard nullifier set", "Halo2 action-bundle verifier", "wallet Orchard witness store", "typed Orchard action-bundle instruction"),
    "penumbra-masp-v1": ("multi-asset state commitment tree", "typed note commitment and nullifier state", "Groth16 verifier registry", "wallet multi-asset witness store", "typed Penumbra shielded-pool transaction admission"),
    "monero-fcmp-plus-plus-v1": ("full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier", "wallet scanning and ownership recovery", "typed FCMP++ transfer instruction"),
    "miden-stark-note-v1": ("STARK VM verifier", "private note hash and nullifier database", "account commitment state", "wallet private-note delivery store", "typed Miden note transaction instruction"),
    "aztec-private-rollup-v1": ("private note-hash tree", "nullifier tree", "encrypted log store", "private-kernel verifier", "wallet private execution environment", "typed Aztec private-rollup transaction instruction"),
    "pq-masp-stark-v0": ("STARK/FRI verifier enabled", "ML-DSA transaction authorization", "ML-KEM note payload encryption", "zk::RegisterAssetHiddenZkPool", "zk::AssetHiddenZkTransfer", "active PQ MASP verifier key"),
}
REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("anonymous account commitment set", "recent anonymity-set roots", "spent link-tag set", "range-proof verifier parameters", "wallet account blinding and receiver recovery metadata"),
    "verange-transparent-range-v1": ("range-proof verifier parameters", "VeRange verifier key registry", "range commitment domain separators", "maximum aggregation policy"),
    "zkat-policy-private-auth-v1": ("policy commitment registry", "policy epoch state", "authorization replay guard", "authorization verifier registry", "wallet policy witness store"),
    "zk-ams-recursive-admission-v0": ("issuer root registry", "admission nullifier set", "anonymous account commitment registry", "recursive verifier parameters", "recursive admission verifier key registry", "wallet admission witness store"),
    "vega-existing-credential-zk-v0": ("credential issuer registry", "supported credential schema registry", "predicate registry", "revocation or expiration policy", "wallet credential predicate witness store", "credential predicate commitment registry", "credential predicate verifier key registry"),
    "silent-threshold-anoncred-v0": ("threshold issuer registry", "credential parameter registry", "verifier policy registry", "credential showing nullifier policy", "wallet credential showing witness store", "credential showing commitment registry", "anonymous credential verifier key registry"),
    "zk-x509-onchain-identity-v0": ("trusted CA root registry", "certificate policy registry", "revocation root registry", "identity proof verifier", "wallet certificate witness store", "certificate subject commitment registry", "ZK-X.509 verifier key registry"),
    "jindo-lattice-pcs-zk-v0": ("lattice PCS parameter registry", "backend verifier implementation", "lattice PCS verifier key registry", "production benchmark vectors"),
    "sis-hints-anoncred-pq-v0": ("lattice credential parameter registry", "issuer parameter registry", "credential showing verifier", "wallet lattice credential witness store", "lattice credential commitment registry", "lattice credential verifier key registry"),
    "zk-ace-pq-authorization-v0": ("registered ZK-ACE identity commitment", "source-account allowlist", "authorization policy hash registry", "active ZK-ACE verifier key", "chain/domain binding state", "transfer digest binding", "replay nullifier uniqueness set", "identity rotation/revocation registry", "STARK/FRI verifier parameter floors", "wallet identity witness and replay-secret store"),
    "orchard-halo2-actions-v1": ("Orchard note commitment tree", "Orchard nullifier set", "Orchard action-bundle verifier key registry", "wallet Orchard witness store"),
    "penumbra-masp-v1": ("multi-asset state commitment tree", "typed nullifier set", "Groth16 spend/output verifier key registry", "wallet asset metadata witness store"),
    "monero-fcmp-plus-plus-v1": ("full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier key registry", "wallet output ownership scan state"),
    "miden-stark-note-v1": ("private note hash database", "input note nullifier set", "account commitment state", "STARK VM verifier key registry", "wallet private note witness store"),
    "aztec-private-rollup-v1": ("private note-hash tree", "nullifier tree", "encrypted log delivery store", "private-kernel verifier key registry", "wallet private execution witness store"),
    "pq-masp-stark-v0": ("PQ MASP asset-set commitment root", "PQ nullifier set", "ML-KEM encrypted note payload store", "wallet PQ note witness store"),
}
REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("Register anonymous account commitments and anonymity-set accumulator state.", "Register the k-out-of-n payment verifier key and range-proof parameters.", "Persist wallet blinding, balance-opening, and receiver recovery witnesses."),
    "verange-transparent-range-v1": ("Register VeRange verifier parameters and allowed bit lengths.", "Define the commitment scheme and domain separators used by dependent algorithms."),
    "zkat-policy-private-auth-v1": ("Register a hidden policy commitment and verifier key.", "Bind the policy to account action classes and epoch rules."),
    "zk-ams-recursive-admission-v0": ("Register credential issuer roots and recursive verifier parameters.", "Define anonymous account commitment format and admission-nullifier derivation."),
    "vega-existing-credential-zk-v0": ("Register supported credential schemas, issuers, and predicates.", "Bind credential proof subjects to wallet or ZK-ACE identity commitments."),
    "silent-threshold-anoncred-v0": ("Register issuer sets, threshold policies, and credential parameters.", "Define showing-nullifier and verifier-policy binding rules."),
    "zk-x509-onchain-identity-v0": ("Register trusted CA roots, certificate policies, and revocation-root feeds.", "Define wallet address binding and domain-separation rules."),
    "jindo-lattice-pcs-zk-v0": ("Track lattice PCS parameter sets and verifier API shape.", "Benchmark prover, verifier, and proof-size behavior before integration."),
    "sis-hints-anoncred-pq-v0": ("Track supported SIS-with-hints parameter sets and issuer parameters.", "Define how future PQ credential showings bind to wallet or authorization contexts."),
    "zk-ace-pq-authorization-v0": ("Register a ZK-ACE identity commitment, source-account allowlist, and verifier key.", "Initialize replay-state tracking for the authorizing wallet.", "Bind authorization policy hash to the allowed transaction action classes."),
    "orchard-halo2-actions-v1": ("Add Orchard-compatible note, nullifier, action, and anchor data model types.", "Register Orchard Halo2 verifier parameters and action-bundle public input layout.", "Persist wallet note plaintexts, diversifiers, Merkle witnesses, and outgoing viewing data."),
    "penumbra-masp-v1": ("Add typed-value notes, asset identifiers, state commitments, and nullifier state.", "Register Groth16/BLS12-377 verifier parameters for spend and output proofs.", "Persist wallet note plaintexts, asset metadata, state commitment positions, and nullifier keys."),
    "monero-fcmp-plus-plus-v1": ("Add output commitment accumulator state suitable for full-chain membership proofs.", "Define link tags/key images and spent-output rejection for Iroha assets.", "Implement wallet scanning, ownership recovery, and amount commitment witness storage."),
    "miden-stark-note-v1": ("Add private note hash/nullifier state and account-local transition verification.", "Register a STARK VM verifier and public-input commitment layout.", "Persist private note data and off-chain delivery metadata in the wallet note store."),
    "aztec-private-rollup-v1": ("Add private note-hash and nullifier trees plus encrypted log delivery metadata.", "Register a private-kernel verifier and public-input layout for private contract side effects.", "Persist wallet PXE-style note discovery, private call witnesses, and app-scoped nullifier keys."),
    "pq-masp-stark-v0": ("Register STARK/FRI verifier parameters and PQ MASP public input layout.", "Define ML-DSA authorization domains and ML-KEM note-encryption payload formats.", "Persist wallet PQ note witnesses, nullifier positions, and encapsulation metadata."),
}
REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = {
    "anonymous-pgc-k-out-of-n-v1": ("Select an anonymity-set root and receiver set.", "Create balance commitments, receiver ciphertext commitments, and link tag.", "Generate the Anonymous PGC proof and submit the transfer instruction."),
    "verange-transparent-range-v1": ("Build amount commitments.", "Generate a range proof bound to the transaction payload.", "Attach the range-proof envelope to the dependent confidential algorithm."),
    "zkat-policy-private-auth-v1": ("Generate a policy-private authenticator proof.", "Attach the authenticator envelope to the transaction authorization path."),
    "zk-ams-recursive-admission-v0": ("Collect admitted account commitments into a batch.", "Generate or import a recursive admission proof.", "Submit the batch proof and admission nullifiers."),
    "vega-existing-credential-zk-v0": ("Parse the credential under a registered schema.", "Generate a predicate proof and bind it to the wallet context.", "Submit the proof envelope to the admission or authorization flow."),
    "silent-threshold-anoncred-v0": ("Generate a credential showing proof under the verifier policy.", "Submit the proof as an admission or authorization component."),
    "zk-x509-onchain-identity-v0": ("Generate a proof of certificate validity, ownership, and revocation status.", "Bind the proof to an institution wallet or ZK-ACE identity commitment."),
    "jindo-lattice-pcs-zk-v0": ("Use as a candidate backend for future PQ circuits only after concrete circuit integration.",),
    "sis-hints-anoncred-pq-v0": ("Use as a future PQ credential backend after a concrete credential protocol is selected.",),
    "zk-ace-pq-authorization-v0": ("Hash the transaction payload and chain/domain context.", "Derive a fresh replay nullifier.", "Generate a ZK-ACE authorization proof and submit a protected transparent transfer."),
    "orchard-halo2-actions-v1": ("Select spend notes and anchors from the wallet witness store.", "Create output notes and value commitments.", "Generate one Halo2 proof over the action bundle and submit nullifiers plus commitments."),
    "penumbra-masp-v1": ("Select positioned notes and derive nullifiers.", "Create typed output notes and balance commitments.", "Submit spend/output actions with proofs against the shielded pool state commitment tree."),
    "monero-fcmp-plus-plus-v1": ("Select owned outputs from the wallet scan state.", "Generate full-chain membership and amount-conservation proofs.", "Submit link tag, output commitments, range proof, and spend authorization."),
    "miden-stark-note-v1": ("Execute the account-local transition against private note witnesses.", "Produce a STARK proof for the transaction script and account state delta.", "Submit note nullifiers, output note hashes, account commitments, and proof."),
    "aztec-private-rollup-v1": ("Execute private contract calls locally against wallet notes.", "Accumulate note hashes, nullifiers, encrypted logs, and public-call requests in the private kernel.", "Submit the recursive private-kernel proof and side-effect commitments for validator verification."),
    "pq-masp-stark-v0": ("Select PQ MASP input notes and derive nullifiers.", "Generate STARK/FRI transfer proofs with ML-DSA authorization and ML-KEM output-note encryption.", "Submit nullifiers, output commitments, PQ policy hash, and proof for verifier admission."),
}

_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON = (
    "[{\"id\":\"transparent-transfer\",\"name\":\"Transparent asset transfer\",\"shortName\":\"Transpare"
    "nt\",\"summary\":\"Public Iroha asset transfer used as the size and latency baseline.\",\"cate"
    "gory\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[],\"proofFamily\":\"none\",\"pu"
    "blicInputsSchema\":null,\"verifierKeyId\":null,\"pqLayers\":{\"proof\":false,\"authorization\":fa"
    "lse,\"noteEncryption\":false},\"implementationStage\":null,\"recommendedFor\":[],\"sourceRefere"
    "nces\":[],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":[],\"setupSteps\":[],\"execut"
    "ionSteps\":[],\"sdkEntrypoints\":[\"buildTransferAssetInstruction\",\"buildTransaction\",\"submi"
    "tSignedTransaction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"Transfer::Asset\"]}"
    ",{\"id\":\"shield\",\"name\":\"Shield into confidential note\",\"shortName\":\"Shield\",\"summary\":\"D"
    "ebits public balance and appends an encrypted receiver note commitment.\",\"category\":\"pay"
    "ment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_receiver\"],\"proofFamily\":\"comm"
    "itment-only\",\"publicInputsSchema\":\"asset,from,amount,note_commitment\",\"verifierKeyId\":\"z"
    "k::Shield\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"impl"
    "ementationStage\":null,\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requ"
    "iredState\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"b"
    "uildShieldInstruction\",\"buildTransaction\",\"submitSignedTransaction\"],\"plannedSdkEntrypoi"
    "nts\":[],\"chainRequirements\":[\"zk::RegisterZkAsset\",\"zk::Shield\"]},{\"id\":\"confidential-tr"
    "ansfer-v2\",\"name\":\"Confidential transfer v2\",\"shortName\":\"Confidential v2\",\"summary\":\"Ha"
    "lo2/Pasta note-to-note transfer that hides amount, sender note, and receiver note while "
    "publishing the asset id.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriter"
    "ia\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":\"halo2-ipa-pasta\",\"publi"
    "cInputsSchema\":\"input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_com"
    "mitment_0,output_commitment_1,root,asset_tag,chain_tag\",\"verifierKeyId\":\"confidential_tr"
    "ansfer_v2\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"impl"
    "ementationStage\":null,\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requ"
    "iredState\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"b"
    "uildConfidentialTransferProofV2\",\"buildZkTransferInstruction\"],\"plannedSdkEntrypoints\":["
    "],\"chainRequirements\":[\"zk::ZkTransfer\",\"active confidential transfer verifier key\",\"wal"
    "let note witness store\"]},{\"id\":\"unshield\",\"name\":\"Unshield to public balance\",\"shortNam"
    "e\":\"Unshield\",\"summary\":\"Spends a private note into a public receiver balance; the priva"
    "te source note remains hidden.\",\"category\":\"payment\",\"maturity\":\"specification\",\"covered"
    "Criteria\":[\"hide_sender\"],\"proofFamily\":\"halo2-ipa-pasta\",\"publicInputsSchema\":\"input_co"
    "mmitment_0,input_commitment_1,nullifier_0,nullifier_1,change_commitment_0,root,public_am"
    "ount,asset_tag,chain_tag\",\"verifierKeyId\":\"confidential_unshield_v3\",\"pqLayers\":{\"proof\""
    ":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":null,\"recomme"
    "ndedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":["
    "],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildConfidentialUnshieldProofV"
    "3\",\"buildUnshieldInstruction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"zk::Unsh"
    "ield\",\"active confidential unshield verifier key\",\"wallet note witness store\"]},{\"id\":\"a"
    "sset-hidden-confidential-transfer-v1\",\"name\":\"Asset-hidden MASP transfer v1\",\"shortName\""
    ":\"MASP v1\",\"summary\":\"Target multi-asset shielded-pool transfer that hides amount, sende"
    "r note, receiver note, and exact asset inside a pool.\",\"category\":\"payment\",\"maturity\":\""
    "specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\",\"hide_asse"
    "t_type\"],\"proofFamily\":\"halo2-ipa-pasta\",\"publicInputsSchema\":\"pool_id,asset_set_root,in"
    "put_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_commitment_0,output_c"
    "ommitment_1,root,chain_tag\",\"verifierKeyId\":\"asset_hidden_transfer_v1\",\"pqLayers\":{\"proo"
    "f\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"validator-"
    "scaffold-as-of-2026-05\",\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"re"
    "quiredState\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":["
    "\"buildConfidentialAssetHiddenTransferProofV1\",\"buildRegisterAssetHiddenZkPoolInstruction"
    "\",\"buildAssetHiddenZkTransferInstruction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements"
    "\":[\"zk::RegisterAssetHiddenZkPool\",\"zk::AssetHiddenZkTransfer\",\"asset-hidden pool verifi"
    "er registry state\",\"pool note witness store\"]},{\"id\":\"zk-ace-pq-authorization-v0\",\"name\""
    ":\"ZK-ACE post-quantum authorization v0\",\"shortName\":\"ZK-ACE PQ auth\",\"summary\":\"STARK/FR"
    "I-backed source-account authorization for transparent asset transfers.\",\"category\":\"auth"
    "orization\",\"maturity\":\"arxiv_preprint\",\"coveredCriteria\":[],\"proofFamily\":\"stark/fri/sha"
    "256-goldilocks\",\"publicInputsSchema\":\"identity_commitment,tx_digest,chain_id,domain_sepa"
    "rator,action_class,replay_nullifier,policy_hash,from,to,asset,amount,verifier_key_id\",\"v"
    "erifierKeyId\":\"zk_ace_pq_authorization_v0\",\"pqLayers\":{\"proof\":true,\"authorization\":true"
    ",\"noteEncryption\":false},\"implementationStage\":\"chain-executable\",\"recommendedFor\":[\"pos"
    "t-quantum transaction authorization migration\",\"identity-private source-account authoriz"
    "ation\",\"authorization envelopes for transparent asset transfers\"],\"sourceReferences\":[{\""
    "label\":\"ZK-ACE: Practical Post-Quantum Authorization for Blockchain\",\"url\":\"https://arxi"
    "v.org/abs/2603.07974\"}],\"securityNotes\":[\"Authorization is only one PQ layer; proof back"
    "end and note encryption must also be PQ before a payment flow is end-to-end post-quantum"
    ".\",\"Replay nullifiers must be chain-domain separated and irreversible after acceptance.\""
    ",\"A dev verifier must never be accepted under a production verifier key id.\",\"Native AIR"
    " openings are blinded so sampled rows do not recover identity or replay witness limbs.\","
    "\"Any chain roots, nullifiers, revocation data, or replay guards for this flow must persi"
    "st across node restarts before admitting ledger mutations.\",\"Wallet witness material and"
    " private inputs must stay local and must not be exposed through SDK or chain APIs.\",\"Pro"
    "duction hardening requires deterministic vectors, negative/adversarial test cases, repla"
    "y/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cr"
    "yptographic review.\"],\"requiredState\":[\"registered ZK-ACE identity commitment\",\"source-a"
    "ccount allowlist\",\"authorization policy hash registry\",\"active ZK-ACE verifier key\",\"cha"
    "in/domain binding state\",\"transfer digest binding\",\"replay nullifier uniqueness set\",\"id"
    "entity rotation/revocation registry\",\"STARK/FRI verifier parameter floors\",\"wallet ident"
    "ity witness and replay-secret store\"],\"failureModes\":[\"transaction digest substitution\","
    "\"chain-id or domain-separator mismatch\",\"replayed nullifier\",\"revoked identity commitmen"
    "t\",\"policy hash mismatch\",\"malformed proof bytes\",\"wrong verifier key\",\"public input mis"
    "match\"],\"setupSteps\":[\"Register a ZK-ACE identity commitment, source-account allowlist, "
    "and verifier key.\",\"Initialize replay-state tracking for the authorizing wallet.\",\"Bind "
    "authorization policy hash to the allowed transaction action classes.\"],\"executionSteps\":"
    "[\"Hash the transaction payload and chain/domain context.\",\"Derive a fresh replay nullifi"
    "er.\",\"Generate a ZK-ACE authorization proof and submit a protected transparent transfer."
    "\"],\"sdkEntrypoints\":[\"buildRegisterZkAceIdentityCommitmentInstruction\",\"buildRotateZkAce"
    "IdentityCommitmentInstruction\",\"buildRevokeZkAceIdentityCommitmentInstruction\",\"buildZkA"
    "ceAuthorizedTransferInstruction\",\"buildZkAceAuthorizationProofV1\"],\"plannedSdkEntrypoint"
    "s\":[],\"chainRequirements\":[\"zk::RegisterZkAceIdentityCommitment\",\"zk::RotateZkAceIdentit"
    "yCommitment\",\"zk::RevokeZkAceIdentityCommitment\",\"zk::SubmitZkAceAuthorizedTransfer\",\"ac"
    "tive stark/fri/sha256-goldilocks ZK-ACE verifier key\",\"ZK-ACE identity source-account al"
    "lowlist\"]},{\"id\":\"anonymous-pgc-k-out-of-n-v1\",\"name\":\"Anonymous PGC k-out-of-n payments"
    " v1\",\"shortName\":\"Anonymous PGC\",\"summary\":\"Account-based anonymous confidential payment"
    " target with hidden sender, hidden amount, receiver privacy, and k-out-of-n receiver-set"
    " proofs.\",\"category\":\"payment\",\"maturity\":\"accepted_conference\",\"coveredCriteria\":[\"hide"
    "_amount\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":\"anonymous-pgc-k-out-of-n\",\"public"
    "InputsSchema\":\"anonymity_set_root,tx_digest,balance_commitments,receiver_set_commitment,"
    "receiver_ciphertext_commitments,receiver_threshold,receiver_count,link_tag,range_commitm"
    "ents,chain_id,domain_separator\",\"verifierKeyId\":\"anonymous_pgc_k_out_of_n_v1\",\"pqLayers\""
    ":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"sdk"
    "-builder\",\"recommendedFor\":[\"account-based private payments\",\"multi-receiver confidentia"
    "l transfers\",\"payment privacy without a note-based shielded pool UX\"],\"sourceReferences\""
    ":[{\"label\":\"Anonymous PGC with k-out-of-n Proofs\",\"url\":\"https://eprint.iacr.org/2025/88"
    "4\"}],\"securityNotes\":[\"Requires fresh anonymity-set roots and replay/link-tag state.\",\"A"
    "mount privacy depends on the range-proof component and commitment binding.\",\"Receiver ci"
    "phertext commitments must bind to the same transaction digest as the proof.\",\"The SDK de"
    "v fixture verifies deterministic binding only; chain execution and production Anonymous "
    "PGC proofs remain unavailable.\",\"Wallet witness material and private inputs must stay lo"
    "cal and must not be exposed through SDK or chain APIs.\",\"Production hardening requires d"
    "eterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests,"
    " parser/verifier fuzzing, performance gates, and internal cryptographic review.\"],\"requi"
    "redState\":[\"anonymous account commitment set\",\"recent anonymity-set roots\",\"spent link-t"
    "ag set\",\"range-proof verifier parameters\",\"wallet account blinding and receiver recovery"
    " metadata\"],\"failureModes\":[\"stale or unknown anonymity-set root\",\"duplicate link tag\",\""
    "receiver-set substitution\",\"range commitment mismatch\",\"authorization envelope mismatch\""
    ",\"malformed proof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Re"
    "gister anonymous account commitments and anonymity-set accumulator state.\",\"Register the"
    " k-out-of-n payment verifier key and range-proof parameters.\",\"Persist wallet blinding, "
    "balance-opening, and receiver recovery witnesses.\"],\"executionSteps\":[\"Select an anonymi"
    "ty-set root and receiver set.\",\"Create balance commitments, receiver ciphertext commitme"
    "nts, and link tag.\",\"Generate the Anonymous PGC proof and submit the transfer instructio"
    "n.\"],\"sdkEntrypoints\":[\"buildAnonymousPgcReceiverSet\",\"buildAnonymousPgcDevProofFixture\""
    ",\"verifyAnonymousPgcDevProofLocally\"],\"plannedSdkEntrypoints\":[\"buildAnonymousPgcAccount"
    "CommitmentInstruction\",\"buildAnonymousPgcKOutOfNProofV1\",\"buildAnonymousPgcTransferInstr"
    "uction\"],\"chainRequirements\":[\"anonymous account commitment accumulator\",\"spent link-tag"
    " set\",\"Anonymous PGC verifier\",\"range-proof component verifier\",\"typed zk::RegisterAnony"
    "mousPgcAccountCommitment instruction\",\"typed zk::SubmitAnonymousPgcTransfer instruction\""
    "]},{\"id\":\"verange-transparent-range-v1\",\"name\":\"VeRange transparent range proofs v1\",\"sh"
    "ortName\":\"VeRange\",\"summary\":\"Verification-efficient transparent range-proof component f"
    "or confidential amounts, solvency proofs, and numeric credential predicates.\",\"category\""
    ":\"proof_backend\",\"maturity\":\"accepted_conference\",\"coveredCriteria\":[\"hide_amount\"],\"pro"
    "ofFamily\":\"verange-transparent-range\",\"publicInputsSchema\":\"commitments,range_parameters"
    ",aggregation_count,domain_separator,payload_digest\",\"verifierKeyId\":\"verange_transparent"
    "_range_v1\",\"pqLayers\":{\"proof\":true,\"authorization\":false,\"noteEncryption\":false},\"imple"
    "mentationStage\":\"component\",\"recommendedFor\":[\"confidential amount range proofs\",\"reserv"
    "e or solvency proofs\",\"numeric credential predicates\"],\"sourceReferences\":[{\"label\":\"VeR"
    "ange: Verification-efficient Zero-knowledge Range Arguments\",\"url\":\"https://eprint.iacr."
    "org/2025/528\"}],\"securityNotes\":[\"This range-proof backend component is not a standalone"
    " payment protocol.\",\"Range parameters must be bound to the transaction payload and verif"
    "ier key.\",\"Aggregated proof limits must be enforced by validators.\",\"The SDK dev fixture"
    " is non-production and verifies deterministic binding only; production VeRange proving r"
    "emains unavailable until the hardening gates pass.\",\"Wallet witnesses and private range "
    "inputs must stay local and must not be exposed through SDK or chain APIs.\",\"Production h"
    "ardening requires deterministic vectors, negative/adversarial test cases, replay/nullifi"
    "er rejection tests, parser/verifier fuzzing, performance gates, and internal cryptograph"
    "ic review.\"],\"requiredState\":[\"range-proof verifier parameters\",\"VeRange verifier key re"
    "gistry\",\"range commitment domain separators\",\"maximum aggregation policy\"],\"failureModes"
    "\":[\"wrong bit length\",\"commitment substitution\",\"verifier-parameter mismatch\",\"oversized"
    " aggregation\",\"malformed proof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"set"
    "upSteps\":[\"Register VeRange verifier parameters and allowed bit lengths.\",\"Define the co"
    "mmitment scheme and domain separators used by dependent algorithms.\"],\"executionSteps\":["
    "\"Build amount commitments.\",\"Generate a range proof bound to the transaction payload.\",\""
    "Attach the range-proof envelope to the dependent confidential algorithm.\"],\"sdkEntrypoin"
    "ts\":[\"buildRangeCommitment\",\"buildVeRangeDevProofFixture\",\"buildVeRangeProofEnvelope\",\"v"
    "erifyVeRangeProofLocally\"],\"plannedSdkEntrypoints\":[\"buildVeRangeProofV1\"],\"chainRequire"
    "ments\":[\"VeRange verifier registry entry\",\"range commitment binding rules\",\"dependent pa"
    "yment or credential verifier\"]},{\"id\":\"zkat-policy-private-auth-v1\",\"name\":\"zkAt policy-"
    "private authorization v1\",\"shortName\":\"zkAt policy auth\",\"summary\":\"Policy-private block"
    "chain authenticator that hides threshold rules, signer sets, and account authorization l"
    "ogic.\",\"category\":\"authorization\",\"maturity\":\"accepted_conference\",\"coveredCriteria\":[],"
    "\"proofFamily\":\"zkat-policy-private-authenticator\",\"publicInputsSchema\":\"policy_commitmen"
    "t,tx_digest,account_id,action_class,domain_separator,policy_epoch\",\"verifierKeyId\":\"zkat"
    "_policy_private_auth_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption"
    "\":false},\"implementationStage\":\"production-hardened\",\"recommendedFor\":[\"institutional wa"
    "llet policy privacy\",\"hidden threshold authorization\",\"authorization-policy migration wi"
    "thout revealing signer topology\"],\"sourceReferences\":[{\"label\":\"zkAt: Zero-Knowledge Aut"
    "henticator for Blockchain\",\"url\":\"https://drops.dagstuhl.de/entities/document/10.4230/LI"
    "PIcs.AFT.2025.2\"}],\"securityNotes\":[\"Hides authorization policy, not payment fields.\",\"P"
    "olicy commitments require explicit epoch, replay, and rotation semantics.\",\"Combining wi"
    "th ZK-ACE requires both proofs to bind the same transaction digest.\",\"Production zkAt pr"
    "oof admission requires canonical policy commitments, transaction digest binding, account"
    "/action/domain binding, verifier-key registration, deterministic vectors, parser/verifie"
    "r fuzzing, performance gates, and internal cryptographic review.\",\"Any chain roots, null"
    "ifiers, revocation data, or replay guards for this flow must persist across node restart"
    "s before admitting ledger mutations.\",\"Wallet witness material and private inputs must s"
    "tay local and must not be exposed through SDK or chain APIs.\",\"Production hardening requ"
    "ires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection "
    "tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.\"],"
    "\"requiredState\":[\"policy commitment registry\",\"policy epoch state\",\"authorization replay"
    " guard\",\"authorization verifier registry\",\"wallet policy witness store\"],\"failureModes\":"
    "[\"policy-root substitution\",\"stale policy epoch\",\"unauthorized signer witness\",\"transact"
    "ion digest mismatch\",\"authorization replay\",\"malformed proof bytes\",\"wrong verifier key\""
    ",\"public input mismatch\"],\"setupSteps\":[\"Register a hidden policy commitment and verifie"
    "r key.\",\"Bind the policy to account action classes and epoch rules.\"],\"executionSteps\":["
    "\"Generate a policy-private authenticator proof.\",\"Attach the authenticator envelope to t"
    "he transaction authorization path.\"],\"sdkEntrypoints\":[\"buildZkAtPolicyCommitment\",\"buil"
    "dZkAtAuthenticatorEnvelope\",\"buildZkAtPolicyProofV1\",\"verifyZkAtPolicyProofV1\"],\"planned"
    "SdkEntrypoints\":[],\"chainRequirements\":[\"zkAt policy commitment registry\",\"zkAt verifier"
    "\",\"account policy epoch state\",\"account policy replay protection\",\"typed zk::RegisterZkA"
    "tPolicyCommitment instruction\",\"typed zk::SubmitZkAtAuthorizedTransaction admission\"]},{"
    "\"id\":\"zk-ams-recursive-admission-v0\",\"name\":\"ZK-AMS recursive anonymous admission v0\",\"s"
    "hortName\":\"ZK-AMS admission\",\"summary\":\"Research target for recursively aggregated anony"
    "mous admission from real-world personhood or eligibility credentials into anonymous on-c"
    "hain accounts.\",\"category\":\"admission\",\"maturity\":\"arxiv_preprint\",\"coveredCriteria\":[],"
    "\"proofFamily\":\"recursive-anonymous-admission\",\"publicInputsSchema\":\"issuer_root,admissio"
    "n_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_admission_dige"
    "st,domain_separator\",\"verifierKeyId\":\"zk_ams_recursive_admission_v0\",\"pqLayers\":{\"proof\""
    ":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"production-h"
    "ardened\",\"recommendedFor\":[\"anonymous onboarding\",\"Sybil-resistant wallet issuance\",\"cre"
    "dential-gated CBDC pilots\"],\"sourceReferences\":[{\"label\":\"ZK-AMS recursive anonymous adm"
    "ission\",\"url\":\"https://arxiv.org/abs/2602.16130\"}],\"securityNotes\":[\"Admission privacy i"
    "s separate from later payment privacy.\",\"Duplicate admission prevention depends on issue"
    "r-scoped nullifiers.\",\"Recursive batching must bind every admitted account commitment.\","
    "\"This production admission component is proof-verifiable but not a standalone payment pr"
    "otocol.\",\"Production ZK-AMS admission requires canonical issuer roots, admission-nullifi"
    "er sets, anonymous account commitments, recursive admission digests, domain binding, ver"
    "ifier-key registration, deterministic vectors, parser/verifier fuzzing, performance gate"
    "s, and internal cryptographic review.\",\"Any chain roots, nullifiers, revocation data, or"
    " replay guards for this flow must persist across node restarts before admitting ledger m"
    "utations.\",\"Wallet admission witnesses and private inputs must stay local and must not b"
    "e exposed through SDK or chain APIs.\",\"Production hardening requires deterministic vecto"
    "rs, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier f"
    "uzzing, performance gates, and internal cryptographic review.\"],\"requiredState\":[\"issuer"
    " root registry\",\"admission nullifier set\",\"anonymous account commitment registry\",\"recur"
    "sive verifier parameters\",\"recursive admission verifier key registry\",\"wallet admission "
    "witness store\"],\"failureModes\":[\"duplicate credential admission\",\"wrong issuer root\",\"ba"
    "tch omission or account commitment substitution\",\"recursive proof parameter mismatch\",\"m"
    "alformed proof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Regis"
    "ter credential issuer roots and recursive verifier parameters.\",\"Define anonymous accoun"
    "t commitment format and admission-nullifier derivation.\"],\"executionSteps\":[\"Collect adm"
    "itted account commitments into a batch.\",\"Generate or import a recursive admission proof"
    ".\",\"Submit the batch proof and admission nullifiers.\"],\"sdkEntrypoints\":[\"buildZkAmsAdmi"
    "ssionBatch\",\"buildZkAmsAdmissionProofEnvelope\",\"buildZkAmsAdmissionBatchProofV0\",\"verify"
    "ZkAmsAdmissionBatchProofV0\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"issuer roo"
    "t registry\",\"admission nullifier set\",\"recursive admission verifier\",\"typed ZK-AMS admis"
    "sion batch instruction\"]},{\"id\":\"vega-existing-credential-zk-v0\",\"name\":\"Vega existing-c"
    "redential ZK proofs v0\",\"shortName\":\"Vega credentials\",\"summary\":\"Low-latency zero-knowl"
    "edge proof target for proving predicates over existing credentials without revealing the"
    " full credential.\",\"category\":\"credential\",\"maturity\":\"technical_report\",\"coveredCriteri"
    "a\":[],\"proofFamily\":\"existing-credential-zk\",\"publicInputsSchema\":\"issuer_commitment,cre"
    "dential_schema,predicate_commitment,subject_binding,expiration_epoch,domain_separator\",\""
    "verifierKeyId\":\"vega_existing_credential_zk_v0\",\"pqLayers\":{\"proof\":false,\"authorization"
    "\":false,\"noteEncryption\":false},\"implementationStage\":\"production-hardened\",\"recommended"
    "For\":[\"legacy credential bridges\",\"private eligibility checks\",\"attribute predicates for"
    " wallet enrollment\"],\"sourceReferences\":[{\"label\":\"Vega: Low-Latency Zero-Knowledge Proo"
    "fs over Existing Credentials\",\"url\":\"https://www.microsoft.com/en-us/research/publicatio"
    "n/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/\"}],\"securityNotes\":["
    "\"Credential schema parsing must be deterministic and versioned.\",\"Proofs must bind to wa"
    "llet or identity commitments to prevent credential replay.\",\"Issuer trust, expiration, a"
    "nd revocation semantics require registered policy inputs.\",\"This production credential c"
    "omponent is proof-verifiable but not a standalone payment protocol.\",\"Production Vega cr"
    "edential admission requires canonical issuer commitments, credential schema, predicate c"
    "ommitment, subject binding, expiration epoch, domain binding, verifier-key registration,"
    " deterministic vectors, parser/verifier fuzzing, performance gates, and internal cryptog"
    "raphic review.\",\"Any chain roots, nullifiers, revocation data, or replay guards for this"
    " flow must persist across node restarts before admitting ledger mutations.\",\"Wallet cred"
    "ential predicate witnesses and private inputs must stay local and must not be exposed th"
    "rough SDK or chain APIs.\",\"Production hardening requires deterministic vectors, negative"
    "/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, perf"
    "ormance gates, and internal cryptographic review.\"],\"requiredState\":[\"credential issuer "
    "registry\",\"supported credential schema registry\",\"predicate registry\",\"revocation or exp"
    "iration policy\",\"wallet credential predicate witness store\",\"credential predicate commit"
    "ment registry\",\"credential predicate verifier key registry\"],\"failureModes\":[\"expired cr"
    "edential\",\"wrong issuer\",\"predicate mismatch\",\"wallet-binding replay\",\"malformed proof b"
    "ytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Register supported cr"
    "edential schemas, issuers, and predicates.\",\"Bind credential proof subjects to wallet or"
    " ZK-ACE identity commitments.\"],\"executionSteps\":[\"Parse the credential under a register"
    "ed schema.\",\"Generate a predicate proof and bind it to the wallet context.\",\"Submit the "
    "proof envelope to the admission or authorization flow.\"],\"sdkEntrypoints\":[\"buildVegaCre"
    "dentialPredicateCommitment\",\"buildVegaCredentialProofEnvelope\",\"buildVegaCredentialPredi"
    "cateProofV0\",\"verifyVegaCredentialPredicateProofV0\"],\"plannedSdkEntrypoints\":[],\"chainRe"
    "quirements\":[\"credential schema registry\",\"issuer registry\",\"credential predicate verifi"
    "er\",\"typed Vega credential proof instruction\"]},{\"id\":\"silent-threshold-anoncred-v0\",\"na"
    "me\":\"Silent threshold anonymous credentials v0\",\"shortName\":\"Silent threshold cred\",\"sum"
    "mary\":\"Research target for threshold-issued anonymous credentials with silent setup, iss"
    "uer hiding, constant-size showings, and dynamic verifier policies.\",\"category\":\"credenti"
    "al\",\"maturity\":\"technical_report\",\"coveredCriteria\":[],\"proofFamily\":\"threshold-anonymou"
    "s-credentials\",\"publicInputsSchema\":\"issuer_set_commitment,threshold_policy_hash,credent"
    "ial_showing_commitment,showing_nullifier,verifier_policy_hash,domain_separator\",\"verifie"
    "rKeyId\":\"silent_threshold_anoncred_v0\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\""
    "noteEncryption\":false},\"implementationStage\":\"production-hardened\",\"recommendedFor\":[\"mu"
    "lti-authority regulated credentials\",\"issuer-hiding eligibility proofs\",\"central-bank or"
    " supervisor issued wallet credentials\"],\"sourceReferences\":[{\"label\":\"Anonymous Credenti"
    "als with Issuer-Hiding, Threshold Issuance, and Silent Setup\",\"url\":\"https://www2.eecs.b"
    "erkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html\"}],\"securityNotes\":[\"Credential issuan"
    "ce and revocation governance are as important as proof verification.\",\"Issuer-set commit"
    "ments require explicit threshold, rotation, and downgrade protections.\",\"This production"
    " credential component is proof-verifiable but not a standalone payment protocol.\",\"Produ"
    "ction silent-threshold credential admission requires canonical issuer-set, threshold-pol"
    "icy, showing-nullifier, verifier-policy, domain binding, verifier-key registration, dete"
    "rministic vectors, parser/verifier fuzzing, performance gates, and internal cryptographi"
    "c review.\",\"Any chain roots, nullifiers, revocation data, or replay guards for this flow"
    " must persist across node restarts before admitting ledger mutations.\",\"Wallet credentia"
    "l showing witnesses and private inputs must stay local and must not be exposed through S"
    "DK or chain APIs.\",\"Production hardening requires deterministic vectors, negative/advers"
    "arial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance"
    " gates, and internal cryptographic review.\"],\"requiredState\":[\"threshold issuer registry"
    "\",\"credential parameter registry\",\"verifier policy registry\",\"credential showing nullifi"
    "er policy\",\"wallet credential showing witness store\",\"credential showing commitment regi"
    "stry\",\"anonymous credential verifier key registry\"],\"failureModes\":[\"insufficient issuer"
    " threshold\",\"issuer-set substitution\",\"credential showing replay\",\"verifier-policy misma"
    "tch\",\"malformed proof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":"
    "[\"Register issuer sets, threshold policies, and credential parameters.\",\"Define showing-"
    "nullifier and verifier-policy binding rules.\"],\"executionSteps\":[\"Generate a credential "
    "showing proof under the verifier policy.\",\"Submit the proof as an admission or authoriza"
    "tion component.\"],\"sdkEntrypoints\":[\"buildSilentThresholdCredentialCommitments\",\"buildSi"
    "lentThresholdCredentialEnvelope\",\"buildSilentThresholdCredentialShowingProofV0\",\"verifyS"
    "ilentThresholdCredentialShowingProofV0\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":"
    "[\"threshold issuer registry\",\"anonymous credential verifier\",\"credential showing replay "
    "policy\",\"typed silent-threshold credential proof instruction\"]},{\"id\":\"zk-x509-onchain-i"
    "dentity-v0\",\"name\":\"ZK-X.509 on-chain identity v0\",\"shortName\":\"ZK-X.509 identity\",\"summ"
    "ary\":\"ZK proof target for X.509 certificate validity, ownership, revocation status, and "
    "wallet-address binding.\",\"category\":\"identity\",\"maturity\":\"arxiv_preprint\",\"coveredCrite"
    "ria\":[],\"proofFamily\":\"zkvm-x509-identity\",\"publicInputsSchema\":\"ca_root_commitment,cert"
    "ificate_policy_hash,revocation_root,subject_commitment,address_binding,domain_separator\""
    ",\"verifierKeyId\":\"zk_x509_onchain_identity_v0\",\"pqLayers\":{\"proof\":false,\"authorization\""
    ":false,\"noteEncryption\":false},\"implementationStage\":\"production-hardened\",\"recommendedF"
    "or\":[\"institutional wallet identity\",\"legal-entity account binding\",\"private PKI-based e"
    "ligibility checks\"],\"sourceReferences\":[{\"label\":\"ZK-X.509 on-chain identity\",\"url\":\"htt"
    "ps://arxiv.org/abs/2603.25190\"}],\"securityNotes\":[\"Legacy X.509 trust roots are usually "
    "not post-quantum.\",\"Revocation root freshness must be explicit in the public inputs.\",\"A"
    "ddress binding must prevent proof replay across wallets and chains.\",\"Production ZK-X.50"
    "9 identity proofs verify canonical CA-root, certificate-policy, revocation-root, subject"
    "-commitment, wallet binding, and domain-separated public inputs while remaining a reusab"
    "le identity component rather than a standalone payment protocol.\",\"Any chain roots, null"
    "ifiers, revocation data, or replay guards for this flow must persist across node restart"
    "s before admitting ledger mutations.\",\"Wallet witness material and private inputs must s"
    "tay local and must not be exposed through SDK or chain APIs.\",\"Production hardening requ"
    "ires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection "
    "tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.\"],"
    "\"requiredState\":[\"trusted CA root registry\",\"certificate policy registry\",\"revocation ro"
    "ot registry\",\"identity proof verifier\",\"wallet certificate witness store\",\"certificate s"
    "ubject commitment registry\",\"ZK-X.509 verifier key registry\"],\"failureModes\":[\"expired c"
    "ertificate\",\"revoked certificate\",\"unknown CA root\",\"wrong wallet address binding\",\"addr"
    "ess-binding replay\",\"stale revocation root\",\"malformed proof bytes\",\"wrong verifier key\""
    ",\"public input mismatch\"],\"setupSteps\":[\"Register trusted CA roots, certificate policies"
    ", and revocation-root feeds.\",\"Define wallet address binding and domain-separation rules"
    ".\"],\"executionSteps\":[\"Generate a proof of certificate validity, ownership, and revocati"
    "on status.\",\"Bind the proof to an institution wallet or ZK-ACE identity commitment.\"],\"s"
    "dkEntrypoints\":[\"buildZkX509IdentityCommitments\",\"buildZkX509IdentityEnvelope\",\"buildZkX"
    "509IdentityProofV0\",\"verifyZkX509IdentityProofV0\"],\"plannedSdkEntrypoints\":[],\"chainRequ"
    "irements\":[\"trusted CA root registry\",\"revocation root registry\",\"ZK-X.509 verifier\",\"ty"
    "ped ZK-X.509 identity proof instruction\"]},{\"id\":\"jindo-lattice-pcs-zk-v0\",\"name\":\"Jindo"
    " lattice polynomial commitment ZK v0\",\"shortName\":\"Jindo lattice PCS\",\"summary\":\"2026 la"
    "ttice-based polynomial commitment candidate for post-quantum zero-knowledge proof system"
    "s.\",\"category\":\"proof_backend\",\"maturity\":\"technical_report\",\"coveredCriteria\":[],\"proof"
    "Family\":\"lattice-polynomial-commitment\",\"publicInputsSchema\":\"commitment,opening_claim,q"
    "uery_set,parameter_hash,domain_separator\",\"verifierKeyId\":\"jindo_lattice_pcs_zk_v0\",\"pqL"
    "ayers\":{\"proof\":true,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\""
    ":\"production-hardened\",\"recommendedFor\":[\"post-quantum proof-system research\",\"future PQ"
    " verifier backend evaluation\",\"lattice PCS benchmarking\"],\"sourceReferences\":[{\"label\":\""
    "Jindo lattice-based polynomial commitment\",\"url\":\"https://eprint.iacr.org.cn/2026/044\"}]"
    ",\"securityNotes\":[\"This is a production proof backend component, not a standalone transa"
    "ction algorithm.\",\"PQ proof coverage alone does not imply PQ authorization or note encry"
    "ption.\",\"Parameter selection and implementation security require independent review.\",\"P"
    "roduction Jindo lattice proving and verifier adapters require registered LatticePcsSis p"
    "arameters, deterministic vectors, parser/verifier fuzzing, performance gates, and intern"
    "al cryptographic review.\",\"Production hardening requires deterministic vectors, negative"
    "/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, perf"
    "ormance gates, and internal cryptographic review.\"],\"requiredState\":[\"lattice PCS parame"
    "ter registry\",\"backend verifier implementation\",\"lattice PCS verifier key registry\",\"pro"
    "duction benchmark vectors\"],\"failureModes\":[\"parameter mismatch\",\"opening claim substitu"
    "tion\",\"unsupported query set\",\"backend misclassified as production-ready\",\"malformed pro"
    "of bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Track lattice PCS"
    " parameter sets and verifier API shape.\",\"Benchmark prover, verifier, and proof-size beh"
    "avior before integration.\"],\"executionSteps\":[\"Use as a candidate backend for future PQ "
    "circuits only after concrete circuit integration.\"],\"sdkEntrypoints\":[\"buildJindoLattice"
    "PublicInputs\",\"buildJindoLatticeProofEnvelope\",\"buildJindoLatticeProofV0\",\"verifyJindoPo"
    "lynomialCommitmentV0\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"Jindo verifier b"
    "ackend\",\"lattice PCS parameter registry\",\"dependent circuit integration\"]},{\"id\":\"sis-hi"
    "nts-anoncred-pq-v0\",\"name\":\"SIS-with-hints PQ anonymous credentials v0\",\"shortName\":\"SIS"
    " hints anoncred\",\"summary\":\"PKC 2026 research foundation for lattice/SIS-with-hints anon"
    "ymous credentials and post-quantum credential proofs.\",\"category\":\"credential\",\"maturity"
    "\":\"accepted_conference\",\"coveredCriteria\":[],\"proofFamily\":\"lattice-anonymous-credential"
    "s\",\"publicInputsSchema\":\"issuer_commitment,credential_commitment,showing_policy_hash,par"
    "ameter_hash,domain_separator\",\"verifierKeyId\":\"sis_hints_anoncred_pq_v0\",\"pqLayers\":{\"pr"
    "oof\":true,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"productio"
    "n-hardened\",\"recommendedFor\":[\"post-quantum anonymous credential research\",\"future PQ KY"
    "C or eligibility proofs\",\"assumption tracking for lattice credential designs\"],\"sourceRe"
    "ferences\":[{\"label\":\"Tight Reductions for SIS-with-Hints Assumptions with Applications\","
    "\"url\":\"https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-with-hi"
    "nts-assumptions-with-applications/\"}],\"securityNotes\":[\"This is a production credential "
    "proof component, not a standalone wallet protocol.\",\"PQ credential proof coverage does n"
    "ot make a payment flow end-to-end post-quantum.\",\"Parameter choices and reduction assump"
    "tions need explicit governance.\",\"Production SIS-with-hints credential proving and verif"
    "ier adapters require registered SisWithHints parameters, deterministic vectors, parser/v"
    "erifier fuzzing, performance gates, and internal cryptographic review.\",\"Wallet lattice "
    "credential witnesses and private inputs must stay local and must not be exposed through "
    "SDK or chain APIs.\",\"Production hardening requires deterministic vectors, negative/adver"
    "sarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performanc"
    "e gates, and internal cryptographic review.\"],\"requiredState\":[\"lattice credential param"
    "eter registry\",\"issuer parameter registry\",\"credential showing verifier\",\"wallet lattice"
    " credential witness store\",\"lattice credential commitment registry\",\"lattice credential "
    "verifier key registry\"],\"failureModes\":[\"wrong parameter set\",\"issuer parameter substitu"
    "tion\",\"credential showing replay\",\"overclaiming production readiness from assumption res"
    "earch\",\"malformed proof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps"
    "\":[\"Track supported SIS-with-hints parameter sets and issuer parameters.\",\"Define how fu"
    "ture PQ credential showings bind to wallet or authorization contexts.\"],\"executionSteps\""
    ":[\"Use as a future PQ credential backend after a concrete credential protocol is selecte"
    "d.\"],\"sdkEntrypoints\":[\"buildSisHintsCredentialCommitments\",\"buildSisHintsCredentialEnve"
    "lope\",\"buildSisHintsAnonymousCredentialProofV0\",\"verifySisHintsAnonymousCredentialProofV"
    "0\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"lattice anonymous credential verifi"
    "er\",\"credential parameter registry\",\"issuer parameter registry\",\"typed SIS-with-hints cr"
    "edential proof instruction\"]},{\"id\":\"orchard-halo2-actions-v1\",\"name\":\"Orchard-style Hal"
    "o2 action bundle v1\",\"shortName\":\"Orchard Halo2\",\"summary\":\"Zcash Orchard-style action b"
    "undle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/outpu"
    "t actions.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amo"
    "unt\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":\"halo2-pasta-action-bundle\",\"publicInp"
    "utsSchema\":\"anchor,nullifiers,cmx,value_commitments,binding_signature\",\"verifierKeyId\":\""
    "orchard_halo2_action_bundle_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEnc"
    "ryption\":false},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":["
    "\"single-asset private transfers\",\"mature note/nullifier wallet design\",\"compact client p"
    "roofs without Groth16 ceremonies\"],\"sourceReferences\":[{\"label\":\"ZIP 224 Orchard Shielde"
    "d Protocol\",\"url\":\"https://zips.z.cash/zip-0224\"},{\"label\":\"Zcash Protocol Specification"
    "\",\"url\":\"https://zips.z.cash/protocol/protocol.pdf\"}],\"securityNotes\":[\"Orchard actions "
    "require circuit-compatible note/nullifier semantics and domain-separated action hashes.\""
    ",\"Viewing-key and outgoing-viewing metadata must remain wallet-local.\",\"Production readi"
    "ness requires audited Halo2 parameters and note-encryption review.\",\"Production hardenin"
    "g requires deterministic vectors, negative/adversarial test cases, replay/nullifier reje"
    "ction tests, parser/verifier fuzzing, performance gates, and internal cryptographic revi"
    "ew.\"],\"requiredState\":[\"Orchard note commitment tree\",\"Orchard nullifier set\",\"Orchard a"
    "ction-bundle verifier key registry\",\"wallet Orchard witness store\"],\"failureModes\":[\"sta"
    "le anchor\",\"duplicate nullifier\",\"invalid action-bundle proof\",\"malformed proof bytes\",\""
    "wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Add Orchard-compatible note,"
    " nullifier, action, and anchor data model types.\",\"Register Orchard Halo2 verifier param"
    "eters and action-bundle public input layout.\",\"Persist wallet note plaintexts, diversifi"
    "ers, Merkle witnesses, and outgoing viewing data.\"],\"executionSteps\":[\"Select spend note"
    "s and anchors from the wallet witness store.\",\"Create output notes and value commitments"
    ".\",\"Generate one Halo2 proof over the action bundle and submit nullifiers plus commitmen"
    "ts.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":[\"buildOrchardActionBundleProofV1\",\"bu"
    "ildOrchardActionBundleInstruction\"],\"chainRequirements\":[\"Orchard note commitment tree\","
    "\"Orchard nullifier set\",\"Halo2 action-bundle verifier\",\"wallet Orchard witness store\",\"t"
    "yped Orchard action-bundle instruction\"]},{\"id\":\"penumbra-masp-v1\",\"name\":\"Penumbra-styl"
    "e multi-asset shielded pool v1\",\"shortName\":\"Penumbra MASP\",\"summary\":\"Single multi-asse"
    "t shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs"
    " for private IBC-style assets.\",\"category\":\"payment\",\"maturity\":\"specification\",\"covered"
    "Criteria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\",\"hide_asset_type\"],\"proofFamily\":"
    "\"groth16-bls12-377-decaf377\",\"publicInputsSchema\":\"state_commitment_anchor,nullifiers,no"
    "te_commitments,balance_commitment,asset_id_commitment\",\"verifierKeyId\":\"penumbra_masp_v1"
    "\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementatio"
    "nStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"multi-asset shielded pools\",\""
    "IBC-style asset privacy\",\"asset-id hiding with typed-value notes\"],\"sourceReferences\":[{"
    "\"label\":\"Penumbra Multi-Asset Shielded Pool\",\"url\":\"https://protocol.penumbra.zone/main/"
    "shielded_pool.html\"},{\"label\":\"Penumbra Cryptographic Primitives\",\"url\":\"https://protoco"
    "l.penumbra.zone/main/crypto.html\"}],\"securityNotes\":[\"Typed asset values must bind asset"
    " identifiers to balance commitments.\",\"Groth16 parameter registration must distinguish s"
    "pend and output circuits.\",\"Wallet note plaintexts and position metadata must not be exp"
    "osed through public APIs.\",\"Production MASP use requires audited parameter governance an"
    "d chain-state integration review.\",\"Production hardening requires deterministic vectors,"
    " negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzz"
    "ing, performance gates, and internal cryptographic review.\"],\"requiredState\":[\"multi-ass"
    "et state commitment tree\",\"typed nullifier set\",\"Groth16 spend/output verifier key regis"
    "try\",\"wallet asset metadata witness store\"],\"failureModes\":[\"stale state commitment anch"
    "or\",\"duplicate nullifier\",\"asset balance commitment mismatch\",\"malformed proof bytes\",\"w"
    "rong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Add typed-value notes, asset "
    "identifiers, state commitments, and nullifier state.\",\"Register Groth16/BLS12-377 verifi"
    "er parameters for spend and output proofs.\",\"Persist wallet note plaintexts, asset metad"
    "ata, state commitment positions, and nullifier keys.\"],\"executionSteps\":[\"Select positio"
    "ned notes and derive nullifiers.\",\"Create typed output notes and balance commitments.\",\""
    "Submit spend/output actions with proofs against the shielded pool state commitment tree."
    "\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":[\"buildPenumbraSpendProofV1\",\"buildPenumb"
    "raOutputProofV1\",\"buildPenumbraShieldedPoolTransaction\"],\"chainRequirements\":[\"multi-ass"
    "et state commitment tree\",\"typed note commitment and nullifier state\",\"Groth16 verifier "
    "registry\",\"wallet multi-asset witness store\",\"typed Penumbra shielded-pool transaction a"
    "dmission\"]},{\"id\":\"monero-fcmp-plus-plus-v1\",\"name\":\"Monero FCMP++ RingCT-style transfer"
    " v1\",\"shortName\":\"FCMP++\",\"summary\":\"Full-chain membership proof target that replaces sm"
    "all decoy rings with a full-output-set spend proof while retaining hidden amounts and on"
    "e-time receivers.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"h"
    "ide_amount\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":\"fcmp-plus-plus-curve-trees-bul"
    "letproofs\",\"publicInputsSchema\":\"membership_root,key_image_or_link_tag,amount_commitment"
    "s,range_commitments,spend_authorization,chain_tag\",\"verifierKeyId\":\"monero_fcmp_plus_plu"
    "s_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implement"
    "ationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"maximal sender anonymity "
    "sets\",\"decoy-ring replacement research\",\"account-independent UTXO spend privacy\"],\"sourc"
    "eReferences\":[{\"label\":\"Monero FCMP++ Development\",\"url\":\"https://web.getmonero.org/2024"
    "/04/27/fcmps.html\"}],\"securityNotes\":[\"Full-chain membership roots must be canonical and"
    " replay protected.\",\"Link tags/key images must be unique without revealing owned outputs"
    ".\",\"Range-proof and amount-commitment parameters require production verifier review.\",\"W"
    "allet witness material and private inputs must stay local and must not be exposed throug"
    "h SDK or chain APIs.\",\"Production hardening requires deterministic vectors, negative/adv"
    "ersarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performa"
    "nce gates, and internal cryptographic review.\"],\"requiredState\":[\"full-output-set commit"
    "ment accumulator\",\"spent link-tag set\",\"FCMP++ verifier key registry\",\"wallet output own"
    "ership scan state\"],\"failureModes\":[\"stale membership root\",\"duplicate link tag\",\"amount"
    " commitment mismatch\",\"malformed proof bytes\",\"wrong verifier key\",\"public input mismatc"
    "h\"],\"setupSteps\":[\"Add output commitment accumulator state suitable for full-chain membe"
    "rship proofs.\",\"Define link tags/key images and spent-output rejection for Iroha assets."
    "\",\"Implement wallet scanning, ownership recovery, and amount commitment witness storage."
    "\"],\"executionSteps\":[\"Select owned outputs from the wallet scan state.\",\"Generate full-c"
    "hain membership and amount-conservation proofs.\",\"Submit link tag, output commitments, r"
    "ange proof, and spend authorization.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":[\"bui"
    "ldFcmpPlusPlusMembershipProofV1\",\"buildFcmpPlusPlusTransferInstruction\"],\"chainRequireme"
    "nts\":[\"full-output-set commitment accumulator\",\"spent link-tag set\",\"FCMP++ verifier\",\"w"
    "allet scanning and ownership recovery\",\"typed FCMP++ transfer instruction\"]},{\"id\":\"mide"
    "n-stark-note-v1\",\"name\":\"Miden-style STARK private note transaction v1\",\"shortName\":\"Mid"
    "en STARK\",\"summary\":\"Client-side STARK-proved account transition using private notes who"
    "se data stays off-chain while note hashes/nullifiers anchor correctness.\",\"category\":\"pa"
    "yment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_receiver\",\"hide"
    "_asset_type\"],\"proofFamily\":\"stark-vm-note-transaction\",\"publicInputsSchema\":\"account_id"
    ",initial_account_commitment,final_account_commitment,input_note_nullifiers,output_note_h"
    "ashes,reference_block\",\"verifierKeyId\":\"miden_stark_note_v1\",\"pqLayers\":{\"proof\":true,\"a"
    "uthorization\":false,\"noteEncryption\":false},\"implementationStage\":\"research-target-as-of"
    "-2026-05\",\"recommendedFor\":[\"client-side proving\",\"private programmable note workflows\","
    "\"parallel account-local transaction execution\"],\"sourceReferences\":[{\"label\":\"Miden Tran"
    "saction Model\",\"url\":\"https://docs.miden.xyz/core-concepts/miden-base/transaction/\"},{\"l"
    "abel\":\"Miden Notes\",\"url\":\"https://docs.miden.xyz/core-concepts/miden-base/note/\"}],\"sec"
    "urityNotes\":[\"Private note data and off-chain delivery metadata must stay wallet-local.\""
    ",\"Account-local transition proofs must bind initial and final account commitments.\",\"Ref"
    "erence blocks must prevent replay against stale account state.\",\"Production Miden note t"
    "ransactions require audited STARK parameters and account-state integration review.\",\"Pro"
    "duction hardening requires deterministic vectors, negative/adversarial test cases, repla"
    "y/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cr"
    "yptographic review.\"],\"requiredState\":[\"private note hash database\",\"input note nullifie"
    "r set\",\"account commitment state\",\"STARK VM verifier key registry\",\"wallet private note "
    "witness store\"],\"failureModes\":[\"stale reference block\",\"duplicate input note nullifier\""
    ",\"account commitment transition mismatch\",\"malformed proof bytes\",\"wrong verifier key\",\""
    "public input mismatch\"],\"setupSteps\":[\"Add private note hash/nullifier state and account"
    "-local transition verification.\",\"Register a STARK VM verifier and public-input commitme"
    "nt layout.\",\"Persist private note data and off-chain delivery metadata in the wallet not"
    "e store.\"],\"executionSteps\":[\"Execute the account-local transition against private note "
    "witnesses.\",\"Produce a STARK proof for the transaction script and account state delta.\","
    "\"Submit note nullifiers, output note hashes, account commitments, and proof.\"],\"sdkEntry"
    "points\":[],\"plannedSdkEntrypoints\":[\"buildMidenStarkTransactionProofV1\",\"buildMidenNoteT"
    "ransactionInstruction\"],\"chainRequirements\":[\"STARK VM verifier\",\"private note hash and "
    "nullifier database\",\"account commitment state\",\"wallet private-note delivery store\",\"typ"
    "ed Miden note transaction instruction\"]},{\"id\":\"aztec-private-rollup-v1\",\"name\":\"Aztec-s"
    "tyle programmable private transaction v1\",\"shortName\":\"Aztec private\",\"summary\":\"Program"
    "mable private-state transaction using client-side private execution, note hashes, nullif"
    "iers, encrypted logs, and recursive private-kernel proofs.\",\"category\":\"payment\",\"maturi"
    "ty\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\"],\"pro"
    "ofFamily\":\"plonkish-private-kernel-rollup\",\"publicInputsSchema\":\"note_hashes,nullifiers,"
    "encrypted_logs,public_call_requests,private_kernel_commitment,rollup_state_roots\",\"verif"
    "ierKeyId\":\"aztec_private_kernel_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"not"
    "eEncryption\":false},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFo"
    "r\":[\"programmable private payments\",\"hybrid public/private contract workflows\",\"wallet-s"
    "ide private execution with encrypted note discovery\"],\"sourceReferences\":[{\"label\":\"Azte"
    "c State Management\",\"url\":\"https://docs.aztec.network/developers/docs/foundational-topic"
    "s/state_management\"},{\"label\":\"Aztec Private Kernel Circuit\",\"url\":\"https://docs.aztec.n"
    "etwork/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel\"}],\""
    "securityNotes\":[\"Private-kernel proofs must bind note hashes, nullifiers, encrypted logs"
    ", and public calls.\",\"Encrypted log delivery metadata must not leak wallet note ownershi"
    "p.\",\"Recursive verifier registration must distinguish private-kernel versions and rollup"
    " state roots.\",\"Production private-rollup use requires audited private-kernel parameters"
    " and rollup-state integration review.\",\"Production hardening requires deterministic vect"
    "ors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier "
    "fuzzing, performance gates, and internal cryptographic review.\"],\"requiredState\":[\"priva"
    "te note-hash tree\",\"nullifier tree\",\"encrypted log delivery store\",\"private-kernel verif"
    "ier key registry\",\"wallet private execution witness store\"],\"failureModes\":[\"stale rollu"
    "p state root\",\"duplicate nullifier\",\"private-kernel public input mismatch\",\"malformed pr"
    "oof bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Add private note"
    "-hash and nullifier trees plus encrypted log delivery metadata.\",\"Register a private-ker"
    "nel verifier and public-input layout for private contract side effects.\",\"Persist wallet"
    " PXE-style note discovery, private call witnesses, and app-scoped nullifier keys.\"],\"exe"
    "cutionSteps\":[\"Execute private contract calls locally against wallet notes.\",\"Accumulate"
    " note hashes, nullifiers, encrypted logs, and public-call requests in the private kernel"
    ".\",\"Submit the recursive private-kernel proof and side-effect commitments for validator "
    "verification.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":[\"buildAztecPrivateKernelPro"
    "ofV1\",\"buildAztecPrivateRollupTransactionInstruction\"],\"chainRequirements\":[\"private not"
    "e-hash tree\",\"nullifier tree\",\"encrypted log store\",\"private-kernel verifier\",\"wallet pr"
    "ivate execution environment\",\"typed Aztec private-rollup transaction instruction\"]},{\"id"
    "\":\"pq-masp-stark-v0\",\"name\":\"Post-quantum MASP STARK v0\",\"shortName\":\"PQ MASP v0\",\"summa"
    "ry\":\"Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, a"
    "nd ML-KEM note encryption.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCrit"
    "eria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\",\"hide_asset_type\",\"post_quantum\"],\"pr"
    "oofFamily\":\"stark-fri\",\"publicInputsSchema\":\"pool_id,asset_set_root,nullifier_set,output"
    "_commitments,root,chain_tag,pq_policy_hash\",\"verifierKeyId\":\"pq_masp_stark_v0\",\"pqLayers"
    "\":{\"proof\":true,\"authorization\":true,\"noteEncryption\":true},\"implementationStage\":\"resea"
    "rch-target-as-of-2026-05\",\"recommendedFor\":[\"end-to-end post-quantum privacy target\",\"lo"
    "ng-horizon central-bank pilot research\",\"strict PQ proof, authorization, and note-encryp"
    "tion experiments\"],\"sourceReferences\":[{\"label\":\"NIST Post-Quantum Standards\",\"url\":\"htt"
    "ps://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-"
    "encryption-standards\"},{\"label\":\"FIPS 203 ML-KEM\",\"url\":\"https://csrc.nist.gov/pubs/fips"
    "/203/final\"},{\"label\":\"FIPS 204 ML-DSA\",\"url\":\"https://csrc.nist.gov/pubs/fips/204/final"
    "\"},{\"label\":\"FIPS 205 SLH-DSA\",\"url\":\"https://csrc.nist.gov/pubs/fips/205/final\"}],\"secu"
    "rityNotes\":[\"PQ MASP combines experimental STARK/FRI proving with production PQ authoriz"
    "ation and note encryption requirements.\",\"ML-DSA domains and ML-KEM ciphertext formats m"
    "ust be bound to verifier keys and pool identifiers.\",\"Post-quantum readiness still requi"
    "res parameter review, parser fuzzing, and internal cryptographic review.\",\"Wallet witnes"
    "s material and private inputs must stay local and must not be exposed through SDK or cha"
    "in APIs.\",\"Any chain roots, nullifiers, revocation data, or replay guards for this flow "
    "must persist across node restarts before admitting ledger mutations.\",\"Production harden"
    "ing requires deterministic vectors, negative/adversarial test cases, replay/nullifier re"
    "jection tests, parser/verifier fuzzing, performance gates, and internal cryptographic re"
    "view.\"],\"requiredState\":[\"PQ MASP asset-set commitment root\",\"PQ nullifier set\",\"ML-KEM "
    "encrypted note payload store\",\"wallet PQ note witness store\"],\"failureModes\":[\"stale ass"
    "et-set root\",\"duplicate PQ nullifier\",\"ML-DSA or ML-KEM domain mismatch\",\"malformed proo"
    "f bytes\",\"wrong verifier key\",\"public input mismatch\"],\"setupSteps\":[\"Register STARK/FRI"
    " verifier parameters and PQ MASP public input layout.\",\"Define ML-DSA authorization doma"
    "ins and ML-KEM note-encryption payload formats.\",\"Persist wallet PQ note witnesses, null"
    "ifier positions, and encapsulation metadata.\"],\"executionSteps\":[\"Select PQ MASP input n"
    "otes and derive nullifiers.\",\"Generate STARK/FRI transfer proofs with ML-DSA authorizati"
    "on and ML-KEM output-note encryption.\",\"Submit nullifiers, output commitments, PQ policy"
    " hash, and proof for verifier admission.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":["
    "\"buildPqMaspStarkTransferProofV0\",\"buildPqMaspStarkRegisterPoolInstruction\",\"buildPqMasp"
    "StarkTransferInstruction\",\"generateMlDsaKeyPair\",\"encapsulateMlKem\"],\"chainRequirements\""
    ":[\"STARK/FRI verifier enabled\",\"ML-DSA transaction authorization\",\"ML-KEM note payload e"
    "ncryption\",\"zk::RegisterAssetHiddenZkPool\",\"zk::AssetHiddenZkTransfer\",\"active PQ MASP v"
    "erifier key\"]}]"
)

_KEY_MAP = {
    "shortName": "short_name",
    "coveredCriteria": "covered_criteria",
    "proofFamily": "proof_family",
    "publicInputsSchema": "public_inputs_schema",
    "verifierKeyId": "verifier_key_id",
    "pqLayers": "pq_layers",
    "noteEncryption": "note_encryption",
    "implementationStage": "implementation_stage",
    "recommendedFor": "recommended_for",
    "sourceReferences": "source_references",
    "securityNotes": "security_notes",
    "requiredState": "required_state",
    "failureModes": "failure_modes",
    "setupSteps": "setup_steps",
    "executionSteps": "execution_steps",
    "sdkEntrypoints": "sdk_entrypoints",
    "plannedSdkEntrypoints": "planned_sdk_entrypoints",
    "chainRequirements": "chain_requirements",
    "hiddenFeatures": "hidden_features",
    "unavailableReason": "unavailable_reason",
    "verifierKeyMetadata": "verifier_key_metadata",
    "backendFamily": "backend_family",
    "productionReady": "production_ready",
    "productionGate": "production_gate",
}

_STRING_LIST_FIELDS = (
    "covered_criteria",
    "chain_requirements",
    "security_notes",
    "required_state",
    "failure_modes",
    "setup_steps",
    "execution_steps",
    "sdk_entrypoints",
    "planned_sdk_entrypoints",
    "recommended_for",
)
_REQUIRED_STRING_FIELDS = ("name", "category", "maturity")
_REQUIRED_DESCRIPTOR_STRING_FIELDS = ("short_name", "summary", "proof_family")
_REQUIRED_PRESENT_FIELDS = (
    "proof_family",
    "public_inputs_schema",
    "verifier_key_id",
)
_REQUIRED_STRING_LIST_FIELDS = (
    "covered_criteria",
    "chain_requirements",
    "security_notes",
    "failure_modes",
    "sdk_entrypoints",
    "planned_sdk_entrypoints",
)
_DERIVED_COMPATIBILITY_FIELDS = (
    "hidden_features",
    "requirements",
    "limitations",
    "status",
    "unavailable_reason",
    "verifier_key_metadata",
    "backend_family",
    "production_ready",
    "production_gate",
)
_ALLOWED_DESCRIPTOR_FIELDS = frozenset(
    (
        "id",
        "name",
        "short_name",
        "summary",
        "category",
        "maturity",
        "covered_criteria",
        "proof_family",
        "public_inputs_schema",
        "verifier_key_id",
        "pq_layers",
        "implementation_stage",
        "recommended_for",
        "source_references",
        "security_notes",
        "required_state",
        "failure_modes",
        "setup_steps",
        "execution_steps",
        "sdk_entrypoints",
        "planned_sdk_entrypoints",
        "chain_requirements",
    )
)
_SOURCE_REFERENCE_FIELDS = frozenset(("label", "url"))
_PQ_LAYER_FIELDS = frozenset(("proof", "authorization", "note_encryption"))
_SOURCE_REFERENCE_LABEL_MAX_LENGTH = 160
_ALLOWED_CATEGORIES = frozenset(
    (
        "payment",
        "authorization",
        "credential",
        "admission",
        "identity",
        "proof_backend",
    )
)
_ALLOWED_MATURITIES = frozenset(
    (
        "peer_reviewed",
        "accepted_conference",
        "technical_report",
        "arxiv_preprint",
        "specification",
    )
)
_RESEARCH_STAGE_MAY_2026 = "research-target-as-of-2026-05"
_CATALOG_STAGE_MAY_2026 = "catalog-as-of-2026-05"
_ALLOWED_IMPLEMENTATION_STAGES = frozenset(
    (
        "validator-scaffold-as-of-2026-05",
        "chain-executable",
        "sdk-builder",
        "component",
        _RESEARCH_STAGE_MAY_2026,
        _CATALOG_STAGE_MAY_2026,
        "production-hardened",
    )
)
_SOURCE_REFERENCED_IMPLEMENTATION_STAGES = frozenset(
    (
        "chain-executable",
        "sdk-builder",
        "component",
        _RESEARCH_STAGE_MAY_2026,
        "production-hardened",
    )
)
_PRE_PRODUCTION_SOURCE_REFERENCED_IMPLEMENTATION_STAGES = frozenset(
    (
        "sdk-builder",
        "component",
        _RESEARCH_STAGE_MAY_2026,
    )
)
_SOURCE_REFERENCED_REQUIRED_LIST_FIELDS = (
    "recommended_for",
    "chain_requirements",
    "security_notes",
    "required_state",
    "failure_modes",
    "setup_steps",
    "execution_steps",
)
_SOURCE_REFERENCED_REQUIRED_VERIFIER_FIELDS = (
    "public_inputs_schema",
    "verifier_key_id",
)
_SOURCE_REFERENCED_FORBIDDEN_PROOF_FAMILIES = frozenset(("none",))
_SOURCE_REFERENCED_FORBIDDEN_BACKEND_FAMILIES = frozenset(("none",))
_SOURCE_REFERENCED_SDK_ENTRYPOINT_FIELDS = (
    "sdk_entrypoints",
    "planned_sdk_entrypoints",
)
_POST_QUANTUM_REQUIRED_SOURCE_URLS = frozenset(
    (
        "https://csrc.nist.gov/pubs/fips/203/final",
        "https://csrc.nist.gov/pubs/fips/204/final",
        "https://csrc.nist.gov/pubs/fips/205/final",
    )
)
_POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS = (
    "MlDsa",
    "MlKem",
)
_FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES = (
    "Fake",
    "Forged",
    "Missing",
    "No",
    "Non",
    "Not",
    "Placeholder",
    "Without",
)
_POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS = ("ML-DSA", "ML-KEM")
_POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS = ("ML-DSA", "ML-KEM")
_POST_QUANTUM_REQUIRED_STATE_TOKENS = ("ML-KEM",)
_RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID = {
    "orchard-halo2-actions-v1": frozenset(("https://zips.z.cash/zip-0224",)),
    "penumbra-masp-v1": frozenset(
        ("https://protocol.penumbra.zone/main/shielded_pool.html",)
    ),
    "monero-fcmp-plus-plus-v1": frozenset(
        ("https://web.getmonero.org/2024/04/27/fcmps.html",)
    ),
    "miden-stark-note-v1": frozenset(
        (
            "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
            "https://docs.miden.xyz/core-concepts/miden-base/note/",
        )
    ),
    "aztec-private-rollup-v1": frozenset(
        (
            "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
        )
    ),
    "pq-masp-stark-v0": _POST_QUANTUM_REQUIRED_SOURCE_URLS,
}
_LEDGER_MUTATION_PROTECTION_METADATA_TOKENS = (
    "nullifier",
    "replay",
    "revocation",
    "link-tag",
    "link tag",
)
_TYPED_CHAIN_ADMISSION_METADATA_FIELDS = (
    "chain_requirements",
    "setup_steps",
    "execution_steps",
)
_TYPED_CHAIN_ADMISSION_TYPE_TOKENS = ("typed", "zk::")
_TYPED_CHAIN_ADMISSION_MUTATION_TOKENS = (
    "instruction",
    "transaction",
    "isi",
    "zk::",
)
_STATEFUL_LEDGER_STATE_TOKENS = (
    "nullifier",
    "commitment",
    "accumulator",
    "root",
    "revocation",
    "replay",
    "link-tag",
    "link tag",
    "tree",
)
_STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS = (
    "security_notes",
    "failure_modes",
    "setup_steps",
    "execution_steps",
    "chain_requirements",
)
_STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS = (
    ("persist", "persistence", "restart", "recovery"),
    ("replay", "nullifier", "revocation", "link-tag", "link tag"),
)
_STATEFUL_LEDGER_FAILURE_MODE_TOKEN_GROUPS = (
    ("stale", "expired", "revoked", "unknown", "wrong"),
    ("duplicate", "replay", "replayed", "nullifier", "link-tag", "link tag"),
)
_WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES = frozenset(
    (
        "chain-executable",
        "sdk-builder",
        _RESEARCH_STAGE_MAY_2026,
        "production-hardened",
    )
)
_WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES = frozenset(("proof_backend",))
_WALLET_STATE_METADATA_TOKENS = ("wallet", "witness")
_CREDENTIAL_STATE_REQUIRED_CATEGORIES = frozenset(
    ("admission", "credential", "identity")
)
_CREDENTIAL_STATE_METADATA_TOKENS = (
    "commitment",
    "commitments",
    "accumulator",
    "accumulators",
)
_VERIFIER_KEY_RECORD_METADATA_FIELDS = (
    "required_state",
    "chain_requirements",
    "setup_steps",
)
_VERIFIER_KEY_RECORD_METADATA_TOKENS = ("verifier key", "verifier-key")
_AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES = frozenset(
    ("no", "non", "not", "without")
)
_AFFIRMED_METADATA_TOKEN_VARIANTS = {
    "persist": (
        "persist",
        "persists",
        "persisted",
        "persisting",
        "persistence",
        "persistent",
    ),
}
_CHAIN_DOMAIN_BINDING_METADATA_FIELDS = (
    "public_inputs_schema",
    "security_notes",
    "failure_modes",
    "setup_steps",
    "execution_steps",
)
_CHAIN_DOMAIN_BINDING_METADATA_TOKENS = (
    "domain_separator",
    "domain separat",
    "domain separated",
    "domain separation",
    "domain separator",
    "domain-separated",
    "domain-separation",
    "domain-separator",
    "chain_id",
    "chain-id",
    "chain_tag",
    "chain tag",
    "tx_digest",
    "tx digest",
    "transaction digest",
    "transaction-digest",
    "reference_block",
    "reference block",
    "reference-block",
    "rollup_state",
    "rollup state",
    "rollup-state",
    "anchor",
    "epoch",
)
_CHAIN_DOMAIN_BINDING_FORBIDDEN_EVIDENCE_PREFIXES = frozenset(
    ("no", "non", "not", "without")
)
_PUBLIC_INPUT_SCHEMA_CHAIN_DOMAIN_BINDING_TOKEN_FRAGMENTS = (
    "domain_separator",
    "chain_id",
    "chain_tag",
    "tx_digest",
    "anchor",
    "reference_block",
    "rollup_state",
)
_PUBLIC_INPUT_SCHEMA_FORBIDDEN_EVIDENCE_PREFIXES = frozenset(
    ("no", "non", "not", "without")
)
_SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS = (
    ("deterministic vector", "deterministic vectors"),
    ("negative/adversarial", "negative test", "adversarial test"),
    ("replay/nullifier", "replay", "nullifier"),
    ("parser/verifier fuzzing", "parser fuzzing"),
    ("parser/verifier fuzzing", "verifier fuzzing"),
    ("audit", "audited", "review"),
    ("performance", "benchmark", "latency"),
)
_SOURCE_REFERENCED_HARDENING_FORBIDDEN_EVIDENCE_PREFIXES = frozenset(
    ("no", "non", "not", "without")
)
_WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS = (
    ("wallet", "witness", "private input", "private inputs", "plaintext", "secret"),
    (
        "local",
        "not exposed",
        "not be exposed",
        "not leak",
        "must not expose",
        "must not leak",
        "never leave",
    ),
)
_WALLET_WITNESS_POSITIVE_NEGATION_PREFIXES = (
    "not leak",
    "must not leak",
    "not expose",
    "must not expose",
    "not exposed",
    "not be exposed",
    "must not be exposed",
    "never leave",
    "never leave the",
)
_VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS = (
    ("malformed proof", "invalid proof", "proof parse", "proof rejected"),
    (
        "wrong verifier key",
        "verifier key mismatch",
        "verifier-key mismatch",
        "unknown verifier key",
    ),
    ("public input mismatch", "wrong public input", "public-input mismatch"),
)
_PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS = (
    "proof",
    "proofs",
    "witness",
    "witnesses",
)
_RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS = ("production",)
_RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS = ("audit", "audited", "review")
_SOURCE_REFERENCE_AUDIT_CLAIM_LABEL_PHRASES = (
    "security review",
    "external review",
    "production review",
    "assurance report",
    "attestation report",
)
_SOURCE_REFERENCE_AUDIT_CLAIM_COMPACT_FRAGMENTS = (
    "securityreview",
    "externalreview",
    "productionreview",
    "assurancereport",
    "attestationreport",
)
_SECURITY_NOTE_COMPLETED_AUDIT_CLAIM_COMPACT_FRAGMENTS = (
    "auditcomplete",
    "auditcompleted",
    "auditpassed",
    "auditapproved",
    "auditcleared",
    "auditsignoff",
    "internalcryptographicreviewcomplete",
    "internalcryptographicreviewcompleted",
    "internalcryptographicreviewpassed",
    "internalcryptographicreviewapproved",
    "securityreviewcomplete",
    "securityreviewcompleted",
    "securityreviewpassed",
    "securityreviewapproved",
    "signoffreceived",
    "signoffcomplete",
    "auditedby",
    "productionclaim",
    "claimedproduction",
    "mainnetclaim",
    "claimedmainnet",
    "mainnetcertified",
    "mainnetapproved",
    "mainnetrelease",
    "auditclaim",
    "claimedaudit",
    "thirdpartyaudited",
    "boiaudited",
    "auditedmainnet",
    "securityauditpassed",
    "securityaudited",
    "certifiedproduction",
    "certifiedmainnet",
    "releaseready",
    "releaseapproved",
    "releasecertified",
)
_DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS = (
    "productionready",
    "productionhardened",
    "productionenabled",
    "productionapproved",
    "productioncertified",
    "mainnetready",
    "mainnetcomplete",
    "mainnetcertified",
    "mainnetrelease",
    "auditedproduction",
    "externallyaudited",
    "thirdpartyaudited",
    "boiaudited",
    "auditedmainnet",
    "internalcryptographicreview",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "securityreviewpassed",
    "securityauditpassed",
    "securityaudited",
    "externalsecurityreview",
    "productionclaim",
    "claimedproduction",
    "productionverified",
    "productiongatepassed",
    "productiongatecomplete",
    "productiongateapproved",
    "mainnetclaim",
    "claimedmainnet",
    "mainnetenabled",
    "mainnetapproved",
    "certifiedproduction",
    "certifiedmainnet",
    "releaseready",
    "releaseapproved",
    "releasecertified",
    "auditcomplete",
    "auditcompleted",
    "auditclaim",
    "claimedaudit",
    "internalcryptographicreviewcomplete",
    "internalcryptographicreviewcompleted",
    "internalcryptographicreviewpassed",
    "internalcryptographicreviewapproved",
    "securityreviewcomplete",
    "securityreviewcompleted",
    "securityreviewapproved",
)
_CATALOG_LABEL_PRODUCTION_CLAIM_COMPACT_FRAGMENTS = (
    *_DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS,
)
_PRODUCTION_CLAIM_CONFUSABLES = {
    "\u0430": "a",
    "\u0435": "e",
    "\u0456": "i",
    "\u043e": "o",
    "\u0440": "p",
    "\u0441": "c",
    "\u0443": "y",
    "\u03b1": "a",
    "\u03b5": "e",
    "\u03bf": "o",
    "\u03c1": "p",
}
_PLACEHOLDER_SOURCE_REFERENCE_HOSTS = frozenset(
    (
        "127.0.0.1",
        "example.com",
        "example.net",
        "example.org",
        "localhost",
    )
)
_PLACEHOLDER_SOURCE_REFERENCE_SUFFIXES = (".example", ".invalid", ".test")
_LOCAL_SOURCE_REFERENCE_SUFFIXES = (".internal", ".lan", ".local", ".localhost")
_REBINDING_SOURCE_REFERENCE_HOSTS = frozenset(
    (
        "localtest.me",
        "lvh.me",
        "nip.io",
        "sslip.io",
    )
)
_REBINDING_SOURCE_REFERENCE_SUFFIXES = (
    ".localtest.me",
    ".nip.io",
    ".sslip.io",
)
_IMPLEMENTATION_STAGE_CHARS = frozenset("abcdefghijklmnopqrstuvwxyz0123456789-")
_PUBLIC_INPUT_SCHEMA_TOKEN_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyz0123456789_"
)
_SOURCE_REFERENCE_URL_DECODE_MAX_DEPTH = 8


def _is_lowercase_hyphenated_identifier(value: str) -> bool:
    return (
        bool(value)
        and value == value.strip()
        and value[0] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and value[-1] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and "--" not in value
        and all(char in _IMPLEMENTATION_STAGE_CHARS for char in value)
    )


def _is_sdk_entrypoint_name(value: str) -> bool:
    first_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
    rest_chars = first_chars + "0123456789"
    segments = value.split(".")
    if any(not segment for segment in segments):
        return False
    return all(
        segment[0] in first_chars
        and all(char in rest_chars for char in segment)
        for segment in segments
    )


def _is_public_input_schema_token(value: str) -> bool:
    return (
        bool(value)
        and value[0] in "abcdefghijklmnopqrstuvwxyz"
        and value[-1] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and "__" not in value
        and all(char in _PUBLIC_INPUT_SCHEMA_TOKEN_CHARS for char in value)
    )


def _public_input_schema_token_has_payload_metadata(value: str) -> bool:
    return any(
        segment in _PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS
        for segment in value.split("_")
    )


def _public_inputs_schema_has_chain_domain_binding(value: str) -> bool:
    return any(
        _public_input_schema_token_has_fragment(token, fragment)
        for token in value.split(",")
        for fragment in _PUBLIC_INPUT_SCHEMA_CHAIN_DOMAIN_BINDING_TOKEN_FRAGMENTS
    )


def _entrypoint_name_has_evidence_fragment(
    name: str,
    fragment: str,
) -> bool:
    start = 0
    while True:
        index = name.find(fragment, start)
        if index == -1:
            return False
        prefix = name[:index]
        suffix = name[index + len(fragment) :]
        has_prefix_boundary = index == 0 or _is_ascii_alnum(name[index - 1])
        has_suffix_boundary = not suffix or (
            "A" <= suffix[0] <= "Z" or "0" <= suffix[0] <= "9"
        )
        has_forbidden_prefix = any(
            prefix.endswith(forbidden_prefix)
            for forbidden_prefix in _FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES
        )
        if has_prefix_boundary and has_suffix_boundary and not has_forbidden_prefix:
            return True
        start = index + 1


def _entrypoint_name_has_terminal_evidence_fragment(
    name: str,
    fragment: str,
) -> bool:
    start = 0
    while True:
        index = name.find(fragment, start)
        if index == -1:
            return False
        prefix = name[:index]
        suffix = name[index + len(fragment) :]
        has_prefix_boundary = index == 0 or _is_ascii_alnum(name[index - 1])
        has_terminal_suffix = not suffix or (
            len(suffix) > 1
            and suffix[0] == "V"
            and all("0" <= char <= "9" for char in suffix[1:])
        )
        has_forbidden_prefix = any(
            prefix.endswith(forbidden_prefix)
            for forbidden_prefix in _FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES
        )
        if has_prefix_boundary and has_terminal_suffix and not has_forbidden_prefix:
            return True
        start = index + 1


def _planned_entrypoint_name_has_primitive_fragment(
    name: str,
    fragment: str,
) -> bool:
    return _entrypoint_name_has_evidence_fragment(name, fragment)


def _public_input_schema_token_has_fragment(token: str, fragment: str) -> bool:
    token_segments = token.split("_")
    fragment_segments = fragment.split("_")
    for index in range(len(token_segments) - len(fragment_segments) + 1):
        if token_segments[index : index + len(fragment_segments)] != fragment_segments:
            continue
        has_forbidden_prefix = any(
            prefix in _PUBLIC_INPUT_SCHEMA_FORBIDDEN_EVIDENCE_PREFIXES
            for prefix in token_segments[:index]
        )
        if not has_forbidden_prefix:
            return True
    return False


def _catalog_text_contains_bounded_token(value: str, token: str) -> bool:
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        after_index = start + len(token)
        after = "" if after_index >= len(value) else value[after_index]
        if not _is_ascii_alnum(before) and not _is_ascii_alnum(after):
            return True
        start = value.find(token, start + 1)
    return False


def _catalog_text_values_contain_bounded_token(
    values: list[str],
    token: str,
) -> bool:
    return any(_catalog_text_contains_bounded_token(value, token) for value in values)


def _catalog_text_values_contain_affirmed_metadata_token(
    values: list[str],
    token: str,
) -> bool:
    return any(
        _catalog_text_contains_affirmed_metadata_token(value, token)
        for value in values
    )


def _catalog_text_contains_metadata_token(value: str, token: str) -> bool:
    if token == "zk::":
        return _catalog_text_contains_namespace_token(value, token)
    return _catalog_text_contains_bounded_token(value, token)


def _catalog_text_contains_typed_admission_token(value: str, token: str) -> bool:
    if token == "zk::":
        return _catalog_text_contains_affirmed_namespace_token(value, token)
    return _catalog_text_contains_affirmed_metadata_token(value, token)


def _catalog_text_contains_affirmed_namespace_token(value: str, token: str) -> bool:
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        if (
            not _is_ascii_alnum(before)
            and before != "_"
            and not _catalog_text_has_forbidden_evidence_prefix(
                value,
                start,
                _AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES,
            )
        ):
            return True
        start = value.find(token, start + 1)
    return False


def _catalog_text_contains_affirmed_metadata_token(value: str, token: str) -> bool:
    for candidate in _AFFIRMED_METADATA_TOKEN_VARIANTS.get(token, (token,)):
        start = value.find(candidate)
        while start != -1:
            before = "" if start == 0 else value[start - 1]
            after_index = start + len(candidate)
            after = "" if after_index >= len(value) else value[after_index]
            if (
                not _is_ascii_alnum(before)
                and not _is_ascii_alnum(after)
                and not _catalog_text_has_forbidden_evidence_prefix(
                    value,
                    start,
                    _AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES,
                )
            ):
                return True
            start = value.find(candidate, start + 1)
    return False


def _catalog_text_contains_wallet_witness_privacy_token(
    value: str,
    token: str,
) -> bool:
    if token.startswith("not ") or token.startswith("must not ") or token == "never leave":
        return _catalog_text_contains_metadata_token(value, token)
    if _catalog_text_contains_affirmed_metadata_token(value, token):
        return True
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        after_index = start + len(token)
        after = "" if after_index >= len(value) else value[after_index]
        if (
            not _is_ascii_alnum(before)
            and not _is_ascii_alnum(after)
            and _catalog_text_has_positive_wallet_witness_privacy_prefix(
                value,
                start,
            )
        ):
            return True
        start = value.find(token, start + 1)
    return False


def _catalog_text_has_positive_wallet_witness_privacy_prefix(
    value: str,
    index: int,
) -> bool:
    segments: list[str] = []
    segment: list[str] = []
    for char in reversed(value[:index].lower()):
        if "a" <= char <= "z" or "0" <= char <= "9":
            segment.append(char)
            continue
        if segment:
            segments.append("".join(reversed(segment)))
            if len(segments) >= 5:
                break
            segment = []
    if segment and len(segments) < 5:
        segments.append("".join(reversed(segment)))
    tail = " ".join(reversed(segments))
    return any(
        tail.endswith(prefix)
        for prefix in _WALLET_WITNESS_POSITIVE_NEGATION_PREFIXES
    )


def _catalog_text_contains_chain_domain_binding_token(
    value: str,
    token: str,
) -> bool:
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        after_index = start + len(token)
        after = "" if after_index >= len(value) else value[after_index]
        if (
            not _is_ascii_alnum(before)
            and not _is_ascii_alnum(after)
            and not _catalog_text_has_forbidden_evidence_prefix(
                value,
                start,
                _CHAIN_DOMAIN_BINDING_FORBIDDEN_EVIDENCE_PREFIXES,
            )
        ):
            return True
        start = value.find(token, start + 1)
    return False


def _catalog_text_contains_source_hardening_token(
    value: str,
    token: str,
) -> bool:
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        after_index = start + len(token)
        after = "" if after_index >= len(value) else value[after_index]
        if (
            not _is_ascii_alnum(before)
            and not _is_ascii_alnum(after)
            and not _catalog_text_has_forbidden_evidence_prefix(
                value,
                start,
                _SOURCE_REFERENCED_HARDENING_FORBIDDEN_EVIDENCE_PREFIXES,
            )
        ):
            return True
        start = value.find(token, start + 1)
    return False


def _catalog_text_has_forbidden_evidence_prefix(
    value: str,
    index: int,
    forbidden_prefixes: frozenset[str],
) -> bool:
    segments: list[str] = []
    segment: list[str] = []
    for char in reversed(value[:index].lower()):
        if "a" <= char <= "z" or "0" <= char <= "9":
            segment.append(char)
            continue
        if segment:
            segments.append("".join(reversed(segment)))
            if len(segments) >= 3:
                break
            segment = []
    if segment and len(segments) < 3:
        segments.append("".join(reversed(segment)))
    return any(segment in forbidden_prefixes for segment in segments)


def _catalog_text_contains_namespace_token(value: str, token: str) -> bool:
    start = value.find(token)
    while start != -1:
        before = "" if start == 0 else value[start - 1]
        if not _is_ascii_alnum(before) and before != "_":
            return True
        start = value.find(token, start + 1)
    return False


def _is_ascii_alnum(value: str) -> bool:
    return len(value) == 1 and (
        "a" <= value <= "z"
        or "A" <= value <= "Z"
        or "0" <= value <= "9"
    )


def _is_proof_family_name(value: str) -> bool:
    token_chars = _PUBLIC_INPUT_SCHEMA_TOKEN_CHARS - frozenset("_")
    parts: list[str] = []
    segment = []
    for char in value:
        if char in "-/":
            if not segment:
                return False
            parts.append("".join(segment))
            segment = []
            continue
        if char not in token_chars:
            return False
        segment.append(char)
    if not segment:
        return False
    parts.append("".join(segment))
    return all(part for part in parts)


def _is_backend_family_name(value: str) -> bool:
    return (
        bool(value)
        and value[0] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and value[-1] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and "--" not in value
        and all(char in _BACKEND_FAMILY_NAME_CHARS for char in value)
    )


def _is_verifier_key_name(value: str) -> bool:
    return (
        bool(value)
        and value[0] in "abcdefghijklmnopqrstuvwxyz"
        and value[-1] in "abcdefghijklmnopqrstuvwxyz0123456789"
        and "__" not in value
        and all(char in _PUBLIC_INPUT_SCHEMA_TOKEN_CHARS for char in value)
    )


def _is_verifier_key_suffix(value: str) -> bool:
    first_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
    rest_chars = first_chars + "0123456789_"
    return (
        bool(value)
        and value[0] in first_chars
        and value[-1] in first_chars + "0123456789"
        and "__" not in value
        and all(char in rest_chars for char in value)
    )


def _is_verifier_key_id(value: str) -> bool:
    parts = value.split("::")
    if len(parts) > 2 or any(not part for part in parts):
        return False
    if not _is_verifier_key_name(parts[0]):
        return False
    if len(parts) == 1:
        return True
    return _is_verifier_key_suffix(parts[1])


def _is_safe_https_source_url(value: str) -> bool:
    if value != value.strip() or "\\" in value:
        return False
    if any(ord(char) < 0x21 or ord(char) > 0x7E for char in value):
        return False
    if not value.startswith("https://"):
        return False
    if _has_malformed_percent_escape(value):
        return False
    try:
        parsed = urlsplit(value)
        _ = parsed.port
    except ValueError:
        return False
    if "%" in parsed.netloc:
        return False
    if parsed.hostname is not None and _hostname_has_invalid_ipv4_literal_shape(
        parsed.hostname
    ):
        return False
    if parsed.hostname is not None and _source_reference_hostname_uses_idna(
        parsed.hostname
    ):
        return False
    return (
        parsed.scheme == "https"
        and bool(parsed.netloc)
        and bool(parsed.hostname)
        and parsed.username is None
        and parsed.password is None
    )


def _source_reference_hostname_uses_idna(hostname: str) -> bool:
    return any(label.startswith("xn--") for label in hostname.lower().split("."))


def _source_reference_path_has_dot_segments(path: str) -> bool:
    return any(unquote(segment).lower() in (".", "..") for segment in path.split("/"))


def _is_canonical_source_reference_url(value: str) -> bool:
    try:
        parsed = urlsplit(value)
        _ = parsed.port
    except ValueError:
        return False
    if parsed.hostname is None:
        return False
    hostname = parsed.hostname.lower()
    if hostname.endswith(".") or parsed.port is not None:
        return False
    if parsed.netloc != hostname:
        return False
    if _source_reference_path_has_dot_segments(parsed.path):
        return False
    return (
        urlunsplit(
            (parsed.scheme, hostname, parsed.path or "/", parsed.query, parsed.fragment)
        )
        == value
    )


def _hostname_has_invalid_ipv4_literal_shape(hostname: str) -> bool:
    labels = hostname.lower().rstrip(".").split(".")
    if len(labels) != 4 or not all(label.isdecimal() for label in labels):
        return False
    return any(int(label) > 255 for label in labels)


def _has_malformed_percent_escape(value: str) -> bool:
    for index, char in enumerate(value):
        if char != "%":
            continue
        if index + 2 >= len(value):
            return True
        escape = value[index + 1:index + 3]
        if not all(escaped in "0123456789abcdefABCDEF" for escaped in escape):
            return True
    return False


def _is_safe_source_reference_label(value: str) -> bool:
    return (
        bool(value)
        and value == value.strip()
        and len(value) <= _SOURCE_REFERENCE_LABEL_MAX_LENGTH
        and all(0x20 <= ord(char) <= 0x7E for char in value)
    )


def _source_reference_label_claims_audit(value: str) -> bool:
    folded = _fold_production_claim_text(value)
    normalized = " ".join(folded.replace("_", " ").replace("-", " ").split())
    compact = _compact_production_claim_text(value)
    return (
        "audit" in compact
        or "signoff" in compact
        or "securityreview" in compact
        or any(
            fragment in compact
            for fragment in _SOURCE_REFERENCE_AUDIT_CLAIM_COMPACT_FRAGMENTS
        )
        or any(
            phrase in normalized
            for phrase in _SOURCE_REFERENCE_AUDIT_CLAIM_LABEL_PHRASES
        )
    )


def _source_reference_url_claims_audit_or_readiness(value: str) -> bool:
    candidates: list[str] = []
    seen: set[str] = set()
    current = value
    for _depth in range(_SOURCE_REFERENCE_URL_DECODE_MAX_DEPTH):
        if current not in seen:
            seen.add(current)
            candidates.append(current)
        if "%" not in current:
            break
        if _has_malformed_percent_escape(current):
            return True
        decoded = unquote(current)
        if decoded == current:
            break
        current = decoded
    if "%" in current and unquote(current) != current:
        return True
    return any(
        _source_reference_label_claims_audit(candidate)
        or _catalog_label_claims_production_readiness(candidate)
        or _catalog_text_claims_completed_audit_or_signoff(candidate)
        for candidate in candidates
    )


def _fold_production_claim_text(value: str) -> str:
    return "".join(
        _PRODUCTION_CLAIM_CONFUSABLES.get(char, char)
        for char in unicodedata.normalize("NFKC", value).lower()
    )


def _compact_production_claim_text(value: str) -> str:
    return "".join(
        char
        for char in _fold_production_claim_text(value)
        if ("a" <= char <= "z" or "0" <= char <= "9")
    )


def _catalog_text_claims_completed_audit_or_signoff(value: str) -> bool:
    compact = _compact_production_claim_text(value)
    return any(
        fragment in compact
        for fragment in _SECURITY_NOTE_COMPLETED_AUDIT_CLAIM_COMPACT_FRAGMENTS
    )


def _display_text_claims_production_readiness(value: str) -> bool:
    compact = _compact_production_claim_text(value)
    return any(
        fragment in compact
        for fragment in _DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS
    )


def _catalog_label_claims_production_readiness(value: str) -> bool:
    compact = _compact_production_claim_text(value)
    return any(
        fragment in compact
        for fragment in _CATALOG_LABEL_PRODUCTION_CLAIM_COMPACT_FRAGMENTS
    )


def _is_placeholder_source_reference_url(value: str) -> bool:
    hostname = urlsplit(value).hostname
    if hostname is None:
        return True
    normalized_hostname = hostname.lower().rstrip(".")
    return (
        normalized_hostname in _PLACEHOLDER_SOURCE_REFERENCE_HOSTS
        or normalized_hostname.endswith(_PLACEHOLDER_SOURCE_REFERENCE_SUFFIXES)
    )


def _parse_ipv4_numeric_label(label: str) -> int | None:
    lowered = label.lower()
    if not lowered:
        return None
    if lowered.startswith("0x"):
        digits = lowered[2:]
        if not digits or any(char not in "0123456789abcdef" for char in digits):
            return None
        return int(digits, 16)
    if len(lowered) > 1 and lowered.startswith("0") and all(
        char in "01234567" for char in lowered
    ):
        return int(lowered, 8)
    if all(char in "0123456789" for char in lowered):
        return int(lowered, 10)
    return None


def _parse_obfuscated_ipv4_source_reference_address(hostname: str) -> Any | None:
    labels = hostname.split(".")
    if not labels or len(labels) > 4:
        return None
    numbers: list[int] = []
    saw_obfuscated_shape = len(labels) != 4
    for label in labels:
        number = _parse_ipv4_numeric_label(label)
        if number is None:
            return None
        if label.lower().startswith("0x") or (
            len(label) > 1 and label.startswith("0")
        ):
            saw_obfuscated_shape = True
        numbers.append(number)
    if not saw_obfuscated_shape:
        return None

    try:
        if len(numbers) == 1:
            if numbers[0] > 0xFFFFFFFF:
                return None
            return ip_address(numbers[0])
        if len(numbers) == 2:
            first, rest = numbers
            if first > 0xFF or rest > 0xFFFFFF:
                return None
            return ip_address((first << 24) | rest)
        if len(numbers) == 3:
            first, second, rest = numbers
            if first > 0xFF or second > 0xFF or rest > 0xFFFF:
                return None
            return ip_address((first << 24) | (second << 16) | rest)
        if all(number <= 0xFF for number in numbers):
            return ip_address(".".join(str(number) for number in numbers))
    except ValueError:
        return None
    return None


def _ipv6_address_matches_prefix(address: Any, prefix: str, prefix_bits: int) -> bool:
    mask_shift = 128 - prefix_bits
    return int(address) >> mask_shift == int(ip_address(prefix)) >> mask_shift


def _ipv6_tail_ipv4_address(address: Any) -> Any:
    return ip_address(int(address) & 0xFFFFFFFF)


def _ipv6_embeds_non_global_ipv4_address(address: Any) -> bool:
    if _ipv6_address_matches_prefix(address, "::", 96):
        return not _ipv6_tail_ipv4_address(address).is_global
    if _ipv6_address_matches_prefix(address, "::ffff:0:0", 96):
        return not _ipv6_tail_ipv4_address(address).is_global
    if _ipv6_address_matches_prefix(address, "64:ff9b::", 96):
        return not _ipv6_tail_ipv4_address(address).is_global
    if _ipv6_address_matches_prefix(address, "2002::", 16):
        embedded = ip_address((int(address) >> 80) & 0xFFFFFFFF)
        return not embedded.is_global
    return False


def _ipv6_address_is_reserved_source_reference(address: Any) -> bool:
    return any(
        _ipv6_address_matches_prefix(address, prefix, prefix_bits)
        for prefix, prefix_bits in (
            ("100::", 64),
            ("2001::", 32),
            ("2001:2::", 48),
            ("2001:10::", 28),
            ("2001:20::", 28),
            ("2001:db8::", 32),
        )
    )


def _is_private_or_local_source_reference_url(value: str) -> bool:
    hostname = urlsplit(value).hostname
    if hostname is None:
        return True
    normalized_hostname = hostname.lower().rstrip(".")
    if normalized_hostname.endswith(_LOCAL_SOURCE_REFERENCE_SUFFIXES):
        return True
    if (
        normalized_hostname in _REBINDING_SOURCE_REFERENCE_HOSTS
        or normalized_hostname.endswith(_REBINDING_SOURCE_REFERENCE_SUFFIXES)
    ):
        return True
    obfuscated_address = _parse_obfuscated_ipv4_source_reference_address(
        normalized_hostname
    )
    if obfuscated_address is not None:
        return not obfuscated_address.is_global
    try:
        address = ip_address(normalized_hostname)
    except ValueError:
        return False
    if address.version == 6 and (
        _ipv6_embeds_non_global_ipv4_address(address)
        or _ipv6_address_is_reserved_source_reference(address)
    ):
        return True
    return not address.is_global or bool(getattr(address, "is_site_local", False))


def _is_clean_catalog_string(value: str) -> bool:
    return value == value.strip() and all(
        not unicodedata.category(char).startswith("C")
        for char in value
    )


def _is_clean_string_list_item(value: str) -> bool:
    return _is_clean_catalog_string(value)


def _canonicalize_value(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {
            str(_KEY_MAP.get(key, key)): _canonicalize_value(item)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [_canonicalize_value(item) for item in value]
    return value


def _entrypoint_is_dev_fixture(entrypoint: str) -> bool:
    normalized = entrypoint.replace("-", "_").lower()
    compact = "".join(
        char for char in entrypoint.lower()
        if char in "abcdefghijklmnopqrstuvwxyz0123456789"
    )
    return (
        "devfixture" in normalized
        or "dev_fixture" in normalized
        or "devprooffixture" in normalized
        or "dev_proof_fixture" in normalized
        or "fixture" in normalized
        or "mock" in normalized
        or "devfixture" in compact
        or "devprooffixture" in compact
        or "fixture" in compact
        or "mock" in compact
    )


def _entrypoint_is_explicit_dev_fixture(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    parts = entrypoint.split(".")
    dotted_dev_fixture = (
        len(parts) >= 2
        and parts[-1] == "Fixture"
        and _entrypoint_name_has_terminal_evidence_fragment(parts[-2], "Dev")
    )
    dotted_dev_proof_fixture = (
        len(parts) >= 3
        and parts[-2:] == ["Proof", "Fixture"]
        and _entrypoint_name_has_terminal_evidence_fragment(parts[-3], "Dev")
    )
    return (
        _entrypoint_name_has_terminal_evidence_fragment(name, "DevFixture")
        or _entrypoint_name_has_terminal_evidence_fragment(name, "DevProofFixture")
        or dotted_dev_fixture
        or dotted_dev_proof_fixture
    )


def _entrypoint_is_local_verifier(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    return name.startswith("verify") and (
        _entrypoint_name_has_evidence_fragment(name, "Local")
        or _entrypoint_name_has_evidence_fragment(name, "Locally")
    )


def _entrypoint_is_instruction_builder(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    return _entrypoint_name_has_terminal_evidence_fragment(name, "Instruction")


def _entrypoint_is_planned_ledger_mutation(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    return (
        any(
            _entrypoint_name_has_terminal_evidence_fragment(name, fragment)
            for fragment in ("Instruction", "Transaction")
        )
        or _entrypoint_name_has_evidence_fragment(name, "Submit")
    )


def _entrypoint_is_proof_helper(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    return (
        "ProofEnvelope" in name
        or "ProofWitness" in name
        or "ProofPublicInputs" in name
        or "ProofRequest" in name
        or "ProofCommitment" in name
    )


def _entrypoint_is_production_proof_builder(entrypoint: str) -> bool:
    name = entrypoint.rsplit(".", 1)[-1]
    return (
        name.startswith("build")
        and _entrypoint_name_has_evidence_fragment(name, "Proof")
        and not _entrypoint_is_instruction_builder(entrypoint)
        and not _entrypoint_is_planned_ledger_mutation(entrypoint)
        and not _entrypoint_is_proof_helper(entrypoint)
        and not _entrypoint_is_dev_fixture(entrypoint)
    )


def _has_dev_fixture_non_production_warning(notes: list[str]) -> bool:
    for note in notes:
        normalized = note.lower()
        if (
            any(
                _catalog_text_contains_affirmed_metadata_token(normalized, token)
                for token in ("dev fixture", "dev fixtures")
            )
            and _catalog_text_contains_affirmed_metadata_token(
                normalized,
                "production",
            )
            and _catalog_text_contains_affirmed_metadata_token(
                normalized,
                "unavailable",
            )
        ):
            return True
    return False


def _validate_public_inputs_schema(value: str, index: int) -> None:
    tokens = value.split(",")
    seen_tokens: set[str] = set()
    for token_index, token in enumerate(tokens):
        if not token:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                "must be a non-empty public input name"
            )
        if token != token.strip():
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                "must be clean and already trimmed"
            )
        if not _is_public_input_schema_token(token):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                "must be a lowercase public input name"
            )
        if _public_input_schema_token_has_payload_metadata(token):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                "must not include proof or witness payload metadata; proof "
                "and witness bytes are carried separately"
            )
        if _catalog_label_claims_production_readiness(token):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                "must not claim production/mainnet/audit readiness before "
                "production gates pass"
            )
        if token in seen_tokens:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' token {token_index} "
                f"duplicates {token!r}"
            )
        seen_tokens.add(token)


def _dedupe_strings(items: list[str]) -> list[str]:
    deduped: list[str] = []
    for item in items:
        if item not in deduped:
            deduped.append(item)
    return deduped


def _production_gate_required_keys(descriptor: Mapping[str, Any]) -> list[str]:
    waived = (
        set(TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS)
        if descriptor.get("id") == "transparent-transfer"
        else set()
    )
    return [key for key, _label in PRODUCTION_GATE_REQUIREMENTS if key not in waived]


def _production_gate_for_descriptor(descriptor: Mapping[str, Any]) -> dict[str, Any]:
    flags = {key: False for key, _label in PRODUCTION_GATE_REQUIREMENTS}
    required_gates = _production_gate_required_keys(descriptor)
    required_gate_set = set(required_gates)
    missing = [
        label
        for key, label in PRODUCTION_GATE_REQUIREMENTS
        if key in required_gate_set
    ]

    if descriptor.get("implementation_stage") != "production-hardened":
        missing.append(PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE)
    if descriptor.get("planned_sdk_entrypoints"):
        missing.append(PRODUCTION_GATE_MISSING_PLANNED_SDK)
    if any(
        _entrypoint_is_dev_fixture(entrypoint)
        for entrypoint in descriptor.get("sdk_entrypoints", [])
        if isinstance(entrypoint, str)
    ):
        missing.append(PRODUCTION_GATE_MISSING_DEV_FIXTURE)
    missing.append(PRODUCTION_GATE_MISSING_ALLOWLIST)

    deduped_missing = _dedupe_strings(missing)
    return {
        "version": PRODUCTION_GATE_VERSION,
        "ready": False,
        "gates": flags,
        "required_gates": required_gates,
        "missing": deduped_missing,
        "audit_references": [],
    }


def _privacy_evidence_text_value(value: Any, *, limit: int = 256) -> str:
    if not isinstance(value, str) or not value:
        return ""
    if len(value) > limit or not _is_clean_catalog_string(value):
        return ""
    return value


def _privacy_evidence_hash_uri(value: Any) -> str:
    text = _privacy_evidence_text_value(value, limit=256)
    if text.startswith("sha256:"):
        digest = text.removeprefix("sha256:")
    elif text.startswith("urn:sha256:"):
        digest = text.removeprefix("urn:sha256:")
    elif text.startswith("hash://sha256/"):
        digest = text.removeprefix("hash://sha256/")
    else:
        return ""
    if len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
        return ""
    return text


def _privacy_evidence_artifact(value: Any) -> dict[str, str] | None:
    if not isinstance(value, Mapping) or set(value) != PRIVACY_PRODUCTION_GATE_ARTIFACT_KEYS:
        return None
    label = _privacy_evidence_text_value(value.get("label"), limit=160)
    uri = _privacy_evidence_hash_uri(value.get("uri"))
    if not label or not uri:
        return None
    return {"label": label, "uri": uri}


def _privacy_evidence_review_artifact(value: Any) -> dict[str, str] | None:
    if not isinstance(value, Mapping) or set(value) != PRIVACY_PRODUCTION_REVIEW_ARTIFACT_KEYS:
        return None
    label = _privacy_evidence_text_value(value.get("label"), limit=160)
    uri = _privacy_evidence_hash_uri(value.get("uri"))
    signature = _privacy_evidence_text_value(value.get("signature"), limit=160)
    signature_body = signature.removeprefix(
        PRIVACY_PRODUCTION_REVIEW_ARTIFACT_SIGNATURE_PREFIX
    )
    if not label or not uri or not signature:
        return None
    if (
        not signature.startswith(PRIVACY_PRODUCTION_REVIEW_ARTIFACT_SIGNATURE_PREFIX)
        or len(signature_body) != 128
        or any(char not in "0123456789abcdef" for char in signature_body)
    ):
        return None
    return {"label": label, "uri": uri, "signature": signature}


def _privacy_evidence_result(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, Mapping) or set(value) != PRIVACY_PRODUCTION_RESULT_KEYS:
        return None
    if value.get("passed") is not True:
        return None
    artifact = _privacy_evidence_artifact(value.get("artifact"))
    if artifact is None:
        return None
    return {"passed": True, "artifact": artifact}


def _privacy_evidence_string_list(value: Any) -> list[str] | None:
    if not isinstance(value, list):
        return None
    result: list[str] = []
    for item in value:
        text = _privacy_evidence_text_value(item, limit=160)
        if not text or text in result:
            return None
        result.append(text)
    return result


def _privacy_descriptor_production_sdk_entrypoints(
    descriptor: Mapping[str, Any],
) -> list[str]:
    entrypoints: list[str] = []
    for entrypoint in [
        *descriptor.get("sdk_entrypoints", []),
        *descriptor.get("planned_sdk_entrypoints", []),
    ]:
        if not isinstance(entrypoint, str):
            continue
        if _entrypoint_is_dev_fixture(entrypoint) or _entrypoint_is_local_verifier(entrypoint):
            continue
        if entrypoint not in entrypoints:
            entrypoints.append(entrypoint)
    return entrypoints


def _privacy_evidence_sdk_entrypoints(
    value: Any,
    descriptor: Mapping[str, Any],
) -> list[str] | None:
    if not isinstance(value, Mapping):
        return None
    if set(value) != set(PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES):
        return None
    expected = _privacy_descriptor_production_sdk_entrypoints(descriptor)
    for surface in PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES:
        entrypoints = _privacy_evidence_string_list(value.get(surface))
        if entrypoints != expected:
            return None
        if any(
            _entrypoint_is_dev_fixture(entrypoint)
            or _entrypoint_is_local_verifier(entrypoint)
            for entrypoint in entrypoints or []
        ):
            return None
    return expected


def _privacy_evidence_sdk_parity_artifacts(
    value: Any,
) -> dict[str, dict[str, dict[str, str]]] | None:
    if not isinstance(value, Mapping):
        return None
    if set(value) != set(PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS):
        return None
    result: dict[str, dict[str, dict[str, str]]] = {}
    for kind in PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS:
        artifacts_for_kind = value.get(kind)
        if not isinstance(artifacts_for_kind, Mapping):
            return None
        if set(artifacts_for_kind) != set(PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES):
            return None
        result[kind] = {}
        for surface in PRIVACY_PRODUCTION_SDK_ENTRYPOINT_SURFACES:
            artifact = _privacy_evidence_artifact(artifacts_for_kind.get(surface))
            if artifact is None:
                return None
            if _entrypoint_is_dev_fixture(artifact["label"]) or _entrypoint_is_dev_fixture(
                artifact["uri"]
            ):
                return None
            result[kind][surface] = artifact
    return result


def _privacy_evidence_required_state_matches(
    value: Any,
    descriptor: Mapping[str, Any],
) -> bool:
    required_state = _privacy_evidence_string_list(value)
    return required_state == list(descriptor.get("required_state", []))


def _privacy_evidence_nullable_equals(value: Any, expected: Any) -> bool:
    if expected is None:
        return value is None
    if not isinstance(expected, str):
        return value == expected
    return _privacy_evidence_text_value(value, limit=512) == expected


def _privacy_evidence_localnet_peer_id(value: Any) -> str:
    text = _privacy_evidence_text_value(value, limit=160)
    if (
        not text
        or _entrypoint_is_dev_fixture(text)
        or ".." in text
        or any(
            not (
                char.isascii()
                and (char.isalnum() or char in {"_", ".", ":", "-", "@"})
            )
            for char in text
        )
    ):
        return ""
    return text


def _privacy_evidence_localnet_run_id(value: Any) -> str:
    text = _privacy_evidence_text_value(value, limit=160)
    compact = text.replace("_", "-").lower()
    if (
        not text
        or _entrypoint_is_dev_fixture(text)
        or ".." in text
        or not ("4-peer" in compact or "4peer" in compact)
        or any(
            not (
                char.isascii()
                and (char.isalnum() or char in {"_", ".", ":", "-"})
            )
            for char in text
        )
    ):
        return ""
    return text


def _privacy_evidence_chain_id(value: Any, *, limit: int = 256) -> str:
    text = _privacy_evidence_text_value(value, limit=limit)
    if (
        not text
        or _entrypoint_is_dev_fixture(text)
        or ".." in text
        or any(
            not (
                char.isascii()
                and (char.isalnum() or char in {"_", ".", ":", "-"})
            )
            for char in text
        )
    ):
        return ""
    return text


def _privacy_evidence_localnet_acceptance(
    value: Any,
    *,
    localnet_run_id: str,
    chain_id: str,
) -> dict[str, Any] | None:
    if not isinstance(value, Mapping) or set(value) != PRIVACY_PRODUCTION_LOCALNET_ACCEPTANCE_KEYS:
        return None
    run_id = _privacy_evidence_text_value(value.get("run_id"), limit=160)
    target = _privacy_evidence_text_value(value.get("target"), limit=32)
    peer_count = value.get("peer_count")
    peer_ids_raw = value.get("peer_ids")
    if not isinstance(peer_ids_raw, list) or len(peer_ids_raw) != 4:
        return None
    peer_ids = [_privacy_evidence_localnet_peer_id(peer_id) for peer_id in peer_ids_raw]
    evidence_chain_id = _privacy_evidence_chain_id(value.get("chain_id"))
    smoke_tx_hash = _privacy_evidence_hash_uri(value.get("smoke_tx_hash"))
    replay_rejection_hash = _privacy_evidence_hash_uri(value.get("replay_rejection_hash"))
    restart_replay_rejection_hash = _privacy_evidence_hash_uri(
        value.get("restart_replay_rejection_hash")
    )
    state_recovery_hash = _privacy_evidence_hash_uri(value.get("state_recovery_hash"))
    localnet_artifact_hashes = (
        smoke_tx_hash,
        replay_rejection_hash,
        restart_replay_rejection_hash,
        state_recovery_hash,
    )
    if (
        run_id != localnet_run_id
        or target != "localnet"
        or not isinstance(peer_count, int)
        or isinstance(peer_count, bool)
        or peer_count != 4
        or not all(peer_ids)
        or len(set(peer_ids)) != 4
        or evidence_chain_id != chain_id
        or not all(localnet_artifact_hashes)
        or len(set(localnet_artifact_hashes)) != len(localnet_artifact_hashes)
    ):
        return None
    required_booleans = (
        "smoke_passed",
        "replay_rejected",
        "restart_persistence_checked",
        "restart_replay_rejected",
        "state_recovery_passed",
    )
    if any(value.get(key) is not True for key in required_booleans):
        return None
    return {
        "run_id": run_id,
        "target": "localnet",
        "peer_count": 4,
        "peer_ids": peer_ids,
        "chain_id": evidence_chain_id,
        "smoke_tx_hash": smoke_tx_hash,
        "replay_rejection_hash": replay_rejection_hash,
        "restart_replay_rejection_hash": restart_replay_rejection_hash,
        "state_recovery_hash": state_recovery_hash,
        **{key: True for key in required_booleans},
    }


def _privacy_evidence_gate_artifacts(
    value: Any,
    descriptor: Mapping[str, Any],
) -> dict[str, list[dict[str, str]]] | None:
    required_gates = _production_gate_required_keys(descriptor)
    if not isinstance(value, Mapping) or set(value) != set(required_gates):
        return None
    result: dict[str, list[dict[str, str]]] = {}
    for key in required_gates:
        items = value.get(key)
        if not isinstance(items, list) or not items or len(items) > 8:
            return None
        artifacts: list[dict[str, str]] = []
        for item in items:
            artifact = _privacy_evidence_artifact(item)
            if artifact is None:
                return None
            artifacts.append(artifact)
        result[key] = artifacts
    return result


def _privacy_production_evidence_rows(value: Any) -> dict[str, dict[str, Any]]:
    if value is None:
        return {}
    source = _canonicalize_value(value)
    raw_rows: list[Any]
    if (
        isinstance(source, Mapping)
        and source.get("version") == PRIVACY_PRODUCTION_EVIDENCE_REGISTRY_VERSION
        and isinstance(source.get("rows"), list)
    ):
        raw_rows = list(source["rows"])
    elif isinstance(source, Mapping):
        raw_rows = [
            {**dict(row), "id": algorithm_id}
            for algorithm_id, row in source.items()
            if isinstance(algorithm_id, str) and isinstance(row, Mapping)
        ]
    else:
        return {}

    rows: dict[str, dict[str, Any]] = {}
    for raw_row in raw_rows:
        if not isinstance(raw_row, Mapping):
            continue
        row = dict(raw_row)
        row_id = row.get("id") or row.get("covered_algorithm_id")
        if not isinstance(row_id, str):
            continue
        keys = set(row)
        if keys == PRIVACY_PRODUCTION_EVIDENCE_ROW_KEYS | {"id"}:
            row.pop("id", None)
        elif keys != PRIVACY_PRODUCTION_EVIDENCE_ROW_KEYS:
            continue
        rows[row_id] = row
    return rows


def _privacy_trusted_production_evidence(
    descriptor: Mapping[str, Any],
    evidence_registry: Any,
    *,
    chain_id: str | None = None,
) -> dict[str, Any] | None:
    algorithm_id = descriptor.get("id")
    if not isinstance(algorithm_id, str):
        return None
    source = _privacy_production_evidence_rows(evidence_registry).get(algorithm_id)
    if source is None:
        return None
    if source.get("version") != PRODUCTION_GATE_VERSION:
        return None
    if source.get("covered_algorithm_id") != algorithm_id:
        return None
    expected_chain_id = (
        _privacy_evidence_chain_id(chain_id)
        if chain_id is not None
        else None
    )
    evidence_chain_id = _privacy_evidence_chain_id(source.get("chain_id"))
    if (
        not evidence_chain_id
        or (chain_id is not None and not expected_chain_id)
        or (
            expected_chain_id is not None
            and evidence_chain_id != expected_chain_id
        )
    ):
        return None
    reviewer_identity = _privacy_evidence_text_value(
        source.get("reviewer_identity"),
        limit=160,
    )
    localnet_run_id = _privacy_evidence_localnet_run_id(
        source.get("localnet_run_id"),
    )
    review_artifact = _privacy_evidence_review_artifact(source.get("review_artifact"))
    if not reviewer_identity or not localnet_run_id or review_artifact is None:
        return None
    if not _privacy_evidence_nullable_equals(
        source.get("verifier_key_id"),
        descriptor.get("verifier_key_id"),
    ):
        return None
    if not _privacy_evidence_nullable_equals(
        source.get("proof_family"),
        descriptor.get("proof_family"),
    ):
        return None
    if not _privacy_evidence_nullable_equals(
        source.get("public_inputs_schema"),
        descriptor.get("public_inputs_schema"),
    ):
        return None
    sdk_entrypoints = _privacy_evidence_sdk_entrypoints(
        source.get("sdk_entrypoints"),
        descriptor,
    )
    if sdk_entrypoints is None:
        return None
    sdk_parity_artifacts = _privacy_evidence_sdk_parity_artifacts(
        source.get("sdk_parity_artifacts")
    )
    if sdk_parity_artifacts is None:
        return None
    if not _privacy_evidence_required_state_matches(
        source.get("required_state"),
        descriptor,
    ):
        return None
    fuzz_results = _privacy_evidence_result(source.get("fuzz_results"))
    performance_results = _privacy_evidence_result(source.get("performance_results"))
    if fuzz_results is None or performance_results is None:
        return None
    localnet_acceptance = _privacy_evidence_localnet_acceptance(
        source.get("localnet_acceptance"),
        localnet_run_id=localnet_run_id,
        chain_id=evidence_chain_id,
    )
    if localnet_acceptance is None:
        return None
    gate_evidence = _privacy_evidence_gate_artifacts(
        source.get("gate_evidence"),
        descriptor,
    )
    if gate_evidence is None:
        return None
    return {
        "chain_id": evidence_chain_id,
        "reviewer_identity": reviewer_identity,
        "review_artifact": review_artifact,
        "sdk_entrypoints": sdk_entrypoints,
        "fuzz_results": fuzz_results,
        "performance_results": performance_results,
        "localnet_run_id": localnet_run_id,
        "localnet_acceptance": localnet_acceptance,
        "gate_evidence": gate_evidence,
    }


def _privacy_production_gate_from_evidence(
    descriptor: Mapping[str, Any],
    evidence: Mapping[str, Any],
) -> dict[str, Any]:
    required_gates = _production_gate_required_keys(descriptor)
    required_gate_set = set(required_gates)
    gates = {
        key: key in required_gate_set
        for key, _label in PRODUCTION_GATE_REQUIREMENTS
    }
    return {
        "version": PRODUCTION_GATE_VERSION,
        "ready": True,
        "gates": gates,
        "required_gates": required_gates,
        "missing": [],
        "audit_references": [dict(evidence["review_artifact"])],
        "chain_id": evidence["chain_id"],
        "reviewer_identity": evidence["reviewer_identity"],
        "localnet_run_id": evidence["localnet_run_id"],
        "localnet_acceptance": copy.deepcopy(evidence["localnet_acceptance"]),
        "fuzz_results": copy.deepcopy(evidence["fuzz_results"]),
        "performance_results": copy.deepcopy(evidence["performance_results"]),
        "gate_evidence": copy.deepcopy(evidence["gate_evidence"]),
    }


def _with_privacy_production_evidence(
    descriptor: Mapping[str, Any],
    evidence_registry: Any,
    *,
    chain_id: str | None = None,
) -> dict[str, Any]:
    result = copy.deepcopy(dict(descriptor))
    trusted_evidence = _privacy_trusted_production_evidence(
        result,
        evidence_registry,
        chain_id=chain_id,
    )
    if trusted_evidence is None:
        return result
    result["implementation_stage"] = "production-hardened"
    result["sdk_entrypoints"] = list(trusted_evidence["sdk_entrypoints"])
    result["planned_sdk_entrypoints"] = []
    result["status"] = "production-ready"
    result["production_ready"] = True
    result["production_gate"] = _privacy_production_gate_from_evidence(
        result,
        trusted_evidence,
    )
    return result


def _with_privacy_production_evidence_registry(
    descriptors: list[dict[str, Any]],
    evidence_registry: Any,
    *,
    chain_id: str | None = None,
) -> list[dict[str, Any]]:
    if evidence_registry is None:
        return descriptors
    return [
        _with_privacy_production_evidence(
            descriptor,
            evidence_registry,
            chain_id=chain_id,
        )
        for descriptor in descriptors
    ]


def _validate_descriptor_shape(descriptor: Mapping[str, Any], index: int) -> None:
    for field in _DERIVED_COMPATIBILITY_FIELDS:
        if field in descriptor:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} is derived and must not be supplied"
            )
    for field in descriptor:
        if field not in _ALLOWED_DESCRIPTOR_FIELDS:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} is not a supported privacy catalog field"
            )

    for field in _REQUIRED_STRING_FIELDS:
        value = descriptor.get(field)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be a non-empty string"
            )
        if not _is_clean_catalog_string(value):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be clean and already trimmed"
            )

    category = descriptor["category"]
    if category not in _ALLOWED_CATEGORIES:
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'category' must be one of {sorted(_ALLOWED_CATEGORIES)}"
        )
    maturity = descriptor["maturity"]
    if maturity not in _ALLOWED_MATURITIES:
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'maturity' must be one of {sorted(_ALLOWED_MATURITIES)}"
        )
    implementation_stage = descriptor.get("implementation_stage")
    if implementation_stage is not None and (
        not isinstance(implementation_stage, str)
        or not _is_lowercase_hyphenated_identifier(implementation_stage)
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'implementation_stage' must be a lowercase "
            "hyphenated identifier"
        )
    if (
        isinstance(implementation_stage, str)
        and implementation_stage not in _ALLOWED_IMPLEMENTATION_STAGES
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'implementation_stage' must be a known implementation stage"
        )

    for field in _REQUIRED_PRESENT_FIELDS:
        if field not in descriptor:
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} field {field!r} is required"
            )
    for field in _REQUIRED_DESCRIPTOR_STRING_FIELDS:
        value = descriptor.get(field)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be a non-empty string"
            )
        if not _is_clean_catalog_string(value):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be clean and already trimmed"
            )
    proof_family = descriptor.get("proof_family")
    if isinstance(proof_family, str) and not _is_proof_family_name(proof_family):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'proof_family' must be a proof family name"
        )
    if isinstance(proof_family, str) and _catalog_label_claims_production_readiness(
        proof_family
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'proof_family' must not claim "
            "production/mainnet/audit readiness before production gates pass"
        )
    for field in ("public_inputs_schema", "verifier_key_id"):
        value = descriptor.get(field)
        if value is not None and (
            not isinstance(value, str) or not value.strip()
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be a non-empty string or null"
            )
        if isinstance(value, str) and not _is_clean_catalog_string(value):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be clean and already trimmed"
            )
    public_inputs_schema = descriptor.get("public_inputs_schema")
    if isinstance(public_inputs_schema, str):
        _validate_public_inputs_schema(public_inputs_schema, index)
    verifier_key_id = descriptor.get("verifier_key_id")
    if (public_inputs_schema is None) != (verifier_key_id is None):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} fields 'public_inputs_schema' and 'verifier_key_id' "
            "must be supplied together"
        )
    if isinstance(verifier_key_id, str) and not _is_verifier_key_id(verifier_key_id):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'verifier_key_id' must be a verifier key id"
        )
    if isinstance(
        verifier_key_id, str
    ) and _catalog_label_claims_production_readiness(verifier_key_id):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'verifier_key_id' must not claim "
            "production/mainnet/audit readiness before production gates pass"
        )

    for field in _REQUIRED_STRING_LIST_FIELDS:
        if field not in descriptor:
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} field {field!r} is required"
            )

    for field in _STRING_LIST_FIELDS:
        if field not in descriptor:
            continue
        value = descriptor[field]
        if not isinstance(value, list):
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} field {field!r} must be a list"
            )
        seen_items: set[str] = set()
        for item_index, item in enumerate(value):
            if not isinstance(item, str):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} must be a string"
                )
            if not item.strip():
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} "
                    "must be a non-empty string"
                )
            if not _is_clean_string_list_item(item):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} "
                    "must be clean and already trimmed"
                )
            if item in seen_items:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} "
                    f"duplicates {item!r}"
            )
            seen_items.add(item)
    for item_index, note in enumerate(descriptor.get("security_notes", [])):
        if _catalog_text_claims_completed_audit_or_signoff(note):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'security_notes' item {item_index} must "
                "describe missing audit/review gates, not completed audit "
                "or signoff claims"
            )
    for item_index, failure_mode in enumerate(descriptor.get("failure_modes", [])):
        if _catalog_text_claims_completed_audit_or_signoff(failure_mode):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'failure_modes' item {item_index} must "
                "describe concrete failure modes, not completed audit "
                "or signoff claims"
            )
    display_field_values = (
        ("name", (str(descriptor["name"]),)),
        ("short_name", (str(descriptor["short_name"]),)),
        ("summary", (str(descriptor["summary"]),)),
        ("recommended_for", tuple(str(item) for item in descriptor.get("recommended_for", []))),
    )
    for field, values in display_field_values:
        for item_index, value in enumerate(values):
            if _display_text_claims_production_readiness(value):
                suffix = f" item {item_index}" if field == "recommended_for" else ""
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r}{suffix} must not claim "
                    "production/mainnet/audit readiness before production gates pass"
                )
    operational_claim_fields = (
        "chain_requirements",
        "required_state",
        "setup_steps",
        "execution_steps",
    )
    for field in operational_claim_fields:
        for item_index, value in enumerate(descriptor.get(field, [])):
            if _display_text_claims_production_readiness(str(value)):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} must not "
                    "claim production/mainnet/audit readiness before "
                    "production gates pass"
                )

    for entrypoint in descriptor["planned_sdk_entrypoints"]:
        if _entrypoint_is_dev_fixture(entrypoint):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'planned_sdk_entrypoints' entry "
                f"{entrypoint!r} is a fixture/mock entrypoint, not a production entrypoint"
            )
        if _entrypoint_is_local_verifier(entrypoint):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'planned_sdk_entrypoints' entry "
                f"{entrypoint!r} is a local-only verifier entrypoint, "
                "not a production entrypoint"
            )

    for field in ("sdk_entrypoints", "planned_sdk_entrypoints"):
        seen_entrypoints: set[str] = set()
        for item_index, entrypoint in enumerate(descriptor[field]):
            if not _is_sdk_entrypoint_name(entrypoint):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} "
                    "must be an SDK entrypoint name"
                )
            if _catalog_label_claims_production_readiness(entrypoint):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} must not "
                    "claim production/mainnet/audit readiness before "
                    "production gates pass"
                )
            if entrypoint in seen_entrypoints:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} "
                    f"duplicates {entrypoint!r}"
                )
            seen_entrypoints.add(entrypoint)

    seen_criteria: set[str] = set()
    for item_index, criterion in enumerate(descriptor["covered_criteria"]):
        if criterion not in PRIVACY_CRITERIA:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'covered_criteria' item {item_index} "
                f"must be one of {list(PRIVACY_CRITERIA)}"
            )
        if criterion in seen_criteria:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'covered_criteria' item {item_index} "
                f"duplicates {criterion!r}"
            )
        seen_criteria.add(criterion)

    sdk_entrypoints = descriptor.get("sdk_entrypoints", [])
    planned_sdk_entrypoints = descriptor.get("planned_sdk_entrypoints", [])
    for entrypoint in planned_sdk_entrypoints:
        if entrypoint in sdk_entrypoints:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'planned_sdk_entrypoints' entry "
                f"{entrypoint!r} is already executable"
            )
    if descriptor.get("implementation_stage") == "component":
        for field, entrypoints in (
            ("sdk_entrypoints", sdk_entrypoints),
            ("planned_sdk_entrypoints", planned_sdk_entrypoints),
        ):
            for entrypoint in entrypoints:
                if _entrypoint_is_instruction_builder(entrypoint):
                    raise RuntimeError(
                        "privacy algorithm catalog entry "
                        f"{index} component targets cannot advertise "
                        f"instruction SDK entrypoint {entrypoint!r} in "
                        f"field {field!r}"
                    )
    if descriptor.get("implementation_stage") == _RESEARCH_STAGE_MAY_2026 and any(
        _entrypoint_is_dev_fixture(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} research targets cannot advertise fixture/mock "
            "SDK entrypoints"
        )
    if descriptor.get("implementation_stage") == _RESEARCH_STAGE_MAY_2026 and any(
        _entrypoint_is_local_verifier(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} research targets cannot advertise local-only "
            "verifier SDK entrypoints"
        )
    if (
        descriptor.get("implementation_stage") == _RESEARCH_STAGE_MAY_2026
        and sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} research targets cannot advertise executable "
            "SDK entrypoints; keep them in planned_sdk_entrypoints until "
            "the production stage advances"
        )
    if descriptor.get("implementation_stage") == "chain-executable" and any(
        _entrypoint_is_dev_fixture(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} chain-executable targets cannot advertise "
            "fixture/mock SDK entrypoints"
        )
    if descriptor.get("implementation_stage") == "chain-executable" and any(
        _entrypoint_is_local_verifier(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} chain-executable targets cannot advertise "
            "local-only verifier SDK entrypoints"
        )
    if descriptor.get("implementation_stage") == "production-hardened" and any(
        _entrypoint_is_dev_fixture(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} production-hardened targets cannot advertise "
            "fixture/mock SDK entrypoints"
        )
    if descriptor.get("implementation_stage") == "production-hardened" and any(
        _entrypoint_is_local_verifier(entrypoint)
        for entrypoint in sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} production-hardened targets cannot advertise "
            "local-only verifier SDK entrypoints"
        )
    for item_index, entrypoint in enumerate(sdk_entrypoints):
        if (
            _entrypoint_is_dev_fixture(entrypoint)
            and not _entrypoint_is_explicit_dev_fixture(entrypoint)
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'sdk_entrypoints' item {item_index} "
                "fixture/mock SDK entrypoints must use explicit DevFixture names"
            )
    if (
        any(_entrypoint_is_local_verifier(entrypoint) for entrypoint in sdk_entrypoints)
        and not any(
            _entrypoint_is_explicit_dev_fixture(entrypoint)
            for entrypoint in sdk_entrypoints
        )
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} executable local-only verifier SDK entrypoints must be "
            "paired with an explicit DevFixture entrypoint"
        )
    if any(_entrypoint_is_explicit_dev_fixture(entrypoint) for entrypoint in sdk_entrypoints):
        if not any(_entrypoint_is_local_verifier(entrypoint) for entrypoint in sdk_entrypoints):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} executable DevFixture SDK entrypoints must be paired "
                "with a local verifier entrypoint"
            )
        if not _has_dev_fixture_non_production_warning(
            list(descriptor.get("security_notes", []))
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} executable DevFixture SDK entrypoints must include "
                "a security note that marks dev fixtures as non-production "
                "and unavailable for production use"
            )
        if not planned_sdk_entrypoints:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} executable DevFixture SDK entrypoints must retain "
                "planned production SDK entrypoints until production gates pass"
            )
        if not any(
            _entrypoint_is_production_proof_builder(entrypoint)
            for entrypoint in planned_sdk_entrypoints
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} executable DevFixture SDK entrypoints must retain "
                "a planned production proof builder until production gates pass"
            )
    if (
        descriptor.get("implementation_stage") == _CATALOG_STAGE_MAY_2026
        and sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} catalog-only targets cannot advertise SDK entrypoints"
        )
    if (
        descriptor.get("implementation_stage") == "production-hardened"
        and planned_sdk_entrypoints
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} production-hardened targets cannot retain planned SDK entrypoints"
        )
    planned_ledger_mutations = [
        entrypoint
        for entrypoint in planned_sdk_entrypoints
        if _entrypoint_is_planned_ledger_mutation(entrypoint)
    ]
    if planned_ledger_mutations:
        protection_values = [
            str(value).lower()
            for field in ("required_state", "failure_modes", "chain_requirements")
            for value in descriptor.get(field, [])
            if isinstance(value, str)
        ]
        if not any(
            _catalog_text_values_contain_affirmed_metadata_token(
                protection_values,
                token,
            )
            for token in _LEDGER_MUTATION_PROTECTION_METADATA_TOKENS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} planned ledger-mutating SDK entrypoints require "
                "replay, nullifier, revocation, or link-tag protection "
                "metadata; missing protection metadata for "
                f"{planned_ledger_mutations}"
            )
        typed_admission_text = " ".join(
            str(value).lower()
            for field in _TYPED_CHAIN_ADMISSION_METADATA_FIELDS
            for value in descriptor.get(field, [])
            if isinstance(value, str)
        )
        has_typed_admission_metadata = any(
            _catalog_text_contains_typed_admission_token(typed_admission_text, token)
            for token in _TYPED_CHAIN_ADMISSION_TYPE_TOKENS
        ) and any(
            _catalog_text_contains_typed_admission_token(typed_admission_text, token)
            for token in _TYPED_CHAIN_ADMISSION_MUTATION_TOKENS
        )
        if not has_typed_admission_metadata:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} planned ledger-mutating SDK entrypoints require "
                "explicit typed chain admission metadata; missing typed "
                f"admission metadata for {planned_ledger_mutations}"
            )
        required_state_text = " ".join(
            str(value).lower()
            for value in descriptor.get("required_state", [])
            if isinstance(value, str)
        )
        has_stateful_ledger_state = any(
            _catalog_text_contains_affirmed_metadata_token(required_state_text, token)
            for token in _STATEFUL_LEDGER_STATE_TOKENS
        )
        if has_stateful_ledger_state:
            persistence_text = " ".join(
                str(value).lower()
                for field in _STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS
                for value in descriptor.get(field, [])
                if isinstance(value, str)
            )
            missing_persistence_groups = [
                tokens
                for tokens in _STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS
                if not any(
                    _catalog_text_contains_affirmed_metadata_token(
                        persistence_text,
                        token,
                    )
                    for token in tokens
                )
            ]
            if missing_persistence_groups:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} planned ledger-mutating SDK entrypoints require "
                    "restart/persistence metadata for root, nullifier, "
                    "revocation, or replay state; missing persistence "
                    f"metadata for {planned_ledger_mutations}"
                )
            failure_modes_text = " ".join(
                str(value).lower()
                for value in descriptor.get("failure_modes", [])
                if isinstance(value, str)
            )
            missing_stateful_failure_mode_groups = [
                tokens
                for tokens in _STATEFUL_LEDGER_FAILURE_MODE_TOKEN_GROUPS
                if not any(
                    _catalog_text_contains_affirmed_metadata_token(
                        failure_modes_text,
                        token,
                    )
                    for token in tokens
                )
            ]
            if missing_stateful_failure_mode_groups:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'failure_modes' must include stale-state "
                    "and duplicate/replay rejection for ledger-mutating root, "
                    "nullifier, revocation, or replay state"
                )

    if "source_references" in descriptor:
        references = descriptor["source_references"]
        if not isinstance(references, list):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'source_references' must be a list"
            )
        seen_reference_labels: set[str] = set()
        seen_reference_urls: set[str] = set()
        for item_index, item in enumerate(references):
            if not isinstance(item, Mapping):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must be an object"
                )
            unsupported_reference_fields = sorted(
                set(item) - _SOURCE_REFERENCE_FIELDS
            )
            if unsupported_reference_fields:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    f"contains unsupported keys {unsupported_reference_fields}"
                )
            if not isinstance(item.get("label"), str) or not isinstance(
                item.get("url"), str
            ):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must include string label and url"
                )
            if not item["label"].strip() or not item["url"].strip():
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must include non-empty label and url"
                )
            if not _is_safe_source_reference_label(item["label"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must include a clean bounded label"
                )
            if _source_reference_label_claims_audit(item["label"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "label must describe protocol source material, not "
                    "audit/signoff evidence"
                )
            if _catalog_label_claims_production_readiness(item["label"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "label must not claim production/mainnet/audit readiness "
                    "before production gates pass"
                )
            if not _is_safe_https_source_url(item["url"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must use an https URL"
                )
            if _is_placeholder_source_reference_url(
                item["url"]
            ) or _is_private_or_local_source_reference_url(item["url"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "url must not be a placeholder, local, or "
                    "private-network URL"
                )
            if not _is_canonical_source_reference_url(item["url"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "url must be canonical"
                )
            if _source_reference_url_claims_audit_or_readiness(item["url"]):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "url must describe protocol source material, not "
                    "audit/signoff or readiness evidence"
                )
            if item["label"] in seen_reference_labels:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    f"duplicates label {item['label']!r}"
                )
            if item["url"] in seen_reference_urls:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    f"duplicates url {item['url']!r}"
                )
            seen_reference_labels.add(item["label"])
            seen_reference_urls.add(item["url"])

    required_research_source_urls = _RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID.get(
        str(descriptor.get("id"))
    )
    if (
        descriptor.get("implementation_stage") == "research-target-as-of-2026-05"
        and required_research_source_urls is not None
    ):
        source_reference_urls = {
            str(item.get("url"))
            for item in descriptor.get("source_references", [])
            if isinstance(item, Mapping)
        }
        missing_research_source_urls = sorted(
            required_research_source_urls - source_reference_urls
        )
        if missing_research_source_urls:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'source_references' must include exact "
                "research target source URLs; missing "
                f"{missing_research_source_urls}"
            )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        and not descriptor.get("source_references")
    ):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'source_references' is required for "
            "source-referenced implementation stages"
        )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
    ):
        for item_index, item in enumerate(descriptor["source_references"]):
            if _is_placeholder_source_reference_url(str(item["url"])):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must not use placeholder or test URLs for "
                    "source-referenced implementation stages"
                )
            if _is_private_or_local_source_reference_url(str(item["url"])):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must not use private, local, or non-global URLs for "
                    "source-referenced implementation stages"
                )
        for field in _SOURCE_REFERENCED_REQUIRED_LIST_FIELDS:
            if not descriptor.get(field):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} must be non-empty for "
                    "source-referenced implementation stages"
                )
        is_transparent_baseline = descriptor.get("id") == "transparent-transfer"
        for field in _SOURCE_REFERENCED_REQUIRED_VERIFIER_FIELDS:
            if not is_transparent_baseline and not descriptor.get(field):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} must be non-empty for "
                    "source-referenced implementation stages"
                )
        if (
            not is_transparent_baseline
            and descriptor.get("proof_family")
            in _SOURCE_REFERENCED_FORBIDDEN_PROOF_FAMILIES
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'proof_family' must be a concrete proof family "
                "for source-referenced implementation stages"
            )
        backend_family = BACKEND_FAMILY_BY_ALGORITHM_ID.get(
            str(descriptor.get("id"))
        )
        if (
            not is_transparent_baseline
            and (
                backend_family is None
                or backend_family in _SOURCE_REFERENCED_FORBIDDEN_BACKEND_FAMILIES
            )
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} must have a registered non-none backend family for "
                "source-referenced implementation stages"
            )
        if (
            descriptor.get("implementation_stage")
            in _PRE_PRODUCTION_SOURCE_REFERENCED_IMPLEMENTATION_STAGES
            and not descriptor.get("planned_sdk_entrypoints")
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'planned_sdk_entrypoints' must be non-empty "
                "for pre-production source-referenced implementation stages"
            )
        if not any(
            descriptor.get(field)
            for field in _SOURCE_REFERENCED_SDK_ENTRYPOINT_FIELDS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} source-referenced implementation stages must expose "
                "at least one executable or planned SDK entrypoint"
            )
    if (
        descriptor.get("implementation_stage")
        in _WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES
        and descriptor.get("category") not in _WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES
    ):
        required_state_text = " ".join(
            str(value).lower()
            for value in descriptor.get("required_state", [])
            if isinstance(value, str)
        )
        if not any(
            _catalog_text_contains_affirmed_metadata_token(required_state_text, token)
            for token in _WALLET_STATE_METADATA_TOKENS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'required_state' must include wallet or witness "
                "state metadata for source-referenced privacy flows"
            )
        security_notes_text = " ".join(
            str(note).lower()
            for note in descriptor.get("security_notes", [])
            if isinstance(note, str)
        )
        missing_witness_privacy_groups = [
            tokens
            for tokens in _WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS
            if not any(
                _catalog_text_contains_wallet_witness_privacy_token(
                    security_notes_text,
                    token,
                )
                for token in tokens
            )
        ]
        if missing_witness_privacy_groups:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'security_notes' must include wallet/witness "
                "privacy notes for source-referenced privacy flows"
            )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        and descriptor.get("category") in _CREDENTIAL_STATE_REQUIRED_CATEGORIES
    ):
        required_state_text = " ".join(
            str(value).lower()
            for value in descriptor.get("required_state", [])
            if isinstance(value, str)
        )
        if not any(
            _catalog_text_contains_affirmed_metadata_token(required_state_text, token)
            for token in _CREDENTIAL_STATE_METADATA_TOKENS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'required_state' must include credential, "
                "identity, or admission commitment/accumulator state metadata"
            )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        and descriptor.get("verifier_key_id") is not None
    ):
        failure_modes_text = " ".join(
            str(value).lower()
            for value in descriptor.get("failure_modes", [])
            if isinstance(value, str)
        )
        missing_negative_failure_mode_groups = [
            tokens
            for tokens in _VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS
            if not any(
                _catalog_text_contains_affirmed_metadata_token(
                    failure_modes_text,
                    token,
                )
                for token in tokens
            )
        ]
        if missing_negative_failure_mode_groups:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'failure_modes' must include "
                "malformed-proof, wrong-verifier-key, and wrong-public-input "
                "rejection for source-referenced verifier entries"
            )
        verifier_key_record_text = " ".join(
            str(value).lower()
            for field in _VERIFIER_KEY_RECORD_METADATA_FIELDS
            for value in descriptor.get(field, [])
            if isinstance(value, str)
        )
        if not any(
            _catalog_text_contains_affirmed_metadata_token(
                verifier_key_record_text,
                token,
            )
            for token in _VERIFIER_KEY_RECORD_METADATA_TOKENS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} must include verifier-key record metadata for "
                "source-referenced verifier entries"
            )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
        and descriptor.get("verifier_key_id") is not None
    ):
        chain_domain_binding_text = " ".join(
            str(value).lower()
            for field in _CHAIN_DOMAIN_BINDING_METADATA_FIELDS
            for value in (
                [descriptor.get(field)]
                if isinstance(descriptor.get(field), str)
                else descriptor.get(field, [])
            )
            if isinstance(value, str)
        )
        if not any(
            _catalog_text_contains_chain_domain_binding_token(
                chain_domain_binding_text,
                token,
            )
            for token in _CHAIN_DOMAIN_BINDING_METADATA_TOKENS
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} must include chain/domain binding metadata for "
                "source-referenced verifier entries"
            )
        if not _public_inputs_schema_has_chain_domain_binding(
            str(descriptor.get("public_inputs_schema", ""))
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'public_inputs_schema' must include "
                "chain/domain binding public input for source-referenced "
                "verifier entries"
            )
    if (
        descriptor.get("implementation_stage")
        in _SOURCE_REFERENCED_IMPLEMENTATION_STAGES
    ):
        security_notes_text = " ".join(
            str(note).lower()
            for note in descriptor.get("security_notes", [])
            if isinstance(note, str)
        )
        missing_hardening_groups = [
            tokens
            for tokens in _SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS
            if not any(
                _catalog_text_contains_source_hardening_token(
                    security_notes_text,
                    token,
                )
                for token in tokens
            )
        ]
        if missing_hardening_groups:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'security_notes' must include deterministic "
                "vectors, negative/adversarial cases, replay/nullifier "
                "rejection tests, parser/verifier fuzzing, performance, and "
                "audit/review hardening gates for source-referenced entries"
            )
    if descriptor.get("implementation_stage") == "research-target-as-of-2026-05":
        security_notes_text = " ".join(
            str(note).lower()
            for note in descriptor.get("security_notes", [])
            if isinstance(note, str)
        )
        has_readiness_marker = all(
            _catalog_text_contains_affirmed_metadata_token(security_notes_text, token)
            for token in _RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS
        )
        has_evidence_marker = any(
            _catalog_text_contains_affirmed_metadata_token(security_notes_text, token)
            for token in _RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS
        )
        if not has_readiness_marker or not has_evidence_marker:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'security_notes' must include production "
                "readiness audit or review gating for research targets"
            )

    pq_layers = descriptor.get("pq_layers")
    if not isinstance(pq_layers, Mapping):
        raise RuntimeError(
            f"privacy algorithm catalog entry {index} field 'pq_layers' must be an object"
        )
    unsupported_pq_layer_fields = sorted(set(pq_layers) - _PQ_LAYER_FIELDS)
    if unsupported_pq_layer_fields:
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'pq_layers' contains unsupported keys "
            f"{unsupported_pq_layer_fields}"
        )
    for key in ("proof", "authorization", "note_encryption"):
        if key not in pq_layers:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'pq_layers.{key}' is required"
            )
        if not isinstance(pq_layers[key], bool):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'pq_layers.{key}' must be a boolean"
            )
    all_pq_layers = all(
        pq_layers[key] is True
        for key in ("proof", "authorization", "note_encryption")
    )
    if "post_quantum" in seen_criteria and not all_pq_layers:
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'covered_criteria' item 'post_quantum' "
            "requires all pq_layers to be true"
        )
    if all_pq_layers and "post_quantum" not in seen_criteria:
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{index} field 'pq_layers' with all layers true requires "
            "covered_criteria item 'post_quantum'"
        )
    if "post_quantum" in seen_criteria:
        source_reference_urls = {
            str(item.get("url"))
            for item in descriptor.get("source_references", [])
            if isinstance(item, Mapping)
        }
        missing_source_urls = sorted(
            _POST_QUANTUM_REQUIRED_SOURCE_URLS - source_reference_urls
        )
        if missing_source_urls:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'source_references' must include NIST "
                "FIPS 203, FIPS 204, and FIPS 205 URLs for post_quantum "
                f"coverage; missing {missing_source_urls}"
            )
        post_quantum_entrypoints = (
            descriptor.get("sdk_entrypoints", [])
            if descriptor.get("implementation_stage") == "production-hardened"
            else planned_sdk_entrypoints
        )
        planned_entrypoint_names = [
            entrypoint.rsplit(".", 1)[-1] for entrypoint in post_quantum_entrypoints
        ]
        missing_planned_entrypoint_fragments = [
            fragment
            for fragment in _POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS
            if not any(
                _planned_entrypoint_name_has_primitive_fragment(name, fragment)
                for name in planned_entrypoint_names
            )
        ]
        if missing_planned_entrypoint_fragments:
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'sdk_entrypoints' or 'planned_sdk_entrypoints' must include "
                "ML-DSA authorization and ML-KEM note-encryption SDK "
                "entrypoints for post_quantum coverage; missing "
                f"{missing_planned_entrypoint_fragments}"
            )
        post_quantum_token_fields = (
            (
                "security_notes",
                _POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS,
                "post-quantum primitive risk notes",
            ),
            (
                "failure_modes",
                _POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS,
                "post-quantum primitive failure modes",
            ),
            (
                "required_state",
                _POST_QUANTUM_REQUIRED_STATE_TOKENS,
                "post-quantum note-encryption state",
            ),
        )
        for field, required_tokens, label in post_quantum_token_fields:
            values = [
                str(value)
                for value in descriptor.get(field, [])
                if isinstance(value, str)
            ]
            missing_tokens = [
                token
                for token in required_tokens
                if not any(
                    _catalog_text_contains_affirmed_metadata_token(
                        value,
                        token,
                    )
                    for value in values
                )
            ]
            if missing_tokens:
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field '{field}' must include {label} for "
                    "post_quantum coverage; missing "
                    f"{missing_tokens}"
                )


def _with_boi_compatibility_fields(descriptor: Mapping[str, Any]) -> dict[str, Any]:
    result = dict(descriptor)
    algorithm_id = result.get("id")
    backend_family = BACKEND_FAMILY_BY_ALGORITHM_ID.get(str(algorithm_id))
    if backend_family is None:
        raise RuntimeError(
            f"privacy algorithm catalog entry {algorithm_id!r} is missing backend family metadata"
        )
    if not _is_backend_family_name(backend_family):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{algorithm_id!r} backend family metadata must be non-empty and use "
            "request-portable verifier-key backend characters"
        )
    if _catalog_label_claims_production_readiness(backend_family):
        raise RuntimeError(
            "privacy algorithm catalog entry "
            f"{algorithm_id!r} backend family metadata must not claim "
            "production/mainnet/audit readiness before production gates pass"
        )
    criteria = list(result.get("covered_criteria") or [])
    requirements = list(result.get("chain_requirements") or [])
    security_notes = list(result.get("security_notes") or [])
    failure_modes = list(result.get("failure_modes") or [])
    production_gate = _production_gate_for_descriptor(result)
    result["backend_family"] = backend_family
    result["hidden_features"] = criteria
    result["requirements"] = requirements
    result["limitations"] = [*security_notes, *failure_modes]
    result["status"] = "cataloged"
    result["unavailable_reason"] = None
    result["production_ready"] = bool(production_gate["ready"])
    result["production_gate"] = production_gate
    result["verifier_key_metadata"] = {
        "verifier_key_id": result.get("verifier_key_id"),
        "proof_family": result.get("proof_family"),
        "public_inputs_schema": result.get("public_inputs_schema"),
        "pq_layers": copy.deepcopy(result.get("pq_layers")),
    }
    return result


def _apply_privacy_inventory_advancements(
    descriptor: Mapping[str, Any],
) -> dict[str, Any]:
    result = dict(descriptor)
    algorithm_id = str(result.get("id") or "")
    production_inventory_algorithm_ids = {
        "transparent-transfer",
        "shield",
        "confidential-transfer-v2",
        "unshield",
        "asset-hidden-confidential-transfer-v1",
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "orchard-halo2-actions-v1",
        "penumbra-masp-v1",
        "monero-fcmp-plus-plus-v1",
        "miden-stark-note-v1",
        "aztec-private-rollup-v1",
        "pq-masp-stark-v0",
    }
    if algorithm_id in production_inventory_algorithm_ids:
        baseline_display_text_by_algorithm_id = {
            "transparent-transfer": (
                "Transparent asset transfer",
                "Transparent",
                "Public Iroha asset transfer used as the size and latency baseline.",
            ),
            "shield": (
                "Shield into confidential note",
                "Shield",
                "Debits public balance and appends an encrypted receiver note commitment.",
            ),
            "confidential-transfer-v2": (
                "Confidential transfer v2",
                "Confidential v2",
                "Halo2/Pasta note-to-note transfer that hides amount, sender note, and receiver note while publishing the asset id.",
            ),
            "unshield": (
                "Unshield to public balance",
                "Unshield",
                "Spends a private note into a public receiver balance; the private source note remains hidden.",
            ),
            "asset-hidden-confidential-transfer-v1": (
                "Asset-hidden MASP transfer v1",
                "MASP v1",
                "Target multi-asset shielded-pool transfer that hides amount, sender note, receiver note, and exact asset inside a pool.",
            ),
            "zk-ace-pq-authorization-v0": (
                "ZK-ACE post-quantum authorization v0",
                "ZK-ACE PQ auth",
                "STARK/FRI-backed source-account authorization for transparent asset transfers.",
            ),
        }
        expected_display_text = (
            REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID.get(algorithm_id)
            or baseline_display_text_by_algorithm_id.get(algorithm_id)
        )
        if expected_display_text and (
            result.get("name"),
            result.get("short_name"),
            result.get("summary"),
        ) != expected_display_text:
            return result
        result["implementation_stage"] = "production-hardened"
        if algorithm_id == "shield":
            result["public_inputs_schema"] = (
                "asset,from,amount,note_commitment,chain_id,domain_separator"
            )
        source_references_by_algorithm_id = {
            "transparent-transfer": (
                {
                    "label": "Iroha repository asset transfer implementation",
                    "url": "https://github.com/hyperledger-iroha/iroha",
                },
            ),
            "shield": (
                {
                    "label": "Iroha repository shielded asset implementation",
                    "url": "https://github.com/hyperledger-iroha/iroha",
                },
            ),
            "confidential-transfer-v2": (
                {
                    "label": "Iroha repository confidential transfer implementation",
                    "url": "https://github.com/hyperledger-iroha/iroha",
                },
            ),
            "unshield": (
                {
                    "label": "Iroha repository confidential unshield implementation",
                    "url": "https://github.com/hyperledger-iroha/iroha",
                },
            ),
            "asset-hidden-confidential-transfer-v1": (
                {
                    "label": "Iroha repository asset-hidden transfer implementation",
                    "url": "https://github.com/hyperledger-iroha/iroha",
                },
            ),
        }
        if not result.get("source_references"):
            result["source_references"] = list(
                source_references_by_algorithm_id.get(algorithm_id, ())
            )
        baseline_metadata_by_algorithm_id = {
            "transparent-transfer": {
                "recommended_for": (
                    "baseline public payment flow",
                    "SDK parity and chain-admission checks",
                ),
                "security_notes": (
                    "Transparent transfers intentionally have no privacy proof requirement.",
                    "Production readiness still requires SDK parity, chain admission, localnet acceptance, replay/restart checks, and internal review evidence.",
                    "Wallet signer material and private keys must stay local and must not be exposed through SDK or chain APIs.",
                    "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
                ),
                "required_state": (
                    "asset definition registry",
                    "source account balance",
                    "destination account",
                    "wallet signer state",
                ),
                "failure_modes": (
                    "insufficient balance",
                    "wrong asset id",
                    "chain-id mismatch",
                    "malformed transfer instruction",
                ),
                "setup_steps": (
                    "Register asset definition and participating accounts.",
                    "Fund the source account.",
                    "Configure signer, chain id, and target domain.",
                ),
                "execution_steps": (
                    "Build the transfer asset instruction.",
                    "Sign the transaction.",
                    "Submit and wait for chain admission.",
                ),
            },
            "shield": {
                "recommended_for": (
                    "entering confidential payment pools",
                    "public-to-private wallet funding",
                ),
                "security_notes": (
                    "Shielding publishes the source debit while hiding the receiver note commitment opening.",
                    "Wallet witness material and note openings must remain local.",
                ),
                "required_state": (
                    "asset definition registry",
                    "shielded note commitment set",
                    "active shield verifier key",
                    "wallet note witness store",
                ),
                "failure_modes": (
                    "insufficient public balance",
                    "malformed note commitment",
                    "malformed proof bytes",
                    "wrong verifier key",
                    "public input mismatch",
                    "wrong asset id",
                    "chain-id mismatch",
                ),
                "setup_steps": (
                    "Register the shield verifier key.",
                    "Register the shielded asset.",
                    "Prepare the receiver note commitment.",
                    "Persist wallet note opening material locally.",
                ),
                "execution_steps": (
                    "Build the shield instruction.",
                    "Submit the public debit and note commitment.",
                    "Record the resulting note witness in the wallet.",
                ),
            },
            "confidential-transfer-v2": {
                "recommended_for": (
                    "amount-private note transfers",
                    "sender and receiver note privacy",
                ),
                "security_notes": (
                    "Proof public inputs must bind note roots, nullifiers, commitments, asset tag, and chain tag.",
                    "Witness note openings must remain wallet-local.",
                ),
                "required_state": (
                    "note commitment tree root",
                    "spent nullifier set",
                    "active confidential transfer verifier key",
                    "wallet note witness store",
                ),
                "failure_modes": (
                    "stale root",
                    "duplicate nullifier",
                    "wrong verifier key",
                    "public input mismatch",
                ),
                "setup_steps": (
                    "Register the confidential transfer verifier key.",
                    "Load spendable note witnesses.",
                    "Synchronize note roots and nullifier state.",
                ),
                "execution_steps": (
                    "Build the confidential transfer proof.",
                    "Build the zk transfer instruction.",
                    "Submit and wait for verifier admission.",
                ),
            },
            "unshield": {
                "recommended_for": (
                    "private-to-public redemption",
                    "confidential note exits",
                ),
                "security_notes": (
                    "Unshield proofs must bind the public amount, destination, root, nullifier, asset tag, and chain tag.",
                    "Spent note witnesses must be redacted after submission.",
                ),
                "required_state": (
                    "note commitment tree root",
                    "spent nullifier set",
                    "active confidential unshield verifier key",
                    "wallet note witness store",
                ),
                "failure_modes": (
                    "stale root",
                    "duplicate nullifier",
                    "wrong verifier key",
                    "public amount mismatch",
                ),
                "setup_steps": (
                    "Register the confidential unshield verifier key.",
                    "Load the note witness and destination account.",
                    "Synchronize note roots and nullifier state.",
                ),
                "execution_steps": (
                    "Build the confidential unshield proof.",
                    "Build the unshield instruction.",
                    "Submit and wait for public balance admission.",
                ),
            },
            "asset-hidden-confidential-transfer-v1": {
                "recommended_for": (
                    "multi-asset confidential pool transfers",
                    "asset-id privacy with note privacy",
                ),
                "security_notes": (
                    "Asset-hidden proofs must bind pool id, asset-set root, nullifiers, output commitments, root, and chain tag.",
                    "Pool membership witnesses and asset openings must remain wallet-local.",
                ),
                "required_state": (
                    "asset-set commitment root",
                    "pool note commitment tree",
                    "spent nullifier set",
                    "active asset-hidden verifier key",
                    "wallet pool witness store",
                ),
                "failure_modes": (
                    "stale asset-set root",
                    "duplicate nullifier",
                    "wrong verifier key",
                    "asset tag mismatch",
                ),
                "setup_steps": (
                    "Register the asset-hidden pool.",
                    "Register the asset-hidden transfer verifier key.",
                    "Synchronize pool note and asset-set roots.",
                ),
                "execution_steps": (
                    "Build the asset-hidden transfer proof.",
                    "Build the pool transfer instruction.",
                    "Submit and wait for verifier admission.",
                ),
            },
        }
        for field, values in baseline_metadata_by_algorithm_id.get(
            algorithm_id, {}
        ).items():
            if not result.get(field):
                result[field] = list(values)
        hardening_note = (
            "Production hardening requires deterministic vectors, "
            "negative/adversarial test cases, replay/nullifier rejection tests, "
            "parser/verifier fuzzing, performance gates, and internal "
            "cryptographic review."
        )
        security_notes = list(result.get("security_notes") or [])
        witness_privacy_note = (
            "Wallet witness material and private inputs must stay local and "
            "must not be exposed through SDK or chain APIs."
        )
        if witness_privacy_note not in security_notes:
            security_notes.append(witness_privacy_note)
        if hardening_note not in security_notes:
            security_notes.append(hardening_note)
            result["security_notes"] = security_notes
        else:
            result["security_notes"] = security_notes
        if result.get("verifier_key_id") is not None:
            failure_modes = list(result.get("failure_modes") or [])
            for failure_mode in (
                "malformed proof bytes",
                "wrong verifier key",
                "public input mismatch",
            ):
                if failure_mode not in failure_modes:
                    failure_modes.append(failure_mode)
            result["failure_modes"] = failure_modes
    if algorithm_id in REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID and (
        result.get("name"),
        result.get("short_name"),
        result.get("summary"),
    ) == REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[algorithm_id]:
        result["security_notes"] = list(
            REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[algorithm_id]
        )
        result["sdk_entrypoints"] = list(
            REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[algorithm_id]
        )
        result["planned_sdk_entrypoints"] = list(
            REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
                algorithm_id
            ]
        )
    return result


def _validate_backend_family_registration(
    descriptors: tuple[Mapping[str, Any], ...],
) -> None:
    catalog_ids = [str(descriptor["id"]) for descriptor in descriptors]
    backend_ids = list(BACKEND_FAMILY_BY_ALGORITHM_ID)
    if backend_ids != catalog_ids:
        raise RuntimeError(
            "privacy algorithm backend-family registration must exactly match "
            "catalog ids"
        )


def _validate_required_privacy_plan_rows(
    descriptors: tuple[Mapping[str, Any], ...],
) -> None:
    by_id = {str(descriptor.get("id")): descriptor for descriptor in descriptors}
    for algorithm_id, implementation_stage, backend_family in REQUIRED_PRIVACY_PLAN_ROWS:
        descriptor = by_id.get(algorithm_id)
        if descriptor is None:
            raise RuntimeError(
                "privacy algorithm catalog missing required production privacy "
                f"plan row {algorithm_id!r}"
            )
        display_text = REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if (
            descriptor.get("name"),
            descriptor.get("short_name"),
            descriptor.get("summary"),
        ) != display_text:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep display text {display_text!r} "
                "until the production inventory is deliberately updated"
            )
        if descriptor.get("implementation_stage") != implementation_stage:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep implementation_stage "
                f"{implementation_stage!r} until the production inventory is "
                "deliberately updated"
            )
        if BACKEND_FAMILY_BY_ALGORITHM_ID.get(algorithm_id) != backend_family:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep backend family {backend_family!r} "
                "until the production inventory is deliberately updated"
            )
        category = REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID[algorithm_id]
        if descriptor.get("category") != category:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep category {category!r} until the "
                "production inventory is deliberately updated"
            )
        maturity = REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID[algorithm_id]
        if descriptor.get("maturity") != maturity:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep maturity {maturity!r} until the "
                "production inventory is deliberately updated"
            )
        recommended_for = REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("recommended_for") or ()) != recommended_for:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep recommendedFor "
                f"{recommended_for!r} until the production inventory is "
                "deliberately updated"
            )
        covered_criteria = REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("covered_criteria") or ()) != covered_criteria:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep covered criteria "
                f"{covered_criteria!r} until the production inventory is "
                "deliberately updated"
            )
        proof_family = REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if descriptor.get("proof_family") != proof_family:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep proof family {proof_family!r} "
                "until the production inventory is deliberately updated"
            )
        public_inputs_schema = REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if descriptor.get("public_inputs_schema") != public_inputs_schema:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep public inputs schema "
                f"{public_inputs_schema!r} until the production inventory is "
                "deliberately updated"
            )
        verifier_key_id = REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if descriptor.get("verifier_key_id") != verifier_key_id:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep verifier key id {verifier_key_id!r} "
                "until the production inventory is deliberately updated"
            )
        pq_layers = REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID[algorithm_id]
        descriptor_pq_layers = descriptor.get("pq_layers") or {}
        for pq_layer_name, expected_enabled in pq_layers.items():
            if descriptor_pq_layers.get(pq_layer_name) is not expected_enabled:
                raise RuntimeError(
                    "privacy algorithm catalog required production privacy "
                    f"plan row {algorithm_id!r} must keep PQ layer "
                    f"{pq_layer_name!r}={expected_enabled!r} until the "
                    "production inventory is deliberately updated"
                )
        chain_requirements = REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("chain_requirements") or ()) != chain_requirements:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep chain requirements "
                f"{chain_requirements!r} until the production inventory is "
                "deliberately updated"
            )
        required_state = REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("required_state") or ()) != required_state:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep required state "
                f"{required_state!r} until the production inventory is "
                "deliberately updated"
            )
        setup_steps = REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID[algorithm_id]
        if tuple(descriptor.get("setup_steps") or ()) != setup_steps:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep setup steps {setup_steps!r} until "
                "the production inventory is deliberately updated"
            )
        execution_steps = REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("execution_steps") or ()) != execution_steps:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep execution steps "
                f"{execution_steps!r} until the production inventory is "
                "deliberately updated"
            )
        failure_modes = REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("failure_modes") or ()) != failure_modes:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep failure modes "
                f"{failure_modes!r} until the production inventory is "
                "deliberately updated"
            )
        security_notes = REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("security_notes") or ()) != security_notes:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep security notes "
                f"{security_notes!r} until the production inventory is "
                "deliberately updated"
            )
        state_text = "\n".join(
            str(value).lower()
            for value in (
                list(descriptor.get("required_state") or [])
                + list(descriptor.get("chain_requirements") or [])
            )
        )
        for state_token in REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID[
            algorithm_id
        ]:
            if not _catalog_text_contains_affirmed_metadata_token(
                state_text,
                state_token,
            ):
                raise RuntimeError(
                    "privacy algorithm catalog required production privacy "
                    f"plan row {algorithm_id!r} must retain required state "
                    f"token {state_token!r} until the production inventory is "
                    "deliberately updated"
                )
        failure_mode_text = "\n".join(
            str(value).lower() for value in descriptor.get("failure_modes") or []
        )
        for failure_token in (
            REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS
            + REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID[algorithm_id]
        ):
            if not _catalog_text_contains_affirmed_metadata_token(
                failure_mode_text,
                failure_token,
            ):
                raise RuntimeError(
                    "privacy algorithm catalog required production privacy "
                    f"plan row {algorithm_id!r} must retain required "
                    f"failure-mode token {failure_token!r} until the "
                    "production inventory is deliberately updated"
                )
        source_reference_pairs = {
            (item.get("label"), item.get("url"))
            for item in descriptor.get("source_references") or []
        }
        source_references = REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[
            algorithm_id
        ]
        for label, url in source_references:
            if (label, url) not in source_reference_pairs:
                raise RuntimeError(
                    "privacy algorithm catalog required production privacy "
                    f"plan row {algorithm_id!r} must retain source reference "
                    f"{label!r} <{url}> until the production inventory is "
                    "deliberately updated"
                )
        descriptor_source_references = tuple(
            (item.get("label"), item.get("url"))
            for item in descriptor.get("source_references") or []
        )
        if descriptor_source_references != source_references:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep source references until the "
                "production inventory is deliberately updated"
            )
        sdk_entrypoints = REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
            algorithm_id
        ]
        if tuple(descriptor.get("sdk_entrypoints") or ()) != sdk_entrypoints:
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep SDK entrypoints "
                f"{sdk_entrypoints!r} until the production inventory is "
                "deliberately updated"
            )
        planned_sdk_entrypoints = (
            REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
                algorithm_id
            ]
        )
        proof_builder_entrypoints = (
            descriptor.get("sdk_entrypoints", [])
            if descriptor.get("implementation_stage") == "production-hardened"
            else descriptor.get("planned_sdk_entrypoints", [])
        )
        if not any(
            _entrypoint_is_production_proof_builder(entrypoint)
            for entrypoint in proof_builder_entrypoints
        ):
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must retain or export a production proof "
                "builder"
            )
        if tuple(descriptor.get("planned_sdk_entrypoints") or []) != (
            planned_sdk_entrypoints
        ):
            raise RuntimeError(
                "privacy algorithm catalog required production privacy plan row "
                f"{algorithm_id!r} must keep planned SDK entrypoints "
                f"{planned_sdk_entrypoints!r} until the production inventory "
                "is deliberately updated"
            )


def _validate_research_target_sdk_entrypoints(
    descriptors: tuple[Mapping[str, Any], ...],
) -> None:
    for descriptor in descriptors:
        if descriptor.get("implementation_stage") != _RESEARCH_STAGE_MAY_2026:
            continue
        sdk_entrypoints = descriptor.get("sdk_entrypoints", [])
        if sdk_entrypoints:
            raise RuntimeError(
                "privacy algorithm catalog research target "
                f"{descriptor.get('id')!r} cannot advertise executable "
                "SDK entrypoints; keep them in planned_sdk_entrypoints until "
                "the production stage advances"
            )


def _load_descriptors() -> tuple[dict[str, Any], ...]:
    loaded = json.loads(_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON)
    if not isinstance(loaded, list):
        raise RuntimeError("privacy algorithm catalog must decode to a list")
    descriptors: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    seen_verifier_key_ids: set[str] = set()
    for index, item in enumerate(loaded):
        if not isinstance(item, Mapping):
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} must decode to an object"
            )
        canonical = _apply_privacy_inventory_advancements(
            _canonicalize_value(item)
        )
        algorithm_id = canonical.get("id")
        if not isinstance(algorithm_id, str) or not algorithm_id.strip():
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} must include a non-empty id"
            )
        if (
            algorithm_id != algorithm_id.lower()
            or algorithm_id[0] not in "abcdefghijklmnopqrstuvwxyz0123456789"
            or algorithm_id[-1] not in "abcdefghijklmnopqrstuvwxyz0123456789"
            or any(char not in _ALGORITHM_ID_CHARS for char in algorithm_id)
        ):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} id {algorithm_id!r} must be lowercase and URL-safe"
            )
        if _catalog_label_claims_production_readiness(algorithm_id):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} id {algorithm_id!r} must not claim "
                "production/mainnet/audit readiness before production gates pass"
            )
        if algorithm_id in seen_ids:
            raise RuntimeError(
                f"privacy algorithm catalog contains duplicate id {algorithm_id!r}"
            )
        seen_ids.add(algorithm_id)
        _validate_descriptor_shape(canonical, index)
        verifier_key_id = canonical.get("verifier_key_id")
        if isinstance(verifier_key_id, str):
            if verifier_key_id in seen_verifier_key_ids:
                raise RuntimeError(
                    "privacy algorithm catalog contains duplicate verifier_key_id "
                    f"{verifier_key_id!r}"
                )
            seen_verifier_key_ids.add(verifier_key_id)
        descriptor = _with_boi_compatibility_fields(canonical)
        descriptors.append(descriptor)
    return tuple(descriptors)


_PRIVACY_ALGORITHM_DESCRIPTORS = _load_descriptors()
_validate_backend_family_registration(_PRIVACY_ALGORITHM_DESCRIPTORS)
_validate_required_privacy_plan_rows(_PRIVACY_ALGORITHM_DESCRIPTORS)
_validate_research_target_sdk_entrypoints(_PRIVACY_ALGORITHM_DESCRIPTORS)
_PRIVACY_ALGORITHM_DESCRIPTOR_BY_ID = {
    str(descriptor["id"]): descriptor
    for descriptor in _PRIVACY_ALGORITHM_DESCRIPTORS
}


def get_privacy_algorithm_descriptors(
    production_evidence: Any | None = None,
    *,
    chain_id: str | None = None,
) -> list[dict[str, Any]]:
    """Return defensive-copy privacy algorithm descriptors."""

    descriptors = copy.deepcopy(list(_PRIVACY_ALGORITHM_DESCRIPTORS))
    return _with_privacy_production_evidence_registry(
        descriptors,
        production_evidence,
        chain_id=chain_id,
    )


def get_privacy_algorithm_descriptor(
    algorithm_id: str,
    production_evidence: Any | None = None,
    *,
    chain_id: str | None = None,
) -> dict[str, Any] | None:
    """Return one defensive-copy descriptor by id, or ``None`` if unknown."""

    if not isinstance(algorithm_id, str):
        return None
    descriptor = _PRIVACY_ALGORITHM_DESCRIPTOR_BY_ID.get(algorithm_id)
    if descriptor is None:
        return None
    return _with_privacy_production_evidence(
        descriptor,
        production_evidence,
        chain_id=chain_id,
    )


def get_privacy_criteria() -> list[str]:
    """Return supported privacy criterion identifiers."""

    return list(PRIVACY_CRITERIA)


def _callable_on_client(client: Any, name: str, *, default: bool = False) -> bool:
    if client is None:
        return default
    try:
        value = getattr(client, name)
    except Exception:  # pragma: no cover - defensive against hostile descriptors
        return False
    return callable(value)


def _callable_on_instruction(name: str) -> bool:
    try:
        from .crypto import Instruction
    except Exception:  # pragma: no cover - optional native extension
        return False
    return callable(getattr(Instruction, name, None))


def _callable_on_crypto(name: str) -> bool:
    try:
        from . import crypto
    except Exception:  # pragma: no cover - optional native extension
        return False
    return callable(getattr(crypto, name, None))


def _callable_on_native_crypto(name: str) -> bool:
    try:
        from . import crypto
    except Exception:  # pragma: no cover - optional native extension
        return False
    return callable(getattr(getattr(crypto, "_crypto", None), name, None))


def _callable_on_verange(name: str) -> bool:
    try:
        from . import verange
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(verange, name, None))


def _callable_on_anonymous_pgc(name: str) -> bool:
    try:
        from . import anonymous_pgc
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(anonymous_pgc, name, None))


def _callable_on_zkat(name: str) -> bool:
    try:
        from . import zkat
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(zkat, name, None))


def _callable_on_zk_ams(name: str) -> bool:
    try:
        from . import zk_ams
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(zk_ams, name, None))


def _callable_on_vega(name: str) -> bool:
    try:
        from . import vega
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(vega, name, None))


def _callable_on_silent_threshold(name: str) -> bool:
    try:
        from . import silent_threshold
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(silent_threshold, name, None))


def _callable_on_zk_x509(name: str) -> bool:
    try:
        from . import zk_x509
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(zk_x509, name, None))


def _callable_on_jindo(name: str) -> bool:
    try:
        from . import jindo
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(jindo, name, None))


def _callable_on_sis_hints(name: str) -> bool:
    try:
        from . import sis_hints
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(sis_hints, name, None))


def _callable_on_research_adapters(name: str) -> bool:
    try:
        from . import research_adapters
    except Exception:  # pragma: no cover - optional package dependency
        return False
    return callable(getattr(research_adapters, name, None))


def _privacy_plan_entrypoints_available(
    algorithm_id: str,
    probe: Any,
) -> bool:
    implementation_stage = next(
        (
            stage
            for row_algorithm_id, stage, _backend_family in REQUIRED_PRIVACY_PLAN_ROWS
            if row_algorithm_id == algorithm_id
        ),
        "",
    )
    if implementation_stage in {"production-hardened", "component"}:
        entrypoints = REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
            algorithm_id
        ]
    else:
        entrypoints = REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
            algorithm_id
        ]
    if not entrypoints:
        return False
    return all(
        probe(entrypoint)
        for entrypoint in entrypoints
    )


def _ml_dsa_available() -> bool:
    try:
        from .crypto import supported_crypto_algorithms
    except Exception:  # pragma: no cover - optional native extension
        return False
    try:
        return "ml-dsa" in supported_crypto_algorithms()
    except Exception:  # pragma: no cover - defensive
        return False


def _privacy_native_available() -> bool:
    try:
        from .crypto import is_privacy_native_available
    except Exception:  # pragma: no cover - optional native extension
        return False
    try:
        return is_privacy_native_available() is True
    except Exception:  # pragma: no cover - defensive against native probes
        return False


def privacy_capabilities(
    client: Any | None = None,
    production_evidence: Any | None = None,
    *,
    chain_id: str | None = None,
) -> dict[str, Any]:
    """Return SDK privacy catalog and implementation capability metadata."""

    zk_ace_register = _callable_on_instruction("register_zk_ace_identity_commitment")
    zk_ace_rotate = _callable_on_instruction("rotate_zk_ace_identity_commitment")
    zk_ace_revoke = _callable_on_instruction("revoke_zk_ace_identity_commitment")
    zk_ace_transfer = _callable_on_instruction("zk_ace_authorized_transfer")
    verify_proof = _callable_on_instruction("verify_proof")
    asset_hidden_pool_registration = _callable_on_instruction(
        "register_asset_hidden_zk_pool"
    )
    asset_hidden_transfer = _callable_on_instruction(
        "asset_hidden_zk_transfer_prepared"
    )
    zk_ace_prover = _callable_on_crypto(
        "build_zk_ace_authorization_proof_v1"
    ) and _callable_on_crypto("zk_ace_build_transfer_authorization_v1")
    zk_ace_sdk_exports = (
        zk_ace_register and zk_ace_rotate and zk_ace_revoke and zk_ace_transfer and zk_ace_prover
    )
    confidential_transfer_proof_v2 = (
        _callable_on_crypto("buildConfidentialTransferProofV2")
        and _callable_on_crypto("build_confidential_transfer_proof_v2")
        and _callable_on_native_crypto("build_confidential_transfer_proof_v2")
    )
    confidential_unshield_proof_v3 = (
        _callable_on_crypto("buildConfidentialUnshieldProofV3")
        and _callable_on_crypto("build_confidential_unshield_proof_v3")
        and _callable_on_native_crypto("build_confidential_unshield_proof_v3")
    )
    asset_hidden_transfer_proof_v1 = (
        _callable_on_crypto("buildConfidentialAssetHiddenTransferProofV1")
        and _callable_on_crypto("build_confidential_asset_hidden_transfer_proof_v1")
        and _callable_on_native_crypto(
            "build_confidential_asset_hidden_transfer_proof_v1"
        )
    )
    verange_commitment_builder = _callable_on_verange("buildRangeCommitment")
    verange_envelope_builder = _callable_on_verange("buildVeRangeProofEnvelope")
    verange_proof_builder = False
    verange_proof_verifier = False
    verange_dev_fixture = _callable_on_verange("buildVeRangeDevProofFixture")
    verange_local_verifier = _callable_on_verange("verifyVeRangeProofLocally")
    verange_sdk_exports = _privacy_plan_entrypoints_available(
        "verange-transparent-range-v1",
        _callable_on_verange,
    )
    anonymous_pgc_receiver_set_builder = _callable_on_anonymous_pgc(
        "buildAnonymousPgcReceiverSet"
    )
    anonymous_pgc_dev_fixture = _callable_on_anonymous_pgc(
        "buildAnonymousPgcDevProofFixture"
    )
    anonymous_pgc_local_verifier = _callable_on_anonymous_pgc(
        "verifyAnonymousPgcDevProofLocally"
    )
    anonymous_pgc_sdk_exports = _privacy_plan_entrypoints_available(
        "anonymous-pgc-k-out-of-n-v1",
        _callable_on_anonymous_pgc,
    )
    zkat_policy_commitment_builder = _callable_on_zkat("buildZkAtPolicyCommitment")
    zkat_authenticator_envelope_builder = _callable_on_zkat(
        "buildZkAtAuthenticatorEnvelope"
    )
    zkat_policy_proof_builder = _callable_on_zkat("buildZkAtPolicyProofV1")
    zkat_policy_proof_verifier = _callable_on_zkat("verifyZkAtPolicyProofV1")
    zkat_sdk_exports = _privacy_plan_entrypoints_available(
        "zkat-policy-private-auth-v1",
        _callable_on_zkat,
    )
    zk_ams_admission_batch_builder = _callable_on_zk_ams("buildZkAmsAdmissionBatch")
    zk_ams_proof_envelope_builder = _callable_on_zk_ams(
        "buildZkAmsAdmissionProofEnvelope"
    )
    zk_ams_admission_proof_builder = _callable_on_zk_ams(
        "buildZkAmsAdmissionBatchProofV0"
    )
    zk_ams_admission_proof_verifier = _callable_on_zk_ams(
        "verifyZkAmsAdmissionBatchProofV0"
    )
    zk_ams_sdk_exports = _privacy_plan_entrypoints_available(
        "zk-ams-recursive-admission-v0",
        _callable_on_zk_ams,
    )
    vega_predicate_commitment_builder = _callable_on_vega(
        "buildVegaCredentialPredicateCommitment"
    )
    vega_proof_envelope_builder = _callable_on_vega("buildVegaCredentialProofEnvelope")
    vega_credential_proof_builder = _callable_on_vega(
        "buildVegaCredentialPredicateProofV0"
    )
    vega_credential_proof_verifier = _callable_on_vega(
        "verifyVegaCredentialPredicateProofV0"
    )
    vega_sdk_exports = _privacy_plan_entrypoints_available(
        "vega-existing-credential-zk-v0",
        _callable_on_vega,
    )
    silent_threshold_commitments_builder = _callable_on_silent_threshold(
        "buildSilentThresholdCredentialCommitments"
    )
    silent_threshold_envelope_builder = _callable_on_silent_threshold(
        "buildSilentThresholdCredentialEnvelope"
    )
    silent_threshold_credential_proof_builder = _callable_on_silent_threshold(
        "buildSilentThresholdCredentialShowingProofV0"
    )
    silent_threshold_credential_proof_verifier = _callable_on_silent_threshold(
        "verifySilentThresholdCredentialShowingProofV0"
    )
    silent_threshold_sdk_exports = _privacy_plan_entrypoints_available(
        "silent-threshold-anoncred-v0",
        _callable_on_silent_threshold,
    )
    zk_x509_identity_commitments_builder = _callable_on_zk_x509(
        "buildZkX509IdentityCommitments"
    )
    zk_x509_identity_envelope_builder = _callable_on_zk_x509(
        "buildZkX509IdentityEnvelope"
    )
    zk_x509_identity_proof_builder = _callable_on_zk_x509(
        "buildZkX509IdentityProofV0"
    )
    zk_x509_identity_proof_verifier = _callable_on_zk_x509(
        "verifyZkX509IdentityProofV0"
    )
    zk_x509_identity_dev_fixture = _callable_on_zk_x509(
        "buildZkX509IdentityDevProofFixture"
    )
    zk_x509_identity_local_verifier = _callable_on_zk_x509(
        "verifyZkX509IdentityProofLocally"
    )
    zk_x509_identity_sdk_exports = _privacy_plan_entrypoints_available(
        "zk-x509-onchain-identity-v0",
        _callable_on_zk_x509,
    )
    jindo_lattice_public_inputs_builder = _callable_on_jindo(
        "buildJindoLatticePublicInputs"
    )
    jindo_lattice_proof_envelope_builder = _callable_on_jindo(
        "buildJindoLatticeProofEnvelope"
    )
    jindo_lattice_proof_builder = _callable_on_jindo(
        "buildJindoLatticeProofV0"
    )
    jindo_lattice_proof_verifier = _callable_on_jindo(
        "verifyJindoPolynomialCommitmentV0"
    )
    jindo_lattice_sdk_exports = _privacy_plan_entrypoints_available(
        "jindo-lattice-pcs-zk-v0",
        _callable_on_jindo,
    )
    sis_hints_credential_commitments_builder = _callable_on_sis_hints(
        "buildSisHintsCredentialCommitments"
    )
    sis_hints_credential_envelope_builder = _callable_on_sis_hints(
        "buildSisHintsCredentialEnvelope"
    )
    sis_hints_credential_proof_builder = _callable_on_sis_hints(
        "buildSisHintsAnonymousCredentialProofV0"
    )
    sis_hints_credential_proof_verifier = _callable_on_sis_hints(
        "verifySisHintsAnonymousCredentialProofV0"
    )
    sis_hints_credential_sdk_exports = _privacy_plan_entrypoints_available(
        "sis-hints-anoncred-pq-v0",
        _callable_on_sis_hints,
    )
    orchard_sdk_exports = _privacy_plan_entrypoints_available(
        "orchard-halo2-actions-v1",
        _callable_on_research_adapters,
    )
    penumbra_sdk_exports = _privacy_plan_entrypoints_available(
        "penumbra-masp-v1",
        _callable_on_research_adapters,
    )
    fcmp_sdk_exports = _privacy_plan_entrypoints_available(
        "monero-fcmp-plus-plus-v1",
        _callable_on_research_adapters,
    )
    miden_sdk_exports = _privacy_plan_entrypoints_available(
        "miden-stark-note-v1",
        _callable_on_research_adapters,
    )
    aztec_sdk_exports = _privacy_plan_entrypoints_available(
        "aztec-private-rollup-v1",
        _callable_on_research_adapters,
    )
    pq_masp_sdk_exports = _privacy_plan_entrypoints_available(
        "pq-masp-stark-v0",
        _callable_on_research_adapters,
    )
    zk_ace_register_available = zk_ace_register and _callable_on_client(
        client, "register_zk_ace_identity_commitment_and_wait", default=True
    )
    zk_ace_rotate_available = zk_ace_rotate and _callable_on_client(
        client, "rotate_zk_ace_identity_commitment_and_wait", default=True
    )
    zk_ace_revoke_available = zk_ace_revoke and _callable_on_client(
        client, "revoke_zk_ace_identity_commitment_and_wait", default=True
    )
    zk_ace_transfer_available = zk_ace_transfer and _callable_on_client(
        client, "zk_ace_authorized_transfer_and_wait", default=True
    )
    zk_ace_validator_support = (
        zk_ace_register_available
        and zk_ace_rotate_available
        and zk_ace_revoke_available
        and zk_ace_transfer_available
    )
    asset_hidden_pool_registration_available = (
        asset_hidden_pool_registration
        and _callable_on_client(
            client,
            "register_asset_hidden_zk_pool_and_wait",
            default=True,
        )
    )
    asset_hidden_transfer_available = asset_hidden_transfer and _callable_on_client(
        client,
        "asset_hidden_zk_transfer_prepared_and_wait",
        default=True,
    )

    return {
        "python_sdk_available": True,
        "bridge_available": _privacy_native_available(),
        "privacy_algorithms": get_privacy_algorithm_descriptors(
            production_evidence,
            chain_id=chain_id,
        ),
        "privacy_criteria": get_privacy_criteria(),
        "transfer_asset_instruction": _callable_on_client(
            client, "transfer_asset_and_wait", default=True
        ),
        "shield_instruction": _callable_on_client(
            client, "shield_asset_and_wait", default=True
        ),
        "zk_transfer_instruction": _callable_on_client(
            client, "zk_transfer_prepared_and_wait", default=True
        ),
        "unshield_instruction": _callable_on_client(
            client, "unshield_prepared_and_wait", default=True
        ),
        "verify_proof_instruction": verify_proof
        and _callable_on_client(client, "verify_proof_and_wait", default=True),
        "zk_ace_register_identity_instruction": zk_ace_register_available,
        "zk_ace_rotate_identity_instruction": zk_ace_rotate_available,
        "zk_ace_revoke_identity_instruction": zk_ace_revoke_available,
        "zk_ace_identity_lifecycle_instruction": zk_ace_register
        and zk_ace_rotate
        and zk_ace_revoke,
        "zk_ace_authorized_transfer_instruction": zk_ace_transfer_available,
        "zk_ace_authorization_proof_v1": zk_ace_prover,
        "zk_ace_native_air_prover_v1": zk_ace_prover,
        "zk_ace_validator_support_v1": zk_ace_validator_support,
        "zk_ace_air_opening_privacy_v1": zk_ace_prover,
        "zk_ace_sdk_exports_v1": zk_ace_sdk_exports,
        "verange_commitment_builder_v1": verange_commitment_builder,
        "verange_proof_envelope_builder_v1": verange_envelope_builder,
        "verange_proof_builder_v1": verange_proof_builder,
        "verange_proof_verifier_v1": verange_proof_verifier,
        "verange_dev_fixture_v1": verange_dev_fixture,
        "verange_local_verifier_v1": verange_local_verifier,
        "verange_sdk_exports_v1": verange_sdk_exports,
        "anonymous_pgc_receiver_set_builder_v1": anonymous_pgc_receiver_set_builder,
        "anonymous_pgc_dev_fixture_v1": anonymous_pgc_dev_fixture,
        "anonymous_pgc_local_verifier_v1": anonymous_pgc_local_verifier,
        "anonymous_pgc_sdk_exports_v1": anonymous_pgc_sdk_exports,
        "zkat_policy_commitment_builder_v1": zkat_policy_commitment_builder,
        "zkat_authenticator_envelope_builder_v1": zkat_authenticator_envelope_builder,
        "zkat_policy_proof_builder_v1": zkat_policy_proof_builder,
        "zkat_policy_proof_verifier_v1": zkat_policy_proof_verifier,
        "zkat_sdk_exports_v1": zkat_sdk_exports,
        "zk_ams_admission_batch_builder_v0": zk_ams_admission_batch_builder,
        "zk_ams_proof_envelope_builder_v0": zk_ams_proof_envelope_builder,
        "zk_ams_admission_batch_proof_builder_v0": zk_ams_admission_proof_builder,
        "zk_ams_admission_batch_proof_verifier_v0": zk_ams_admission_proof_verifier,
        "zk_ams_sdk_exports_v0": zk_ams_sdk_exports,
        "vega_predicate_commitment_builder_v0": vega_predicate_commitment_builder,
        "vega_proof_envelope_builder_v0": vega_proof_envelope_builder,
        "vega_credential_predicate_proof_builder_v0": vega_credential_proof_builder,
        "vega_credential_predicate_proof_verifier_v0": vega_credential_proof_verifier,
        "vega_sdk_exports_v0": vega_sdk_exports,
        "silent_threshold_commitments_builder_v0": silent_threshold_commitments_builder,
        "silent_threshold_envelope_builder_v0": silent_threshold_envelope_builder,
        "silent_threshold_credential_proof_builder_v0": silent_threshold_credential_proof_builder,
        "silent_threshold_credential_proof_verifier_v0": silent_threshold_credential_proof_verifier,
        "silent_threshold_sdk_exports_v0": silent_threshold_sdk_exports,
        "zk_x509_identity_commitments_builder_v0": zk_x509_identity_commitments_builder,
        "zk_x509_identity_envelope_builder_v0": zk_x509_identity_envelope_builder,
        "zk_x509_identity_proof_builder_v0": zk_x509_identity_proof_builder,
        "zk_x509_identity_proof_verifier_v0": zk_x509_identity_proof_verifier,
        "zk_x509_identity_dev_fixture_v0": zk_x509_identity_dev_fixture,
        "zk_x509_identity_local_verifier_v0": zk_x509_identity_local_verifier,
        "zk_x509_identity_sdk_exports_v0": zk_x509_identity_sdk_exports,
        "jindo_lattice_public_inputs_builder_v0": jindo_lattice_public_inputs_builder,
        "jindo_lattice_proof_envelope_builder_v0": jindo_lattice_proof_envelope_builder,
        "jindo_lattice_proof_builder_v0": jindo_lattice_proof_builder,
        "jindo_lattice_proof_verifier_v0": jindo_lattice_proof_verifier,
        "jindo_lattice_sdk_exports_v0": jindo_lattice_sdk_exports,
        "sis_hints_credential_commitments_builder_v0": sis_hints_credential_commitments_builder,
        "sis_hints_credential_envelope_builder_v0": sis_hints_credential_envelope_builder,
        "sis_hints_credential_proof_builder_v0": sis_hints_credential_proof_builder,
        "sis_hints_credential_proof_verifier_v0": sis_hints_credential_proof_verifier,
        "sis_hints_credential_sdk_exports_v0": sis_hints_credential_sdk_exports,
        "orchard_halo2_actions_sdk_exports_v1": orchard_sdk_exports,
        "penumbra_masp_sdk_exports_v1": penumbra_sdk_exports,
        "monero_fcmp_plus_plus_sdk_exports_v1": fcmp_sdk_exports,
        "miden_stark_note_sdk_exports_v1": miden_sdk_exports,
        "aztec_private_rollup_sdk_exports_v1": aztec_sdk_exports,
        "pq_masp_stark_sdk_exports_v0": pq_masp_sdk_exports,
        "confidential_transfer_proof_v2": confidential_transfer_proof_v2,
        "confidential_unshield_proof_v3": confidential_unshield_proof_v3,
        "asset_hidden_transfer_instruction": asset_hidden_transfer_available,
        "asset_hidden_pool_registration_instruction": asset_hidden_pool_registration_available,
        "asset_hidden_transfer_proof_v1": asset_hidden_transfer_proof_v1,
        "stark_proof_family": True,
        "ml_dsa_authorization": _ml_dsa_available(),
        "ml_kem_note_encryption": False,
    }
