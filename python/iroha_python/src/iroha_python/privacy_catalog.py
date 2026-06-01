"""Privacy algorithm catalog metadata for the Python SDK."""

from __future__ import annotations

import copy
import json
from typing import Any, Mapping

PRIVACY_CRITERIA = (
    "hide_amount",
    "hide_sender",
    "hide_receiver",
    "hide_asset_type",
    "post_quantum",
)

_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON = (
    "[{\"id\":\"transparent-transfer\",\"name\":\"Transparent asset transfer\",\"shortName\":\"Transparent\",\"sum"
    "mary\":\"Public Iroha asset transfer used as the size and latency baseline.\",\"category\":\"payment\","
    "\"maturity\":\"specification\",\"coveredCriteria\":[],\"proofFamily\":\"none\",\"publicInputsSchema\":null,\""
    "verifierKeyId\":null,\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"imp"
    "lementationStage\":null,\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requiredSta"
    "te\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildTransferAss"
    "etInstruction\",\"buildTransaction\",\"submitSignedTransaction\"],\"plannedSdkEntrypoints\":[],\"chainRe"
    "quirements\":[\"Transfer::Asset\"]},{\"id\":\"shield\",\"name\":\"Shield into confidential note\",\"shortNam"
    "e\":\"Shield\",\"summary\":\"Debits public balance and appends an encrypted receiver note commitment.\""
    ",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_receiver\"],\"proofFamil"
    "y\":\"commitment-only\",\"publicInputsSchema\":\"asset,from,amount,note_commitment\",\"verifierKeyId\":\"z"
    "k::Shield\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementati"
    "onStage\":null,\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requiredState\":[],\"f"
    "ailureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildShieldInstruction\",\""
    "buildTransaction\",\"submitSignedTransaction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"zk"
    "::RegisterZkAsset\",\"zk::Shield\"]},{\"id\":\"confidential-transfer-v2\",\"name\":\"Confidential transfer"
    " v2\",\"shortName\":\"Confidential v2\",\"summary\":\"Halo2/Pasta note-to-note transfer that hides amoun"
    "t, sender note, and receiver note while publishing the asset id.\",\"category\":\"payment\",\"maturity"
    "\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":"
    "\"halo2-ipa-pasta\",\"publicInputsSchema\":\"input_commitment_0,input_commitment_1,nullifier_0,nullif"
    "ier_1,output_commitment_0,output_commitment_1,root,asset_tag,chain_tag\",\"verifierKeyId\":\"confide"
    "ntial_transfer_v2\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"impl"
    "ementationStage\":null,\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requiredStat"
    "e\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildConfidential"
    "TransferProofV2\",\"buildZkTransferInstruction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\""
    "zk::ZkTransfer\",\"active confidential transfer verifier key\",\"wallet note witness store\"]},{\"id\":"
    "\"unshield\",\"name\":\"Unshield to public balance\",\"shortName\":\"Unshield\",\"summary\":\"Spends a privat"
    "e note into a public receiver balance; the private source note remains hidden.\",\"category\":\"paym"
    "ent\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_sender\"],\"proofFamily\":\"halo2-ipa-pasta"
    "\",\"publicInputsSchema\":\"input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,change_com"
    "mitment_0,root,public_amount,asset_tag,chain_tag\",\"verifierKeyId\":\"confidential_unshield_v3\",\"pq"
    "Layers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":null,"
    "\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":["
    "],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildConfidentialUnshieldProofV3\",\"buil"
    "dUnshieldInstruction\"],\"plannedSdkEntrypoints\":[],\"chainRequirements\":[\"zk::Unshield\",\"active co"
    "nfidential unshield verifier key\",\"wallet note witness store\"]},{\"id\":\"asset-hidden-confidential"
    "-transfer-v1\",\"name\":\"Asset-hidden MASP transfer v1\",\"shortName\":\"MASP v1\",\"summary\":\"Target mul"
    "ti-asset shielded-pool transfer that hides amount, sender note, receiver note, and exact asset i"
    "nside a pool.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\","
    "\"hide_sender\",\"hide_receiver\",\"hide_asset_type\"],\"proofFamily\":\"halo2-ipa-pasta\",\"publicInputsSc"
    "hema\":\"pool_id,asset_set_root,input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,outp"
    "ut_commitment_0,output_commitment_1,root,chain_tag\",\"verifierKeyId\":\"asset_hidden_transfer_v1\",\""
    "pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"va"
    "lidator-scaffold-as-of-2026-05\",\"recommendedFor\":[],\"sourceReferences\":[],\"securityNotes\":[],\"re"
    "quiredState\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildRe"
    "gisterAssetHiddenZkPoolInstruction\",\"buildAssetHiddenZkTransferInstruction\"],\"plannedSdkEntrypoi"
    "nts\":[\"buildConfidentialAssetHiddenTransferProofV1\"],\"chainRequirements\":[\"zk::RegisterAssetHidd"
    "enZkPool\",\"zk::AssetHiddenZkTransfer\",\"asset-hidden pool verifier registry state\",\"pool note wit"
    "ness store\"]},{\"id\":\"zk-ace-pq-authorization-v0\",\"name\":\"ZK-ACE post-quantum authorization v0\",\""
    "shortName\":\"ZK-ACE PQ auth\",\"summary\":\"STARK/FRI-backed source-account authorization for trans"
    "parent asset transfers.\",\"category\":\"authorization\",\"maturity\":\"arxiv_preprint\",\"coveredCriter"
    "ia\":[],\"proofFamily\":\"stark/fri/sha256-goldilocks\",\"publicInputsSchema\":\"identity_commit"
    "ment,tx_digest,chain_id,domain_separator,action_class,replay_nullifier,policy_hash,from,to,as"
    "set,amount,verifier_key_id\",\"verifierKeyId\":\"zk_ace_pq_authorization_v0\",\"pqLayers\":{\"proof\":"
    "true,\"authorization\":true,\""
    "noteEncryption\":false},\"implementationStage\":\"chain-executable\",\"recommendedFor\":[\"post-quantum "
    "transaction authorization migration\",\"identity-private source-account authorization\",\"authoriza"
    "tion envelopes for transparent asset transfers\"],\"sourceReferences\":[{\"label\":\"ZK-ACE: Practical"
    " Post-Quantum Authorization for Blockchain\",\"url\":\"https://arxiv.org/abs/2603.07974\"}],\"se"
    "curityNotes\":[\"Authorization is only one PQ layer; proof backend and note encryption must also b"
    "e PQ before a payment flow is end-to-end post-quantum.\",\"Replay nullifiers must be chain-domain "
    "separated and irreversible after acceptance.\",\"A dev verifier must never be accepted under a pro"
    "duction verifier key id.\",\"Native AIR openings are blinded so sampled rows do not recover"
    " identity or replay witness limbs.\"],\"requiredState\":[\"active identity commitment registry\",\"replay nullif"
    "ier set\",\"authorization verifier registry\",\"wallet identity witness and replay-secret store\"],\"f"
    "ailureModes\":[\"transaction digest substitution\",\"chain-id or domain-separator mismatch\",\"replaye"
    "d nullifier\",\"revoked identity commitment\",\"policy hash mismatch\"],\"setupSteps\":[\"Register a ZK-"
    "ACE identity commitment, source-account allowlist, and verifier key.\",\"Initialize replay-state "
    "tracking for the authorizing wallet.\",\"Bind authorization policy hash to the allowed transactio"
    "n action classes.\"],\"executio"
    "nSteps\":[\"Hash the transaction payload and chain/domain context.\",\"Derive a fresh replay nullifi"
    "er.\",\"Generate a ZK-ACE authorization proof and submit a protected transparent transfer.\"],\"sdkE"
    "ntrypoints\":[\"buildRegisterZkAceIdentityCommitmentInstruction\",\"buildRotateZkAceIdentityCommitme"
    "ntInstruction\",\"buildRevokeZkAceIdentityCommitmentInstruction\",\"buildZkAceAuthorizedTransferInst"
    "ruction\",\"buildZkAceAuthorizationProofV1\"],\"plannedSdkEntrypoints\":[\"buildShieldedZkAceAuthorize"
    "dTransferInstruction\"],\"chainRequirements\":[\"zk::RegisterZkAceIdentityCommitment\",\"zk::RotateZkA"
    "ceIdentityCommitment\",\"zk::RevokeZkAceIdentityCommitment\",\"zk::SubmitZkAceAuthorizedTransfer\",\"a"
    "ctive stark/fri/sha256-goldilocks ZK-ACE verifier key\",\"ZK-ACE identity source-account allowli"
    "st\"]},{\"id\":\"anonymous-pgc-k-out-of-n-v1\",\"na"
    "me\":\"Anonymous PGC k-out-of-n payments v1\",\"shortName\":\"Anonymous PGC\",\"summary\":\"Account-based "
    "anonymous confidential payment target with hidden sender, hidden amount, receiver privacy, and k"
    "-out-of-n receiver-set proofs.\",\"category\":\"payment\",\"maturity\":\"accepted_conference\",\"coveredCr"
    "iteria\":[\"hide_amount\",\"hide_sender\",\"hide_receiver\"],\"proofFamily\":\"anonymous-pgc-k-out-of-n\",\""
    "publicInputsSchema\":\"anonymity_set_root,tx_digest,balance_commitments,receiver_set_commitment,re"
    "ceiver_ciphertext_commitments,receiver_threshold,receiver_count,link_tag,range_commitments,chai"
    "n_id,domain_separator\",\"verifierKeyId\":\"anonymous_pgc_k_out_of_n_v1\",\"pqLayers\":{\"proof\":fal"
    "se,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"sdk-builder\",\"recomme"
    "ndedFor\":[\"account-based private payments\",\"multi-receiver confi"
    "dential transfers\",\"payment privacy without a note-based shielded pool UX\"],\"sourceReferences\":["
    "{\"label\":\"Anonymous PGC with k-out-of-n Proofs\",\"url\":\"https://eprint.iacr.org/2025/884\"}],\"secu"
    "rityNotes\":[\"Requires fresh anonymity-set roots and replay/link-tag state.\",\"Amount privacy depe"
    "nds on the range-proof component and commitment binding.\",\"Receiver ciphertext commitments must "
    "bind to the same transaction digest as the proof.\",\"The SDK dev fixture verifies deterministic "
    "binding only; chain execution and production Anonymous PGC proofs remain unavailable.\"],\"requir"
    "edState\":[\"anonymous account commitme"
    "nt set\",\"recent anonymity-set roots\",\"spent link-tag set\",\"range-proof verifier parameters\",\"wal"
    "let account blinding and receiver recovery metadata\"],\"failureModes\":[\"stale or unknown anonymit"
    "y-set root\",\"duplicate link tag\",\"receiver-set substitution\",\"range commitment mismatch\",\"author"
    "ization envelope mismatch\"],\"setupSteps\":[\"Register anonymous account commitments and anonymity-"
    "set accumulator state.\",\"Register the k-out-of-n payment verifier key and range-proof parameters"
    ".\",\"Persist wallet blinding, balance-opening, and receiver recovery witnesses.\"],\"executionSteps"
    "\":[\"Select an anonymity-set root and receiver set.\",\"Create balance commitments, receiver cipher"
    "text commitments, and link tag.\",\"Generate the Anonymous PGC proof and submit the transfer instr"
    "uction.\"],\"sdkEntrypoints\":[\"buildAnonymousPgcReceiverSet\",\"buildAnonymousPgcDevProofFixture\","
    "\"verifyAnonymousPgcDevProofLocally\"],\"plannedSdkEntrypoints\":[\"buildAnonymousPgcAccountCommitm"
    "entInstruction\",\"buildAnonymousPgcKOutOfNProofV1\",\"buildAnonymousPgcTransferInstruction\"],\"chai"
    "nRequirements\":[\"anonymous account commitment accumulator\",\"spent link-tag "
    "set\",\"Anonymous PGC verifier\",\"range-proof component verifier\"]},{\"id\":\"verange-transparent-rang"
    "e-v1\",\"name\":\"VeRange transparent range proofs v1\",\"shortName\":\"VeRange\",\"summary\":\"Verification"
    "-efficient transparent range-proof component for confidential amounts, solvency proofs, and nume"
    "ric credential predicates.\",\"category\":\"proof_backend\",\"maturity\":\"accepted_conference\",\"covered"
    "Criteria\":[\"hide_amount\"],\"proofFamily\":\"verange-transparent-range\",\"publicInputsSchema\":\"commit"
    "ments,range_parameters,aggregation_count,domain_separator,payload_digest\",\"verifierKeyId\":\"veran"
    "ge_transparent_range_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},"
    "\"implementationStage\":\"component\",\"recommendedFor\":[\"confidential amount range proofs\",\"reserve "
    "or solvency proofs\",\"numeric credential predicates\"],\"sourceReferences\":[{\"label\":\"V"
    "eRange: Verification-efficient Zero-knowledge Range Arguments\",\"url\":\"https://eprint.iacr.org/20"
    "25/528\"}],\"securityNotes\":[\"This is a component, not a complete payment protocol.\",\"Range parame"
    "ters must be bound to the transaction payload and verifier key.\",\"Aggregated proof limits must b"
    "e enforced by validators.\",\"Local verification is limited to deterministic dev fixtures; the prod"
    "uction VeRange prover remains unavailable.\"],\"requiredState\":[\"range-proof verifier parameters\","
    "\"range commitment"
    " domain separators\",\"maximum aggregation policy\"],\"failureModes\":[\"wrong bit length\",\"commitment"
    " substitution\",\"verifier-parameter mismatch\",\"oversized aggregation\"],\"setupSteps\":[\"Register Ve"
    "Range verifier parameters and allowed bit lengths.\",\"Define the commitment scheme and domain sep"
    "arators used by dependent algorithms.\"],\"executionSteps\":[\"Build amount commitments.\",\"Generate "
    "a range proof bound to the transaction payload.\",\"Attach the range-proof envelope to the depende"
    "nt confidential algorithm.\"],\"sdkEntrypoints\":[\"buildRangeCommitment\",\"buildVeRangeDevProofFixt"
    "ure\",\"buildVeRangeProofEnvelope\",\"verifyVeRangeProofLocally\"],\"plannedSdkEntrypoints\":[\"build"
    "VeRangeProofV1\"],\"chainRequire"
    "ments\":[\"VeRange verifier registry entry\",\"range commitment binding rules\",\"dependent payment or cr"
    "edential verifier\"]},{\"id\":\"zkat-policy-private-auth-v1\",\"name\":\"zkAt policy-private authorizati"
    "on v1\",\"shortName\":\"zkAt policy auth\",\"summary\":\"Policy-private blockchain authenticator that hi"
    "des threshold rules, signer sets, and account authorization logic.\",\"category\":\"authorization\",\""
    "maturity\":\"accepted_conference\",\"coveredCriteria\":[],\"proofFamily\":\"zkat-policy-private-authenti"
    "cator\",\"publicInputsSchema\":\"policy_commitment,tx_digest,account_id,action_class,domain_separato"
    "r,policy_epoch\",\"verifierKeyId\":\"zkat_policy_private_auth_v1\",\"pqLayers\":{\"proof\":false,\"authori"
    "zation\":false,\"noteEncryption\":false},\"implementationStage\":\"sdk-builder\",\"recommended"
    "For\":[\"institutional wallet policy privacy\",\"hidden threshold authorization\",\"authorization-poli"
    "cy migration without revealing signer topology\"],\"sourceReferences\":[{\"label\":\"zkAt: Zero-Knowle"
    "dge Authenticator for Blockchain\",\"url\":\"https://drops.dagstuhl.de/entities/document/10.4230/LIP"
    "Ics.AFT.2025.2\"}],\"securityNotes\":[\"Hides authorization policy, not payment fields.\",\"Policy com"
    "mitments require explicit epoch and rotation semantics.\",\"Combining with ZK-ACE requires both pr"
    "oofs to bind the same transaction digest.\",\"The SDK dev fixture verifies deterministic binding o"
    "nly; chain policy state and production zkAt proofs remain unavailable.\"],\"requiredState\":[\"poli"
    "cy commitment registry\",\"polic"
    "y epoch state\",\"authorization verifier registry\"],\"failureModes\":[\"policy-root substitution\",\"st"
    "ale policy epoch\",\"unauthorized signer witness\",\"transaction digest mismatch\"],\"setupSteps\":[\"Re"
    "gister a hidden policy commitment and verifier key.\",\"Bind the policy to account action classes "
    "and epoch rules.\"],\"executionSteps\":[\"Generate a policy-private authenticator proof.\",\"Attach th"
    "e authenticator envelope to the transaction authorization path.\"],\"sdkEntrypoints\":[\"buildZkAtP"
    "olicyCommitment\",\"buildZkAtAuthenticatorEnvelope\",\"buildZkAtDevProofFixture\",\"verifyZkAtAuthent"
    "icatorLocally\"],\"plannedSdkEntrypoints\":[\"buildZkAtPolicyCommitmentInstruction\",\"buildZkAtPoli"
    "cyProofV1\",\"buildZkAtAuthorizedTransaction\"],\"chainRequirements\":[\"zkAt policy commitment r"
    "egistry\",\"zkAt verifier\",\"account policy epoch state\"]},{\"id\":\"zk-ams-recursive-admission-v0\",\"n"
    "ame\":\"ZK-AMS recursive anonymous admission v0\",\"shortName\":\"ZK-AMS admission\",\"summary\":\"Researc"
    "h target for recursively aggregated anonymous admission from real-world personhood or eligibilit"
    "y credentials into anonymous on-chain accounts.\",\"category\":\"admission\",\"maturity\":\"arxiv_prepri"
    "nt\",\"coveredCriteria\":[],\"proofFamily\":\"recursive-anonymous-admission\",\"publicInputsSchema\":\"iss"
    "uer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_proof"
    "_digest,domain_separator\",\"verifierKeyId\":\"zk_ams_recursive_admission_v0\",\"pqLayers\":{\"proof\":f"
    "alse,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\":\"sdk-builder\",\"recomm"
    "endedFor\":"
    "[\"anonymous onboarding\",\"Sybil-resistant wallet issuance\",\"credential-gated CBDC pilots\"],\"sourc"
    "eReferences\":[{\"label\":\"ZK-AMS recursive anonymous admission\",\"url\":\"https://arxiv.org/abs/2602."
    "16130\"}],\"securityNotes\":[\"Admission privacy is separate from later payment privacy.\",\"Duplicate"
    " admission prevention depends on issuer-scoped nullifiers.\",\"Recursive batching must bind every "
    "admitted account commitment.\",\"The SDK dev fixture verifies deterministic binding only; chain a"
    "dmission state and production recursive proofs remain unavailable.\"],\"requiredState\":[\"issuer root"
    " registry\",\"admission nullifier set\""
    ",\"anonymous account commitment registry\",\"recursive verifier parameters\"],\"failureModes\":[\"dupli"
    "cate credential admission\",\"wrong issuer root\",\"batch omission or account commitment substitutio"
    "n\",\"recursive proof parameter mismatch\"],\"setupSteps\":[\"Register credential issuer roots and rec"
    "ursive verifier parameters.\",\"Define anonymous account commitment format and admission-nullifier"
    " derivation.\"],\"executionSteps\":[\"Collect admitted account commitments into a batch.\",\"Generate "
    "or import a recursive admission proof.\",\"Submit the batch proof and admission nullifiers.\"],\"sdk"
    "Entrypoints\":[\"buildZkAmsAdmissionBatch\",\"buildZkAmsAdmissionProofEnvelope\",\"buildZkAmsAdmiss"
    "ionDevProofFixture\",\"verifyZkAmsAdmissionProofLocally\"],\"plannedSdkEntrypoints\":[\"buildZkAmsAd"
    "missionBatchProofV0\",\"buildSubmitZkAmsAdmissionBatchInstruction\"],\"chainRequirements\":[\"issuer "
    "root registry\",\"admission nullifier set\",\"r"
    "ecursive admission verifier\"]},{\"id\":\"vega-existing-credential-zk-v0\",\"name\":\"Vega existing-cred"
    "ential ZK proofs v0\",\"shortName\":\"Vega credentials\",\"summary\":\"Low-latency zero-knowledge proof "
    "target for proving predicates over existing credentials without revealing the full credential.\","
    "\"category\":\"credential\",\"maturity\":\"technical_report\",\"coveredCriteria\":[],\"proofFamily\":\"existi"
    "ng-credential-zk\",\"publicInputsSchema\":\"issuer_commitment,credential_schema,predicate_commitment"
    ",subject_binding,expiration_epoch,domain_separator\",\"verifierKeyId\":\"vega_existing_credential_zk"
    "_v0\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStag"
    "e\":\"sdk-builder\",\"recommendedFor\":[\"legacy credential bridges\",\"private eligibility ch"
    "ecks\",\"attribute predicates for wallet enrollment\"],\"sourceReferences\":[{\"label\":\"Vega: Low-Late"
    "ncy Zero-Knowledge Proofs over Existing Credentials\",\"url\":\"https://www.microsoft.com/en-us/rese"
    "arch/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/\"}],\"securityN"
    "otes\":[\"Credential schema parsing must be deterministic and versioned.\",\"Proofs must bind to wal"
    "let or identity commitments to prevent credential replay.\",\"Issuer trust and revocation semantic"
    "s remain external policy inputs.\",\"The SDK dev fixture verifies deterministic binding only; chai"
    "n credential policy state and production Vega proofs remain unavailable.\"],\"requiredState\":[\"cre"
    "dential issuer registry\",\"supported cred"
    "ential schema registry\",\"predicate registry\",\"revocation or expiration policy\"],\"failureModes\":["
    "\"expired credential\",\"wrong issuer\",\"predicate mismatch\",\"wallet-binding replay\"],\"setupSteps\":["
    "\"Register supported credential schemas, issuers, and predicates.\",\"Bind credential proof subject"
    "s to wallet or ZK-ACE identity commitments.\"],\"executionSteps\":[\"Parse the credential under a re"
    "gistered schema.\",\"Generate a predicate proof and bind it to the wallet context.\",\"Submit the pr"
    "oof envelope to the admission or authorization flow.\"],\"sdkEntrypoints\":[\"buildVegaCredentialP"
    "redicateCommitment\",\"buildVegaCredentialProofEnvelope\",\"buildVegaCredentialDevProofFixture\",\"v"
    "erifyVegaCredentialProofLocally\"],\"plannedSdkEntrypoints\":[\"buildVegaCredentialPredicateProofV"
    "0\",\"buildSubmitVegaCredentialProofInstruction\"],\"chainRequirements"
    "\":[\"credential schema registry\",\"issuer registry\",\"credential predicate verifier\"]},{\"id\":\"silen"
    "t-threshold-anoncred-v0\",\"name\":\"Silent threshold anonymous credentials v0\",\"shortName\":\"Silent "
    "threshold cred\",\"summary\":\"Research target for threshold-issued anonymous credentials with silen"
    "t setup, issuer hiding, constant-size showings, and dynamic verifier policies.\",\"category\":\"cred"
    "ential\",\"maturity\":\"technical_report\",\"coveredCriteria\":[],\"proofFamily\":\"threshold-anonymous-cr"
    "edentials\",\"publicInputsSchema\":\"issuer_set_commitment,threshold_policy_hash,credential_showing_"
    "commitment,showing_nullifier,verifier_policy_hash,domain_separator\",\"verifierKeyId\":\"silent_th"
    "reshold_anoncred_v0\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\""
    "implementationStage\":\"sdk-builder\",\"recommendedFor\":[\"multi-authority regulated credentials\",\"issuer-hiding "
    "eligibility proofs\",\"central-bank or supervisor issued wallet credentials\"],\"sourceReferences\":["
    "{\"label\":\"Anonymous Credentials with Issuer-Hiding, Threshold Issuance, and Silent Setup\",\"url\":"
    "\"https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html\"}],\"securityNotes\":[\"Crede"
    "ntial issuance and revocation governance are as important as proof verification.\",\"Issuer-set co"
    "mmitments need rotation and downgrade protections.\",\"This is a credential layer, not a private p"
    "ayment protocol.\",\"The SDK dev fixture verifies deterministic binding only; chain credential sta"
    "te and production silent-threshold proofs remain unavailable.\"],\"requiredState\":[\"threshold issuer registry\",\"credential parameter registry\","
    "\"verifier policy registry\",\"credential showing nullifier policy\"],\"failureModes\":[\"insufficient "
    "issuer threshold\",\"issuer-set substitution\",\"credential showing replay\",\"verifier-policy mismatc"
    "h\"],\"setupSteps\":[\"Register issuer sets, threshold policies, and credential parameters.\",\"Define"
    " showing-nullifier and verifier-policy binding rules.\"],\"executionSteps\":[\"Generate a credential"
    " showing proof under the verifier policy.\",\"Submit the proof as an admission or authorization co"
    "mponent.\"],\"sdkEntrypoints\":[\"buildSilentThresholdCredentialCommitments\",\"buildSilentThreshold"
    "CredentialEnvelope\",\"buildSilentThresholdCredentialDevProofFixture\",\"verifySilentThresholdCred"
    "entialProofLocally\"],\"plannedSdkEntrypoints\":[\"buildSilentThresholdCredentialShowingProofV0\",\""
    "buildSubmitSilentThresholdCredentialProofInstruction\"],\"chainRequirements\":[\"threshold issuer registry"
    "\",\"anonymous credential verifier\",\"credential showing replay policy\"]},{\"id\":\"zk-x509-onchain-id"
    "entity-v0\",\"name\":\"ZK-X.509 on-chain identity v0\",\"shortName\":\"ZK-X.509 identity\",\"summary\":\"ZK "
    "proof target for X.509 certificate validity, ownership, revocation status, and wallet-address bi"
    "nding.\",\"category\":\"identity\",\"maturity\":\"arxiv_preprint\",\"coveredCriteria\":[],\"proofFamily\":\"zk"
    "vm-x509-identity\",\"publicInputsSchema\":\"ca_root_commitment,certificate_policy_hash,revocation_ro"
    "ot,subject_commitment,address_binding,domain_separator\",\"verifierKeyId\":\"zk_x509_onchain_identit"
    "y_v0\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationSta"
    "ge\":\"sdk-builder\",\"recommendedFor\":[\"institutional wallet identity\",\"legal-entity acco"
    "unt binding\",\"private PKI-based eligibility checks\"],\"sourceReferences\":[{\"label\":\"ZK-X.509 on-c"
    "hain identity\",\"url\":\"https://arxiv.org/abs/2603.25190\"}],\"securityNotes\":[\"Legacy X.509 trust r"
    "oots are usually not post-quantum.\",\"Revocation root freshness must be explicit in the public in"
    "puts.\",\"Address binding must prevent proof replay across wallets and chains.\",\"The SDK dev fixture"
    " verifies deterministic public-input binding only; chain trust-root, revocation, policy state, "
    "and production ZK-X.509 proofs remain unavailable.\"],\"requiredState\":["
    "\"trusted CA root registry\",\"certificate policy registry\",\"revocation root registry\",\"identity pr"
    "oof verifier\"],\"failureModes\":[\"expired certificate\",\"revoked certificate\",\"unknown CA root\",\"wr"
    "ong wallet address binding\",\"stale revocation root\"],\"setupSteps\":[\"Register trusted CA roots, c"
    "ertificate policies, and revocation-root feeds.\",\"Define wallet address binding and domain-separ"
    "ation rules.\"],\"executionSteps\":[\"Generate a proof of certificate validity, ownership, and revoc"
    "ation status.\",\"Bind the proof to an institution wallet or ZK-ACE identity commitment.\"],\"sdkEnt"
    "rypoints\":[\"buildZkX509IdentityCommitments\",\"buildZkX509IdentityEnvelope\",\"buildZkX509Identit"
    "yDevProofFixture\",\"verifyZkX509IdentityProofLocally\"],\"plannedSdkEntrypoints\":[\"buildZkX509Id"
    "entityProofV0\",\"buildSubmitZkX509IdentityProofInstruction\"],\"chainRequirements\":[\"trusted CA roo"
    "t registry\",\"revocation root registry\",\"ZK-X.509 verifier\""
    "]},{\"id\":\"jindo-lattice-pcs-zk-v0\",\"name\":\"Jindo lattice polynomial commitment ZK v0\",\"shortName"
    "\":\"Jindo lattice PCS\",\"summary\":\"2026 lattice-based polynomial commitment candidate for post-qua"
    "ntum zero-knowledge proof systems.\",\"category\":\"proof_backend\",\"maturity\":\"technical_report\",\"co"
    "veredCriteria\":[],\"proofFamily\":\"lattice-polynomial-commitment\",\"publicInputsSchema\":\"commitment"
    ",opening_claim,query_set,parameter_hash,domain_separator\",\"verifierKeyId\":\"jindo_lattice_pcs_zk_"
    "v0\",\"pqLayers\":{\"proof\":true,\"authorization\":false,\"noteEncryption\":false},\"implementationStage\""
    ":\"sdk-builder\",\"recommendedFor\":[\"post-quantum proof-system research\",\"future PQ verif"
    "ier backend evaluation\",\"lattice PCS benchmarking\"],\"sourceReferences\":[{\"label\":\"Jindo lattice-"
    "based polynomial commitment\",\"url\":\"https://eprint.iacr.org.cn/2026/044\"}],\"securityNotes\":[\"Thi"
    "s is a proof backend candidate, not a transaction algorithm.\",\"PQ proof coverage alone does not "
    "imply PQ authorization or note encryption.\",\"Parameter selection and implementation security req"
    "uire independent review.\",\"The SDK dev fixture verifies deterministic public-input binding only;"
    " production Jindo lattice proving and verifier backends remain unavailable.\"],\"requiredState\":[\""
    "lattice PCS parameter registry\",\"backend verifier i"
    "mplementation\",\"benchmark fixtures\"],\"failureModes\":[\"parameter mismatch\",\"opening claim substit"
    "ution\",\"unsupported query set\",\"backend misclassified as production-ready\"],\"setupSteps\":[\"Track"
    " lattice PCS parameter sets and verifier API shape.\",\"Benchmark prover, verifier, and proof-size"
    " behavior before integration.\"],\"executionSteps\":[\"Use as a candidate backend for future PQ circ"
    "uits only after concrete circuit integration.\"],\"sdkEntrypoints\":[\"buildJindoLatticePublicInput"
    "s\",\"buildJindoLatticeProofEnvelope\",\"buildJindoLatticeDevProofFixture\",\"verifyJindoLatticeProo"
    "fLocally\"],\"plannedSdkEntrypoints\":[\"buildJindoLatticeProofV0\",\"verifyJindoPolynomialCommitme"
    "ntV0\"],\"chainRequirements\":[\"Jindo verifie"
    "r backend\",\"lattice PCS parameter registry\",\"dependent circuit integration\"]},{\"id\":\"sis-hints-a"
    "noncred-pq-v0\",\"name\":\"SIS-with-hints PQ anonymous credentials v0\",\"shortName\":\"SIS hints anoncr"
    "ed\",\"summary\":\"PKC 2026 research foundation for lattice/SIS-with-hints anonymous credentials and"
    " post-quantum credential proofs.\",\"category\":\"credential\",\"maturity\":\"accepted_conference\",\"cove"
    "redCriteria\":[],\"proofFamily\":\"lattice-anonymous-credentials\",\"publicInputsSchema\":\"issuer_commi"
    "tment,credential_commitment,showing_policy_hash,parameter_hash,domain_separator\",\"verifierKeyId\""
    ":\"sis_hints_anoncred_pq_v0\",\"pqLayers\":{\"proof\":true,\"authorization\":false,\"noteEncryption\":fals"
    "e},\"implementationStage\":\"sdk-builder\",\"recommendedFor\":[\"post-quantum anonymous crede"
    "ntial research\",\"future PQ KYC or eligibility proofs\",\"assumption tracking for lattice credentia"
    "l designs\"],\"sourceReferences\":[{\"label\":\"Tight Reductions for SIS-with-Hints Assumptions with A"
    "pplications\",\"url\":\"https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-wi"
    "th-hints-assumptions-with-applications/\"}],\"securityNotes\":[\"This is a credential foundation, no"
    "t an immediately deployable wallet protocol.\",\"PQ credential proof coverage does not make a paym"
    "ent flow end-to-end post-quantum.\",\"Parameter choices and reduction assumptions need explicit go"
    "vernance.\",\"The SDK dev fixture verifies deterministic public-input binding only; production SI"
    "S-with-hints credential proving and verifier backends remain unavailable.\"],\"requiredState\":[\"la"
    "ttice credential parameter registry\",\"issuer parameter registry\""
    ",\"credential showing verifier\"],\"failureModes\":[\"wrong parameter set\",\"issuer parameter substitu"
    "tion\",\"credential showing replay\",\"overclaiming production readiness from assumption research\"],"
    "\"setupSteps\":[\"Track supported SIS-with-hints parameter sets and issuer parameters.\",\"Define how"
    " future PQ credential showings bind to wallet or authorization contexts.\"],\"executionSteps\":[\"Us"
    "e as a future PQ credential backend after a concrete credential protocol is selected.\"],\"sdkEntr"
    "ypoints\":[\"buildSisHintsCredentialCommitments\",\"buildSisHintsCredentialEnvelope\",\"buildSisHint"
    "sCredentialDevProofFixture\",\"verifySisHintsCredentialProofLocally\"],\"plannedSdkEntrypoints\":[\"b"
    "uildSisHintsAnonymousCredentialProofV0\",\"buildSubmitSisHintsCredentialProofInstruction\"],\"chain"
    "Requirements\":[\"lattice anonymous credential verifier\",\"credential param"
    "eter registry\",\"issuer parameter registry\"]},{\"id\":\"orchard-halo2-actions-v1\",\"name\":\"Orchard-st"
    "yle Halo2 action bundle v1\",\"shortName\":\"Orchard Halo2\",\"summary\":\"Zcash Orchard-style action bu"
    "ndle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions"
    ".\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender"
    "\",\"hide_receiver\"],\"proofFamily\":\"halo2-pasta-action-bundle\",\"publicInputsSchema\":\"anchor,nullif"
    "iers,cmx,value_commitments,binding_signature,proof\",\"verifierKeyId\":\"orchard_halo2_action_bundle"
    "_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":false},\"implementationStag"
    "e\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"single-asset private transfers\",\"mature no"
    "te/nullifier wallet design\",\"compact client proofs without Groth16 ceremonies\"],\"sourceReference"
    "s\":[{\"label\":\"ZIP 224 Orchard Shielded Protocol\",\"url\":\"https://zips.z.cash/zip-0224\"},{\"label\":"
    "\"Zcash Protocol Specification\",\"url\":\"https://zips.z.cash/protocol/protocol.pdf\"}],\"securityNote"
    "s\":[],\"requiredState\":[],\"failureModes\":[],\"setupSteps\":[\"Add Orchard-compatible note, nullifier"
    ", action, and anchor data model types.\",\"Register Orchard Halo2 verifier parameters and action-b"
    "undle public input layout.\",\"Persist wallet note plaintexts, diversifiers, Merkle witnesses, and"
    " outgoing viewing data.\"],\"executionSteps\":[\"Select spend notes and anchors from the wallet witn"
    "ess store.\",\"Create output notes and value commitments.\",\"Generate one Halo2 proof over the acti"
    "on bundle and submit nullifiers plus commitments.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":"
    "[\"buildOrchardActionBundleProofV1\",\"buildOrchardActionBundleInstruction\"],\"chainRequirements\":[\""
    "Orchard note commitment tree\",\"Orchard nullifier set\",\"Halo2 action-bundle verifier\",\"wallet Orc"
    "hard witness store\"]},{\"id\":\"penumbra-masp-v1\",\"name\":\"Penumbra-style multi-asset shielded pool "
    "v1\",\"shortName\":\"Penumbra MASP\",\"summary\":\"Single multi-asset shielded pool using typed notes, n"
    "ote commitments, nullifiers, and spend/output proofs for private IBC-style assets.\",\"category\":\""
    "payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"hide_receive"
    "r\",\"hide_asset_type\"],\"proofFamily\":\"groth16-bls12-377-decaf377\",\"publicInputsSchema\":\"state_com"
    "mitment_anchor,nullifiers,note_commitments,balance_commitment,asset_id_commitment,proof\",\"verifi"
    "erKeyId\":\"penumbra_masp_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":fal"
    "se},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"multi-asset shielde"
    "d pools\",\"IBC-style asset privacy\",\"asset-id hiding with typed-value notes\"],\"sourceReferences\":"
    "[{\"label\":\"Penumbra Multi-Asset Shielded Pool\",\"url\":\"https://protocol.penumbra.zone/main/shield"
    "ed_pool.html\"},{\"label\":\"Penumbra Cryptographic Primitives\",\"url\":\"https://protocol.penumbra.zon"
    "e/main/crypto.html\"}],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":[],\"setupSteps\":[\"Add"
    " typed-value notes, asset identifiers, state commitments, and nullifier state.\",\"Register Groth1"
    "6/BLS12-377 verifier parameters for spend and output proofs.\",\"Persist wallet note plaintexts, a"
    "sset metadata, state commitment positions, and nullifier keys.\"],\"executionSteps\":[\"Select posit"
    "ioned notes and derive nullifiers.\",\"Create typed output notes and balance commitments.\",\"Submit"
    " spend/output actions with proofs against the shielded pool state commitment tree.\"],\"sdkEntrypo"
    "ints\":[],\"plannedSdkEntrypoints\":[\"buildPenumbraSpendProofV1\",\"buildPenumbraOutputProofV1\",\"buil"
    "dPenumbraShieldedPoolTransaction\"],\"chainRequirements\":[\"multi-asset state commitment tree\",\"typ"
    "ed note commitment and nullifier state\",\"Groth16 verifier registry\",\"wallet multi-asset witness "
    "store\"]},{\"id\":\"monero-fcmp-plus-plus-v1\",\"name\":\"Monero FCMP++ RingCT-style transfer v1\",\"short"
    "Name\":\"FCMP++\",\"summary\":\"Full-chain membership proof target that replaces small decoy rings wit"
    "h a full-output-set spend proof while retaining hidden amounts and one-time receivers.\",\"categor"
    "y\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"hide_rec"
    "eiver\"],\"proofFamily\":\"fcmp-plus-plus-curve-trees-bulletproofs\",\"publicInputsSchema\":\"membership"
    "_root,key_image_or_link_tag,amount_commitments,range_proof,spend_authorization\",\"verifierKeyId\":"
    "\"monero_fcmp_plus_plus_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryption\":fals"
    "e},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"maximal sender anony"
    "mity sets\",\"decoy-ring replacement research\",\"account-independent UTXO spend privacy\"],\"sourceRe"
    "ferences\":[{\"label\":\"Monero FCMP++ Development\",\"url\":\"https://web.getmonero.org/2024/04/27/fcmp"
    "s.html\"}],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":[],\"setupSteps\":[\"Add output comm"
    "itment accumulator state suitable for full-chain membership proofs.\",\"Define link tags/key image"
    "s and spent-output rejection for Iroha assets.\",\"Implement wallet scanning, ownership recovery, "
    "and amount commitment witness storage.\"],\"executionSteps\":[\"Select owned outputs from the wallet"
    " scan state.\",\"Generate full-chain membership and amount-conservation proofs.\",\"Submit link tag,"
    " output commitments, range proof, and spend authorization.\"],\"sdkEntrypoints\":[],\"plannedSdkEntr"
    "ypoints\":[\"buildFcmpPlusPlusMembershipProofV1\",\"buildFcmpPlusPlusTransferInstruction\"],\"chainReq"
    "uirements\":[\"full-output-set commitment accumulator\",\"spent link-tag set\",\"FCMP++ verifier\",\"wal"
    "let scanning and ownership recovery\"]},{\"id\":\"miden-stark-note-v1\",\"name\":\"Miden-style STARK pri"
    "vate note transaction v1\",\"shortName\":\"Miden STARK\",\"summary\":\"Client-side STARK-proved account "
    "transition using private notes whose data stays off-chain while note hashes/nullifiers anchor co"
    "rrectness.\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hi"
    "de_receiver\",\"hide_asset_type\"],\"proofFamily\":\"stark-vm-note-transaction\",\"publicInputsSchema\":\""
    "account_id,initial_account_commitment,final_account_commitment,input_note_nullifiers,output_note"
    "_hashes,reference_block\",\"verifierKeyId\":\"miden_stark_note_v1\",\"pqLayers\":{\"proof\":true,\"authori"
    "zation\":false,\"noteEncryption\":false},\"implementationStage\":\"research-target-as-of-2026-05\",\"rec"
    "ommendedFor\":[\"client-side proving\",\"private programmable note workflows\",\"parallel account-loca"
    "l transaction execution\"],\"sourceReferences\":[{\"label\":\"Miden Transaction Model\",\"url\":\"https://"
    "docs.miden.xyz/core-concepts/miden-base/transaction/\"},{\"label\":\"Miden Notes\",\"url\":\"https://doc"
    "s.miden.xyz/core-concepts/miden-base/note/\"}],\"securityNotes\":[],\"requiredState\":[],\"failureMode"
    "s\":[],\"setupSteps\":[\"Add private note hash/nullifier state and account-local transition verifica"
    "tion.\",\"Register a STARK VM verifier and public-input commitment layout.\",\"Persist private note "
    "data and off-chain delivery metadata in the wallet note store.\"],\"executionSteps\":[\"Execute the "
    "account-local transition against private note witnesses.\",\"Produce a STARK proof for the transac"
    "tion script and account state delta.\",\"Submit note nullifiers, output note hashes, account commi"
    "tments, and proof.\"],\"sdkEntrypoints\":[],\"plannedSdkEntrypoints\":[\"buildMidenStarkTransactionPro"
    "ofV1\",\"buildMidenNoteTransactionInstruction\"],\"chainRequirements\":[\"STARK VM verifier\",\"private "
    "note hash and nullifier database\",\"account commitment state\",\"wallet private-note delivery store"
    "\"]},{\"id\":\"aztec-private-rollup-v1\",\"name\":\"Aztec-style programmable private transaction v1\",\"sh"
    "ortName\":\"Aztec private\",\"summary\":\"Programmable private-state transaction using client-side pri"
    "vate execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs.\",\""
    "category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender\",\"h"
    "ide_receiver\"],\"proofFamily\":\"plonkish-private-kernel-rollup\",\"publicInputsSchema\":\"note_hashes,"
    "nullifiers,encrypted_logs,public_call_requests,private_kernel_proof,rollup_state_roots\",\"verifie"
    "rKeyId\":\"aztec_private_kernel_v1\",\"pqLayers\":{\"proof\":false,\"authorization\":false,\"noteEncryptio"
    "n\":false},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"programmable "
    "private payments\",\"hybrid public/private contract workflows\",\"wallet-side private execution with"
    " encrypted note discovery\"],\"sourceReferences\":[{\"label\":\"Aztec State Management\",\"url\":\"https:/"
    "/docs.aztec.network/developers/docs/foundational-topics/state_management\"},{\"label\":\"Aztec Priva"
    "te Kernel Circuit\",\"url\":\"https://docs.aztec.network/developers/nightly/docs/foundational-topics"
    "/advanced/circuits/private_kernel\"}],\"securityNotes\":[],\"requiredState\":[],\"failureModes\":[],\"se"
    "tupSteps\":[\"Add private note-hash and nullifier trees plus encrypted log delivery metadata.\",\"Re"
    "gister a private-kernel verifier and public-input layout for private contract side effects.\",\"Pe"
    "rsist wallet PXE-style note discovery, private call witnesses, and app-scoped nullifier keys.\"],"
    "\"executionSteps\":[\"Execute private contract calls locally against wallet notes.\",\"Accumulate not"
    "e hashes, nullifiers, encrypted logs, and public-call requests in the private kernel.\",\"Submit t"
    "he recursive private-kernel proof and side-effect commitments for validator verification.\"],\"sdk"
    "Entrypoints\":[],\"plannedSdkEntrypoints\":[\"buildAztecPrivateKernelProofV1\",\"buildAztecPrivateRoll"
    "upTransactionInstruction\"],\"chainRequirements\":[\"private note-hash tree\",\"nullifier tree\",\"encry"
    "pted log store\",\"private-kernel verifier\",\"wallet private execution environment\"]},{\"id\":\"pq-mas"
    "p-stark-v0\",\"name\":\"Post-quantum MASP STARK v0\",\"shortName\":\"PQ MASP v0\",\"summary\":\"Target end-t"
    "o-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption"
    ".\",\"category\":\"payment\",\"maturity\":\"specification\",\"coveredCriteria\":[\"hide_amount\",\"hide_sender"
    "\",\"hide_receiver\",\"hide_asset_type\",\"post_quantum\"],\"proofFamily\":\"stark-fri\",\"publicInputsSchem"
    "a\":\"pool_id,asset_set_root,nullifier_set,output_commitments,root,chain_tag,pq_policy_hash\",\"veri"
    "fierKeyId\":\"pq_masp_stark_v0\",\"pqLayers\":{\"proof\":true,\"authorization\":true,\"noteEncryption\":tru"
    "e},\"implementationStage\":\"research-target-as-of-2026-05\",\"recommendedFor\":[\"end-to-end post-quan"
    "tum privacy target\",\"long-horizon central-bank pilot research\",\"strict PQ proof, authorization, "
    "and note-encryption experiments\"],\"sourceReferences\":[{\"label\":\"NIST Post-Quantum Standards\",\"ur"
    "l\":\"https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-e"
    "ncryption-standards\"},{\"label\":\"FIPS 203 ML-KEM\",\"url\":\"https://csrc.nist.gov/pubs/fips/203/fina"
    "l\"},{\"label\":\"FIPS 204 ML-DSA\",\"url\":\"https://csrc.nist.gov/pubs/fips/204/final\"},{\"label\":\"FIPS"
    " 205 SLH-DSA\",\"url\":\"https://csrc.nist.gov/pubs/fips/205/final\"}],\"securityNotes\":[],\"requiredSt"
    "ate\":[],\"failureModes\":[],\"setupSteps\":[],\"executionSteps\":[],\"sdkEntrypoints\":[\"buildRegisterAs"
    "setHiddenZkPoolInstruction\",\"buildAssetHiddenZkTransferInstruction\"],\"plannedSdkEntrypoints\":[\"b"
    "uildPqMaspStarkTransferProofV0\",\"generateMlDsaKeyPair\",\"encapsulateMlKem\"],\"chainRequirements\":["
    "\"STARK/FRI verifier enabled\",\"ML-DSA transaction authorization\",\"ML-KEM note payload encryption\""
    ",\"zk::RegisterAssetHiddenZkPool\",\"zk::AssetHiddenZkTransfer\",\"active PQ MASP verifier key\"]}]"
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


def _canonicalize_value(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {str(_KEY_MAP.get(key, key)): _canonicalize_value(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_canonicalize_value(item) for item in value]
    return value


def _validate_descriptor_shape(descriptor: Mapping[str, Any], index: int) -> None:
    for field in _REQUIRED_STRING_FIELDS:
        value = descriptor.get(field)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field {field!r} must be a non-empty string"
            )

    for field in _REQUIRED_PRESENT_FIELDS:
        if field not in descriptor:
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} field {field!r} is required"
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
        for item_index, item in enumerate(value):
            if not isinstance(item, str):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field {field!r} item {item_index} must be a string"
                )

    if "source_references" in descriptor:
        references = descriptor["source_references"]
        if not isinstance(references, list):
            raise RuntimeError(
                "privacy algorithm catalog entry "
                f"{index} field 'source_references' must be a list"
            )
        for item_index, item in enumerate(references):
            if not isinstance(item, Mapping):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must be an object"
                )
            if not isinstance(item.get("label"), str) or not isinstance(
                item.get("url"), str
            ):
                raise RuntimeError(
                    "privacy algorithm catalog entry "
                    f"{index} field 'source_references' item {item_index} "
                    "must include string label and url"
                )

    pq_layers = descriptor.get("pq_layers")
    if not isinstance(pq_layers, Mapping):
        raise RuntimeError(
            f"privacy algorithm catalog entry {index} field 'pq_layers' must be an object"
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


def _with_boi_compatibility_fields(descriptor: Mapping[str, Any]) -> dict[str, Any]:
    result = dict(descriptor)
    criteria = list(result.get("covered_criteria") or [])
    requirements = list(result.get("chain_requirements") or [])
    security_notes = list(result.get("security_notes") or [])
    failure_modes = list(result.get("failure_modes") or [])
    result["hidden_features"] = criteria
    result["requirements"] = requirements
    result["limitations"] = [*security_notes, *failure_modes]
    result.setdefault("status", "cataloged")
    result.setdefault("unavailable_reason", None)
    result["verifier_key_metadata"] = {
        "verifier_key_id": result.get("verifier_key_id"),
        "proof_family": result.get("proof_family"),
        "public_inputs_schema": result.get("public_inputs_schema"),
        "pq_layers": copy.deepcopy(result.get("pq_layers")),
    }
    return result


def _load_descriptors() -> tuple[dict[str, Any], ...]:
    loaded = json.loads(_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON)
    if not isinstance(loaded, list):
        raise RuntimeError("privacy algorithm catalog must decode to a list")
    descriptors: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for index, item in enumerate(loaded):
        if not isinstance(item, Mapping):
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} must decode to an object"
            )
        canonical = _canonicalize_value(item)
        algorithm_id = canonical.get("id")
        if not isinstance(algorithm_id, str) or not algorithm_id.strip():
            raise RuntimeError(
                f"privacy algorithm catalog entry {index} must include a non-empty id"
            )
        if algorithm_id in seen_ids:
            raise RuntimeError(
                f"privacy algorithm catalog contains duplicate id {algorithm_id!r}"
            )
        seen_ids.add(algorithm_id)
        _validate_descriptor_shape(canonical, index)
        descriptor = _with_boi_compatibility_fields(canonical)
        descriptors.append(descriptor)
    return tuple(descriptors)


