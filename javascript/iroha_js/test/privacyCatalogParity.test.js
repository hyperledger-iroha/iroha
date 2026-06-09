import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  getPrivacyAlgorithmDescriptor as getSrcPrivacyAlgorithmDescriptor,
  getPrivacyAlgorithmDescriptors as getSrcPrivacyAlgorithmDescriptors,
  getPrivacyCapabilities as getSrcPrivacyCapabilities,
  getPrivacyCriteria as getSrcPrivacyCriteria,
  validatePrivacyAlgorithmDescriptor as validateSrcPrivacyAlgorithmDescriptor,
} from "../src/privacyAlgorithms.js";
import * as jsSrcCrypto from "../src/crypto.js";
import * as jsSrcBrowserCrypto from "../src/crypto.browser.js";
import * as jsSrcInstructionBuilders from "../src/instructionBuilders.js";
import * as jsSrcPackage from "../src/index.js";
import {
  getPrivacyAlgorithmDescriptor as getDistPrivacyAlgorithmDescriptor,
  getPrivacyAlgorithmDescriptors as getDistPrivacyAlgorithmDescriptors,
  getPrivacyCapabilities as getDistPrivacyCapabilities,
  getPrivacyCriteria as getDistPrivacyCriteria,
  validatePrivacyAlgorithmDescriptor as validateDistPrivacyAlgorithmDescriptor,
} from "../dist/privacyAlgorithms.js";
import * as jsDistCrypto from "../dist/crypto.js";
import * as jsDistBrowserCrypto from "../dist/crypto.browser.js";
import * as jsDistInstructionBuilders from "../dist/instructionBuilders.js";
import * as jsDistPackage from "../dist/index.js";

