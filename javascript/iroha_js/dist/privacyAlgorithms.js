import { isPrivacyNativeAvailable } from "./crypto.js";

const PRIVACY_CRITERIA = Object.freeze([
  "hide_amount",
  "hide_sender",
  "hide_receiver",
  "hide_asset_type",
  "post_quantum",
]);
const PRIVACY_CRITERIA_SET = new Set(PRIVACY_CRITERIA);

const ALLOWED_CATEGORIES = new Set([
  "payment",
  "authorization",
  "credential",
  "admission",
  "identity",
  "proof_backend",
]);

const ALLOWED_MATURITIES = new Set([
  "peer_reviewed",
  "accepted_conference",
  "technical_report",
  "arxiv_preprint",
  "specification",
]);

const DERIVED_COMPATIBILITY_FIELDS = Object.freeze([
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
const PRIVACY_DESCRIPTOR_FIELDS = new Set([
  "id",
  "name",
  "shortName",
  "summary",
  "category",
  "maturity",
  "coveredCriteria",
  "proofFamily",
  "publicInputsSchema",
  "verifierKeyId",
  "pqLayers",
  "implementationStage",
  "recommendedFor",
  "sourceReferences",
  "securityNotes",
  "requiredState",
  "failureModes",
  "setupSteps",
  "executionSteps",
  "sdkEntrypoints",
  "plannedSdkEntrypoints",
  "chainRequirements",
]);
const SOURCE_REFERENCE_FIELDS = new Set(["label", "url"]);
const PQ_LAYER_FIELDS = new Set(["proof", "authorization", "noteEncryption"]);
const SOURCE_REFERENCE_LABEL_MAX_LENGTH = 160;
const SOURCE_REFERENCE_URL_DECODE_MAX_DEPTH = 8;

const PQ_LAYER_NONE = Object.freeze({
  proof: false,
  authorization: false,
  noteEncryption: false,
});

const RESEARCH_STAGE_MAY_2026 = "research-target-as-of-2026-05";
const CATALOG_STAGE_MAY_2026 = "catalog-as-of-2026-05";
const ALLOWED_IMPLEMENTATION_STAGES = new Set([
  "validator-scaffold-as-of-2026-05",
  "chain-executable",
  "sdk-builder",
  "component",
  RESEARCH_STAGE_MAY_2026,
  CATALOG_STAGE_MAY_2026,
  "production-hardened",
]);
const SOURCE_REFERENCED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "component",
  RESEARCH_STAGE_MAY_2026,
  "production-hardened",
]);
const PRE_PRODUCTION_SOURCE_REFERENCED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  "component",
  RESEARCH_STAGE_MAY_2026,
]);
const SOURCE_REFERENCED_REQUIRED_LIST_FIELDS = Object.freeze([
  "recommendedFor",
  "chainRequirements",
  "securityNotes",
  "requiredState",
  "failureModes",
  "setupSteps",
  "executionSteps",
]);
const SOURCE_REFERENCED_REQUIRED_VERIFIER_FIELDS = Object.freeze([
  "publicInputsSchema",
  "verifierKeyId",
]);
const SOURCE_REFERENCED_FORBIDDEN_PROOF_FAMILIES = new Set(["none"]);
const SOURCE_REFERENCED_FORBIDDEN_BACKEND_FAMILIES = new Set(["none"]);
const SOURCE_REFERENCED_SDK_ENTRYPOINT_FIELDS = Object.freeze([
  "sdkEntrypoints",
  "plannedSdkEntrypoints",
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
  "chainRequirements",
  "setupSteps",
  "executionSteps",
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
  "securityNotes",
  "failureModes",
  "setupSteps",
  "executionSteps",
  "chainRequirements",
]);
const STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["persist", "persistence", "restart", "recovery"]),
  Object.freeze(["replay", "nullifier", "revocation", "link-tag", "link tag"]),
]);
const WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES = new Set([
  "chain-executable",
  "sdk-builder",
  RESEARCH_STAGE_MAY_2026,
  "production-hardened",
]);
const WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES = new Set(["proof_backend"]);
const WALLET_STATE_METADATA_TOKENS = Object.freeze(["wallet", "witness"]);
const CREDENTIAL_STATE_REQUIRED_CATEGORIES = new Set([
  "admission",
  "credential",
  "identity",
]);
const CREDENTIAL_STATE_METADATA_TOKENS = Object.freeze(["commitment", "accumulator"]);
const VERIFIER_KEY_RECORD_METADATA_FIELDS = Object.freeze([
  "requiredState",
  "chainRequirements",
  "setupSteps",
]);
const VERIFIER_KEY_RECORD_METADATA_TOKENS = Object.freeze(["verifier key", "verifier-key"]);
const CHAIN_DOMAIN_BINDING_METADATA_FIELDS = Object.freeze([
  "publicInputsSchema",
  "securityNotes",
  "failureModes",
  "setupSteps",
  "executionSteps",
]);
const CHAIN_DOMAIN_BINDING_METADATA_TOKENS = Object.freeze([
  "domain_separator",
  "domain-separat",
  "domain separat",
  "chain_id",
  "chain_tag",
  "tx_digest",
  "transaction",
  "reference_block",
  "reference block",
  "rollup_state",
  "rollup state",
  "anchor",
  "epoch",
]);
const SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS = Object.freeze([
  Object.freeze(["audit", "audited", "review"]),
  Object.freeze(["fuzz", "fuzzing"]),
  Object.freeze(["performance", "benchmark", "latency"]),
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
const SOURCE_REFERENCE_AUDIT_CLAIM_LABEL_PHRASES = Object.freeze([
  "security review",
  "external review",
  "production review",
  "assurance report",
  "attestation report",
]);
const SOURCE_REFERENCE_AUDIT_CLAIM_COMPACT_FRAGMENTS = Object.freeze([
  "securityreview",
  "externalreview",
  "productionreview",
  "assurancereport",
  "attestationreport",
]);
const SECURITY_NOTE_COMPLETED_AUDIT_CLAIM_COMPACT_FRAGMENTS = Object.freeze([
  "auditcomplete",
  "auditcompleted",
  "auditpassed",
  "auditapproved",
  "auditcleared",
  "auditsignoff",
  "externalauditcomplete",
  "externalauditcompleted",
  "externalauditpassed",
  "externalauditapproved",
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
  "auditclaim",
  "claimedaudit",
]);
const DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS = Object.freeze([
  "productionready",
  "productionhardened",
  "productionenabled",
  "productionapproved",
  "productioncertified",
  "mainnetready",
  "mainnetcomplete",
  "auditedproduction",
  "externallyaudited",
  "auditpassed",
  "auditapproved",
  "auditsignoff",
  "securityreviewpassed",
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
  "auditcomplete",
  "auditcompleted",
  "auditclaim",
  "claimedaudit",
  "externalauditcomplete",
  "externalauditcompleted",
  "externalauditpassed",
  "externalauditapproved",
  "securityreviewcomplete",
  "securityreviewcompleted",
  "securityreviewapproved",
]);
const CATALOG_LABEL_PRODUCTION_CLAIM_COMPACT_FRAGMENTS = Object.freeze([
  ...DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS,
]);
const PLACEHOLDER_SOURCE_REFERENCE_HOSTS = new Set([
  "127.0.0.1",
  "example.com",
  "example.net",
  "example.org",
  "localhost",
]);
const PLACEHOLDER_SOURCE_REFERENCE_SUFFIXES = Object.freeze([
  ".example",
  ".invalid",
  ".test",
]);
const LOCAL_SOURCE_REFERENCE_SUFFIXES = Object.freeze([
  ".internal",
  ".lan",
  ".local",
  ".localhost",
]);
const REBINDING_SOURCE_REFERENCE_HOSTS = new Set([
  "localtest.me",
  "lvh.me",
  "nip.io",
  "sslip.io",
]);
const REBINDING_SOURCE_REFERENCE_SUFFIXES = Object.freeze([
  ".localtest.me",
  ".nip.io",
  ".sslip.io",
]);
const PRODUCTION_GATE_VERSION = "privacy-production-gate-v1";
const PRODUCTION_GATE_REQUIREMENTS = Object.freeze([
  Object.freeze(["real_proving", "real proving engine is not registered"]),
  Object.freeze(["real_verification", "real verifier is not registered"]),
  Object.freeze(["chain_admission", "chain admission path is not enabled"]),
  Object.freeze(["sdk_parity", "cross-SDK parity is incomplete"]),
  Object.freeze(["wallet_state", "wallet/state support is incomplete"]),
  Object.freeze(["deterministic_tests", "deterministic tests are incomplete"]),
  Object.freeze(["fuzzing", "fuzzing gate is incomplete"]),
  Object.freeze(["performance_gates", "performance gate is incomplete"]),
  Object.freeze(["external_audit", "external audit signoff is missing"]),
]);
const PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE =
  "implementation stage is not production-hardened";
const PRODUCTION_GATE_MISSING_PLANNED_SDK = "planned SDK entrypoints remain";
const PRODUCTION_GATE_MISSING_DEV_FIXTURE =
  "dev fixture entrypoints are not production entrypoints";
const PRODUCTION_GATE_MISSING_ALLOWLIST =
  "Iroha production allowlist is not enabled for this audited row";
const PRODUCTION_GATE_SUPPLEMENTAL_MISSING_REASONS = Object.freeze([
  PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE,
  PRODUCTION_GATE_MISSING_PLANNED_SDK,
  PRODUCTION_GATE_MISSING_DEV_FIXTURE,
  PRODUCTION_GATE_MISSING_ALLOWLIST,
]);
const BACKEND_FAMILY_BY_ALGORITHM_ID = Object.freeze({
  "transparent-transfer": "none",
  shield: "commitment-only",
  "confidential-transfer-v2": "halo2-ipa-pasta",
  unshield: "halo2-ipa-pasta",
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
});
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
    "active identity commitment registry",
    "replay nullifier set",
    "authorization verifier registry",
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
  "zk-x509-onchain-identity-v0": Object.freeze(["expired certificate", "revoked certificate", "unknown CA root", "wrong wallet address binding", "stale revocation root", "malformed proof bytes", "wrong verifier key", "public input mismatch"]),
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
  "anonymous-pgc-k-out-of-n-v1": Object.freeze(["Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "verange-transparent-range-v1": Object.freeze(["This is a component, not a complete payment protocol.", "Range parameters must be bound to the transaction payload and verifier key.", "Aggregated proof limits must be enforced by validators.", "Local verification is limited to deterministic dev fixtures; the production VeRange prover remains unavailable.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "zkat-policy-private-auth-v1": Object.freeze(["Hides authorization policy, not payment fields.", "Policy commitments require explicit epoch, replay, and rotation semantics.", "Combining with ZK-ACE requires both proofs to bind the same transaction digest.", "The SDK dev fixture verifies deterministic binding only; chain policy state and production zkAt proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "zk-ams-recursive-admission-v0": Object.freeze(["Admission privacy is separate from later payment privacy.", "Duplicate admission prevention depends on issuer-scoped nullifiers.", "Recursive batching must bind every admitted account commitment.", "The SDK dev fixture verifies deterministic binding only; chain admission state and production recursive proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "vega-existing-credential-zk-v0": Object.freeze(["Credential schema parsing must be deterministic and versioned.", "Proofs must bind to wallet or identity commitments to prevent credential replay.", "Issuer trust and revocation semantics remain external policy inputs.", "The SDK dev fixture verifies deterministic binding only; chain credential policy state and production Vega proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "silent-threshold-anoncred-v0": Object.freeze(["Credential issuance and revocation governance are as important as proof verification.", "Issuer-set commitments need rotation and downgrade protections.", "This is a credential layer, not a private payment protocol.", "The SDK dev fixture verifies deterministic binding only; chain credential state and production silent-threshold proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "zk-x509-onchain-identity-v0": Object.freeze(["Legacy X.509 trust roots are usually not post-quantum.", "Revocation root freshness must be explicit in the public inputs.", "Address binding must prevent proof replay across wallets and chains.", "The SDK dev fixture verifies deterministic public-input binding only; chain trust-root, revocation, policy state, and production ZK-X.509 proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "jindo-lattice-pcs-zk-v0": Object.freeze(["This is a proof backend candidate, not a transaction algorithm.", "PQ proof coverage alone does not imply PQ authorization or note encryption.", "Parameter selection and implementation security require independent review.", "The SDK dev fixture verifies deterministic public-input binding only; production Jindo lattice proving and verifier backends remain unavailable.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "sis-hints-anoncred-pq-v0": Object.freeze(["This is a credential foundation, not an immediately deployable wallet protocol.", "PQ credential proof coverage does not make a payment flow end-to-end post-quantum.", "Parameter choices and reduction assumptions need explicit governance.", "The SDK dev fixture verifies deterministic public-input binding only; production SIS-with-hints credential proving and verifier backends remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "zk-ace-pq-authorization-v0": Object.freeze(["Authorization is only one PQ layer; proof backend and note encryption must also be PQ before a payment flow is end-to-end post-quantum.", "Replay nullifiers must be chain-domain separated and irreversible after acceptance.", "A dev verifier must never be accepted under a production verifier key id.", "Native AIR openings are blinded so sampled rows do not recover identity or replay witness limbs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "orchard-halo2-actions-v1": Object.freeze(["Orchard actions require circuit-compatible note/nullifier semantics and domain-separated action hashes.", "Viewing-key and outgoing-viewing metadata must remain wallet-local.", "Production readiness requires audited Halo2 parameters and note-encryption review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "penumbra-masp-v1": Object.freeze(["Typed asset values must bind asset identifiers to balance commitments.", "Groth16 parameter registration must distinguish spend and output circuits.", "Wallet note plaintexts and position metadata must not be exposed through public APIs.", "Production MASP use requires audited parameter governance and chain-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "monero-fcmp-plus-plus-v1": Object.freeze(["Full-chain membership roots must be canonical and replay protected.", "Link tags/key images must be unique without revealing owned outputs.", "Range-proof and amount-commitment parameters require production verifier review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "miden-stark-note-v1": Object.freeze(["Private note data and off-chain delivery metadata must stay wallet-local.", "Account-local transition proofs must bind initial and final account commitments.", "Reference blocks must prevent replay against stale account state.", "Production Miden note transactions require audited STARK parameters and account-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "aztec-private-rollup-v1": Object.freeze(["Private-kernel proofs must bind note hashes, nullifiers, encrypted logs, and public calls.", "Encrypted log delivery metadata must not leak wallet note ownership.", "Recursive verifier registration must distinguish private-kernel versions and rollup state roots.", "Production private-rollup use requires audited private-kernel parameters and rollup-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
  "pq-masp-stark-v0": Object.freeze(["PQ MASP combines experimental STARK/FRI proving with production PQ authorization and note encryption requirements.", "ML-DSA domains and ML-KEM ciphertext formats must be bound to verifier keys and pool identifiers.", "Post-quantum readiness still requires parameter review, parser fuzzing, and external audit.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."]),
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
  "anonymous-pgc-k-out-of-n-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "verange-transparent-range-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "zkat-policy-private-auth-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "zk-ams-recursive-admission-v0": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "vega-existing-credential-zk-v0": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "silent-threshold-anoncred-v0": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "zk-x509-onchain-identity-v0": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "jindo-lattice-pcs-zk-v0": Object.freeze({ proof: true, authorization: false, noteEncryption: false }),
  "sis-hints-anoncred-pq-v0": Object.freeze({ proof: true, authorization: false, noteEncryption: false }),
  "zk-ace-pq-authorization-v0": Object.freeze({ proof: true, authorization: true, noteEncryption: false }),
  "orchard-halo2-actions-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "penumbra-masp-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "monero-fcmp-plus-plus-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "miden-stark-note-v1": Object.freeze({ proof: true, authorization: false, noteEncryption: false }),
  "aztec-private-rollup-v1": Object.freeze({ proof: false, authorization: false, noteEncryption: false }),
  "pq-masp-stark-v0": Object.freeze({ proof: true, authorization: true, noteEncryption: true }),
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
  "zk-ace-pq-authorization-v0": Object.freeze(["active identity commitment registry", "replay nullifier set", "authorization verifier registry", "wallet identity witness and replay-secret store"]),
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

function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${context} must be a non-empty string`);
  }
}

function assertCleanNonEmptyString(value, context) {
  assertNonEmptyString(value, context);
  if (!isCleanCatalogString(value)) {
    throw new Error(`${context} must be clean and already trimmed`);
  }
}

function isCleanCatalogString(value) {
  return value === value.trim() && !/\p{C}/u.test(value);
}

function isCleanStringListItem(value) {
  return isCleanCatalogString(value);
}

function assertStringList(value, context, { required = false } = {}) {
  if (value === undefined && !required) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new Error(`${context} must be an array`);
  }
  const seen = new Set();
  value.forEach((item, index) => {
    if (typeof item !== "string") {
      throw new Error(`${context}[${index}] must be a string`);
    }
    if (item.trim() === "") {
      throw new Error(`${context}[${index}] must be a non-empty string`);
    }
    if (!isCleanStringListItem(item)) {
      throw new Error(`${context}[${index}] must be clean and already trimmed`);
    }
    if (seen.has(item)) {
      throw new Error(`${context}[${index}] duplicates ${item}`);
    }
    seen.add(item);
  });
  return value;
}

function isLowercaseHyphenatedIdentifier(value) {
  return (
    typeof value === "string" &&
    value.trim() === value &&
    /^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$/.test(value) &&
    !value.includes("--")
  );
}

function isSdkEntrypointName(value) {
  return /^[A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z][A-Za-z0-9]*)*$/.test(value);
}

function isPublicInputSchemaToken(value) {
  return /^[a-z](?:[a-z0-9_]*[a-z0-9])?$/.test(value) && !value.includes("__");
}

function publicInputSchemaTokenHasPayloadMetadata(value) {
  return value
    .split("_")
    .some((segment) => PUBLIC_INPUT_SCHEMA_FORBIDDEN_PAYLOAD_TOKEN_SEGMENTS.includes(segment));
}

function isProofFamilyName(value) {
  return /^[a-z0-9]+(?:[-/][a-z0-9]+)*$/.test(value);
}

function isBackendFamilyName(value) {
  return /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value) && !value.includes("--");
}

function isVerifierKeyName(value) {
  return /^[a-z](?:[a-z0-9_]*[a-z0-9])?$/.test(value) && !value.includes("__");
}

function isVerifierKeySuffix(value) {
  return /^[A-Za-z](?:[A-Za-z0-9_]*[A-Za-z0-9])?$/.test(value) && !value.includes("__");
}

function isVerifierKeyId(value) {
  const parts = value.split("::");
  if (parts.length === 1) {
    return isVerifierKeyName(parts[0]);
  }
  return parts.length === 2 && isVerifierKeyName(parts[0]) && isVerifierKeySuffix(parts[1]);
}

function validatePublicInputsSchema(value, index) {
  const tokens = value.split(",");
  const seen = new Set();
  tokens.forEach((token, tokenIndex) => {
    if (token === "") {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} must be a non-empty public input name`,
      );
    }
    if (token !== token.trim()) {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} must be clean and already trimmed`,
      );
    }
    if (!isPublicInputSchemaToken(token)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} must be a lowercase public input name`,
      );
    }
    if (publicInputSchemaTokenHasPayloadMetadata(token)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} must not include proof or witness payload metadata; proof and witness bytes are carried separately`,
      );
    }
    if (catalogLabelClaimsProductionReadiness(token)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} must not claim production/mainnet/audit readiness before production gates pass`,
      );
    }
    if (seen.has(token)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.publicInputsSchema token ${tokenIndex} duplicates ${token}`,
      );
    }
    seen.add(token);
  });
}

function isSafeHttpsSourceUrl(value) {
  if (
    value !== value.trim() ||
    value.includes("\\") ||
    /[^\u0021-\u007e]/.test(value) ||
    !value.startsWith("https://") ||
    /%(?![0-9a-fA-F]{2})/.test(value) ||
    sourceReferenceUrlAuthority(value).includes("%")
  ) {
    return false;
  }
  try {
    const parsed = new URL(value);
    return (
      parsed.protocol === "https:" &&
      parsed.hostname !== "" &&
      !sourceReferenceHostnameUsesIdna(parsed.hostname) &&
      parsed.username === "" &&
      parsed.password === ""
    );
  } catch {
    return false;
  }
}

function sourceReferenceHostnameUsesIdna(hostname) {
  return hostname
    .toLowerCase()
    .split(".")
    .some((label) => label.startsWith("xn--"));
}

function isCanonicalSourceReferenceUrl(value) {
  const parsed = new URL(value);
  return parsed.href === value && parsed.port === "" && !parsed.hostname.endsWith(".");
}

function sourceReferenceUrlAuthority(value) {
  const rest = value.slice("https://".length);
  const end = rest.search(/[/?#]/);
  return end === -1 ? rest : rest.slice(0, end);
}

function isSafeSourceReferenceLabel(value) {
  return (
    value !== "" &&
    value === value.trim() &&
    value.length <= SOURCE_REFERENCE_LABEL_MAX_LENGTH &&
    !/[^\u0020-\u007e]/.test(value)
  );
}

const PRODUCTION_CLAIM_CONFUSABLES = Object.freeze(new Map([
  ["\u0430", "a"],
  ["\u0435", "e"],
  ["\u0456", "i"],
  ["\u043E", "o"],
  ["\u0440", "p"],
  ["\u0441", "c"],
  ["\u0443", "y"],
  ["\u03B1", "a"],
  ["\u03B5", "e"],
  ["\u03BF", "o"],
  ["\u03C1", "p"],
]));

function foldProductionClaimText(value) {
  return Array.from(value.normalize("NFKC").toLowerCase(), (char) =>
    PRODUCTION_CLAIM_CONFUSABLES.get(char) ?? char
  ).join("");
}

function compactProductionClaimText(value) {
  return foldProductionClaimText(value).replace(/[^a-z0-9]/g, "");
}

function sourceReferenceLabelClaimsAudit(value) {
  const folded = foldProductionClaimText(value);
  const normalized = folded.replace(/[_-]+/g, " ").replace(/\s+/g, " ");
  const compact = compactProductionClaimText(value);
  return (
    compact.includes("audit") ||
    compact.includes("signoff") ||
    compact.includes("securityreview") ||
    SOURCE_REFERENCE_AUDIT_CLAIM_COMPACT_FRAGMENTS.some((fragment) =>
      compact.includes(fragment)
    ) ||
    SOURCE_REFERENCE_AUDIT_CLAIM_LABEL_PHRASES.some((phrase) =>
      normalized.includes(phrase)
    )
  );
}

function sourceReferenceUrlClaimsAuditOrReadiness(value) {
  const candidates = [];
  const seen = new Set();
  let current = value;
  for (let depth = 0; depth < SOURCE_REFERENCE_URL_DECODE_MAX_DEPTH; depth += 1) {
    if (!seen.has(current)) {
      seen.add(current);
      candidates.push(current);
    }
    if (!current.includes("%")) {
      break;
    }
    let decoded;
    try {
      decoded = decodeURIComponent(current);
    } catch {
      return true;
    }
    if (decoded === current) {
      break;
    }
    current = decoded;
  }
  if (current.includes("%")) {
    try {
      if (decodeURIComponent(current) !== current) {
        return true;
      }
    } catch {
      return true;
    }
  }
  return candidates.some(
    (candidate) =>
      sourceReferenceLabelClaimsAudit(candidate) ||
      catalogLabelClaimsProductionReadiness(candidate) ||
      catalogTextClaimsCompletedAuditOrSignoff(candidate),
  );
}

function catalogTextClaimsCompletedAuditOrSignoff(value) {
  const compact = compactProductionClaimText(value);
  return SECURITY_NOTE_COMPLETED_AUDIT_CLAIM_COMPACT_FRAGMENTS.some((fragment) =>
    compact.includes(fragment)
  );
}

function displayTextClaimsProductionReadiness(value) {
  const compact = compactProductionClaimText(value);
  return DISPLAY_FIELD_PRODUCTION_CLAIM_COMPACT_FRAGMENTS.some((fragment) =>
    compact.includes(fragment)
  );
}

function catalogLabelClaimsProductionReadiness(value) {
  const compact = compactProductionClaimText(value);
  return CATALOG_LABEL_PRODUCTION_CLAIM_COMPACT_FRAGMENTS.some((fragment) =>
    compact.includes(fragment)
  );
}

function isPlaceholderSourceReferenceUrl(value) {
  const hostname = new URL(value).hostname.toLowerCase().replace(/\.$/, "");
  return (
    PLACEHOLDER_SOURCE_REFERENCE_HOSTS.has(hostname) ||
    PLACEHOLDER_SOURCE_REFERENCE_SUFFIXES.some((suffix) => hostname.endsWith(suffix))
  );
}

function isNonGlobalIpv4Address(hostname) {
  if (!/^\d+\.\d+\.\d+\.\d+$/.test(hostname)) {
    return false;
  }
  const parts = hostname.split(".").map((part) => Number.parseInt(part, 10));
  return ipv4OctetsAreNonGlobal(parts);
}

function ipv4OctetsAreNonGlobal(parts) {
  if (parts.some((part) => part < 0 || part > 255)) {
    return true;
  }
  const [a, b, c] = parts;
  return (
    a === 0 ||
    a === 10 ||
    a === 127 ||
    a >= 224 ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    (a === 192 && b === 0 && c === 0) ||
    (a === 192 && b === 0 && c === 2) ||
    (a === 198 && (b === 18 || b === 19)) ||
    (a === 198 && b === 51 && c === 100) ||
    (a === 203 && b === 0 && c === 113)
  );
}

function parseIpv6Hextet(value) {
  if (!/^[0-9a-f]{1,4}$/.test(value)) {
    return null;
  }
  return Number.parseInt(value, 16);
}

function ipv4OctetsFromTwoIpv6Hextets(first, second) {
  return [
    (first >> 8) & 0xff,
    first & 0xff,
    (second >> 8) & 0xff,
    second & 0xff,
  ];
}

function ipv6EmbedsNonGlobalIpv4Address(normalized) {
  if (normalized.startsWith("::ffff:")) {
    const suffix = normalized.slice("::ffff:".length);
    const mappedHextets = suffix.split(":");
    if (mappedHextets.length !== 2) {
      return false;
    }
    const first = parseIpv6Hextet(mappedHextets[0]);
    const second = parseIpv6Hextet(mappedHextets[1]);
    return (
      first !== null &&
      second !== null &&
      ipv4OctetsAreNonGlobal(ipv4OctetsFromTwoIpv6Hextets(first, second))
    );
  }
  if (normalized.startsWith("2002:")) {
    const hextets = normalized.split(":");
    const first = parseIpv6Hextet(hextets[1] ?? "");
    const second = parseIpv6Hextet(hextets[2] ?? "");
    return (
      first !== null &&
      second !== null &&
      ipv4OctetsAreNonGlobal(ipv4OctetsFromTwoIpv6Hextets(first, second))
    );
  }
  return false;
}

function expandIpv6AddressHextets(normalized) {
  const address = normalized.replace(/^\[/, "").replace(/\]$/, "").toLowerCase();
  const doubleColonParts = address.split("::");
  if (doubleColonParts.length > 2) {
    return null;
  }
  const parseHextets = (side) => {
    if (side === "") {
      return [];
    }
    const parsed = [];
    for (const part of side.split(":")) {
      const hextet = parseIpv6Hextet(part);
      if (hextet === null) {
        return null;
      }
      parsed.push(hextet);
    }
    return parsed;
  };

  if (doubleColonParts.length === 1) {
    const hextets = parseHextets(address);
    return hextets !== null && hextets.length === 8 ? hextets : null;
  }

  const left = parseHextets(doubleColonParts[0]);
  const right = parseHextets(doubleColonParts[1]);
  if (left === null || right === null) {
    return null;
  }
  const zeroFill = 8 - left.length - right.length;
  if (zeroFill < 1) {
    return null;
  }
  return [...left, ...Array(zeroFill).fill(0), ...right];
}

function ipv6PrefixMatches(hextets, prefixHextets, prefixBits) {
  let remainingBits = prefixBits;
  for (let index = 0; index < prefixHextets.length && remainingBits > 0; index += 1) {
    const bits = Math.min(16, remainingBits);
    const mask = bits === 16 ? 0xffff : (0xffff << (16 - bits)) & 0xffff;
    if ((hextets[index] & mask) !== (prefixHextets[index] & mask)) {
      return false;
    }
    remainingBits -= bits;
  }
  return true;
}

function expandedIpv6AddressIsNonGlobal(hextets) {
  if (hextets === null) {
    return false;
  }

  const embeddedIpv4 = ipv4OctetsFromTwoIpv6Hextets(hextets[6], hextets[7]);
  const firstSixZero = hextets.slice(0, 6).every((hextet) => hextet === 0);
  const mappedIpv4 = hextets.slice(0, 5).every((hextet) => hextet === 0) && hextets[5] === 0xffff;
  const nat64WellKnown =
    hextets[0] === 0x0064 &&
    hextets[1] === 0xff9b &&
    hextets.slice(2, 6).every((hextet) => hextet === 0);
  if (
    (firstSixZero || mappedIpv4 || nat64WellKnown) &&
    ipv4OctetsAreNonGlobal(embeddedIpv4)
  ) {
    return true;
  }

  if (
    ipv6PrefixMatches(hextets, [0x0100, 0, 0, 0], 64) ||
    ipv6PrefixMatches(hextets, [0x2001, 0x0000], 32) ||
    ipv6PrefixMatches(hextets, [0x2001, 0x0010], 28) ||
    ipv6PrefixMatches(hextets, [0x2001, 0x0020], 28) ||
    ipv6PrefixMatches(hextets, [0x2001, 0x0002, 0], 48) ||
    ipv6PrefixMatches(hextets, [0x2001, 0x0db8], 32)
  ) {
    return true;
  }

  if (
    hextets[0] === 0x2002 &&
    ipv4OctetsAreNonGlobal(ipv4OctetsFromTwoIpv6Hextets(hextets[1], hextets[2]))
  ) {
    return true;
  }

  return false;
}

function isNonGlobalIpv6Address(hostname) {
  const normalized = hostname.replace(/^\[/, "").replace(/\]$/, "").toLowerCase();
  if (!normalized.includes(":")) {
    return false;
  }
  return (
    normalized === "::" ||
    normalized === "::1" ||
    /^fe[89a-f]/.test(normalized) ||
    normalized.startsWith("fc") ||
    normalized.startsWith("fd") ||
    normalized.startsWith("ff") ||
    normalized.startsWith("2001:db8:") ||
    normalized.startsWith("2001:10:") ||
    normalized.startsWith("2001:2:") ||
    ipv6EmbedsNonGlobalIpv4Address(normalized) ||
    expandedIpv6AddressIsNonGlobal(expandIpv6AddressHextets(normalized))
  );
}

function isPrivateOrLocalSourceReferenceUrl(value) {
  const hostname = new URL(value).hostname.toLowerCase().replace(/\.$/, "");
  return (
    LOCAL_SOURCE_REFERENCE_SUFFIXES.some((suffix) => hostname.endsWith(suffix)) ||
    REBINDING_SOURCE_REFERENCE_HOSTS.has(hostname) ||
    REBINDING_SOURCE_REFERENCE_SUFFIXES.some((suffix) => hostname.endsWith(suffix)) ||
    isNonGlobalIpv4Address(hostname) ||
    isNonGlobalIpv6Address(hostname)
  );
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

function entrypointIsExplicitDevFixture(entrypoint) {
  const normalized = entrypoint.replaceAll("-", "_").toLowerCase();
  const compact = entrypoint.toLowerCase().replace(/[^a-z0-9]/g, "");
  return (
    normalized.includes("devfixture") ||
    normalized.includes("dev_fixture") ||
    normalized.includes("devprooffixture") ||
    normalized.includes("dev_proof_fixture") ||
    compact.includes("devfixture") ||
    compact.includes("devprooffixture")
  );
}

function entrypointIsLocalVerifier(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  const lower = name.toLowerCase();
  return (
    lower.startsWith("verify") &&
    (lower.endsWith("locally") ||
      lower.endsWith("local") ||
      lower.includes("localverifier") ||
      lower.includes("localonly"))
  );
}

function entrypointIsInstructionBuilder(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return name.endsWith("Instruction");
}

function entrypointIsPlannedLedgerMutation(entrypoint) {
  const segments = entrypoint.split(".");
  const name = segments[segments.length - 1];
  return (
    name.endsWith("Instruction") ||
    name.endsWith("Transaction") ||
    name.includes("Submit")
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
    name.includes("Proof") &&
    !entrypointIsInstructionBuilder(entrypoint) &&
    !entrypointIsPlannedLedgerMutation(entrypoint) &&
    !entrypointIsProofHelper(entrypoint) &&
    !entrypointIsDevFixture(entrypoint)
  );
}

function hasDevFixtureNonProductionWarning(notes) {
  return notes.some((note) => {
    const normalized = note.toLowerCase();
    return (
      normalized.includes("dev fixture") &&
      normalized.includes("production") &&
      normalized.includes("unavailable")
    );
  });
}

function dedupeStrings(items) {
  const deduped = [];
  for (const item of items) {
    if (!deduped.includes(item)) {
      deduped.push(item);
    }
  }
  return deduped;
}

function deepFreeze(value) {
  if (value === null || typeof value !== "object" || Object.isFrozen(value)) {
    return value;
  }
  for (const nested of Object.values(value)) {
    deepFreeze(nested);
  }
  return Object.freeze(value);
}

function productionGateForDescriptor(descriptor) {
  const gates = Object.fromEntries(
    PRODUCTION_GATE_REQUIREMENTS.map(([key]) => [key, false]),
  );
  const missing = PRODUCTION_GATE_REQUIREMENTS.map(([_key, label]) => label);
  if (descriptor.implementationStage !== "production-hardened") {
    missing.push(PRODUCTION_GATE_MISSING_IMPLEMENTATION_STAGE);
  }
  if ((descriptor.plannedSdkEntrypoints ?? []).length > 0) {
    missing.push(PRODUCTION_GATE_MISSING_PLANNED_SDK);
  }
  if ((descriptor.sdkEntrypoints ?? []).some(entrypointIsDevFixture)) {
    missing.push(PRODUCTION_GATE_MISSING_DEV_FIXTURE);
  }
  missing.push(PRODUCTION_GATE_MISSING_ALLOWLIST);
  return {
    version: PRODUCTION_GATE_VERSION,
    ready: false,
    gates,
    missing: dedupeStrings(missing),
    auditReferences: [],
  };
}

function backendFamilyForDescriptor(descriptor) {
  const backendFamily = BACKEND_FAMILY_BY_ALGORITHM_ID[descriptor.id];
  if (typeof backendFamily !== "string") {
    throw new Error(
      `privacy algorithm descriptor ${descriptor.id} is missing backend family metadata`,
    );
  }
  if (!isBackendFamilyName(backendFamily)) {
    throw new Error(
      `privacy algorithm descriptor ${descriptor.id} backendFamily metadata must be non-empty and use request-portable verifier-key backend characters`,
    );
  }
  if (catalogLabelClaimsProductionReadiness(backendFamily)) {
    throw new Error(
      `privacy algorithm descriptor ${descriptor.id} backendFamily metadata must not claim production/mainnet/audit readiness before production gates pass`,
    );
  }
  return backendFamily;
}

function validateRequiredPrivacyPlanRows(descriptors) {
  const descriptorById = new Map(
    descriptors.map((descriptor) => [descriptor.id, descriptor]),
  );
  for (const [algorithmId, implementationStage, backendFamily] of REQUIRED_PRIVACY_PLAN_ROWS) {
    const descriptor = descriptorById.get(algorithmId);
    if (descriptor == null) {
      throw new Error(
        `privacy algorithm catalog missing required production privacy plan row ${algorithmId}`,
      );
    }
    const displayText =
      REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.name !== displayText[0] ||
      descriptor.shortName !== displayText[1] ||
      descriptor.summary !== displayText[2]
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep display text ${displayText.join(" | ")} until the production inventory is deliberately updated`,
      );
    }
    if (descriptor.implementationStage !== implementationStage) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep implementationStage ${implementationStage} until the production inventory is deliberately updated`,
      );
    }
    if (BACKEND_FAMILY_BY_ALGORITHM_ID[algorithmId] !== backendFamily) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep backend family ${backendFamily} until the production inventory is deliberately updated`,
      );
    }
    const category = REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID[algorithmId];
    if (descriptor.category !== category) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep category ${category} until the production inventory is deliberately updated`,
      );
    }
    const maturity = REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID[algorithmId];
    if (descriptor.maturity !== maturity) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep maturity ${maturity} until the production inventory is deliberately updated`,
      );
    }
    const recommendedFor =
      REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.recommendedFor.length !== recommendedFor.length ||
      !recommendedFor.every(
        (recommendation, recommendationIndex) =>
          descriptor.recommendedFor[recommendationIndex] === recommendation,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep recommendedFor ${recommendedFor.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const coveredCriteria =
      REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.coveredCriteria.length !== coveredCriteria.length ||
      !coveredCriteria.every(
        (criterion, criterionIndex) =>
          descriptor.coveredCriteria[criterionIndex] === criterion,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep covered criteria ${coveredCriteria.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const proofFamily =
      REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID[algorithmId];
    if (descriptor.proofFamily !== proofFamily) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep proof family ${proofFamily} until the production inventory is deliberately updated`,
      );
    }
    const publicInputsSchema =
      REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID[algorithmId];
    if (descriptor.publicInputsSchema !== publicInputsSchema) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep public inputs schema ${publicInputsSchema} until the production inventory is deliberately updated`,
      );
    }
    const verifierKeyId =
      REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID[algorithmId];
    if (descriptor.verifierKeyId !== verifierKeyId) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep verifier key id ${verifierKeyId} until the production inventory is deliberately updated`,
      );
    }
    const pqLayers =
      REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID[algorithmId];
    for (const [layerName, expectedEnabled] of Object.entries(pqLayers)) {
      if ((descriptor.pqLayers ?? {})[layerName] !== expectedEnabled) {
        throw new Error(
          `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep PQ layer ${layerName}=${expectedEnabled} until the production inventory is deliberately updated`,
        );
      }
    }
    const chainRequirements =
      REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.chainRequirements.length !== chainRequirements.length ||
      !chainRequirements.every(
        (requirement, requirementIndex) =>
          descriptor.chainRequirements[requirementIndex] === requirement,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep chain requirements ${chainRequirements.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const requiredState =
      REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.requiredState.length !== requiredState.length ||
      !requiredState.every(
        (stateItem, stateIndex) =>
          descriptor.requiredState[stateIndex] === stateItem,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep required state ${requiredState.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const setupSteps =
      REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.setupSteps.length !== setupSteps.length ||
      !setupSteps.every(
        (setupStep, setupStepIndex) =>
          descriptor.setupSteps[setupStepIndex] === setupStep,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep setup steps ${setupSteps.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const executionSteps =
      REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.executionSteps.length !== executionSteps.length ||
      !executionSteps.every(
        (executionStep, executionStepIndex) =>
          descriptor.executionSteps[executionStepIndex] === executionStep,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep execution steps ${executionSteps.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const failureModes =
      REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.failureModes.length !== failureModes.length ||
      !failureModes.every(
        (failureMode, failureModeIndex) =>
          descriptor.failureModes[failureModeIndex] === failureMode,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep failure modes ${failureModes.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const securityNotes =
      REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.securityNotes.length !== securityNotes.length ||
      !securityNotes.every(
        (securityNote, securityNoteIndex) =>
          descriptor.securityNotes[securityNoteIndex] === securityNote,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep security notes ${securityNotes.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const stateText = [
      ...(descriptor.requiredState ?? []),
      ...(descriptor.chainRequirements ?? []),
    ]
      .map((value) => String(value).toLowerCase())
      .join("\n");
    for (const stateToken of REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID[
      algorithmId
    ]) {
      if (!stateText.includes(stateToken)) {
        throw new Error(
          `privacy algorithm catalog required production privacy plan row ${algorithmId} must retain required state token ${stateToken} until the production inventory is deliberately updated`,
        );
      }
    }
    const failureModeText = (descriptor.failureModes ?? [])
      .map((value) => String(value).toLowerCase())
      .join("\n");
    for (const failureToken of [
      ...REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS,
      ...REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID[algorithmId],
    ]) {
      if (!failureModeText.includes(failureToken)) {
        throw new Error(
          `privacy algorithm catalog required production privacy plan row ${algorithmId} must retain required failure-mode token ${failureToken} until the production inventory is deliberately updated`,
        );
      }
    }
    const sourceReferences =
      REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID[algorithmId];
    for (const sourceReference of sourceReferences) {
      if (
        !(descriptor.sourceReferences ?? []).some(
          (reference) =>
            reference.label === sourceReference.label &&
            reference.url === sourceReference.url,
        )
      ) {
        throw new Error(
          `privacy algorithm catalog required production privacy plan row ${algorithmId} must retain source reference ${sourceReference.label} <${sourceReference.url}> until the production inventory is deliberately updated`,
        );
      }
    }
    const descriptorSourceReferences = descriptor.sourceReferences ?? [];
    if (
      descriptorSourceReferences.length !== sourceReferences.length ||
      !sourceReferences.every(
        (sourceReference, sourceReferenceIndex) =>
          descriptorSourceReferences[sourceReferenceIndex]?.label ===
            sourceReference.label &&
          descriptorSourceReferences[sourceReferenceIndex]?.url ===
            sourceReference.url,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep source references until the production inventory is deliberately updated`,
      );
    }
    const sdkEntrypoints =
      REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[algorithmId];
    if (
      descriptor.sdkEntrypoints.length !== sdkEntrypoints.length ||
      !sdkEntrypoints.every(
        (sdkEntrypoint, sdkEntrypointIndex) =>
          descriptor.sdkEntrypoints[sdkEntrypointIndex] === sdkEntrypoint,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep SDK entrypoints ${sdkEntrypoints.join(",")} until the production inventory is deliberately updated`,
      );
    }
    const plannedSdkEntrypoints =
      REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID[
        algorithmId
      ];
    const descriptorPlannedSdkEntrypoints =
      descriptor.plannedSdkEntrypoints ?? [];
    if (
      descriptorPlannedSdkEntrypoints.length !== plannedSdkEntrypoints.length ||
      !plannedSdkEntrypoints.every(
        (entrypoint, entrypointIndex) =>
          descriptorPlannedSdkEntrypoints[entrypointIndex] === entrypoint,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must keep planned SDK entrypoints ${plannedSdkEntrypoints.join(",")} until the production inventory is deliberately updated`,
      );
    }
    if (
      !(descriptor.plannedSdkEntrypoints ?? []).some(
        entrypointIsProductionProofBuilder,
      )
    ) {
      throw new Error(
        `privacy algorithm catalog required production privacy plan row ${algorithmId} must retain a planned production proof builder until production gates pass`,
      );
    }
  }
}

function validateResearchTargetSdkEntrypoints(descriptors) {
  for (const descriptor of descriptors) {
    if (descriptor.implementationStage !== RESEARCH_STAGE_MAY_2026) {
      continue;
    }
    if ((descriptor.sdkEntrypoints ?? []).length > 0) {
      throw new Error(
        `privacy algorithm catalog research target ${descriptor.id} cannot advertise executable SDK entrypoints; keep them in plannedSdkEntrypoints until the production stage advances`,
      );
    }
  }
}

export function validatePrivacyAlgorithmDescriptor(descriptor, index = 0) {
  if (!isPlainObject(descriptor)) {
    throw new Error(`privacy algorithm descriptor ${index} must be an object`);
  }
  for (const field of DERIVED_COMPATIBILITY_FIELDS) {
    if (Object.hasOwn(descriptor, field)) {
      throw new Error(
        `privacy algorithm descriptor ${index} field ${field} is derived and must not be supplied`,
      );
    }
  }
  for (const field of Object.keys(descriptor)) {
    if (!PRIVACY_DESCRIPTOR_FIELDS.has(field)) {
      throw new Error(
        `privacy algorithm descriptor ${index} field ${field} is not a supported privacy catalog field`,
      );
    }
  }
  for (const field of ["id", "name", "shortName", "summary", "category", "maturity", "proofFamily"]) {
    assertCleanNonEmptyString(descriptor[field], `privacy algorithm descriptor ${index}.${field}`);
  }
  if (!isProofFamilyName(descriptor.proofFamily)) {
    throw new Error(`privacy algorithm descriptor ${index}.proofFamily must be a proof family name`);
  }
  if (catalogLabelClaimsProductionReadiness(descriptor.proofFamily)) {
    throw new Error(
      `privacy algorithm descriptor ${index}.proofFamily must not claim production/mainnet/audit readiness before production gates pass`,
    );
  }
  if (!/^[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?$/.test(descriptor.id)) {
    throw new Error(`privacy algorithm descriptor ${index}.id must be lowercase and URL-safe`);
  }
  if (catalogLabelClaimsProductionReadiness(descriptor.id)) {
    throw new Error(
      `privacy algorithm descriptor ${index}.id must not claim production/mainnet/audit readiness before production gates pass`,
    );
  }
  if (!ALLOWED_CATEGORIES.has(descriptor.category)) {
    throw new Error(`privacy algorithm descriptor ${index}.category must be a known category`);
  }
  if (!ALLOWED_MATURITIES.has(descriptor.maturity)) {
    throw new Error(`privacy algorithm descriptor ${index}.maturity must be a known maturity`);
  }
  if (
    descriptor.implementationStage !== undefined &&
    descriptor.implementationStage !== null &&
    !isLowercaseHyphenatedIdentifier(descriptor.implementationStage)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index}.implementationStage must be a lowercase hyphenated identifier`,
    );
  }
  if (
    descriptor.implementationStage !== undefined &&
    descriptor.implementationStage !== null &&
    !ALLOWED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index}.implementationStage must be a known implementation stage`,
    );
  }

  const criteria = assertStringList(
    descriptor.coveredCriteria,
    `privacy algorithm descriptor ${index}.coveredCriteria`,
    { required: true },
  );
  const seenCriteria = new Set();
  criteria.forEach((criterion, criterionIndex) => {
    if (!PRIVACY_CRITERIA_SET.has(criterion)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.coveredCriteria[${criterionIndex}] must be a known privacy criterion`,
      );
    }
    if (seenCriteria.has(criterion)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.coveredCriteria[${criterionIndex}] duplicates ${criterion}`,
      );
    }
    seenCriteria.add(criterion);
  });

  if (
    descriptor.publicInputsSchema !== null &&
    descriptor.publicInputsSchema !== undefined &&
    (typeof descriptor.publicInputsSchema !== "string" ||
      descriptor.publicInputsSchema.trim() === "")
  ) {
    throw new Error(`privacy algorithm descriptor ${index}.publicInputsSchema must be a non-empty string or null`);
  }
  if (
    typeof descriptor.publicInputsSchema === "string" &&
    !isCleanCatalogString(descriptor.publicInputsSchema)
  ) {
    throw new Error(`privacy algorithm descriptor ${index}.publicInputsSchema must be clean and already trimmed`);
  }
  if (typeof descriptor.publicInputsSchema === "string") {
    validatePublicInputsSchema(descriptor.publicInputsSchema, index);
  }
  if (
    descriptor.verifierKeyId !== null &&
    descriptor.verifierKeyId !== undefined &&
    (typeof descriptor.verifierKeyId !== "string" || descriptor.verifierKeyId.trim() === "")
  ) {
    throw new Error(`privacy algorithm descriptor ${index}.verifierKeyId must be a non-empty string or null`);
  }
  if (
    typeof descriptor.verifierKeyId === "string" &&
    !isCleanCatalogString(descriptor.verifierKeyId)
  ) {
    throw new Error(`privacy algorithm descriptor ${index}.verifierKeyId must be clean and already trimmed`);
  }
  if ((descriptor.publicInputsSchema == null) !== (descriptor.verifierKeyId == null)) {
    throw new Error(
      `privacy algorithm descriptor ${index}.publicInputsSchema and verifierKeyId must be supplied together`,
    );
  }
  if (typeof descriptor.verifierKeyId === "string" && !isVerifierKeyId(descriptor.verifierKeyId)) {
    throw new Error(`privacy algorithm descriptor ${index}.verifierKeyId must be a verifier key id`);
  }
  if (
    typeof descriptor.verifierKeyId === "string" &&
    catalogLabelClaimsProductionReadiness(descriptor.verifierKeyId)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index}.verifierKeyId must not claim production/mainnet/audit readiness before production gates pass`,
    );
  }
  if (!isPlainObject(descriptor.pqLayers)) {
    throw new Error(`privacy algorithm descriptor ${index}.pqLayers must be an object`);
  }
  for (const field of Object.keys(descriptor.pqLayers)) {
    if (!PQ_LAYER_FIELDS.has(field)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.pqLayers field ${field} is not supported`,
      );
    }
  }
  for (const key of ["proof", "authorization", "noteEncryption"]) {
    if (typeof descriptor.pqLayers[key] !== "boolean") {
      throw new Error(`privacy algorithm descriptor ${index}.pqLayers.${key} must be a boolean`);
    }
  }
  const allPqLayers = ["proof", "authorization", "noteEncryption"].every(
    (key) => descriptor.pqLayers[key] === true,
  );
  if (
    criteria.includes("post_quantum") &&
    !allPqLayers
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index}.coveredCriteria post_quantum requires all pqLayers to be true`,
    );
  }
  if (allPqLayers && !criteria.includes("post_quantum")) {
    throw new Error(
      `privacy algorithm descriptor ${index}.pqLayers with all layers true requires coveredCriteria post_quantum`,
    );
  }

  const sdkEntrypoints = assertStringList(
    descriptor.sdkEntrypoints,
    `privacy algorithm descriptor ${index}.sdkEntrypoints`,
    { required: true },
  );
  const plannedSdkEntrypoints = assertStringList(
    descriptor.plannedSdkEntrypoints,
    `privacy algorithm descriptor ${index}.plannedSdkEntrypoints`,
  );
  assertStringList(
    descriptor.chainRequirements,
    `privacy algorithm descriptor ${index}.chainRequirements`,
    { required: true },
  );
  for (const listName of [
    "recommendedFor",
    "securityNotes",
    "requiredState",
    "failureModes",
    "setupSteps",
    "executionSteps",
  ]) {
    assertStringList(descriptor[listName], `privacy algorithm descriptor ${index}.${listName}`);
  }
  (descriptor.securityNotes ?? []).forEach((note, noteIndex) => {
    if (catalogTextClaimsCompletedAuditOrSignoff(note)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.securityNotes[${noteIndex}] must describe missing audit/review gates, not completed audit or signoff claims`,
      );
    }
  });
  (descriptor.failureModes ?? []).forEach((failureMode, failureModeIndex) => {
    if (catalogTextClaimsCompletedAuditOrSignoff(failureMode)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.failureModes[${failureModeIndex}] must describe concrete failure modes, not completed audit or signoff claims`,
      );
    }
  });
  for (const [fieldName, values] of [
    ["name", [descriptor.name]],
    ["shortName", [descriptor.shortName]],
    ["summary", [descriptor.summary]],
    ["recommendedFor", descriptor.recommendedFor ?? []],
  ]) {
    values.forEach((value, valueIndex) => {
      if (displayTextClaimsProductionReadiness(value)) {
        const suffix = fieldName === "recommendedFor" ? `[${valueIndex}]` : "";
        throw new Error(
          `privacy algorithm descriptor ${index}.${fieldName}${suffix} must not claim production/mainnet/audit readiness before production gates pass`,
        );
      }
    });
  }
  for (const fieldName of ["chainRequirements", "requiredState", "setupSteps", "executionSteps"]) {
    (descriptor[fieldName] ?? []).forEach((value, valueIndex) => {
      if (displayTextClaimsProductionReadiness(value)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${fieldName}[${valueIndex}] must not claim production/mainnet/audit readiness before production gates pass`,
        );
      }
    });
  }
  for (const entrypoint of plannedSdkEntrypoints) {
    if (entrypointIsDevFixture(entrypoint)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints entry ${entrypoint} is a fixture/mock entrypoint, not a production entrypoint`,
      );
    }
    if (entrypointIsLocalVerifier(entrypoint)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints entry ${entrypoint} is a local-only verifier entrypoint, not a production entrypoint`,
      );
    }
  }
  for (const [listName, values] of [
    ["sdkEntrypoints", sdkEntrypoints],
    ["plannedSdkEntrypoints", plannedSdkEntrypoints],
  ]) {
    const seen = new Set();
    values.forEach((entrypoint, entrypointIndex) => {
      if (!isSdkEntrypointName(entrypoint)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${listName}[${entrypointIndex}] must be an SDK entrypoint name`,
        );
      }
      if (catalogLabelClaimsProductionReadiness(entrypoint)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${listName}[${entrypointIndex}] must not claim production/mainnet/audit readiness before production gates pass`,
        );
      }
      if (seen.has(entrypoint)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${listName}[${entrypointIndex}] duplicates ${entrypoint}`,
        );
      }
      seen.add(entrypoint);
    });
  }
  for (const entrypoint of plannedSdkEntrypoints) {
    if (sdkEntrypoints.includes(entrypoint)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints entry ${entrypoint} is already executable`,
      );
    }
  }
  if (descriptor.implementationStage === "component") {
    for (const [listName, values] of [
      ["sdkEntrypoints", sdkEntrypoints],
      ["plannedSdkEntrypoints", plannedSdkEntrypoints],
    ]) {
      for (const entrypoint of values) {
        if (entrypointIsInstructionBuilder(entrypoint)) {
          throw new Error(
            `privacy algorithm descriptor ${index} component targets cannot advertise instruction SDK entrypoint ${entrypoint} in ${listName}`,
          );
        }
      }
    }
  }
  if (
    descriptor.implementationStage === RESEARCH_STAGE_MAY_2026 &&
    sdkEntrypoints.some(entrypointIsDevFixture)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} research targets cannot advertise fixture/mock SDK entrypoints`,
    );
  }
  if (
    descriptor.implementationStage === RESEARCH_STAGE_MAY_2026 &&
    sdkEntrypoints.some(entrypointIsLocalVerifier)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} research targets cannot advertise local-only verifier SDK entrypoints`,
    );
  }
  if (
    descriptor.implementationStage === RESEARCH_STAGE_MAY_2026 &&
    sdkEntrypoints.length > 0
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} research targets cannot advertise executable SDK entrypoints; keep them in plannedSdkEntrypoints until the production stage advances`,
    );
  }
  if (
    descriptor.implementationStage === "chain-executable" &&
    sdkEntrypoints.some(entrypointIsDevFixture)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} chain-executable targets cannot advertise fixture/mock SDK entrypoints`,
    );
  }
  if (
    descriptor.implementationStage === "chain-executable" &&
    sdkEntrypoints.some(entrypointIsLocalVerifier)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} chain-executable targets cannot advertise local-only verifier SDK entrypoints`,
    );
  }
  if (
    descriptor.implementationStage === "production-hardened" &&
    sdkEntrypoints.some(entrypointIsDevFixture)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} production-hardened targets cannot advertise fixture/mock SDK entrypoints`,
    );
  }
  if (
    descriptor.implementationStage === "production-hardened" &&
    sdkEntrypoints.some(entrypointIsLocalVerifier)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} production-hardened targets cannot advertise local-only verifier SDK entrypoints`,
    );
  }
  sdkEntrypoints.forEach((entrypoint, entrypointIndex) => {
    if (
      entrypointIsDevFixture(entrypoint) &&
      !entrypointIsExplicitDevFixture(entrypoint)
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index}.sdkEntrypoints[${entrypointIndex}] fixture/mock SDK entrypoints must use explicit DevFixture names`,
      );
    }
  });
  if (
    sdkEntrypoints.some(entrypointIsLocalVerifier) &&
    !sdkEntrypoints.some(entrypointIsExplicitDevFixture)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} executable local-only verifier SDK entrypoints must be paired with an explicit DevFixture entrypoint`,
    );
  }
  if (
    sdkEntrypoints.some(entrypointIsExplicitDevFixture) &&
    !sdkEntrypoints.some(entrypointIsLocalVerifier)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} executable DevFixture SDK entrypoints must be paired with a local verifier entrypoint`,
    );
  }
  if (
    sdkEntrypoints.some(entrypointIsExplicitDevFixture) &&
    !hasDevFixtureNonProductionWarning(descriptor.securityNotes ?? [])
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} executable DevFixture SDK entrypoints must include a security note that marks dev fixtures as non-production and unavailable for production use`,
    );
  }
  if (
    sdkEntrypoints.some(entrypointIsExplicitDevFixture) &&
    plannedSdkEntrypoints.length === 0
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} executable DevFixture SDK entrypoints must retain planned production SDK entrypoints until production gates pass`,
    );
  }
  if (
    sdkEntrypoints.some(entrypointIsExplicitDevFixture) &&
    !plannedSdkEntrypoints.some(entrypointIsProductionProofBuilder)
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index} executable DevFixture SDK entrypoints must retain a planned production proof builder until production gates pass`,
    );
  }
  if (descriptor.implementationStage === CATALOG_STAGE_MAY_2026 && sdkEntrypoints.length > 0) {
    throw new Error(
      `privacy algorithm descriptor ${index} catalog-only targets cannot advertise SDK entrypoints`,
    );
  }
  if (descriptor.implementationStage === "production-hardened" && plannedSdkEntrypoints.length > 0) {
    throw new Error(
      `privacy algorithm descriptor ${index} production-hardened targets cannot retain planned SDK entrypoints`,
    );
  }
  const plannedLedgerMutations = plannedSdkEntrypoints.filter(entrypointIsPlannedLedgerMutation);
  if (plannedLedgerMutations.length > 0) {
    const protectionValues = [
      ...(descriptor.requiredState ?? []),
      ...(descriptor.failureModes ?? []),
      ...(descriptor.chainRequirements ?? []),
    ].map((value) => value.toLowerCase());
    const hasProtectionMetadata = LEDGER_MUTATION_PROTECTION_METADATA_TOKENS.some((token) =>
      protectionValues.some((value) => value.includes(token)),
    );
    if (!hasProtectionMetadata) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints ledger-mutating entries require replay, nullifier, revocation, or link-tag protection metadata; missing protection metadata for ${plannedLedgerMutations.join(", ")}`,
      );
    }
    const typedAdmissionText = TYPED_CHAIN_ADMISSION_METADATA_FIELDS.flatMap(
      (field) => descriptor[field] ?? [],
    ).join(" ").toLowerCase();
    const hasTypedAdmissionMetadata =
      TYPED_CHAIN_ADMISSION_TYPE_TOKENS.some((token) => typedAdmissionText.includes(token)) &&
      TYPED_CHAIN_ADMISSION_MUTATION_TOKENS.some((token) => typedAdmissionText.includes(token));
    if (!hasTypedAdmissionMetadata) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints ledger-mutating entries require explicit typed chain admission metadata; missing typed admission metadata for ${plannedLedgerMutations.join(", ")}`,
      );
    }
    const requiredStateText = (descriptor.requiredState ?? []).join(" ").toLowerCase();
    const hasStatefulLedgerState = STATEFUL_LEDGER_STATE_TOKENS.some((token) =>
      requiredStateText.includes(token),
    );
    if (hasStatefulLedgerState) {
      const persistenceText = STATEFUL_LEDGER_PERSISTENCE_METADATA_FIELDS.flatMap(
        (field) => descriptor[field] ?? [],
      ).join(" ").toLowerCase();
      const missingPersistenceGroups = STATEFUL_LEDGER_PERSISTENCE_TOKEN_GROUPS.filter(
        (tokens) => !tokens.some((token) => persistenceText.includes(token)),
      );
      if (missingPersistenceGroups.length > 0) {
        throw new Error(
          `privacy algorithm descriptor ${index}.plannedSdkEntrypoints ledger-mutating entries require restart/persistence metadata for root, nullifier, revocation, or replay state; missing persistence metadata for ${plannedLedgerMutations.join(", ")}`,
        );
      }
    }
  }

  if (descriptor.sourceReferences !== undefined) {
    if (!Array.isArray(descriptor.sourceReferences)) {
      throw new Error(`privacy algorithm descriptor ${index}.sourceReferences must be an array`);
    }
    const seenSourceReferenceLabels = new Set();
    const seenSourceReferenceUrls = new Set();
    descriptor.sourceReferences.forEach((reference, referenceIndex) => {
      if (!isPlainObject(reference)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] must be an object`,
        );
      }
      for (const field of Object.keys(reference)) {
        if (!SOURCE_REFERENCE_FIELDS.has(field)) {
          throw new Error(
            `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] field ${field} is not supported`,
          );
        }
      }
      assertNonEmptyString(
        reference.label,
        `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].label`,
      );
      assertNonEmptyString(
        reference.url,
        `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].url`,
      );
      if (!isSafeSourceReferenceLabel(reference.label)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].label must be clean and bounded`,
        );
      }
      if (sourceReferenceLabelClaimsAudit(reference.label)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].label must describe protocol source material, not audit/signoff evidence`,
        );
      }
      if (catalogLabelClaimsProductionReadiness(reference.label)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].label must not claim production/mainnet/audit readiness before production gates pass`,
        );
      }
      if (!isSafeHttpsSourceUrl(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].url must use https`,
        );
      }
      if (
        isPlaceholderSourceReferenceUrl(reference.url) ||
        isPrivateOrLocalSourceReferenceUrl(reference.url)
      ) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].url must not be a placeholder, local, or private-network URL`,
        );
      }
      if (!isCanonicalSourceReferenceUrl(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].url must be canonical`,
        );
      }
      if (sourceReferenceUrlClaimsAuditOrReadiness(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}].url must describe protocol source material, not audit/signoff or readiness evidence`,
        );
      }
      if (seenSourceReferenceLabels.has(reference.label)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] duplicates label ${reference.label}`,
        );
      }
      if (seenSourceReferenceUrls.has(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] duplicates url ${reference.url}`,
        );
      }
      seenSourceReferenceLabels.add(reference.label);
      seenSourceReferenceUrls.add(reference.url);
    });
  }
  if (descriptor.coveredCriteria.includes("post_quantum")) {
    const sourceReferenceUrls = new Set(
      (descriptor.sourceReferences ?? []).map((reference) => reference.url),
    );
    const missingSourceUrls = POST_QUANTUM_REQUIRED_SOURCE_URLS.filter(
      (url) => !sourceReferenceUrls.has(url),
    );
    if (missingSourceUrls.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.sourceReferences must include NIST FIPS 203, FIPS 204, and FIPS 205 URLs for post_quantum coverage; missing ${missingSourceUrls.join(", ")}`,
      );
    }
    const plannedEntrypointNames = plannedSdkEntrypoints.map((entrypoint) => {
      const segments = entrypoint.split(".");
      return segments[segments.length - 1];
    });
    const missingPlannedEntrypointFragments =
      POST_QUANTUM_REQUIRED_PLANNED_ENTRYPOINT_FRAGMENTS.filter(
        (fragment) => !plannedEntrypointNames.some((name) => name.includes(fragment)),
      );
    if (missingPlannedEntrypointFragments.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints must include planned ML-DSA authorization and ML-KEM note-encryption SDK entrypoints for post_quantum coverage; missing ${missingPlannedEntrypointFragments.join(", ")}`,
      );
    }
    for (const [fieldName, values, requiredTokens, label] of [
      [
        "securityNotes",
        descriptor.securityNotes ?? [],
        POST_QUANTUM_REQUIRED_SECURITY_NOTE_TOKENS,
        "post-quantum primitive risk notes",
      ],
      [
        "failureModes",
        descriptor.failureModes ?? [],
        POST_QUANTUM_REQUIRED_FAILURE_MODE_TOKENS,
        "post-quantum primitive failure modes",
      ],
      [
        "requiredState",
        descriptor.requiredState ?? [],
        POST_QUANTUM_REQUIRED_STATE_TOKENS,
        "post-quantum note-encryption state",
      ],
    ]) {
      const missingTokens = requiredTokens.filter(
        (token) => !values.some((value) => value.includes(token)),
      );
      if (missingTokens.length > 0) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${fieldName} must include ${label} for post_quantum coverage; missing ${missingTokens.join(", ")}`,
        );
      }
    }
  }
  const requiredResearchSourceUrls = RESEARCH_TARGET_REQUIRED_SOURCE_URLS_BY_ID[descriptor.id];
  if (
    descriptor.implementationStage === RESEARCH_STAGE_MAY_2026 &&
    requiredResearchSourceUrls !== undefined
  ) {
    const sourceReferenceUrls = new Set(
      (descriptor.sourceReferences ?? []).map((reference) => reference.url),
    );
    const missingResearchSourceUrls = requiredResearchSourceUrls.filter(
      (url) => !sourceReferenceUrls.has(url),
    );
    if (missingResearchSourceUrls.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.sourceReferences must include exact research target source URLs; missing ${missingResearchSourceUrls.join(", ")}`,
      );
    }
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
    (descriptor.sourceReferences ?? []).length === 0
  ) {
    throw new Error(
      `privacy algorithm descriptor ${index}.sourceReferences is required for source-referenced implementation stages`,
    );
  }
  if (SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage)) {
    descriptor.sourceReferences.forEach((reference, referenceIndex) => {
      if (isPlaceholderSourceReferenceUrl(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] must not use placeholder or test URLs for source-referenced implementation stages`,
        );
      }
      if (isPrivateOrLocalSourceReferenceUrl(reference.url)) {
        throw new Error(
          `privacy algorithm descriptor ${index}.sourceReferences[${referenceIndex}] must not use private, local, or non-global URLs for source-referenced implementation stages`,
        );
      }
    });
    for (const field of SOURCE_REFERENCED_REQUIRED_LIST_FIELDS) {
      if ((descriptor[field] ?? []).length === 0) {
        throw new Error(
          `privacy algorithm descriptor ${index}.${field} must be non-empty for source-referenced implementation stages`,
        );
      }
    }
    for (const field of SOURCE_REFERENCED_REQUIRED_VERIFIER_FIELDS) {
      if (descriptor[field] == null || descriptor[field] === "") {
        throw new Error(
          `privacy algorithm descriptor ${index}.${field} must be non-empty for source-referenced implementation stages`,
        );
      }
    }
    if (SOURCE_REFERENCED_FORBIDDEN_PROOF_FAMILIES.has(descriptor.proofFamily)) {
      throw new Error(
        `privacy algorithm descriptor ${index}.proofFamily must be a concrete proof family for source-referenced implementation stages`,
      );
    }
    const backendFamily = BACKEND_FAMILY_BY_ALGORITHM_ID[descriptor.id];
    if (
      backendFamily == null ||
      SOURCE_REFERENCED_FORBIDDEN_BACKEND_FAMILIES.has(backendFamily)
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index} must have a registered non-none backend family for source-referenced implementation stages`,
      );
    }
    if (
      PRE_PRODUCTION_SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
      (descriptor.plannedSdkEntrypoints ?? []).length === 0
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index}.plannedSdkEntrypoints must be non-empty for pre-production source-referenced implementation stages`,
      );
    }
    if (
      !SOURCE_REFERENCED_SDK_ENTRYPOINT_FIELDS.some(
        (field) => (descriptor[field] ?? []).length > 0,
      )
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index} source-referenced implementation stages must expose at least one executable or planned SDK entrypoint`,
      );
    }
  }
  if (
    WALLET_STATE_REQUIRED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
    !WALLET_STATE_REQUIRED_EXCLUDED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = (descriptor.requiredState ?? []).join(" ").toLowerCase();
    if (!WALLET_STATE_METADATA_TOKENS.some((token) => requiredStateText.includes(token))) {
      throw new Error(
        `privacy algorithm descriptor ${index}.requiredState must include wallet or witness state metadata for source-referenced privacy flows`,
      );
    }
    const securityNotesText = (descriptor.securityNotes ?? []).join(" ").toLowerCase();
    const missingWitnessPrivacyGroups = WALLET_WITNESS_PRIVACY_NOTE_TOKEN_GROUPS.filter(
      (tokens) => !tokens.some((token) => securityNotesText.includes(token)),
    );
    if (missingWitnessPrivacyGroups.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.securityNotes must include wallet/witness privacy notes for source-referenced privacy flows`,
      );
    }
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
    CREDENTIAL_STATE_REQUIRED_CATEGORIES.has(descriptor.category)
  ) {
    const requiredStateText = (descriptor.requiredState ?? []).join(" ").toLowerCase();
    if (!CREDENTIAL_STATE_METADATA_TOKENS.some((token) => requiredStateText.includes(token))) {
      throw new Error(
        `privacy algorithm descriptor ${index}.requiredState must include credential, identity, or admission commitment/accumulator state metadata`,
      );
    }
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
    descriptor.verifierKeyId !== undefined &&
    descriptor.verifierKeyId !== null
  ) {
    const failureModesText = (descriptor.failureModes ?? []).join(" ").toLowerCase();
    const missingNegativeFailureModeGroups = VERIFIER_NEGATIVE_FAILURE_MODE_TOKEN_GROUPS.filter(
      (tokens) => !tokens.some((token) => failureModesText.includes(token)),
    );
    if (missingNegativeFailureModeGroups.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.failureModes must include malformed-proof, wrong-verifier-key, and wrong-public-input rejection for source-referenced verifier entries`,
      );
    }
    const verifierKeyRecordText = VERIFIER_KEY_RECORD_METADATA_FIELDS.flatMap(
      (field) => descriptor[field] ?? [],
    ).join(" ").toLowerCase();
    if (
      !VERIFIER_KEY_RECORD_METADATA_TOKENS.some((token) =>
        verifierKeyRecordText.includes(token),
      )
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index} must include verifier-key record metadata for source-referenced verifier entries`,
      );
    }
  }
  if (
    SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage) &&
    descriptor.verifierKeyId !== undefined &&
    descriptor.verifierKeyId !== null
  ) {
    const chainDomainBindingText = CHAIN_DOMAIN_BINDING_METADATA_FIELDS.flatMap((field) => {
      const value = descriptor[field];
      return Array.isArray(value) ? value : [value];
    }).join(" ").toLowerCase();
    if (
      !CHAIN_DOMAIN_BINDING_METADATA_TOKENS.some((token) =>
        chainDomainBindingText.includes(token),
      )
    ) {
      throw new Error(
        `privacy algorithm descriptor ${index} must include chain/domain binding metadata for source-referenced verifier entries`,
      );
    }
  }
  if (SOURCE_REFERENCED_IMPLEMENTATION_STAGES.has(descriptor.implementationStage)) {
    const securityNotesText = (descriptor.securityNotes ?? []).join(" ").toLowerCase();
    const missingHardeningGroups = SOURCE_REFERENCED_HARDENING_NOTE_TOKEN_GROUPS.filter(
      (tokens) => !tokens.some((token) => securityNotesText.includes(token)),
    );
    if (missingHardeningGroups.length > 0) {
      throw new Error(
        `privacy algorithm descriptor ${index}.securityNotes must include audit/review, fuzzing, and performance hardening gates for source-referenced entries`,
      );
    }
  }
  if (descriptor.implementationStage === RESEARCH_STAGE_MAY_2026) {
    const securityNotesText = (descriptor.securityNotes ?? []).join(" ").toLowerCase();
    const hasReadinessMarker = RESEARCH_TARGET_PRODUCTION_READINESS_TOKENS.every((token) =>
      securityNotesText.includes(token),
    );
    const hasEvidenceMarker = RESEARCH_TARGET_READINESS_EVIDENCE_TOKENS.some((token) =>
      securityNotesText.includes(token),
    );
    if (!hasReadinessMarker || !hasEvidenceMarker) {
      throw new Error(
        `privacy algorithm descriptor ${index}.securityNotes must include production readiness audit or review gating for research targets`,
      );
    }
  }
  return cloneDescriptor(descriptor);
}

