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