const PYTHON_PRIVACY_CATALOG = fileURLToPath(
  new URL("../../../python/iroha_python/src/iroha_python/privacy_catalog.py", import.meta.url),
);
const JS_DECLARATIONS = "javascript/iroha_js/index.d.ts";
const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PRODUCTION_GATE_VERSION = "privacy-production-gate-v1";
const PRODUCTION_GATE_REQUIREMENTS = Object.freeze([
  Object.freeze(["real_proving", "real proving engine is not registered"]),
  Object.freeze(["real_verification", "real verifier is not registered"]),
  Object.freeze(["chain_admission", "chain admission path is not enabled"]),
  Object.freeze(["sdk_parity", "cross-SDK parity is incomplete"]),
  Object.freeze(["wallet_state", "wallet/state support is incomplete"]),
  Object.freeze(["witness_privacy_checks", "witness privacy checks are incomplete"]),
  Object.freeze(["deterministic_tests", "deterministic tests are incomplete"]),
  Object.freeze(["negative_adversarial_tests", "negative/adversarial tests are incomplete"]),
  Object.freeze(["replay_nullifier_tests", "replay/nullifier rejection tests are incomplete"]),
  Object.freeze(["fuzzing", "fuzzing gate is incomplete"]),
  Object.freeze(["parser_fuzzing", "parser fuzzing gate is incomplete"]),
  Object.freeze(["verifier_fuzzing", "verifier fuzzing gate is incomplete"]),
  Object.freeze(["performance_gates", "performance gate is incomplete"]),
  Object.freeze(["external_audit", "internal cryptographic review signoff is missing"]),
]);
const PRODUCTION_GATE_REQUIRED_REASONS = Object.freeze(
  PRODUCTION_GATE_REQUIREMENTS.map(([_key, reason]) => reason),
);
const TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS = Object.freeze([
  "real_proving",
  "real_verification",
  "witness_privacy_checks",
  "verifier_fuzzing",
]);
const SUPPLEMENTAL_FAIL_CLOSED_REASONS = Object.freeze([
  "implementation stage is not production-hardened",
  "planned SDK entrypoints remain",
  "dev fixture entrypoints are not production entrypoints",
  "Iroha production allowlist is not enabled for this audited row",
]);
const POST_QUANTUM_REQUIRED_SOURCE_URLS = Object.freeze([
  "https://csrc.nist.gov/pubs/fips/203/final",
  "https://csrc.nist.gov/pubs/fips/204/final",
  "https://csrc.nist.gov/pubs/fips/205/final",
]);
const POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS = Object.freeze([
  "MlDsa",
  "MlKem",
]);
const FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES = Object.freeze([
  "Fake",
  "Forged",
  "Missing",
  "No",
  "Non",
  "Not",
  "Placeholder",
  "Without",
]);
const POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS = Object.freeze(["ML-DSA", "ML-KEM"]);
const POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS = Object.freeze(["ML-DSA", "ML-KEM"]);
const POST_QUANTUM_REQUIRED_STATE_TOKENS = Object.freeze(["ML-KEM"]);
const RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID = Object.freeze({
  "orchard-halo2-actions-v1": Object.freeze(["https://zips.z.cash/zip-0224"]),
  "penumbra-masp-v1": Object.freeze([
    "https://protocol.penumbra.zone/main/shielded_pool.html",
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    "https://web.getmonero.org/2024/04/27/fcmps.html",
  ]),
  "miden-stark-note-v1": Object.freeze([
    "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
    "https://docs.miden.xyz/core-concepts/miden-base/note/",
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
  ]),
  "pq-masp-stark-v0": POST_QUANTUM_REQUIRED_SOURCE_URLS,
});
const LEDGER_MUTATION_PROTECTION_METADATA_TOKENS = Object.freeze([
  "nullifier",
  "replay",
  "revocation",
  "link-tag",
  "link tag",
]);
const TYPED_CHAIN_ADMISSION_METADATA_FIELDS = Object.freeze([
  "chain_requirements",
  "setup_steps",
  "execution_steps",
]);
const TYPED_CHAIN_ADMISSION_TYPE_TOKENS = Object.freeze(["typed", "zk::"]);
const TYPED_CHAIN_ADMISSION_MUTATION_TOKENS = Object.freeze([
  "instruction",
  "transaction",
  "isi",
  "zk::",
]);
const STATEFUL_LEDGER_STATE_TOKENS = Object.freeze([
  "nullifier",
  "commitment",
  "accumulator",
  "root",
  "revocation",
  "replay",
  "link-tag",
  "link tag",
  "tree",
]);
const STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS = Object.freeze([
  "security_notes",
  "failure_modes",
  "setup_steps",
  "execution_steps",
  "chain_requirements",
]);
const STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["persist", "persistence", "restart", "recovery"]),
  Object.freeze(["replay", "nullifier", "revocation", "link-tag", "link tag"]),
]);
const STATEFUL_LEDGER_FAILURE_MODE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["stale", "expired", "revoked", "unknown", "wrong"]),
  Object.freeze(["duplicate", "replay", "replayed", "nullifier", "link-tag", "link tag"]),
]);
const WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "research-target-as-of-2026-05",
  "production-hardened",
]);
const SOURCE_REFERENCED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "component",
  "research-target-as-of-2026-05",
  "production-hardened",
]);
const WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES = new Set(["proof_backend"]);
const WALLET_STATE_METADATA_TOKENS = Object.freeze(["wallet", "witness"]);
const CREDENTIAL_STATE_REQUIRED_CATEGORIES = new Set([
  "admission",
  "credential",
  "identity",
]);
const CREDENTIAL_STATE_METADATA_TOKENS = Object.freeze([
  "commitment",
  "commitments",
  "accumulator",
  "accumulators",
]);
const VERIFIER_KEY_RECORD_METADATA_FIELDS = Object.freeze([
  "required_state",
  "chain_requirements",
  "setup_steps",
]);
const VERIFIER_KEY_RECORD_METADATA_TOKENS = Object.freeze(["verifier key", "verifier-key"]);
const AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES = Object.freeze([
  "no",
  "non",
  "not",
  "without",
]);
const CHAIN_DOMAIN_BINDING_METADATA_FIELDS = Object.freeze([
  "public_inputs_schema",
  "security_notes",
  "failure_modes",
  "setup_steps",
  "execution_steps",
]);
const CHAIN_DOMAIN_BINDING_METADATA_TOKENS = Object.freeze([
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
]);
const CHAIN_DOMAIN_BINDING_FORBIDDEN_EVIDENCE_PREFIXES = Object.freeze([
  "no",
  "non",
  "not",
  "without",
]);
const PUBLIC_INPUT_SCHEMA_CHAIN_DOMAIN_BINDING_TOKEN_FRAGMENTS = Object.freeze([
  "domain_separator",
  "chain_id",
  "chain_tag",
  "tx_digest",
  "anchor",
  "reference_block",
  "rollup_state",
]);
const PUBLIC_INPUT_SCHEMA_FORBIDDEN_EVIDENCE_PREFIXES = Object.freeze([
  "no",
  "non",
  "not",
  "without",
]);
const SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["deterministic vector", "deterministic vectors"]),
  Object.freeze(["negative/adversarial", "negative test", "adversarial test"]),
  Object.freeze(["replay/nullifier", "replay", "nullifier"]),
  Object.freeze(["parser/verifier fuzzing", "parser fuzzing"]),
  Object.freeze(["parser/verifier fuzzing", "verifier fuzzing"]),
  Object.freeze(["audit", "audited", "review"]),
  Object.freeze(["performance", "benchmark", "latency"]),
]);
const SOURCE_REFERENCED_HARDENING_FORBIDDEN_EVIDENCE_PREFIXES = Object.freeze([
  "no",
  "non",
  "not",
  "without",
]);
const WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["wallet", "witness", "private input", "private inputs", "plaintext", "secret"]),
  Object.freeze([
    "local",
    "not exposed",
    "not be exposed",
    "not leak",
    "must not expose",
    "must not leak",
    "never leave",
  ]),
]);
const WALLET_WITNESS_POSITIVE_NEGATION_PREFIXES = Object.freeze([
  "not leak",
  "must not leak",
  "not expose",
  "must not expose",
  "not exposed",
  "not be exposed",
  "must not be exposed",
  "never leave",
  "never leave the",
]);
const VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["malformed proof", "invalid proof", "proof parse", "proof rejected"]),
  Object.freeze([
    "wrong verifier key",
    "verifier key mismatch",
    "verifier-key mismatch",
    "unknown verifier key",
  ]),
  Object.freeze(["public input mismatch", "wrong public input", "public-input mismatch"]),
]);
const PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS = Object.freeze([
  "proof",
  "proofs",
  "witness",
  "witnesses",
]);
const RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS = Object.freeze(["production"]);
const RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS = Object.freeze([
  "audit",
  "audited",
  "review",
]);
const RUST_NATIVE_SUPPLEMENTAL_FAIL_CLOSED_REASONS = Object.freeze([
  "real protocol engine is not production-enabled",
  "Iroha production allowlist is not enabled for this audited row",
]);
const REQUIRED_PRIVACY_PLAN_ROWS = Object.freeze([
  Object.freeze(["anonymous-pgc-k-out-of-n-v1", "sdk-builder", "anonymous-pgc"]),
  Object.freeze(["verange-transparent-range-v1", "component", "verange"]),
  Object.freeze(["zkat-policy-private-auth-v1", "sdk-builder", "zkat"]),
  Object.freeze([
    "zk-ams-recursive-admission-v0",
    "sdk-builder",
    "recursive-anonymous-admission",
  ]),
  Object.freeze([
    "vega-existing-credential-zk-v0",
    "sdk-builder",
    "vega-existing-credential-zk",
  ]),
  Object.freeze([
    "silent-threshold-anoncred-v0",
    "sdk-builder",
    "silent-threshold-anoncred",
  ]),
  Object.freeze(["zk-x509-onchain-identity-v0", "sdk-builder", "zk-x509"]),
  Object.freeze(["jindo-lattice-pcs-zk-v0", "sdk-builder", "lattice-pcs-sis"]),
  Object.freeze(["sis-hints-anoncred-pq-v0", "sdk-builder", "sis-with-hints"]),
  Object.freeze(["zk-ace-pq-authorization-v0", "chain-executable", "stark-fri"]),
  Object.freeze([
    "orchard-halo2-actions-v1",
    "research-target-as-of-2026-05",
    "halo2-ipa-orchard",
  ]),
  Object.freeze([
    "penumbra-masp-v1",
    "research-target-as-of-2026-05",
    "groth16-bls12-377",
  ]),
  Object.freeze([
    "monero-fcmp-plus-plus-v1",
    "research-target-as-of-2026-05",
    "fcmp-plus-plus-curve-tree",
  ]),
  Object.freeze([
    "miden-stark-note-v1",
    "research-target-as-of-2026-05",
    "miden-stark",
  ]),
  Object.freeze([
    "aztec-private-rollup-v1",
    "research-target-as-of-2026-05",
    "aztec-plonkish-private-kernel",
  ]),
  Object.freeze(["pq-masp-stark-v0", "research-target-as-of-2026-05", "pq-masp-stark-fri"]),
]);
const REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["Anonymous PGC k-out-of-n payments v1", "Anonymous PGC", "Account-based anonymous confidential payment target with hidden sender, hidden amount, receiver privacy, and k-out-of-n receiver-set proofs."]),
  "verange-transparent-range-v1": Object.freeze(["VeRange transparent range proofs v1", "VeRange", "Verification-efficient transparent range-proof component for confidential amounts, solvency proofs, and numeric credential predicates."]),
  "zkat-policy-private-auth-v1": Object.freeze(["zkAt policy-private authorization v1", "zkAt policy auth", "Policy-private blockchain authenticator that hides threshold rules, signer sets, and account authorization logic."]),
  "zk-ams-recursive-admission-v0": Object.freeze(["ZK-AMS recursive anonymous admission v0", "ZK-AMS admission", "Research target for recursively aggregated anonymous admission from real-world personhood or eligibility credentials into anonymous on-chain accounts."]),
  "vega-existing-credential-zk-v0": Object.freeze(["Vega existing-credential ZK proofs v0", "Vega credentials", "Low-latency zero-knowledge proof target for proving predicates over existing credentials without revealing the full credential."]),
  "silent-threshold-anoncred-v0": Object.freeze(["Silent threshold anonymous credentials v0", "Silent threshold cred", "Research target for threshold-issued anonymous credentials with silent setup, issuer hiding, constant-size showings, and dynamic verifier policies."]),
  "zk-x509-onchain-identity-v0": Object.freeze(["ZK-X.509 on-chain identity v0", "ZK-X.509 identity", "ZK proof target for X.509 certificate validity, ownership, revocation status, and wallet-address binding."]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["Jindo lattice polynomial commitment ZK v0", "Jindo lattice PCS", "2026 lattice-based polynomial commitment candidate for post-quantum zero-knowledge proof systems."]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["SIS-with-hints PQ anonymous credentials v0", "SIS hints anoncred", "PKC 2026 research foundation for lattice/SIS-with-hints anonymous credentials and post-quantum credential proofs."]),
  "zk-ace-pq-authorization-v0": Object.freeze(["ZK-ACE post-quantum authorization v0", "ZK-ACE PQ auth", "STARK/FRI-backed source-account authorization for transparent asset transfers."]),
  "orchard-halo2-actions-v1": Object.freeze(["Orchard-style Halo2 action bundle v1", "Orchard Halo2", "Zcash Orchard-style action bundle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions."]),
  "penumbra-masp-v1": Object.freeze(["Penumbra-style multi-asset shielded pool v1", "Penumbra MASP", "Single multi-asset shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs for private IBC-style assets."]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["Monero FCMP++ RingCT-style transfer v1", "FCMP++", "Full-chain membership proof target that replaces small decoy rings with a full-output-set spend proof while retaining hidden amounts and one-time receivers."]),
  "miden-stark-note-v1": Object.freeze(["Miden-style STARK private note transaction v1", "Miden STARK", "Client-side STARK-proved account transition using private notes whose data stays off-chain while note hashes/nullifiers anchor correctness."]),
  "aztec-private-rollup-v1": Object.freeze(["Aztec-style programmable private transaction v1", "Aztec private", "Programmable private-state transaction using client-side private execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs."]),
  "pq-masp-stark-v0": Object.freeze(["Post-quantum MASP STARK v0", "PQ MASP v0", "Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption."]),
});
const REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = Object.freeze({
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
});
const REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = Object.freeze({
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
});
const REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["account-based private payments", "multi-receiver confidential transfers", "payment privacy without a note-based shielded pool UX"]),
  "verange-transparent-range-v1": Object.freeze(["confidential amount range proofs", "reserve or solvency proofs", "numeric credential predicates"]),
  "zkat-policy-private-auth-v1": Object.freeze(["institutional wallet policy privacy", "hidden threshold authorization", "authorization-policy migration without revealing signer topology"]),
  "zk-ams-recursive-admission-v0": Object.freeze(["anonymous onboarding", "Sybil-resistant wallet issuance", "credential-gated CBDC pilots"]),
  "vega-existing-credential-zk-v0": Object.freeze(["legacy credential bridges", "private eligibility checks", "attribute predicates for wallet enrollment"]),
  "silent-threshold-anoncred-v0": Object.freeze(["multi-authority regulated credentials", "issuer-hiding eligibility proofs", "central-bank or supervisor issued wallet credentials"]),
  "zk-x509-onchain-identity-v0": Object.freeze(["institutional wallet identity", "legal-entity account binding", "private PKI-based eligibility checks"]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["post-quantum proof-system research", "future PQ verifier backend evaluation", "lattice PCS benchmarking"]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["post-quantum anonymous credential research", "future PQ KYC or eligibility proofs", "assumption tracking for lattice credential designs"]),
  "zk-ace-pq-authorization-v0": Object.freeze(["post-quantum transaction authorization migration", "identity-private source-account authorization", "authorization envelopes for transparent asset transfers"]),
  "orchard-halo2-actions-v1": Object.freeze(["single-asset private transfers", "mature note/nullifier wallet design", "compact client proofs without Groth16 ceremonies"]),
  "penumbra-masp-v1": Object.freeze(["multi-asset shielded pools", "IBC-style asset privacy", "asset-id hiding with typed-value notes"]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["maximal sender anonymity sets", "decoy-ring replacement research", "account-independent UTXO spend privacy"]),
  "miden-stark-note-v1": Object.freeze(["client-side proving", "private programmable note workflows", "parallel account-local transaction execution"]),
  "aztec-private-rollup-v1": Object.freeze(["programmable private payments", "hybrid public/private contract workflows", "wallet-side private execution with encrypted note discovery"]),
  "pq-masp-stark-v0": Object.freeze(["end-to-end post-quantum privacy target", "long-horizon central-bank pilot research", "strict PQ proof, authorization, and note-encryption experiments"]),
});
const REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
  "verange-transparent-range-v1": Object.freeze(["hide_amount"]),
  "zkat-policy-private-auth-v1": Object.freeze([]),
  "zk-ams-recursive-admission-v0": Object.freeze([]),
  "vega-existing-credential-zk-v0": Object.freeze([]),
  "silent-threshold-anoncred-v0": Object.freeze([]),
  "zk-x509-onchain-identity-v0": Object.freeze([]),
  "jindo-lattice-pcs-zk-v0": Object.freeze([]),
  "sis-hints-anoncred-pq-v0": Object.freeze([]),
  "zk-ace-pq-authorization-v0": Object.freeze([]),
  "orchard-halo2-actions-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
  "penumbra-masp-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver", "hide_asset_type"]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
  "miden-stark-note-v1": Object.freeze(["hide_amount", "hide_receiver", "hide_asset_type"]),
  "aztec-private-rollup-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
  "pq-masp-stark-v0": Object.freeze(["hide_amount", "hide_sender", "hide_receiver", "hide_asset_type", "post_quantum"]),
});
const REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = Object.freeze({
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
});
const REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1":
    "anonymity_set_root,tx_digest,balance_commitments,receiver_set_commitment,receiver_ciphertext_commitments,receiver_threshold,receiver_count,link_tag,range_commitments,chain_id,domain_separator",
  "verange-transparent-range-v1":
    "commitments,range_parameters,aggregation_count,domain_separator,payload_digest",
  "zkat-policy-private-auth-v1":
    "policy_commitment,tx_digest,account_id,action_class,domain_separator,policy_epoch",
  "zk-ams-recursive-admission-v0":
    "issuer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_admission_digest,domain_separator",
  "vega-existing-credential-zk-v0":
    "issuer_commitment,credential_schema,predicate_commitment,subject_binding,expiration_epoch,domain_separator",
  "silent-threshold-anoncred-v0":
    "issuer_set_commitment,threshold_policy_hash,credential_showing_commitment,showing_nullifier,verifier_policy_hash,domain_separator",
  "zk-x509-onchain-identity-v0":
    "ca_root_commitment,certificate_policy_hash,revocation_root,subject_commitment,address_binding,domain_separator",
  "jindo-lattice-pcs-zk-v0":
    "commitment,opening_claim,query_set,parameter_hash,domain_separator",
  "sis-hints-anoncred-pq-v0":
    "issuer_commitment,credential_commitment,showing_policy_hash,parameter_hash,domain_separator",
  "zk-ace-pq-authorization-v0":
    "identity_commitment,tx_digest,chain_id,domain_separator,action_class,replay_nullifier,policy_hash,from,to,asset,amount,verifier_key_id",
  "orchard-halo2-actions-v1":
    "anchor,nullifiers,cmx,value_commitments,binding_signature",
  "penumbra-masp-v1":
    "state_commitment_anchor,nullifiers,note_commitments,balance_commitment,asset_id_commitment",
  "monero-fcmp-plus-plus-v1":
    "membership_root,key_image_or_link_tag,amount_commitments,range_commitments,spend_authorization,chain_tag",
  "miden-stark-note-v1":
    "account_id,initial_account_commitment,final_account_commitment,input_note_nullifiers,output_note_hashes,reference_block",
  "aztec-private-rollup-v1":
    "note_hashes,nullifiers,encrypted_logs,public_call_requests,private_kernel_commitment,rollup_state_roots",
  "pq-masp-stark-v0":
    "pool_id,asset_set_root,nullifier_set,output_commitments,root,chain_tag,pq_policy_hash",
});
const REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = Object.freeze({
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
});
const REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze([
    "anonymous account commitment",
    "anonymity-set roots",
    "spent link-tag",
    "range-proof",
    "wallet account blinding",
  ]),
  "verange-transparent-range-v1": Object.freeze([
    "range-proof verifier parameters",
    "verange verifier",
    "range commitment",
    "dependent payment or credential verifier",
  ]),
  "zkat-policy-private-auth-v1": Object.freeze([
    "policy commitment registry",
    "policy epoch state",
    "authorization replay",
    "wallet policy witness",
    "typed zk::submitzkatauthorizedtransaction",
  ]),
  "zk-ams-recursive-admission-v0": Object.freeze([
    "issuer root registry",
    "admission nullifier set",
    "anonymous account commitment registry",
    "wallet admission witness",
    "typed zk-ams admission batch instruction",
  ]),
  "vega-existing-credential-zk-v0": Object.freeze([
    "credential issuer registry",
    "credential schema registry",
    "revocation or expiration policy",
    "wallet credential predicate witness",
    "typed vega credential proof instruction",
  ]),
  "silent-threshold-anoncred-v0": Object.freeze([
    "threshold issuer registry",
    "credential showing nullifier policy",
    "wallet credential showing witness",
    "anonymous credential verifier key registry",
    "typed silent-threshold credential proof instruction",
  ]),
  "zk-x509-onchain-identity-v0": Object.freeze([
    "trusted ca root registry",
    "revocation root registry",
    "certificate subject commitment registry",
    "wallet certificate witness",
    "typed zk-x.509 identity proof instruction",
  ]),
  "jindo-lattice-pcs-zk-v0": Object.freeze([
    "lattice pcs parameter registry",
    "backend verifier implementation",
    "lattice pcs verifier key registry",
    "dependent circuit integration",
  ]),
  "sis-hints-anoncred-pq-v0": Object.freeze([
    "lattice credential parameter registry",
    "credential showing verifier",
    "wallet lattice credential witness",
    "lattice credential verifier key registry",
    "typed sis-with-hints credential proof instruction",
  ]),
  "zk-ace-pq-authorization-v0": Object.freeze([
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
  ]),
  "orchard-halo2-actions-v1": Object.freeze([
    "orchard note commitment tree",
    "orchard nullifier set",
    "orchard action-bundle verifier key registry",
    "wallet orchard witness",
    "typed orchard action-bundle instruction",
  ]),
  "penumbra-masp-v1": Object.freeze([
    "multi-asset state commitment tree",
    "typed nullifier set",
    "groth16 spend/output verifier key registry",
    "wallet asset metadata witness",
    "typed penumbra shielded-pool transaction admission",
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    "full-output-set commitment accumulator",
    "spent link-tag set",
    "fcmp++ verifier key registry",
    "wallet output ownership scan state",
    "typed fcmp++ transfer instruction",
  ]),
  "miden-stark-note-v1": Object.freeze([
    "private note hash database",
    "input note nullifier set",
    "account commitment state",
    "stark vm verifier key registry",
    "wallet private note witness",
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    "private note-hash tree",
    "nullifier tree",
    "encrypted log delivery store",
    "private-kernel verifier key registry",
    "wallet private execution witness",
    "typed aztec private-rollup transaction instruction",
  ]),
  "pq-masp-stark-v0": Object.freeze([
    "pq masp asset-set commitment root",
    "pq nullifier set",
    "ml-kem encrypted note payload store",
    "wallet pq note witness",
    "active pq masp verifier key",
  ]),
});
const REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = Object.freeze([
  "malformed proof bytes",
  "wrong verifier key",
  "public input mismatch",
]);
const REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze([
    "stale or unknown anonymity-set root",
    "duplicate link tag",
    "receiver-set substitution",
  ]),
  "verange-transparent-range-v1": Object.freeze([
    "wrong bit length",
    "commitment substitution",
    "verifier-parameter mismatch",
  ]),
  "zkat-policy-private-auth-v1": Object.freeze([
    "policy-root substitution",
    "stale policy epoch",
    "authorization replay",
  ]),
  "zk-ams-recursive-admission-v0": Object.freeze([
    "duplicate credential admission",
    "wrong issuer root",
    "batch omission or account commitment substitution",
  ]),
  "vega-existing-credential-zk-v0": Object.freeze([
    "expired credential",
    "predicate mismatch",
    "wallet-binding replay",
  ]),
  "silent-threshold-anoncred-v0": Object.freeze([
    "insufficient issuer threshold",
    "issuer-set substitution",
    "credential showing replay",
  ]),
  "zk-x509-onchain-identity-v0": Object.freeze([
    "expired certificate",
    "revoked certificate",
    "stale revocation root",
  ]),
  "jindo-lattice-pcs-zk-v0": Object.freeze([
    "parameter mismatch",
    "opening claim substitution",
    "unsupported query set",
  ]),
  "sis-hints-anoncred-pq-v0": Object.freeze([
    "wrong parameter set",
    "issuer parameter substitution",
    "credential showing replay",
  ]),
  "zk-ace-pq-authorization-v0": Object.freeze([
    "transaction digest substitution",
    "chain-id or domain-separator mismatch",
    "replayed nullifier",
  ]),
  "orchard-halo2-actions-v1": Object.freeze([
    "stale anchor",
    "duplicate nullifier",
    "invalid action-bundle proof",
  ]),
  "penumbra-masp-v1": Object.freeze([
    "stale state commitment anchor",
    "duplicate nullifier",
    "asset balance commitment mismatch",
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    "stale membership root",
    "duplicate link tag",
    "amount commitment mismatch",
  ]),
  "miden-stark-note-v1": Object.freeze([
    "stale reference block",
    "duplicate input note nullifier",
    "account commitment transition mismatch",
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    "stale rollup state root",
    "duplicate nullifier",
    "private-kernel public input mismatch",
  ]),
  "pq-masp-stark-v0": Object.freeze([
    "stale asset-set root",
    "duplicate pq nullifier",
    "ml-dsa or ml-kem domain mismatch",
  ]),
});
const REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["stale or unknown anonymity-set root", "duplicate link tag", "receiver-set substitution", "range commitment mismatch", "authorization envelope mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "verange-transparent-range-v1": Object.freeze(["wrong bit length", "commitment substitution", "verifier-parameter mismatch", "oversized aggregation", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "zkat-policy-private-auth-v1": Object.freeze(["policy-root substitution", "stale policy epoch", "unauthorized signer witness", "transaction digest mismatch", "authorization replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "zk-ams-recursive-admission-v0": Object.freeze(["duplicate credential admission", "wrong issuer root", "batch omission or account commitment substitution", "recursive proof parameter mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "vega-existing-credential-zk-v0": Object.freeze(["expired credential", "wrong issuer", "predicate mismatch", "wallet-binding replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "silent-threshold-anoncred-v0": Object.freeze(["insufficient issuer threshold", "issuer-set substitution", "credential showing replay", "verifier-policy mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "zk-x509-onchain-identity-v0": Object.freeze(["expired certificate", "revoked certificate", "unknown CA root", "wrong wallet address binding", "address-binding replay", "stale revocation root", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["parameter mismatch", "opening claim substitution", "unsupported query set", "backend misclassified as production-ready", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["wrong parameter set", "issuer parameter substitution", "credential showing replay", "overclaiming production readiness from assumption research", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "zk-ace-pq-authorization-v0": Object.freeze(["transaction digest substitution", "chain-id or domain-separator mismatch", "replayed nullifier", "revoked identity commitment", "policy hash mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "orchard-halo2-actions-v1": Object.freeze(["stale anchor", "duplicate nullifier", "invalid action-bundle proof", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "penumbra-masp-v1": Object.freeze(["stale state commitment anchor", "duplicate nullifier", "asset balance commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["stale membership root", "duplicate link tag", "amount commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "miden-stark-note-v1": Object.freeze(["stale reference block", "duplicate input note nullifier", "account commitment transition mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "aztec-private-rollup-v1": Object.freeze(["stale rollup state root", "duplicate nullifier", "private-kernel public input mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
  "pq-masp-stark-v0": Object.freeze(["stale asset-set root", "duplicate PQ nullifier", "ML-DSA or ML-KEM domain mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
});
const REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "verange-transparent-range-v1": Object.freeze(["This is a component, not a complete payment protocol.", "Range parameters must be bound to the transaction payload and verifier key.", "Aggregated proof limits must be enforced by validators.", "Local verification is limited to deterministic dev fixtures; the production VeRange prover remains unavailable.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "zkat-policy-private-auth-v1": Object.freeze(["Hides authorization policy, not payment fields.", "Policy commitments require explicit epoch, replay, and rotation semantics.", "Combining with ZK-ACE requires both proofs to bind the same transaction digest.", "The SDK dev fixture verifies deterministic binding only; chain policy state and production zkAt proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "zk-ams-recursive-admission-v0": Object.freeze(["Admission privacy is separate from later payment privacy.", "Duplicate admission prevention depends on issuer-scoped nullifiers.", "Recursive batching must bind every admitted account commitment.", "The SDK dev fixture verifies deterministic binding only; chain admission state and production recursive proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "vega-existing-credential-zk-v0": Object.freeze(["Credential schema parsing must be deterministic and versioned.", "Proofs must bind to wallet or identity commitments to prevent credential replay.", "Issuer trust and revocation semantics remain external policy inputs.", "The SDK dev fixture verifies deterministic binding only; chain credential policy state and production Vega proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "silent-threshold-anoncred-v0": Object.freeze(["Credential issuance and revocation governance are as important as proof verification.", "Issuer-set commitments need rotation and downgrade protections.", "This is a credential layer, not a private payment protocol.", "The SDK dev fixture verifies deterministic binding only; chain credential state and production silent-threshold proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "zk-x509-onchain-identity-v0": Object.freeze(["Legacy X.509 trust roots are usually not post-quantum.", "Revocation root freshness must be explicit in the public inputs.", "Address binding must prevent proof replay across wallets and chains.", "The SDK dev fixture verifies deterministic public-input binding only; chain trust-root, revocation, policy state, and production ZK-X.509 proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["This is a proof backend candidate, not a transaction algorithm.", "PQ proof coverage alone does not imply PQ authorization or note encryption.", "Parameter selection and implementation security require independent review.", "The SDK dev fixture verifies deterministic public-input binding only; production Jindo lattice proving and verifier backends remain unavailable.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["This is a credential foundation, not an immediately deployable wallet protocol.", "PQ credential proof coverage does not make a payment flow end-to-end post-quantum.", "Parameter choices and reduction assumptions need explicit governance.", "The SDK dev fixture verifies deterministic public-input binding only; production SIS-with-hints credential proving and verifier backends remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "zk-ace-pq-authorization-v0": Object.freeze(["Authorization is only one PQ layer; proof backend and note encryption must also be PQ before a payment flow is end-to-end post-quantum.", "Replay nullifiers must be chain-domain separated and irreversible after acceptance.", "A dev verifier must never be accepted under a production verifier key id.", "Native AIR openings are blinded so sampled rows do not recover identity or replay witness limbs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "orchard-halo2-actions-v1": Object.freeze(["Orchard actions require circuit-compatible note/nullifier semantics and domain-separated action hashes.", "Viewing-key and outgoing-viewing metadata must remain wallet-local.", "Production readiness requires audited Halo2 parameters and note-encryption review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "penumbra-masp-v1": Object.freeze(["Typed asset values must bind asset identifiers to balance commitments.", "Groth16 parameter registration must distinguish spend and output circuits.", "Wallet note plaintexts and position metadata must not be exposed through public APIs.", "Production MASP use requires audited parameter governance and chain-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["Full-chain membership roots must be canonical and replay protected.", "Link tags/key images must be unique without revealing owned outputs.", "Range-proof and amount-commitment parameters require production verifier review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "miden-stark-note-v1": Object.freeze(["Private note data and off-chain delivery metadata must stay wallet-local.", "Account-local transition proofs must bind initial and final account commitments.", "Reference blocks must prevent replay against stale account state.", "Production Miden note transactions require audited STARK parameters and account-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "aztec-private-rollup-v1": Object.freeze(["Private-kernel proofs must bind note hashes, nullifiers, encrypted logs, and public calls.", "Encrypted log delivery metadata must not leak wallet note ownership.", "Recursive verifier registration must distinguish private-kernel versions and rollup state roots.", "Production private-rollup use requires audited private-kernel parameters and rollup-state integration review.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
  "pq-masp-stark-v0": Object.freeze(["PQ MASP combines experimental STARK/FRI proving with production PQ authorization and note encryption requirements.", "ML-DSA domains and ML-KEM ciphertext formats must be bound to verifier keys and pool identifiers.", "Post-quantum readiness still requires parameter review, parser fuzzing, and internal cryptographic review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review."]),
});
const REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze([
    Object.freeze({
      label: "Anonymous PGC with k-out-of-n Proofs",
      url: "https://eprint.iacr.org/2025/884",
    }),
  ]),
  "verange-transparent-range-v1": Object.freeze([
    Object.freeze({
      label: "VeRange: Verification-efficient Zero-knowledge Range Arguments",
      url: "https://eprint.iacr.org/2025/528",
    }),
  ]),
  "zkat-policy-private-auth-v1": Object.freeze([
    Object.freeze({
      label: "zkAt: Zero-Knowledge Authenticator for Blockchain",
      url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
    }),
  ]),
  "zk-ams-recursive-admission-v0": Object.freeze([
    Object.freeze({
      label: "ZK-AMS recursive anonymous admission",
      url: "https://arxiv.org/abs/2602.16130",
    }),
  ]),
  "vega-existing-credential-zk-v0": Object.freeze([
    Object.freeze({
      label: "Vega: Low-Latency Zero-Knowledge Proofs over Existing Credentials",
      url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
    }),
  ]),
  "silent-threshold-anoncred-v0": Object.freeze([
    Object.freeze({
      label:
        "Anonymous Credentials with Issuer-Hiding, Threshold Issuance, and Silent Setup",
      url: "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html",
    }),
  ]),
  "zk-x509-onchain-identity-v0": Object.freeze([
    Object.freeze({
      label: "ZK-X.509 on-chain identity",
      url: "https://arxiv.org/abs/2603.25190",
    }),
  ]),
  "jindo-lattice-pcs-zk-v0": Object.freeze([
    Object.freeze({
      label: "Jindo lattice-based polynomial commitment",
      url: "https://eprint.iacr.org.cn/2026/044",
    }),
  ]),
  "sis-hints-anoncred-pq-v0": Object.freeze([
    Object.freeze({
      label:
        "Tight Reductions for SIS-with-Hints Assumptions with Applications",
      url: "https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-with-hints-assumptions-with-applications/",
    }),
  ]),
  "zk-ace-pq-authorization-v0": Object.freeze([
    Object.freeze({
      label: "ZK-ACE: Practical Post-Quantum Authorization for Blockchain",
      url: "https://arxiv.org/abs/2603.07974",
    }),
  ]),
  "orchard-halo2-actions-v1": Object.freeze([
    Object.freeze({
      label: "ZIP 224 Orchard Shielded Protocol",
      url: "https://zips.z.cash/zip-0224",
    }),
    Object.freeze({
      label: "Zcash Protocol Specification",
      url: "https://zips.z.cash/protocol/protocol.pdf",
    }),
  ]),
  "penumbra-masp-v1": Object.freeze([
    Object.freeze({
      label: "Penumbra Multi-Asset Shielded Pool",
      url: "https://protocol.penumbra.zone/main/shielded_pool.html",
    }),
    Object.freeze({
      label: "Penumbra Cryptographic Primitives",
      url: "https://protocol.penumbra.zone/main/crypto.html",
    }),
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    Object.freeze({
      label: "Monero FCMP++ Development",
      url: "https://web.getmonero.org/2024/04/27/fcmps.html",
    }),
  ]),
  "miden-stark-note-v1": Object.freeze([
    Object.freeze({
      label: "Miden Transaction Model",
      url: "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
    }),
    Object.freeze({
      label: "Miden Notes",
      url: "https://docs.miden.xyz/core-concepts/miden-base/note/",
    }),
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    Object.freeze({
      label: "Aztec State Management",
      url: "https://docs.aztec.network/developers/docs/foundational-topics/state_management",
    }),
    Object.freeze({
      label: "Aztec Private Kernel Circuit",
      url: "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
    }),
  ]),
  "pq-masp-stark-v0": Object.freeze([
    Object.freeze({
      label: "NIST Post-Quantum Standards",
      url: "https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards",
    }),
    Object.freeze({
      label: "FIPS 203 ML-KEM",
      url: "https://csrc.nist.gov/pubs/fips/203/final",
    }),
    Object.freeze({
      label: "FIPS 204 ML-DSA",
      url: "https://csrc.nist.gov/pubs/fips/204/final",
    }),
    Object.freeze({
      label: "FIPS 205 SLH-DSA",
      url: "https://csrc.nist.gov/pubs/fips/205/final",
    }),
  ]),
});
const REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["buildAnonymousPgcReceiverSet", "buildAnonymousPgcDevProofFixture", "verifyAnonymousPgcDevProofLocally"]),
  "verange-transparent-range-v1": Object.freeze(["buildRangeCommitment", "buildVeRangeDevProofFixture", "buildVeRangeProofEnvelope", "verifyVeRangeProofLocally"]),
  "zkat-policy-private-auth-v1": Object.freeze(["buildZkAtPolicyCommitment", "buildZkAtAuthenticatorEnvelope", "buildZkAtDevProofFixture", "verifyZkAtAuthenticatorLocally"]),
  "zk-ams-recursive-admission-v0": Object.freeze(["buildZkAmsAdmissionBatch", "buildZkAmsAdmissionProofEnvelope", "buildZkAmsAdmissionDevProofFixture", "verifyZkAmsAdmissionProofLocally"]),
  "vega-existing-credential-zk-v0": Object.freeze(["buildVegaCredentialPredicateCommitment", "buildVegaCredentialProofEnvelope", "buildVegaCredentialDevProofFixture", "verifyVegaCredentialProofLocally"]),
  "silent-threshold-anoncred-v0": Object.freeze(["buildSilentThresholdCredentialCommitments", "buildSilentThresholdCredentialEnvelope", "buildSilentThresholdCredentialDevProofFixture", "verifySilentThresholdCredentialProofLocally"]),
  "zk-x509-onchain-identity-v0": Object.freeze(["buildZkX509IdentityCommitments", "buildZkX509IdentityEnvelope", "buildZkX509IdentityDevProofFixture", "verifyZkX509IdentityProofLocally"]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["buildJindoLatticePublicInputs", "buildJindoLatticeProofEnvelope", "buildJindoLatticeDevProofFixture", "verifyJindoLatticeProofLocally"]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["buildSisHintsCredentialCommitments", "buildSisHintsCredentialEnvelope", "buildSisHintsCredentialDevProofFixture", "verifySisHintsCredentialProofLocally"]),
  "zk-ace-pq-authorization-v0": Object.freeze(["buildRegisterZkAceIdentityCommitmentInstruction", "buildRotateZkAceIdentityCommitmentInstruction", "buildRevokeZkAceIdentityCommitmentInstruction", "buildZkAceAuthorizedTransferInstruction", "buildZkAceAuthorizationProofV1"]),
  "orchard-halo2-actions-v1": Object.freeze([]),
  "penumbra-masp-v1": Object.freeze([]),
  "monero-fcmp-plus-plus-v1": Object.freeze([]),
  "miden-stark-note-v1": Object.freeze([]),
  "aztec-private-rollup-v1": Object.freeze([]),
  "pq-masp-stark-v0": Object.freeze([]),
});
const REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze([
    "buildAnonymousPgcAccountCommitmentInstruction",
    "buildAnonymousPgcKOutOfNProofV1",
    "buildAnonymousPgcTransferInstruction",
  ]),
  "verange-transparent-range-v1": Object.freeze(["buildVeRangeProofV1"]),
  "zkat-policy-private-auth-v1": Object.freeze([
    "buildZkAtPolicyCommitmentInstruction",
    "buildZkAtPolicyProofV1",
    "buildZkAtAuthorizedTransaction",
  ]),
  "zk-ams-recursive-admission-v0": Object.freeze([
    "buildZkAmsAdmissionBatchProofV0",
    "buildSubmitZkAmsAdmissionBatchInstruction",
  ]),
  "vega-existing-credential-zk-v0": Object.freeze([
    "buildVegaCredentialPredicateProofV0",
    "buildSubmitVegaCredentialProofInstruction",
  ]),
  "silent-threshold-anoncred-v0": Object.freeze([
    "buildSilentThresholdCredentialShowingProofV0",
    "buildSubmitSilentThresholdCredentialProofInstruction",
  ]),
  "zk-x509-onchain-identity-v0": Object.freeze([
    "buildZkX509IdentityProofV0",
    "buildSubmitZkX509IdentityProofInstruction",
  ]),
  "jindo-lattice-pcs-zk-v0": Object.freeze([
    "buildJindoLatticeProofV0",
    "verifyJindoPolynomialCommitmentV0",
  ]),
  "sis-hints-anoncred-pq-v0": Object.freeze([
    "buildSisHintsAnonymousCredentialProofV0",
    "buildSubmitSisHintsCredentialProofInstruction",
  ]),
  "zk-ace-pq-authorization-v0": Object.freeze([
    "buildShieldedZkAceAuthorizationProofV1",
    "buildShieldedZkAceAuthorizedTransferInstruction",
  ]),
  "orchard-halo2-actions-v1": Object.freeze([
    "buildOrchardActionBundleProofV1",
    "buildOrchardActionBundleInstruction",
  ]),
  "penumbra-masp-v1": Object.freeze([
    "buildPenumbraSpendProofV1",
    "buildPenumbraOutputProofV1",
    "buildPenumbraShieldedPoolTransaction",
  ]),
  "monero-fcmp-plus-plus-v1": Object.freeze([
    "buildFcmpPlusPlusMembershipProofV1",
    "buildFcmpPlusPlusTransferInstruction",
  ]),
  "miden-stark-note-v1": Object.freeze([
    "buildMidenStarkTransactionProofV1",
    "buildMidenNoteTransactionInstruction",
  ]),
  "aztec-private-rollup-v1": Object.freeze([
    "buildAztecPrivateKernelProofV1",
    "buildAztecPrivateRollupTransactionInstruction",
  ]),
  "pq-masp-stark-v0": Object.freeze([
    "buildPqMaspStarkTransferProofV0",
    "buildPqMaspStarkRegisterPoolInstruction",
    "buildPqMaspStarkTransferInstruction",
    "generateMlDsaKeyPair",
    "encapsulateMlKem",
  ]),
});
const REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "verange-transparent-range-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "zkat-policy-private-auth-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "zk-ams-recursive-admission-v0": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "vega-existing-credential-zk-v0": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "silent-threshold-anoncred-v0": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "zk-x509-onchain-identity-v0": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "jindo-lattice-pcs-zk-v0": Object.freeze({ proof: true, authorization: false, note_encryption: false }),
  "sis-hints-anoncred-pq-v0": Object.freeze({ proof: true, authorization: false, note_encryption: false }),
  "zk-ace-pq-authorization-v0": Object.freeze({ proof: true, authorization: true, note_encryption: false }),
  "orchard-halo2-actions-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "penumbra-masp-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "monero-fcmp-plus-plus-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "miden-stark-note-v1": Object.freeze({ proof: true, authorization: false, note_encryption: false }),
  "aztec-private-rollup-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false }),
  "pq-masp-stark-v0": Object.freeze({ proof: true, authorization: true, note_encryption: true }),
});
const REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["anonymous account commitment accumulator", "spent link-tag set", "Anonymous PGC verifier", "range-proof component verifier", "typed zk::RegisterAnonymousPgcAccountCommitment instruction", "typed zk::SubmitAnonymousPgcTransfer instruction"]),
  "verange-transparent-range-v1": Object.freeze(["VeRange verifier registry entry", "range commitment binding rules", "dependent payment or credential verifier"]),
  "zkat-policy-private-auth-v1": Object.freeze(["zkAt policy commitment registry", "zkAt verifier", "account policy epoch state", "account policy replay protection", "typed zk::RegisterZkAtPolicyCommitment instruction", "typed zk::SubmitZkAtAuthorizedTransaction admission"]),
  "zk-ams-recursive-admission-v0": Object.freeze(["issuer root registry", "admission nullifier set", "recursive admission verifier", "typed ZK-AMS admission batch instruction"]),
  "vega-existing-credential-zk-v0": Object.freeze(["credential schema registry", "issuer registry", "credential predicate verifier", "typed Vega credential proof instruction"]),
  "silent-threshold-anoncred-v0": Object.freeze(["threshold issuer registry", "anonymous credential verifier", "credential showing replay policy", "typed silent-threshold credential proof instruction"]),
  "zk-x509-onchain-identity-v0": Object.freeze(["trusted CA root registry", "revocation root registry", "ZK-X.509 verifier", "typed ZK-X.509 identity proof instruction"]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["Jindo verifier backend", "lattice PCS parameter registry", "dependent circuit integration"]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["lattice anonymous credential verifier", "credential parameter registry", "issuer parameter registry", "typed SIS-with-hints credential proof instruction"]),
  "zk-ace-pq-authorization-v0": Object.freeze(["zk::RegisterZkAceIdentityCommitment", "zk::RotateZkAceIdentityCommitment", "zk::RevokeZkAceIdentityCommitment", "zk::SubmitZkAceAuthorizedTransfer", "active stark/fri/sha256-goldilocks ZK-ACE verifier key", "ZK-ACE identity source-account allowlist"]),
  "orchard-halo2-actions-v1": Object.freeze(["Orchard note commitment tree", "Orchard nullifier set", "Halo2 action-bundle verifier", "wallet Orchard witness store", "typed Orchard action-bundle instruction"]),
  "penumbra-masp-v1": Object.freeze(["multi-asset state commitment tree", "typed note commitment and nullifier state", "Groth16 verifier registry", "wallet multi-asset witness store", "typed Penumbra shielded-pool transaction admission"]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier", "wallet scanning and ownership recovery", "typed FCMP++ transfer instruction"]),
  "miden-stark-note-v1": Object.freeze(["STARK VM verifier", "private note hash and nullifier database", "account commitment state", "wallet private-note delivery store", "typed Miden note transaction instruction"]),
  "aztec-private-rollup-v1": Object.freeze(["private note-hash tree", "nullifier tree", "encrypted log store", "private-kernel verifier", "wallet private execution environment", "typed Aztec private-rollup transaction instruction"]),
  "pq-masp-stark-v0": Object.freeze(["STARK/FRI verifier enabled", "ML-DSA transaction authorization", "ML-KEM note payload encryption", "zk::RegisterAssetHiddenZkPool", "zk::AssetHiddenZkTransfer", "active PQ MASP verifier key"]),
});
const REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["anonymous account commitment set", "recent anonymity-set roots", "spent link-tag set", "range-proof verifier parameters", "wallet account blinding and receiver recovery metadata"]),
  "verange-transparent-range-v1": Object.freeze(["range-proof verifier parameters", "VeRange verifier key registry", "range commitment domain separators", "maximum aggregation policy"]),
  "zkat-policy-private-auth-v1": Object.freeze(["policy commitment registry", "policy epoch state", "authorization replay guard", "authorization verifier registry", "wallet policy witness store"]),
  "zk-ams-recursive-admission-v0": Object.freeze(["issuer root registry", "admission nullifier set", "anonymous account commitment registry", "recursive verifier parameters", "recursive admission verifier key registry", "wallet admission witness store"]),
  "vega-existing-credential-zk-v0": Object.freeze(["credential issuer registry", "supported credential schema registry", "predicate registry", "revocation or expiration policy", "wallet credential predicate witness store", "credential predicate commitment registry", "credential predicate verifier key registry"]),
  "silent-threshold-anoncred-v0": Object.freeze(["threshold issuer registry", "credential parameter registry", "verifier policy registry", "credential showing nullifier policy", "wallet credential showing witness store", "credential showing commitment registry", "anonymous credential verifier key registry"]),
  "zk-x509-onchain-identity-v0": Object.freeze(["trusted CA root registry", "certificate policy registry", "revocation root registry", "identity proof verifier", "wallet certificate witness store", "certificate subject commitment registry", "ZK-X.509 verifier key registry"]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["lattice PCS parameter registry", "backend verifier implementation", "lattice PCS verifier key registry", "benchmark fixtures"]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["lattice credential parameter registry", "issuer parameter registry", "credential showing verifier", "wallet lattice credential witness store", "lattice credential commitment registry", "lattice credential verifier key registry"]),
  "zk-ace-pq-authorization-v0": Object.freeze(["registered ZK-ACE identity commitment", "source-account allowlist", "authorization policy hash registry", "active ZK-ACE verifier key", "chain/domain binding state", "transfer digest binding", "replay nullifier uniqueness set", "identity rotation/revocation registry", "STARK/FRI verifier parameter floors", "wallet identity witness and replay-secret store"]),
  "orchard-halo2-actions-v1": Object.freeze(["Orchard note commitment tree", "Orchard nullifier set", "Orchard action-bundle verifier key registry", "wallet Orchard witness store"]),
  "penumbra-masp-v1": Object.freeze(["multi-asset state commitment tree", "typed nullifier set", "Groth16 spend/output verifier key registry", "wallet asset metadata witness store"]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier key registry", "wallet output ownership scan state"]),
  "miden-stark-note-v1": Object.freeze(["private note hash database", "input note nullifier set", "account commitment state", "STARK VM verifier key registry", "wallet private note witness store"]),
  "aztec-private-rollup-v1": Object.freeze(["private note-hash tree", "nullifier tree", "encrypted log delivery store", "private-kernel verifier key registry", "wallet private execution witness store"]),
  "pq-masp-stark-v0": Object.freeze(["PQ MASP asset-set commitment root", "PQ nullifier set", "ML-KEM encrypted note payload store", "wallet PQ note witness store"]),
});
const REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["Register anonymous account commitments and anonymity-set accumulator state.", "Register the k-out-of-n payment verifier key and range-proof parameters.", "Persist wallet blinding, balance-opening, and receiver recovery witnesses."]),
  "verange-transparent-range-v1": Object.freeze(["Register VeRange verifier parameters and allowed bit lengths.", "Define the commitment scheme and domain separators used by dependent algorithms."]),
  "zkat-policy-private-auth-v1": Object.freeze(["Register a hidden policy commitment and verifier key.", "Bind the policy to account action classes and epoch rules."]),
  "zk-ams-recursive-admission-v0": Object.freeze(["Register credential issuer roots and recursive verifier parameters.", "Define anonymous account commitment format and admission-nullifier derivation."]),
  "vega-existing-credential-zk-v0": Object.freeze(["Register supported credential schemas, issuers, and predicates.", "Bind credential proof subjects to wallet or ZK-ACE identity commitments."]),
  "silent-threshold-anoncred-v0": Object.freeze(["Register issuer sets, threshold policies, and credential parameters.", "Define showing-nullifier and verifier-policy binding rules."]),
  "zk-x509-onchain-identity-v0": Object.freeze(["Register trusted CA roots, certificate policies, and revocation-root feeds.", "Define wallet address binding and domain-separation rules."]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["Track lattice PCS parameter sets and verifier API shape.", "Benchmark prover, verifier, and proof-size behavior before integration."]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["Track supported SIS-with-hints parameter sets and issuer parameters.", "Define how future PQ credential showings bind to wallet or authorization contexts."]),
  "zk-ace-pq-authorization-v0": Object.freeze(["Register a ZK-ACE identity commitment, source-account allowlist, and verifier key.", "Initialize replay-state tracking for the authorizing wallet.", "Bind authorization policy hash to the allowed transaction action classes."]),
  "orchard-halo2-actions-v1": Object.freeze(["Add Orchard-compatible note, nullifier, action, and anchor data model types.", "Register Orchard Halo2 verifier parameters and action-bundle public input layout.", "Persist wallet note plaintexts, diversifiers, Merkle witnesses, and outgoing viewing data."]),
  "penumbra-masp-v1": Object.freeze(["Add typed-value notes, asset identifiers, state commitments, and nullifier state.", "Register Groth16/BLS12-377 verifier parameters for spend and output proofs.", "Persist wallet note plaintexts, asset metadata, state commitment positions, and nullifier keys."]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["Add output commitment accumulator state suitable for full-chain membership proofs.", "Define link tags/key images and spent-output rejection for Iroha assets.", "Implement wallet scanning, ownership recovery, and amount commitment witness storage."]),
  "miden-stark-note-v1": Object.freeze(["Add private note hash/nullifier state and account-local transition verification.", "Register a STARK VM verifier and public-input commitment layout.", "Persist private note data and off-chain delivery metadata in the wallet note store."]),
  "aztec-private-rollup-v1": Object.freeze(["Add private note-hash and nullifier trees plus encrypted log delivery metadata.", "Register a private-kernel verifier and public-input layout for private contract side effects.", "Persist wallet PXE-style note discovery, private call witnesses, and app-scoped nullifier keys."]),
  "pq-masp-stark-v0": Object.freeze(["Register STARK/FRI verifier parameters and PQ MASP public input layout.", "Define ML-DSA authorization domains and ML-KEM note-encryption payload formats.", "Persist wallet PQ note witnesses, nullifier positions, and encapsulation metadata."]),
});
const REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = Object.freeze({
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["Select an anonymity-set root and receiver set.", "Create balance commitments, receiver ciphertext commitments, and link tag.", "Generate the Anonymous PGC proof and submit the transfer instruction."]),
  "verange-transparent-range-v1": Object.freeze(["Build amount commitments.", "Generate a range proof bound to the transaction payload.", "Attach the range-proof envelope to the dependent confidential algorithm."]),
  "zkat-policy-private-auth-v1": Object.freeze(["Generate a policy-private authenticator proof.", "Attach the authenticator envelope to the transaction authorization path."]),
  "zk-ams-recursive-admission-v0": Object.freeze(["Collect admitted account commitments into a batch.", "Generate or import a recursive admission proof.", "Submit the batch proof and admission nullifiers."]),
  "vega-existing-credential-zk-v0": Object.freeze(["Parse the credential under a registered schema.", "Generate a predicate proof and bind it to the wallet context.", "Submit the proof envelope to the admission or authorization flow."]),
  "silent-threshold-anoncred-v0": Object.freeze(["Generate a credential showing proof under the verifier policy.", "Submit the proof as an admission or authorization component."]),
  "zk-x509-onchain-identity-v0": Object.freeze(["Generate a proof of certificate validity, ownership, and revocation status.", "Bind the proof to an institution wallet or ZK-ACE identity commitment."]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["Use as a candidate backend for future PQ circuits only after concrete circuit integration."]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["Use as a future PQ credential backend after a concrete credential protocol is selected."]),
  "zk-ace-pq-authorization-v0": Object.freeze(["Hash the transaction payload and chain/domain context.", "Derive a fresh replay nullifier.", "Generate a ZK-ACE authorization proof and submit a protected transparent transfer."]),
  "orchard-halo2-actions-v1": Object.freeze(["Select spend notes and anchors from the wallet witness store.", "Create output notes and value commitments.", "Generate one Halo2 proof over the action bundle and submit nullifiers plus commitments."]),
  "penumbra-masp-v1": Object.freeze(["Select positioned notes and derive nullifiers.", "Create typed output notes and balance commitments.", "Submit spend/output actions with proofs against the shielded pool state commitment tree."]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["Select owned outputs from the wallet scan state.", "Generate full-chain membership and amount-conservation proofs.", "Submit link tag, output commitments, range proof, and spend authorization."]),
  "miden-stark-note-v1": Object.freeze(["Execute the account-local transition against private note witnesses.", "Produce a STARK proof for the transaction script and account state delta.", "Submit note nullifiers, output note hashes, account commitments, and proof."]),
  "aztec-private-rollup-v1": Object.freeze(["Execute private contract calls locally against wallet notes.", "Accumulate note hashes, nullifiers, encrypted logs, and public-call requests in the private kernel.", "Submit the recursive private-kernel proof and side-effect commitments for validator verification."]),
  "pq-masp-stark-v0": Object.freeze(["Select PQ MASP input notes and derive nullifiers.", "Generate STARK/FRI transfer proofs with ML-DSA authorization and ML-KEM output-note encryption.", "Submit nullifiers, output commitments, PQ policy hash, and proof for verifier admission."]),
});
const BRIDGE_MISSING_REASON_SOURCES = Object.freeze([
  Object.freeze({
    label: "Java Android",
    path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    start: "PRODUCTION_GATE_MISSING =",
    end: "));",
  }),
  Object.freeze({
    label: "Kotlin JVM",
    path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    start: "val MISSING_REASONS: List<String> =",
    end: "@JvmStatic",
  }),
  Object.freeze({
    label: "Swift",
    path: "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    start: "public static let missingReasons = [",
    end: "]",
  }),
  Object.freeze({
    label: "C#",
    path: "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    start: "public static IReadOnlyList<string> MissingReasons",
    end: "});",
  }),
]);
const RUST_PRIVACY_ALGORITHM_SOURCES = Object.freeze([
  Object.freeze({
    label: "connect_norito_bridge",
    path: "crates/connect_norito_bridge/src/lib.rs",
  }),
  Object.freeze({
    label: "iroha_js_host",
    path: "crates/iroha_js_host/src/lib.rs",
  }),
  Object.freeze({
    label: "iroha_python_rs",
    path: "python/iroha_python/iroha_python_rs/src/lib.rs",
  }),
]);
const DERIVED_JS_COMPATIBILITY_FIELDS = Object.freeze([
  "hiddenFeatures",
  "hidden_features",
  "requirements",
  "limitations",
  "status",
  "unavailableReason",
  "unavailable_reason",
  "verifierKeyMetadata",
  "verifier_key_metadata",
  "backendFamily",
  "backend_family",
  "productionReady",
  "production_ready",
  "productionGate",
  "production_gate",
]);
const PUBLIC_PRIVACY_API_DECLARATION_SURFACES = Object.freeze([
  Object.freeze({
    label: "JS TypeScript declarations",
    path: JS_DECLARATIONS,
  }),
  Object.freeze({
    label: "Python package exports",
    path: "python/iroha_python/src/iroha_python/__init__.py",
  }),
  Object.freeze({
    label: "Swift privacy bridge",
    path: "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
  }),
  Object.freeze({
    label: "Java Android privacy bridge",
    path: "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
  }),
  Object.freeze({
    label: "Kotlin JVM privacy bridge",
    path: "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
  }),
  Object.freeze({
    label: "C# privacy native bridge",
    path: "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
  }),
]);
const PUBLIC_PRIVACY_API_SOURCE_SCAN_SURFACES = Object.freeze([
  Object.freeze({
    label: "JS src SDK",
    root: "javascript/iroha_js/src",
    extensions: Object.freeze([".js"]),
    language: "javascript",
  }),
  Object.freeze({
    label: "JS dist SDK",
    root: "javascript/iroha_js/dist",
    extensions: Object.freeze([".js"]),
    language: "javascript",
  }),
  Object.freeze({
    label: "Python SDK",
    root: "python/iroha_python/src/iroha_python",
    extensions: Object.freeze([".py"]),
    language: "python",
  }),
  Object.freeze({
    label: "Swift SDK",
    root: "IrohaSwift/Sources/IrohaSwift",
    extensions: Object.freeze([".swift"]),
    language: "swift",
  }),
  Object.freeze({
    label: "Java Android SDK",
    root: "java/iroha_android/src/main/java",
    extensions: Object.freeze([".java"]),
    language: "java",
  }),
  Object.freeze({
    label: "Kotlin JVM SDK",
    root: "kotlin/core-jvm/src/main/java",
    extensions: Object.freeze([".kt"]),
    language: "kotlin",
  }),
  Object.freeze({
    label: "C# SDK",
    root: "csharp/src",
    extensions: Object.freeze([".cs"]),
    language: "csharp",
  }),
]);
function snakeEntrypointName(entrypoint) {
  return entrypoint.replace(/(?<!^)(?=[A-Z])/g, "_").toLowerCase();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function publicApiNameVariants(entrypoint) {
  const snakeEntrypoint = snakeEntrypointName(entrypoint);
  return [
    entrypoint,
    `${entrypoint[0].toUpperCase()}${entrypoint.slice(1)}`,
    snakeEntrypoint,
    snakeEntrypoint.replace("ve_range", "verange"),
    snakeEntrypoint.replace("zk_at", "zkat"),
  ];
}

function rawJsPrivacyDescriptor(patch = {}) {
  const researchPatch = patch.implementationStage === "research-target-as-of-2026-05";
  return {
    id: "shield",
    name: "Shape check",
    shortName: "Shape",
    summary: "Descriptor used to test hostile catalog input validation.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: [],
    proofFamily: "shape-proof",
    publicInputsSchema: "root,domain_separator",
    verifierKeyId: "shape_verifier_v0",
    pqLayers: {
      proof: false,
      authorization: false,
      noteEncryption: false,
    },
    implementationStage: "production-hardened",
    recommendedFor: ["shape validation"],
    sourceReferences: [
      {
        label: "Shape fixture",
        url: "https://zips.z.cash/zip-0224",
      },
    ],
    securityNotes: [
      "Production readiness requires audit review for shape proof constraints.",
      "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
    ],
    requiredState: ["shape verifier key registry"],
    failureModes: ["shape proof rejected"],
    setupSteps: ["Register shape verifier key"],
    executionSteps: ["Build shape proof"],
    sdkEntrypoints: researchPatch ? [] : ["buildShapeProof"],
    plannedSdkEntrypoints: [],
    chainRequirements: ["shape verifier key registry"],
    ...patch,
  };
}

function validatorSurfaces() {
  return [
    ["src", validateSrcPrivacyAlgorithmDescriptor],
    ["dist", validateDistPrivacyAlgorithmDescriptor],
  ];
}

function assertJsValidatorsReject(patch, pattern) {
  for (const [label, validate] of validatorSurfaces()) {
    assert.throws(
      () => validate(rawJsPrivacyDescriptor(patch), 99),
      pattern,
      `${label} validator must reject hostile privacy descriptor patch ${JSON.stringify(patch)}`,
    );
  }
}

function entrypointIsDevFixture(entrypoint) {
  const normalized = entrypoint.replaceAll("-", "_").toLowerCase();
  const compact = entrypoint.toLowerCase().replace(/[^a-z0-9]/g, "");
  return (
    normalized.includes("devfixture") ||
    normalized.includes("dev_fixture") ||
    normalized.includes("devprooffixture") ||
    normalized.includes("dev_proof_fixture") ||
    normalized.includes("fixture") ||
    normalized.includes("mock") ||
    compact.includes("devfixture") ||
    compact.includes("devprooffixture") ||
    compact.includes("fixture") ||
    compact.includes("mock")
  );
}

function entrypointIsLocalVerifier(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.startsWith("verify") &&
    (entrypointNameHasEvidenceFragment(name, "Local") ||
      entrypointNameHasEvidenceFragment(name, "Locally"))
  );
}

function entrypointIsInstructionBuilder(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return entrypointNameHasTerminalEvidenceFragment(name, "Instruction");
}

function entrypointIsPlannedLedgerMutation(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    ["Instruction", "Transaction"].some((fragment) =>
      entrypointNameHasTerminalEvidenceFragment(name, fragment)
    ) || entrypointNameHasEvidenceFragment(name, "Submit")
  );
}

function entrypointIsProofHelper(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.includes("ProofEnvelope") ||
    name.includes("ProofWitness") ||
    name.includes("ProofPublicInputs") ||
    name.includes("ProofRequest") ||
    name.includes("ProofCommitment")
  );
}

function entrypointIsProductionProofBuilder(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.startsWith("build") &&
    entrypointNameHasEvidenceFragment(name, "Proof") &&
    !entrypointIsInstructionBuilder(entrypoint) &&
    !entrypointIsPlannedLedgerMutation(entrypoint) &&
    !entrypointIsProofHelper(entrypoint) &&
    !entrypointIsDevFixture(entrypoint)
  );
}

function publicInputsSchemaHasChainDomainBinding(value) {
  return value.split(",").some((token) =>
    PUBLIC_INPUT_SCHEMA_CHAIN_DOMAIN_BINDING_TOKEN_FRAGMENTS.some((fragment) =>
      publicInputSchemaTokenHasFragment(token, fragment),
    )
  );
}

function entrypointNameHasEvidenceFragment(name, fragment) {
  let index = name.indexOf(fragment);
  while (index !== -1) {
    const prefix = name.slice(0, index);
    const suffix = name.slice(index + fragment.length);
    const hasPrefixBoundary = index === 0 || /^[A-Za-z0-9]$/u.test(name[index - 1]);
    const hasSuffixBoundary = suffix === "" || /^[A-Z0-9]$/u.test(suffix[0]);
    const hasForbiddenPrefix = FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES.some(
      (forbiddenPrefix) => prefix.endsWith(forbiddenPrefix),
    );
    if (hasPrefixBoundary && hasSuffixBoundary && !hasForbiddenPrefix) {
      return true;
    }
    index = name.indexOf(fragment, index + 1);
  }
  return false;
}

function entrypointNameHasTerminalEvidenceFragment(name, fragment) {
  let index = name.indexOf(fragment);
  while (index !== -1) {
    const prefix = name.slice(0, index);
    const suffix = name.slice(index + fragment.length);
    const hasPrefixBoundary = index === 0 || /^[A-Za-z0-9]$/u.test(name[index - 1]);
    const hasTerminalSuffix = suffix === "" || /^V[0-9]+$/u.test(suffix);
    const hasForbiddenPrefix = FORBIDDEN_ENTRYPOINT_EVIDENCE_FRAGMENT_PREFIXES.some(
      (forbiddenPrefix) => prefix.endsWith(forbiddenPrefix),
    );
    if (hasPrefixBoundary && hasTerminalSuffix && !hasForbiddenPrefix) {
      return true;
    }
    index = name.indexOf(fragment, index + 1);
  }
  return false;
}

function plannedEntrypointNameHasPrimitiveFragment(name, fragment) {
  return entrypointNameHasEvidenceFragment(name, fragment);
}

function publicInputSchemaTokenHasFragment(token, fragment) {
  const tokenSegments = token.split("_");
  const fragmentSegments = fragment.split("_");
  for (let index = 0; index <= tokenSegments.length - fragmentSegments.length; index += 1) {
    const matchesFragment = fragmentSegments.every(
      (segment, offset) => tokenSegments[index + offset] === segment,
    );
    if (!matchesFragment) {
      continue;
    }
    const hasForbiddenPrefix = tokenSegments
      .slice(0, index)
      .some((prefix) => PUBLIC_INPUT_SCHEMA_FORBIDDEN_EVIDENCE_PREFIXES.includes(prefix));
    if (!hasForbiddenPrefix) {
      return true;
    }
  }
  return false;
}

function catalogTextContainsBoundedToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    const after = index + token.length >= value.length ? "" : value[index + token.length];
    if (!isAsciiAlnum(before) && !isAsciiAlnum(after)) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextValuesContainBoundedToken(values, token) {
  return values.some((value) => catalogTextContainsBoundedToken(value, token));
}

function catalogTextValuesContainAffirmedMetadataToken(values, token) {
  return values.some((value) => catalogTextContainsAffirmedMetadataToken(value, token));
}

function catalogTextContainsMetadataToken(value, token) {
  return token === "zk::"
    ? catalogTextContainsNamespaceToken(value, token)
    : catalogTextContainsBoundedToken(value, token);
}

function catalogTextContainsTypedAdmissionToken(value, token) {
  return token === "zk::"
    ? catalogTextContainsAffirmedNamespaceToken(value, token)
    : catalogTextContainsAffirmedMetadataToken(value, token);
}

function catalogTextContainsAffirmedNamespaceToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    if (
      !isAsciiAlnum(before) &&
      before !== "_" &&
      !catalogTextHasForbiddenEvidencePrefix(
        value,
        index,
        AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES,
      )
    ) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextContainsAffirmedMetadataToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    const after = index + token.length >= value.length ? "" : value[index + token.length];
    if (
      !isAsciiAlnum(before) &&
      !isAsciiAlnum(after) &&
      !catalogTextHasForbiddenEvidencePrefix(
        value,
        index,
        AFFIRMED_METADATA_FORBIDDEN_EVIDENCE_PREFIXES,
      )
    ) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextContainsWalletWitnessPrivacyToken(value, token) {
  if (token.startsWith("not ") || token.startsWith("must not ") || token === "never leave") {
    return catalogTextContainsMetadataToken(value, token);
  }
  if (catalogTextContainsAffirmedMetadataToken(value, token)) {
    return true;
  }
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    const after = index + token.length >= value.length ? "" : value[index + token.length];
    if (
      !isAsciiAlnum(before) &&
      !isAsciiAlnum(after) &&
      catalogTextHasPositiveWalletWitnessPrivacyPrefix(value, index)
    ) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextHasPositiveWalletWitnessPrivacyPrefix(value, index) {
  const segments = value.slice(0, index).toLowerCase().match(/[a-z0-9]+/gu) ?? [];
  const tail = segments.slice(-5).join(" ");
  return WALLET_WITNESS_POSITIVE_NEGATION_PREFIXES.some((prefix) => tail.endsWith(prefix));
}

function catalogTextContainsChainDomainBindingToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    const after = index + token.length >= value.length ? "" : value[index + token.length];
    if (
      !isAsciiAlnum(before) &&
      !isAsciiAlnum(after) &&
      !catalogTextHasForbiddenEvidencePrefix(
        value,
        index,
        CHAIN_DOMAIN_BINDING_FORBIDDEN_EVIDENCE_PREFIXES,
      )
    ) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextContainsSourceHardeningToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    const after = index + token.length >= value.length ? "" : value[index + token.length];
    if (
      !isAsciiAlnum(before) &&
      !isAsciiAlnum(after) &&
      !catalogTextHasForbiddenEvidencePrefix(
        value,
        index,
        SOURCE_REFERENCED_HARDENING_FORBIDDEN_EVIDENCE_PREFIXES,
      )
    ) {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function catalogTextHasForbiddenEvidencePrefix(value, index, forbiddenPrefixes) {
  const prefix = value.slice(0, index).toLowerCase();
  const segments = prefix.match(/[a-z0-9]+/gu) ?? [];
  return segments.slice(-3).some((segment) => forbiddenPrefixes.includes(segment));
}

function catalogTextContainsNamespaceToken(value, token) {
  let index = value.indexOf(token);
  while (index !== -1) {
    const before = index === 0 ? "" : value[index - 1];
    if (!isAsciiAlnum(before) && before !== "_") {
      return true;
    }
    index = value.indexOf(token, index + 1);
  }
  return false;
}

function isAsciiAlnum(value) {
  return /^[A-Za-z0-9]$/u.test(value);
}

function entrypointIsExplicitDevFixture(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    entrypointNameHasTerminalEvidenceFragment(name, "DevFixture") ||
    entrypointNameHasTerminalEvidenceFragment(name, "DevProofFixture")
  );
}

function loadPythonPrivacyCatalog() {
  const script = `
import importlib.util
import json
import sys

path = sys.argv[1]
spec = importlib.util.spec_from_file_location("privacy_catalog_direct", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
print(json.dumps({
    "criteria": module.get_privacy_criteria(),
    "descriptors": module.get_privacy_algorithm_descriptors(),
    "backend_family_items": list(module.BACKEND_FAMILY_BY_ALGORITHM_ID.items()),
}, sort_keys=False))
`;
  const result = spawnSync("python3", ["-c", script, PYTHON_PRIVACY_CATALOG], {
    encoding: "utf8",
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
  });
  assert.equal(
    result.status,
    0,
    `python privacy catalog loader failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return JSON.parse(result.stdout);
}

function toPythonDescriptorShape(descriptor) {
  return {
    id: descriptor.id,
    name: descriptor.name,
    short_name: descriptor.shortName,
    summary: descriptor.summary,
    category: descriptor.category,
    maturity: descriptor.maturity,
    covered_criteria: descriptor.coveredCriteria,
    proof_family: descriptor.proofFamily,
    public_inputs_schema: descriptor.publicInputsSchema,
    verifier_key_id: descriptor.verifierKeyId,
    backend_family: descriptor.backendFamily,
    pq_layers: {
      proof: descriptor.pqLayers.proof,
      authorization: descriptor.pqLayers.authorization,
      note_encryption: descriptor.pqLayers.noteEncryption,
    },
    implementation_stage: descriptor.implementationStage,
    recommended_for: descriptor.recommendedFor,
    source_references: descriptor.sourceReferences,
    security_notes: descriptor.securityNotes,
    required_state: descriptor.requiredState,
    failure_modes: descriptor.failureModes,
    setup_steps: descriptor.setupSteps,
    execution_steps: descriptor.executionSteps,
    sdk_entrypoints: descriptor.sdkEntrypoints,
    planned_sdk_entrypoints: descriptor.plannedSdkEntrypoints,
    chain_requirements: descriptor.chainRequirements,
    production_ready: descriptor.productionReady,
    production_gate: {
      version: descriptor.productionGate.version,
      ready: descriptor.productionGate.ready,
      gates: descriptor.productionGate.gates,
      missing: descriptor.productionGate.missing,
      audit_references: descriptor.productionGate.auditReferences,
    },
  };
}

function assertFailClosedDescriptor(label, descriptor) {
  const expectedGateEntries = PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]);
  const expectedMissingReasons = [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    ...SUPPLEMENTAL_FAIL_CLOSED_REASONS.filter((reason) =>
      descriptor.production_gate.missing.includes(reason),
    ),
  ];

  assert.equal(
    descriptor.production_gate.version,
    PRODUCTION_GATE_VERSION,
    `${label} ${descriptor.id} production gate version drifted`,
  );
  assert.equal(
    descriptor.production_ready,
    false,
    `${label} ${descriptor.id} must not claim production readiness`,
  );
  assert.equal(
    descriptor.production_gate.ready,
    false,
    `${label} ${descriptor.id} production gate must fail closed`,
  );
  assert.deepEqual(
    descriptor.production_gate.audit_references,
    [],
    `${label} ${descriptor.id} must not claim audit references before signoff`,
  );
  assert.deepEqual(
    descriptor.production_gate.gates,
    Object.fromEntries(expectedGateEntries),
    `${label} ${descriptor.id} production gate keys must be stable and fail closed`,
  );
  assert.deepEqual(
    Object.entries(descriptor.production_gate.gates),
    expectedGateEntries,
    `${label} ${descriptor.id} production gate keys must stay in canonical order`,
  );
  assert.deepEqual(
    Object.values(descriptor.production_gate.gates),
    Object.values(descriptor.production_gate.gates).map(() => false),
    `${label} ${descriptor.id} must keep every production gate false`,
  );
  assert.equal(
    new Set(descriptor.production_gate.missing).size,
    descriptor.production_gate.missing.length,
    `${label} ${descriptor.id} production gate missing reasons must not contain duplicates`,
  );
  for (const missing of [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    "Iroha production allowlist is not enabled for this audited row",
  ]) {
    assert.ok(
      descriptor.production_gate.missing.includes(missing),
      `${label} ${descriptor.id} missing production gate reason ${missing}`,
    );
  }
  assert.deepEqual(
    descriptor.production_gate.missing,
    expectedMissingReasons,
    `${label} ${descriptor.id} production gate missing reasons must stay canonical and ordered`,
  );
}

function canonicalBridgeMissingReasons() {
  return [
    ...PRODUCTION_GATE_REQUIRED_REASONS,
    ...SUPPLEMENTAL_FAIL_CLOSED_REASONS,
  ];
}

function fileText(relativePath) {
  return readFileSync(new URL(relativePath, `file://${REPO_ROOT}/`), "utf8");
}

function sourceFilesUnder(relativeRoot, extensions) {
  const files = [];
  const ignoredDirectories = new Set([".git", ".gradle", ".swiftpm", "bin", "build", "dist", "node_modules", "obj"]);
  const walk = (absoluteDirectory, relativeDirectory) => {
    for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!ignoredDirectories.has(entry.name)) {
          walk(
            `${absoluteDirectory}/${entry.name}`,
            relativeDirectory === "" ? entry.name : `${relativeDirectory}/${entry.name}`,
          );
        }
        continue;
      }
      if (!entry.isFile()) {
        continue;
      }
      if (extensions.some((extension) => entry.name.endsWith(extension))) {
        files.push(
          relativeDirectory === ""
            ? `${relativeRoot}/${entry.name}`
            : `${relativeRoot}/${relativeDirectory}/${entry.name}`,
        );
      }
    }
  };
  walk(`${REPO_ROOT}/${relativeRoot}`, "");
  return files.sort();
}

function publicDeclarationPatterns(language, name) {
  const escaped = escapeRegExp(name);
  switch (language) {
    case "javascript":
      return [
        new RegExp(`\\bexport\\s+(?:async\\s+)?function\\s+${escaped}\\s*\\(`),
        new RegExp(`\\bexport\\s+(?:const|let|var)\\s+${escaped}\\b`),
        new RegExp(`\\bexport\\s*\\{[^}]*\\b${escaped}\\b[^}]*\\}`),
      ];
    case "python":
      return [
        new RegExp(`^(?:def\\s+${escaped}\\s*\\(|${escaped}\\s*=)`, "m"),
      ];
    case "swift":
      return [
        new RegExp(`^\\s*public\\s+(?:static\\s+)?func\\s+${escaped}\\s*\\(`, "m"),
        new RegExp(`^\\s*public\\s+(?:static\\s+)?(?:let|var)\\s+${escaped}\\b`, "m"),
      ];
    case "java":
      return [
        new RegExp(
          `^\\s*public\\s+(?:static\\s+)?(?:final\\s+)?[\\w<>\\[\\].?,\\s]+\\s+${escaped}\\s*\\(`,
          "m",
        ),
      ];
    case "kotlin":
      return [
        new RegExp(`^\\s*(?!(?:private|internal)\\b)(?:public\\s+)?fun\\s+${escaped}\\s*\\(`, "m"),
        new RegExp(
          `^\\s*(?!(?:private|internal)\\b)(?:public\\s+)?(?:val|var)\\s+${escaped}\\b`,
          "m",
        ),
      ];
    case "csharp":
      return [
        new RegExp(
          `^\\s*public\\s+(?:static\\s+)?[\\w<>\\[\\].?,\\s]+\\s+${escaped}\\s*(?:\\(|\\{)`,
          "m",
        ),
      ];
    default:
      throw new Error(`unsupported source declaration scan language ${language}`);
  }
}

function publicPrivacyApiSourceTexts() {
  return PUBLIC_PRIVACY_API_SOURCE_SCAN_SURFACES.flatMap((surface) =>
    sourceFilesUnder(surface.root, surface.extensions).map((path) => ({
      ...surface,
      path,
      text: fileText(path),
    })),
  );
}

function assertPythonCatalogDefensiveCopyCoverage() {
  const text = fileText("python/iroha_python/tests/privacy_catalog_test.py");
  for (const snippet of [
    'descriptors[0]["pq_layers"]["proof"] = "tampered"',
    'descriptors[0]["production_gate"]["audit_references"].append(',
    'planned["planned_sdk_entrypoints"].clear()',
    'source_descriptor["source_references"][0]["url"] = "https://audit.example/forged"',
    'descriptor["source_references"][0]["label"] = "forged source"',
    'capabilities["privacy_algorithms"][0]["pq_layers"]["proof"] = "tampered"',
    'source_descriptor["source_references"].append(',
  ]) {
    assert.ok(
      text.includes(snippet),
      `Python privacy catalog defensive-copy coverage missing ${snippet}`,
    );
  }
}

function assertPythonZkAceProofBuilderCoverage() {
  const pythonCatalogSource = fileText(
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
  );
  const pythonCrypto = fileText("python/iroha_python/src/iroha_python/crypto.py");
  const pythonPackageRoot = fileText("python/iroha_python/src/iroha_python/__init__.py");
  const pythonReadme = fileText("python/iroha_python/README.md");
  const pythonCatalogTests = fileText("python/iroha_python/tests/privacy_catalog_test.py");

  assert.match(
    pythonCatalogSource,
    /zk_ace_prover\s*=\s*_callable_on_crypto\(\s*"build_zk_ace_authorization_proof_v1"\s*\)\s*and\s*_callable_on_crypto\("zk_ace_build_transfer_authorization_v1"\)/,
    "Python privacy capabilities must require both ZK-ACE proof-builder names",
  );
  assert.match(
    pythonCrypto,
    /def\s+build_zk_ace_authorization_proof_v1\(\*\*kwargs:\s*Any\)\s*->\s*Dict\[str,\s*Any\]:[\s\S]*return\s+zk_ace_build_transfer_authorization_v1\(\*\*kwargs\)/,
    "Python catalog-named ZK-ACE proof builder must delegate to the native-backed builder",
  );
  for (const [label, text] of [
    ["Python crypto exports", pythonCrypto],
    ["Python package root exports", pythonPackageRoot],
    ["Python README", pythonReadme],
  ]) {
    assert.ok(
      text.includes("build_zk_ace_authorization_proof_v1") &&
        text.includes("zk_ace_build_transfer_authorization_v1"),
      `${label} must expose both ZK-ACE proof-builder names`,
    );
  }
  for (const snippet of [
    "def test_zk_ace_python_capabilities_require_both_proof_builder_names(",
    '"build_zk_ace_authorization_proof_v1"',
    '"zk_ace_build_transfer_authorization_v1"',
    'assert capabilities["zk_ace_authorization_proof_v1"] is False',
    'assert capabilities["zk_ace_sdk_exports_v1"] is False',
    "test_privacy_capabilities_reports_native_bridge_without_production_claims",
    "zk_ace_capability = next(",
    'assert zk_ace_capability["proof_family"] == "stark/fri/sha256-goldilocks"',
    'assert zk_ace_capability["backend_family"] == "stark-fri"',
    'assert zk_ace_capability["production_gate"]["audit_references"] == []',
    "test_privacy_catalog_enforces_execution_and_metadata_invariants",
    'assert zk_ace["proof_family"] == "stark/fri/sha256-goldilocks"',
    'assert zk_ace["backend_family"] == "stark-fri"',
    'assert zk_ace["production_gate"]["audit_references"] == []',
    'assert all(ready is False for ready in zk_ace["production_gate"]["gates"].values())',
    "Iroha production allowlist is not enabled for this audited row",
  ]) {
    assert.ok(
      pythonCatalogTests.includes(snippet),
      `Python ZK-ACE capability fail-closed coverage missing ${snippet}`,
    );
  }
  assert.match(
    pythonCatalogTests,
    /test_zk_ace_python_catalog_named_proof_builder_delegates[\s\S]*test_zk_ace_python_catalog_named_proof_builder_propagates_native_errors[\s\S]*test_zk_ace_python_transfer_authorization_rejects_non_object_native_payload/,
    "Python tests must cover ZK-ACE alias delegation, missing-native propagation, and malformed native prover payloads",
  );
}

function assertZkAceJsBuilderAmountCoverage() {
  const instructionBuilderTests = fileText("javascript/iroha_js/test/instructionBuilders.test.js");
  for (const [label, text] of [
    ["JS source instruction builders", fileText("javascript/iroha_js/src/instructionBuilders.js")],
    ["JS dist instruction builders", fileText("javascript/iroha_js/dist/instructionBuilders.js")],
  ]) {
    assert.match(
      text,
      /function asPositiveU128JsonNumber\(value, name\)[\s\S]*const amount = asU128JsonNumber\(value, name\)[\s\S]*amount <= 0[\s\S]*must be greater than zero[\s\S]*function normalizeZkAcePublicInputs[\s\S]*amount: asPositiveU128JsonNumber\(source\.amount, `\$\{name\}\.amount`\)[\s\S]*function buildZkAceAuthorizedTransferInstruction[\s\S]*amount: asPositiveU128JsonNumber\(source\.amount, "zkAceAuthorizedTransfer\.amount"\)/,
      `${label} must require positive ZK-ACE proof and transfer amounts`,
    );
  }
  for (const snippet of [
    "descriptorTest(\"ZK-ACE builders reject malformed proof and replay inputs\"",
    "ZK-ACE builders reject malformed proof and replay inputs",
    "must be greater than zero",
    "Number.MAX_SAFE_INTEGER + 1",
    "BigInt(Number.MAX_SAFE_INTEGER) + 1n",
    "{ toString: () => \"17\" }",
    "canonicalAmountTransfer.amount, 17",
    "buildZkAceAuthorizationProofV1({",
    "buildZkAceAuthorizedTransferInstruction({",
  ]) {
    assert.ok(
      instructionBuilderTests.includes(snippet),
      `JS ZK-ACE positive-amount coverage missing ${snippet}`,
    );
  }
}

function assertZkAcePythonTransactionAmountCoverage() {
  const pythonTx = fileText("python/iroha_python/src/iroha_python/tx.py");
  const pythonClient = fileText("python/iroha_python/src/iroha_python/client.py");
  const pythonClientTests = fileText("python/iroha_python/tests/client_ledger_helpers_test.py");

  assert.match(
    pythonTx,
    /PositiveU128Like = Union\[str, int\][\s\S]*_U128_MAX = \(1 << 128\) - 1[\s\S]*def _normalize_positive_u128_literal\(quantity: Any, context: str\) -> str:[\s\S]*isinstance\(quantity, bool\)[\s\S]*text\.isdecimal\(\)[\s\S]*value <= 0 or value > _U128_MAX[\s\S]*def zk_ace_authorized_transfer[\s\S]*amount: PositiveU128Like[\s\S]*_normalize_positive_u128_literal\(amount, "amount"\)/,
    "Python transaction draft must require positive decimal u128 ZK-ACE transfer amounts",
  );
  assert.match(
    pythonClient,
    /def zk_ace_authorized_transfer_and_wait[\s\S]*amount: Union\[str, int\]/,
    "Python client ZK-ACE helper must expose the strict positive-u128 amount contract",
  );
  for (const snippet of [
    "test_zk_ace_transaction_amount_normalizer_matches_proof_builder_boundary",
    "_normalize_positive_u128_literal",
    "\"00017\"",
    "str((1 << 128) - 1)",
    "Decimal(\"1\")",
    "\"1e3\"",
    "str(1 << 128)",
    "id=\"zk-ace-transfer-zero-amount\"",
    "positive decimal u128",
  ]) {
    assert.ok(
      pythonClientTests.includes(snippet),
      `Python ZK-ACE transaction amount coverage missing ${snippet}`,
    );
  }
}

function assertZkAceExecutableDescriptorShape(label, descriptor) {
  assert.equal(
    descriptor.implementationStage,
    "chain-executable",
    `${label} ZK-ACE descriptor must remain chain-executable`,
  );
  assert.equal(
    descriptor.backendFamily,
    "stark-fri",
    `${label} ZK-ACE descriptor must stay on the STARK/FRI backend`,
  );
  assert.equal(
    descriptor.proofFamily,
    "stark/fri/sha256-goldilocks",
    `${label} ZK-ACE descriptor must expose the concrete STARK/FRI SHA-256 Goldilocks verifier profile`,
  );
  assert.deepEqual(
    descriptor.sdkEntrypoints,
    [
      "buildRegisterZkAceIdentityCommitmentInstruction",
      "buildRotateZkAceIdentityCommitmentInstruction",
      "buildRevokeZkAceIdentityCommitmentInstruction",
      "buildZkAceAuthorizedTransferInstruction",
      "buildZkAceAuthorizationProofV1",
    ],
    `${label} ZK-ACE descriptor must advertise the executable transparent authorization proof builder`,
  );
  assert.deepEqual(
    descriptor.plannedSdkEntrypoints,
    [
      "buildShieldedZkAceAuthorizationProofV1",
      "buildShieldedZkAceAuthorizedTransferInstruction",
    ],
    `${label} ZK-ACE descriptor must keep shielded builders planned until production gates pass`,
  );
  assert.ok(
    !descriptor.plannedSdkEntrypoints.includes("buildZkAceAuthorizationProofV0"),
    `${label} ZK-ACE descriptor must not retain stale v0 proof-builder drift`,
  );
  assert.deepEqual(
    descriptor.requiredState,
    [
      "registered ZK-ACE identity commitment",
      "source-account allowlist",
      "authorization policy hash registry",
      "active ZK-ACE verifier key",
      "chain/domain binding state",
      "transfer digest binding",
      "replay nullifier uniqueness set",
      "identity rotation/revocation registry",
      "STARK/FRI verifier parameter floors",
      "wallet identity witness and replay-secret store",
    ],
    `${label} ZK-ACE descriptor must pin every production admission state gate`,
  );
  assert.deepEqual(
    descriptor.pqLayers,
    {
      proof: true,
      authorization: true,
      noteEncryption: false,
    },
    `${label} ZK-ACE descriptor must keep note encryption out of PQ coverage`,
  );
  assert.equal(
    descriptor.coveredCriteria.includes("post_quantum"),
    false,
    `${label} ZK-ACE descriptor must not claim full post-quantum coverage`,
  );
  assert.equal(
    descriptor.productionReady,
    false,
    `${label} ZK-ACE descriptor must remain fail-closed until audited production gates pass`,
  );
  assert.equal(
    descriptor.productionGate.ready,
    false,
    `${label} ZK-ACE production gate must remain closed`,
  );
  assert.deepEqual(
    descriptor.productionGate.auditReferences,
    [],
    `${label} ZK-ACE production gate must not claim audit references before signoff`,
  );
  assert.deepEqual(
    Object.entries(descriptor.productionGate.gates),
    PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]),
    `${label} ZK-ACE production gate must keep every required gate false`,
  );
  assert.ok(
    descriptor.productionGate.missing.includes("planned SDK entrypoints remain"),
    `${label} ZK-ACE production gate must report planned shielded SDK entrypoints`,
  );
  assert.ok(
    descriptor.productionGate.missing.includes(
      "Iroha production allowlist is not enabled for this audited row",
    ),
    `${label} ZK-ACE production gate must not inherit verifier-backend allowlist admission`,
  );
  assert.deepEqual(
    descriptor.productionGate.missing,
    [
      ...PRODUCTION_GATE_REQUIRED_REASONS,
      "implementation stage is not production-hardened",
      "planned SDK entrypoints remain",
      "Iroha production allowlist is not enabled for this audited row",
    ],
    `${label} ZK-ACE production gate must stay fail-closed despite the STARK/FRI verifier profile allowlist`,
  );
}