function validatePrivacyAlgorithmCatalog(descriptors) {
  if (!Array.isArray(descriptors)) {
    throw new Error("privacy algorithm catalog must be an array");
  }
  const ids = new Set();
  const verifierKeyIds = new Set();
  const entries = descriptors.map((descriptor, index) => {
    validatePrivacyAlgorithmDescriptor(descriptor, index);
    if (ids.has(descriptor.id)) {
      throw new Error(`privacy algorithm catalog contains duplicate id ${descriptor.id}`);
    }
    ids.add(descriptor.id);
    if (descriptor.verifierKeyId != null) {
      if (verifierKeyIds.has(descriptor.verifierKeyId)) {
        throw new Error(
          `privacy algorithm catalog contains duplicate verifierKeyId ${descriptor.verifierKeyId}`,
        );
      }
      verifierKeyIds.add(descriptor.verifierKeyId);
    }
    return descriptor;
  });
  const backendIds = Object.keys(BACKEND_FAMILY_BY_ALGORITHM_ID);
  const catalogIds = entries.map((descriptor) => descriptor.id);
  if (
    backendIds.length !== catalogIds.length ||
    backendIds.some((id, index) => id !== catalogIds[index])
  ) {
    throw new Error("privacy algorithm backend-family registration must exactly match catalog ids");
  }
  validateRequiredPrivacyPlanRows(entries);
  validateResearchTargetSdkEntrypoints(entries);
  return entries;
}