_PRIVACY_ALGORITHM_DESCRIPTORS = _load_descriptors()
_PRIVACY_ALGORITHM_DESCRIPTOR_BY_ID = {
    str(descriptor["id"]): descriptor
    for descriptor in _PRIVACY_ALGORITHM_DESCRIPTORS
}


def get_privacy_algorithm_descriptors() -> list[dict[str, Any]]:
    """Return defensive-copy privacy algorithm descriptors."""

    return copy.deepcopy(list(_PRIVACY_ALGORITHM_DESCRIPTORS))


def get_privacy_algorithm_descriptor(algorithm_id: str) -> dict[str, Any] | None:
    """Return one defensive-copy descriptor by id, or ``None`` if unknown."""

    if not isinstance(algorithm_id, str):
        return None
    descriptor = _PRIVACY_ALGORITHM_DESCRIPTOR_BY_ID.get(algorithm_id)
    return copy.deepcopy(descriptor) if descriptor is not None else None


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


def _ml_dsa_available() -> bool:
    try:
        from .crypto import supported_crypto_algorithms
    except Exception:  # pragma: no cover - optional native extension
        return False
    try:
        return "ml-dsa" in supported_crypto_algorithms()
    except Exception:  # pragma: no cover - defensive
        return False


def privacy_capabilities(client: Any | None = None) -> dict[str, Any]:
    """Return SDK privacy catalog and implementation capability metadata."""

    zk_ace_register = _callable_on_instruction("register_zk_ace_identity_commitment")
    zk_ace_rotate = _callable_on_instruction("rotate_zk_ace_identity_commitment")
    zk_ace_revoke = _callable_on_instruction("revoke_zk_ace_identity_commitment")
    zk_ace_transfer = _callable_on_instruction("zk_ace_authorized_transfer")
    zk_ace_prover = _callable_on_crypto("zk_ace_build_transfer_authorization_v1")
    zk_ace_sdk_exports = (
        zk_ace_register and zk_ace_rotate and zk_ace_revoke and zk_ace_transfer and zk_ace_prover
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

    return {
        "python_sdk_available": True,
        "bridge_available": False,
        "privacy_algorithms": get_privacy_algorithm_descriptors(),
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
        "confidential_transfer_proof_v2": False,
        "confidential_unshield_proof_v3": False,
        "asset_hidden_transfer_instruction": False,
        "asset_hidden_pool_registration_instruction": False,
        "asset_hidden_transfer_proof_v1": False,
        "stark_proof_family": True,
        "ml_dsa_authorization": _ml_dsa_available(),
        "ml_kem_note_encryption": False,
    }