function pythonDescriptorToJsShape(descriptor) {
  return {
    implementationStage: descriptor.implementation_stage,
    backendFamily: descriptor.backend_family,
    proofFamily: descriptor.proof_family,
    requiredState: descriptor.required_state,
    sdkEntrypoints: descriptor.sdk_entrypoints,
    plannedSdkEntrypoints: descriptor.planned_sdk_entrypoints,
    pqLayers: {
      proof: descriptor.pq_layers.proof,
      authorization: descriptor.pq_layers.authorization,
      noteEncryption: descriptor.pq_layers.note_encryption,
    },
    coveredCriteria: descriptor.covered_criteria,
    productionReady: descriptor.production_ready,
    productionGate: {
      ready: descriptor.production_gate.ready,
      gates: descriptor.production_gate.gates,
      missing: descriptor.production_gate.missing,
      auditReferences: descriptor.production_gate.audit_references,
    },
  };
}

function assertZkAceCapabilitySurfaceFailClosed(label, capabilities) {
  const zkAceCapability = capabilities.privacyAlgorithms.find(
    (descriptor) => descriptor.id === "zk-ace-pq-authorization-v0",
  );
  assert.ok(zkAceCapability, `${label} ZK-ACE capability descriptor must exist`);
  assertZkAceExecutableDescriptorShape(`${label} capability`, zkAceCapability);
  assert.equal(
    Object.isFrozen(zkAceCapability),
    true,
    `${label} ZK-ACE capability descriptor must be frozen`,
  );
  assert.equal(
    Object.isFrozen(zkAceCapability.productionGate),
    true,
    `${label} ZK-ACE capability production gate must be frozen`,
  );
  assert.equal(
    Object.isFrozen(zkAceCapability.productionGate.gates),
    true,
    `${label} ZK-ACE capability production gate bits must be frozen`,
  );
  assert.equal(
    Object.isFrozen(zkAceCapability.productionGate.missing),
    true,
    `${label} ZK-ACE capability missing reasons must be frozen`,
  );
  assert.equal(
    Object.isFrozen(zkAceCapability.productionGate.auditReferences),
    true,
    `${label} ZK-ACE capability audit references must be frozen`,
  );
  assert.equal(
    zkAceCapability.productionReady,
    false,
    `${label} ZK-ACE capability must stay fail-closed through getPrivacyCapabilities`,
  );
  assert.equal(
    zkAceCapability.productionGate.ready,
    false,
    `${label} ZK-ACE capability production gate must stay closed through getPrivacyCapabilities`,
  );
  assert.throws(() => {
    zkAceCapability.productionReady = true;
  });
  assert.throws(() => {
    zkAceCapability.productionGate.gates.external_audit = true;
  });
  assert.throws(() => {
    zkAceCapability.productionGate.auditReferences.push({
      label: "forged capability audit",
      url: "https://audit.example/forged-capability",
    });
  });
  return zkAceCapability;
}