const PRIVACY_ALGORITHMS = Object.freeze(validatePrivacyAlgorithmCatalog([
  Object.freeze({
    id: "transparent-transfer",
    name: "Transparent asset transfer",
    shortName: "Transparent",
    summary: "Public Iroha asset transfer used as the size and latency baseline.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze([]),
    proofFamily: "none",
    publicInputsSchema: null,
    verifierKeyId: null,
    pqLayers: PQ_LAYER_NONE,
    sdkEntrypoints: Object.freeze([
      "buildTransferAssetInstruction",
      "buildTransaction",
      "submitSignedTransaction",
    ]),
    chainRequirements: Object.freeze(["Transfer::Asset"]),
  }),
  Object.freeze({
    id: "shield",
    name: "Shield into confidential note",
    shortName: "Shield",
    summary:
      "Debits public balance and appends an encrypted receiver note commitment.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_receiver"]),
    proofFamily: "commitment-only",
    publicInputsSchema: "asset,from,amount,note_commitment",
    verifierKeyId: "zk::Shield",
    pqLayers: PQ_LAYER_NONE,
    sdkEntrypoints: Object.freeze([
      "buildShieldInstruction",
      "buildTransaction",
      "submitSignedTransaction",
    ]),
    chainRequirements: Object.freeze(["zk::RegisterZkAsset", "zk::Shield"]),
  }),
  Object.freeze({
    id: "confidential-transfer-v2",
    name: "Confidential transfer v2",
    shortName: "Confidential v2",
    summary:
      "Halo2/Pasta note-to-note transfer that hides amount, sender note, and receiver note while publishing the asset id.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "halo2-ipa-pasta",
    publicInputsSchema:
      "input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_commitment_0,output_commitment_1,root,asset_tag,chain_tag",
    verifierKeyId: "confidential_transfer_v2",
    pqLayers: PQ_LAYER_NONE,
    sdkEntrypoints: Object.freeze([
      "buildConfidentialTransferProofV2",
      "buildZkTransferInstruction",
    ]),
    chainRequirements: Object.freeze([
      "zk::ZkTransfer",
      "active confidential transfer verifier key",
      "wallet note witness store",
    ]),
  }),
  Object.freeze({
    id: "unshield",
    name: "Unshield to public balance",
    shortName: "Unshield",
    summary:
      "Spends a private note into a public receiver balance; the private source note remains hidden.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_sender"]),
    proofFamily: "halo2-ipa-pasta",
    publicInputsSchema:
      "input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,change_commitment_0,root,public_amount,asset_tag,chain_tag",
    verifierKeyId: "confidential_unshield_v3",
    pqLayers: PQ_LAYER_NONE,
    sdkEntrypoints: Object.freeze([
      "buildConfidentialUnshieldProofV3",
      "buildUnshieldInstruction",
    ]),
    chainRequirements: Object.freeze([
      "zk::Unshield",
      "active confidential unshield verifier key",
      "wallet note witness store",
    ]),
  }),
  Object.freeze({
    id: "asset-hidden-confidential-transfer-v1",
    name: "Asset-hidden MASP transfer v1",
    shortName: "MASP v1",
    summary:
      "Target multi-asset shielded-pool transfer that hides amount, sender note, receiver note, and exact asset inside a pool.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze([
      "hide_amount",
      "hide_sender",
      "hide_receiver",
      "hide_asset_type",
    ]),
    proofFamily: "halo2-ipa-pasta",
    publicInputsSchema:
      "pool_id,asset_set_root,input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_commitment_0,output_commitment_1,root,chain_tag",
    verifierKeyId: "asset_hidden_transfer_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "validator-scaffold-as-of-2026-05",
    sdkEntrypoints: Object.freeze([
      "buildRegisterAssetHiddenZkPoolInstruction",
      "buildAssetHiddenZkTransferInstruction",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildConfidentialAssetHiddenTransferProofV1",
    ]),
    chainRequirements: Object.freeze([
      "zk::RegisterAssetHiddenZkPool",
      "zk::AssetHiddenZkTransfer",
      "asset-hidden pool verifier registry state",
      "pool note witness store",
    ]),
  }),
  Object.freeze({
    id: "zk-ace-pq-authorization-v0",
    name: "ZK-ACE post-quantum authorization v0",
    shortName: "ZK-ACE PQ auth",
    summary:
      "STARK/FRI-backed source-account authorization for transparent asset transfers.",
    category: "authorization",
    maturity: "arxiv_preprint",
    coveredCriteria: Object.freeze([]),
    proofFamily: "stark/fri/sha256-goldilocks",
    publicInputsSchema:
      "identity_commitment,tx_digest,chain_id,domain_separator,action_class,replay_nullifier,policy_hash,from,to,asset,amount,verifier_key_id",
    verifierKeyId: "zk_ace_pq_authorization_v0",
    pqLayers: Object.freeze({
      proof: true,
      authorization: true,
      noteEncryption: false,
    }),
    implementationStage: "chain-executable",
    recommendedFor: Object.freeze([
      "post-quantum transaction authorization migration",
      "identity-private source-account authorization",
      "authorization envelopes for transparent asset transfers",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "ZK-ACE: Practical Post-Quantum Authorization for Blockchain",
        url: "https://arxiv.org/abs/2603.07974",
      }),
    ]),
    securityNotes: Object.freeze([
      "Authorization is only one PQ layer; proof backend and note encryption must also be PQ before a payment flow is end-to-end post-quantum.",
      "Replay nullifiers must be chain-domain separated and irreversible after acceptance.",
      "A dev verifier must never be accepted under a production verifier key id.",
      "Native AIR openings are blinded so sampled rows do not recover identity or replay witness limbs.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "active identity commitment registry",
      "replay nullifier set",
      "authorization verifier registry",
      "wallet identity witness and replay-secret store",
    ]),
    failureModes: Object.freeze([
      "transaction digest substitution",
      "chain-id or domain-separator mismatch",
      "replayed nullifier",
      "revoked identity commitment",
      "policy hash mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register a ZK-ACE identity commitment, source-account allowlist, and verifier key.",
      "Initialize replay-state tracking for the authorizing wallet.",
      "Bind authorization policy hash to the allowed transaction action classes.",
    ]),
    executionSteps: Object.freeze([
      "Hash the transaction payload and chain/domain context.",
      "Derive a fresh replay nullifier.",
      "Generate a ZK-ACE authorization proof and submit a protected transparent transfer.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildRegisterZkAceIdentityCommitmentInstruction",
      "buildRotateZkAceIdentityCommitmentInstruction",
      "buildRevokeZkAceIdentityCommitmentInstruction",
      "buildZkAceAuthorizedTransferInstruction",
      "buildZkAceAuthorizationProofV1",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildShieldedZkAceAuthorizationProofV1",
      "buildShieldedZkAceAuthorizedTransferInstruction",
    ]),
    chainRequirements: Object.freeze([
      "zk::RegisterZkAceIdentityCommitment",
      "zk::RotateZkAceIdentityCommitment",
      "zk::RevokeZkAceIdentityCommitment",
      "zk::SubmitZkAceAuthorizedTransfer",
      "active stark/fri/sha256-goldilocks ZK-ACE verifier key",
      "ZK-ACE identity source-account allowlist",
    ]),
  }),
  Object.freeze({
    id: "anonymous-pgc-k-out-of-n-v1",
    name: "Anonymous PGC k-out-of-n payments v1",
    shortName: "Anonymous PGC",
    summary:
      "Account-based anonymous confidential payment target with hidden sender, hidden amount, receiver privacy, and k-out-of-n receiver-set proofs.",
    category: "payment",
    maturity: "accepted_conference",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "anonymous-pgc-k-out-of-n",
    publicInputsSchema:
      "anonymity_set_root,tx_digest,balance_commitments,receiver_set_commitment,receiver_ciphertext_commitments,receiver_threshold,receiver_count,link_tag,range_commitments,chain_id,domain_separator",
    verifierKeyId: "anonymous_pgc_k_out_of_n_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "account-based private payments",
      "multi-receiver confidential transfers",
      "payment privacy without a note-based shielded pool UX",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Anonymous PGC with k-out-of-n Proofs",
        url: "https://eprint.iacr.org/2025/884",
      }),
    ]),
    securityNotes: Object.freeze([
      "Requires fresh anonymity-set roots and replay/link-tag state.",
      "Amount privacy depends on the range-proof component and commitment binding.",
      "Receiver ciphertext commitments must bind to the same transaction digest as the proof.",
      "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "anonymous account commitment set",
      "recent anonymity-set roots",
      "spent link-tag set",
      "range-proof verifier parameters",
      "wallet account blinding and receiver recovery metadata",
    ]),
    failureModes: Object.freeze([
      "stale or unknown anonymity-set root",
      "duplicate link tag",
      "receiver-set substitution",
      "range commitment mismatch",
      "authorization envelope mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register anonymous account commitments and anonymity-set accumulator state.",
      "Register the k-out-of-n payment verifier key and range-proof parameters.",
      "Persist wallet blinding, balance-opening, and receiver recovery witnesses.",
    ]),
    executionSteps: Object.freeze([
      "Select an anonymity-set root and receiver set.",
      "Create balance commitments, receiver ciphertext commitments, and link tag.",
      "Generate the Anonymous PGC proof and submit the transfer instruction.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildAnonymousPgcReceiverSet",
      "buildAnonymousPgcDevProofFixture",
      "verifyAnonymousPgcDevProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildAnonymousPgcAccountCommitmentInstruction",
      "buildAnonymousPgcKOutOfNProofV1",
      "buildAnonymousPgcTransferInstruction",
    ]),
    chainRequirements: Object.freeze([
      "anonymous account commitment accumulator",
      "spent link-tag set",
      "Anonymous PGC verifier",
      "range-proof component verifier",
      "typed zk::RegisterAnonymousPgcAccountCommitment instruction",
      "typed zk::SubmitAnonymousPgcTransfer instruction",
    ]),
  }),
  Object.freeze({
    id: "verange-transparent-range-v1",
    name: "VeRange transparent range proofs v1",
    shortName: "VeRange",
    summary:
      "Verification-efficient transparent range-proof component for confidential amounts, solvency proofs, and numeric credential predicates.",
    category: "proof_backend",
    maturity: "accepted_conference",
    coveredCriteria: Object.freeze(["hide_amount"]),
    proofFamily: "verange-transparent-range",
    publicInputsSchema:
      "commitments,range_parameters,aggregation_count,domain_separator,payload_digest",
    verifierKeyId: "verange_transparent_range_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "component",
    recommendedFor: Object.freeze([
      "confidential amount range proofs",
      "reserve or solvency proofs",
      "numeric credential predicates",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "VeRange: Verification-efficient Zero-knowledge Range Arguments",
        url: "https://eprint.iacr.org/2025/528",
      }),
    ]),
    securityNotes: Object.freeze([
      "This is a component, not a complete payment protocol.",
      "Range parameters must be bound to the transaction payload and verifier key.",
      "Aggregated proof limits must be enforced by validators.",
      "Local verification is limited to deterministic dev fixtures; the production VeRange prover remains unavailable.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "range-proof verifier parameters",
      "VeRange verifier key registry",
      "range commitment domain separators",
      "maximum aggregation policy",
    ]),
    failureModes: Object.freeze([
      "wrong bit length",
      "commitment substitution",
      "verifier-parameter mismatch",
      "oversized aggregation",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register VeRange verifier parameters and allowed bit lengths.",
      "Define the commitment scheme and domain separators used by dependent algorithms.",
    ]),
    executionSteps: Object.freeze([
      "Build amount commitments.",
      "Generate a range proof bound to the transaction payload.",
      "Attach the range-proof envelope to the dependent confidential algorithm.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildRangeCommitment",
      "buildVeRangeDevProofFixture",
      "buildVeRangeProofEnvelope",
      "verifyVeRangeProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildVeRangeProofV1",
    ]),
    chainRequirements: Object.freeze([
      "VeRange verifier registry entry",
      "range commitment binding rules",
      "dependent payment or credential verifier",
    ]),
  }),
  Object.freeze({
    id: "zkat-policy-private-auth-v1",
    name: "zkAt policy-private authorization v1",
    shortName: "zkAt policy auth",
    summary:
      "Policy-private blockchain authenticator that hides threshold rules, signer sets, and account authorization logic.",
    category: "authorization",
    maturity: "accepted_conference",
    coveredCriteria: Object.freeze([]),
    proofFamily: "zkat-policy-private-authenticator",
    publicInputsSchema:
      "policy_commitment,tx_digest,account_id,action_class,domain_separator,policy_epoch",
    verifierKeyId: "zkat_policy_private_auth_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "institutional wallet policy privacy",
      "hidden threshold authorization",
      "authorization-policy migration without revealing signer topology",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "zkAt: Zero-Knowledge Authenticator for Blockchain",
        url: "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2",
      }),
    ]),
    securityNotes: Object.freeze([
      "Hides authorization policy, not payment fields.",
      "Policy commitments require explicit epoch, replay, and rotation semantics.",
      "Combining with ZK-ACE requires both proofs to bind the same transaction digest.",
      "The SDK dev fixture verifies deterministic binding only; chain policy state and production zkAt proofs remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "policy commitment registry",
      "policy epoch state",
      "authorization replay guard",
      "authorization verifier registry",
      "wallet policy witness store",
    ]),
    failureModes: Object.freeze([
      "policy-root substitution",
      "stale policy epoch",
      "unauthorized signer witness",
      "transaction digest mismatch",
      "authorization replay",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register a hidden policy commitment and verifier key.",
      "Bind the policy to account action classes and epoch rules.",
    ]),
    executionSteps: Object.freeze([
      "Generate a policy-private authenticator proof.",
      "Attach the authenticator envelope to the transaction authorization path.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildZkAtPolicyCommitment",
      "buildZkAtAuthenticatorEnvelope",
      "buildZkAtDevProofFixture",
      "verifyZkAtAuthenticatorLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildZkAtPolicyCommitmentInstruction",
      "buildZkAtPolicyProofV1",
      "buildZkAtAuthorizedTransaction",
    ]),
    chainRequirements: Object.freeze([
      "zkAt policy commitment registry",
      "zkAt verifier",
      "account policy epoch state",
      "account policy replay protection",
      "typed zk::RegisterZkAtPolicyCommitment instruction",
      "typed zk::SubmitZkAtAuthorizedTransaction admission",
    ]),
  }),
  Object.freeze({
    id: "zk-ams-recursive-admission-v0",
    name: "ZK-AMS recursive anonymous admission v0",
    shortName: "ZK-AMS admission",
    summary:
      "Research target for recursively aggregated anonymous admission from real-world personhood or eligibility credentials into anonymous on-chain accounts.",
    category: "admission",
    maturity: "arxiv_preprint",
    coveredCriteria: Object.freeze([]),
    proofFamily: "recursive-anonymous-admission",
    publicInputsSchema:
      "issuer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_admission_digest,domain_separator",
    verifierKeyId: "zk_ams_recursive_admission_v0",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "anonymous onboarding",
      "Sybil-resistant wallet issuance",
      "credential-gated CBDC pilots",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "ZK-AMS recursive anonymous admission",
        url: "https://arxiv.org/abs/2602.16130",
      }),
    ]),
    securityNotes: Object.freeze([
      "Admission privacy is separate from later payment privacy.",
      "Duplicate admission prevention depends on issuer-scoped nullifiers.",
      "Recursive batching must bind every admitted account commitment.",
      "The SDK dev fixture verifies deterministic binding only; chain admission state and production recursive proofs remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "issuer root registry",
      "admission nullifier set",
      "anonymous account commitment registry",
      "recursive verifier parameters",
      "recursive admission verifier key registry",
      "wallet admission witness store",
    ]),
    failureModes: Object.freeze([
      "duplicate credential admission",
      "wrong issuer root",
      "batch omission or account commitment substitution",
      "recursive proof parameter mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register credential issuer roots and recursive verifier parameters.",
      "Define anonymous account commitment format and admission-nullifier derivation.",
    ]),
    executionSteps: Object.freeze([
      "Collect admitted account commitments into a batch.",
      "Generate or import a recursive admission proof.",
      "Submit the batch proof and admission nullifiers.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildZkAmsAdmissionBatch",
      "buildZkAmsAdmissionProofEnvelope",
      "buildZkAmsAdmissionDevProofFixture",
      "verifyZkAmsAdmissionProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildZkAmsAdmissionBatchProofV0",
      "buildSubmitZkAmsAdmissionBatchInstruction",
    ]),
    chainRequirements: Object.freeze([
      "issuer root registry",
      "admission nullifier set",
      "recursive admission verifier",
      "typed ZK-AMS admission batch instruction",
    ]),
  }),
  Object.freeze({
    id: "vega-existing-credential-zk-v0",
    name: "Vega existing-credential ZK proofs v0",
    shortName: "Vega credentials",
    summary:
      "Low-latency zero-knowledge proof target for proving predicates over existing credentials without revealing the full credential.",
    category: "credential",
    maturity: "technical_report",
    coveredCriteria: Object.freeze([]),
    proofFamily: "existing-credential-zk",
    publicInputsSchema:
      "issuer_commitment,credential_schema,predicate_commitment,subject_binding,expiration_epoch,domain_separator",
    verifierKeyId: "vega_existing_credential_zk_v0",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "legacy credential bridges",
      "private eligibility checks",
      "attribute predicates for wallet enrollment",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Vega: Low-Latency Zero-Knowledge Proofs over Existing Credentials",
        url: "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/",
      }),
    ]),
    securityNotes: Object.freeze([
      "Credential schema parsing must be deterministic and versioned.",
      "Proofs must bind to wallet or identity commitments to prevent credential replay.",
      "Issuer trust and revocation semantics remain external policy inputs.",
      "The SDK dev fixture verifies deterministic binding only; chain credential policy state and production Vega proofs remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "credential issuer registry",
      "supported credential schema registry",
      "predicate registry",
      "revocation or expiration policy",
      "wallet credential predicate witness store",
      "credential predicate commitment registry",
      "credential predicate verifier key registry",
    ]),
    failureModes: Object.freeze([
      "expired credential",
      "wrong issuer",
      "predicate mismatch",
      "wallet-binding replay",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register supported credential schemas, issuers, and predicates.",
      "Bind credential proof subjects to wallet or ZK-ACE identity commitments.",
    ]),
    executionSteps: Object.freeze([
      "Parse the credential under a registered schema.",
      "Generate a predicate proof and bind it to the wallet context.",
      "Submit the proof envelope to the admission or authorization flow.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildVegaCredentialPredicateCommitment",
      "buildVegaCredentialProofEnvelope",
      "buildVegaCredentialDevProofFixture",
      "verifyVegaCredentialProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildVegaCredentialPredicateProofV0",
      "buildSubmitVegaCredentialProofInstruction",
    ]),
    chainRequirements: Object.freeze([
      "credential schema registry",
      "issuer registry",
      "credential predicate verifier",
      "typed Vega credential proof instruction",
    ]),
  }),
  Object.freeze({
    id: "silent-threshold-anoncred-v0",
    name: "Silent threshold anonymous credentials v0",
    shortName: "Silent threshold cred",
    summary:
      "Research target for threshold-issued anonymous credentials with silent setup, issuer hiding, constant-size showings, and dynamic verifier policies.",
    category: "credential",
    maturity: "technical_report",
    coveredCriteria: Object.freeze([]),
    proofFamily: "threshold-anonymous-credentials",
    publicInputsSchema:
      "issuer_set_commitment,threshold_policy_hash,credential_showing_commitment,showing_nullifier,verifier_policy_hash,domain_separator",
    verifierKeyId: "silent_threshold_anoncred_v0",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "multi-authority regulated credentials",
      "issuer-hiding eligibility proofs",
      "central-bank or supervisor issued wallet credentials",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Anonymous Credentials with Issuer-Hiding, Threshold Issuance, and Silent Setup",
        url: "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html",
      }),
    ]),
    securityNotes: Object.freeze([
      "Credential issuance and revocation governance are as important as proof verification.",
      "Issuer-set commitments need rotation and downgrade protections.",
      "This is a credential layer, not a private payment protocol.",
      "The SDK dev fixture verifies deterministic binding only; chain credential state and production silent-threshold proofs remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "threshold issuer registry",
      "credential parameter registry",
      "verifier policy registry",
      "credential showing nullifier policy",
      "wallet credential showing witness store",
      "credential showing commitment registry",
      "anonymous credential verifier key registry",
    ]),
    failureModes: Object.freeze([
      "insufficient issuer threshold",
      "issuer-set substitution",
      "credential showing replay",
      "verifier-policy mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register issuer sets, threshold policies, and credential parameters.",
      "Define showing-nullifier and verifier-policy binding rules.",
    ]),
    executionSteps: Object.freeze([
      "Generate a credential showing proof under the verifier policy.",
      "Submit the proof as an admission or authorization component.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildSilentThresholdCredentialCommitments",
      "buildSilentThresholdCredentialEnvelope",
      "buildSilentThresholdCredentialDevProofFixture",
      "verifySilentThresholdCredentialProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildSilentThresholdCredentialShowingProofV0",
      "buildSubmitSilentThresholdCredentialProofInstruction",
    ]),
    chainRequirements: Object.freeze([
      "threshold issuer registry",
      "anonymous credential verifier",
      "credential showing replay policy",
      "typed silent-threshold credential proof instruction",
    ]),
  }),
  Object.freeze({
    id: "zk-x509-onchain-identity-v0",
    name: "ZK-X.509 on-chain identity v0",
    shortName: "ZK-X.509 identity",
    summary:
      "ZK proof target for X.509 certificate validity, ownership, revocation status, and wallet-address binding.",
    category: "identity",
    maturity: "arxiv_preprint",
    coveredCriteria: Object.freeze([]),
    proofFamily: "zkvm-x509-identity",
    publicInputsSchema:
      "ca_root_commitment,certificate_policy_hash,revocation_root,subject_commitment,address_binding,domain_separator",
    verifierKeyId: "zk_x509_onchain_identity_v0",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "institutional wallet identity",
      "legal-entity account binding",
      "private PKI-based eligibility checks",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "ZK-X.509 on-chain identity",
        url: "https://arxiv.org/abs/2603.25190",
      }),
    ]),
    securityNotes: Object.freeze([
      "Legacy X.509 trust roots are usually not post-quantum.",
      "Revocation root freshness must be explicit in the public inputs.",
      "Address binding must prevent proof replay across wallets and chains.",
      "The SDK dev fixture verifies deterministic public-input binding only; chain trust-root, revocation, policy state, and production ZK-X.509 proofs remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "trusted CA root registry",
      "certificate policy registry",
      "revocation root registry",
      "identity proof verifier",
      "wallet certificate witness store",
      "certificate subject commitment registry",
      "ZK-X.509 verifier key registry",
    ]),
    failureModes: Object.freeze([
      "expired certificate",
      "revoked certificate",
      "unknown CA root",
      "wrong wallet address binding",
      "stale revocation root",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register trusted CA roots, certificate policies, and revocation-root feeds.",
      "Define wallet address binding and domain-separation rules.",
    ]),
    executionSteps: Object.freeze([
      "Generate a proof of certificate validity, ownership, and revocation status.",
      "Bind the proof to an institution wallet or ZK-ACE identity commitment.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildZkX509IdentityCommitments",
      "buildZkX509IdentityEnvelope",
      "buildZkX509IdentityDevProofFixture",
      "verifyZkX509IdentityProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildZkX509IdentityProofV0",
      "buildSubmitZkX509IdentityProofInstruction",
    ]),
    chainRequirements: Object.freeze([
      "trusted CA root registry",
      "revocation root registry",
      "ZK-X.509 verifier",
      "typed ZK-X.509 identity proof instruction",
    ]),
  }),
  Object.freeze({
    id: "jindo-lattice-pcs-zk-v0",
    name: "Jindo lattice polynomial commitment ZK v0",
    shortName: "Jindo lattice PCS",
    summary:
      "2026 lattice-based polynomial commitment candidate for post-quantum zero-knowledge proof systems.",
    category: "proof_backend",
    maturity: "technical_report",
    coveredCriteria: Object.freeze([]),
    proofFamily: "lattice-polynomial-commitment",
    publicInputsSchema:
      "commitment,opening_claim,query_set,parameter_hash,domain_separator",
    verifierKeyId: "jindo_lattice_pcs_zk_v0",
    pqLayers: Object.freeze({
      proof: true,
      authorization: false,
      noteEncryption: false,
    }),
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "post-quantum proof-system research",
      "future PQ verifier backend evaluation",
      "lattice PCS benchmarking",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Jindo lattice-based polynomial commitment",
        url: "https://eprint.iacr.org.cn/2026/044",
      }),
    ]),
    securityNotes: Object.freeze([
      "This is a proof backend candidate, not a transaction algorithm.",
      "PQ proof coverage alone does not imply PQ authorization or note encryption.",
      "Parameter selection and implementation security require independent review.",
      "The SDK dev fixture verifies deterministic public-input binding only; production Jindo lattice proving and verifier backends remain unavailable.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "lattice PCS parameter registry",
      "backend verifier implementation",
      "lattice PCS verifier key registry",
      "benchmark fixtures",
    ]),
    failureModes: Object.freeze([
      "parameter mismatch",
      "opening claim substitution",
      "unsupported query set",
      "backend misclassified as production-ready",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Track lattice PCS parameter sets and verifier API shape.",
      "Benchmark prover, verifier, and proof-size behavior before integration.",
    ]),
    executionSteps: Object.freeze([
      "Use as a candidate backend for future PQ circuits only after concrete circuit integration.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildJindoLatticePublicInputs",
      "buildJindoLatticeProofEnvelope",
      "buildJindoLatticeDevProofFixture",
      "verifyJindoLatticeProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildJindoLatticeProofV0",
      "verifyJindoPolynomialCommitmentV0",
    ]),
    chainRequirements: Object.freeze([
      "Jindo verifier backend",
      "lattice PCS parameter registry",
      "dependent circuit integration",
    ]),
  }),
  Object.freeze({
    id: "sis-hints-anoncred-pq-v0",
    name: "SIS-with-hints PQ anonymous credentials v0",
    shortName: "SIS hints anoncred",
    summary:
      "PKC 2026 research foundation for lattice/SIS-with-hints anonymous credentials and post-quantum credential proofs.",
    category: "credential",
    maturity: "accepted_conference",
    coveredCriteria: Object.freeze([]),
    proofFamily: "lattice-anonymous-credentials",
    publicInputsSchema:
      "issuer_commitment,credential_commitment,showing_policy_hash,parameter_hash,domain_separator",
    verifierKeyId: "sis_hints_anoncred_pq_v0",
    pqLayers: Object.freeze({
      proof: true,
      authorization: false,
      noteEncryption: false,
    }),
    implementationStage: "sdk-builder",
    recommendedFor: Object.freeze([
      "post-quantum anonymous credential research",
      "future PQ KYC or eligibility proofs",
      "assumption tracking for lattice credential designs",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Tight Reductions for SIS-with-Hints Assumptions with Applications",
        url: "https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-with-hints-assumptions-with-applications/",
      }),
    ]),
    securityNotes: Object.freeze([
      "This is a credential foundation, not an immediately deployable wallet protocol.",
      "PQ credential proof coverage does not make a payment flow end-to-end post-quantum.",
      "Parameter choices and reduction assumptions need explicit governance.",
      "The SDK dev fixture verifies deterministic public-input binding only; production SIS-with-hints credential proving and verifier backends remain unavailable.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "lattice credential parameter registry",
      "issuer parameter registry",
      "credential showing verifier",
      "wallet lattice credential witness store",
      "lattice credential commitment registry",
      "lattice credential verifier key registry",
    ]),
    failureModes: Object.freeze([
      "wrong parameter set",
      "issuer parameter substitution",
      "credential showing replay",
      "overclaiming production readiness from assumption research",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Track supported SIS-with-hints parameter sets and issuer parameters.",
      "Define how future PQ credential showings bind to wallet or authorization contexts.",
    ]),
    executionSteps: Object.freeze([
      "Use as a future PQ credential backend after a concrete credential protocol is selected.",
    ]),
    sdkEntrypoints: Object.freeze([
      "buildSisHintsCredentialCommitments",
      "buildSisHintsCredentialEnvelope",
      "buildSisHintsCredentialDevProofFixture",
      "verifySisHintsCredentialProofLocally",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildSisHintsAnonymousCredentialProofV0",
      "buildSubmitSisHintsCredentialProofInstruction",
    ]),
    chainRequirements: Object.freeze([
      "lattice anonymous credential verifier",
      "credential parameter registry",
      "issuer parameter registry",
      "typed SIS-with-hints credential proof instruction",
    ]),
  }),
  Object.freeze({
    id: "orchard-halo2-actions-v1",
    name: "Orchard-style Halo2 action bundle v1",
    shortName: "Orchard Halo2",
    summary:
      "Zcash Orchard-style action bundle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "halo2-pasta-action-bundle",
    publicInputsSchema:
      "anchor,nullifiers,cmx,value_commitments,binding_signature",
    verifierKeyId: "orchard_halo2_action_bundle_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "single-asset private transfers",
      "mature note/nullifier wallet design",
      "compact client proofs without Groth16 ceremonies",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "ZIP 224 Orchard Shielded Protocol",
        url: "https://zips.z.cash/zip-0224",
      }),
      Object.freeze({
        label: "Zcash Protocol Specification",
        url: "https://zips.z.cash/protocol/protocol.pdf",
      }),
    ]),
    securityNotes: Object.freeze([
      "Orchard actions require circuit-compatible note/nullifier semantics and domain-separated action hashes.",
      "Viewing-key and outgoing-viewing metadata must remain wallet-local.",
      "Production readiness requires audited Halo2 parameters and note-encryption review.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "Orchard note commitment tree",
      "Orchard nullifier set",
      "Orchard action-bundle verifier key registry",
      "wallet Orchard witness store",
    ]),
    failureModes: Object.freeze([
      "stale anchor",
      "duplicate nullifier",
      "invalid action-bundle proof",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Add Orchard-compatible note, nullifier, action, and anchor data model types.",
      "Register Orchard Halo2 verifier parameters and action-bundle public input layout.",
      "Persist wallet note plaintexts, diversifiers, Merkle witnesses, and outgoing viewing data.",
    ]),
    executionSteps: Object.freeze([
      "Select spend notes and anchors from the wallet witness store.",
      "Create output notes and value commitments.",
      "Generate one Halo2 proof over the action bundle and submit nullifiers plus commitments.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildOrchardActionBundleProofV1",
      "buildOrchardActionBundleInstruction",
    ]),
    chainRequirements: Object.freeze([
      "Orchard note commitment tree",
      "Orchard nullifier set",
      "Halo2 action-bundle verifier",
      "wallet Orchard witness store",
      "typed Orchard action-bundle instruction",
    ]),
  }),
  Object.freeze({
    id: "penumbra-masp-v1",
    name: "Penumbra-style multi-asset shielded pool v1",
    shortName: "Penumbra MASP",
    summary:
      "Single multi-asset shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs for private IBC-style assets.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze([
      "hide_amount",
      "hide_sender",
      "hide_receiver",
      "hide_asset_type",
    ]),
    proofFamily: "groth16-bls12-377-decaf377",
    publicInputsSchema:
      "state_commitment_anchor,nullifiers,note_commitments,balance_commitment,asset_id_commitment",
    verifierKeyId: "penumbra_masp_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "multi-asset shielded pools",
      "IBC-style asset privacy",
      "asset-id hiding with typed-value notes",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Penumbra Multi-Asset Shielded Pool",
        url: "https://protocol.penumbra.zone/main/shielded_pool.html",
      }),
      Object.freeze({
        label: "Penumbra Cryptographic Primitives",
        url: "https://protocol.penumbra.zone/main/crypto.html",
      }),
    ]),
    securityNotes: Object.freeze([
      "Typed asset values must bind asset identifiers to balance commitments.",
      "Groth16 parameter registration must distinguish spend and output circuits.",
      "Wallet note plaintexts and position metadata must not be exposed through public APIs.",
      "Production MASP use requires audited parameter governance and chain-state integration review.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "multi-asset state commitment tree",
      "typed nullifier set",
      "Groth16 spend/output verifier key registry",
      "wallet asset metadata witness store",
    ]),
    failureModes: Object.freeze([
      "stale state commitment anchor",
      "duplicate nullifier",
      "asset balance commitment mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Add typed-value notes, asset identifiers, state commitments, and nullifier state.",
      "Register Groth16/BLS12-377 verifier parameters for spend and output proofs.",
      "Persist wallet note plaintexts, asset metadata, state commitment positions, and nullifier keys.",
    ]),
    executionSteps: Object.freeze([
      "Select positioned notes and derive nullifiers.",
      "Create typed output notes and balance commitments.",
      "Submit spend/output actions with proofs against the shielded pool state commitment tree.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildPenumbraSpendProofV1",
      "buildPenumbraOutputProofV1",
      "buildPenumbraShieldedPoolTransaction",
    ]),
    chainRequirements: Object.freeze([
      "multi-asset state commitment tree",
      "typed note commitment and nullifier state",
      "Groth16 verifier registry",
      "wallet multi-asset witness store",
      "typed Penumbra shielded-pool transaction admission",
    ]),
  }),
  Object.freeze({
    id: "monero-fcmp-plus-plus-v1",
    name: "Monero FCMP++ RingCT-style transfer v1",
    shortName: "FCMP++",
    summary:
      "Full-chain membership proof target that replaces small decoy rings with a full-output-set spend proof while retaining hidden amounts and one-time receivers.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "fcmp-plus-plus-curve-trees-bulletproofs",
    publicInputsSchema:
      "membership_root,key_image_or_link_tag,amount_commitments,range_commitments,spend_authorization,chain_tag",
    verifierKeyId: "monero_fcmp_plus_plus_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "maximal sender anonymity sets",
      "decoy-ring replacement research",
      "account-independent UTXO spend privacy",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Monero FCMP++ Development",
        url: "https://web.getmonero.org/2024/04/27/fcmps.html",
      }),
    ]),
    securityNotes: Object.freeze([
      "Full-chain membership roots must be canonical and replay protected.",
      "Link tags/key images must be unique without revealing owned outputs.",
      "Range-proof and amount-commitment parameters require production verifier review.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "full-output-set commitment accumulator",
      "spent link-tag set",
      "FCMP++ verifier key registry",
      "wallet output ownership scan state",
    ]),
    failureModes: Object.freeze([
      "stale membership root",
      "duplicate link tag",
      "amount commitment mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Add output commitment accumulator state suitable for full-chain membership proofs.",
      "Define link tags/key images and spent-output rejection for Iroha assets.",
      "Implement wallet scanning, ownership recovery, and amount commitment witness storage.",
    ]),
    executionSteps: Object.freeze([
      "Select owned outputs from the wallet scan state.",
      "Generate full-chain membership and amount-conservation proofs.",
      "Submit link tag, output commitments, range proof, and spend authorization.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildFcmpPlusPlusMembershipProofV1",
      "buildFcmpPlusPlusTransferInstruction",
    ]),
    chainRequirements: Object.freeze([
      "full-output-set commitment accumulator",
      "spent link-tag set",
      "FCMP++ verifier",
      "wallet scanning and ownership recovery",
      "typed FCMP++ transfer instruction",
    ]),
  }),
  Object.freeze({
    id: "miden-stark-note-v1",
    name: "Miden-style STARK private note transaction v1",
    shortName: "Miden STARK",
    summary:
      "Client-side STARK-proved account transition using private notes whose data stays off-chain while note hashes/nullifiers anchor correctness.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze([
      "hide_amount",
      "hide_receiver",
      "hide_asset_type",
    ]),
    proofFamily: "stark-vm-note-transaction",
    publicInputsSchema:
      "account_id,initial_account_commitment,final_account_commitment,input_note_nullifiers,output_note_hashes,reference_block",
    verifierKeyId: "miden_stark_note_v1",
    pqLayers: Object.freeze({
      proof: true,
      authorization: false,
      noteEncryption: false,
    }),
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "client-side proving",
      "private programmable note workflows",
      "parallel account-local transaction execution",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Miden Transaction Model",
        url: "https://docs.miden.xyz/core-concepts/miden-base/transaction/",
      }),
      Object.freeze({
        label: "Miden Notes",
        url: "https://docs.miden.xyz/core-concepts/miden-base/note/",
      }),
    ]),
    securityNotes: Object.freeze([
      "Private note data and off-chain delivery metadata must stay wallet-local.",
      "Account-local transition proofs must bind initial and final account commitments.",
      "Reference blocks must prevent replay against stale account state.",
      "Production Miden note transactions require audited STARK parameters and account-state integration review.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "private note hash database",
      "input note nullifier set",
      "account commitment state",
      "STARK VM verifier key registry",
      "wallet private note witness store",
    ]),
    failureModes: Object.freeze([
      "stale reference block",
      "duplicate input note nullifier",
      "account commitment transition mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Add private note hash/nullifier state and account-local transition verification.",
      "Register a STARK VM verifier and public-input commitment layout.",
      "Persist private note data and off-chain delivery metadata in the wallet note store.",
    ]),
    executionSteps: Object.freeze([
      "Execute the account-local transition against private note witnesses.",
      "Produce a STARK proof for the transaction script and account state delta.",
      "Submit note nullifiers, output note hashes, account commitments, and proof.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildMidenStarkTransactionProofV1",
      "buildMidenNoteTransactionInstruction",
    ]),
    chainRequirements: Object.freeze([
      "STARK VM verifier",
      "private note hash and nullifier database",
      "account commitment state",
      "wallet private-note delivery store",
      "typed Miden note transaction instruction",
    ]),
  }),
  Object.freeze({
    id: "aztec-private-rollup-v1",
    name: "Aztec-style programmable private transaction v1",
    shortName: "Aztec private",
    summary:
      "Programmable private-state transaction using client-side private execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "plonkish-private-kernel-rollup",
    publicInputsSchema:
      "note_hashes,nullifiers,encrypted_logs,public_call_requests,private_kernel_commitment,rollup_state_roots",
    verifierKeyId: "aztec_private_kernel_v1",
    pqLayers: PQ_LAYER_NONE,
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "programmable private payments",
      "hybrid public/private contract workflows",
      "wallet-side private execution with encrypted note discovery",
    ]),
    sourceReferences: Object.freeze([
      Object.freeze({
        label: "Aztec State Management",
        url: "https://docs.aztec.network/developers/docs/foundational-topics/state_management",
      }),
      Object.freeze({
        label: "Aztec Private Kernel Circuit",
        url: "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel",
      }),
    ]),
    securityNotes: Object.freeze([
      "Private-kernel proofs must bind note hashes, nullifiers, encrypted logs, and public calls.",
      "Encrypted log delivery metadata must not leak wallet note ownership.",
      "Recursive verifier registration must distinguish private-kernel versions and rollup state roots.",
      "Production private-rollup use requires audited private-kernel parameters and rollup-state integration review.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "private note-hash tree",
      "nullifier tree",
      "encrypted log delivery store",
      "private-kernel verifier key registry",
      "wallet private execution witness store",
    ]),
    failureModes: Object.freeze([
      "stale rollup state root",
      "duplicate nullifier",
      "private-kernel public input mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Add private note-hash and nullifier trees plus encrypted log delivery metadata.",
      "Register a private-kernel verifier and public-input layout for private contract side effects.",
      "Persist wallet PXE-style note discovery, private call witnesses, and app-scoped nullifier keys.",
    ]),
    executionSteps: Object.freeze([
      "Execute private contract calls locally against wallet notes.",
      "Accumulate note hashes, nullifiers, encrypted logs, and public-call requests in the private kernel.",
      "Submit the recursive private-kernel proof and side-effect commitments for validator verification.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildAztecPrivateKernelProofV1",
      "buildAztecPrivateRollupTransactionInstruction",
    ]),
    chainRequirements: Object.freeze([
      "private note-hash tree",
      "nullifier tree",
      "encrypted log store",
      "private-kernel verifier",
      "wallet private execution environment",
      "typed Aztec private-rollup transaction instruction",
    ]),
  }),
  Object.freeze({
    id: "pq-masp-stark-v0",
    name: "Post-quantum MASP STARK v0",
    shortName: "PQ MASP v0",
    summary:
      "Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption.",
    category: "payment",
    maturity: "specification",
    coveredCriteria: Object.freeze([
      "hide_amount",
      "hide_sender",
      "hide_receiver",
      "hide_asset_type",
      "post_quantum",
    ]),
    proofFamily: "stark-fri",
    publicInputsSchema:
      "pool_id,asset_set_root,nullifier_set,output_commitments,root,chain_tag,pq_policy_hash",
    verifierKeyId: "pq_masp_stark_v0",
    pqLayers: Object.freeze({
      proof: true,
      authorization: true,
      noteEncryption: true,
    }),
    implementationStage: RESEARCH_STAGE_MAY_2026,
    recommendedFor: Object.freeze([
      "end-to-end post-quantum privacy target",
      "long-horizon central-bank pilot research",
      "strict PQ proof, authorization, and note-encryption experiments",
    ]),
    sourceReferences: Object.freeze([
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
    securityNotes: Object.freeze([
      "PQ MASP combines experimental STARK/FRI proving with production PQ authorization and note encryption requirements.",
      "ML-DSA domains and ML-KEM ciphertext formats must be bound to verifier keys and pool identifiers.",
      "Post-quantum readiness still requires parameter review, parser fuzzing, and external audit.",
      "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.",
      "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.",
      "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review.",
    ]),
    requiredState: Object.freeze([
      "PQ MASP asset-set commitment root",
      "PQ nullifier set",
      "ML-KEM encrypted note payload store",
      "wallet PQ note witness store",
    ]),
    failureModes: Object.freeze([
      "stale asset-set root",
      "duplicate PQ nullifier",
      "ML-DSA or ML-KEM domain mismatch",
      "malformed proof bytes",
      "wrong verifier key",
      "public input mismatch",
    ]),
    setupSteps: Object.freeze([
      "Register STARK/FRI verifier parameters and PQ MASP public input layout.",
      "Define ML-DSA authorization domains and ML-KEM note-encryption payload formats.",
      "Persist wallet PQ note witnesses, nullifier positions, and encapsulation metadata.",
    ]),
    executionSteps: Object.freeze([
      "Select PQ MASP input notes and derive nullifiers.",
      "Generate STARK/FRI transfer proofs with ML-DSA authorization and ML-KEM output-note encryption.",
      "Submit nullifiers, output commitments, PQ policy hash, and proof for verifier admission.",
    ]),
    sdkEntrypoints: Object.freeze([]),
    plannedSdkEntrypoints: Object.freeze([
      "buildPqMaspStarkTransferProofV0",
      "buildPqMaspStarkRegisterPoolInstruction",
      "buildPqMaspStarkTransferInstruction",
      "generateMlDsaKeyPair",
      "encapsulateMlKem",
    ]),
    chainRequirements: Object.freeze([
      "STARK/FRI verifier enabled",
      "ML-DSA transaction authorization",
      "ML-KEM note payload encryption",
      "zk::RegisterAssetHiddenZkPool",
      "zk::AssetHiddenZkTransfer",
      "active PQ MASP verifier key",
    ]),
  }),
]));

