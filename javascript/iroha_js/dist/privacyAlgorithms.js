const PRIVACY_CRITERIA = Object.freeze([
  "hide_amount",
  "hide_sender",
  "hide_receiver",
  "hide_asset_type",
  "post_quantum",
]);

const PQ_LAYER_NONE = Object.freeze({
  proof: false,
  authorization: false,
  noteEncryption: false,
});

const RESEARCH_STAGE_MAY_2026 = "research-target-as-of-2026-05";
const CATALOG_STAGE_MAY_2026 = "catalog-as-of-2026-05";

const PRIVACY_ALGORITHMS = Object.freeze([
  Object.freeze({
    id: "transparent-transfer",
    name: "Transparent asset transfer",
    shortName: "Transparent",
    summary: "Public Iroha asset transfer used as the size and latency baseline.",
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
    ]),
    requiredState: Object.freeze([
      "range-proof verifier parameters",
      "range commitment domain separators",
      "maximum aggregation policy",
    ]),
    failureModes: Object.freeze([
      "wrong bit length",
      "commitment substitution",
      "verifier-parameter mismatch",
      "oversized aggregation",
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
      "Policy commitments require explicit epoch and rotation semantics.",
      "Combining with ZK-ACE requires both proofs to bind the same transaction digest.",
      "The SDK dev fixture verifies deterministic binding only; chain policy state and production zkAt proofs remain unavailable.",
    ]),
    requiredState: Object.freeze([
      "policy commitment registry",
      "policy epoch state",
      "authorization verifier registry",
    ]),
    failureModes: Object.freeze([
      "policy-root substitution",
      "stale policy epoch",
      "unauthorized signer witness",
      "transaction digest mismatch",
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
      "issuer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_proof_digest,domain_separator",
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
    ]),
    requiredState: Object.freeze([
      "issuer root registry",
      "admission nullifier set",
      "anonymous account commitment registry",
      "recursive verifier parameters",
    ]),
    failureModes: Object.freeze([
      "duplicate credential admission",
      "wrong issuer root",
      "batch omission or account commitment substitution",
      "recursive proof parameter mismatch",
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
    ]),
    requiredState: Object.freeze([
      "credential issuer registry",
      "supported credential schema registry",
      "predicate registry",
      "revocation or expiration policy",
    ]),
    failureModes: Object.freeze([
      "expired credential",
      "wrong issuer",
      "predicate mismatch",
      "wallet-binding replay",
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
    ]),
    requiredState: Object.freeze([
      "threshold issuer registry",
      "credential parameter registry",
      "verifier policy registry",
      "credential showing nullifier policy",
    ]),
    failureModes: Object.freeze([
      "insufficient issuer threshold",
      "issuer-set substitution",
      "credential showing replay",
      "verifier-policy mismatch",
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
    ]),
    requiredState: Object.freeze([
      "trusted CA root registry",
      "certificate policy registry",
      "revocation root registry",
      "identity proof verifier",
    ]),
    failureModes: Object.freeze([
      "expired certificate",
      "revoked certificate",
      "unknown CA root",
      "wrong wallet address binding",
      "stale revocation root",
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
    ]),
    requiredState: Object.freeze([
      "lattice PCS parameter registry",
      "backend verifier implementation",
      "benchmark fixtures",
    ]),
    failureModes: Object.freeze([
      "parameter mismatch",
      "opening claim substitution",
      "unsupported query set",
      "backend misclassified as production-ready",
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
    ]),
    requiredState: Object.freeze([
      "lattice credential parameter registry",
      "issuer parameter registry",
      "credential showing verifier",
    ]),
    failureModes: Object.freeze([
      "wrong parameter set",
      "issuer parameter substitution",
      "credential showing replay",
      "overclaiming production readiness from assumption research",
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
    ]),
  }),
  Object.freeze({
    id: "orchard-halo2-actions-v1",
    name: "Orchard-style Halo2 action bundle v1",
    shortName: "Orchard Halo2",
    summary:
      "Zcash Orchard-style action bundle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions.",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "halo2-pasta-action-bundle",
    publicInputsSchema:
      "anchor,nullifiers,cmx,value_commitments,binding_signature,proof",
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
    ]),
  }),
  Object.freeze({
    id: "penumbra-masp-v1",
    name: "Penumbra-style multi-asset shielded pool v1",
    shortName: "Penumbra MASP",
    summary:
      "Single multi-asset shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs for private IBC-style assets.",
    coveredCriteria: Object.freeze([
      "hide_amount",
      "hide_sender",
      "hide_receiver",
      "hide_asset_type",
    ]),
    proofFamily: "groth16-bls12-377-decaf377",
    publicInputsSchema:
      "state_commitment_anchor,nullifiers,note_commitments,balance_commitment,asset_id_commitment,proof",
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
    ]),
  }),
  Object.freeze({
    id: "monero-fcmp-plus-plus-v1",
    name: "Monero FCMP++ RingCT-style transfer v1",
    shortName: "FCMP++",
    summary:
      "Full-chain membership proof target that replaces small decoy rings with a full-output-set spend proof while retaining hidden amounts and one-time receivers.",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "fcmp-plus-plus-curve-trees-bulletproofs",
    publicInputsSchema:
      "membership_root,key_image_or_link_tag,amount_commitments,range_proof,spend_authorization",
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
    ]),
  }),
  Object.freeze({
    id: "miden-stark-note-v1",
    name: "Miden-style STARK private note transaction v1",
    shortName: "Miden STARK",
    summary:
      "Client-side STARK-proved account transition using private notes whose data stays off-chain while note hashes/nullifiers anchor correctness.",
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
    ]),
  }),
  Object.freeze({
    id: "aztec-private-rollup-v1",
    name: "Aztec-style programmable private transaction v1",
    shortName: "Aztec private",
    summary:
      "Programmable private-state transaction using client-side private execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs.",
    coveredCriteria: Object.freeze(["hide_amount", "hide_sender", "hide_receiver"]),
    proofFamily: "plonkish-private-kernel-rollup",
    publicInputsSchema:
      "note_hashes,nullifiers,encrypted_logs,public_call_requests,private_kernel_proof,rollup_state_roots",
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
    ]),
  }),
  Object.freeze({
    id: "pq-masp-stark-v0",
    name: "Post-quantum MASP STARK v0",
    shortName: "PQ MASP v0",
    summary:
      "Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption.",
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
    sdkEntrypoints: Object.freeze([
      "buildRegisterAssetHiddenZkPoolInstruction",
      "buildAssetHiddenZkTransferInstruction",
    ]),
    plannedSdkEntrypoints: Object.freeze([
      "buildPqMaspStarkTransferProofV0",
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
]);

function cloneDescriptor(descriptor) {
  return {
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
  };
}

export function getPrivacyCriteria() {
  return [...PRIVACY_CRITERIA];
}

export function getPrivacyAlgorithmDescriptors() {
  return PRIVACY_ALGORITHMS.map(cloneDescriptor);
}

export function getPrivacyAlgorithmDescriptor(id) {
  return getPrivacyAlgorithmDescriptors().find((algorithm) => algorithm.id === id) ?? null;
}