function extractJsBackendFamilyEntries(text, label) {
  const block = requireMatch(
    text,
    /const BACKEND_FAMILY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);/,
    `${label} backend family registration map`,
  )[1];
  const entries = [...block.matchAll(/^\s*(?:"([^"]+)"|([A-Za-z_$][\w$]*)):\s*"([^"]+)",?$/gm)]
    .map((match) => [match[1] ?? match[2], match[3]]);

  assert.ok(entries.length > 0, `${label} backend family registration map is empty`);
  assert.equal(
    new Set(entries.map(([id]) => id)).size,
    entries.length,
    `${label} backend family registration map contains duplicate ids`,
  );
  return entries;
}

function isBackendFamilyName(value) {
  return /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value) && !value.includes("--");
}

function assertBackendFamilyRegistrationParity(pythonCatalog) {
  const expected = pythonCatalog.descriptors.map((descriptor) => [
    descriptor.id,
    descriptor.backend_family,
  ]);

  assert.deepEqual(
    pythonCatalog.backend_family_items,
    expected,
    "Python backend-family registration map must exactly match catalog row order",
  );
  for (const source of [
    Object.freeze({
      label: "JS src",
      path: "javascript/iroha_js/src/privacyAlgorithms.js",
    }),
    Object.freeze({
      label: "JS dist",
      path: "javascript/iroha_js/dist/privacyAlgorithms.js",
    }),
  ]) {
    assert.ok(
      fileText(source.path).includes(
        "privacy algorithm backend-family registration must exactly match catalog ids",
      ),
      `${source.label} must keep runtime backend-family registration exactness guard`,
    );
    assert.ok(
      fileText(source.path).includes(
        "catalogLabelClaimsProductionReadiness(backendFamily)",
      ),
      `${source.label} must reject backend-family production/mainnet/audit claim labels`,
    );
    assert.ok(
      fileText(source.path).includes("function isBackendFamilyName(value)") &&
        fileText(source.path).includes("!isBackendFamilyName(backendFamily)"),
      `${source.label} must reject backend-family labels that cannot be encoded as vk_ref backend components`,
    );
    assert.match(
      fileText(source.path),
      /function\s+isBackendFamilyName\([^)]*\)\s*\{[\s\S]*\^\[a-z0-9\]\(\?:\[a-z0-9-\]\*\[a-z0-9\]\)\?\$[\s\S]*!value\.includes\("--"\)/,
      `${source.label} must reject uppercase, dotted, underscored, and repeated-separator backend-family aliases before vk_ref binding`,
    );
    assert.ok(
      fileText(source.path).includes("compactProductionClaimText(value)") &&
        fileText(source.path).includes("PRODUCTION_CLAIM_CONFUSABLES"),
      `${source.label} must fold Unicode-confusable production/mainnet/audit claim labels before compact matching`,
    );
    assert.deepEqual(
      extractJsBackendFamilyEntries(fileText(source.path), source.label),
      expected,
      `${source.label} backend-family registration map drifted from Python catalog`,
    );
    for (const [algorithmId, backendFamily] of extractJsBackendFamilyEntries(
      fileText(source.path),
      source.label,
    )) {
      assert.ok(
        isBackendFamilyName(backendFamily),
        `${source.label} backend family for ${algorithmId} must be a vk_ref backend component`,
      );
    }
    assert.equal(
      isBackendFamilyName("Halo2-ipa-pasta"),
      false,
      `${source.label} backend family validator must reject uppercase aliases`,
    );
    for (const backendFamily of [
      "halo2.ipa.pasta",
      "halo2_ipa_pasta",
      "halo2--ipa-pasta",
    ]) {
      assert.equal(
        isBackendFamilyName(backendFamily),
        false,
        `${source.label} backend family validator must reject non-canonical separator ${backendFamily}`,
      );
    }
    for (const backendFamily of [
      ".halo2-ipa-pasta",
      "-halo2-ipa-pasta",
      "_halo2-ipa-pasta",
      "halo2-ipa-pasta.",
      "halo2-ipa-pasta-",
      "halo2-ipa-pasta_",
    ]) {
      assert.equal(
        isBackendFamilyName(backendFamily),
        false,
        `${source.label} backend family validator must reject edge separator ${backendFamily}`,
      );
    }
  }
  const pythonCatalogSource = fileText("python/iroha_python/src/iroha_python/privacy_catalog.py");
  assert.ok(
    pythonCatalogSource.includes("_compact_production_claim_text") &&
      pythonCatalogSource.includes("_PRODUCTION_CLAIM_CONFUSABLES"),
    "Python privacy catalog must fold Unicode-confusable production/mainnet/audit claim labels before compact matching",
  );
  assert.ok(
    pythonCatalogSource.includes("def _is_backend_family_name(value: str) -> bool") &&
      pythonCatalogSource.includes("_is_backend_family_name(backend_family)"),
    "Python privacy catalog must reject backend-family labels that cannot be encoded as vk_ref backend components",
  );
}

