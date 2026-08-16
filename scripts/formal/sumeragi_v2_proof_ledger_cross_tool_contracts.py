# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

_DURABLE_INTENT_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "TagProjection", "a2816900778eb7ba8d2f8167528090e5e955b4b3dcaab7f2b44c9a29a5a50b30", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "PendingProjection", "60161447cde7da8a227555838fb809cdb492d9c1c9e9f44f935f6784f55173ea", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionDurableIntentTraceProjection", "a35e463374616a20e4ec9ae0d03eaa2a54a6f8121cb2cc9d1d305277920e66bb", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "LockedCommitProgressWitnessProjection", "93ff3786da370f62eca2e28b2e2cc8780750ea67c8aa5ceace209dbe6c8ea4da", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "locked_commit_progress_witness_body", "7c8ea7d7f39d47b14c8111ed8a4baa864d066888d720f59af69b5d0cede3d6b7", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "locked_commit_progress_witness_is_valid", "7aeaf9fa283c74eb189e8e4d48de90a9b4f8f467064449f12a7d630cc2f9b6fc"),
    CrossToolSourceItemSeal(
        "crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs",
        "locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations",
        "b95c8e7404f46ce322e7005b3acfcb2753bb6099a4c8c287b61d493734462e71",
    ),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "BoundaryCapabilityKey", "26f2397d654f1f73d3800b194d54ecc23c1077fed2a7c2638f93011a0f339a8d", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "EffectCapabilityKey", "2a61193fbec35b7d98220de60e82297d9560b37f9e51518671215b4396daeb86", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "EffectSlotProjection", "c8600f9d3064b1107f31927092caf12005bafdd6dd398ea391d5c0b372eea27b", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "EffectTrace", "9404b0c35af3f4a1ae9f20ec7d8e7ce932ae5b3e76ea13742a3745704a040b6f", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ReplayPlanSlotProjection", "fb6183dfedfa450507e2246225a3cc0406c75f7e69bdfeaac13e704d11a7c5a0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ReplayPlanProjection", "b4ca658003484dc3e313ef39c261535805a2760573cf7ad0bc26da0bda397c9f", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SubjectProjection", "ec3c03b7de6c0273a1bd5a97d6548c0dd0fdb25922953ceadd0bf964d5d7aea4", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "EVENT_PERSISTENCE_FAILED", "6a22b9eb79c972e27a9144237b7e954bbcb5115bc506141f4c351d6ed92412af", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "pending_projection", "68fd42dda1c45572ad3911876d77c507ab2c4277c771e00ac432ec7fa6d2dadd", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "boundary_for_pending", "8b26caff839a21e4350fcfc48d65ad15485ef4740673caee4323c3111a3358ce", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "boundary_claim", "2be2c4e14a098fd2334fdbe96fa4b8044461d8d94b7f2a6a3f820274fc5d293e", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "boundary_grant", "1e263db662a7b2e1f7bbdead0f4bb3026e6fbd5174cde1c11ba254036a3dc7ff", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "transition_projection", "49d3a03ed60e2baa9aeb2796d0959c1e1754f77c335920f88eee1694f2768f02", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "event_kind", "dda92f84e6994e7ae557198f8346fbe75ce0c3e424442bd760789465e525d282", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "progress_witness_violation", "8c15eb7f0f261819eb387288d1cce374be7d6b61d3489167cd0a53accfe06d69", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/reducer.rs", "locked_commit_progress_witness_projection", "c19038941ba1aecc7614e2769c7662f8ba3c82074b9166273414be51e2392455", brace_context=(("impl", "Reducer"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionTagProjection", "cedf388a1d75dd88d021cad5f9e1d32148bf6490f155ae5ab8da4b6277e9e3f8", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionPendingProjection", "768c83d4e75107886e1a7571a6a36235cf9edeb929f0d06544c08bb74c335999", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionBoundaryCapabilityKeyProjection", "76b0228be37593251bb40524de9b19183930ffc21eeb39fda1c63716fe491673", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionEffectCapabilityKeyProjection", "0a1144e781ce9e2173a526351529ba26e3c207735bd0abec3c1025ca767fdc66", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionEffectSlotProjection", "cff4a20f7283faad47df2973426e7b6a2c5185c17b8ef67912896f3c0b4b7835", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionEffectTraceProjection", "8e21963a629bd08d8d678023679f639e72d7268be5499a4d9d12cf1d3f9a46c4", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionReplayPlanSlotProjection", "74593e56aac37b4987c0b5dd718801770962ea55a61cb64fa4da3c9793fcc9af", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionReplayPlanProjection", "4a6d3985352c693aa81b94f5c2c2dc635dd99bde36aa1a60a3f049c40703fd63", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionSubjectProjection", "dec194dd86582bafaaefc08d64d5e9d5b4a96a891deed91f6c84a6a3911450d9", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionDurableIntentTraceProjection", "155d58355db1f55df2029dcee4be1c4c9d83a8a7b2a956baa7333ff1666b9c5a", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "LockedCommitProgressWitnessProjection", "ca4b9d60f702fc3ef3c142b7a138862832defa91348a54b397e6ac49c2aedbed", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "locked_commit_progress_witness_projection", "2582b57e6acb3bbd6a693534e0e9a223cbfd322e6f2501b6d88973f4940a7c28", brace_context=(("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "locked_commit_progress_witness_is_valid_kernel", "4f82a73ea44bb746f347cc9348311b8007f181b13b482f7cb6f16a4aeb4ae84b", brace_context=(("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "locked_commit_progress_witness_is_valid", "95b00841bdc864b9be5efa8567b633c53f4d82f2f8ac92f83c9f6f1c0b0aedaa", brace_context=(("verus", "!"),)),
)


_DECISION_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_CONTEXT", "33409c13a220d1d98e9177726db1a182f8a026f4da0e151fa644af901625ae1d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_SUBJECT", "b99774a01d151d72ddc81bc7af1e55c1e089e026b71ec802d7bc8541e74a8c62", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_PAYLOAD", "02f7c3bd232da081ce9fa052ef76a0a5955763f5240520ee9da75bc975cdc774", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_DURABLE_ARTIFACT", "4cf57514ca8437667f4806c168fe88a442c2c99c0f5011570ba4db704aaab448", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_WIRE_HEIGHT_CONTEXT", "b309f42e9d81116473faca384cce0290b74b664e18f8da39fe12db4e05ec04af", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_WIRE_BLOCK_SUBJECT", "7880c318f8ecd6fee5958c5b29b5dc88a0e734e22f42e7007ec4bf6c199a553d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_BLOCK_HEADER", "bc5d0961da79b1743e0c3323d9d54abbaba5c8c7e08a28c2401ba8fd301cc4c7", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_CANONICAL_PAYLOAD", "529035c05fd93a4c1fad214a7a291fc20682b4c75ff6feebfa3d257417e1a153", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_EXECUTION_COMMITMENT", "dbef0d9c0a4006106fd9e29853ecf1730b1066f29b47ff499fba29f327b54a00", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_QUORUM_CERTIFICATE", "18c59ab0bc525d5e27c838beba73fd242b9758db231e7aadd9ad2ac79b867fc2", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_PAYLOAD_MANIFEST", "9248c610f7161ef44e17ef9b770ffc46593416fd2012fff467621224cfdd673a", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_EXECUTED_BLOCK_WIRE", "4824eaa780e3286d058e4d2bb4d0bbdfdf09b3d9e6bd556f9f39406852ac2b84", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_DURABLE_BODY_FRAME", "dcec9166a699504ebd22fda6b8fd416d2052ac4685f4d82afb0aa4fa65056d55", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_FINALITY_ARTIFACT", "566709d706a9035416a72b6c5bdff55acfab73dc386708d2cbbd9bb198fdbf02", "const"),
)


_DECISION_IDENTITY_SOURCE_ITEM_SEALS = _DECISION_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionDecisionIdentityProjection", "e25607726e5f9eee5d6028ce31204182c3076414227fd7d6dee638f7b8dca317", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionQuorumCertificateIdentityProjection", "7dcc65d36334e60d6f913e48672b53c5286cf3582eeb9c2f419bfbd7e6ae6737", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionDurableBodyIdentityProjection", "323488a9f69eb7ecd88214191fb6357dd7bf9c352f4c608618598f5a5b6ef947", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_decision_identity_is_canonical_body", "fd9c398ffdda1eb3d0070999fa4f96bfb5338394aafe3e980cc8789769915915", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_decision_identity_equal_body", "ace4b411e1bea7b9a5e3c879f18c5c42f2b3e589aeecb629bbf5242c26cc55a9", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_quorum_certificate_is_canonical_body", "13e873f589ff5fff16e10b1f538d2e77b2325baa41993197c465a6fb64f76fad", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_quorum_certificate_equal_body", "7c2af1e0691ae79d0cb43237ce1b30cf391e2392cb6fe1534a91c40bdf671a0f", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_durable_body_is_canonical_body", "93396afe2b19d8e66fd3c577b80a51dbd21629dc2107b4b0ca6708f442167fee", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_durable_body_equal_body", "c9c7e3991cf8a23b285f83186a39bfbec4896ec81420db1042e11277c8d57be0", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionDecisionIdentityProjection", "1fdc0691a72f2d0d75a3c534621fda55ab26dd88ec9c7f6166c498bdff51461a", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionQuorumCertificateIdentityProjection", "3b11ab39d07b5c1beafd9f558c3404144ecd1741e25330839916204acaff1364", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionDurableBodyIdentityProjection", "e6c4fba52a718d7f0c6e3db642c0061dd0f40e9586da28af638043bd34d1deaa", "struct", (("verus", "!"),)),
)


_DECISION_RECOVERY_SOURCE_ITEM_SEALS = _DECISION_IDENTITY_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionDecisionRecoveryTraceProjection", "c652698461c4f6ad894ee536cb7c670047951ec579d4a7d2e33c2f3ea884e061", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_decision_recovery_trace_body", "8c0db740cb3c62621e42f7ed6fa3db06c2926f11e530d7549b07efda2b5bf64d", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionDecisionRecoveryTraceProjection", "43e1b475d9e888070aa00dee853d09ffb197fc62745a94791bb4d24b96435cd9", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "PendingKuraApply", "ae72d12b11f78a26b9604a5a97f5b0c354517eb5fb7d4e5c4537a83fee9979c8", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "PendingKuraApplyRecoveryEvidence", "70dbf9941b5ea8a5f41dcb2799b18bd68c05bb6ef046db62f675bad6b8ab363b", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "canonical_typed_identity", "c23d82dc5bb245bf7be77d50d89071ca077a5739817f03bc3503b21c31cbbdcf"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "canonical_hash_identity", "4fd1019eace5b02f4e4ddc295066775995f4154b84c46029e47d0536f91764b6"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "recovery_decision_projection", "74f98dec8698675f48e5394d6bed80affb0939e11e88d8795e72d558e033e3a4"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "recovery_certificate_projection", "f757e341b8e778e85272f87c5d61473ecbf25d2cd1294c512e85974be6b08125"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "recovery_body_projection", "bd2d387d7a9ff306b662e4dc4292591a5b10c42002848cb370ae9bfb42122534"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "recovery_refinement_projection", "561267713efa4664b94996d9707c9a7e7206ad4984177edfc399e6024c9e3ea7", brace_context=(("impl", "PendingKuraApplyRecoveryEvidence"),)),
)


_RELIABLE_FLUSH_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_PAYLOAD", "02f7c3bd232da081ce9fa052ef76a0a5955763f5240520ee9da75bc975cdc774", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_PEER", "9b5a37c7482ccc012e5ce38e72a5e759cec8867724d862860dbe966c734c4147", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_PEER", "05bb4f192a717e2845736a5df77938f40eb7ac6cb228e54d67252eb3f1b7884d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_REQUEST", "c30a75d8552b5af0b8dc836e465e6f665c7923129af482430218bd070afeaee1", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_REPLY_PAYLOAD", "850ae962426684e410dcc6221add8f8edc5e34b8ae395944264dd64cdf75f05b", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_MERGE_ENTRY", "316a64c62735e90a1f4501ce5673d6b02817e22157aeb44449c4daa95caccf84", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_REFERENCE_DIGEST", "0912aa93b8bc087485896d3acd45c3804971c9c0eff4d5c6e6028abb16bc904a", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_NETWORK_RESPONSE", "b67904dff7edab1079bf10428060ddf22fc36a2363092dc49d3f93139f36cc36", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_RESPONSE", "fa7c758a702eee4984d2061a4c614cf6c7e0238231e50b74626566e7fca92e27", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_CHUNK", "c9ea1ca035ceb40618bb13c54ef43e821109b4a8ae2f57518ef40cc55779d892", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_PAYLOAD", "3a24ad5eadf539399d64577fbd1282c4d8e5860c965397ef90b8ba6d2b8fdae3", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_PROCESS_LOCAL", "b3c5efe194c5e5efe52590912006aea56019f100efcb19bf8bfe3d1785207cc4", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_SIBLING_STATE", "6f325449d02f0a2a1aa315dd4e0bf5882be3cea654d0dfd22ef8fc3d3038783f", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE", "d293040073e283f9fbe18b9008a1921c311deae3cac2fa7c211a3b5534a7ba4b", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE", "ae248ce00fd1c263fafcb57584d347a6fe345d9ac14d436e6ed395e98da7614b", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE", "edaf1d9ea5edce375e77927bb141ee73b7f669a6919d6c4a1003b66c08405364", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_REPLY_SOURCE_KEY", "944b4b99e19f87bf6d49d25811c1a208dfc9c21f1cd67360888050ab0bd2e19f", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_REPLY_DELIVERY_ROUTE", "9a51346f80f7afc6fc688f27a55333286ae1ffeca482fffc7bf8f34311e40b7c", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_REPLY_WRITER_OCCURRENCE", "91cb512f72164fb0c8bd4baeb31ffcf1382dbed4a134f25e75e1c747292ae2b2", "const"),
)