function cloneDescriptor(descriptor) {
  const productionGate = productionGateForDescriptor(descriptor);
  const backendFamily = backendFamilyForDescriptor(descriptor);
  return deepFreeze({
    id: descriptor.id,
    name: descriptor.name,
    shortName: descriptor.shortName,
    summary: descriptor.summary,
    category: descriptor.category ?? "payment",
    maturity: descriptor.maturity ?? "specification",
    coveredCriteria: [...descriptor.coveredCriteria],
    proofFamily: descriptor.proofFamily,
    publicInputsSchema: descriptor.publicInputsSchema,
    verifierKeyId: descriptor.verifierKeyId,
    backendFamily,
    pqLayers: { ...descriptor.pqLayers },
    implementationStage: descriptor.implementationStage ?? null,
    recommendedFor: [...(descriptor.recommendedFor ?? [])],
    sourceReferences: (descriptor.sourceReferences ?? []).map((reference) => ({
      label: reference.label,
      url: reference.url,
    })),
    securityNotes: [...(descriptor.securityNotes ?? [])],
    requiredState: [...(descriptor.requiredState ?? [])],
    failureModes: [...(descriptor.failureModes ?? [])],
    setupSteps: [...(descriptor.setupSteps ?? [])],
    executionSteps: [...(descriptor.executionSteps ?? [])],
    sdkEntrypoints: [...descriptor.sdkEntrypoints],
    plannedSdkEntrypoints: [...(descriptor.plannedSdkEntrypoints ?? [])],
    chainRequirements: [...descriptor.chainRequirements],
    productionReady: productionGate.ready,
    productionGate,
  });
}

export function getPrivacyCriteria() {
  return Object.freeze([...PRIVACY_CRITERIA]);
}

export function getPrivacyAlgorithmDescriptors() {
  return Object.freeze(PRIVACY_ALGORITHMS.map(cloneDescriptor));
}

export function getPrivacyAlgorithmDescriptor(id) {
  return getPrivacyAlgorithmDescriptors().find((algorithm) => algorithm.id === id) ?? null;
}

export function getPrivacyCapabilities() {
  return deepFreeze({
    javascriptSdkAvailable: true,
    bridgeAvailable: isPrivacyNativeAvailable() === true,
    privacyAlgorithms: getPrivacyAlgorithmDescriptors(),
    privacyCriteria: getPrivacyCriteria(),
  });
}