function requireMatch(text, pattern, label) {
  const match = text.match(pattern);
  assert.notEqual(match, null, `${label} missing pattern ${pattern}`);
  return match;
}

function extractQuotedStringsBetween(text, startMarker, endMarker, label) {
  const start = text.indexOf(startMarker);
  assert.notEqual(start, -1, `${label} missing start marker ${startMarker}`);
  const end = text.indexOf(endMarker, start + startMarker.length);
  assert.notEqual(end, -1, `${label} missing end marker ${endMarker}`);
  const block = text.slice(start, end + endMarker.length);
  return [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

function assertBridgeMissingReasonParity(pythonCatalog) {
  const expected = canonicalBridgeMissingReasons();
  const catalogMissingReasons = new Set(
    pythonCatalog.descriptors.flatMap((descriptor) => descriptor.production_gate.missing),
  );

  for (const reason of expected) {
    assert.ok(catalogMissingReasons.has(reason), `catalog missing fail-closed reason ${reason}`);
  }
  assert.equal(
    new Set(expected).size,
    expected.length,
    "canonical bridge missing reasons must not contain duplicates",
  );

  for (const source of BRIDGE_MISSING_REASON_SOURCES) {
    const reasons = extractQuotedStringsBetween(
      fileText(source.path),
      source.start,
      source.end,
      source.label,
    );
    assert.deepEqual(
      reasons,
      expected,
      `${source.label} privacy bridge missing production-gate reasons drifted`,
    );
  }
}

function extractRustPrivacyAlgorithmEntries(text, label) {
  const start = text.indexOf("const PRIVACY_ALGORITHM_ENTRIES");
  assert.notEqual(start, -1, `${label} missing PRIVACY_ALGORITHM_ENTRIES`);
  const end = text.indexOf("struct PrivacyProductionGateStatusV1", start);
  assert.notEqual(end, -1, `${label} missing PrivacyProductionGateStatusV1 marker`);
  const block = text.slice(start, end);
  const entries = [...block.matchAll(/PrivacyAlgorithmEntry\s*\{([\s\S]*?)\n\s*\},/g)].map(
    (match, index) => {
      const entry = match[1];
      const stringField = (field) =>
        requireMatch(
          entry,
          new RegExp(`${field}:\\s*"([^"]+)"`),
          `${label} privacy algorithm entry ${index} ${field}`,
        )[1];
      const listField = (field) => {
        const list = requireMatch(
          entry,
          new RegExp(`${field}:\\s*&\\[([\\s\\S]*?)\\]`),
          `${label} privacy algorithm entry ${index} ${field}`,
        )[1];
        return [...list.matchAll(/"([^"]+)"/g)].map((item) => item[1]);
      };
      return {
        id: stringField("id"),
        proof_family: stringField("proof_family"),
        backend_family: stringField("backend_family"),
        sdk_entrypoints: listField("sdk_entrypoints"),
        planned_sdk_entrypoints: listField("planned_entrypoints"),
      };
    },
  );

  assert.ok(entries.length > 0, `${label} native privacy capability catalog is empty`);
  assert.equal(
    new Set(entries.map(({ id }) => id)).size,
    entries.length,
    `${label} native privacy capability catalog has duplicate algorithm ids`,
  );
  return entries;
}

function extractRustPrivacyProductionGateContract(text, label) {
  const version = requireMatch(
    text,
    /const PRIVACY_PRODUCTION_GATE_VERSION:\s*&str\s*=\s*"([^"]+)"/,
    `${label} native privacy production gate version`,
  )[1];
  const requirementsBlock = requireMatch(
    text,
    /const PRIVACY_PRODUCTION_GATE_REQUIREMENTS:\s*&\[\(&str,\s*&str\)\]\s*=\s*&\[([\s\S]*?)\];/,
    `${label} native privacy production gate requirements`,
  )[1];
  const requirements = [
    ...requirementsBlock.matchAll(/\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,?\s*\)/g),
  ].map(
    (match) => [match[1], match[2]],
  );

  const gateStart = text.indexOf("fn privacy_production_gate()");
  assert.notEqual(gateStart, -1, `${label} missing privacy_production_gate`);
  const gateEnd = text.indexOf("fn privacy_capabilities()", gateStart);
  assert.notEqual(gateEnd, -1, `${label} missing privacy_capabilities marker`);
  const gateBlock = text.slice(gateStart, gateEnd);
  const supplementalBlock = requireMatch(
    gateBlock,
    /\.chain\(\s*\[\s*([\s\S]*?)\]\s*\.into_iter\(\)/,
    `${label} native privacy supplemental missing reasons`,
  )[1];
  const supplementalMissingReasons = [...supplementalBlock.matchAll(/"([^"]+)"/g)].map(
    (match) => match[1],
  );

  const capabilitiesStart = text.indexOf("fn privacy_capabilities()");
  assert.notEqual(capabilitiesStart, -1, `${label} missing privacy_capabilities`);
  const capabilitiesEnd = text.indexOf("fn privacy_algorithm_entry", capabilitiesStart);
  assert.notEqual(capabilitiesEnd, -1, `${label} missing privacy_algorithm_entry marker`);
  const capabilitiesBlock = text.slice(capabilitiesStart, capabilitiesEnd);

  return {
    version,
    requirements,
    supplementalMissingReasons,
    gateBlock,
    capabilitiesBlock,
  };
}

function assertRustNativeProductionGateParity(pythonCatalog) {
  const expectedGates = Object.fromEntries(
    PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]),
  );
  const expectedGateEntries = PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]);
  for (const descriptor of pythonCatalog.descriptors) {
    const expectedMissingReasons = [
      ...PRODUCTION_GATE_REQUIRED_REASONS,
      ...SUPPLEMENTAL_FAIL_CLOSED_REASONS.filter((reason) =>
        descriptor.production_gate.missing.includes(reason),
      ),
    ];

    assert.equal(
      descriptor.production_gate.version,
      PRODUCTION_GATE_VERSION,
      `Python catalog ${descriptor.id} production gate version drifted`,
    );
    assert.deepEqual(
      descriptor.production_gate.gates,
      expectedGates,
      `Python catalog ${descriptor.id} production gate keys drifted`,
    );
    assert.deepEqual(
      Object.entries(descriptor.production_gate.gates),
      expectedGateEntries,
      `Python catalog ${descriptor.id} production gate key order drifted`,
    );
    assert.equal(
      new Set(descriptor.production_gate.missing).size,
      descriptor.production_gate.missing.length,
      `Python catalog ${descriptor.id} production gate missing reasons contain duplicates`,
    );
    assert.deepEqual(
      descriptor.production_gate.missing,
      expectedMissingReasons,
      `Python catalog ${descriptor.id} production gate missing reasons drifted`,
    );
  }

  for (const source of RUST_PRIVACY_ALGORITHM_SOURCES) {
    const contract = extractRustPrivacyProductionGateContract(fileText(source.path), source.label);
    assert.equal(
      contract.version,
      PRODUCTION_GATE_VERSION,
      `${source.label} native privacy production gate version drifted`,
    );
    assert.deepEqual(
      contract.requirements,
      PRODUCTION_GATE_REQUIREMENTS,
      `${source.label} native privacy production gate requirements drifted`,
    );
    assert.deepEqual(
      contract.supplementalMissingReasons,
      RUST_NATIVE_SUPPLEMENTAL_FAIL_CLOSED_REASONS,
      `${source.label} native privacy supplemental missing reasons drifted`,
    );
    for (const requiredSnippet of [
      "ready: false",
      "passed: false",
      "audit_references: Vec::new()",
    ]) {
      assert.ok(
        contract.gateBlock.includes(requiredSnippet),
        `${source.label} native privacy gate must contain ${requiredSnippet}`,
      );
    }
    for (const requiredSnippet of [
      "version: PRIVACY_FFI_VERSION_V1",
      "gate_version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned()",
      "production_ready: false",
      "production_gate: privacy_production_gate()",
    ]) {
      assert.ok(
        contract.capabilitiesBlock.includes(requiredSnippet),
        `${source.label} native privacy capabilities must contain ${requiredSnippet}`,
      );
    }
  }
}

function assertRustNativeCatalogParity(pythonCatalog) {
  const expected = pythonCatalog.descriptors.map((descriptor) => ({
    id: descriptor.id,
    proof_family: descriptor.proof_family,
    backend_family: descriptor.backend_family,
    sdk_entrypoints: descriptor.sdk_entrypoints,
    planned_sdk_entrypoints: descriptor.planned_sdk_entrypoints,
  }));
  for (const descriptor of expected) {
    assert.equal(
      typeof descriptor.backend_family,
      "string",
      `native privacy backend family missing for ${descriptor.id}`,
    );
  }
  for (const source of RUST_PRIVACY_ALGORITHM_SOURCES) {
    const actual = extractRustPrivacyAlgorithmEntries(fileText(source.path), source.label);
    assert.deepEqual(
      actual,
      expected,
      `${source.label} native privacy capability catalog drifted from SDK catalog`,
    );
  }
}

function assertNoDuplicateEntrypoints(label, descriptor) {
  for (const field of ["sdk_entrypoints", "planned_sdk_entrypoints"]) {
    const values = descriptor[field];
    assert.equal(
      new Set(values).size,
      values.length,
      `${label} ${descriptor.id} field ${field} must not contain duplicate entrypoints`,
    );
  }
  for (const entrypoint of descriptor.planned_sdk_entrypoints) {
    assert.equal(
      descriptor.sdk_entrypoints.includes(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} is already executable`,
    );
    assert.equal(
      entrypointIsDevFixture(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} must not be a fixture/mock entrypoint`,
    );
    assert.equal(
      entrypointIsLocalVerifier(entrypoint),
      false,
      `${label} ${descriptor.id} planned entrypoint ${entrypoint} must not be a local-only verifier entrypoint`,
    );
  }
  if (descriptor.sdk_entrypoints.some(entrypointIsLocalVerifier)) {
    assert.ok(
      descriptor.sdk_entrypoints.some(entrypointIsExplicitDevFixture),
      `${label} ${descriptor.id} local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint`,
    );
  }
  if (descriptor.sdk_entrypoints.some(entrypointIsExplicitDevFixture)) {
    assert.ok(
      descriptor.planned_sdk_entrypoints.some(entrypointIsProductionProofBuilder),
      `${label} ${descriptor.id} DevFixture SDK entrypoints must retain a planned production proof builder`,
    );
  }
  if (descriptor.implementation_stage === "component") {
    for (const entrypoint of [
      ...descriptor.sdk_entrypoints,
      ...descriptor.planned_sdk_entrypoints,
    ]) {
      assert.equal(
        entrypointIsInstructionBuilder(entrypoint),
        false,
        `${label} ${descriptor.id} component entrypoint ${entrypoint} must not be an instruction builder`,
      );
    }
  }
  const plannedLedgerMutations = descriptor.planned_sdk_entrypoints.filter(
    entrypointIsPlannedLedgerMutation,
  );
  if (plannedLedgerMutations.length > 0) {
    const protectionValues = [
      ...descriptor.required_state,
      ...descriptor.failure_modes,
      ...descriptor.chain_requirements,
    ].map((value) => value.toLowerCase());
    assert.ok(
      LEDGER_MUTATION_PROTECTION_METADATA_TOKENS.some((token) =>
        catalogTextValuesContainAffirmedMetadataToken(protectionValues, token),
      ),
      `${label} ${descriptor.id} planned ledger-mutating entrypoints missing protection metadata`,
    );
    const typedAdmissionText = TYPED_CHAIN_ADMISSION_METADATA_FIELDS.flatMap(
      (field) => descriptor[field],
    ).join(" ").toLowerCase();
    assert.ok(
      TYPED_CHAIN_ADMISSION_TYPE_TOKENS.some((token) =>
        catalogTextContainsTypedAdmissionToken(typedAdmissionText, token),
      ) &&
        TYPED_CHAIN_ADMISSION_MUTATION_TOKENS.some((token) =>
          catalogTextContainsTypedAdmissionToken(typedAdmissionText, token),
      ),
      `${label} ${descriptor.id} planned ledger-mutating entrypoints missing typed chain admission metadata`,
    );
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    if (
      STATEFUL_LEDGER_STATE_TOKENS.some((token) =>
        catalogTextContainsAffirmedMetadataToken(requiredStateText, token),
      )
    ) {
      const persistenceText = STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS.flatMap(
        (field) => descriptor[field],
      ).join(" ").toLowerCase();
      for (const tokens of STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS) {
        assert.ok(
          tokens.some((token) =>
            catalogTextContainsAffirmedMetadataToken(persistenceText, token),
          ),
          `${label} ${descriptor.id} planned ledger-mutating entrypoints missing restart/persistence metadata for ${tokens.join("/")}`,
        );
      }
      const failureModesText = descriptor.failure_modes.join(" ").toLowerCase();
      for (const tokens of STATEFUL_LEDGER_FAILURE_MODE_TOKEN_GROUPS) {
        assert.ok(
          tokens.some((token) =>
            catalogTextContainsAffirmedMetadataToken(failureModesText, token),
          ),
          `${label} ${descriptor.id} planned ledger-mutating entrypoints missing stale-state or duplicate/replay failure-mode metadata for ${tokens.join("/")}`,
        );
      }
    }
  }
  if (
    WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    !WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    assert.ok(
      WALLET_STATE_METADATA_TOKENS.some((token) =>
        catalogTextContainsAffirmedMetadataToken(requiredStateText, token),
      ),
      `${label} ${descriptor.id} missing wallet or witness required-state metadata`,
    );
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    for (const tokens of WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) =>
          catalogTextContainsWalletWitnessPrivacyToken(securityNotesText, token),
        ),
        `${label} ${descriptor.id} missing wallet/witness privacy note for ${tokens.join("/")}`,
      );
    }
  }
  if (
    descriptor.implementation_stage !== null &&
    CREDENTIAL_STATE_REQUIRED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = descriptor.required_state.join(" ").toLowerCase();
    assert.ok(
      CREDENTIAL_STATE_METADATA_TOKENS.some((token) =>
        catalogTextContainsAffirmedMetadataToken(requiredStateText, token),
      ),
      `${label} ${descriptor.id} missing credential commitment/accumulator required-state metadata`,
    );
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    descriptor.verifier_key_id !== null
  ) {
    for (const token of (descriptor.public_inputs_schema ?? "").split(",").filter(Boolean)) {
      const forbiddenSegment = token
        .split("_")
        .find((segment) => PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS.includes(segment));
      assert.equal(
        forbiddenSegment,
        undefined,
        `${label} ${descriptor.id} public input schema must not include proof/witness payload token ${token}`,
      );
    }
    const failureModesText = descriptor.failure_modes.join(" ").toLowerCase();
    for (const tokens of VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) => catalogTextContainsAffirmedMetadataToken(failureModesText, token)),
        `${label} ${descriptor.id} missing source-referenced verifier negative failure mode for ${tokens.join("/")}`,
      );
    }
    const verifierKeyRecordText = VERIFIER_KEY_RECORD_METADATA_FIELDS.flatMap(
      (field) => descriptor[field],
    ).join(" ").toLowerCase();
    assert.ok(
      VERIFIER_KEY_RECORD_METADATA_TOKENS.some((token) =>
        catalogTextContainsAffirmedMetadataToken(verifierKeyRecordText, token),
      ),
      `${label} ${descriptor.id} missing verifier-key record metadata`,
    );
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage) &&
    descriptor.verifier_key_id !== null
  ) {
    const chainDomainBindingText = CHAIN_DOMAIN_BINDING_METADATA_FIELDS.flatMap(
      (field) => {
        const value = descriptor[field];
        return Array.isArray(value) ? value : [value];
      },
    ).join(" ").toLowerCase();
    assert.ok(
      CHAIN_DOMAIN_BINDING_METADATA_TOKENS.some((token) =>
        catalogTextContainsChainDomainBindingToken(chainDomainBindingText, token),
      ),
      `${label} ${descriptor.id} missing chain/domain binding metadata`,
    );
    assert.ok(
      publicInputsSchemaHasChainDomainBinding(descriptor.public_inputs_schema),
      `${label} ${descriptor.id} missing chain/domain binding public input`,
    );
  }
  if (SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementation_stage)) {
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    for (const tokens of SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS) {
      assert.ok(
        tokens.some((token) => catalogTextContainsSourceHardeningToken(securityNotesText, token)),
        `${label} ${descriptor.id} missing source-referenced hardening gate note for ${tokens.join("/")}`,
      );
    }
  }
  if (descriptor.implementation_stage === "research-target-as-of-2026-05") {
    assert.equal(
      descriptor.sdk_entrypoints.some(entrypointIsDevFixture),
      false,
      `${label} ${descriptor.id} research targets must not expose fixture/mock SDK entrypoints`,
    );
    assert.equal(
      descriptor.sdk_entrypoints.some(entrypointIsLocalVerifier),
      false,
      `${label} ${descriptor.id} research targets must not expose local-only verifier SDK entrypoints`,
    );
    assert.equal(
      descriptor.sdk_entrypoints.length,
      0,
      `${label} ${descriptor.id} research targets must keep executable SDK entrypoints planned-only`,
    );
    const requiredResearchSourceUrls = RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID[descriptor.id];
    assert.ok(
      requiredResearchSourceUrls,
      `${label} ${descriptor.id} research target missing exact source URL contract`,
    );
    const sourceUrls = new Set(
      descriptor.source_references.map((reference) => reference.url),
    );
    for (const requiredUrl of requiredResearchSourceUrls) {
      assert.ok(
        sourceUrls.has(requiredUrl),
        `${label} ${descriptor.id} research target missing exact source URL ${requiredUrl}`,
      );
    }
    const securityNotesText = descriptor.security_notes.join(" ").toLowerCase();
    assert.ok(
      RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS.every((token) =>
        catalogTextContainsAffirmedMetadataToken(securityNotesText, token),
      ),
      `${label} ${descriptor.id} research target missing production readiness note`,
    );
    assert.ok(
      RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS.some((token) =>
        catalogTextContainsAffirmedMetadataToken(securityNotesText, token),
      ),
      `${label} ${descriptor.id} research target missing audit/review readiness note`,
    );
  }
  if (descriptor.covered_criteria.includes("post_quantum")) {
    const sourceUrls = new Set(
      descriptor.source_references.map((reference) => reference.url),
    );
    for (const requiredUrl of POST_QUANTUM_REQUIRED_SOURCE_URLS) {
      assert.ok(
        sourceUrls.has(requiredUrl),
        `${label} ${descriptor.id} post_quantum row missing source ${requiredUrl}`,
      );
    }
    const plannedEntrypointNames = descriptor.planned_sdk_entrypoints.map((entrypoint) => {
      const segments = entrypoint.split(".");
      return segments[segments.length - 1];
    });
    for (const requiredFragment of POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS) {
      assert.ok(
        plannedEntrypointNames.some((name) =>
          plannedEntrypointNameHasPrimitiveFragment(name, requiredFragment),
        ),
        `${label} ${descriptor.id} post_quantum row missing planned SDK entrypoint fragment ${requiredFragment}`,
      );
    }
    for (const [fieldName, values, requiredTokens] of [
      ["security_notes", descriptor.security_notes, POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS],
      ["failure_modes", descriptor.failure_modes, POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS],
      ["required_state", descriptor.required_state, POST_QUANTUM_REQUIRED_STATE_TOKENS],
    ]) {
      for (const requiredToken of requiredTokens) {
        assert.ok(
          values.some((value) =>
            catalogTextContainsAffirmedMetadataToken(value, requiredToken),
          ),
          `${label} ${descriptor.id} post_quantum row missing ${fieldName} token ${requiredToken}`,
        );
      }
    }
  }
}