_RELIABLE_FLUSH_APPLICATION_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionReliableFlushApplicationProjection", "7ffaf7f495f2a7f96320e1c377c0358c895d00dcfd35bf53f3e8e4d61a443e05", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_reliable_flush_application_body", "49559bef138c5b61161d45ca264e50a35e4c6f48d76d517b289b4f5e6fa64b80", "macro"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_reliable_flush_two_phase_link_body", "b165517bbaf656b9136ab8e10c9c909e53ec68fff3dc2116ce4a1a2eade84da6", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionReliableFlushApplicationProjection", "e71a4e1d1f591e86542c9379fb5d280702e291c1a644144975fd30f8b8e361fd", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushProjectionBytes", "6561689ace006734d5b7da3ed40d527460e78aedd7dd356aeda4219dcb1e8033", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushTargetGateResidual", "13f961f2f17b810e1fc59378e946d0f42828b196f88433e6b9837364729f7993", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushTargetOutboundResidual", "c7059660e4aba5989c599d08bb30907eb313e025c455f749b42ffa9e4df51959", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushChunkArcIdentity", "070ca20fc32e5a4cbc0e90831bd568e88cc665b6e7f32b4c5222c081c93ab408", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushSharedTransferSnapshot", "4d8641651448847cc75c596ed7b65b5de0d845fbb60fb97885013aa3f2235955", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushSiblingGateSnapshot", "d6b5d5cdbfbb0f1f7bd0a27575dfc811cef3a74b86c177cb5d29d6d372b057c6", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushSiblingOutboundSnapshot", "64183ad2a502d96cbc5de2ad0e808b5193dfe628fcf59d66bc0a7f9ca15aa8aa", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushSiblingStateSnapshot", "d5f2c6c8a5687a08677946625293f80a8521f02dc65e5a41fc3c44c48889dfb7", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushGateApplicationPlan", "77cd6b2af38d39809917b3d082104a8e63427bc78ae492d252d1373caa480c04", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushOutboundAttemptPlan", "ecc860974a2764785636c7ed6eb65c77fb35d2b3cfb89133a2b997f766d2618f", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushOutboundApplicationPlan", "e65da131630e5683b7f03c983c0d0ecabe061aad0792c7ed413bb3f553d7c879", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushApplicationPlan", "03544a8e78dcb42adb438eb3575e7e09ed70443daeec153b4aa9da064fbd5e89", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ReliableFlushApplicationObservation", "6a7997f95d96558f590af20e48c903ac8903e017440786bc9380e1d757e298c3", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "field", "70176741a50024ca7c15acc33b51a4d0f582016f2b9029dca403131c97841f01", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "encoded", "5726670fada38d0dfccf0a9135dfff97c15078ef6ebe7929654f765ab56ebc4e", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "bool", "27373cbe2a95736ba5428927384e165b5e543de12001c44e9bbea52a6878f5cf", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "u8", "5315ed728437b29a1fc5fdb839d54afd1d8745cc80a54d2297aed7dea18df5ef", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "u64", "3ef64ae236b3fb465833a63f5916a2b470a9ba0177d79dc25a2d3cec56f380d7", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "usize", "663dae82a41537b3c9157c0d1ce34d9b8430a51f84e3ffe12d5dad957228939d", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "hash", "8d52e6bc0b3e36a2275563271d3f1767a3e54341203a8ff0cd1e792e67be62c5", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "typed_hash", "9e4cd4400baa11201ece5bf143c9d2618b529db97413aabe39a0b2cd77705a1e", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "source", "9d8de926c5a07c7e7d6ba62f11c9e44c5f8a6c875f0f01133559859b51dd5ebc", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "cursor", "002f5526f0b10ffdc6d02129e091459a0ce28de800a186c7fcf1bb74dfe18c59", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "pending_chunk", "6cf5d0b970d680b70cf8afd9f88f01061fedac7e81abe959a51fbe13c4f3ee3c", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "key", "73a8e5fa851fd941b9004bbc3c1eaab53c1ad9538d51132d4ced6f8e6004e2bb", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "finish", "d92b5b03bcfab47c5813a3398fe7a773aa566eec9284b49e0cf91ec7b30d2d0d", brace_context=(("impl", "ReliableFlushProjectionBytes"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "fe82f420ac5c5bf2fd5cb0e60b6635688c006ccdbef572feb130776e409ec5a3", brace_context=(("impl", "ReliableFlushRouteIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "digest", "53ce7070e038967a279ea803de9d507bc7608284337c8a73c7aaf1c8654d78b4", brace_context=(("impl", "ReliableFlushRouteIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "cd0e744904b0fa142732607dfa400340cbdb172f28809973847f73d12ad6c31c", brace_context=(("impl", "ReliableFlushTargetGateResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "digest", "2ef6e6b5f3403e2f307d64b3962c8dbdf32dfdd2ff352608d177105034e6c212", brace_context=(("impl", "ReliableFlushTargetGateResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "e8fba25aa719fe80850ff4633d7db9b0ff53a52872149a86e1102d12041a5679", brace_context=(("impl", "ReliableFlushTargetOutboundResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "digest", "e29da87c7d60f617ac76a0fad11b7d82dce2c137118815ea0da7307c96fd96ff", brace_context=(("impl", "ReliableFlushTargetOutboundResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "15454080eae97fb168dade79fabb3cd4598eee476b71253e6be1797fd0ce293a", brace_context=(("impl", "ReliableFlushChunkArcIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "append_to", "c4e8f5e58b5587c1d067f2cc97a321b8a6b4736d76d69c0b27e3917f27d49abb", brace_context=(("impl", "ReliableFlushChunkArcIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "a0da4d0e59f8b4c0f9ed9af08ed6b80c9e9603b8b3eab608e11e1139be4142dc", brace_context=(("impl", "ReliableFlushSharedTransferSnapshot"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "digest", "be53565d8672ac62cdd092462388d168bcaded210cfe53e28a9c1f20722c113a", brace_context=(("impl", "ReliableFlushSharedTransferSnapshot"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "capture", "7b1586cfae44a33a2e5e2f0d9772c20528ac844711559deb2e2e4b39e6ea7aa4", brace_context=(("impl", "ReliableFlushSiblingStateSnapshot"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "digest", "cd7f087a31da89e54eadce3f660636441ba7b6c2a190f6b6650e18cc9a199e21", brace_context=(("impl", "ReliableFlushSiblingStateSnapshot"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "eq", "2682491a6b73b17ce91aaf6dd374f2158a0196b6a9e0b3ca6487a269466dd0a1", brace_context=(("impl", "PartialEq", "for", "ReliableFlushRouteIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "eq", "85fca42ad6e1b4ae30138ad40b3b4a54b6bf187f3f9a73e54571da62caf98b3f", brace_context=(("impl", "PartialEq", "for", "ReliableFlushTargetGateResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "eq", "b249fcbef09dc41c47eae28e6b1fe275305e502edc078ae817241b0581c8d70a", brace_context=(("impl", "PartialEq", "for", "ReliableFlushTargetOutboundResidual"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "eq", "19d57e02a57170cdb0c1394fc08fe93f44ca5d89ba058aeceafd2deb6662fb0f", brace_context=(("impl", "PartialEq", "for", "ReliableFlushChunkArcIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_typed_identity", "e0f75753b395d116820035bcfe43ba3759fe8973d38d414b5cdbadd3a1a5db8f"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_hash_identity", "3a6cc6e2dcb6137ff1722f167fbd5b4828596bce1bf8422e8177d75c24d0f6f3"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_peer_identity", "3a1c9e36eaf39d860738c32f7612b187ef8dc45db64f7cf57622fbc3ca10a944"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_ordinal_halves", "a70b0fa3ed0d67d201abaee9d4a2cd344a04e741fc991218ce58a0316200833f"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_usize", "674058b603b97c8ffaf4ea506e42514d76237ad8392c0de38eb6a815ad2d30b2"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_topic_tag", "0ce52c4dfc94894c9656eec6bd9348c55b1653b634660857b9f4e6514ee72eaf"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_target_order_position", "c82e9b6840fd471ad4a4cc3cba73d364ac172bae343ac5f39d3cadea6cd8fc5f"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "preflight_reliable_flush_gate", "6f7b8e5cc37b371acc0850850ac00df8c50c957094aac32d6ed13ec16edf6b90"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "preflight_reliable_flush_outbound", "a09d5a66a346811fa664361fa38de502d06af0986384fd4f491cd5cb298d6667"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "preflight_reliable_flush_outbound_attempt", "cdbcb606745395d0325f2046dd5f4cbdf0ad2e4595d2aa6887d9866f232c1d11"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "finish_reliable_flush_application_plan", "86dd7b1577741b605a95df8bd114c3c33717d21f395f229551950394f173d751"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "apply_reliable_flush_application", "93912d6c690daa60be7fda65daa0a997a0e55839d3661560ada00aaca8dd8a52"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "observe_reliable_flush_application", "4b1799bf12e791d70d96dbdeaa7faf4fa73e1ed2c228f441d4862e0a5e68b34a"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_application_occurrence_projection", "a15d6e74a32f1b0117578bf9d2c12ae08f9e0cef7818229886252a4ef9f3bc7f"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "project_reliable_flush_marker", "aa021429243b00767306e42643531251aa0a28d43cc992eae1494278cc5791c3"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "project_reliable_flush_transition", "e65ebf42b756072f30b783798446834e4ef67c44fe051792d3995f5009f065fe"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "process_local_projection", "deef81bb181613540e6a2c3c70ceb9b06f484fac5fc2bf66ed6211133384ffc7"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "project_reliable_flush_residuals", "6c38a1e1206d937ab4d10fa07f18c689490bafb01d1593b326849134398b2ba6"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "reliable_flush_application_projection", "d2a4c7c5289e54b2624e79498974b32c714a64f9883909b1963bdf005ab15863"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "bind_confirmed_worker_trace", "72f0a08d15704fe3639e2fd082a4fb60c694b84e4f1bcf1e38adcf4884edc984", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "process_local_identity_hash", "1f7501bb4b856a53acd3ced8f2e4b4483651b933217748bf9e221046101d8f3d", brace_context=(("impl", "NetworkReplySourceKey"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "process_local_identity_hash", "e5338e78cad94a62cd29e477ff6cb92a09a1049c5adcac7e4859baa421d2faed", brace_context=(("impl", "NetworkReplyRoute"),)),
)


_RELIABLE_FLUSH_SOURCE_ITEM_SEALS = (
    _RELIABLE_FLUSH_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS
    + _RELIABLE_FLUSH_APPLICATION_SOURCE_ITEM_SEALS
    + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionReliableFlushTraceProjection", "ec137085348f9e052ece6aaf19aaabcdc11f6ea0848b1998495f1ea2ff48402b", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_reliable_flush_trace_body", "7b830dafccef15b33a2966e57e2fccd18c6f6ce3e454831cb7378d12ae2db1d5", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionReliableFlushTraceProjection", "908e6622ab099784f36095f2bf96c9ace7fcc2b2be791d6c55d755a4fdd7850d", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "CertifiedMergeSidecarChunkFlushProjection", "e33ee8cd88404b0627305a89a28beb6e9d74cc3a80d9eb105af395fa0effd0db", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "CertifiedMergeSidecarChunkAdmission", "f8f5605e0249bfbe54216287e4d82c2a1cb2a27621731e2073a6f0c16f3e53fa", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "ServerPendingChunkIdentity", "f829ed6abcaefd45ee46ea385995c58f3ac40b535782fdbf36d551d72e221504", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "from_admitted_reply", "a2b593a56511c97eae5bfaca2235490ace31a128884729704bb6dc907a98da44", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "projection", "7c2b58e389e8a3f8d3875bef22e7407f914c44d16baa8db933c14d1b53c754ef", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "matches_ack_identity", "b41736d098c44a77011b4e5e02b84fc849f7c41f4a523ec4d5e7c7138f9308c7", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "projection_matches_identity", "14d965c4c6fa7416fe47a93c29d32b1f244f5baf7af3d72ad64282b996579303", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "matches_materialized_chunk", "353fc1629ed277f4b30242bb13134084b118bb92efa3c92437ae6c7f4b8b507a", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "is_bound_to_attempt", "bc91d3dca2a1cd0d409ae3a00956265c44db98e16fece0312721a30dc7c41d0e", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "is_bound_to_source", "9b3eb15cf936a718438f1aaed1f91a1ff839fad37327b5050180c25859d224f7", brace_context=(("impl", "CertifiedMergeSidecarChunkAdmission"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "from_message", "332520780a3fd24ecb3408909165c5018800bad19562c8434c6b4de225cd7d46", brace_context=(("impl", "ServerPendingChunkIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "matches_admission", "ca5f53b31a91aac8cc3ceec313c4ee309b8ba10126c86c1fde5a97f7083807c5", brace_context=(("impl", "ServerPendingChunkIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/merge_sidecar.rs", "acknowledge_outbound_chunk", "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484", brace_context=(("impl", "MergeSidecarTransport"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_typed_identity", "e0f75753b395d116820035bcfe43ba3759fe8973d38d414b5cdbadd3a1a5db8f"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_hash_identity", "3a6cc6e2dcb6137ff1722f167fbd5b4828596bce1bf8422e8177d75c24d0f6f3"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_peer_identity", "3a1c9e36eaf39d860738c32f7612b187ef8dc45db64f7cf57622fbc3ca10a944"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_ordinal_halves", "efc05f069320482233885330aa53196e5f5d6382da0b8eb482759e7f7854e52b"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_usize", "674058b603b97c8ffaf4ea506e42514d76237ad8392c0de38eb6a815ad2d30b2"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_worker.rs", "reliable_flush_trace_projection", "7d8f14adddee04f2fcb04c6bf45d51debc1b346fcb09e51afee76f1ca520bef5"),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "NetworkActorAdmittedTicketIdentity", "a77a7f1d0c9204b74b042466c371e4bf93d85adb56c77f6533730cc5754fc5e0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "same_ticket", "1264d6ad9adafb1984aa90db2e301a5832e47694a99b33313f9d71adf97dc120", brace_context=(("impl", "NetworkActorAdmittedTicketIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "process_local_identity_hash", "462c668bd2f0bb4f56421c2f95293a2432a6a3e59e35526a6d61ff2915e2ed9e", brace_context=(("impl", "NetworkActorAdmittedTicketIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "matches", "d7aa1ff1bba2b408eacaa4821e5c8791a3298f8ecbbc10a120e4cf4f18dd00aa", brace_context=(("impl", "WeakProgressDeliveryAuthority"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "try_reserve_for_source", "b938440d9f487ac064ca8ac46269d88551da51349ce14b4e470c87d672e82888", brace_context=(("impl", "NetworkActorProgressBudget"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "NetworkReplyFlushIdentity", "dce80dad1dd1ec1ff9d40090bec2dcff4ed40a7565e316acc69f5b87dc8eb089", "struct"),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "from_admitted_ticket", "f675d999da4ab51537f07fc3e02388d5f461d6cf358810aece18dbf173391b1a", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "claim_writer_flush_once", "9dc52cc2206c5ad132a2e7e21d252d1027c6a4ae78046d984b873c1458e17bf9", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "same_ticket_identity", "e417d0a3861187a82e4e2f40558e8a20512d04dfe4090b7d4c96f8a42b660b95", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "same_delivery_occurrence", "6f6a202fa4e0766254d9122a634eee219b02249cf45d7b73b01689bac4600ced", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "same_writer_flush_occurrence", "99beb4a19b45a6bf748b5ad7ee256a5bc60ae7706d9c621f11605ac5886c8711", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "process_local_route_identity_hash", "f96752abc1985978ea7c184b7b435679dfd1465a92c4a4632e11591490223f25", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "process_local_writer_occurrence_identity_hash", "a55b5ddbf7943de293e7253c2f72e3b609fde9f54379e10c00ff1ac25c1c425c", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "is_bound_to_delivery", "fa124e8e78d582904c7ab3a6e1adc21bfd7d124c1541ee02eae08508b924a502", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "is_bound_to_canonical_reply", "83f8ecdc66d15a9e3970f10c5e732707ccb58947d2857be203c7b3e2eec0d9bf", brace_context=(("impl", "NetworkReplyFlushIdentity"),)),
)
)


_TWO_STAGE_RELAY_RETRY_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionTwoStageRelayRetryTraceProjection", "05761b9b01a10197e4cf9aec83adc858eccbe6ecf14ec74a99fbb2167da88a3b", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_two_stage_relay_retry_trace_body", "fda81d366704ada4700101cb7ee870acfec2fabe141133ece61c5d9eb1d84ea9", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionTwoStageRelayRetryTraceProjection", "d57f9345ce3300bdbf9676f2d2672c5119d4e75c5eb44b8724d3ae500d1c1ebe", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "inbound_source_credit_capacity", "f842d672d209cbcaefccb037b5699832b4775813eb02af0d8fa3c427ace221eb"),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "reply_route_source_capacity", "118a3c85c1e4faa0dac128e48981d346085cfb0b90110c0123414f8e8bb0a770", brace_context=(("impl", "<", "T", ":", "Pload", "+", "message", "::", "ClassifyTopic", ",", "E", ":", "Enc", "+", "Sync", ">", "NetworkBaseHandle", "<", "T", ",", "E", ">"),)),
    CrossToolSourceItemSeal("crates/iroha_p2p/src/network.rs", "authenticated_source_credit_capacity", "2c9a309017b0f51e9e3e9a6e3604ce9819c9eb4fed1fb9f004eb653b7354dbaa", brace_context=(("impl", "<", "T", ":", "Pload", "+", "message", "::", "ClassifyTopic", ",", "E", ":", "Enc", "+", "Sync", ">", "NetworkBaseHandle", "<", "T", ",", "E", ">"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "SumeragiRelayCapacityGeometry", "fd24144fb9fd13a9964e0e74742da6afd2a2537c9447b5dcd756e8f0646288dd", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "checked", "437cc7139737ea555afc202dcd74399007ec6be43117bbce7dec68a6cc51f4bd", brace_context=(("impl", "SumeragiRelayCapacityGeometry"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "daemon_source_capacity_matches_two_upstream_lanes", "e076da72adb47662a2908743a343731b0668f8061eb4645365ab1cb40e9d0d9b", brace_context=(("impl", "SumeragiRelayCapacityGeometry"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "class_corridor_covers_authenticated_sources", "02d410bef16235ed815cbe067cc5a2f36cb0063d3fa4f03490c6533d168270b5", brace_context=(("impl", "SumeragiRelayCapacityGeometry"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "SumeragiRelaySourceCredits", "9b0dde87c00cf8e524577325581281be0a24b796b32b243abec53980e0cd6c7d", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "geometry", "b4c2ec9807ac341e0cec136a0febc5ee5f1014d7704bae2753dc2966f773e09a", brace_context=(("impl", "SumeragiRelaySourceCredits"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "semaphore", "95028ea0d5e2780689acb0d23c363b2eeff0b5b0f24143b89d726eb1f2788a02", brace_context=(("impl", "SumeragiRelaySourceCredits"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "HeldSumeragiRelayOwnership", "ca1bf309503b4384b12bca302e2fa926a6f6cba52cfef23282e46286243bce5f", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "CreditedSumeragiRelayWorkItem", "76c7a64c3f811475a019115ad10930a045550a2c3188806cc6a52ef79ef856ed", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "SumeragiRelayIngress", "faab37bed32a2d7e27306de0b5ed8cc60114532555153ea4584cdda93b83593e", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "SumeragiRelayRetention", "8de4f558b1c75000423eadeeaf0fbc66a0fb6987cdd37904196384631a70d2cd", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "FairRetainedQueue", "9788b700c50fed3e82c7078727325543fed4c01b0b291ee49e69aa782ca236a3", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "FairRetainedSelectionTrace", "0f305222909a5fa1fa8f3ae523b84bc3f232bfb57b7b76944887b54922c0f098", "struct"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "send", "897eae8bd11a0e121267b60b141acdc36d9c326c41b4516c0f4ffddba539ce83", brace_context=(("impl", "SumeragiRelayIngress"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "send_rehydrated", "d10242d792a48f0778102fd02d6eb3b1843dfb2c099101cddc181b817eadf668", brace_context=(("impl", "SumeragiRelayIngress"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_rehydrated_ownership_matches", "e256e3116ecb06539aa891d0057e502b9b6bdf46150211a9891168eeb38a256e"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "rehydrate_held_sumeragi_relay_work", "c70d1d27ef4f639937fdbb8e4c5deb035142532b31c7bcfdb539ab752e36bb16"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "prepare_sumeragi_relay_work", "6fbdddbe5b1a6cddad8e75a566f897a44e0d7cc74be88dc037d9aa7339a3791f", brace_context=(("impl", "NetworkRelayShared"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "push", "d30cc609e125945f143c35d42f513479d7255ed739859859a6a2149aa74abf82", brace_context=(("impl", "<", "K", ":", "Clone", "+", "Ord", ",", "T", ">", "FairRetainedQueue", "<", "K", ",", "T", ">"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "pop_if_with_trace", "a370e5f5cee51cc1029e98aa16701951ea9c307d3b01f291fcf29c9a7c67f83c", brace_context=(("impl", "<", "K", ":", "Clone", "+", "Ord", ",", "T", ">", "FairRetainedQueue", "<", "K", ",", "T", ">"),)),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "finish_sumeragi_block_ingress_attempt", "b3022c31815414001838c0db14d09029fbb063a37384696ddd24e1a58388cb27"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "attempt_sumeragi_relay_work", "48785399f28a8eedac4f2e71ef4429c2c7e1ea892511b14ed608360127fafecf"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_relay_source_capacity", "d0bb1db54ebde2668f6f6c52b6dae565c696950422448fbdb994ae9ceb9641ec"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "new_sumeragi_relay_retained_queue", "ed48c8fb78adf6d9cc4750004bf768828d4dca2676d0d58ef76ed272a4e43314"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_relay_source_credits", "142b0e99fd19e9b6ef02821ac06acd5589b1c4644414d99460affcb8ab8cdd59"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_relay_dispatcher_capacity", "5d5c9a68cde0a263c47641d250717da5f186f7b1c3c9437a7d57d05fc0bb581c"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_relay_class_capacity", "6a0f470f2cc7ba50e6893bb586cbd18e99a6ceffa29029279decaf1e692f7d49"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "sumeragi_relay_retain_retry", "f668e5ce645905c1e717c47f35512de6102d0d64f71da6b8508ce454209015f6"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "spawn_sumeragi_relay_dispatcher", "8ec9a952ab902393a16405c7454b18025ac711460e2158745bbb1af40c4efece"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "spawn_network_relay_worker", "78bcf5e86d56182049fcdb979b37ba564aa3724f78be8dd4547216aad26afe44"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "forward_relay_lane", "8aca2344fdba8a6e935e2d852ed79b0c3f4d904560b3a6a52d0428c7990ddceb"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "drive_network_relay_ingress_inner", "fbe68c06eba092f488d4f4f92191b2014fa6923de62d111ffc12319a7cd67bb8"),
    CrossToolSourceItemSeal("crates/irohad/src/main.rs", "run", "33c13b597c3c9ea26897299ca82499093898f6deb5f0f8319b2bdc39e917195a", brace_context=(("impl", "NetworkRelay"),)),
)


_APPLICATION_SOURCE_ITEM_SEALS = _DECISION_IDENTITY_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionApplicationTraceProjection", "12c5a94a04249a64ad0d02f0c8ca519b7756e1dd9a29df4581209965fa7d1496", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "production_application_trace_body", "c2a02aabdd24ec9d7a2f23d57cf0cf59514e1c6928105dd5d3617d47a55dc719", "macro"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionApplicationTraceProjection", "6403b7daf3931631613c93432d3a3c55d546e4bb47f4bc88fbe8c571b259132f", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "DurableApplicationEvidence", "570baf9d8e7044ec944afe52e282de93eb6d4180f4a38973ceeb33761ca4557d", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_typed_identity", "bf0797326a94905ad34fbf1c0a042b0255403ffe70a8892cc760d4f74883401d"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_hash_identity", "d303c934eb8417c06c608edb365ece4f905d2db7ad925723597dbb302da9d172"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_decision_projection", "38df71952019e159d899b79564c44c99d59d2eaf17db3f56090fa30cc579439b"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_certificate_projection", "2e889e4b38a0f374c0684a099fad3affe8a09d543b016377e557292d0e3b0ca2"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_body_projection", "2de15e67317d9b499678fffc13159c125857b15b53d8da365d3dcdb431e55fd3"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_apply.rs", "application_refinement_projection", "bbe11f04a26bf614ec37d77c60d8e0fbfe3017870fc20f7d77d1eb0c22a375ac", brace_context=(("impl", "DurableApplicationEvidence"),)),
)


_SUCCESSOR_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS = (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_CONTEXT", "33409c13a220d1d98e9177726db1a182f8a026f4da0e151fa644af901625ae1d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_SUBJECT", "b99774a01d151d72ddc81bc7af1e55c1e089e026b71ec802d7bc8541e74a8c62", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_PAYLOAD", "02f7c3bd232da081ce9fa052ef76a0a5955763f5240520ee9da75bc975cdc774", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_DOMAIN_DURABLE_ARTIFACT", "4cf57514ca8437667f4806c168fe88a442c2c99c0f5011570ba4db704aaab448", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_WIRE_HEIGHT_CONTEXT", "b309f42e9d81116473faca384cce0290b74b664e18f8da39fe12db4e05ec04af", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_WIRE_BLOCK_SUBJECT", "7880c318f8ecd6fee5958c5b29b5dc88a0e734e22f42e7007ec4bf6c199a553d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_BLOCK_HEADER", "bc5d0961da79b1743e0c3323d9d54abbaba5c8c7e08a28c2401ba8fd301cc4c7", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_CANONICAL_PAYLOAD", "529035c05fd93a4c1fad214a7a291fc20682b4c75ff6feebfa3d257417e1a153", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_QUORUM_CERTIFICATE", "18c59ab0bc525d5e27c838beba73fd242b9758db231e7aadd9ad2ac79b867fc2", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_PAYLOAD_MANIFEST", "9248c610f7161ef44e17ef9b770ffc46593416fd2012fff467621224cfdd673a", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_FINALITY_ARTIFACT", "566709d706a9035416a72b6c5bdff55acfab73dc386708d2cbbd9bb198fdbf02", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD", "1d4fd582252ce375bb72348997971f88caf3785ef272900d72168637adf850f5", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST", "83779f4730c45568e22b7e13fa625168818482e8aeb3319a5581e51e2085d185", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_CONSENSUS_MESSAGE", "f25e9d99528bdd6a6358b66d0a02e68212d4cad65c78cb2c05f8cd94ec70d5de", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "IDENTITY_KIND_CERTIFIED_BODY_REQUEST", "cd606dbb53ebc3457fb5751114407efb9436cae3085a16701e496f7d337ef554", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_AUTHORITY_APPLIED", "b0a29832fd7bd23b0d7afa5736d36f1dc7581a72e088473af4110c9c2bfe0f26", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP", "e37d83f5baaec17cc5617e12f14f7ea6f5c2270103f2714f0be3c36e544ce2ab", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP", "6c2cb1bb1d2a28102ea0a92c6b093bdf1f35ee155bd453ff4deb9014eb61ecbd", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_STAGE_NONE", "c73b27a2712267f30d1cd20a6a292dd35824a3b0ef4a7ad64155867f06e3b651", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_STAGE_QUEUED", "a15a08fc5ec40f4e27bc6d9397042e137650a2ca9dfdde0730262f95b6fffe6f", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_STAGE_RUNNING", "665b557280f70736846a32c60fdbb9a8b4ddfa0678a4a0229feb06538c08f0dd", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_STAGE_COMPLETE", "932bdcc2b84375f727e92014f59e463e6c7fb010659fdd7699f6b44e4d020ce3", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_LIFECYCLE_BEGIN", "447134b62620762580e5fc2653fd6934221a2f3fbe9018c6f52146797b7ed7ed", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_LIFECYCLE_FAIL", "3042dfe5803b0d70c7ebfb4f3d417a5ca412cebe9a22055617c066c2e2f9fe6e", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP", "144caa773050442610f7f43fbe833368a4f15e5fa5f9672ce43753d7ac146f8d", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP", "ee642fd47e827e1cc0a4fdece9163475c66ee3abc9d48ea4d19d12a41571a85e", "const"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "SUCCESSOR_MARKER_ACTIVATED", "6f42d80ed84466bf92fe4c2dbc5e3a765d47f001256dad2de000f9163d1d8a7c", "const"),
)

_SUCCESSOR_COMMON_SOURCE_ITEM_SEALS = _SUCCESSOR_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionDurablePredecessorIdentityProjection", "361743754f3beb16f3c983261f0c2564472326da5bf4082f1bb108a7390f8fb6", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionSuccessorSnapshotProjection", "4101c48d7d4dae52dbde7fd92d1038e79ca22ba00e2cc5feb6cb024c34891be0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionDurablePredecessorIdentityProjection", "8be10fb5a0ea6ddca2fe8c28fa52c10ad4e6fd1d87ca0b2cbc638f73c2d41403", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionSuccessorSnapshotProjection", "e3db3b9321bdb7a1a202112e16f4cb3db6225ffc6de86ad1e1e00a98c1611282", "struct", (("verus", "!"),)),
)

_SUCCESSOR_TERMINAL_APPLICATION_SOURCE_ITEM_SEALS = _SUCCESSOR_COMMON_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionTerminalApplicationWithoutSuccessorActivationProjection", "7f33dbd27571a808a8331cf9cffce8dc84295ab122518c744831f13f1bd5449f", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionTerminalApplicationWithoutSuccessorActivationProjection", "453c0f2f7f823948209dfcbac254ef213e6ca39048eb965f3a09129aa2d2d937", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "DurableV2PredecessorIdentity", "beecd698ba568e1b7719313ef6c0e4513527c3357b2b0245203d608c35d89b48", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "successor_context_refinement_projection", "b58b4a33cf97312646966c919f688b56bb236c686ab78f10fc64657478821e3b"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "successor_block_refinement_projection", "eb3997d346ef005b73eb0786efbc709bb9ced685598ade9615fe39487b44e929"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "authenticate", "0cc8dbe78393a70abbecde6abe87063dce8948c5cb840d4e39ed5419025741bf", brace_context=(("impl", "DurableV2PredecessorIdentity"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "refinement_projection", "48a2b3d658dca723f2595dcd8e7ec29356e06199dd3747d54eb8ad6b3d235677", brace_context=(("impl", "DurableV2PredecessorIdentity"),)),
)

_SUCCESSOR_APPLIED_SOURCE_ITEM_SEALS = _SUCCESSOR_COMMON_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionSuccessorPredecessorBindingProjection", "4d276153e342565d08b35af2dc2af53a79e19c2a469dfe25068fac80867f3823", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionAppliedSuccessorTraceProjection", "05a61b7fa25c881f9be025ebd782cebb0ec1c8f8b18d95fdc3c811d355f09a74", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionSuccessorPredecessorBindingProjection", "7e0b6a3554f38a7a8467793a11ab9f82427db3100350650310cabbac3d9e5f1a", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionAppliedSuccessorTraceProjection", "9a7ab584116cf8acb0eba39442a72932a7eb807b21741d5be4c3a53c2f551b86", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "DurableV2PredecessorIdentity", "beecd698ba568e1b7719313ef6c0e4513527c3357b2b0245203d608c35d89b48", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "DurableSuccessorActivationAuthority", "42bccef606c5b0dc2fdc88408e2b1ea08183deacae7a64ba5c6fbfacf6db3444", "struct"),
)

_SUCCESSOR_RECOVERED_SOURCE_ITEM_SEALS = _SUCCESSOR_COMMON_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionRecoveredSuccessorTraceProjection", "fed07c9381bcfdc4859159b474632efd271fcd053b33c7cb0e62c5c1c17f23b4", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionRecoveredSuccessorTraceProjection", "abcb774c1b2caf8d4e4b8ecea97fdf74621bf18ad90be64582f25ca00404816c", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "DurableSuccessorActivationAuthority", "42bccef606c5b0dc2fdc88408e2b1ea08183deacae7a64ba5c6fbfacf6db3444", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_recovery.rs", "SnapshotSuccessorActivationAuthority", "9cc850d963b528903679c143d32c2e21587e30f190ff42fcc1a55a4c96d8a34a", "struct"),
)

_SUCCESSOR_LIFECYCLE_SOURCE_ITEM_SEALS = _SUCCESSOR_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionSuccessorStartupLifecycleProjection", "1846479b24f79643f6c1920d0e81451dfc8b41ed964fe4a99ffe66b3bff73bcb", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionSuccessorStartupLifecycleProjection", "48fa11976c000de7d8dc685a35f96ce34532b9291605e572442f4d87e166dbfd", "struct", (("verus", "!"),)),
)

_HISTORICAL_CERTIFICATE_SOURCE_ITEM_SEALS = _SUCCESSOR_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionHistoricalCertificateTraceProjection", "712b2fe8b282fd2eac57c35f50fcfd8b45f9dafcc5b8e78dd868f5d1f1f90e9d", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionHistoricalCertificateTraceProjection", "45b0ab84342eb33d7c22e12f7564e55d9d5a404e9bbd8860b83c45960de282d1", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_block_sync.rs", "DiscoveredCommitCertificate", "e0558b3b785f1354d46a6cddfb050a0ce41f08c149a5414c73501f4cc51b4757", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "CommitCertificateReducerAdmission", "35b1b60416baec13dbb3016053b709b602badd3376aea47f26c75b75195fdd7e", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "refinement_projection", "bc8b920d4b2627246b78c109999be2e6b97470909c4968f3ab03ace352771414", brace_context=(("impl", "CommitCertificateReducerAdmission"),)),
)

_HISTORICAL_BODY_SOURCE_ITEM_SEALS = _SUCCESSOR_IDENTITY_CONSTANT_SOURCE_ITEM_SEALS + (
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "CanonicalIdentityProjection", "6988ca07a23b2b0b2f6f97862355cd26b65549402367584c30cc32311fa907b0", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_core/refinement.rs", "ProductionHistoricalBodyPipelineTraceProjection", "2fd6008bed8f678cbd90891c79db7783a5d641f4e8086a1e7e3846376cd4642b", "struct"),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "CanonicalIdentityProjection", "bbf3897009970afe8953d3fd51bfc88105d3a39e332853973ef6f1cb57013fe6", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_sumeragi_core/src/verus_proofs.rs", "ProductionHistoricalBodyPipelineTraceProjection", "0a83de2f2abcd66aae01529e6f8b3d187b416c5b7467295b43a6ed16d39a1f9a", "struct", (("verus", "!"),)),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "PendingFetch", "50ace9e19f281c23899dfeb61aefbac861572c406d75752913b02788844e656e", "struct"),
    CrossToolSourceItemSeal("crates/iroha_core/src/sumeragi/v2_effects.rs", "BodyPipelineOwner", "b4921e315128ae81983acbc72055d53f13077652eb5c79e666e17ee61cdf9969", "struct"),
)

_SUCCESSOR_APPLIED_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
    ("durable_predecessor_is_canonical_body", "2a069095fb4ff28848a8a44a60ec3e61cfc4cb6dd142962349a9facbe426a601"),
    ("durable_predecessor_equal_body", "b0d856effda808421bd8311b27ea82a6e5b2917973177abe3710b3f98bd9038f"),
    ("production_successor_snapshot_body", "5d9f972378ab50288168be5ab429b4dcedaa86fcc193a08e67e913ed5691a13b"),
    ("production_successor_predecessor_binding_body", "3c0492a309ce27f73309380ad0c2ead66c16b7402060fc0c61dfb51262d56943"),
    ("production_applied_successor_trace_body", "4a86242b872ef685fa80cfa024e5c4a4022546c640ab5eb9cee1b4923b29f1a0"),
)

_SUCCESSOR_TERMINAL_APPLICATION_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
    ("durable_predecessor_is_canonical_body", "2a069095fb4ff28848a8a44a60ec3e61cfc4cb6dd142962349a9facbe426a601"),
    ("production_terminal_application_without_successor_activation_body", "2805c2464eee61a440951bfe77bbc34eb6fb2fd850cdebc30ef95e3441558ae4"),
)

_SUCCESSOR_RECOVERED_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
    ("canonical_identity_is_zero_body", "659e4ab0b79335d08311a07134239aa7338818f507fb721b71c336fc65a52f6d"),
    ("durable_predecessor_is_canonical_body", "2a069095fb4ff28848a8a44a60ec3e61cfc4cb6dd142962349a9facbe426a601"),
    ("durable_predecessor_is_zero_body", "08bbec95f27904e084acd9eab948011578e3b3c4a2d3e252f7613120e346fcdf"),
    ("production_successor_snapshot_body", "5d9f972378ab50288168be5ab429b4dcedaa86fcc193a08e67e913ed5691a13b"),
    ("production_recovered_successor_trace_body", "1619ba6365b28436ae6f69dcfc74e5d57ee5227a66fd5d27a717c43b7d30a76d"),
)

_SUCCESSOR_LIFECYCLE_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("production_startup_failure_and_restart_trace_body", "5aa2d29e594aef4f585c74cf2c6875f5d7f98f8b04a6fb597657bc62a8d42eb7"),
)

_HISTORICAL_CERTIFICATE_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
    ("production_historical_certificate_trace_body", "1b004a044c33700c93a06a9684a21559c64c25e5eae42b337866ed0f5cd26b4e"),
)

_HISTORICAL_BODY_SHARED_MACROS = (
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
    ("production_historical_body_pipeline_trace_body", "c999442642f008ace2e73caf53cd2f18af1732dd06638b8d86ae3e17ba7a3ead"),
)


_EFFECTIVE_LOCK_VERUS_SOURCE = (
    "crates/iroha_sumeragi_core/src/effective_lock_verus_proofs.rs"
)
_GENERAL_VERUS_SOURCE = "crates/iroha_sumeragi_core/src/verus_proofs.rs"
_EFFECTIVE_LOCK_TRACE_SHARED_MACRO_SHA256 = {
    "effective_lock_trace_step_body": (
        "33c6474c2b4716e7895b80a9158122c5a77fdf6df79a71f980dbf63285f4b75b"
    ),
    "effective_lock_trace_claim_body": (
        "deb74517f78d1fa98a63eeec0dc8f5ffb958ee213dd345a147f0a5460bee2ffb"
    ),
    "certificate_identity_is_canonical_body": (
        "17d23a7cc1ea1c3ef92612744845897b819823bbb0cb86e0f78762a7fd5a2873"
    ),
    "certificate_identity_equal_body": (
        "868ce2e6bea8294b5486eece5363d634465a5ba51aaca7958544e630e2150577"
    ),
    "timeout_identity_equal_body": (
        "5289d62287a7547f85a9f6edc04538c924bf2152a40e101eaeb8030d4a1645c9"
    ),
    "prepare_identity_in_context_body": (
        "0c65640619289950154223b88ccb4dadbde5cb4221280ef07795a00403b9a30e"
    ),
    "enter_view_locked_prepare_qc_identity_body": (
        "fa803e8d9ee4d9919e5b32d5572c7abf048a2a32f3a10651326395d64376d722"
    ),
    "enter_view_projection_gate_body": (
        "eb798caf85e9275880186d7f992bf0b21c5915d114319d68876392e97ff83aef"
    ),
    "tag_projection_strictly_advances_body": (
        "3671834240e325ebfd10306320d682266254384a6e163059afbbc9b01ba1575c"
    ),
    "canonical_identity_equal_body": (
        "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"
    ),
    "canonical_identity_is_typed_body": (
        "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"
    ),
    "canonical_identity_is_zero_body": (
        "659e4ab0b79335d08311a07134239aa7338818f507fb721b71c336fc65a52f6d"
    ),
}

_ENTER_VIEW_IDENTITY_PRODUCTION_SOURCE = (
    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
)
_ENTER_VIEW_IDENTITY_PRODUCTION_ITEM_SHA256 = {
    "context_identity_projection": (
        "b7cc0bb7f6b90fb6a783a346f523485a323a40a6acbee6a18d2a2ca416ccf100"
    ),
    "subject_identity_projection": (
        "25c3bf2a8f7fc3584d2e356fa35cea40d4917342f59227ec5686d3d02dfc64e6"
    ),
    "certificate_evidence_class": (
        "ecd7ebb0e99fdc78dd26c561704d894612ab8a3ac76d86b08925d45fc23e464f"
    ),
    "certificate_signer_projection": (
        "f38a706561c047c6bdeb245b89348f4d086ec8ea1979742c7cb96ae7b635243f"
    ),
    "certificate_identity_projection": (
        "9063f96070f5694f42984545227e6f99ce1302bcc080996ffb7dc95ecdb7a8d5"
    ),
    "timeout_identity_projection": (
        "30862b71547dbc4b13e2de042810f398e13cc9b3cac1cec48f0db19c2a14aea0"
    ),
    "enter_view_projection": (
        "28e6c56712ef0a6a24df5828dcf1fdc43fe0a5be843b7f8812d8051cb5951dfc"
    ),
}
_ENTER_VIEW_IDENTITY_SUBSTITUTION_TEST = (
    "enter_view_effect_cannot_substitute_an_equal_reference_certificate"
)
_ENTER_VIEW_IDENTITY_SUBSTITUTION_TEST_SHA256 = (
    "c5a2e26e14f83468edbb83187ad6261a8d0ca6ebce9a27ff288fb4b5c0dfe71f"
)

# This inventory is deliberately code-owned rather than evidence-owned.  An
# evidence writer may record these mappings but may not choose or substitute
# them. The 4 + 7 + 6 claim cardinalities are the complete external-constant
# seam currently declared by the three ledger-facing release proof modules.
CROSS_TOOL_REFINEMENT_CONTRACTS = (
    CrossToolObligationContract(
        obligation_id="effective-lock-body-acquisition-production-refinement",
        module="SumeragiV2AsyncLivenessProofs",
        ledger_symbol="EffectiveLockBodyAcquisitionProductionRefinementObligation",
        tla_theorem="EffectiveLockBodyAcquisitionCrossToolRefinement",
        tla_statement=(
            "ProductionEffectiveLockBodyAcquisitionRefinement "
            "=> EffectiveLockBodyAcquisitionProductionRefinementObligation"
        ),
        ledger_declaration_kind="operator",
        ledger_statement=(
            "/\\ ProductionEffectiveLockBodyAcquisitionRefinement "
            "/\\ EffectiveLockAcquisitionModelObligation"
        ),
        tla_proof=(
            "BY EffectiveLockAcquisitionModelObligation "
            "DEF EffectiveLockBodyAcquisitionProductionRefinementObligation"
        ),
        claims=(
            CrossToolClaimContract(
                constant="ProductionEnterViewUsesPostInstallEffectiveLock",
                verus_theorem="production_enter_view_uses_post_install_effective_lock",
                verus_source=_EFFECTIVE_LOCK_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2.rs",
                    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                ),
                verus_parameters="projection: ProductionTransitionProjection,",
                verus_requires=(
                    "projection.enter_view.active, "
                    "production_kernel_relation(projection)"
                ),
                verus_ensures="""
                    production_enter_view_uses_post_install_effective_lock_kernel(
                        production_enter_view_effective_lock_trace(projection),
                        projection.enter_view,
                    ),
                    production_enter_view_effective_lock_trace(projection).kind == 1u8,
                    production_enter_view_effective_lock_trace(projection).protected_after
                        == production_enter_view_effective_lock_trace(projection).protected_before,
                    production_enter_view_effective_lock_trace(projection).owner_after
                        == production_enter_view_effective_lock_trace(projection).owner_before,
                    projection.enter_view.effect_protected_lock.present
                        == projection.enter_view.durable_lock_after.present,
                    projection.enter_view.following_fetch_lock.present
                        == projection.enter_view.durable_lock_after.present,
                    production_enter_view_retains_high_prepare_qc_identity(
                        projection.enter_view
                    ),
                    projection.enter_view.prepare_control_slot_present_after
                        == projection.enter_view.durable_highest_after.present,
                    certificate_identity_equal_body!(
                        projection.enter_view.retained_prepare_qc_after,
                        projection.enter_view.durable_highest_after
                    )
                """,
                verified_kernel=(
                    "production_enter_view_uses_post_install_effective_lock_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "trace: EffectiveLockTraceProjection, "
                    "enter_view: EnterViewProjection,"
                ),
                verified_kernel_body=(
                    "effective_lock_trace_claim_body!(trace, 1u8) "
                    "&& enter_view_locked_prepare_qc_identity_body!(enter_view) "
                    "&& enter_view_high_prepare_qc_control_identity_body!(enter_view)"
                ),
                theorem_kernel_projection=(
                    "production_enter_view_effective_lock_trace(projection), "
                    "projection.enter_view,"
                ),
                theorem_projection_builder=(
                    "production_enter_view_effective_lock_trace"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionTransitionProjection,"
                ),
                theorem_projection_builder_return="EffectiveLockTraceProjection",
                theorem_projection_builder_item_sha256=(
                    "10b679c25dab61a7b295fc9750a74eb9ebf8193062fed9214200f6e274fa7e49"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source=(
                            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                        ),
                        item="check",
                        item_token_sha256=(
                            "ea543ec5958603f0d1bd8bb682e241fa18d328881c6886aff4683818a8c39b52"
                        ),
                        projection="trace, enter_view",
                        required_expression="""
                            if projection.enter_view.active {
                                let enter_view = projection.enter_view;
                                let protected_after = u64::from(
                                    enter_view.durable_lock_after.present
                                );
                                let ownership_after = u64::from(
                                    enter_view.following_fetch_lock.present
                                );
                                let trace = EffectiveLockTraceProjection {
                                    kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
                                    relation_exact:
                                        enter_view_projection_gate_body!(enter_view)
                                        && enter_view.enter_count
                                            == effect_count_body!(projection.effects, 8u8)
                                        && enter_view.fetch_count
                                            == effect_count_body!(projection.effects, 2u8),
                                    protected_before: protected_after,
                                    protected_after: u64::from(
                                        enter_view.effect_protected_lock.present
                                    ),
                                    owner_before: enter_view.fetch_count,
                                    owner_after: ownership_after,
                                    owner_reused: false,
                                    ready_before: 0,
                                    retired_retained: 0,
                                    retired_ready: 0,
                                    ready_after: 0,
                                    store_before: 0,
                                    retired_store: 0,
                                    store_after: 0,
                                    cursor_before: 0,
                                    completion_ready: false,
                                    progress_ready: false,
                                    normal_ready: false,
                                    selected: 0,
                                    cursor_after: 0,
                                };
                                let checked_effective_lock =
                                    check_production_enter_view_effective_lock_transition(
                                        trace, enter_view
                                    )?;
                                let _authorized_effective_lock =
                                    checked_effective_lock.into_projection();
                            }
                        """,
                        token_consumptions=(
                            """
                                let _authorized_effective_lock =
                                    checked_effective_lock.into_projection();
                            """,
                        ),
                    ),
                ),
                linked_consumers=(
                    CrossToolLinkedConsumerContract(
                        source=(
                            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
                        ),
                        item="step",
                        required_expression="""
                            let Some(checked_refinement) =
                                refinement::check(transition)
                            else {
                                let diagnostic = refinement::diagnose(transition);
                                iroha_logger::error!(
                                    event = ?audit_event,
                                    ?diagnostic,
                                    "Sumeragi v2 reducer rejected the transition refinement predicate"
                                );
                                return Err(ReducerError::RefinementViolation);
                            };
                        """,
                        mutation_boundaries=("*self = next;",),
                        brace_context=(("impl", "Reducer"),),
                        item_token_sha256=(
                            "c9f1ab80636f76db9de0ac05f8ce5ca6d121ec9ecdf46b235c551844f9263b97"
                        ),
                        token_consumptions=(
                            """
                                let _authorized_refinement =
                                    checked_refinement.into_projection();
                            """,
                        ),
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionBodyOwnershipPreservesEffectiveLock",
                verus_theorem="production_body_ownership_preserves_effective_lock",
                verus_source=_EFFECTIVE_LOCK_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_body_store.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                verus_parameters=(
                    "current: Option<ProductionExactBodyOwnerProjection>, "
                    "incoming: ProductionExactBodyOwnerProjection, "
                    "binding: ProductionExactBodyOwnerBindingProjection,"
                ),
                verus_requires=(
                    "production_exact_body_owner_binding(current, incoming) "
                    "== Some(binding)"
                ),
                verus_ensures="""
                    production_body_ownership_preserves_effective_lock_kernel(
                        production_body_ownership_effective_lock_trace(
                            current, incoming, binding
                        ),
                    ),
                    production_body_ownership_effective_lock_trace(
                        current,
                        incoming,
                        binding,
                    ).owner_after == 1u64,
                    production_body_ownership_effective_lock_trace(
                        current,
                        incoming,
                        binding,
                    ).protected_after
                        >= production_body_ownership_effective_lock_trace(
                            current,
                            incoming,
                            binding,
                        ).protected_before,
                    production_body_ownership_effective_lock_trace(
                        current,
                        incoming,
                        binding,
                    ).owner_reused
                        == (production_body_ownership_effective_lock_trace(
                            current,
                            incoming,
                            binding,
                        ).owner_before == 1u64)
                """,
                verified_kernel=(
                    "production_body_ownership_preserves_effective_lock_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: EffectiveLockTraceProjection,"
                ),
                verified_kernel_body=(
                    "effective_lock_trace_claim_body!(projection, 2u8)"
                ),
                theorem_kernel_projection=(
                    "production_body_ownership_effective_lock_trace("
                    "current, incoming, binding),"
                ),
                theorem_projection_builder=(
                    "production_body_ownership_effective_lock_trace"
                ),
                theorem_projection_builder_parameters=(
                    "current: Option<ProductionExactBodyOwnerProjection>, "
                    "incoming: ProductionExactBodyOwnerProjection, "
                    "binding: ProductionExactBodyOwnerBindingProjection,"
                ),
                theorem_projection_builder_return="EffectiveLockTraceProjection",
                theorem_projection_builder_item_sha256=(
                    "b92b3a3ae647ca3233651d216b5fc81d5d175cd6b09378ea6a32217785c741b5"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="plan_body_pipeline_owner_hash",
                        item_token_sha256=(
                            "752ff235ca71762b585e46f93d91afb04bbd6fb8ae10454665c083329c2f8d80"
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        projection="ownership_trace",
                        required_expression="""
                            let incoming = Self::project_body_pipeline_owner(
                                tag, key, manifest_hash
                            );
                            let current = self.body_pipeline_owners
                                .get(&key)
                                .copied()
                                .map(|owner| {
                                    Self::project_body_pipeline_owner(
                                        owner.tag, key, owner.manifest_hash
                                    )
                                });
                            let Some(binding) = plan_exact_body_owner_binding(
                                current, incoming
                            ) else {
                                let reason = if current.is_some_and(
                                    |owner| owner.tag != incoming.tag
                                ) {
                                    "one exact body pipeline has conflicting reducer ownership"
                                } else {
                                    "one exact body pipeline has conflicting manifest ownership"
                                };
                                return Err(EffectExecutorError::Contract(
                                    reason.to_owned()
                                ));
                            };
                            let ownership_trace = EffectiveLockTraceProjection {
                                kind: EFFECTIVE_LOCK_TRACE_OWNER,
                                relation_exact: plan_exact_body_owner_binding(
                                    current, incoming
                                ) == Some(binding),
                                protected_before: u64::from(current.is_some_and(
                                    |owner| owner.manifest_hash.is_some()
                                )),
                                protected_after: u64::from(
                                    binding.owner.manifest_hash.is_some()
                                ),
                                owner_before: u64::from(current.is_some()),
                                owner_after: 1,
                                owner_reused: binding.already_owned,
                                ready_before: 0,
                                retired_retained: 0,
                                retired_ready: 0,
                                ready_after: 0,
                                store_before: 0,
                                retired_store: 0,
                                store_after: 0,
                                cursor_before: 0,
                                completion_ready: false,
                                progress_ready: false,
                                normal_ready: false,
                                selected: 0,
                                cursor_after: 0,
                            };
                            let Some(checked_effective_lock) =
                                check_production_body_ownership_effective_lock_transition(
                                    ownership_trace
                                )
                            else {
                                return Err(EffectExecutorError::Contract(
                                    "exact body ownership did not refine the effective-lock trace"
                                        .to_owned(),
                                ));
                            };
                            Ok(BodyPipelineOwnerBindingPlan {
                                key,
                                owner: BodyPipelineOwner {
                                    tag,
                                    manifest_hash: binding.owner.manifest_hash,
                                },
                                already_owned: binding.already_owned,
                                checked_effective_lock,
                            })
                        """,
                    ),
                ),
                linked_consumers=(
                    CrossToolLinkedConsumerContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="commit_body_pipeline_owner",
                        item_token_sha256="df0d9e50cb6432bf90079e1fddc22c2abd9bf962dd562976969b7c0affed2f88",
                        required_expression="""
                            let BodyPipelineOwnerBindingPlan {
                                key,
                                owner,
                                already_owned: _,
                                checked_effective_lock,
                            } = plan;
                        """,
                        mutation_boundaries=(
                            "self.body_pipeline_owners.insert(key, owner);",
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        token_consumptions=(
                            """
                                let _authorized_ownership =
                                    checked_effective_lock.into_projection();
                            """,
                        ),
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionBodyCapacityRetirementPreservesEffectiveLock",
                verus_theorem=(
                    "production_body_capacity_retirement_preserves_effective_lock"
                ),
                verus_source=_EFFECTIVE_LOCK_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_core/scheduler.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                verus_parameters=(
                    "ready_before: u64, retained_bytes: u64, ready_bytes: u64, "
                    "store_before: u64, store_bytes: u64, "
                    "accounting: ExactBodyRetirementAccountingProjection,"
                ),
                verus_requires="""
                    exact_body_retirement_accounting(
                        ready_before,
                        retained_bytes,
                        ready_bytes,
                        store_before,
                        store_bytes,
                    ) == Some(accounting)
                """,
                verus_ensures="""
                    production_body_capacity_retirement_preserves_effective_lock_kernel(
                        production_body_capacity_retirement_effective_lock_trace(
                            ready_before,
                            retained_bytes,
                            ready_bytes,
                            store_before,
                            store_bytes,
                            accounting,
                        ),
                    ),
                    retained_bytes <= ready_before,
                    ready_bytes <= ready_before - retained_bytes,
                    accounting.ready_after == ready_before - retained_bytes - ready_bytes,
                    store_bytes <= store_before,
                    accounting.store_after == store_before - store_bytes
                """,
                verified_kernel=(
                    "production_body_capacity_retirement_preserves_effective_lock_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: EffectiveLockTraceProjection,"
                ),
                verified_kernel_body=(
                    "effective_lock_trace_claim_body!(projection, 3u8)"
                ),
                theorem_kernel_projection="""
                    production_body_capacity_retirement_effective_lock_trace(
                        ready_before,
                        retained_bytes,
                        ready_bytes,
                        store_before,
                        store_bytes,
                        accounting,
                    ),
                """,
                theorem_projection_builder=(
                    "production_body_capacity_retirement_effective_lock_trace"
                ),
                theorem_projection_builder_parameters=(
                    "ready_before: u64, retained_bytes: u64, ready_bytes: u64, "
                    "store_before: u64, store_bytes: u64, "
                    "accounting: ExactBodyRetirementAccountingProjection,"
                ),
                theorem_projection_builder_return="EffectiveLockTraceProjection",
                theorem_projection_builder_item_sha256=(
                    "a6dffa239c25e8ec4cd99876e43c7cf582b887e30ad6a0d8361dc1cf68f7f12a"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="reconcile_protected_lock",
                        item_token_sha256=(
                            "41b00d09f09c0265f828d53726daea233b3c62c9626137117ef7f0c5f0daf63f"
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        projection="retirement_trace",
                        required_expression="""
                            let accounting = plan_exact_body_retirement_accounting(
                                self.ready_body_bytes,
                                retained_bytes,
                                ready_bytes,
                                self.pending_store_bytes,
                                retired_store_bytes,
                            ).ok_or_else(|| {
                                EffectExecutorError::Contract(
                                    "superseded body byte accounting underflow or leakage"
                                        .to_owned(),
                                )
                            })?;
                            let retirement_trace = EffectiveLockTraceProjection {
                                kind: EFFECTIVE_LOCK_TRACE_RETIRE,
                                relation_exact: plan_exact_body_retirement_accounting(
                                    self.ready_body_bytes,
                                    retained_bytes,
                                    ready_bytes,
                                    self.pending_store_bytes,
                                    retired_store_bytes,
                                ) == Some(accounting),
                                protected_before: 0,
                                protected_after: 0,
                                owner_before: 0,
                                owner_after: 0,
                                owner_reused: false,
                                ready_before: self.ready_body_bytes,
                                retired_retained: retained_bytes,
                                retired_ready: ready_bytes,
                                ready_after: accounting.ready_after,
                                store_before: self.pending_store_bytes,
                                retired_store: retired_store_bytes,
                                store_after: accounting.store_after,
                                cursor_before: 0,
                                completion_ready: false,
                                progress_ready: false,
                                normal_ready: false,
                                selected: 0,
                                cursor_after: 0,
                            };
                            let Some(checked_retirement) =
                                check_production_body_capacity_retirement_effective_lock_transition(
                                    retirement_trace
                                )
                            else {
                                return Err(EffectExecutorError::Contract(
                                    "body retirement did not refine exact effective-lock capacity"
                                        .to_owned(),
                                ));
                            };
                        """,
                        token_consumptions=(
                            """
                                let _authorized_retirement =
                                    checked_retirement.into_projection();
                            """,
                        ),
                        mutation_boundaries=(
                            """
                                self.runtime.retire_body_pipeline_completions(
                                    owner.tag, *round, *subject
                                )
                            """,
                        ),
                    ),
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="plan_certified_view_body_cleanup",
                        item_token_sha256=(
                            "a5fe08d7c00b3873335df4fb485fea37e9791f64112767f56977e1c8b83a53f4"
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        projection="cleanup_trace",
                        required_expression="""
                            let accounting = plan_exact_body_retirement_accounting(
                                self.ready_body_bytes,
                                0,
                                retired_ready_bytes,
                                self.pending_store_bytes,
                                retired_store_bytes,
                            ).ok_or_else(|| {
                                EffectExecutorError::Contract(
                                    "certified-view body cleanup byte accounting underflow or leakage"
                                        .to_owned(),
                                )
                            })?;
                            let cleanup_trace = EffectiveLockTraceProjection {
                                kind: EFFECTIVE_LOCK_TRACE_RETIRE,
                                relation_exact: plan_exact_body_retirement_accounting(
                                    self.ready_body_bytes,
                                    0,
                                    retired_ready_bytes,
                                    self.pending_store_bytes,
                                    retired_store_bytes,
                                ) == Some(accounting),
                                protected_before: 0,
                                protected_after: 0,
                                owner_before: 0,
                                owner_after: 0,
                                owner_reused: false,
                                ready_before: self.ready_body_bytes,
                                retired_retained: 0,
                                retired_ready: retired_ready_bytes,
                                ready_after: accounting.ready_after,
                                store_before: self.pending_store_bytes,
                                retired_store: retired_store_bytes,
                                store_after: accounting.store_after,
                                cursor_before: 0,
                                completion_ready: false,
                                progress_ready: false,
                                normal_ready: false,
                                selected: 0,
                                cursor_after: 0,
                            };
                            let Some(checked_effective_lock) =
                                check_production_body_capacity_retirement_effective_lock_transition(
                                    cleanup_trace
                                )
                            else {
                                return Err(EffectExecutorError::Contract(
                                    "certified-view cleanup did not refine exact effective-lock capacity"
                                        .to_owned(),
                                ));
                            };
                            Ok(CertifiedViewBodyCleanupPlan {
                                stale_stores,
                                stale_ready,
                                protected_ready_rebinds,
                                accounting,
                                checked_effective_lock,
                            })
                        """,
                    ),
                ),
                linked_consumers=(
                    CrossToolLinkedConsumerContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="install_view",
                        item_token_sha256="aead33e5d7df955164b9ff8764dc4aefbe46648b96041d5c056fb4f366d3697d",
                        required_expression="""
                            let stale_body_cleanup =
                                self.plan_certified_view_body_cleanup(
                                    tag, protected_body
                                )?;
                        """,
                        mutation_boundaries=(
                            "services.cancel_body_store(*id).map_err(service_error)?;",
                            """
                                self.ready_body_bytes =
                                    stale_body_cleanup.accounting.ready_after;
                            """,
                            """
                                self.pending_store_bytes =
                                    stale_body_cleanup.accounting.store_after;
                            """,
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        token_consumptions=(
                            """
                                let _authorized_body_cleanup =
                                    stale_body_cleanup.checked_effective_lock
                                        .into_projection();
                            """,
                        ),
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionBodyServiceRefinesAsyncFairness",
                verus_theorem="production_body_service_refines_async_fairness",
                verus_source=_EFFECTIVE_LOCK_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                verus_parameters=(
                    "cursor: u8, completion_ready: bool, progress_ready: bool, "
                    "normal_ready: bool, "
                    "selection: BoundedServiceSelectionProjection,"
                ),
                verus_requires="""
                    cursor >= 1u8,
                    cursor <= 3u8,
                    completion_ready || progress_ready || normal_ready,
                    selection == bounded_service_selection(
                        cursor,
                        completion_ready,
                        progress_ready,
                        normal_ready,
                    )
                """,
                verus_ensures="""
                    production_body_service_refines_async_fairness_kernel(
                        production_body_service_effective_lock_trace(
                            cursor,
                            completion_ready,
                            progress_ready,
                            normal_ready,
                            selection,
                        ),
                    ),
                    selection.selected >= 1u8,
                    selection.selected <= 3u8,
                    selection.next >= 1u8,
                    selection.next <= 3u8,
                    selection.selected == 1u8 ==> completion_ready,
                    selection.selected == 2u8 ==> progress_ready,
                    selection.selected == 3u8 ==> normal_ready
                """,
                verified_kernel=(
                    "production_body_service_refines_async_fairness_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: EffectiveLockTraceProjection,"
                ),
                verified_kernel_body=(
                    "effective_lock_trace_claim_body!(projection, 4u8)"
                ),
                theorem_kernel_projection="""
                    production_body_service_effective_lock_trace(
                        cursor,
                        completion_ready,
                        progress_ready,
                        normal_ready,
                        selection,
                    ),
                """,
                theorem_projection_builder=(
                    "production_body_service_effective_lock_trace"
                ),
                theorem_projection_builder_parameters=(
                    "cursor: u8, completion_ready: bool, progress_ready: bool, "
                    "normal_ready: bool, "
                    "selection: BoundedServiceSelectionProjection,"
                ),
                theorem_projection_builder_return="EffectiveLockTraceProjection",
                theorem_projection_builder_item_sha256=(
                    "56ac93d71cbda0dd6a1b584062ec64db7bb5ae6fc9ddfb822aea1f7e6edcb4c4"
                ),
                source_item_seals=(
                    CrossToolSourceItemSeal(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="step",
                        item_token_sha256="bafd283fd50fe929e000481a8314f98cd0ad3aef30c8e8677a93b0784045136c",
                        brace_context=((
                            "impl", "<", "D", ":", "RuntimeDriver", ">",
                            "SerializedV2Runtime", "<", "D", ">",
                        ),),
                        required_expressions=(
                            """
                            if self
                                .exact_serve_target_ordinal
                                .is_some_and(|target| owner.lifecycle_ordinal() < target)
                            {
                                self.exact_serve_predecessor_retry_attempted = true;
                            }
                            """,
                            """
                            if self
                                .retained_response_predecessor_target_ordinal
                                .is_some_and(|target| owner.lifecycle_ordinal() < target)
                            {
                                self.retained_response_predecessor_retry_attempted = true;
                            }
                            """,
                        ),
                    ),
                    CrossToolSourceItemSeal(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="step_recovery",
                        item_token_sha256=(
                            "818947b3b1356bfe825b34f2b4ee35f8293b24d9a12ef21fdd0d4f5d97c4ef0e"
                        ),
                        brace_context=((
                            "impl", "<", "D", ":", "RuntimeDriver", ">",
                            "SerializedV2Runtime", "<", "D", ">",
                        ),),
                    ),
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="pop_next_with_ownership",
                        item_token_sha256=(
                            "b8ecf17c88e9ef65032956a4dfb65312903342a96a64e7521ac15c99c9a2fbf2"
                        ),
                        brace_context=((
                            "impl", "<", "C", ":", "ExactRuntimeCommandIdentity",
                            ">", "BoundedIngress", "<", "C", ">",
                        ),),
                        projection="service_trace",
                        required_expression="""
                            let queue_before = self.ownership_snapshot();
                            let cursor_before = self.next_class.service_code();
                            if self.oldest_lifecycle_ordinal()?.is_none() {
                                return Ok(None);
                            }
                            let (completion_ready, progress_ready, normal_ready) =
                                self.class_readiness();
                            let selection = select_bounded_service_class(
                                cursor_before,
                                completion_ready,
                                progress_ready,
                                normal_ready,
                            );
                            let service_trace = EffectiveLockTraceProjection {
                                kind: EFFECTIVE_LOCK_TRACE_SERVICE,
                                relation_exact: select_bounded_service_class(
                                    cursor_before,
                                    completion_ready,
                                    progress_ready,
                                    normal_ready,
                                ) == selection,
                                protected_before: 0,
                                protected_after: 0,
                                owner_before: 0,
                                owner_after: 0,
                                owner_reused: false,
                                ready_before: 0,
                                retired_retained: 0,
                                retired_ready: 0,
                                ready_after: 0,
                                store_before: 0,
                                retired_store: 0,
                                store_after: 0,
                                cursor_before,
                                completion_ready,
                                progress_ready,
                                normal_ready,
                                selected: selection.selected,
                                cursor_after: selection.next,
                            };
                            let Some(checked_service) =
                                check_production_body_service_effective_lock_transition(
                                    service_trace
                                )
                            else {
                                panic!(
                                    "Sumeragi v2 bounded service violated the effective-lock trace"
                                );
                            };
                        """,
                        token_consumptions=(
                            """
                                let _authorized_service =
                                    checked_service.into_projection();
                                self.next_class = next;
                                return Ok(None);
                            """,
                            """
                                let _authorized_service =
                                    checked_service.into_projection();
                                self.next_class = next;
                                for skipped_class in [
                            """,
                        ),
                        mutation_boundaries=(
                            """
                                self.next_class = next;
                                return Ok(None);
                            """,
                            """
                                self.next_class = next;
                                for skipped_class in [
                            """,
                            """
                                oldest.eligible_skips = oldest
                                    .eligible_skips
                                    .checked_add(1)
                                    .expect("service debt overflow was preflighted");
                            """,
                            """
                                let command = self.commands
                                    .remove(index)
                                    .expect(
                                        "selected runtime FIFO owner remains present"
                                    );
                            """,
                        ),
                    ),
                ),
            ),
        ),
    ),
    CrossToolObligationContract(
        obligation_id="progress-witness-production-refinement",
        module="SumeragiV2AsyncTemporalClosureProofs",
        ledger_symbol="ProgressWitnessProductionRefinementObligation",
        tla_theorem="ProgressWitnessCrossToolRefinement",
        tla_statement=(
            "ProductionProgressWitnessTraceRefinement "
            "=> ProgressWitnessProductionRefinementObligation"
        ),
        ledger_declaration_kind="operator",
        ledger_statement=(
            "/\\ ProductionProgressWitnessTraceRefinement "
            "/\\ ProgressWitnessObligation"
        ),
        tla_proof=(
            "BY ProgressWitnessObligation "
            "DEF ProgressWitnessProductionRefinementObligation"
        ),
        claims=(
            CrossToolClaimContract(
                constant="ProductionDurableIntentTraceRefinesProgressWitness",
                verus_theorem="production_durable_intent_trace_refines_progress_witness",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2.rs",
                    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
                    "crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionDurableIntentTraceProjection,"
                ),
                verus_requires=(
                    "production_durable_intent_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_durable_intent_trace_refines_progress_witness_kernel(
                        production_durable_intent_trace_projection(projection),
                    ),
                    effect_slots_authorized_body!(projection.effects),
                    effect_count_body!(
                        projection.effects,
                        refinement_tag_value!(EFFECT_PERSIST)
                    ) <= 1u64,
                    projection.durable_sequence_after
                        >= projection.durable_sequence_before,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_BEGIN_WAL)
                        ==> projection.durable_sequence_before < u64::MAX,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_BEGIN_WAL)
                        ==> projection.pending_after.persistence_id
                            == projection.durable_sequence_before + 1,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_BEGIN_WAL)
                        ==> projection.durable_sequence_after
                            == projection.durable_sequence_before,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_ACKNOWLEDGE_WAL)
                        ==> projection.durable_sequence_before < u64::MAX,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_ACKNOWLEDGE_WAL)
                        ==> projection.durable_sequence_after
                            == projection.durable_sequence_before + 1,
                    projection.boundary_claimed.kind
                        == refinement_tag_value!(BOUNDARY_ACKNOWLEDGE_WAL)
                        ==> projection.pending_before.persistence_id
                            == projection.durable_sequence_after
                """,
                verified_kernel=(
                    "production_durable_intent_trace_refines_progress_witness_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionDurableIntentTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_durable_intent_trace_body!(projection)"
                ),
                verified_kernel_const=False,
                verified_kernel_shared_macro_sha256=(
                    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
                    ("production_durable_intent_trace_body", "8bc5f5f07d5d188dc48e6cbc02f0c43d67af2591a8cf5f379e11b39fd0013e4c"),
                    ("pending_projection_is_absent_body", "1ee3ae9d3fa183f21f330af54f681f77c7c54300b6c436f34ff47fca238871ac"),
                    ("pending_projection_equal_body", "76c11e957c8c596fed3caa5b3685c25b1fdd3511b82ebdbb9b12c10cd30577f8"),
                    ("pending_projection_matches_boundary_body", "5cc9a32b9cbe39fa8eb953596b684503b349ea2bb7e7b8a4771465f040478772"),
                    ("pending_round_can_begin_body", "382f5b2b4d7ab9fa32b8c9784e6ac71055c0935904f30e16cf86f31f6a147125"),
                    ("pending_round_can_acknowledge_body", "c0a038824177fa5ea68ab7ceff3af3bd16691395521132dca1e2d51a0ac65b4e"),
                    ("wal_record_proposal_round_is_exact_body", "2d0dcc4843df453a63f1a085b2ae7d515cf655ec6080f5f57bd0d5631948ef22"),
                    ("wal_record_round_matches_owner_body", "9ae760541518b7040a1b85a9399732ea494daaeeac74c686cf07809683c825dc"),
                    ("persist_slot_matches_boundary_body", "b5f99d1ada38d831dd5dc57759d11f02af735c4b5b7a8631f4256f67282a668d"),
                    ("tag_projection_equal_body", "d91ebfd02eabc6ce1b5ba4d26d463aab760936cd64fce0dbadd5f0ce7981fc84"),
                    ("tag_projection_strictly_advances_body", "3671834240e325ebfd10306320d682266254384a6e163059afbbc9b01ba1575c"),
                    ("wal_record_continuation_is_exact_body", "de373bdbfbb8e0583f76c04880d2aac199a931f2d535d780a6826cb5a4c48817"),
                    ("event_can_start_wal_record_body", "6983161a91809ce521aaf1e28ba1f5e1f33823fca5830b1747c08f444d5d0a6e"),
                    ("effect_count_body", "8146967fdc6f0597745f26a1483812e3b03e84b85a9df79a8dfc22ae04874640"),
                    ("effect_slots_authorized_body", "177751a58769f362d74418ab543f7dc5e86042722773af43cead0a68dfe77278"),
                    ("active_effect_slot_body", "f25929e357210020cbe386bf21bcf934df1dd744cbf4fec08cfaed6d6b2bb8f7"),
                    ("inactive_effect_slot_body", "4128eb97be4888d167f474bf0c6e9249b81e4764dce7d3499f9a2a2448f3cf27"),
                    ("capability_key_equal_body", "4ffffdf23d3d44eac78fd7101d565c87234d494324bf188571a3502a4ee9cf3f"),
                    ("capability_key_is_none_body", "9095ad5d42ea25676c97d0ad4900a436ba76d38f8fd0c7c52ab091fe07c25e3b"),
                    ("boundary_capability_equal_body", "c7e989a4f72d02abbdaa5cde20886314cbbbdd8fbf8c2df1666f846cf3c98916"),
                    ("boundary_capability_is_absent_body", "83be3b1b562c52286fdbfe6341901e7b1229386bd4b747aefebf953b592efb43"),
                    ("boundary_identity_is_canonical_body", "49069d7b346a387d959c9d75e57e09cd114252a71033bafe19f300e1add16904"),
                    ("subject_projection_equal_body", "73088430b897bc3f1fe3a5bc6782527aa1243bb42b312c15392f7870764d287f"),
                    ("replay_plan_slot_well_formed_body", "9cb5c2ed24719ef74148515407d80e08154ebe803b0ce86e2d9be37aaef9a68b"),
                    ("replay_plan_well_formed_body", "7f42d9d48cdf49dbdb1233aa26b52d32c18a25456ea30dc32c969b9de2cc34eb"),
                    ("replay_plan_equal_body", "74e1ffcfc77f74d806a8bbb84db9fa11de0ce9d51e4ccd610b3617854df08f68"),
                    ("canonical_identity_equal_body", "f69b194278ecc6d1c17bd77f7e6abc279dd58894cdee3817eed727f6127afff3"),
                    ("canonical_identity_is_typed_body", "8031c3fce9aa31c612f61c4e969ef3709f3494063cf46007634f7e66c2b43f76"),
                    ("canonical_identity_is_zero_body", "659e4ab0b79335d08311a07134239aa7338818f507fb721b71c336fc65a52f6d"),
                ),
                theorem_kernel_projection=(
                    "production_durable_intent_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_durable_intent_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionDurableIntentTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionDurableIntentTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "a684b70395aa064dad90598cdd8ff613da2f5dae6ffc21a67b321a5ba9faa42e"
                ),
                source_item_seals=_DURABLE_INTENT_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source=(
                            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
                        ),
                        item="step",
                        item_token_sha256=(
                            "9c05b735ac51637004b6ae3286716360ae2b341f2f2bdd853688f1ca2a4cb9d4"
                        ),
                        brace_context=(("impl", "Reducer"),),
                        projection="durable_intent_trace,",
                        required_expression="""
                            let durable_intent_trace = ProductionDurableIntentTraceProjection {
                                event_tag: transition.event_tag,
                                owner_tag_before: Self::tag_projection(self.current_tag()),
                                owner_tag_after: Self::tag_projection(next.current_tag()),
                                event_kind: transition.event_kind,
                                event_persistence_id: match &audit_event {
                                    Event::Persisted { id, .. }
                                    | Event::PersistenceFailed { id, .. } => {
                                        id.get()
                                    }
                                    _ => 0,
                                },
                                pending_before: transition.pending_before,
                                pending_after: next.pending_projection(),
                                boundary_claimed: transition.boundary_claimed,
                                boundary_granted: transition.boundary_granted,
                                effects: transition.effects,
                                durable_sequence_before: self.durable.last_id().get(),
                                durable_sequence_after: next.durable.last_id().get(),
                            };
                            if !production_durable_intent_trace_refines_progress_witness_kernel(
                                durable_intent_trace,
                            ) {
                                return Err(ReducerError::RefinementViolation);
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionDecisionTraceRefinesRecoveryWitness",
                verus_theorem="production_decision_trace_refines_recovery_witness",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
                    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                source_item_seals=_DECISION_RECOVERY_SOURCE_ITEM_SEALS,
                verus_parameters=(
                    "projection: ProductionDecisionRecoveryTraceProjection,"
                ),
                verus_requires=(
                    "production_decision_recovery_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_decision_trace_refines_recovery_witness_kernel(
                        production_decision_recovery_trace_projection(projection),
                    ),
                    projection.expected_height > 0u64,
                    projection.state_height <= projection.expected_height,
                    projection.expected_height - projection.state_height <= 1u64,
                    projection.durable_body.height == projection.frozen_height,
                    projection.stage == 1u8,
                    projection.replay_tag.height == projection.owner_tag.height,
                    projection.replay_tag.view == projection.owner_tag.view,
                    projection.replay_tag.generation
                        == projection.owner_tag.generation,
                    projection.manifest_round.view
                        == projection.durable_body.view,
                    projection.durable_body.view
                        == projection.commit_qc.decision.proposal_view
                """,
                verified_kernel=(
                    "production_decision_trace_refines_recovery_witness_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionDecisionRecoveryTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_decision_recovery_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
                    ("production_decision_recovery_trace_body", "8c0db740cb3c62621e42f7ed6fa3db06c2926f11e530d7549b07efda2b5bf64d"),
                ),
                theorem_kernel_projection=(
                    "production_decision_recovery_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_decision_recovery_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionDecisionRecoveryTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionDecisionRecoveryTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "2d8ba74963e56343fb960d0de34d3dafbf0f040500f6f3d762e8db35047f67df"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="verify_pending_kura_apply_parts",
                        item_token_sha256=(
                            "ea3813c240db1a1756e948beaed9279710762fd3505e73325731b82711d727bc"
                        ),
                        projection="recovery_trace",
                        required_expression="""
                            let recovery_trace = evidence
                                .recovery_refinement_projection()
                                .ok_or_else(|| {
                                    mismatch(
                                        "replayed Decision recovery evidence cannot be represented losslessly"
                                    )
                                })?;
                            if !production_decision_trace_refines_recovery_witness_kernel(
                                recovery_trace
                            ) {
                                return Err(mismatch(
                                    "replayed Decision recovery evidence failed the shared exact-identity kernel",
                                ));
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionSchedulerTraceRefinesProtectedOwnership",
                verus_theorem="production_scheduler_trace_refines_protected_ownership",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_core/scheduler.rs",
                    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionSchedulerTraceProjection,"
                ),
                verus_requires="""
                    projection.timeout_due
                        ==> projection.selected == 1u8
                            && projection.fifo_owed_after
                                == projection.fifo_ready,
                    !projection.timeout_due
                        && projection.fifo_ready
                        && projection.fifo_owed_before
                        ==> projection.selected == 3u8
                            && !projection.fifo_owed_after,
                    !projection.timeout_due
                        && !(projection.fifo_ready
                            && projection.fifo_owed_before)
                        && projection.periodic_timer_due
                        ==> projection.selected == 2u8
                            && projection.fifo_owed_after
                                == projection.fifo_ready,
                    !projection.timeout_due
                        && !(projection.fifo_ready
                            && projection.fifo_owed_before)
                        && !projection.periodic_timer_due
                        && projection.fifo_ready
                        ==> projection.selected == 3u8
                            && !projection.fifo_owed_after,
                    !projection.timeout_due
                        && !projection.fifo_ready
                        && !projection.periodic_timer_due
                        ==> projection.selected == 0u8
                            && !projection.fifo_owed_after
                """,
                verus_ensures="""
                    production_scheduler_trace_refines_protected_ownership_kernel(
                        production_scheduler_trace_projection(projection),
                    ),
                    projection.selected <= 3u8,
                    projection.timeout_due ==> projection.selected == 1u8,
                    !projection.timeout_due
                            && !projection.fifo_ready
                            && !projection.periodic_timer_due
                        ==> projection.selected == 0u8
                            && !projection.fifo_owed_after
                """,
                verified_kernel=(
                    "production_scheduler_trace_refines_protected_ownership_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionSchedulerTraceProjection,"
                ),
                verified_kernel_body="production_scheduler_trace_body!(projection)",
                theorem_kernel_projection=(
                    "production_scheduler_trace_projection(projection),"
                ),
                theorem_projection_builder="production_scheduler_trace_projection",
                theorem_projection_builder_parameters=(
                    "projection: ProductionSchedulerTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionSchedulerTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "715290630e15ee9514ad688cf506bba8baafeed6072c274e746a20041ea2695a"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source=(
                            "crates/iroha_core/src/sumeragi/v2_core/scheduler.rs"
                        ),
                        item="select",
                        item_token_sha256=(
                            "a685ec19d5dfb98b747dade9860b2bacbec992c4e98ab43a15e4c7605fd100ed"
                        ),
                        brace_context=(("impl", "ScheduleState"),),
                        projection="scheduler_trace",
                        required_expression="""
                            let scheduler_trace = ProductionSchedulerTraceProjection {
                                fifo_owed_before: self.fifo_owed,
                                timeout_due,
                                periodic_timer_due,
                                fifo_ready,
                                selected: match selected {
                                    ScheduledWork::Timeout => 1,
                                    ScheduledWork::PeriodicTimer => 2,
                                    ScheduledWork::Fifo => 3,
                                    ScheduledWork::Idle => 0,
                                },
                                fifo_owed_after: next.fifo_owed,
                            };
                            if !production_scheduler_trace_refines_protected_ownership_kernel(
                                scheduler_trace
                            ) {
                                panic!(
                                    "Sumeragi v2 scheduler lost the selected progress owner"
                                );
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant=(
                    "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership"
                ),
                verus_theorem=(
                    "production_ingress_identity_and_class_trace_refines_protected_ownership"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/mod.rs",
                    "crates/iroha_core/src/sumeragi/v2.rs",
                    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
                    "crates/iroha_core/src/sumeragi/v2_transport.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionIngressIdentityAndClassTraceProjection,"
                ),
                verus_requires="""
                    projection.incoming_height == projection.stored_height,
                    projection.incoming_view == projection.stored_view,
                    projection.incoming_generation == projection.stored_generation,
                    projection.incoming_class == projection.stored_class,
                    projection.incoming_class >= 1u8,
                    projection.incoming_class <= 3u8,
                    projection.queue_len_before < u64::MAX,
                    projection.queue_len_after
                        == projection.queue_len_before + 1u64,
                    projection.queue_len_after <= projection.queue_capacity
                """,
                verus_ensures="""
                    production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
                        production_ingress_identity_and_class_trace_projection(
                            projection
                        ),
                    ),
                    projection.incoming_height == projection.stored_height,
                    projection.incoming_view == projection.stored_view,
                    projection.incoming_generation == projection.stored_generation,
                    projection.incoming_class == projection.stored_class,
                    projection.queue_len_after > projection.queue_len_before,
                    projection.queue_len_after <= projection.queue_capacity
                """,
                verified_kernel=(
                    "production_ingress_identity_and_class_trace_refines_protected_ownership_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionIngressIdentityAndClassTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_ingress_identity_and_class_trace_body!(projection)"
                ),
                theorem_kernel_projection="""
                    production_ingress_identity_and_class_trace_projection(
                        projection
                    ),
                """,
                theorem_projection_builder=(
                    "production_ingress_identity_and_class_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionIngressIdentityAndClassTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionIngressIdentityAndClassTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "1e4c6297b173a49599e374232166bac651bbad2f05431803173c0f213b3355b3"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="enqueue_classified_command",
                        item_token_sha256=(
                            "5094e5f2a8ece4a1ad598907ede3c54f506d8ca70a00a863ee94c3c2268671e3"
                        ),
                        brace_context=((
                            "impl", "<", "C", ">", "BoundedIngress", "<", "C", ">",
                        ),),
                        projection="ingress_trace,",
                        required_expression="""
                            let incoming_tag = command.tag;
                            let incoming_class = command.class.service_code();
                            let queue_len_before = u64::try_from(self.commands.len())
                                .expect(
                                    "bounded runtime ingress length is representable as u64"
                                );
                            self.commands.push_back(command);
                            let stored = self
                                .commands
                                .back()
                                .expect(
                                    "successful runtime ingress retains the admitted command"
                                );
                            let ingress_trace =
                                ProductionIngressIdentityAndClassTraceProjection {
                                    incoming_height: incoming_tag.height(),
                                    incoming_view: incoming_tag.view(),
                                    incoming_generation: incoming_tag.generation().get(),
                                    incoming_class,
                                    stored_height: stored.tag.height(),
                                    stored_view: stored.tag.view(),
                                    stored_generation: stored.tag.generation().get(),
                                    stored_class: stored.class.service_code(),
                                    queue_len_before,
                                    queue_len_after: u64::try_from(self.commands.len())
                                        .expect(
                                            "bounded runtime ingress length is representable as u64"
                                        ),
                                    queue_capacity: u64::try_from(self.config.capacity)
                                        .expect(
                                            "bounded runtime ingress capacity is representable as u64"
                                        ),
                                };
                            if !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
                                ingress_trace,
                            ) {
                                panic!(
                                    "Sumeragi v2 ingress changed command identity or service class"
                                );
                            }
                        """,
                    ),
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="enqueue_completion_batch",
                        item_token_sha256=(
                            "48acda40f72343b15819a7d152fa093ed858318eb563b75ad33bf637c16b6c5c"
                        ),
                        brace_context=((
                            "impl", "<", "C", ">", "BoundedIngress", "<", "C", ">",
                        ),),
                        projection="ingress_trace,",
                        required_expression="""
                            if commands
                                .iter()
                                .any(|command| command.class != CommandClass::Completion)
                            {
                                return Err(EnqueueError::FailClosed);
                            }
                            if commands.len() > self.remaining_capacity() {
                                return Err(EnqueueError::Full);
                            }
                            let first_ordinal = self
                                .claim_admission_ordinal_range(commands.len())?;
                            if let Some(first_ordinal) = first_ordinal {
                                for (offset, command) in
                                    commands.iter_mut().enumerate()
                                {
                                    let offset = u128::try_from(offset).expect(
                                        "bounded runtime batch length is representable as u128"
                                    );
                                    command.admission_ordinal = Some(
                                        first_ordinal
                                            .checked_add(offset)
                                            .expect(
                                                "admission ordinal range was preflighted"
                                            ),
                                    );
                                }
                            }
                            for command in commands {
                                let incoming_tag = command.tag;
                                let incoming_class = command.class.service_code();
                                let queue_len_before =
                                    u64::try_from(self.commands.len()).expect(
                                        "bounded runtime ingress length is representable as u64"
                                    );
                                self.commands.push_back(command);
                                let stored = self
                                    .commands
                                    .back()
                                    .expect(
                                        "successful runtime batch ingress retains the admitted command"
                                    );
                                let ingress_trace =
                                    ProductionIngressIdentityAndClassTraceProjection {
                                        incoming_height: incoming_tag.height(),
                                        incoming_view: incoming_tag.view(),
                                        incoming_generation:
                                            incoming_tag.generation().get(),
                                        incoming_class,
                                        stored_height: stored.tag.height(),
                                        stored_view: stored.tag.view(),
                                        stored_generation:
                                            stored.tag.generation().get(),
                                        stored_class: stored.class.service_code(),
                                        queue_len_before,
                                        queue_len_after:
                                            u64::try_from(self.commands.len()).expect(
                                                "bounded runtime ingress length is representable as u64"
                                            ),
                                        queue_capacity:
                                            u64::try_from(self.config.capacity).expect(
                                                "bounded runtime ingress capacity is representable as u64"
                                            ),
                                    };
                                if !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
                                    ingress_trace,
                                ) {
                                    panic!(
                                        "Sumeragi v2 batch ingress changed command identity or service class"
                                    );
                                }
                            }
                        """,
                    ),
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_runtime.rs",
                        item="commit_canonical_body_available",
                        item_token_sha256=(
                            "cc6c3d153bfb275d531aa4064a357849720066c2d14eab36404036dba7853f74"
                        ),
                        brace_context=((
                            "impl", "BoundedIngress", "<", "AdapterCommand", ">",
                        ),),
                        projection="ingress_trace,",
                        required_expression="""
                            if !reservation.owns_new_slot() {
                                return Ok(());
                            }
                            if self.reserved_body_available.as_ref()
                                != Some(&reservation)
                            {
                                return Err(EnqueueError::FailClosed);
                            }
                            let mut command = TaggedCommand::new(
                                reservation.tag(),
                                CommandClass::Completion,
                                AdapterCommand::BodyAvailable {
                                    manifest: reservation.manifest.clone(),
                                },
                                Instant::now(),
                            );
                            command.admission_ordinal = reservation.admission_ordinal;
                            command.lifecycle_ordinal = reservation.lifecycle_ordinal;
                            command.causal_origin = reservation
                                .causal_origin
                                .clone()
                                .expect("new body reservation retains its causal root");
                            let incoming_tag = command.tag;
                            let incoming_class = command.class.service_code();
                            let retained_len = self
                                .commands
                                .iter()
                                .filter(|queued| {
                                    !queued
                                        .command
                                        .is_authenticated_proposal_conflicting_with(
                                            reservation.manifest(),
                                        )
                                })
                                .count();
                            let queue_len_before = u64::try_from(retained_len)
                                .expect(
                                    "bounded runtime ingress length is representable as u64"
                                );
                            let queue_len_after = queue_len_before
                                .checked_add(1)
                                .expect(
                                    "bounded runtime ingress length cannot overflow u64"
                                );
                            let ingress_trace =
                                ProductionIngressIdentityAndClassTraceProjection {
                                    incoming_height: incoming_tag.height(),
                                    incoming_view: incoming_tag.view(),
                                    incoming_generation: incoming_tag.generation().get(),
                                    incoming_class,
                                    stored_height: command.tag.height(),
                                    stored_view: command.tag.view(),
                                    stored_generation: command.tag.generation().get(),
                                    stored_class: command.class.service_code(),
                                    queue_len_before,
                                    queue_len_after,
                                    queue_capacity: u64::try_from(self.config.capacity)
                                        .expect(
                                            "bounded runtime ingress capacity is representable as u64"
                                        ),
                                };
                            let checked_transition =
                                check_production_ingress_transition(ingress_trace)
                                    .expect(
                                        "Sumeragi v2 canonical body prospective ingress must pass its gate"
                                    );
                            let _authorized_transition =
                                checked_transition.into_projection();
                            self.reserved_body_available = None;
                            self.discard_proposals_conflicting_with(
                                reservation.manifest()
                            );
                            self.commands.push_back(command);
                            Ok(())
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant=(
                    "ProductionTwoStageRelayRetryTraceRefinesSourceFairness"
                ),
                verus_theorem=(
                    "production_two_stage_relay_retry_trace_refines_source_fairness"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/irohad/src/main.rs",
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                    "crates/iroha_p2p/src/network.rs",
                ),
                verus_parameters=(
                    "projection: ProductionTwoStageRelayRetryTraceProjection,"
                ),
                verus_requires=(
                    "production_two_stage_relay_retry_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_two_stage_relay_retry_trace_refines_source_fairness_kernel(
                        production_two_stage_relay_retry_trace_projection(projection),
                    ),
                    projection.daemon_source_capacity_matches_two_upstream_lanes,
                    projection.class_corridor_covers_authenticated_sources,
                    projection.total_depth_after == projection.total_depth_before,
                    projection.selected_source_rank_after
                        == projection.ready_sources_after - 1u64,
                    projection.selected_item_rank_after
                        == projection.source_depth_after - 1u64,
                    projection.source_depth_after <= projection.source_capacity
                """,
                verified_kernel=(
                    "production_two_stage_relay_retry_trace_refines_source_fairness_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionTwoStageRelayRetryTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_two_stage_relay_retry_trace_body!(projection)"
                ),
                verified_kernel_public=True,
                verified_kernel_shared_macro_sha256=((
                    "production_two_stage_relay_retry_trace_body",
                    "fda81d366704ada4700101cb7ee870acfec2fabe141133ece61c5d9eb1d84ea9",
                ),),
                theorem_kernel_projection="""
                    production_two_stage_relay_retry_trace_projection(projection),
                """,
                theorem_projection_builder=(
                    "production_two_stage_relay_retry_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionTwoStageRelayRetryTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionTwoStageRelayRetryTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "e5fe2d9fe2e38d53d806554e2d8f66c3d83c39ca69ff40afc6262059cbb34ca7"
                ),
                source_item_seals=_TWO_STAGE_RELAY_RETRY_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/irohad/src/main.rs",
                        item="sumeragi_relay_retain_retry",
                        item_token_sha256=(
                            "4eaa732c6b69e6c455ac7bef64be8b0c76425c70bb5ce5138fb1fa4f063395c1"
                        ),
                        projection="projection",
                        required_expression="""
                            let projection = ProductionTwoStageRelayRetryTraceProjection {
                                daemon_source_capacity_matches_two_upstream_lanes:
                                    retry_geometry
                                        .daemon_source_capacity_matches_two_upstream_lanes(),
                                class_corridor_covers_authenticated_sources:
                                    retry_geometry
                                        .class_corridor_covers_authenticated_sources(),
                                authenticated_source_matches_resource_owner:
                                    selection.source == retry_source
                                        && retry_route.is_authenticated_via(
                                            &selection.source.via
                                        ),
                                retry_route_same_delivery:
                                    selected_item_rank_after.is_some(),
                                retry_route_active,
                                selected_eligible: selection.selected_eligible,
                                ready_sources_before:
                                    u64::try_from(selection.ready_sources_before)
                                        .expect(
                                            "retained ready-source count must fit u64"
                                        ),
                                selected_source_rank_before:
                                    u64::try_from(
                                        selection.selected_source_rank_before
                                    )
                                    .expect("retained source rank must fit u64"),
                                ready_sources_after:
                                    u64::try_from(retained.ready.len())
                                        .expect(
                                            "retained ready-source count must fit u64"
                                        ),
                                selected_source_rank_after:
                                    selected_source_rank_after
                                        .and_then(|rank| u64::try_from(rank).ok())
                                        .unwrap_or(u64::MAX),
                                source_depth_before:
                                    u64::try_from(selection.source_depth_before)
                                        .expect(
                                            "retained source depth must fit u64"
                                        ),
                                selected_item_rank_before:
                                    u64::try_from(
                                        selection.selected_item_rank_before
                                    )
                                    .expect("retained item rank must fit u64"),
                                source_depth_after: source_depth_after
                                    .and_then(|depth| u64::try_from(depth).ok())
                                    .unwrap_or(u64::MAX),
                                selected_item_rank_after:
                                    selected_item_rank_after
                                        .and_then(|rank| u64::try_from(rank).ok())
                                        .unwrap_or(u64::MAX),
                                total_depth_before:
                                    u64::try_from(selection.total_depth_before)
                                        .expect(
                                            "retained total depth must fit u64"
                                        ),
                                total_depth_after: u64::try_from(retained.len)
                                    .expect("retained total depth must fit u64"),
                                source_capacity:
                                    u64::try_from(retained.source_capacity)
                                        .expect(
                                            "retained source capacity must fit u64"
                                        ),
                                total_capacity: u64::try_from(retained.capacity)
                                    .expect(
                                        "retained total capacity must fit u64"
                                    ),
                            };
                            if production_two_stage_relay_retry_trace_refines_source_fairness_kernel(
                                projection
                            ) {
                                Ok(())
                            } else {
                                Err(
                                    SumeragiRelayRetryRetentionError::RefinementViolation
                                )
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionReliableFlushTraceRefinesOutboundOwnership",
                verus_theorem="production_reliable_flush_trace_refines_outbound_ownership",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/merge_sidecar.rs",
                    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                    "crates/iroha_core/src/sumeragi/v2_transport.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                    "crates/iroha_p2p/src/network.rs",
                    "crates/iroha_p2p/src/peer.rs",
                ),
                source_item_seals=_RELIABLE_FLUSH_SOURCE_ITEM_SEALS,
                verus_parameters="""
                    worker: ProductionReliableFlushTraceProjection,
                    application: ProductionReliableFlushApplicationProjection,
                """,
                verus_requires="""
                    production_reliable_flush_trace_body!(worker),
                    production_reliable_flush_application_body!(application),
                    production_reliable_flush_two_phase_link_body!(worker, application)
                """,
                verus_ensures="""
                    production_reliable_flush_trace_refines_outbound_ownership_kernel(
                        production_reliable_flush_trace_projection(worker),
                    ),
                    production_reliable_flush_application_refines_source_lane_kernel(
                        production_reliable_flush_application_projection(application),
                    ),
                    production_reliable_flush_two_phase_link_kernel(
                        production_reliable_flush_trace_projection(worker),
                        production_reliable_flush_application_projection(application),
                    ),
                    worker.status == 2u8,
                    worker.stream_epoch > 0u64,
                    application.stream_epoch > 0u64,
                    application.marker_stream_epoch > 0u64,
                    worker.stream_epoch == application.stream_epoch,
                    worker.stream_epoch == application.marker_stream_epoch,
                    application.stream_epoch == application.marker_stream_epoch,
                    worker.service_generation > 0u64,
                    application.service_generation > 0u64,
                    application.marker_service_generation > 0u64,
                    worker.service_generation == application.service_generation,
                    worker.service_generation
                        == application.marker_service_generation,
                    application.service_generation
                        == application.marker_service_generation,
                    worker.semantic_sequence > 0u64,
                    application.semantic_sequence > 0u64,
                    application.marker_semantic_sequence > 0u64,
                    worker.semantic_sequence == application.semantic_sequence,
                    worker.semantic_sequence
                        == application.marker_semantic_sequence,
                    application.semantic_sequence
                        == application.marker_semantic_sequence,
                    worker.reply_writer_timeout_attempt
                        == application.reply_writer_timeout_attempt,
                    application.claim_acquired,
                    application.gate_marker_present_before,
                    !application.gate_marker_present_after,
                    application.gate_cursor_after == application.gate_cursor_before + 1u64,
                    application.chunk_cursor_after == application.gate_cursor_after,
                    application.sibling_records_equal,
                    canonical_identity_equal_body!(
                        application.sibling_state_before,
                        application.sibling_state_after
                    ),
                    application.outbound_order_count_after <= 1u64,
                    application.sibling_order_len_after == application.sibling_order_len_before,
                    canonical_identity_equal_body!(
                        worker.source_key_identity,
                        application.source_key_identity
                    ),
                    canonical_identity_equal_body!(
                        worker.delivery_route_identity,
                        application.delivery_route_identity
                    ),
                    canonical_identity_equal_body!(
                        worker.writer_occurrence_identity,
                        application.writer_occurrence_identity
                    ),
                    canonical_identity_equal_body!(worker.chunk_hash, application.chunk_hash)
                """,
                verified_kernel=(
                    "production_reliable_flush_trace_refines_outbound_ownership_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionReliableFlushTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_reliable_flush_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
                    ("production_reliable_flush_trace_body", "7b830dafccef15b33a2966e57e2fccd18c6f6ce3e454831cb7378d12ae2db1d5"),
                ),
                theorem_kernel_projection=(
                    "production_reliable_flush_trace_projection(worker),"
                ),
                theorem_projection_builder=(
                    "production_reliable_flush_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionReliableFlushTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionReliableFlushTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "00420917e434001952041f2ba02d2fc20d2da2c495aea3fa073104c1b4b878a5"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_worker.rs",
                        item="poll_reply_flushes",
                        item_token_sha256=(
                            "eae8ee4dc4996b077b9d0e3315e96e8c35a18b0189f2add40e898e60a4167749"
                        ),
                        brace_context=(("impl", "PendingExactOutput"),),
                        projection="flush_trace",
                        required_expression="""
                            let flush_trace = match reliable_flush_trace_projection(
                                admission,
                                status,
                                flushing_before,
                                flushing_after,
                                admitted_before,
                                admitted_after,
                                self.sidecar_admission_capacity,
                            ) {
                                Ok(flush_trace) => flush_trace,
                                Err(error) => {
                                    let error = error.to_string();
                                    self.restore_pending_flush(
                                        fanout_index,
                                        target_index,
                                        pending_flush
                                    )?;
                                    return Err(error);
                                }
                            };
                            if !production_reliable_flush_trace_refines_outbound_ownership_kernel(
                                flush_trace
                            ) {
                                self.restore_pending_flush(
                                    fanout_index,
                                    target_index,
                                    pending_flush
                                )?;
                                return Err(MergeSidecarError::FlushIdentityMismatch(
                                    "sidecar flush transition failed its exact ownership kernel",
                                )
                                .to_string());
                            }
                        """,
                    ),
                ),
                supplemental_kernels=(
                    CrossToolSupplementalKernelContract(
                        verified_kernel=(
                            "production_reliable_flush_application_refines_source_lane_kernel"
                        ),
                        verified_kernel_source=(
                            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                        ),
                        verified_kernel_parameters=(
                            "projection: ProductionReliableFlushApplicationProjection,"
                        ),
                        verified_kernel_body=(
                            "production_reliable_flush_application_body!(projection)"
                        ),
                        verified_kernel_shared_macro_sha256=(
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
                            ("production_reliable_flush_application_body", "49559bef138c5b61161d45ca264e50a35e4c6f48d76d517b289b4f5e6fa64b80"),
                        ),
                        theorem_kernel_projection=(
                            "production_reliable_flush_application_projection(application),"
                        ),
                        theorem_projection_builders=(
                            CrossToolProjectionBuilderContract(
                                name=(
                                    "production_reliable_flush_application_projection"
                                ),
                                parameters=(
                                    "projection: ProductionReliableFlushApplicationProjection,"
                                ),
                                return_type=(
                                    "ProductionReliableFlushApplicationProjection"
                                ),
                                item_token_sha256=(
                                    "960617b4624978ca78a7790e71d236420889b5e22d4f4ce002ba45ae4e6dbdf7"
                                ),
                            ),
                        ),
                        production_call_sites=(
                            CrossToolProductionCallContract(
                                source=(
                                    "crates/iroha_core/src/merge_sidecar.rs"
                                ),
                                item="acknowledge_outbound_chunk",
                                brace_context=(("impl", "MergeSidecarTransport"),),
                                projection="application",
                                required_expression="""
                                    if !production_reliable_flush_application_refines_source_lane_kernel(
                                        application
                                    ) {
                                        return Err(MergeSidecarError::FlushIdentityMismatch(
                                            "writer flush application violated the source-lane refinement",
                                        ));
                                    }
                                """,
                                item_token_sha256=(
                                    "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484"
                                ),
                            ),
                        ),
                    ),
                    CrossToolSupplementalKernelContract(
                        verified_kernel=(
                            "production_reliable_flush_two_phase_link_kernel"
                        ),
                        verified_kernel_source=(
                            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                        ),
                        verified_kernel_parameters="""
                            worker: ProductionReliableFlushTraceProjection,
                            application: ProductionReliableFlushApplicationProjection,
                        """,
                        verified_kernel_body=(
                            "production_reliable_flush_two_phase_link_body!(worker, application)"
                        ),
                        verified_kernel_shared_macro_sha256=(
                            ("production_reliable_flush_two_phase_link_body", "b165517bbaf656b9136ab8e10c9c909e53ec68fff3dc2116ce4a1a2eade84da6"),
                        ),
                        theorem_kernel_projection="""
                            production_reliable_flush_trace_projection(worker),
                            production_reliable_flush_application_projection(application),
                        """,
                        theorem_projection_builders=(
                            CrossToolProjectionBuilderContract(
                                name="production_reliable_flush_trace_projection",
                                parameters=(
                                    "projection: ProductionReliableFlushTraceProjection,"
                                ),
                                return_type=(
                                    "ProductionReliableFlushTraceProjection"
                                ),
                                item_token_sha256=(
                                    "00420917e434001952041f2ba02d2fc20d2da2c495aea3fa073104c1b4b878a5"
                                ),
                            ),
                            CrossToolProjectionBuilderContract(
                                name=(
                                    "production_reliable_flush_application_projection"
                                ),
                                parameters=(
                                    "projection: ProductionReliableFlushApplicationProjection,"
                                ),
                                return_type=(
                                    "ProductionReliableFlushApplicationProjection"
                                ),
                                item_token_sha256=(
                                    "960617b4624978ca78a7790e71d236420889b5e22d4f4ce002ba45ae4e6dbdf7"
                                ),
                            ),
                        ),
                        production_call_sites=(
                            CrossToolProductionCallContract(
                                source=(
                                    "crates/iroha_core/src/merge_sidecar.rs"
                                ),
                                item="acknowledge_outbound_chunk",
                                brace_context=(("impl", "MergeSidecarTransport"),),
                                projection="worker_trace, application",
                                required_expression="""
                                    if !production_reliable_flush_two_phase_link_kernel(
                                        worker_trace,
                                        application
                                    ) {
                                        return Err(MergeSidecarError::FlushIdentityMismatch(
                                            "writer flush application disconnected from its accepted worker transition",
                                        ));
                                    }
                                """,
                                item_token_sha256=(
                                    "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484"
                                ),
                            ),
                        ),
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionApplicationTraceRefinesDecisionCompletion",
                verus_theorem="production_application_trace_refines_decision_completion",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_apply.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                source_item_seals=_APPLICATION_SOURCE_ITEM_SEALS,
                verus_parameters=(
                    "projection: ProductionApplicationTraceProjection,"
                ),
                verus_requires=(
                    "production_application_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_application_trace_refines_decision_completion_kernel(
                        production_application_trace_projection(projection),
                    ),
                    projection.context_height > 0u64,
                    projection.state_height_after == projection.context_height,
                    projection.artifact_height == projection.context_height,
                    projection.completion_work_id == projection.task_work_id,
                    canonical_identity_equal_body!(
                        projection.artifact_context_id,
                        projection.context_id
                    ),
                    projection.task_tag.height == projection.owner_tag.height,
                    projection.task_tag.view == projection.owner_tag.view,
                    projection.task_tag.generation
                        == projection.owner_tag.generation,
                    projection.validated_body.view
                        == projection.commit_qc.decision.proposal_view
                """,
                verified_kernel=(
                    "production_application_trace_refines_decision_completion_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionApplicationTraceProjection,"
                ),
                verified_kernel_body="production_application_trace_body!(projection)",
                verified_kernel_shared_macro_sha256=(
    ("refinement_tag_value", "db6d42572257edf02ae97e143c4270d2ade85c45bce4e11c46232dd12a47d49c"),
                    ("production_application_trace_body", "c2a02aabdd24ec9d7a2f23d57cf0cf59514e1c6928105dd5d3617d47a55dc719"),
                ),
                theorem_kernel_projection=(
                    "production_application_trace_projection(projection),"
                ),
                theorem_projection_builder="production_application_trace_projection",
                theorem_projection_builder_parameters=(
                    "projection: ProductionApplicationTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionApplicationTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "940dcb91056a03a04c3033576b8fbbcf442b945fd89da431dcf0efaeb42b1fd5"
                ),
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_apply.rs",
                        item="finish_durable_apply_completion",
                        item_token_sha256=(
                            "c2aeba7048889670718af0ecb8fba4a9c6da634a8a721ed4142ef343f416429a"
                        ),
                        brace_context=(("impl", "V2ApplyService"),),
                        projection="application_trace",
                        required_expression="""
                            let application_trace = evidence
                                .application_refinement_projection()
                                .ok_or_else(|| {
                                    V2ApplyError::committed_recovery_required(
                                        "application refinement evidence",
                                        &"native application identity cannot be represented losslessly",
                                    )
                                })?;
                            if !production_application_trace_refines_decision_completion_kernel(
                                application_trace
                            ) {
                                return Err(V2ApplyError::committed_recovery_required(
                                    "application refinement evidence",
                                    &"durable application does not refine its Decision completion",
                                ));
                            }
                        """,
                    ),
                ),
            ),
        ),
    ),
    CrossToolObligationContract(
        obligation_id=(
            "successor-activation-exact-recovery-production-refinement"
        ),
        module="SumeragiV2ChainEpochRefinement",
        ledger_symbol=(
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
        ),
        tla_theorem=(
            "SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement"
        ),
        tla_statement=(
            "ProductionSuccessorAndExactRecoveryTraceRefinement => "
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
        ),
        ledger_declaration_kind="operator",
        ledger_statement=(
            "/\\ ProductionSuccessorAndExactRecoveryTraceRefinement "
            "/\\ (IndexedChainSpec => []"
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)"
        ),
        tla_proof=(
            "BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant "
            "DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
        ),
        claims=(
            CrossToolClaimContract(
                constant="ProductionAppliedSuccessorTraceRefinesIndexedActivation",
                verus_theorem="production_applied_successor_trace_refines_indexed_activation",
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/status.rs",
                    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionAppliedSuccessorTraceProjection,"
                ),
                verus_requires=(
                    "production_applied_successor_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_applied_successor_trace_refines_indexed_activation_kernel(
                        production_applied_successor_trace_projection(projection),
                    ),
                    projection.predecessor_stage_before
                        == refinement_tag_value!(SUCCESSOR_STAGE_RUNNING),
                    projection.predecessor_stage_after
                        == refinement_tag_value!(SUCCESSOR_STAGE_COMPLETE),
                    projection.successor.height
                        == projection.binding.expected_predecessor.height + 1u64,
                    projection.successor.marker_height == projection.successor.height
                """,
                verified_kernel=(
                    "production_applied_successor_trace_refines_indexed_activation_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionAppliedSuccessorTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_applied_successor_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _SUCCESSOR_APPLIED_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_applied_successor_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_applied_successor_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionAppliedSuccessorTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionAppliedSuccessorTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "1beed7d15640a11bbb8036fbfdd7950cce576ad24c2d639e99e0894971d74cd1"
                ),
                source_item_seals=_SUCCESSOR_APPLIED_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/status.rs",
                        item="activate_v2_successor_height_at",
                        item_token_sha256=(
                            "987944ae9b1aa0e75b7647317456b76388830e9f20ab8d6d63e8b76f540be914"
                        ),
                        projection="trace",
                        required_expression="""
                            if !production_applied_successor_trace_refines_indexed_activation_kernel(
                                trace
                            ) {
                                return Err(V2SuccessorActivationError::RefinementRejected);
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionRecoveredSuccessorTraceRefinesIndexedActivation",
                verus_theorem=(
                    "production_recovered_successor_trace_refines_indexed_activation"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/status.rs",
                    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionRecoveredSuccessorTraceProjection,"
                ),
                verus_requires=(
                    "production_recovered_successor_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_recovered_successor_trace_refines_indexed_activation_kernel(
                        production_recovered_successor_trace_projection(projection),
                    ),
                    projection.published_status_height_before == 0u64,
                    projection.successor.last_committed_height < u64::MAX,
                    projection.successor.height
                        == projection.successor.last_committed_height + 1u64,
                    projection.authority_kind
                            == refinement_tag_value!(
                                SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP
                            )
                        || projection.authority_kind
                            == refinement_tag_value!(
                                SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP
                            )
                """,
                verified_kernel=(
                    "production_recovered_successor_trace_refines_indexed_activation_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionRecoveredSuccessorTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_recovered_successor_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _SUCCESSOR_RECOVERED_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_recovered_successor_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_recovered_successor_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionRecoveredSuccessorTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionRecoveredSuccessorTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "60e59c06af0520d9b69ff7abb10209f3a1377a905eed3e67fc08a029258f229d"
                ),
                source_item_seals=_SUCCESSOR_RECOVERED_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/status.rs",
                        item="publish_recovered_v2_successor_height_at",
                        item_token_sha256=(
                            "4d4f6ae501991b5bdbc9e131f6f0d6b7db6547fbc2f7f644300658deb2d53650"
                        ),
                        projection="trace",
                        required_expression="""
                            if !production_recovered_successor_trace_refines_indexed_activation_kernel(
                                trace
                            ) {
                                if let Some(published) = published {
                                    return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(
                                        published.height,
                                    ));
                                }
                                return Err(V2SuccessorActivationError::RefinementRejected);
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant=(
                    "ProductionStartupFailureAndRestartRefinesIndexedLifecycle"
                ),
                verus_theorem=(
                    "production_startup_failure_and_restart_refines_indexed_lifecycle"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/status.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
                    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
                ),
                verus_parameters=(
                    "projection: ProductionSuccessorStartupLifecycleProjection,"
                ),
                verus_requires=(
                    "production_startup_failure_and_restart_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(
                        production_successor_startup_lifecycle_projection(projection),
                    ),
                    projection.status_height > 0u64,
                    projection.published_height_after
                        == projection.published_height_before,
                    projection.transition_kind
                            == refinement_tag_value!(SUCCESSOR_LIFECYCLE_FAIL)
                        ==> projection.stage_after == projection.stage_before
                            && projection.restart_required_after,
                    projection.transition_kind
                            != refinement_tag_value!(SUCCESSOR_LIFECYCLE_FAIL)
                        ==> !projection.restart_required_after
                """,
                verified_kernel=(
                    "production_startup_failure_and_restart_refines_indexed_lifecycle_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionSuccessorStartupLifecycleProjection,"
                ),
                verified_kernel_body=(
                    "production_startup_failure_and_restart_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _SUCCESSOR_LIFECYCLE_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_successor_startup_lifecycle_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_successor_startup_lifecycle_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionSuccessorStartupLifecycleProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionSuccessorStartupLifecycleProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "79c4f19220ed57c22fe661405bd7137c64f4771aaef3bcb2076042c81a7804b2"
                ),
                source_item_seals=_SUCCESSOR_LIFECYCLE_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/status.rs",
                        item="begin_v2_successor_activation",
                        item_token_sha256=(
                            "f95cef32673e4198e82e345d2c65013bfbd4ce2b805d23577c1a1d4e095f8c2a"
                        ),
                        projection="lifecycle",
                        required_expression="""
                            let Some(checked_lifecycle) =
                                check_production_successor_startup_lifecycle_transition(lifecycle)
                            else {
                                return Err(V2SuccessorActivationError::RefinementRejected);
                            };
                            let _authorized_lifecycle = checked_lifecycle.into_projection();
                        """,
                    ),
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/status.rs",
                        item="mark_v2_restart_required",
                        item_token_sha256=(
                            "9b5397ef6ec9c1ec10afcd09f165cca3b490745fa5116bbcc987195d8f665164"
                        ),
                        projection="lifecycle",
                        required_expression="""
                            let Some(checked_lifecycle) =
                                check_production_successor_startup_lifecycle_transition(lifecycle)
                            else {
                                iroha_logger::error!(
                                    height = status.height,
                                    "Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"
                                );
                                return;
                            };
                            let _authorized_lifecycle = checked_lifecycle.into_projection();
                        """,
                    ),
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
                        item="recovered",
                        item_token_sha256=(
                            "a99d3aec22c01501fabb4e6b90526ae066b6728ab78043301476653432fac5fd"
                        ),
                        brace_context=(("impl", "PendingSuccessorActivation"),),
                        projection="lifecycle",
                        required_expression="""
                            let Some(checked_lifecycle) =
                                check_production_successor_startup_lifecycle_transition(lifecycle)
                            else {
                                return Err(V2RunnerError::SuccessorRefinementRejected);
                            };
                            let _authorized_lifecycle = checked_lifecycle.into_projection();
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionHistoricalCertificateTraceRefinesIndexedAsync",
                verus_theorem=(
                    "production_historical_certificate_trace_refines_indexed_async"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionHistoricalCertificateTraceProjection,"
                ),
                verus_requires=(
                    "production_historical_certificate_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_historical_certificate_trace_refines_indexed_async_kernel(
                        production_historical_certificate_trace_projection(projection),
                    ),
                    projection.context_height > 0u64,
                    projection.certificate_height == projection.context_height,
                    projection.request_present_before,
                    !projection.request_present_after,
                    canonical_identity_equal_body!(
                        projection.message_hash,
                        projection.admitted_message_hash
                    )
                """,
                verified_kernel=(
                    "production_historical_certificate_trace_refines_indexed_async_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionHistoricalCertificateTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_historical_certificate_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _HISTORICAL_CERTIFICATE_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_historical_certificate_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_historical_certificate_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionHistoricalCertificateTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionHistoricalCertificateTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "9964ce8c6d7e99de1cdec79061d4f5ce8edb58dbd00a94495f35fa61c4a9473a"
                ),
                source_item_seals=_HISTORICAL_CERTIFICATE_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_block_sync.rs",
                        item="enqueue_and_complete",
                        item_token_sha256=(
                            "af3ec8e7240f6406ed88556fa35eea30250a57aaba10b503bf9ddf87f8a07575"
                        ),
                        brace_context=(("impl", "V2BlockSyncDiscovery"),),
                        projection="historical_trace",
                        required_expression="""
                            if !production_historical_certificate_trace_refines_indexed_async_kernel(
                                historical_trace
                            ) {
                                return Err(
                                    CommitCertificateAdmissionError::RefinementRejected
                                );
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant="ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync",
                verus_theorem=(
                    "production_historical_body_pipeline_trace_refines_indexed_async"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_apply.rs",
                    "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
                    "crates/iroha_core/src/sumeragi/v2_body_store.rs",
                    "crates/iroha_core/src/sumeragi/v2_effects.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner.rs",
                    "crates/iroha_core/src/sumeragi/v2_worker.rs",
                ),
                verus_parameters=(
                    "projection: ProductionHistoricalBodyPipelineTraceProjection,"
                ),
                verus_requires=(
                    "production_historical_body_pipeline_trace_body!(projection)"
                ),
                verus_ensures="""
                    production_historical_body_pipeline_trace_refines_indexed_async_kernel(
                        production_historical_body_pipeline_trace_projection(projection),
                    ),
                    projection.owner_present_after,
                    projection.owner_tag.height == projection.fetch_tag.height,
                    projection.owner_tag.view == projection.fetch_tag.view,
                    projection.owner_tag.generation == projection.fetch_tag.generation,
                    !projection.pending_fetch_present_after,
                    !projection.request_present_after
                """,
                verified_kernel=(
                    "production_historical_body_pipeline_trace_refines_indexed_async_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionHistoricalBodyPipelineTraceProjection,"
                ),
                verified_kernel_body=(
                    "production_historical_body_pipeline_trace_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _HISTORICAL_BODY_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_historical_body_pipeline_trace_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_historical_body_pipeline_trace_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionHistoricalBodyPipelineTraceProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionHistoricalBodyPipelineTraceProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "dd77408f8049394320567884ec23eb0ddbe19d6c64aba492b5a25e8442f106e5"
                ),
                source_item_seals=_HISTORICAL_BODY_SOURCE_ITEM_SEALS,
                production_call_sites=(
                    CrossToolProductionCallContract(
                        source="crates/iroha_core/src/sumeragi/v2_effects.rs",
                        item="accept_certified_body_response_inner",
                        item_token_sha256=(
                            "bb0c8b87aa2efd09eebf75ed149d55ae44d974cd0aa24a8f8c15ca83b4da44f4"
                        ),
                        brace_context=((
                            "impl", "<", "R", ":", "EffectRuntime", ">",
                            "V2EffectExecutor", "<", "R", ">",
                        ),),
                        projection="historical_trace",
                        required_expression="""
                            if !production_historical_body_pipeline_trace_refines_indexed_async_kernel(
                                historical_trace
                            ) {
                                return Err(self.fail_closed_transport(
                                    "certified body admission did not preserve its exact historical pipeline owner",
                                    services,
                                ));
                            }
                        """,
                    ),
                ),
            ),
            CrossToolClaimContract(
                constant=(
                    "ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal"
                ),
                verus_theorem=(
                    "production_terminal_application_without_successor_activation_refines_indexed_terminal"
                ),
                verus_source=_GENERAL_VERUS_SOURCE,
                production_sources=(
                    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
                    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
                ),
                verus_parameters=(
                    "projection: ProductionTerminalApplicationWithoutSuccessorActivationProjection,"
                ),
                verus_requires=(
                    "production_terminal_application_without_successor_activation_body!(projection)"
                ),
                verus_ensures="""
                    production_terminal_application_without_successor_activation_kernel(
                        production_terminal_application_without_successor_activation_projection(projection),
                    ),
                    projection.context_height > 0u64,
                    projection.receipt_height == projection.context_height,
                    projection.artifact_height == projection.context_height,
                    projection.predecessor.height == projection.context_height,
                    !projection.pending_successor_activation_present,
                    canonical_identity_equal_body!(
                        projection.receipt_context_id,
                        projection.context_id
                    ),
                    canonical_identity_equal_body!(
                        projection.artifact_context_id,
                        projection.context_id
                    )
                """,
                verified_kernel=(
                    "production_terminal_application_without_successor_activation_kernel"
                ),
                verified_kernel_source=(
                    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
                ),
                verified_kernel_parameters=(
                    "projection: ProductionTerminalApplicationWithoutSuccessorActivationProjection,"
                ),
                verified_kernel_body=(
                    "production_terminal_application_without_successor_activation_body!(projection)"
                ),
                verified_kernel_shared_macro_sha256=(
                    _SUCCESSOR_TERMINAL_APPLICATION_SHARED_MACROS
                ),
                theorem_kernel_projection=(
                    "production_terminal_application_without_successor_activation_projection(projection),"
                ),
                theorem_projection_builder=(
                    "production_terminal_application_without_successor_activation_projection"
                ),
                theorem_projection_builder_parameters=(
                    "projection: ProductionTerminalApplicationWithoutSuccessorActivationProjection,"
                ),
                theorem_projection_builder_return=(
                    "ProductionTerminalApplicationWithoutSuccessorActivationProjection"
                ),
                theorem_projection_builder_item_sha256=(
                    "2a68c7a3cec5cc3b865c079701908d5ccdded26cbcd13f13f580b07ad54b4d10"
                ),
                source_item_seals=(
                    _SUCCESSOR_TERMINAL_APPLICATION_SOURCE_ITEM_SEALS
                ),
            ),
        ),
    ),
)


# The thirteen production-facing progress/successor claims use total checked
# gates.  These token seals are intentionally centralized: runner/worker work
# may move the two marked authoritative items while the tree is still being
# stabilized, and every seal must be refreshed together after that source
# movement stops.
_CHECKED_PRODUCTION_TOKEN_SOURCE = (
    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
)
_CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE = (
    "crates/iroha_core/src/sumeragi/v2_core/refinement/first_release_witness.rs"
)
_CHECKED_PRODUCTION_TOKEN_CLOSURE_SHA256 = (
    "03681c3a3df1ab231d64ba64e62d7aea1e125b9bff3133ec81d4d3bce11f1f1e"
)
_CHECKED_PRODUCTION_TOKEN_STRUCT_SHA256 = (
    "c79d22d356c0f53aceb7b771cb14660ea68ab6da856d46e186506bb5e570a9aa"
)
_CHECKED_PRODUCTION_TOKEN_IMPL_SHA256 = (
    "9a6f880e32c89dfc44a0bd12fca625d5cdbc5a41037ba46ee8cdc57549354068"
)
_CHECKED_PRODUCTION_TOKEN_UNWITNESSED_SHA256 = (
    "157599b4d0b6db28be7ea76c1e59848b97814a424407dc174865f8ea5be3528b"
)
_CHECKED_PRODUCTION_TOKEN_WITNESS_BINDER_SHA256 = (
    "f31a2bcc1a2a79939f6edc2f5d5362d29338984b4eb482ccc5dcc86881595483"
)
_CHECKED_PRODUCTION_TOKEN_WITNESS_ACCESSOR_SHA256 = (
    "9113ffd3920c71ccfca509e8f5247b547c47e98cedb7339f80362bc7afc6f8b6"
)
_CHECKED_PRODUCTION_TOKEN_CONSUMER_SHA256 = (
    "ba2bba675ad645198011a24ae3c60a024cdad47caf2eb1e0acefc16467ef9994"
)
_CHECKED_PRODUCTION_TOKEN_BORROWER_SHA256 = (
    "d22e80b13a8e1223839e9f87b6db64aae115138d649a5ed754989cd55fe19468"
)
_CHECKED_PRODUCTION_IN_FLIGHT_PROJECTION_SHA256 = (
    "0d89c85775aded9990deeefe79f082098629e60e348775c4820ce8a2e6667ed2"
)
_CHECKED_PRODUCTION_IN_FLIGHT_CONSTRUCTOR_SHA256 = (
    "96fe254882b529cb780963a465e6d930ea56258ab5862b74ec3f2c949daf7d18"
)
_CHECKED_PRODUCTION_FIRST_RELEASE_CONSTRUCTOR_SHA256 = (
    "c549d2d053ab1f7dfdee866c9a5b85d278733464111ea44e7f6b9b2e120a112a"
)
_CHECKED_PRODUCTION_IN_FLIGHT_MACRO_SHA256 = (
    "9e060bcae1f30be96faecfb04e75b66bfd7671e06f18f83c5c099520b6d5df74"
)
_CHECKED_PRODUCTION_IN_FLIGHT_KERNEL_SHA256 = (
    "c68bd376597d39cc72ae3195df249c286badeb8b8275c7933b1b1c933f9bd1aa"
)
_CHECKED_PRODUCTION_EFFECTIVE_LOCK_GATE_SHA256 = {
    "check_production_enter_view_effective_lock_transition": (
        "01d34e10a6b65314b25d6f41b2e35985831618404ceb5f3e8fc1d65e4c5c2fdd"
    ),
    "check_production_body_ownership_effective_lock_transition": (
        "c140204abebe6509b7b9fd185923a3ad5dffad911d2dd1be763a55b6dacd0fa7"
    ),
    "check_production_body_capacity_retirement_effective_lock_transition": (
        "bfc78743ec28a1591d6593242f24b6525ed61c0539f06a5c36ca6772973c38da"
    ),
    "check_production_body_service_effective_lock_transition": (
        "caa8f0973b35099ff923b243fa579944b5334bec61eeec6e48b85e51754aefd3"
    ),
}
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_PROJECTION_SHA256 = (
    "fd5b0d359c46f6f058a5ad7925d5e2aa6db215fd13e325025213679ce676f232"
)
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_VERUS_PROJECTION_SHA256 = (
    "14e34dc3fd5650279c2bb5665f384dd831725d4863e873612d0758f726e072d2"
)
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_MACRO_SHA256 = (
    "16f4268fe5032a9f91c683b6d71b3c8403d71f2ac4d47e597f65043b72e48611"
)
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_GATE_SHA256 = (
    "aaff756b0a731e4e90807fc7ed125502eebb2a8c48e1613afb1138b20c7f1f0a"
)
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_VERUS_GATE_SHA256 = (
    "3a3c7b1e2c1c9ac68ab20fc8022666b7d8da7825775f7f3aa4e30477d109048f"
)
_CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_AUXILIARY_THEOREM_SHA256 = (
    "67ebe84f470c8d16cc5207f20a574d778d542e52d970a6c5c57cf29ea60e7d47"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_PROJECTION_SHA256 = (
    "5fa36e7fa40e8d96deb04371a539f8ffa58dbaaf8bdd36ecc2dbd67249531eb1"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_VERUS_PROJECTION_SHA256 = (
    "490481c013f0e98760035000231290a993a72876b3b3d7e566bde7003e1d341f"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_MACRO_SHA256 = (
    "e3fc5ceca76f5a878abf66de4607ff4f4b393aff18b5f53870a451674acfb8ab"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_GATE_SHA256 = (
    "257d63bfc22f879efba083dbc4869e443e52bce841c5dff3d87208ac778c6429"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_VERUS_GATE_SHA256 = (
    "07e41c8605ff29ceb732e64ff1238ba8e22722b09d7be416f425e18b956f0fa0"
)
_CHECKED_PRODUCTION_EFFECT_TO_CANDIDATE_AUXILIARY_THEOREM_SHA256 = (
    "464d4610fd3499807750f084905df57cc80d185046b6a7e8e8d71e5e89c7b35b"
)
_CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_SHA256 = (
    "5cec83a64f58fe1751a5f60a7b3a0ad5d80512adb16a00132fd8242457f77d8f"
)
_CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_DESCENT_SHA256 = (
    "c99a968bf360ff9d32fd9813addefeb8e10562acd341d24d7b9479ba057589f9"
)
_TOTAL_GATE_THEOREM_ITEM_SHA256 = {
    "ProductionDurableIntentTraceRefinesProgressWitness": "129a6981dfcf42f154616ba6569eea2b671902267b2dfbddf4ede6add37560d0",
    "ProductionDecisionTraceRefinesRecoveryWitness": "d050e91b2250b3e9bdb5782e0544ed6950b44bfd74e9768ac959f2dea097b36b",
    "ProductionSchedulerTraceRefinesProtectedOwnership": "69d99a2f25774db5316ab2aeefd0872c24bf15a16c519bf5edd75a512c4d09aa",
    "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership": "e0d479c18f5a707de17d0872ff2187f94d7097afa856cff3b5b715e3fa6c5ffe",
    "ProductionTwoStageRelayRetryTraceRefinesSourceFairness": "dd13aaaa8240e1315ec88fffdd6afaee6d2d203a194b86e5c1587b324adf16b9",
    "ProductionReliableFlushTraceRefinesOutboundOwnership": "fda5ca38426f361cb4080f4a87a1fa8cfc21097a1a59c588e82d67e58b5cb66d",
    "ProductionApplicationTraceRefinesDecisionCompletion": "e88b1c4526fa790421a2623b255d08d1355b0554a6ee1b3e3adb3f1672af4adb",
    "ProductionAppliedSuccessorTraceRefinesIndexedActivation": "45e1424b31f0d9af04f396d573ba66893a9fb81bf4a1cb4aa1a1f2ad781f78f6",
    "ProductionRecoveredSuccessorTraceRefinesIndexedActivation": "6ce05b8be1b89ee4859e1f04d3587a1fee24a98fe0da856ce3f9afc3f2a07b5e",
    "ProductionStartupFailureAndRestartRefinesIndexedLifecycle": "efe0c59307f0ea67beeb2b272b0db043c8fcb9944cfafcec87822881f19a72f8",
    "ProductionHistoricalCertificateTraceRefinesIndexedAsync": "183d568934f16439f681bd6f14bf76dcd4e301376e568a92e549d0016c25b406",
    "ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync": "64be1b23ca6c77d63702616fbf6dc2a65de4ea450dff3f198eed92c1aec70a59",
    "ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal": "47663df352984e99efb32b9683bf854a938af27bfef2fafe889770b40a7dc3f0",
}
_LEGACY_EFFECTIVE_LOCK_CLAIM_CONSTANTS = (
    "ProductionEnterViewUsesPostInstallEffectiveLock",
    "ProductionBodyOwnershipPreservesEffectiveLock",
    "ProductionBodyCapacityRetirementPreservesEffectiveLock",
    "ProductionBodyServiceRefinesAsyncFairness",
)
_EFFECTIVE_LOCK_CHECKED_GATE_BY_CLAIM = {
    "ProductionEnterViewUsesPostInstallEffectiveLock": (
        "check_production_enter_view_effective_lock_transition"
    ),
    "ProductionBodyOwnershipPreservesEffectiveLock": (
        "check_production_body_ownership_effective_lock_transition"
    ),
    "ProductionBodyCapacityRetirementPreservesEffectiveLock": (
        "check_production_body_capacity_retirement_effective_lock_transition"
    ),
    "ProductionBodyServiceRefinesAsyncFairness": (
        "check_production_body_service_effective_lock_transition"
    ),
}
_EFFECTIVE_LOCK_CHECKED_CALL_SHAPES = {
    "ProductionEnterViewUsesPostInstallEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "check",
            1,
            0,
        ),
    ),
    "ProductionBodyOwnershipPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "plan_body_pipeline_owner_hash",
            0,
            0,
        ),
    ),
    "ProductionBodyCapacityRetirementPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "reconcile_protected_lock",
            1,
            1,
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "plan_certified_view_body_cleanup",
            0,
            0,
        ),
    ),
    "ProductionBodyServiceRefinesAsyncFairness": (
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "pop_next_with_ownership",
            2,
            4,
        ),
    ),
}
_EFFECTIVE_LOCK_LINKED_CONSUMER_SHAPES = {
    "ProductionEnterViewUsesPostInstallEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "step",
            1,
            1,
        ),
    ),
    "ProductionBodyOwnershipPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "commit_body_pipeline_owner",
            1,
            1,
        ),
    ),
    "ProductionBodyCapacityRetirementPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "install_view",
            1,
            3,
        ),
    ),
    "ProductionBodyServiceRefinesAsyncFairness": (),
}
_EFFECTIVE_LOCK_PLAN_CARRIERS = {
    "ProductionBodyOwnershipPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "BodyPipelineOwnerBindingPlan",
        ),
    ),
    "ProductionBodyCapacityRetirementPreservesEffectiveLock": (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "CertifiedViewBodyCleanupPlan",
        ),
    ),
}


@lru_cache(maxsize=1)
def _verus_evidence_contract_module() -> Any:
    """Load the independently pinned Verus evidence validator."""

    path = ROOT_DIR / "scripts" / "formal" / "sumeragi_v2_verus_evidence.py"
    spec = importlib.util.spec_from_file_location(
        "_sumeragi_v2_verus_evidence_contract", path
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Verus evidence contract: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _cross_tool_contract_errors() -> list[str]:
    """Check the immutable 4 + 7 + 6 refinement mapping itself."""

    errors: list[str] = []
    try:
        verus_evidence_sources = frozenset(
            _verus_evidence_contract_module().REQUIRED_SOURCE_PATHS
        )
    except (AttributeError, OSError, RuntimeError, ValueError) as error:
        errors.append(
            "cross-tool contract cannot load the Verus source inventory: "
            f"{error}"
        )
        verus_evidence_sources = frozenset()
    expected_ids = (
        "effective-lock-body-acquisition-production-refinement",
        "progress-witness-production-refinement",
        "successor-activation-exact-recovery-production-refinement",
    )
    observed_ids = tuple(
        contract.obligation_id for contract in CROSS_TOOL_REFINEMENT_CONTRACTS
    )
    if observed_ids != expected_ids:
        errors.append(
            "cross-tool obligation inventory must equal the canonical three "
            f"production seams {expected_ids!r}; found {observed_ids!r}"
        )
    observed_counts = tuple(
        len(contract.claims) for contract in CROSS_TOOL_REFINEMENT_CONTRACTS
    )
    if observed_counts != (4, 7, 6):
        errors.append(
            "cross-tool production claim cardinalities must equal (4, 7, 6); "
            f"found {observed_counts!r}"
        )

    expected_premises = {
        "effective-lock-body-acquisition-production-refinement": (
            "ProductionEffectiveLockBodyAcquisitionRefinement"
        ),
        "progress-witness-production-refinement": (
            "ProductionProgressWitnessTraceRefinement"
        ),
        "successor-activation-exact-recovery-production-refinement": (
            "ProductionSuccessorAndExactRecoveryTraceRefinement"
        ),
    }
    expected_ledger_bindings = {
        "effective-lock-body-acquisition-production-refinement": (
            "operator",
            "/\\ ProductionEffectiveLockBodyAcquisitionRefinement "
            "/\\ EffectiveLockAcquisitionModelObligation",
            "BY EffectiveLockAcquisitionModelObligation "
            "DEF EffectiveLockBodyAcquisitionProductionRefinementObligation",
        ),
        "progress-witness-production-refinement": (
            "operator",
            "/\\ ProductionProgressWitnessTraceRefinement "
            "/\\ ProgressWitnessObligation",
            "BY ProgressWitnessObligation "
            "DEF ProgressWitnessProductionRefinementObligation",
        ),
        "successor-activation-exact-recovery-production-refinement": (
            "operator",
            "/\\ ProductionSuccessorAndExactRecoveryTraceRefinement "
            "/\\ (IndexedChainSpec => []"
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)",
            "BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant "
            "DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
        ),
    }

    constants: list[str] = []
    verus_theorems: list[str] = []
    auxiliary_verus_theorems: list[str] = []
    tla_theorems: list[str] = []
    proof_modes: list[str] = []
    total_gate_names: list[str] = []
    for contract in CROSS_TOOL_REFINEMENT_CONTRACTS:
        reviewed_target = REQUIRED_PROOF_OBLIGATION_INVENTORY.get(
            contract.obligation_id
        )
        if reviewed_target != (contract.module, contract.ledger_symbol):
            errors.append(
                f"cross-tool contract {contract.obligation_id} targets "
                f"{(contract.module, contract.ledger_symbol)!r}, not reviewed "
                f"ledger target {reviewed_target!r}"
            )
        expected_premise = expected_premises.get(contract.obligation_id)
        expected_statement = (
            None
            if expected_premise is None
            else f"{expected_premise} => {contract.ledger_symbol}"
        )
        if contract.tla_statement != expected_statement:
            errors.append(
                f"cross-tool contract {contract.obligation_id} must imply its "
                f"exact ledger theorem symbol using {expected_statement!r}; "
                f"found {contract.tla_statement!r}"
            )
        expected_ledger_binding = expected_ledger_bindings.get(
            contract.obligation_id, (None, None, None)
        )
        observed_ledger_binding = (
            contract.ledger_declaration_kind,
            contract.ledger_statement,
            contract.tla_proof,
        )
        if observed_ledger_binding != expected_ledger_binding:
            errors.append(
                f"cross-tool contract {contract.obligation_id} must bind the exact "
                "reviewed ledger declaration kind, normalized definition, and bridge "
                f"proof {expected_ledger_binding!r}; found {observed_ledger_binding!r}"
            )
        tla_theorems.append(contract.tla_theorem)
        for claim in contract.claims:
            constants.append(claim.constant)
            verus_theorems.append(claim.verus_theorem)
            proof_modes.append(claim.proof_mode)
            if claim.total_gate is not None:
                total_gate_names.append(claim.total_gate.name)
            total_gate_names.extend(
                kernel.total_gate.name
                for kernel in claim.supplemental_kernels
                if kernel.total_gate is not None
            )
            auxiliary_verus_theorems.extend(
                kernel.auxiliary_verus_theorem
                for kernel in claim.supplemental_kernels
                if kernel.auxiliary_verus_theorem is not None
            )
            if not claim.production_sources:
                errors.append(
                    f"cross-tool claim {claim.constant} has no production sources"
                )
            for relative in (
                claim.verus_source,
                *claim.production_sources,
                *(consumer.source for consumer in claim.linked_consumers),
            ):
                path = Path(relative)
                if path.is_absolute() or ".." in path.parts:
                    errors.append(
                        f"cross-tool claim {claim.constant} has unsafe source path "
                        f"{relative!r}"
                    )
            required_evidence_sources = (
                claim.verus_source,
                claim.verified_kernel_source,
                *(call_site.source for call_site in claim.production_call_sites),
                *(
                    kernel.verified_kernel_source
                    for kernel in claim.supplemental_kernels
                ),
                *(
                    call_site.source
                    for kernel in claim.supplemental_kernels
                    for call_site in kernel.production_call_sites
                ),
                *(consumer.source for consumer in claim.linked_consumers),
            )
            missing_evidence_sources = tuple(
                relative
                for relative in required_evidence_sources
                if _nonempty_string(relative)
                and relative not in verus_evidence_sources
            )
            if missing_evidence_sources:
                errors.append(
                    f"cross-tool claim {claim.constant} has proof, kernel, or "
                    "authoritative production call sources outside the Verus "
                    f"evidence inventory: {missing_evidence_sources!r}"
                )
    for label, values, expected_count in (
        ("TLA constants", constants, 17),
        ("Verus theorem names", verus_theorems, 17),
        ("TLA theorem names", tla_theorems, 3),
    ):
        if len(values) != expected_count or len(set(values)) != expected_count:
            errors.append(
                f"cross-tool {label} must contain {expected_count} unique names"
            )
    if tuple(proof_modes).count("legacy_requires_builder") != 4:
        errors.append(
            "cross-tool proof modes must retain exactly four legacy "
            "effective-lock claims"
        )
    if tuple(proof_modes).count("total_checked_gate") != 13:
        errors.append(
            "cross-tool proof modes must contain exactly thirteen total "
            "checked-gate claims"
        )
    observed_legacy_constants = tuple(
        claim.constant
        for contract in CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.proof_mode == "legacy_requires_builder"
    )
    if observed_legacy_constants != _LEGACY_EFFECTIVE_LOCK_CLAIM_CONSTANTS:
        errors.append(
            "cross-tool legacy mode must contain exactly the four canonical "
            "effective-lock claims"
        )
    observed_total_constants = tuple(
        claim.constant
        for contract in CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.proof_mode == "total_checked_gate"
    )
    if observed_total_constants != tuple(_TOTAL_GATE_BY_CLAIM):
        errors.append(
            "cross-tool total checked-gate mode must contain exactly the "
            "canonical thirteen progress/successor claims"
        )
    unknown_modes = tuple(
        mode
        for mode in proof_modes
        if mode not in {"legacy_requires_builder", "total_checked_gate"}
    )
    if unknown_modes:
        errors.append(f"cross-tool claims have unknown proof modes {unknown_modes!r}")
    if auxiliary_verus_theorems != [
        "production_ingress_reservation_materialization_refines_protected_ownership",
        "production_leader_wire_admission_trace_refines_lifecycle_ownership",
        "production_effect_to_candidate_trace_refines_async_ownership",
    ]:
        errors.append(
            "cross-tool auxiliary Verus theorem inventory must contain exactly "
            "the reservation-materialization, durable leader-wire, and "
            "effect-to-candidate proofs"
        )
    if len(total_gate_names) != 18 or len(set(total_gate_names)) != 18:
        errors.append(
            "cross-tool total checked-gate inventory must contain exactly "
            "eighteen unique gates"
        )
    return errors


@dataclass(frozen=True)
class _CrossToolKernelContractView:
    """Uniform validated view of a claim's primary or supplemental kernel."""

    verified_kernel: str
    verified_kernel_source: str
    verified_kernel_parameters: str
    verified_kernel_body: str
    verified_kernel_const: bool
    verified_kernel_public: bool
    verified_kernel_shared_macro_sha256: tuple[tuple[str, str], ...]
    theorem_kernel_projection: str
    theorem_projection_builders: tuple[CrossToolProjectionBuilderContract, ...]
    production_call_sites: tuple[CrossToolProductionCallContract, ...]
    total_gate: CrossToolTotalGateContract | None
    auxiliary_verus_theorem: str | None
    auxiliary_verus_parameters: str | None
    auxiliary_verus_theorem_item_sha256: str | None


def _cross_tool_kernel_views(
    claim: CrossToolClaimContract,
) -> tuple[_CrossToolKernelContractView, ...]:
    """Return every exact kernel carried by one theorem contract."""

    shared_primary_values = (
        claim.verified_kernel,
        claim.verified_kernel_source,
        claim.verified_kernel_parameters,
        claim.verified_kernel_body,
        claim.theorem_kernel_projection,
    )
    if not all(_nonempty_string(value) for value in shared_primary_values):
        return ()
    (
        kernel,
        kernel_source,
        kernel_parameters,
        kernel_body,
        theorem_projection,
    ) = shared_primary_values
    assert isinstance(kernel, str)
    assert isinstance(kernel_source, str)
    assert isinstance(kernel_parameters, str)
    assert isinstance(kernel_body, str)
    assert isinstance(theorem_projection, str)
    if claim.proof_mode == "legacy_requires_builder":
        builder_values = (
            claim.theorem_projection_builder,
            claim.theorem_projection_builder_parameters,
            claim.theorem_projection_builder_return,
            claim.theorem_projection_builder_item_sha256,
        )
        if not all(_nonempty_string(value) for value in builder_values):
            return ()
        builder, builder_parameters, builder_return, builder_sha256 = builder_values
        assert isinstance(builder, str)
        assert isinstance(builder_parameters, str)
        assert isinstance(builder_return, str)
        assert isinstance(builder_sha256, str)
        builders = (
            CrossToolProjectionBuilderContract(
                name=builder,
                parameters=builder_parameters,
                return_type=builder_return,
                item_token_sha256=builder_sha256,
            ),
        )
    elif claim.proof_mode == "total_checked_gate":
        if claim.total_gate is None:
            return ()
        builders = ()
    else:
        return ()
    primary = _CrossToolKernelContractView(
        verified_kernel=kernel,
        verified_kernel_source=kernel_source,
        verified_kernel_parameters=kernel_parameters,
        verified_kernel_body=kernel_body,
        verified_kernel_const=claim.verified_kernel_const,
        verified_kernel_public=claim.verified_kernel_public,
        verified_kernel_shared_macro_sha256=(
            claim.verified_kernel_shared_macro_sha256
        ),
        theorem_kernel_projection=theorem_projection,
        theorem_projection_builders=builders,
        production_call_sites=claim.production_call_sites,
        total_gate=claim.total_gate,
        auxiliary_verus_theorem=None,
        auxiliary_verus_parameters=None,
        auxiliary_verus_theorem_item_sha256=None,
    )
    supplemental = tuple(
        _CrossToolKernelContractView(
            verified_kernel=contract.verified_kernel,
            verified_kernel_source=contract.verified_kernel_source,
            verified_kernel_parameters=contract.verified_kernel_parameters,
            verified_kernel_body=contract.verified_kernel_body,
            verified_kernel_const=contract.verified_kernel_const,
            verified_kernel_public=contract.verified_kernel_public,
            verified_kernel_shared_macro_sha256=(
                contract.verified_kernel_shared_macro_sha256
            ),
            theorem_kernel_projection=contract.theorem_kernel_projection,
            theorem_projection_builders=contract.theorem_projection_builders,
            production_call_sites=contract.production_call_sites,
            total_gate=contract.total_gate,
            auxiliary_verus_theorem=contract.auxiliary_verus_theorem,
            auxiliary_verus_parameters=contract.auxiliary_verus_parameters,
            auxiliary_verus_theorem_item_sha256=(
                contract.auxiliary_verus_theorem_item_sha256
            ),
        )
        for contract in claim.supplemental_kernels
    )
    return (primary, *supplemental)


def _rust_parameter_names(parameters: str) -> tuple[str, ...]:
    """Return simple named Rust parameters from one reviewed signature fragment."""

    # Cross-tool kernels deliberately use a flat, named projection signature.
    # Rejecting patterns/destructuring here keeps the source-fidelity check
    # unambiguous and prevents a changed projection from hiding in syntax the
    # checker does not normalize.
    names = re.findall(
        r"(?:^|,)\s*(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:", parameters
    )
    return tuple(names)


def _normalized_rust_contract(source: str) -> str:
    """Normalize one code-owned Rust/Verus contract to its token stream."""

    return " ".join(rust_code_tokens(source))


def _checked_token_exact_item(
    source: str,
    name: str,
    header: str,
    body: str,
    expected_sha256: str,
    label: str,
    *,
    brace_context: tuple[tuple[str, ...], ...] = (),
    attributes: tuple[str, ...] = ("#[must_use]",),
    sealed: bool = True,
) -> tuple[Any, str]:
    """Require one exact, unconditional checked-token function item."""

    items = rust_items(source, name)
    if len(items) != 1:
        raise ValueError(
            f"cross-tool checked-token closure requires exactly one {label}"
        )
    item = items[0]
    if (
        item.brace_context != brace_context
        or item.ancestor_inner_attributes
        or tuple(rust_code_tokens(attribute) for attribute in item.attributes)
        != tuple(rust_code_tokens(attribute) for attribute in attributes)
        or _rust_item_header_tokens(item) != rust_code_tokens(header)
        or rust_code_tokens(item.body) != rust_code_tokens(body)
    ):
        raise ValueError(
            f"cross-tool checked-token {label} changed its exact fail-closed shape"
        )
    item_sha256 = (
        _rust_sealed_item_token_sha256(item)
        if sealed
        else _rust_item_token_sha256(item)
    )
    if item_sha256 != expected_sha256:
        raise ValueError(
            f"cross-tool checked-token {label} does not match its exact "
            "reviewed token seal"
        )
    return item, item_sha256


def _checked_token_exact_struct(
    source: str,
    name: str,
    header: str,
    body: str,
    expected_sha256: str,
    label: str,
    *,
    attributes: tuple[str, ...],
) -> tuple[Any, str]:
    """Require one exact, unconditional checked-token struct item."""

    items = rust_struct_items(source, name)
    if len(items) != 1:
        raise ValueError(
            f"cross-tool checked-token closure requires exactly one {label}"
        )
    item = items[0]
    if (
        item.brace_context != ()
        or item.ancestor_inner_attributes
        or tuple(rust_code_tokens(attribute) for attribute in item.attributes)
        != tuple(rust_code_tokens(attribute) for attribute in attributes)
        or _rust_item_header_tokens(item) != rust_code_tokens(header)
        or rust_code_tokens(item.body) != rust_code_tokens(body)
    ):
        raise ValueError(
            f"cross-tool checked-token {label} changed its exact reviewed layout"
        )
    item_sha256 = _rust_sealed_item_token_sha256(item)
    if item_sha256 != expected_sha256:
        raise ValueError(
            f"cross-tool checked-token {label} does not match its exact "
            "reviewed token seal"
        )
    return item, item_sha256


def _checked_token_impl_blocks(source: str) -> tuple[tuple[str, ...], ...]:
    """Return every explicit impl block whose header names the opaque token."""

    tokens = rust_code_tokens(source)
    blocks: list[tuple[str, ...]] = []
    matching = {">": "<", "]": "[", ")": "(", "}": "{"}
    for start, token in enumerate(tokens):
        if token != "impl":
            continue
        stack: list[str] = []
        body_start: int | None = None
        for index in range(start + 1, len(tokens)):
            current = tokens[index]
            if current == "{" and not stack:
                body_start = index
                break
            if current == "<":
                if not stack or stack[-1] == "<":
                    stack.append(current)
            elif current in ("[", "(", "{"):
                stack.append(current)
            elif current in matching and stack and stack[-1] == matching[current]:
                stack.pop()
            elif current == ";" and not stack:
                break
        if body_start is None:
            continue
        header = tokens[start:body_start]
        if "CheckedProductionTransition" not in header:
            continue
        depth = 0
        for end in range(body_start, len(tokens)):
            if tokens[end] == "{":
                depth += 1
            elif tokens[end] == "}":
                depth -= 1
                if depth == 0:
                    blocks.append(tokens[start : end + 1])
                    break
        else:
            raise ValueError(
                "cross-tool CheckedProductionTransition impl is unterminated"
            )
    return tuple(blocks)


def _checked_token_named_literal_count(tokens: tuple[str, ...]) -> int:
    """Count direct opaque-token literals, including balanced turbofish types."""

    count = 0
    matching = {">": "<", "]": "[", ")": "(", "}": "{"}
    for index, token in enumerate(tokens):
        if token != "CheckedProductionTransition" or index + 1 >= len(tokens):
            continue
        cursor = index + 1
        if tokens[cursor] == "{":
            count += 1
            continue
        if tokens[cursor : cursor + 2] != ("::", "<"):
            continue
        cursor += 1
        stack: list[str] = []
        while cursor < len(tokens):
            current = tokens[cursor]
            if current == "<":
                if not stack or stack[-1] == "<":
                    stack.append(current)
            elif current in ("[", "(", "{"):
                stack.append(current)
            elif current in matching and stack and stack[-1] == matching[current]:
                stack.pop()
                if not stack:
                    cursor += 1
                    break
            cursor += 1
        if not stack and cursor < len(tokens) and tokens[cursor] == "{":
            count += 1
    return count


def _checked_token_alias_names(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Return aliases that could obscure an opaque-token impl or literal."""

    aliases: set[str] = set()
    identifier = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
    matching = {">": "<", "]": "[", ")": "(", "}": "{"}
    for start, token in enumerate(tokens):
        if token not in ("type", "use"):
            continue
        stack: list[str] = []
        end = len(tokens)
        for cursor in range(start + 1, len(tokens)):
            current = tokens[cursor]
            if current == "<":
                if not stack or stack[-1] == "<":
                    stack.append(current)
            elif current in ("[", "(", "{"):
                stack.append(current)
            elif current in matching and stack and stack[-1] == matching[current]:
                stack.pop()
            elif current == ";" and not stack:
                end = cursor
                break
        declaration = tokens[start:end]
        if "CheckedProductionTransition" not in declaration:
            continue
        if token == "type" and len(declaration) > 1 and "=" in declaration:
            name = declaration[1]
            aliases.add(
                name if identifier.fullmatch(name) else "<macro-type-alias>"
            )
        for cursor, current in enumerate(declaration[:-2]):
            if (
                current == "CheckedProductionTransition"
                and declaration[cursor + 1] == "as"
                and identifier.fullmatch(declaration[cursor + 2])
            ):
                aliases.add(declaration[cursor + 2])
            if current == "CheckedProductionTransition":
                for grouped in range(cursor + 1, len(declaration) - 2):
                    if (
                        declaration[grouped : grouped + 2] == ("self", "as")
                        and identifier.fullmatch(declaration[grouped + 2])
                    ):
                        aliases.add(declaration[grouped + 2])
    return tuple(sorted(aliases))


def _checked_token_macro_reference_count(tokens: tuple[str, ...]) -> int:
    """Count opaque-token names passed through macro invocations."""

    count = 0
    matching = {")": "(", "]": "[", "}": "{"}
    delimiters: list[tuple[str, bool]] = []
    for index, token in enumerate(tokens):
        if token in ("(", "[", "{"):
            inherited = delimiters[-1][1] if delimiters else False
            delimiters.append(
                (token, inherited or (index > 0 and tokens[index - 1] == "!"))
            )
        elif token in matching:
            if delimiters and delimiters[-1][0] == matching[token]:
                delimiters.pop()
        elif (
            token == "CheckedProductionTransition"
            and delimiters
            and delimiters[-1][1]
        ):
            count += 1
    return count


def _cross_tool_checked_token_payload(
    *,
    source_entries: list[Any],
    root_dir: Path,
) -> dict[str, Any]:
    """Validate the opaque authorization token in its authenticated include closure."""

    source_entry = _verus_source_entry(
        source_entries,
        _CHECKED_PRODUCTION_TOKEN_SOURCE,
        description="checked production token parent source",
    )
    definition_entry = _verus_source_entry(
        source_entries,
        _CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
        description="checked production token definition source",
    )
    path = root_dir / _CHECKED_PRODUCTION_TOKEN_SOURCE
    definition_path = root_dir / _CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE
    for checked_path, description in (
        (path, "parent"),
        (definition_path, "definition"),
    ):
        if not checked_path.is_file() or checked_path.is_symlink():
            raise ValueError(
                "cross-tool checked-token "
                f"{description} source is not a regular file: {checked_path}"
            )
    source_sha256 = _sha256_file(path)
    definition_sha256 = _sha256_file(definition_path)
    if source_entry.get("sha256") != source_sha256:
        raise ValueError("cross-tool checked-token parent source digest mismatch")
    if definition_entry.get("sha256") != definition_sha256:
        raise ValueError(
            "cross-tool checked-token definition source digest mismatch"
        )
    source_errors: list[str] = []
    _loaded_path, source = _read_reviewed_rust_source(
        root_dir,
        _CHECKED_PRODUCTION_TOKEN_SOURCE,
        source_errors,
        "checked production token authenticated include closure",
    )
    if source_errors:
        raise ValueError(
            "cross-tool checked-token authenticated include closure is invalid: "
            + "; ".join(source_errors)
        )
    definition_source = definition_path.read_text(encoding="utf-8")
    aliases = _checked_token_alias_names(rust_code_tokens(source))
    provider_aliases = _checked_token_alias_names(
        rust_code_tokens(definition_source)
    )
    if aliases or provider_aliases:
        raise ValueError(
            "cross-tool CheckedProductionTransition may not have type or use "
            f"aliases; found closure={aliases!r}, provider={provider_aliases!r}"
        )
    macro_references = _checked_token_macro_reference_count(
        rust_code_tokens(source)
    )
    provider_macro_references = _checked_token_macro_reference_count(
        rust_code_tokens(definition_source)
    )
    if macro_references or provider_macro_references:
        raise ValueError(
            "cross-tool CheckedProductionTransition may not be passed through "
            "macro invocations; found "
            f"closure={macro_references}, provider={provider_macro_references}"
        )

    token_attributes = (
        '#[must_use = "checked transition evidence must be consumed"]',
        "#[derive(Debug, PartialEq, Eq)]",
    )
    token_body = """
        projection: P,
        first_release_witness:
            Option<ProductionInFlightFirstReleaseTransitionWitnessV1>,
    """
    struct, struct_sha256 = _checked_token_exact_struct(
        source,
        "CheckedProductionTransition",
        "pub struct CheckedProductionTransition<P>",
        token_body,
        _CHECKED_PRODUCTION_TOKEN_STRUCT_SHA256,
        "CheckedProductionTransition struct",
        attributes=token_attributes,
    )
    provider_struct, provider_struct_sha256 = _checked_token_exact_struct(
        definition_source,
        "CheckedProductionTransition",
        "pub struct CheckedProductionTransition<P>",
        token_body,
        _CHECKED_PRODUCTION_TOKEN_STRUCT_SHA256,
        "CheckedProductionTransition provider struct",
        attributes=token_attributes,
    )
    if provider_struct.source != struct.source:
        raise ValueError(
            "cross-tool CheckedProductionTransition must be supplied by its "
            "exact authenticated definition provider"
        )

    impl_blocks = _checked_token_impl_blocks(source)
    provider_impl_blocks = _checked_token_impl_blocks(definition_source)
    if len(impl_blocks) != 1 or len(provider_impl_blocks) != 1:
        raise ValueError(
            "cross-tool CheckedProductionTransition must have exactly one "
            "explicit inherent impl in its authenticated closure and provider"
        )
    if impl_blocks[0] != provider_impl_blocks[0]:
        raise ValueError(
            "cross-tool CheckedProductionTransition impl must come from its "
            "exact authenticated definition provider"
        )
    impl_sha256 = hashlib.sha256(
        "\0".join(impl_blocks[0]).encode("utf-8")
    ).hexdigest()
    if impl_sha256 != _CHECKED_PRODUCTION_TOKEN_IMPL_SHA256:
        raise ValueError(
            "cross-tool CheckedProductionTransition impl does not match its "
            "exact reviewed token seal"
        )

    impl_context = (
        ("impl", "<", "P", ">", "CheckedProductionTransition", "<", "P", ">"),
    )
    method_contracts = (
        (
            "unwitnessed",
            "const fn unwitnessed(projection: P) -> Self",
            "Self { projection, first_release_witness: None, }",
            _CHECKED_PRODUCTION_TOKEN_UNWITNESSED_SHA256,
            "private const unwitnessed constructor",
            (),
        ),
        (
            "accepted_projection",
            "pub(crate) const fn accepted_projection(&self) -> &P",
            "&self.projection",
            _CHECKED_PRODUCTION_TOKEN_BORROWER_SHA256,
            "borrowed accepted_projection accessor",
            ("#[must_use]",),
        ),
        (
            "with_first_release_witness",
            """
                pub(super) fn with_first_release_witness(
                    mut self,
                    witness: ProductionInFlightFirstReleaseTransitionWitnessV1,
                ) -> Self
            """,
            """
                self.first_release_witness = Some(witness);
                self
            """,
            _CHECKED_PRODUCTION_TOKEN_WITNESS_BINDER_SHA256,
            "first-release witness binder",
            ("#[must_use]",),
        ),
        (
            "first_release_witness",
            """
                pub(crate) const fn first_release_witness(
                    &self,
                ) -> Option<&ProductionInFlightFirstReleaseTransitionWitnessV1>
            """,
            "self.first_release_witness.as_ref()",
            _CHECKED_PRODUCTION_TOKEN_WITNESS_ACCESSOR_SHA256,
            "first-release witness accessor",
            (
                "#[must_use]",
                """
                    #[cfg_attr(
                        not(test),
                        allow(dead_code, reason = "first-release witness")
                    )]
                """,
            ),
        ),
        (
            "into_projection",
            "pub fn into_projection(self) -> P",
            "self.projection",
            _CHECKED_PRODUCTION_TOKEN_CONSUMER_SHA256,
            "consuming into_projection method",
            ("#[must_use]",),
        ),
    )
    method_payload: dict[str, str] = {}
    for name, header, body, expected_sha256, label, attributes in method_contracts:
        item, item_sha256 = _checked_token_exact_item(
            source,
            name,
            header,
            body,
            expected_sha256,
            label,
            brace_context=impl_context,
            attributes=attributes,
        )
        provider_item, provider_item_sha256 = _checked_token_exact_item(
            definition_source,
            name,
            header,
            body,
            expected_sha256,
            f"provider {label}",
            brace_context=impl_context,
            attributes=attributes,
        )
        if provider_item.source != item.source:
            raise ValueError(
                f"cross-tool checked-token {label} must come from its exact "
                "authenticated definition provider"
            )
        method_payload[name] = item_sha256
        assert provider_item_sha256 == item_sha256
    expected_method_names = tuple(contract[0] for contract in method_contracts)
    for method_source, description in (
        (source, "authenticated include closure"),
        (definition_source, "definition provider"),
    ):
        observed_method_names = tuple(
            item.name
            for item in _rust_all_function_items(method_source)
            if item.brace_context == impl_context
        )
        if observed_method_names != expected_method_names:
            raise ValueError(
                "cross-tool CheckedProductionTransition method inventory in "
                f"its {description} must equal {expected_method_names!r}; "
                f"found {observed_method_names!r}"
            )

    projection_attributes = (
        "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]",
    )
    _in_flight_projection, in_flight_projection_sha256 = (
        _checked_token_exact_struct(
            source,
            "ProductionInFlightReservationTransitionProjection",
            """
                pub(crate) struct
                    ProductionInFlightReservationTransitionProjection
            """,
            """
                pub(crate) action: u8,
                pub(crate) requested_reservation_identity:
                    CanonicalIdentityProjection,
                pub(crate) requested_release_identity:
                    CanonicalIdentityProjection,
                pub(crate) before:
                    ProductionInFlightReservationOwnerProjection,
                pub(crate) after:
                    ProductionInFlightReservationOwnerProjection,
            """,
            _CHECKED_PRODUCTION_IN_FLIGHT_PROJECTION_SHA256,
            "in-flight reservation transition projection",
            attributes=projection_attributes,
        )
    )
    macros = rust_macro_items(
        source, "production_in_flight_reservation_transition_body"
    )
    if len(macros) != 1:
        raise ValueError(
            "cross-tool checked-token closure requires exactly one in-flight "
            "reservation transition macro"
        )
    in_flight_macro = macros[0]
    if (
        in_flight_macro.brace_context != ()
        or in_flight_macro.delimiter_context != ()
        or in_flight_macro.attributes
        or in_flight_macro.ancestor_inner_attributes
    ):
        raise ValueError(
            "cross-tool in-flight reservation transition macro must remain "
            "unconditional and top-level"
        )
    in_flight_macro_sha256 = _rust_item_token_sha256(in_flight_macro)
    if in_flight_macro_sha256 != _CHECKED_PRODUCTION_IN_FLIGHT_MACRO_SHA256:
        raise ValueError(
            "cross-tool in-flight reservation transition macro does not match "
            "its exact reviewed token seal"
        )
    _in_flight_kernel, in_flight_kernel_sha256 = _checked_token_exact_item(
        source,
        "production_in_flight_reservation_transition_kernel",
        """
            pub(crate) const fn production_in_flight_reservation_transition_kernel(
                projection: ProductionInFlightReservationTransitionProjection,
            ) -> bool
        """,
        "production_in_flight_reservation_transition_body!(projection)",
        _CHECKED_PRODUCTION_IN_FLIGHT_KERNEL_SHA256,
        "in-flight reservation transition kernel",
        attributes=(),
        sealed=False,
    )
    _in_flight_constructor, in_flight_constructor_sha256 = (
        _checked_token_exact_item(
            source,
            "check_production_in_flight_reservation_transition",
            """
                pub(crate) fn check_production_in_flight_reservation_transition(
                    projection: ProductionInFlightReservationTransitionProjection,
                ) -> Option<CheckedProductionTransition<
                    ProductionInFlightReservationTransitionProjection
                >>
            """,
            """
                if production_in_flight_reservation_transition_kernel(projection) {
                    Some(CheckedProductionTransition::unwitnessed(projection))
                } else {
                    None
                }
            """,
            _CHECKED_PRODUCTION_IN_FLIGHT_CONSTRUCTOR_SHA256,
            "in-flight reservation checked constructor",
        )
    )
    _first_release_constructor, first_release_constructor_sha256 = (
        _checked_token_exact_item(
            source,
            "check_production_in_flight_first_release_transition",
            """
                pub(crate) fn check_production_in_flight_first_release_transition(
                    projection: ProductionInFlightFirstReleaseTransitionProjection,
                ) -> Option<CheckedProductionTransition<
                    ProductionInFlightFirstReleaseTransitionProjection
                >>
            """,
            """
                if production_in_flight_first_release_transition_kernel(projection) {
                    Some(CheckedProductionTransition::unwitnessed(projection))
                } else {
                    None
                }
            """,
            _CHECKED_PRODUCTION_FIRST_RELEASE_CONSTRUCTOR_SHA256,
            "in-flight first-release checked constructor",
            attributes=("#[must_use]", "#[allow(dead_code)]"),
        )
    )
    _materialization, materialization_projection_sha256 = (
        _checked_token_exact_struct(
            source,
            "ProductionIngressReservationMaterializationTraceProjection",
            "pub struct ProductionIngressReservationMaterializationTraceProjection",
            """
                pub(crate) incoming_height: u64,
                pub(crate) incoming_view: u64,
                pub(crate) incoming_generation: u64,
                pub(crate) incoming_class: u8,
                pub(crate) stored_height: u64,
                pub(crate) stored_view: u64,
                pub(crate) stored_generation: u64,
                pub(crate) stored_class: u8,
                pub(crate) queue_len_before: u64,
                pub(crate) queue_len_after: u64,
                pub(crate) reserved_slots_before: u8,
                pub(crate) reserved_slots_after: u8,
                pub(crate) queue_capacity: u64,
                pub(crate) ordinal_source_before: u128,
                pub(crate) physical_admission_ordinal: u128,
                pub(crate) lifecycle_ordinal: u128,
                pub(crate) ordinal_source_after: u128,
                pub(crate) dormant_reservations_before: u64,
                pub(crate) dormant_reservations_after: u64,
                pub(crate) dormant_owner_ordinal: u128,
            """,
            _CHECKED_PRODUCTION_INGRESS_MATERIALIZATION_PROJECTION_SHA256,
            "ingress reservation materialization projection",
            attributes=projection_attributes,
        )
    )

    legacy_gate_contracts = (
        (
            "check_production_enter_view_effective_lock_transition",
            """
                pub(crate) fn check_production_enter_view_effective_lock_transition(
                    trace: EffectiveLockTraceProjection,
                    enter_view: EnterViewProjection,
                ) -> Option<CheckedProductionTransition<(
                    EffectiveLockTraceProjection, EnterViewProjection
                )>>
            """,
            """
                if production_enter_view_uses_post_install_effective_lock_kernel(
                    trace, enter_view
                ) {
                    Some(CheckedProductionTransition::unwitnessed((
                        trace, enter_view,
                    )))
                } else {
                    None
                }
            """,
        ),
        (
            "check_production_body_ownership_effective_lock_transition",
            """
                pub(crate) fn check_production_body_ownership_effective_lock_transition(
                    projection: EffectiveLockTraceProjection,
                ) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>>
            """,
            """
                if production_body_ownership_preserves_effective_lock_kernel(projection) {
                    Some(CheckedProductionTransition::unwitnessed(projection))
                } else {
                    None
                }
            """,
        ),
        (
            "check_production_body_capacity_retirement_effective_lock_transition",
            """
                pub(crate) fn check_production_body_capacity_retirement_effective_lock_transition(
                    projection: EffectiveLockTraceProjection,
                ) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>>
            """,
            """
                if production_body_capacity_retirement_preserves_effective_lock_kernel(projection) {
                    Some(CheckedProductionTransition::unwitnessed(projection))
                } else {
                    None
                }
            """,
        ),
        (
            "check_production_body_service_effective_lock_transition",
            """
                pub(crate) fn check_production_body_service_effective_lock_transition(
                    projection: EffectiveLockTraceProjection,
                ) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>>
            """,
            """
                if production_body_service_refines_async_fairness_kernel(projection) {
                    Some(CheckedProductionTransition::unwitnessed(projection))
                } else {
                    None
                }
            """,
        ),
    )
    legacy_gate_payload: list[dict[str, str]] = []
    for legacy_name, legacy_header, legacy_body in legacy_gate_contracts:
        _legacy_item, legacy_sha256 = _checked_token_exact_item(
            source,
            legacy_name,
            legacy_header,
            legacy_body,
            _CHECKED_PRODUCTION_EFFECTIVE_LOCK_GATE_SHA256[legacy_name],
            f"legacy effective-lock constructor {legacy_name}",
        )
        legacy_gate_payload.append(
            {"name": legacy_name, "item_token_sha256": legacy_sha256}
        )

    source_tokens = rust_code_tokens(source)
    constructor_count = _token_sequence_count(
        source_tokens,
        ("CheckedProductionTransition", "::", "unwitnessed", "("),
    )
    if constructor_count != 25:
        raise ValueError(
            "cross-tool opaque token closure must contain exactly twenty-five "
            "unwitnessed calls (eighteen total gates, four effective-lock gates, "
            "the outer reducer, in-flight reservation, and first-release gates); "
            f"found {constructor_count}"
        )
    raw_literal_count = _checked_token_named_literal_count(source_tokens)
    if raw_literal_count != 0:
        raise ValueError(
            "cross-tool opaque token closure may not contain raw "
            "CheckedProductionTransition struct literals; found "
            f"{raw_literal_count}"
        )
    structural = mask_rust_comments_and_literals(source)
    if re.search(
        r"impl[^{};]{0,240}\b(?:Clone|Copy|Default|Encode|Decode)\b"
        r"[^{};]{0,240}\bCheckedProductionTransition\b",
        structural,
    ):
        raise ValueError(
            "cross-tool checked token may not gain Clone, Copy, Default, or "
            "codec construction"
        )
    closure_sha256 = hashlib.sha256(source.encode("utf-8")).hexdigest()
    if closure_sha256 != _CHECKED_PRODUCTION_TOKEN_CLOSURE_SHA256:
        raise ValueError(
            "cross-tool checked-token authenticated include closure does not "
            "match its exact reviewed source seal"
        )

    reexports = (
        (
            "crates/iroha_core/src/sumeragi/v2_core.rs",
            ("pub", "use", "refinement", "::", "{", "CheckedProductionTransition", ","),
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            ("pub", "use", "v2_core", "::", "{", "CheckedProductionTransition", ","),
        ),
    )
    reexport_payload: list[dict[str, str]] = []
    for relative, tokens in reexports:
        reexport_path = root_dir / relative
        if not reexport_path.is_file() or reexport_path.is_symlink():
            raise ValueError(
                "cross-tool checked-token re-export source is invalid: "
                f"{reexport_path}"
            )
        reexport_source = reexport_path.read_text(encoding="utf-8")
        if _token_sequence_count(rust_code_tokens(reexport_source), tokens) != 1:
            raise ValueError(
                f"cross-tool checked-token re-export changed in {relative}"
            )
        reexport_payload.append(
            {"path": relative, "sha256": _sha256_file(reexport_path)}
        )
    return {
        "source": _CHECKED_PRODUCTION_TOKEN_SOURCE,
        "source_sha256": source_sha256,
        "definition_source": _CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
        "definition_source_sha256": definition_sha256,
        "struct_item_token_sha256": struct_sha256,
        "definition_struct_item_token_sha256": provider_struct_sha256,
        "unwitnessed_item_token_sha256": method_payload["unwitnessed"],
        "borrower_item_token_sha256": method_payload["accepted_projection"],
        "witness_binder_item_token_sha256": method_payload[
            "with_first_release_witness"
        ],
        "witness_accessor_item_token_sha256": method_payload[
            "first_release_witness"
        ],
        "consumer_item_token_sha256": method_payload["into_projection"],
        "in_flight_projection_item_token_sha256": in_flight_projection_sha256,
        "in_flight_macro_item_token_sha256": in_flight_macro_sha256,
        "in_flight_kernel_item_token_sha256": in_flight_kernel_sha256,
        "in_flight_constructor_item_token_sha256": in_flight_constructor_sha256,
        "first_release_constructor_item_token_sha256": (
            first_release_constructor_sha256
        ),
        "materialization_projection_item_token_sha256": (
            materialization_projection_sha256
        ),
        "legacy_effective_lock_constructor_items": legacy_gate_payload,
        "constructor_count": constructor_count,
        "raw_named_literal_count": raw_literal_count,
        "reexports": reexport_payload,
    }


def _first_json_mismatch(
    expected: Any, observed: Any, path: str = "$"
) -> str | None:
    """Return the first path at which two evidence values differ."""

    if type(expected) is not type(observed):
        return path
    if isinstance(expected, dict):
        if set(expected) != set(observed):
            return path
        for key in sorted(expected):
            mismatch = _first_json_mismatch(
                expected[key], observed[key], f"{path}.{key}"
            )
            if mismatch is not None:
                return mismatch
        return None
    if isinstance(expected, list):
        if len(expected) != len(observed):
            return path
        # Exact lengths were checked above; plain zip keeps this verifier
        # compatible with the repository's Python 3.9 floor.
        for index, (expected_item, observed_item) in enumerate(zip(expected, observed)):
            mismatch = _first_json_mismatch(
                expected_item, observed_item, f"{path}[{index}]"
            )
            if mismatch is not None:
                return mismatch
        return None
    return None if expected == observed else path