function assertRequiredPrivacyPlanRows(label, descriptors) {
  const descriptorById = new Map(
    descriptors.map((descriptor) => [descriptor.id, descriptor]),
  );
  for (const [algorithmId, implementationStage, backendFamily] of REQUIRED_PRIVACY_PLAN_ROWS) {
    const descriptor = descriptorById.get(algorithmId);
    assert.notEqual(
      descriptor,
      undefined,
      `${label} missing required production privacy plan row ${algorithmId}`,
    );
    assert.deepEqual(
      [descriptor.name, descriptor.short_name, descriptor.summary],
      REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan display text drifted`,
    );
    assert.equal(
      descriptor.implementation_stage,
      implementationStage,
      `${label} ${algorithmId} required production privacy plan stage drifted`,
    );
    assert.equal(
      descriptor.backend_family,
      backendFamily,
      `${label} ${algorithmId} required production privacy plan backend drifted`,
    );
    assert.equal(
      descriptor.category,
      REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan category drifted`,
    );
    assert.equal(
      descriptor.maturity,
      REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan maturity drifted`,
    );
    assert.deepEqual(
      descriptor.recommended_for,
      REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan recommendedFor drifted`,
    );
    assert.deepEqual(
      descriptor.covered_criteria,
      REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan covered criteria drifted`,
    );
    assert.equal(
      descriptor.proof_family,
      REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan proof family drifted`,
    );
    assert.equal(
      descriptor.public_inputs_schema,
      REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan public input schema drifted`,
    );
    assert.equal(
      descriptor.verifier_key_id,
      REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan verifier-key id drifted`,
    );
    assert.deepEqual(
      descriptor.pq_layers,
      REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan PQ layer drifted`,
    );
    assert.deepEqual(
      descriptor.chain_requirements,
      REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan chain requirements drifted`,
    );
    assert.deepEqual(
      descriptor.required_state,
      REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan required state drifted`,
    );
    assert.deepEqual(
      descriptor.setup_steps,
      REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan setup steps drifted`,
    );
    assert.deepEqual(
      descriptor.execution_steps,
      REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan execution steps drifted`,
    );
    assert.deepEqual(
      descriptor.failure_modes,
      REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan failure modes drifted`,
    );
    assert.deepEqual(
      descriptor.security_notes,
      REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan security notes drifted`,
    );
    const stateText = [
      ...(descriptor.required_state ?? []),
      ...(descriptor.chain_requirements ?? []),
    ]
      .map((value) => String(value).toLowerCase())
      .join("\n");
    for (const stateToken of REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID[
      algorithmId
    ]) {
      assert.ok(
        catalogTextContainsAffirmedMetadataToken(stateText, stateToken),
        `${label} ${algorithmId} required production privacy plan state token drifted: ${stateToken}`,
      );
    }
    const failureModeText = (descriptor.failure_modes ?? [])
      .map((value) => String(value).toLowerCase())
      .join("\n");
    for (const failureToken of [
      ...REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS,
      ...REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID[algorithmId],
    ]) {
      assert.ok(
        catalogTextContainsAffirmedMetadataToken(failureModeText, failureToken),
        `${label} ${algorithmId} required production privacy plan failure-mode token drifted: ${failureToken}`,
      );
    }
    for (const sourceReference of REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[
      algorithmId
    ]) {
      assert.ok(
        (descriptor.source_references ?? []).some(
          (reference) =>
            reference.label === sourceReference.label &&
            reference.url === sourceReference.url,
        ),
        `${label} ${algorithmId} required production privacy plan source reference drifted: ${sourceReference.label} <${sourceReference.url}>`,
      );
    }
    assert.deepEqual(
      descriptor.source_references,
      REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan source references drifted`,
    );
    assert.deepEqual(
      descriptor.sdk_entrypoints,
      REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan SDK entrypoints drifted`,
    );
    assert.deepEqual(
      descriptor.planned_sdk_entrypoints,
      REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[algorithmId],
      `${label} ${algorithmId} required production privacy plan planned SDK entrypoints drifted`,
    );
    assert.ok(
      descriptor.planned_sdk_entrypoints.some(entrypointIsProductionProofBuilder),
      `${label} ${algorithmId} required production privacy plan row must retain a planned production proof builder until production gates pass`,
    );
  }
}

function assertResearchTargetSdkEntrypointsFailClosed(label, descriptors) {
  assert.ok(
    descriptors.some(
      (descriptor) =>
        descriptor.implementation_stage !== "research-target-as-of-2026-05" &&
        descriptor.sdk_entrypoints.length > 0,
    ),
    `${label} non-research SDK entrypoints missing`,
  );
  for (const descriptor of descriptors) {
    if (descriptor.implementation_stage !== "research-target-as-of-2026-05") {
      continue;
    }
    assert.equal(
      descriptor.sdk_entrypoints.length,
      0,
      `${label} ${descriptor.id} research target executable SDK entrypoints must stay planned-only`,
    );
    assert.ok(
      descriptor.planned_sdk_entrypoints.length > 0,
      `${label} ${descriptor.id} research target planned SDK entrypoints missing`,
    );
  }
}

function assertExecutableEntrypointsExported(label, descriptors, moduleExports) {
  for (const descriptor of descriptors) {
    for (const entrypoint of descriptor.sdkEntrypoints) {
      assert.equal(
        typeof moduleExports[entrypoint],
        "function",
        `${label} ${descriptor.id} executable SDK entrypoint ${entrypoint} must be exported`,
      );
    }
  }
}

function assertExecutableEntrypointsDeclared(label, descriptors, declarationText) {
  for (const descriptor of descriptors) {
    for (const entrypoint of descriptor.sdkEntrypoints) {
      assert.equal(
        new RegExp(`\\bexport\\s+function\\s+${escapeRegExp(entrypoint)}\\s*\\(`).test(
          declarationText,
        ),
        true,
        `${label} ${descriptor.id} executable SDK entrypoint ${entrypoint} must be declared`,
      );
    }
  }
}

function assertCatalogParity(label, criteria, descriptors, pythonCatalog) {
  const normalizedDescriptors = descriptors.map(toPythonDescriptorShape);
  assert.deepEqual(criteria, pythonCatalog.criteria, `${label} privacy criteria drifted`);
  assert.deepEqual(
    normalizedDescriptors.map(({ id }) => id),
    pythonCatalog.descriptors.map(({ id }) => id),
    `${label} privacy algorithm id order drifted`,
  );
  assertRequiredPrivacyPlanRows(label, normalizedDescriptors);
  assertResearchTargetSdkEntrypointsFailClosed(label, normalizedDescriptors);

  const verifierKeyIds = normalizedDescriptors
    .map((descriptor) => descriptor.verifier_key_id)
    .filter((verifierKeyId) => verifierKeyId !== null);
  assert.equal(
    new Set(verifierKeyIds).size,
    verifierKeyIds.length,
    `${label} privacy verifier key ids must be unique`,
  );

  for (const [index, descriptor] of normalizedDescriptors.entries()) {
    assert.deepEqual(
      descriptor,
      Object.fromEntries(
        Object.entries(pythonCatalog.descriptors[index]).filter(([key]) =>
          Object.hasOwn(descriptor, key),
        ),
      ),
      `${label} privacy algorithm descriptor ${descriptor.id} drifted from Python catalog`,
    );
    assertFailClosedDescriptor(label, descriptor);
    assertNoDuplicateEntrypoints(label, descriptor);
  }
}

test("privacy algorithm catalogs stay fail-closed and in parity across JS and Python", () => {
  const pythonCatalog = loadPythonPrivacyCatalog();

  assertCatalogParity(
    "src",
    getSrcPrivacyCriteria(),
    getSrcPrivacyAlgorithmDescriptors(),
    pythonCatalog,
  );
  assertCatalogParity(
    "dist",
    getDistPrivacyCriteria(),
    getDistPrivacyAlgorithmDescriptors(),
    pythonCatalog,
  );
  assertBridgeMissingReasonParity(pythonCatalog);
  assertBackendFamilyRegistrationParity(pythonCatalog);
  assertPythonCatalogDefensiveCopyCoverage();
  assertPythonZkAceProofBuilderCoverage();
  assertZkAceJsBuilderAmountCoverage();
  assertZkAcePythonTransactionAmountCoverage();
  assertRustNativeProductionGateParity(pythonCatalog);
  assertRustNativeCatalogParity(pythonCatalog);
});

test("privacy algorithm catalogs pin executable ZK-ACE proof-builder descriptor shape", () => {
  for (const [label, getDescriptor] of [
    ["src", getSrcPrivacyAlgorithmDescriptor],
    ["dist", getDistPrivacyAlgorithmDescriptor],
  ]) {
    assertZkAceExecutableDescriptorShape(
      label,
      getDescriptor("zk-ace-pq-authorization-v0"),
    );
  }
  const pythonCatalog = loadPythonPrivacyCatalog();
  const pythonZkAce = pythonCatalog.descriptors.find(
    (descriptor) => descriptor.id === "zk-ace-pq-authorization-v0",
  );
  assert.ok(pythonZkAce, "python ZK-ACE descriptor must exist");
  assertZkAceExecutableDescriptorShape(
    "python",
    pythonDescriptorToJsShape(pythonZkAce),
  );
});

test("privacy algorithm catalogs require proof builders on required production plan rows", () => {
  for (const [label, text] of [
    ["JS source", fileText("javascript/iroha_js/src/privacyAlgorithms.js")],
    ["JS dist", fileText("javascript/iroha_js/dist/privacyAlgorithms.js")],
  ]) {
    assert.match(
      text,
      /function\s+entrypointIsProofHelper\([^)]*\)\s*\{[\s\S]*ProofEnvelope[\s\S]*ProofWitness[\s\S]*ProofPublicInputs[\s\S]*ProofRequest[\s\S]*ProofCommitment/,
      `${label} must classify proof helper and wrapper entrypoints`,
    );
    assert.match(
      text,
      /function\s+entrypointIsPlannedLedgerMutation\([^)]*\)\s*\{[\s\S]*\["Instruction",\s*"Transaction"\]\.some\(\(fragment\)\s*=>[\s\S]*entrypointNameHasTerminalEvidenceFragment\(name,\s*fragment\)[\s\S]*entrypointNameHasEvidenceFragment\(name,\s*"Submit"\)/,
      `${label} planned ledger mutation classifier must require non-negated entrypoint evidence`,
    );
    assert.match(
      text,
      /function\s+entrypointIsProductionProofBuilder\([^)]*\)\s*\{[\s\S]*name\.startsWith\("build"\)[\s\S]*entrypointNameHasEvidenceFragment\(name,\s*"Proof"\)[\s\S]*!entrypointIsInstructionBuilder\(entrypoint\)[\s\S]*!entrypointIsPlannedLedgerMutation\(entrypoint\)[\s\S]*!entrypointIsProofHelper\(entrypoint\)[\s\S]*!entrypointIsDevFixture\(entrypoint\)/,
      `${label} production proof-builder classifier must reject ledger mutations and proof helpers`,
    );
    assert.match(
      text,
      /function\s+validateRequiredPrivacyPlanRows\([^)]*\)\s*\{[\s\S]*REQUIRED_PRIVACY_PLAN_ROWS[\s\S]*REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID[\s\S]*must keep category[\s\S]*REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID[\s\S]*must keep maturity[\s\S]*REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID[\s\S]*must keep recommendedFor[\s\S]*REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID[\s\S]*must keep covered criteria[\s\S]*REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID[\s\S]*must keep proof family[\s\S]*REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID[\s\S]*must keep public inputs schema[\s\S]*REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID[\s\S]*must keep verifier key id[\s\S]*REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID[\s\S]*must keep PQ layer[\s\S]*REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID[\s\S]*must keep chain requirements[\s\S]*REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID[\s\S]*must keep required state[\s\S]*REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID[\s\S]*must keep setup steps[\s\S]*REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID[\s\S]*must keep execution steps[\s\S]*REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID[\s\S]*must keep failure modes[\s\S]*REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[\s\S]*must keep security notes[\s\S]*REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID[\s\S]*must retain required state token[\s\S]*REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID[\s\S]*must retain required failure-mode token[\s\S]*REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[\s\S]*must retain source reference[\s\S]*must keep source references[\s\S]*REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[\s\S]*must keep SDK entrypoints[\s\S]*REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[\s\S]*must keep planned SDK entrypoints[\s\S]*entrypointIsProductionProofBuilder[\s\S]*must retain a planned production proof builder/,
      `${label} required production plan rows must require covered-criteria, proof-family, public-input schema, verifier-key, PQ-layer, chain-requirement, exact required-state, setup-step, execution-step, exact failure-mode, exact security-note, state-token, failure-mode token, source-reference, SDK entrypoint, planned SDK entrypoint, and planned proof-builder checks`,
    );
    assert.match(
      text,
      /function\s+validateRequiredPrivacyPlanRows\([^)]*\)\s*\{[\s\S]*REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[\s\S]*must keep display text[\s\S]*REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID/,
      `${label} required production plan rows must keep exact display text`,
    );
  }

  const pythonCatalogSource = fileText(
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_validate_required_privacy_plan_rows[\s\S]*REQUIRED_PRIVACY_PLAN_ROWS[\s\S]*REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID[\s\S]*must keep category[\s\S]*REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID[\s\S]*must keep maturity[\s\S]*REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID[\s\S]*must keep recommendedFor[\s\S]*REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID[\s\S]*must keep covered criteria[\s\S]*REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID[\s\S]*must keep proof family[\s\S]*REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID[\s\S]*must keep public inputs schema[\s\S]*REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID[\s\S]*must keep verifier key id[\s\S]*REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID[\s\S]*must keep PQ layer[\s\S]*REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID[\s\S]*must keep chain requirements[\s\S]*REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID[\s\S]*must keep required state[\s\S]*REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID[\s\S]*must keep setup steps[\s\S]*REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID[\s\S]*must keep execution steps[\s\S]*REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID[\s\S]*must keep failure modes[\s\S]*REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[\s\S]*must keep security notes[\s\S]*REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID[\s\S]*must retain required state[\s\S]*REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID[\s\S]*must retain required[\s\S]*failure-mode token[\s\S]*REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[\s\S]*must retain source reference[\s\S]*must keep source references[\s\S]*REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[\s\S]*must keep SDK entrypoints[\s\S]*REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[\s\S]*_entrypoint_is_production_proof_builder[\s\S]*must retain a planned production proof[\s\S]*must keep planned SDK entrypoints/,
    "Python required production plan rows must require covered-criteria, proof-family, public-input schema, verifier-key, PQ-layer, chain-requirement, exact required-state, setup-step, execution-step, exact failure-mode, exact security-note, state-token, failure-mode token, source-reference, SDK entrypoint, planned SDK entrypoint, and planned production proof checks",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_validate_required_privacy_plan_rows[\s\S]*REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[\s\S]*must keep display text[\s\S]*REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID/,
    "Python required production plan rows must keep exact display text",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_entrypoint_is_planned_ledger_mutation[\s\S]*_entrypoint_name_has_terminal_evidence_fragment\(name,\s*fragment\)[\s\S]*\("Instruction",\s*"Transaction"\)[\s\S]*_entrypoint_name_has_evidence_fragment\(name,\s*"Submit"\)/,
    "Python planned ledger mutation classifier must require non-negated entrypoint evidence",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_entrypoint_is_production_proof_builder[\s\S]*_entrypoint_is_instruction_builder\(entrypoint\)[\s\S]*_entrypoint_is_planned_ledger_mutation\(entrypoint\)[\s\S]*_entrypoint_is_dev_fixture\(entrypoint\)/,
    "Python production proof-builder classifier must reject ledger mutations",
  );
  assert.match(
    pythonCatalogSource,
    /def\s+_entrypoint_is_proof_helper[\s\S]*ProofEnvelope[\s\S]*ProofWitness[\s\S]*ProofPublicInputs[\s\S]*ProofRequest[\s\S]*ProofCommitment[\s\S]*def\s+_entrypoint_is_production_proof_builder[\s\S]*_entrypoint_is_proof_helper\(entrypoint\)/,
    "Python production proof-builder classifier must reject proof helpers",
  );

  const pythonCatalogTests = fileText("python/iroha_python/tests/privacy_catalog_test.py");
  assert.match(
    pythonCatalogTests,
    /(?=[\s\S]*test_privacy_catalog_rejects_required_production_privacy_plan_without_proof_builder)(?=[\s\S]*deriveOrchardWitness)(?=[\s\S]*buildAnonymousPgcProductionInstruction)(?=[\s\S]*buildAnonymousPgcProofTransaction)(?=[\s\S]*buildSubmitAnonymousPgcProof)(?=[\s\S]*buildAnonymousPgcProofEnvelope)(?=[\s\S]*buildAnonymousPgcProofWitness)(?=[\s\S]*buildAnonymousPgcProofPublicInputs)(?=[\s\S]*buildAnonymousPgcProofRequest)(?=[\s\S]*buildAnonymousPgcProofCommitment)(?=[\s\S]*buildAnonymousPgcDevProofFixture)/,
    "Python tests must cover helper-only, instruction-only, transaction-only, submit-only, proof-helper-only, and fixture-only required rows",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_proof_family_drift[\s\S]*forged-proof-family[\s\S]*must keep proof family/,
    "Python tests must cover required production plan proof-family drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_covered_criteria_drift[\s\S]*hide_asset_type[\s\S]*must keep covered criteria/,
    "Python tests must cover required production plan covered-criteria drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_public_input_schema_drift[\s\S]*forged_public_input[\s\S]*must keep public inputs schema/,
    "Python tests must cover required production plan public-input schema drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_category_drift[\s\S]*authorization[\s\S]*must keep category/,
    "Python tests must cover required production plan category drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_maturity_drift[\s\S]*specification[\s\S]*must keep maturity/,
    "Python tests must cover required production plan maturity drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_recommended_for_drift[\s\S]*claimed production rollout[\s\S]*must keep recommendedFor/,
    "Python tests must cover required production plan recommendedFor drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_verifier_key_drift[\s\S]*forged_verifier_key[\s\S]*must keep verifier key id/,
    "Python tests must cover required production plan verifier-key drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_pq_layer_drift[\s\S]*descriptor\["pq_layers"\]\["proof"\] = True[\s\S]*must keep PQ layer/,
    "Python tests must cover required production plan PQ-layer drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_chain_requirement_drift[\s\S]*typed zk::SubmitAnonymousPgcProofOnly instruction[\s\S]*must keep chain requirements/,
    "Python tests must cover required production plan chain-requirement drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_required_state_drift[\s\S]*forged wallet recovery placeholder[\s\S]*must keep required state/,
    "Python tests must cover required production plan required-state drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_setup_step_drift[\s\S]*Register forged Anonymous PGC verifier setup\.[\s\S]*must keep setup steps/,
    "Python tests must cover required production plan setup-step drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_execution_step_drift[\s\S]*Submit forged Anonymous PGC proof-only envelope\.[\s\S]*must keep execution steps/,
    "Python tests must cover required production plan execution-step drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_failure_modes_drift[\s\S]*accept forged replay tag[\s\S]*must keep failure modes/,
    "Python tests must cover required production plan exact failure-mode drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_security_note_drift[\s\S]*latency gates[\s\S]*must keep security notes/,
    "Python tests must cover required production plan exact security-note drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_state_token_drift[\s\S]*forged state placeholder[\s\S]*must retain required state token/,
    "Python tests must cover required production plan state-token drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_state_token_concatenated_false_positive[\s\S]*notwallet account blinding[\s\S]*must retain required state token/,
    "Python tests must cover required production plan state-token concatenated false positives",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_state_token_negated_bounded_false_positive[\s\S]*not wallet account blinding[\s\S]*must retain required state token/,
    "Python tests must cover required production plan state-token bounded negation false positives",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_drift[\s\S]*forged failure placeholder[\s\S]*must retain required[\s\S]*failure-mode token/,
    "Python tests must cover required production plan failure-mode drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_concatenated_false_positive[\s\S]*notreceiver-set substitution[\s\S]*must retain required[\s\S]*failure-mode token/,
    "Python tests must cover required production plan failure-mode concatenated false positives",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_negated_bounded_false_positive[\s\S]*not receiver-set substitution[\s\S]*must retain required[\s\S]*failure-mode token/,
    "Python tests must cover required production plan failure-mode bounded negation false positives",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_drift[\s\S]*https:\/\/example\.com\/forged-source[\s\S]*must retain source reference/,
    "Python tests must cover required production plan source-reference drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_extra[\s\S]*https:\/\/example\.com\/forged-extra-source[\s\S]*must keep source references/,
    "Python tests must cover required production plan exact source-reference drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_sdk_entrypoint_drift[\s\S]*buildForgedAnonymousPgcProductionProof[\s\S]*must keep SDK entrypoints/,
    "Python tests must cover required production plan SDK entrypoint drift",
  );
  assert.match(
    pythonCatalogTests,
    /test_privacy_catalog_rejects_required_production_privacy_plan_planned_sdk_entrypoint_drift[\s\S]*buildForgedAnonymousPgcProofV1[\s\S]*must keep planned SDK entrypoints/,
    "Python tests must cover required production plan planned SDK entrypoint drift",
  );
});

test("privacy algorithm JS getters return immutable fail-closed production metadata", () => {
  for (const [label, getCapabilities, getDescriptors, getDescriptor] of [
    [
      "src",
      getSrcPrivacyCapabilities,
      getSrcPrivacyAlgorithmDescriptors,
      getSrcPrivacyAlgorithmDescriptor,
    ],
    [
      "dist",
      getDistPrivacyCapabilities,
      getDistPrivacyAlgorithmDescriptors,
      getDistPrivacyAlgorithmDescriptor,
    ],
  ]) {
    const capabilities = getCapabilities();
    assertZkAceCapabilitySurfaceFailClosed(label, capabilities);
    const descriptors = getDescriptors();
    const descriptor = descriptors.find((entry) => entry.plannedSdkEntrypoints.length > 0);
    assert.ok(descriptor, `${label} must expose a planned fail-closed privacy row`);
    const lookup = getDescriptor(descriptor.id);
    assert.ok(lookup, `${label} single descriptor lookup must find ${descriptor.id}`);

    assert.notEqual(descriptor, lookup, `${label} descriptor getters must return fresh objects`);
    assert.equal(Object.isFrozen(capabilities), true, `${label} capabilities must be frozen`);
    assert.equal(
      Object.isFrozen(capabilities.privacyAlgorithms),
      true,
      `${label} privacy algorithm array must be frozen`,
    );
    assert.equal(
      Object.isFrozen(descriptors),
      true,
      `${label} descriptor array must be frozen`,
    );

    for (const frozenDescriptor of [descriptor, lookup]) {
      assert.equal(
        Object.isFrozen(frozenDescriptor),
        true,
        `${label} descriptor ${frozenDescriptor.id} must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.pqLayers),
        true,
        `${label} descriptor ${frozenDescriptor.id} pqLayers must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.sourceReferences),
        true,
        `${label} descriptor ${frozenDescriptor.id} sourceReferences must be frozen`,
      );
      if (frozenDescriptor.sourceReferences.length > 0) {
        assert.equal(
          Object.isFrozen(frozenDescriptor.sourceReferences[0]),
          true,
          `${label} descriptor ${frozenDescriptor.id} sourceReference rows must be frozen`,
        );
      }
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.gates),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.gates must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.missing),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.missing must be frozen`,
      );
      assert.equal(
        Object.isFrozen(frozenDescriptor.productionGate.auditReferences),
        true,
        `${label} descriptor ${frozenDescriptor.id} productionGate.auditReferences must be frozen`,
      );
      assert.equal(frozenDescriptor.productionReady, false);
      assert.equal(frozenDescriptor.productionGate.ready, false);
      assert.equal(frozenDescriptor.productionGate.gates.external_audit, false);
      assert.ok(
        frozenDescriptor.productionGate.missing.includes("planned SDK entrypoints remain"),
        `${label} descriptor ${frozenDescriptor.id} must expose planned-entrypoint production blocker`,
      );

      assert.throws(() => {
        frozenDescriptor.productionReady = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.ready = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.gates.external_audit = true;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.missing.length = 0;
      });
      assert.throws(() => {
        frozenDescriptor.productionGate.auditReferences.push({
          label: "forged audit",
          url: "https://audit.example/forged",
        });
      });
      assert.throws(() => {
        frozenDescriptor.pqLayers.proof = true;
      });
      assert.throws(() => {
        frozenDescriptor.plannedSdkEntrypoints.length = 0;
      });
      assert.throws(() => {
        frozenDescriptor.sourceReferences.push({
          label: "forged source",
          url: "https://audit.example/forged",
        });
      });
    }

    assert.throws(() => {
      capabilities.privacyAlgorithms.length = 0;
    });
    assert.throws(() => {
      capabilities.privacyCriteria.push("tampered");
    });

    const fresh = getDescriptor(descriptor.id);
    assert.ok(fresh, `${label} fresh descriptor lookup must find ${descriptor.id}`);
    assert.equal(fresh.productionReady, false);
    assert.equal(fresh.productionGate.ready, false);
    assert.equal(fresh.productionGate.gates.external_audit, false);
    assert.ok(
      fresh.productionGate.missing.includes("planned SDK entrypoints remain"),
      `${label} fresh descriptor ${descriptor.id} must remain fail-closed`,
    );
    assert.deepEqual(
      fresh.plannedSdkEntrypoints,
      descriptor.plannedSdkEntrypoints,
      `${label} planned SDK entrypoints must survive attempted mutation`,
    );
  }
});

test("privacy algorithm JS validators reject supplied derived production fields", () => {
  for (const field of DERIVED_JS_COMPATIBILITY_FIELDS) {
    assertJsValidatorsReject(
      { [field]: field.endsWith("Gate") || field.endsWith("_gate") ? { ready: true } : "forged" },
      new RegExp(`field ${field} is derived and must not be supplied`),
    );
  }
});

test("privacy algorithm JS validators reject hostile catalog descriptor shapes", () => {
  for (const [patch, pattern] of [
    [
      { id: "unmapped-backend-family" },
      /registered non-none backend family/,
    ],
    [
      { auditReferences: [{ label: "forged", url: "https://audit.example/forged" }] },
      /field auditReferences is not a supported privacy catalog field/,
    ],
    [{ shortName: "" }, /shortName must be a non-empty string/],
    [{ shortName: " Shape" }, /shortName must be clean and already trimmed/],
    [{ summary: "   " }, /summary must be a non-empty string/],
    [{ summary: "Descriptor\u007fsummary" }, /summary must be clean and already trimmed/],
    [{ summary: "Descriptor\u200bsummary" }, /summary must be clean and already trimmed/],
    [
      { summary: "Mainnet-ready audited production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { summary: "M\u0430innet-re\u0430dy proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { summary: "Claimed production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { name: "Claimed mainnet transfer" },
      /name must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { shortName: "Audit claim" },
      /shortName must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { id: "mainnet-ready-shield" },
      /id must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { id: "claimed-mainnet-shield" },
      /id must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [{ id: "Shield" }, /id must be lowercase and URL-safe/],
    [{ id: "shield.v1" }, /id must be lowercase and URL-safe/],
    [{ id: "shield/../../admin" }, /id must be lowercase and URL-safe/],
    [{ id: "_shield" }, /id must be lowercase and URL-safe/],
    [{ id: "-shield" }, /id must be lowercase and URL-safe/],
    [{ id: "shield_" }, /id must be lowercase and URL-safe/],
    [{ id: "shield-" }, /id must be lowercase and URL-safe/],
    [{ proofFamily: "" }, /proofFamily must be a non-empty string/],
    [{ proofFamily: " halo2-ipa" }, /proofFamily must be clean and already trimmed/],
    [{ proofFamily: "Halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2..ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2/../ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2--ipa" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "/halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "-halo2" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2/" }, /proofFamily must be a proof family name/],
    [{ proofFamily: "halo2-" }, /proofFamily must be a proof family name/],
    [
      { proofFamily: "halo2/mainnet-ready" },
      /proofFamily must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { proofFamily: "halo2/production-claim" },
      /proofFamily must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "" },
      /publicInputsSchema must be a non-empty string or null/,
    ],
    [
      { publicInputsSchema: "root,\nproof" },
      /publicInputsSchema must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: "root," },
      /publicInputsSchema token 1 must be a non-empty public input name/,
    ],
    [
      { publicInputsSchema: "root, proof" },
      /publicInputsSchema token 1 must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: "root,Proof" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,1proof" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,field_" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,field__digest" },
      /publicInputsSchema token 1 must be a lowercase public input name/,
    ],
    [
      { publicInputsSchema: "root,proof" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,recursive_proof_digest" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,wallet_witness_digest" },
      /publicInputsSchema token 1 must not include proof or witness payload metadata/,
    ],
    [
      { publicInputsSchema: "root,production_gate_passed" },
      /publicInputsSchema token 1 must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "root,audit_claim" },
      /publicInputsSchema token 1 must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { publicInputsSchema: "root,root" },
      /publicInputsSchema token 1 duplicates root/,
    ],
    [
      { verifierKeyId: "   " },
      /verifierKeyId must be a non-empty string or null/,
    ],
    [
      { verifierKeyId: 7 },
      /verifierKeyId must be a non-empty string or null/,
    ],
    [
      { verifierKeyId: "zk::Shield\t" },
      /verifierKeyId must be clean and already trimmed/,
    ],
    [
      { publicInputsSchema: null, verifierKeyId: "orphan_verifier_key" },
      /publicInputsSchema and verifierKeyId must be supplied together/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: null },
      /publicInputsSchema and verifierKeyId must be supplied together/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "VerifierKey" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier_key_" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier__key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "verifier.key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk:Shield" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk_::Shield" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield_" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield__Key" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "zk::Shield/../../admin" },
      /verifierKeyId must be a verifier key id/,
    ],
    [
      { publicInputsSchema: "root", verifierKeyId: "audited_production_vk" },
      /verifierKeyId must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [{ category: "production_claim" }, /category must be a known category/],
    [{ maturity: "audited" }, /maturity must be a known maturity/],
    [
      { implementationStage: "Production Hardened" },
      /implementationStage must be a lowercase hyphenated identifier/,
    ],
    [
      { implementationStage: "audited-production" },
      /implementationStage must be a known implementation stage/,
    ],
    [
      { implementationStage: "production-ready" },
      /implementationStage must be a known implementation stage/,
    ],
    [{ coveredCriteria: ["hide_sender", "hide_sender"] }, /duplicates hide_sender/],
    [{ coveredCriteria: ["hide_operator"] }, /must be a known privacy criterion/],
    [
      { recommendedFor: ["audit evidence", "audit evidence"] },
      /recommendedFor\[1\] duplicates audit evidence/,
    ],
    [
      { recommendedFor: [" audit evidence"] },
      /recommendedFor\[0\] must be clean and already trimmed/,
    ],
    [
      { recommendedFor: ["Production-ready bank deployment"] },
      /recommendedFor\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { recommendedFor: ["claimed audit rollout"] },
      /recommendedFor\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { chainRequirements: ["production-ready verifier"] },
      /chainRequirements\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { requiredState: ["claimed mainnet root"] },
      /requiredState\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { setupSteps: ["Install audit claim verifier"] },
      /setupSteps\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { executionSteps: ["Submit claimed production proof"] },
      /executionSteps\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { sdkEntrypoints: ["buildMainnetReadyProof"] },
      /sdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { plannedSdkEntrypoints: ["buildAuditSignoffProof"] },
      /plannedSdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { plannedSdkEntrypoints: ["buildClaimedAuditProof"] },
      /plannedSdkEntrypoints\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { securityNotes: ["line\nbreak"] },
      /securityNotes\[0\] must be clean and already trimmed/,
    ],
    [
      { securityNotes: ["line\u200bbreak"] },
      /securityNotes\[0\] must be clean and already trimmed/,
    ],
    [
      { securityNotes: ["External audit completed and production sign-off received."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["A.u.d.i.t passed; s.e.c.u.r.i.t.y review approved."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["External \u0430udit p\u0430ssed."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["Claimed audit coverage is present."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { securityNotes: ["Mainnet claim accepted by reviewer."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      { failureModes: ["External audit completed."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      { failureModes: ["Mainnet claim accepted by reviewer."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      { chainRequirements: ["verifier registry", "verifier registry"] },
      /chainRequirements\[1\] duplicates verifier registry/,
    ],
    [
      { chainRequirements: ["registry\u007f"] },
      /chainRequirements\[0\] must be clean and already trimmed/,
    ],
    [
      { chainRequirements: ["registry\u200b"] },
      /chainRequirements\[0\] must be clean and already trimmed/,
    ],
    [
      { sourceReferences: [{ label: " paper", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "paper\nnext", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "paper\u007f", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "paper\u200b", url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "p".repeat(161), url: "https://example.invalid" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "External audit signoff", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "Protocol s.e.c.u.r.i.t.y review", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "Protocol security rev\u0456ew", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "External.review report", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      { sourceReferences: [{ label: "\u0391ssurance.report", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must be clean and bounded/,
    ],
    [
      { sourceReferences: [{ label: "Production-ready protocol source", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "http://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "HTTPS://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: " https://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://example.invalid/path\nnext" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://user:pass@example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.ca\u0455h/zip-0224" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://xn--cah-ghd.org/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/prot\u03bfcol/protocol.pdf" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?claim=m\u0430innet" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://ZIPS.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash:443/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash:8443/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash./zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/protocol/../zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/protocol/%2e%2e/zip-0224" }] },
      /sourceReferences\[0\]\.url must be canonical/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://127%2e0%2e0%2e1/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://localhost%2elocaltest%2eme/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://256.256.256.256/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://example.invalid\\evil" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?section=notes%ZZappendix" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-audit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?production=ready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=audit%3Dcomplete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=mainnet%2520claim" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-%2561udit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%2525253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://audit.example/forged-signoff" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://127.0.0.1.nip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://10.0.0.1.sslip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://localhost.localtest.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://lvh.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://2130706433/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://0x7f000001/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://017700000001/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://127.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://192.168.257/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:127.0.0.1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[64:ff9b::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:c0a8:101]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2001:0000:4136:e378:8000:63bf:3fff:fdd2]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[100::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2001:20::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[fec0::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "paper", url: "https://[2002:7f00:1::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      {
        sourceReferences: [
          {
            label: "paper",
            url: "https://example.invalid",
            productionGate: { ready: true },
          },
        ],
      },
      /sourceReferences\[0\] field productionGate is not supported/,
    ],
    [
      {
        sourceReferences: [
          { label: "paper", url: "https://zips.z.cash/zip-0224" },
          { label: "paper", url: "https://zips.z.cash/zip-0225" },
        ],
      },
      /sourceReferences\[1\] duplicates label paper/,
    ],
    [
      {
        sourceReferences: [
          { label: "paper A", url: "https://zips.z.cash/zip-0224" },
          { label: "paper B", url: "https://zips.z.cash/zip-0224" },
        ],
      },
      /sourceReferences\[1\] duplicates url https:\/\/zips\.z\.cash\/zip-0224/,
    ],
    [
      { implementationStage: "chain-executable", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", sourceReferences: undefined },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        securityNotes: ["Production readiness requires audit review."],
        sourceReferences: [],
      },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "Zcash Protocol Specification",
            url: "https://zips.z.cash/protocol/protocol.pdf",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /sourceReferences must include exact research target source URLs/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "ZIP 224 Orchard Shielded Protocol",
            url: "https://zips.z.cash/zip-0224",
          },
        ],
        securityNotes: [
          "Orchard note semantics must remain domain-separated.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
        ],
        requiredState: [
          "Orchard note commitment tree",
          "wallet Orchard witness store",
        ],
        failureModes: [
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /securityNotes must include production readiness audit or review gating for research targets/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "ZIP 224 Orchard Shielded Protocol",
            url: "https://zips.z.cash/zip-0224",
          },
        ],
        securityNotes: [
          "Orchard note semantics must remain domain-separated.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
          "notproduction readiness planning remains gated.",
        ],
        requiredState: [
          "Orchard note commitment tree",
          "wallet Orchard witness store",
          "Orchard action-bundle verifier key registry",
        ],
        failureModes: [
          "stale anchor",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register Orchard verifier key parameters."],
        executionSteps: ["Build Orchard proof."],
        proofFamily: "halo2-pasta-action-bundle",
        publicInputsSchema: "anchor,nullifiers,cmx",
        verifierKeyId: "orchard_halo2_action_bundle_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /securityNotes must include production readiness audit or review gating for research targets/,
    ],
    [
      {
        id: "orchard-halo2-actions-v1",
        implementationStage: "research-target-as-of-2026-05",
        sourceReferences: [
          {
            label: "ZIP 224 Orchard Shielded Protocol",
            url: "https://zips.z.cash/zip-0224",
          },
        ],
        securityNotes: [
          "Orchard note semantics must remain domain-separated.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Hardening gates require deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance review, and internal cryptographic review.",
          "not production readiness planning remains gated.",
        ],
        requiredState: [
          "Orchard note commitment tree",
          "wallet Orchard witness store",
          "Orchard action-bundle verifier key registry",
        ],
        failureModes: [
          "stale anchor",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register Orchard verifier key parameters."],
        executionSteps: ["Build Orchard proof."],
        proofFamily: "halo2-pasta-action-bundle",
        publicInputsSchema: "anchor,nullifiers,cmx",
        verifierKeyId: "orchard_halo2_action_bundle_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildOrchardActionBundleProofV1"],
      },
      /securityNotes must include production readiness audit or review gating for research targets/,
    ],
    [
      { implementationStage: "production-hardened", sourceReferences: [] },
      /sourceReferences is required for source-referenced implementation stages/,
    ],
    [
      { sourceReferences: [{ label: "placeholder", url: "https://example.invalid/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "test", url: "https://example.test/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "example", url: "https://example.com/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "localhost", url: "https://localhost/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "loopback", url: "https://127.0.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://10.0.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://172.16.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "private", url: "https://192.168.1.10/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "link local", url: "https://169.254.1.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "carrier nat", url: "https://100.64.0.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://192.0.2.1/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://198.51.100.10/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "documentation", url: "https://203.0.113.5/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 loopback", url: "https://[::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 link local", url: "https://[fe80::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 ula", url: "https://[fc00::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "ipv6 documentation", url: "https://[2001:db8::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "local dns", url: "https://source.local/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { sourceReferences: [{ label: "internal dns", url: "https://source.internal/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      { implementationStage: "sdk-builder", recommendedFor: [] },
      /recommendedFor must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", recommendedFor: undefined },
      /recommendedFor must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "chain-executable", chainRequirements: [] },
      /chainRequirements must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", securityNotes: [] },
      /securityNotes must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", requiredState: [] },
      /requiredState must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "chain-executable", failureModes: [] },
      /failureModes must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "sdk-builder", setupSteps: [] },
      /setupSteps must be non-empty for source-referenced implementation stages/,
    ],
    [
      { implementationStage: "component", executionSteps: [] },
      /executionSteps must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [],
      },
      /source-referenced implementation stages must expose at least one executable or planned SDK entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        publicInputsSchema: null,
        verifierKeyId: null,
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        publicInputsSchema: null,
        verifierKeyId: null,
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        publicInputsSchema: null,
        verifierKeyId: null,
      },
      /publicInputsSchema must be non-empty for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        proofFamily: "none",
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /proofFamily must be a concrete proof family for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "production-hardened",
        proofFamily: "none",
      },
      /proofFamily must be a concrete proof family for source-referenced implementation stages/,
    ],
    [
      {
        id: "transparent-transfer",
        implementationStage: "sdk-builder",
        plannedSdkEntrypoints: ["buildFutureShapeProof"],
      },
      /registered non-none backend family for source-referenced implementation stages/,
    ],
    [
      {
        id: "unmapped-backend-family",
        implementationStage: "production-hardened",
      },
      /registered non-none backend family for source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["buildShapeProof"],
        plannedSdkEntrypoints: [],
      },
      /plannedSdkEntrypoints must be non-empty for pre-production source-referenced implementation stages/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [],
      },
      /plannedSdkEntrypoints must be non-empty for pre-production source-referenced implementation stages/,
    ],
    [
      {
        pqLayers: {
          proof: false,
          authorization: false,
          noteEncryption: false,
          audit: true,
        },
      },
      /pqLayers field audit is not supported/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: false,
          authorization: true,
          noteEncryption: true,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: false,
          noteEncryption: true,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: false,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: false,
          authorization: false,
          noteEncryption: false,
        },
      },
      /coveredCriteria post_quantum requires all pqLayers to be true/,
    ],
    [
      {
        coveredCriteria: [],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        coveredCriteria: ["hide_amount"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        coveredCriteria: ["hide_amount", "hide_sender"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
      },
      /pqLayers with all layers true requires coveredCriteria post_quantum/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
        ],
      },
      /sourceReferences must include NIST FIPS 203, FIPS 204, and FIPS 205/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "encapsulateMlKem",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateNotMlDsaKeyPair",
          "encapsulateMlKem",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateNotMlKem",
        ],
      },
      /plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /securityNotes must include post-quantum primitive risk notes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["not ML-DSA and not ML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /securityNotes must include post-quantum primitive risk notes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["notML-DSA and notML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /securityNotes must include post-quantum primitive risk notes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /failureModes must include post-quantum primitive failure modes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["not ML-DSA or not ML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /failureModes must include post-quantum primitive failure modes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["notML-DSA or notML-KEM domain mismatch"],
        requiredState: ["ML-KEM encrypted note payload store"],
      },
      /failureModes must include post-quantum primitive failure modes/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["PQ nullifier set"],
      },
      /requiredState must include post-quantum note-encryption state/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["notML-KEM encrypted note payload store"],
      },
      /requiredState must include post-quantum note-encryption state/,
    ],
    [
      {
        id: "pq-masp-stark-v0",
        implementationStage: "research-target-as-of-2026-05",
        coveredCriteria: ["post_quantum"],
        pqLayers: {
          proof: true,
          authorization: true,
          noteEncryption: true,
        },
        sourceReferences: [
          {
            label: "FIPS 203",
            url: "https://csrc.nist.gov/pubs/fips/203/final",
          },
          {
            label: "FIPS 204",
            url: "https://csrc.nist.gov/pubs/fips/204/final",
          },
          {
            label: "FIPS 205",
            url: "https://csrc.nist.gov/pubs/fips/205/final",
          },
        ],
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildPqMaspStarkTransferProofV0",
          "generateMlDsaKeyPair",
          "encapsulateMlKem",
        ],
        securityNotes: ["ML-DSA and ML-KEM primitive domains require audit"],
        failureModes: ["ML-DSA or ML-KEM domain mismatch"],
        requiredState: ["not ML-KEM encrypted note payload store"],
      },
      /requiredState must include post-quantum note-encryption state/,
    ],
    [{ sdkEntrypoints: [" buildProof"] }, /sdkEntrypoints\[0\] must be clean and already trimmed/],
    [
      { plannedSdkEntrypoints: ["buildFuture\t"] },
      /plannedSdkEntrypoints\[0\] must be clean and already trimmed/,
    ],
    [{ sdkEntrypoints: ["buildProof-withSuffix"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["build$Proof"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["_buildProof"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["buildProof_"] }, /must be an SDK entrypoint name/],
    [{ sdkEntrypoints: ["build_Proof"] }, /must be an SDK entrypoint name/],
    [
      { sdkEntrypoints: ["Iroha._Privacy.buildProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { sdkEntrypoints: ["Iroha.Privacy_.buildProof"] },
      /must be an SDK entrypoint name/,
    ],
    [{ plannedSdkEntrypoints: ["buildFuture$Proof"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["_buildFutureProof"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["buildFutureProof_"] }, /must be an SDK entrypoint name/],
    [{ plannedSdkEntrypoints: ["buildFuture_Proof"] }, /must be an SDK entrypoint name/],
    [
      { plannedSdkEntrypoints: ["Iroha._Privacy.buildFutureProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy_.buildFutureProof"] },
      /must be an SDK entrypoint name/,
    ],
    [
      { plannedSdkEntrypoints: ["buildFutureDev.Proof.Fixture"] },
      /fixture\/mock entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["verifyFutureShapeProofLocally"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["verifyFutureShapeProofLocal"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy.verifyFutureShapeProofLocally"] },
      /local-only verifier entrypoint/,
    ],
    [
      { plannedSdkEntrypoints: ["Iroha.Privacy.verifyFutureShapeProofLocalVerifier"] },
      /local-only verifier entrypoint/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /chain-executable targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["buildShapeDev.Proof.Fixture"],
      },
      /chain-executable targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["verifyShapeProofLocally"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["verifyShapeProofLocal"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "chain-executable",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
      },
      /chain-executable targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: ["buildShapeInstruction"],
        plannedSdkEntrypoints: ["buildShapeProofV1"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: ["Iroha.Privacy.buildShapeInstruction"],
        plannedSdkEntrypoints: ["buildShapeProofV1"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildShapeInstruction"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: "component",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["Iroha.Privacy.buildShapeInstruction"],
      },
      /component targets cannot advertise instruction SDK entrypoint/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape verifier registry"],
        failureModes: ["shape verifier mismatch"],
        chainRequirements: ["shape verifier registry"],
      },
      /ledger-mutating entries require replay, nullifier, revocation, or link-tag protection metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization not replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "not nullifier replay state",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /ledger-mutating entries require replay, nullifier, revocation, or link-tag protection metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization notreplay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "stale notreplay state",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /ledger-mutating entries require replay, nullifier, revocation, or link-tag protection metadata/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape replay guard"],
        failureModes: ["shape replay"],
        chainRequirements: ["shape verifier registry"],
        setupSteps: ["Register shape verifier."],
        executionSteps: ["Submit shape proof."],
      },
      /ledger-mutating entries require explicit typed chain admission metadata/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape replay guard"],
        failureModes: ["shape replay"],
        chainRequirements: ["shape verifier registry"],
        setupSteps: ["Register shape verifier."],
        executionSteps: ["Submit untyped shape noninstruction admission."],
      },
      /ledger-mutating entries require explicit typed chain admission metadata/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape replay guard"],
        failureModes: ["shape replay"],
        chainRequirements: ["shape verifier registry"],
        setupSteps: ["Register shape verifier."],
        executionSteps: ["Submit notzk::ShapeTransfer admission."],
      },
      /ledger-mutating entries require explicit typed chain admission metadata/,
    ],
    [
      {
        implementationStage: null,
        sdkEntrypoints: [],
        plannedSdkEntrypoints: [
          "buildShapeTransferInstruction",
          "buildShapeAuthorizedTransaction",
        ],
        requiredState: ["shape replay guard"],
        failureModes: ["shape replay"],
        chainRequirements: ["shape verifier registry"],
        setupSteps: ["Register shape verifier."],
        executionSteps: ["Submit not typed shape no instruction admission."],
      },
      /ledger-mutating entries require explicit typed chain admission metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: ["authorization replay"],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /ledger-mutating entries require restart\/persistence metadata for root, nullifier, revocation, or replay state/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Replay guard must not persist across restart.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "stale replay state",
          "duplicate replay rejection",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /ledger-mutating entries require restart\/persistence metadata for root, nullifier, revocation, or replay state/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: ["Policy proof review required."],
        requiredState: ["policy commitment registry"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register policy verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /requiredState must include wallet or witness state metadata for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: ["Policy proof review required."],
        requiredState: ["policy commitment registry", "notwallet policy notwitness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register policy verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /requiredState must include wallet or witness state metadata for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: ["Policy proof review required."],
        requiredState: [
          "policy commitment registry",
          "not wallet policy store",
          "no witness state",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register policy verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /requiredState must include wallet or witness state metadata for source-referenced privacy flows/,
    ],
    [
      {
        id: "vega-existing-credential-zk-v0",
        category: "credential",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "Vega source",
            url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
          },
        ],
        recommendedFor: ["credential predicate proofs"],
        chainRequirements: ["credential predicate verifier"],
        securityNotes: [
          "Credential proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "credential issuer registry",
          "wallet credential witness store",
          "revocation policy",
        ],
        failureModes: [
          "credential replay",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register credential verifier."],
        executionSteps: ["Build credential proof."],
        proofFamily: "existing-credential-zk",
        publicInputsSchema: "issuer_commitment,credential_schema",
        verifierKeyId: "vega_existing_credential_zk_v0",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildVegaCredentialPredicateProofV0"],
      },
      /requiredState must include credential, identity, or admission commitment\/accumulator state metadata/,
    ],
    [
      {
        id: "vega-existing-credential-zk-v0",
        category: "credential",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "Vega source",
            url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
          },
        ],
        recommendedFor: ["credential predicate proofs"],
        chainRequirements: ["credential predicate verifier"],
        securityNotes: [
          "Credential proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "credential issuer registry",
          "wallet credential witness store",
          "notcommitment predicate store",
          "notaccumulator admission state",
        ],
        failureModes: [
          "credential replay",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register credential verifier."],
        executionSteps: ["Build credential proof."],
        proofFamily: "existing-credential-zk",
        publicInputsSchema: "issuer_commitment,credential_schema",
        verifierKeyId: "vega_existing_credential_zk_v0",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildVegaCredentialPredicateProofV0"],
      },
      /requiredState must include credential, identity, or admission commitment\/accumulator state metadata/,
    ],
    [
      {
        id: "vega-existing-credential-zk-v0",
        category: "credential",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "Vega source",
            url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
          },
        ],
        recommendedFor: ["credential predicate proofs"],
        chainRequirements: ["credential predicate verifier"],
        securityNotes: [
          "Credential proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "credential issuer registry",
          "wallet credential witness store",
          "not commitment predicate store",
          "without accumulator admission state",
        ],
        failureModes: [
          "credential replay",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register credential verifier."],
        executionSteps: ["Build credential proof."],
        proofFamily: "existing-credential-zk",
        publicInputsSchema: "issuer_commitment,credential_schema",
        verifierKeyId: "vega_existing_credential_zk_v0",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildVegaCredentialPredicateProofV0"],
      },
      /requiredState must include credential, identity, or admission commitment\/accumulator state metadata/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include verifier-key record metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["not verifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "without verifier-key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register no verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include verifier-key record metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["notverifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "notverifier-key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register notverifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include verifier-key record metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,policy_hash",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include chain\/domain binding metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Policy proof is not domain separation evidence.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "without tx_digest binding",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key without anchor evidence."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,not_tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include chain\/domain binding metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "notdomain-separation planning remains under review.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,notanchor",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /must include chain\/domain binding metadata for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "domain separator mismatch",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,policy_hash",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /publicInputsSchema must include chain\/domain binding public input for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "domain separator mismatch",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,not_tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /publicInputsSchema must include chain\/domain binding public input for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "domain separator mismatch",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,notanchor",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /publicInputsSchema must include chain\/domain binding public input for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: ["authorization replay"],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /failureModes must include malformed-proof, wrong-verifier-key, and wrong-public-input rejection for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "not malformed proof bytes",
          "not wrong verifier key",
          "no public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /failureModes must include malformed-proof, wrong-verifier-key, and wrong-public-input rejection for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "notmalformed proof bytes",
          "notwrong verifier key",
          "notpublic input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /failureModes must include malformed-proof, wrong-verifier-key, and wrong-public-input rejection for source-referenced verifier entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include wallet\/witness privacy notes for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs are not local and no private input remains protected.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include wallet\/witness privacy notes for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs stay notlocal and notexposed through SDK APIs.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include wallet\/witness privacy notes for source-referenced privacy flows/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Policy proof review required.",
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
        ],
        requiredState: ["policy commitment registry", "wallet policy witness store"],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include deterministic vectors, negative\/adversarial cases, replay\/nullifier rejection tests, parser\/verifier fuzzing, performance, and audit\/review hardening gates for source-referenced entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires not deterministic vectors, no negative/adversarial test cases, without replay/nullifier rejection tests, not parser/verifier fuzzing, no verifier fuzzing, not performance gates, and without audit review.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include deterministic vectors, negative\/adversarial cases, replay\/nullifier rejection tests, parser\/verifier fuzzing, performance, and audit\/review hardening gates for source-referenced entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: ["zkAt verifier key registry"],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires notdeterministic vectors, notnegative/adversarial test cases, notreplay/nullifier rejection tests, notparser/verifier fuzzing, notperformance gates, and notaudit queue.",
        ],
        requiredState: [
          "policy commitment registry",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "policy-root substitution",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: ["Build policy proof."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1"],
      },
      /securityNotes must include deterministic vectors, negative\/adversarial cases, replay\/nullifier rejection tests, parser\/verifier fuzzing, performance, and audit\/review hardening gates for source-referenced entries/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key and persist replay state."],
        executionSteps: ["Build policy proof and update replay guard."],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyProofV1", "buildZkAtPolicyInstruction"],
      },
      /failureModes must include stale-state and duplicate\/replay rejection for ledger-mutating root, nullifier, revocation, or replay state/,
    ],
    [
      {
        id: "zkat-policy-private-auth-v1",
        category: "authorization",
        implementationStage: "sdk-builder",
        sourceReferences: [
          {
            label: "zkAt source",
            url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
          },
        ],
        recommendedFor: ["policy privacy"],
        chainRequirements: [
          "zkAt verifier key registry",
          "typed zk::ZkAtPolicyCommitment instruction admission",
        ],
        securityNotes: [
          "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
          "Replay guard must persist across restart.",
          "Production hardening requires deterministic vectors, negative/adversarial test cases, replay/nullifier rejection tests, parser/verifier fuzzing, performance gates, and internal cryptographic review.",
        ],
        requiredState: [
          "policy commitment registry",
          "authorization replay guard",
          "wallet policy witness store",
          "zkAt verifier key registry",
        ],
        failureModes: [
          "not stale replay state",
          "no duplicate replay rejection",
          "malformed proof bytes",
          "wrong verifier key",
          "public input mismatch",
        ],
        setupSteps: ["Register zkAt verifier key."],
        executionSteps: [
          "Submit typed zk::ZkAtPolicyCommitment instruction with tx_digest.",
        ],
        proofFamily: "zkat-policy-private-authenticator",
        publicInputsSchema: "policy_commitment,tx_digest",
        verifierKeyId: "zkat_policy_private_auth_v1",
        sdkEntrypoints: [],
        plannedSdkEntrypoints: ["buildZkAtPolicyCommitmentInstruction"],
      },
      /failureModes must include stale-state and duplicate\/replay rejection for ledger-mutating root, nullifier, revocation, or replay state/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDev.Proof.Fixture"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["verifyShapeProof"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProductionProof"],
        plannedSdkEntrypoints: ["buildShapeProductionProofV1"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProofEnvelope"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "research-target-as-of-2026-05",
        sdkEntrypoints: ["buildShapeProductionInstruction"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /research targets cannot advertise executable SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildFutureDev.Proof.Fixture"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["buildFutureMockProofV2"],
      },
      /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["verifyShapeProofLocally"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
      },
      /production-hardened targets cannot advertise local-only verifier SDK entrypoints/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["verifyShapeProofLocalVerifier"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "sdk-builder",
        sdkEntrypoints: ["Iroha.Privacy.verifyShapeProofLocally"],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeNotDevFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeNoDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildProofFixture"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildMockProof"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildMockProofV2"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildProof.Fixture"],
      },
      /fixture\/mock SDK entrypoints must use explicit DevFixture names/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevFixture"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: [
          "buildShapeDevProofFixture",
          "verifyShapeProofNotLocalVerifier",
        ],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: [
          "buildShapeDevProofFixture",
          "verifyShapeProofNonLocalOnly",
        ],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProductionProof"],
      },
      /executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK not dev fixture is deterministic only; not production Shape proofs remain not unavailable.",
        ],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: ["The SDK dev fixture is deterministic only."],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: ["Production Shape proofs remain unavailable."],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK notdev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; notproduction Shape proofs remain notunavailable.",
        ],
      },
      /executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: [],
      },
      /executable DevFixture SDK entrypoints must retain planned production SDK entrypoints until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: [
          "buildShapeProductionInstruction",
          "buildShapeProofInstruction",
        ],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeNoProofBuilder"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeNotProofBuilder"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofTransaction"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildSubmitShapeProof"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofEnvelope"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofWitness"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofPublicInputs"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofRequest"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        implementationStage: "validator-scaffold-as-of-2026-05",
        sdkEntrypoints: ["buildShapeDevProofFixture", "verifyShapeProofLocally"],
        securityNotes: [
          "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        ],
        plannedSdkEntrypoints: ["buildShapeProofCommitment"],
      },
      /executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass/,
    ],
    [
      {
        sdkEntrypoints: ["buildProof"],
        plannedSdkEntrypoints: ["buildProof"],
      },
      /is already executable/,
    ],
    [
      {
        implementationStage: "catalog-as-of-2026-05",
        sdkEntrypoints: ["buildForgedProductionProof"],
      },
      /catalog-only targets cannot advertise SDK entrypoints/,
    ],
    [
      {
        implementationStage: "production-hardened",
        plannedSdkEntrypoints: ["buildFutureProductionProof"],
      },
      /production-hardened targets cannot retain planned SDK entrypoints/,
    ],
  ]) {
    assertJsValidatorsReject(patch, pattern);
  }
});

test("explicit DevFixture classifier requires non-negated terminal fixture evidence", () => {
  for (const [entrypoint, expected] of [
    ["buildShapeDevFixture", true],
    ["buildShapeDevFixtureV1", true],
    ["buildShapeDevProofFixture", true],
    ["buildShapeDevProofFixtureV1", true],
    ["Iroha.Privacy.buildShapeDevProofFixture", true],
    ["buildShapeNotDevFixture", false],
    ["buildShapeNoDevFixture", false],
    ["buildShapeNonDevFixture", false],
    ["buildShapeWithoutDevFixture", false],
    ["buildShapeNotDevProofFixture", false],
    ["buildShapeNoDevProofFixture", false],
    ["buildShapeNonDevProofFixture", false],
    ["buildShapeWithoutDevProofFixture", false],
    ["buildShapeDevFixtureFactory", false],
    ["buildShapeDevelopmentFixture", false],
  ]) {
    assert.equal(
      entrypointIsExplicitDevFixture(entrypoint),
      expected,
      `${entrypoint} explicit DevFixture classification`,
    );
  }
});

test("public input schema chain/domain binding matcher rejects negated fragments", () => {
  for (const [publicInputsSchema, expected] of [
    ["policy_commitment,tx_digest", true],
    ["policy_commitment,policy_tx_digest", true],
    ["policy_commitment,tx_digest_v1", true],
    ["policy_commitment,domain_separator", true],
    ["policy_commitment,policy_domain_separator_hash", true],
    ["policy_commitment,anchor", true],
    ["policy_commitment,reference_block_height", true],
    ["policy_commitment,rollup_state_root", true],
    ["policy_commitment,not_tx_digest", false],
    ["policy_commitment,no_chain_id", false],
    ["policy_commitment,non_chain_tag", false],
    ["policy_commitment,without_reference_block", false],
    ["policy_commitment,not_anchor", false],
    ["policy_commitment,policy_not_tx_digest", false],
    ["policy_commitment,not_policy_tx_digest", false],
    ["policy_commitment,no_policy_domain_separator", false],
    ["policy_commitment,policy_without_reference_block", false],
    ["policy_commitment,non_policy_rollup_state", false],
    ["policy_commitment,anchorless", false],
  ]) {
    assert.equal(
      publicInputsSchemaHasChainDomainBinding(publicInputsSchema),
      expected,
      `${publicInputsSchema} chain/domain binding classification`,
    );
  }
});

test("chain/domain binding metadata matcher rejects negated bounded tokens", () => {
  for (const [value, token, expected] of [
    ["domain separation binds the verifier inputs", "domain separation", true],
    ["policy tx_digest binding is explicit", "tx_digest", true],
    ["reference-block finality is pinned", "reference-block", true],
    ["not domain separation evidence", "domain separation", false],
    ["without tx_digest binding", "tx_digest", false],
    ["no policy domain separator", "domain separator", false],
    ["non-domain-separated placeholder", "domain-separated", false],
    ["not_anchor placeholder", "anchor", false],
    ["not a domain separator", "domain separator", false],
  ]) {
    assert.equal(
      catalogTextContainsChainDomainBindingToken(value, token),
      expected,
      `${value} / ${token} chain-domain metadata classification`,
    );
  }
});

test("typed chain admission matcher rejects negated bounded tokens", () => {
  for (const [value, token, expected] of [
    ["typed zk::SubmitShapeTransfer instruction", "typed", true],
    ["typed zk::SubmitShapeTransfer instruction", "instruction", true],
    ["zk::SubmitShapeTransfer", "zk::", true],
    ["not typed shape instruction", "typed", false],
    ["typed shape no instruction admission", "instruction", false],
    ["not transaction admission", "transaction", false],
    ["not zk::SubmitShapeTransfer", "zk::", false],
    ["without zk::SubmitShapeTransfer", "zk::", false],
    ["typed notzk::SubmitShapeTransfer instruction", "zk::", false],
    ["typed not_zk::SubmitShapeTransfer instruction", "zk::", false],
  ]) {
    assert.equal(
      catalogTextContainsTypedAdmissionToken(value, token),
      expected,
      `${value} / ${token} typed admission metadata classification`,
    );
  }
});

test("source hardening metadata matcher rejects negated bounded tokens", () => {
  for (const [value, token, expected] of [
    ["deterministic vectors are required", "deterministic vectors", true],
    ["negative/adversarial test cases are required", "negative/adversarial", true],
    ["replay/nullifier rejection tests are required", "replay/nullifier", true],
    ["parser/verifier fuzzing is required", "parser/verifier fuzzing", true],
    ["performance gates are required", "performance", true],
    ["internal cryptographic review is required", "review", true],
    ["not deterministic vectors", "deterministic vectors", false],
    ["no negative/adversarial test cases", "negative/adversarial", false],
    ["without replay/nullifier rejection tests", "replay/nullifier", false],
    ["not parser/verifier fuzzing", "parser/verifier fuzzing", false],
    ["no verifier fuzzing", "verifier fuzzing", false],
    ["not performance gates", "performance", false],
    ["without audit review", "audit", false],
  ]) {
    assert.equal(
      catalogTextContainsSourceHardeningToken(value, token),
      expected,
      `${value} / ${token} source hardening metadata classification`,
    );
  }
});

test("affirmed metadata matcher rejects negated bounded state tokens", () => {
  for (const [value, token, expected] of [
    ["wallet witness store", "wallet", true],
    ["credential commitment registry", "commitment", true],
    ["accumulator state registry", "accumulator", true],
    ["verifier key registry", "verifier key", true],
    ["malformed proof bytes", "malformed proof", true],
    ["wrong verifier key", "wrong verifier key", true],
    ["public input mismatch", "public input mismatch", true],
    ["authorization replay guard", "replay", true],
    ["nullifier set must persist across restart", "nullifier", true],
    ["nullifier set must persist across restart", "persist", true],
    ["stale replay state", "stale", true],
    ["duplicate nullifier rejection", "duplicate", true],
    ["production readiness audit", "production", true],
    ["production readiness audit", "audit", true],
    ["ML-DSA and ML-KEM domains", "ML-DSA", true],
    ["ML-DSA and ML-KEM domains", "ML-KEM", true],
    ["not wallet state", "wallet", false],
    ["no witness store", "witness", false],
    ["non-wallet placeholder", "wallet", false],
    ["without commitment registry", "commitment", false],
    ["not accumulator state", "accumulator", false],
    ["not verifier key registry", "verifier key", false],
    ["without verifier-key registration", "verifier-key", false],
    ["not malformed proof bytes", "malformed proof", false],
    ["not wrong verifier key", "wrong verifier key", false],
    ["no public input mismatch", "public input mismatch", false],
    ["not replay guard", "replay", false],
    ["without nullifier persistence", "nullifier", false],
    ["not persist across restart", "persist", false],
    ["not stale replay state", "stale", false],
    ["no duplicate nullifier rejection", "duplicate", false],
    ["not production readiness audit", "production", false],
    ["no audit review", "audit", false],
    ["not ML-DSA domain", "ML-DSA", false],
    ["without ML-KEM state", "ML-KEM", false],
  ]) {
    assert.equal(
      catalogTextContainsAffirmedMetadataToken(value, token),
      expected,
      `${value} / ${token} affirmed metadata classification`,
    );
  }
});

test("wallet witness privacy matcher preserves exposure negation and rejects state negation", () => {
  for (const [value, token, expected] of [
    ["wallet witness material stays local", "wallet", true],
    ["wallet witness material stays local", "local", true],
    ["private inputs must not be exposed", "not be exposed", true],
    ["plaintext must not leak", "must not leak", true],
    ["secrets never leave the wallet", "never leave", true],
    ["must not leak wallet note ownership", "wallet", true],
    ["must not expose wallet witness data", "wallet", true],
    ["never leave the wallet", "wallet", true],
    ["wallet witness material is not local", "local", false],
    ["no private input remains protected", "private input", false],
    ["without wallet witness custody", "wallet", false],
    ["not secret material", "secret", false],
  ]) {
    assert.equal(
      catalogTextContainsWalletWitnessPrivacyToken(value, token),
      expected,
      `${value} / ${token} wallet witness privacy classification`,
    );
  }
});

test("planned ledger mutation classifier requires non-negated entrypoint evidence", () => {
  for (const [entrypoint, expected] of [
    ["buildShapeTransferInstruction", true],
    ["Iroha.Privacy.buildShapeTransferInstruction", true],
    ["buildShapeInstructionV1", true],
    ["buildShapeAuthorizedTransaction", true],
    ["buildShapeTransactionV1", true],
    ["buildSubmitShapeProof", true],
    ["buildSubmitShapeProofV1", true],
    ["buildShapeNoInstruction", false],
    ["buildShapeNotInstruction", false],
    ["buildShapeNonInstruction", false],
    ["buildShapeWithoutInstruction", false],
    ["buildShapeNoTransaction", false],
    ["buildShapeNotTransaction", false],
    ["buildMidenStarkTransactionProofV1", false],
    ["buildNoSubmitShapeProof", false],
    ["buildNotSubmitShapeProof", false],
    ["buildNonSubmitShapeProof", false],
    ["buildWithoutSubmitShapeProof", false],
    ["buildShapeInstructionalProof", false],
    ["buildShapeTransactionalProof", false],
    ["buildShapeSubmitterProof", false],
  ]) {
    assert.equal(
      entrypointIsPlannedLedgerMutation(entrypoint),
      expected,
      `${entrypoint} planned ledger mutation classification`,
    );
  }
});

test("planned privacy SDK entrypoints remain unexported until production gates pass", () => {
  const descriptors = getSrcPrivacyAlgorithmDescriptors();
  const plannedEntryPoints = new Set(
    descriptors.flatMap((descriptor) => descriptor.plannedSdkEntrypoints),
  );
  const executableEntryPoints = new Set(
    descriptors.flatMap((descriptor) => descriptor.sdkEntrypoints),
  );
  const sourceCapabilityKeys = new Set(Object.keys(getSrcPrivacyCapabilities()));
  const distCapabilityKeys = new Set(Object.keys(getDistPrivacyCapabilities()));
  const publicApiDeclarationTexts = PUBLIC_PRIVACY_API_DECLARATION_SURFACES.map(
    ({ label, path }) => [label, fileText(path)],
  );
  const publicApiSourceTexts = publicPrivacyApiSourceTexts();
  const moduleExportSurfaces = [
    ["JS src package", jsSrcPackage],
    ["JS src crypto", jsSrcCrypto],
    ["JS src browser crypto", jsSrcBrowserCrypto],
    ["JS src instruction builders", jsSrcInstructionBuilders],
    ["JS dist package", jsDistPackage],
    ["JS dist crypto", jsDistCrypto],
    ["JS dist browser crypto", jsDistBrowserCrypto],
    ["JS dist instruction builders", jsDistInstructionBuilders],
  ];

  assertExecutableEntrypointsExported("JS src package", descriptors, jsSrcPackage);
  assertExecutableEntrypointsExported(
    "JS dist package",
    getDistPrivacyAlgorithmDescriptors(),
    jsDistPackage,
  );
  assertExecutableEntrypointsDeclared(
    "JS TypeScript declarations",
    descriptors,
    fileText(JS_DECLARATIONS),
  );

  assert.ok(
    plannedEntryPoints.size > 0,
    "privacy catalog must include planned production entrypoints",
  );
  for (const entrypoint of plannedEntryPoints) {
    assert.equal(
      executableEntryPoints.has(entrypoint),
      false,
      `${entrypoint} must not be both planned and executable`,
    );
    for (const [label, moduleExports] of moduleExportSurfaces) {
      for (const name of publicApiNameVariants(entrypoint)) {
        assert.equal(
          Object.hasOwn(moduleExports, name),
          false,
          `${entrypoint} must not be exported as ${name} from ${label} until production gates pass`,
        );
      }
    }
    for (const [label, text] of publicApiDeclarationTexts) {
      for (const name of publicApiNameVariants(entrypoint)) {
        assert.equal(
          new RegExp(`\\b${escapeRegExp(name)}\\b`).test(text),
          false,
          `${entrypoint} must not be declared as ${name} in ${label} until production gates pass`,
        );
      }
    }
    for (const source of publicApiSourceTexts) {
      for (const name of publicApiNameVariants(entrypoint)) {
        for (const pattern of publicDeclarationPatterns(source.language, name)) {
          assert.equal(
            pattern.test(source.text),
            false,
            `${entrypoint} must not be publicly declared as ${name} in ${source.label} ${source.path} until production gates pass`,
          );
        }
      }
    }
    for (const name of publicApiNameVariants(entrypoint)) {
      const capabilityKey = snakeEntrypointName(name);
      assert.equal(
        sourceCapabilityKeys.has(capabilityKey),
        false,
        `${entrypoint} must not have a JS src capability key ${capabilityKey} until production gates pass`,
      );
      assert.equal(
        distCapabilityKeys.has(capabilityKey),
        false,
        `${entrypoint} must not have a JS dist capability key ${capabilityKey} until production gates pass`,
      );
    }
  }

  for (const descriptor of descriptors) {
    if (descriptor.plannedSdkEntrypoints.length > 0) {
      assert.equal(descriptor.productionReady, false);
      assert.equal(descriptor.productionGate.ready, false);
      assert.ok(
        descriptor.productionGate.missing.includes("planned SDK entrypoints remain"),
        `${descriptor.id} must explain that planned SDK entrypoints still block production`,
      );
    }
  }

  const anonymousPgc = descriptors.find(
    (descriptor) => descriptor.id === "anonymous-pgc-k-out-of-n-v1",
  );
  assert.ok(anonymousPgc, "Anonymous PGC catalog row must exist");
  assert.deepEqual(
    anonymousPgc.chainRequirements.filter((requirement) =>
      requirement.includes("zk::") && requirement.includes("instruction"),
    ),
    [
      "typed zk::RegisterAnonymousPgcAccountCommitment instruction",
      "typed zk::SubmitAnonymousPgcTransfer instruction",
    ],
    "Anonymous PGC planned ledger mutations must retain explicit typed zk:: instruction metadata",
  );
  const zkat = descriptors.find(
    (descriptor) => descriptor.id === "zkat-policy-private-auth-v1",
  );
  assert.ok(zkat, "ZK-AT catalog row must exist");
  assert.deepEqual(
    zkat.chainRequirements.filter((requirement) => requirement.includes("zk::")),
    [
      "typed zk::RegisterZkAtPolicyCommitment instruction",
      "typed zk::SubmitZkAtAuthorizedTransaction admission",
    ],
    "ZK-AT planned ledger mutations must retain explicit typed zk:: admission metadata",
  );
});
