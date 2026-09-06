# Historical project evidence

Captured from the dirty working copies on 2026-09-06. These records are historical,
including statements once labelled active; they are not current release readiness.

The manifest records every original occurrence, byte order, heading context and hash.
Its hash-bound `record-inventory.json.gz` contains the complete JSON occurrence
inventory; compression removes metadata repetition without dropping source evidence.
Only exact sections with the same relative-link context are deduplicated. Link
rewrites are reversible; code and pre-existing unresolved targets remain evidence.

| Original source | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| `status.md` | 26090588 | 339672 | `ebafd062901d863418432e4a27eca2049b4fd57470b1a1c9d7d1499cdb2ebb2d` |
| `roadmap.md` | 1761216 | 25135 | `ba0930ff8fa9cb9ee45b8c6d31ac67c571452e78d9d7696b568a808cb3509835` |

64736 unique records preserve 67311 source occurrences.

0 distinct source/target pairs were already unresolved at capture; see
[the original-link audit](unresolved-original-links.json). Their original destinations
are retained, not silently redirected. External URLs and fragments in other files
are not network-checked. Defined reference links are expanded reversibly so they
continue to work when their definition and use land on different archive pages.

## Subsystem navigation

| Subsystem | Records | Pages |
| --- | ---: | ---: |
| [architecture-build](records/architecture-build/index.md) | 329 | 61 |
| [community-docs](records/community-docs/index.md) | 10 | 3 |
| [consensus-network](records/consensus-network/index.md) | 15120 | 400 |
| [crypto-proofs](records/crypto-proofs/index.md) | 7742 | 214 |
| [general](records/general/index.md) | 4656 | 166 |
| [governance-identity](records/governance-identity/index.md) | 1021 | 69 |
| [kagemusha](records/kagemusha/index.md) | 211 | 40 |
| [ledger-state](records/ledger-state/index.md) | 1044 | 85 |
| [musubi-taikai](records/musubi-taikai/index.md) | 1479 | 69 |
| [operations-torii](records/operations-torii/index.md) | 1566 | 101 |
| [sccp-settlement](records/sccp-settlement/index.md) | 17301 | 385 |
| [sdks-native](records/sdks-native/index.md) | 2472 | 107 |
| [sorafs](records/sorafs/index.md) | 9181 | 241 |
| [soranet](records/soranet/index.md) | 1064 | 51 |
| [vm-norito-model](records/vm-norito-model/index.md) | 1540 | 108 |

## Original roadmap areas

Every original heading remains traceable; the current roadmap consolidates outcomes.

- [First-release architecture redesign](records/architecture-build/undated-001.md#record-1997a07734bef10975baa4b887499c5df120a6538dfc8d7bc44e56b0a9f4c1c1)
- [KAGEMUSHA product coordinator and durable recovery](records/kagemusha/undated-001.md#record-87b862d2432f3ab4fcb646561f5a0482493172e59f38e9b4ffd355939abd23d5)
- [Android native device integration](records/sdks-native/undated-001.md#record-6ae57e65e9ee5a56a5ced930a1037963b5f2f5d2e5f1b651b88af9228c40bf67)
- [Additive SNS dataspace bootstrap qualification](records/governance-identity/undated-001.md#record-d925ec7f3aa62d0cd18be20dedb9fac66fba3564781573d6645067f1d93cfc82)
- [Python SDK first-release follow-up](records/sdks-native/undated-001.md#record-e3a0e597b4858cd5aa181a81409737b434651057f0aef890fb59cf5c07ed0b08)
- [JavaScript SDK first-release follow-up](records/sdks-native/undated-001.md#record-fef41c7bb66bf1a137803de51fe3e57eed7c52d475b1bb167be91740cb88f0e5)
- [C# SDK release qualification](records/sdks-native/undated-001.md#record-525e941458b1e44230dde287827746a97d071a82576ef0909abe3b9b63b0f35a)
- [Revision-4 DA/RBC release qualification](records/consensus-network/undated-001.md#record-721bffd59e2a51a1ff0d48bb4ffe05f9455966f5315ac86568f0b83104c3e4d1)
- [SORA Parliament release qualification](records/governance-identity/undated-001.md#record-cb13f7e39e4b1539434f17a47b735b7b81bbd783600f415e66ca0434d675b8f0)
- [Default-on capability follow-up](records/vm-norito-model/undated-001.md#record-2bf3d306cf0b260b41696e447ce2897472468605d139d1f34d4b0b3fc7d1742c)
- [Security-audit release qualification](records/operations-torii/undated-001.md#record-bb62a9c10c6414df9abac2fdd5ac35ac308aef6d395bcb406bad05bd651ea9ec)
- [Atomic private settlement release qualification](records/sccp-settlement/undated-001.md#record-83c77ac6268405965ff5cb85bcc0d77bb5f90dad500e0489e801ca031531a17e)
- [Telemetry first-release closure](records/operations-torii/undated-001.md#record-6d2d0f9a90238235a87760f3c8ba31eebafeca4b9e3b51e23107303d62931f32)
- [Derive/proc-macro first-release closure](records/vm-norito-model/undated-001.md#record-e573a1385f6014f938780ed89871c3d610d090de528cb8e910e21c1f1cc33602)
- [Mochi first-release closure](records/operations-torii/undated-001.md#record-942a5982e3c8d37f0e54ab3fddb54b3eae2a915e6ec6018dc070e7f05a9244ee)
- [Iroha configuration first-release closure](records/operations-torii/undated-001.md#record-46b04aeb6273d60e87594bc65bb4fac8ce1b6d151165f7c5675ac13d8200c63f)
- [Kagami first-release closure](records/vm-norito-model/undated-001.md#record-9a64e40891502efba2175ae21cf5e544258d8f72e5db60e1694b5ccc446dd94e)
- [Digital-signature release qualification](records/crypto-proofs/undated-001.md#record-a1f5bc5283b3f5c0e21a76c9f5d7b065bf607b32d5032556dbbe5065475d301f)
- [Iroha crypto first-release closure](records/crypto-proofs/undated-001.md#record-d545f503dfec200e016344b706beb6511ebaddbbfb8078ee309ab123c452eb33)
- [SDK first-release closure](records/sdks-native/undated-001.md#record-7b211e9dfb51fc5426743e9f63079573688cc165bcd440c9ed10da4962642dbc)
- [Iroha Core first-release closure](records/ledger-state/undated-001.md#record-a94faab561091217c5e39b36e2c131484c38bd842ce56a9c1af55a418aba1330)
- [P2P first-release closure](records/consensus-network/undated-001.md#record-ae668e73833a0cb1f376a5cbe4c58a6d4c8b8b5be26830a5c168daf6dbcd3c4f)
- [Sumeragi first-release closure](records/consensus-network/undated-001.md#record-abeb1a5dcaacefb277d02f7a2aa602f9a895b02601f1513afd771991fad050e7)
- [IVM first-release closure](records/vm-norito-model/undated-001.md#record-fd57eb39e3436e10a590a487f0dc701aa3e9d47f9a569dbea8680b0b64f59527)
- [Kura emergency-start closure](records/ledger-state/undated-001.md#record-483409e2328fd132198abb6e070f9e11d71911bc2834d54095d8d09268073197)
- [Norito archive API closure](records/vm-norito-model/undated-001.md#record-bf8ff550d7e13d78c32e0c3fef33554fe586922aabde7d974a8f31466dbcb087)
- [Torii first-release closure](records/operations-torii/undated-001.md#record-9c38d61b1d6b65167fc3d373373c0184e26fe1b550df4064a7c9c8a20584c5ae)
- [Taikai first-release closure](records/musubi-taikai/undated-001.md#record-e40b44d28d1d582a3532db61744ee4a7fcde0598f8d32405ae923c858f940ad1)
- [Data-model first-release closure](records/vm-norito-model/undated-001.md#record-01cfec196832d8ad8acba4489e0a4b4f5c734c1c8a4c618501b3fb17535033e3)
- [Generated HF isolated execution](records/general/undated-001.md#record-11c49efd369322ecc4d3b01c2db1484e14fcde550f8f1c2ec38deafaffe888ee)
- [Merged-candidate compile recovery](records/architecture-build/undated-001.md#record-3dd449171c9b45b2bfd3845fa08e8d0846ebda55afefd55806e90612b66a291e)
- [SORA Parliament hardening](records/governance-identity/undated-001.md#record-79f28b80fef0958b690d85c67b9247097a7cbc4c733366eeff25414d3f944f44)
- [ZK algorithm release qualification](records/crypto-proofs/undated-001.md#record-2619311276e1ec58aab7dfebe54b2549e0ee4b6bd61e75fd6c7dcf990a14611c)
- [Inrou V1 release qualification](records/soranet/undated-001.md#record-38679ba9eb1a575a025153a1dd61678beb0a31dcb313ebc8fd56be5004f84167)
- [SoraNet first-release security qualification](records/soranet/undated-001.md#record-9c6eed8499b14f862d833c15c3fd3c3514f58e9898508a82c76c38278e471ff7)
- [KAGEMUSHA V1 release completion](records/kagemusha/undated-001.md#record-78bdc4a7ca9db1d5af58c3fd9526ba2a656850c5709989cd7b371128e72034a6)
- [MKHE and Figure 9 evidence-gated completion](records/crypto-proofs/undated-001.md#record-0ff775b0d366b78314daec5e6ebbfe5a3ac44dba9793224bfba471d5a7e34911)
- [First-release hard-cut closeout](records/operations-torii/undated-001.md#record-99384def184ef5fcc74be96cf3df2c8b4c7e71fe80d3e0e24ed576ed25f79a3f)
- [Workspace review closure](records/architecture-build/undated-001.md#record-4a3dbbd41d761574bb331c487b1defba81947d4ef5e89c3f5a1d54ee081738f3)
- [Nexus topology model closure](records/general/undated-001.md#record-58c29285ca2b4d0d4867fc8f0be753bfc0e9b4f4ad95ca986a02046a56c39d5e)
- [Build-efficiency closeout](records/architecture-build/undated-001.md#record-6491d9de098e123bbf423e8b9ed3ffb9ebda8778390531d8d0a40cbc630ee460)
- [Exact network identity completion](records/general/undated-001.md#record-b014cec3380ec155257ac3fd2df781ff66ad516769c237330aa4b1c6c5291e55)
- [Native Torii MCP release qualification](records/sdks-native/undated-001.md#record-a9f25c74f1125dd1d65b62e110042673534277c1c751c44369e109cfcb59e35a)
- [Sumeragi v2 revision-4 release gates](records/consensus-network/undated-001.md#record-ee3b65699c268c402b8559f0aa7ddd9c8ded75739f03a5b57c6f3b388cc41c2c)
- [Musubi first-release registry and developer ecosystem reset](records/musubi-taikai/undated-001.md#record-8f404caa97ef74d270b01c4789fab39964aba9dd364ce5fdff66a637626cc71e)
- [First-release security remediation validation](records/operations-torii/undated-001.md#record-64cef74910761fcdb2aeabd78751777a5b5afd2a2b445438dacdc7c5272c7a7f)
- [SoraFS V1 Governance DAG deployment closure](records/sorafs/undated-001.md#record-18be783fe52db4f02eb18618c7b8966bd924d9bc1252410bf3c1f7b0f4a53748)
- [Repository structure follow-ups](records/architecture-build/undated-001.md#record-a602bda12c3f3a0f415b06de0c3d38ad3ea6d82160477e6a01ee2ffd6c0e4f09)
- [Public Taira node onboarding](records/governance-identity/undated-001.md#record-9c4e75e4776f063b9d01264eb702608774a102b2ed62306fe4c4d15f65d3bf73)
- [Disposable Taira-compatible development networks](records/operations-torii/undated-001.md#record-3457d6f720b76711d9d8bdfcb375d8e55b0a1141508e36425110b0850b31c8a3)
- [ZK-ACE JavaScript signed-transaction parity](records/crypto-proofs/undated-001.md#record-a58552b75091ca8a26b9037c9a7954ed3b9ce9866f82a94aee01d32e26ad9a31)
- [JavaScript governance private-file release closure](records/governance-identity/undated-001.md#record-b2ca62f7139af639dfda909e19d4365e7b818fab2c68e953d3ecde9f4a2d0025)
- [Sumeragi V2 production multilane release closure](records/consensus-network/undated-002.md#record-87736892db47251724036ec2be0bc9b3fff5b7f601514c947f7ccf0e8e7c83d5)
- [SoraFS V1 production closure](records/sorafs/undated-001.md#record-98e13eaa065dc4812321ef11d903a353e56e4572d79e2f042e4a9e8d201cb0c1)
- [Memory-containment follow-ups](records/architecture-build/undated-001.md#record-7dcf7aaac3cd3dbb150fb16d145e33d5cc280d4c43cc5050d844060e89be7f2d)
- [Peer transport V1 release evidence](records/kagemusha/undated-001.md#record-d4edf6497d1878ac4e12aad5c2ba8da0651a3aff4ce1cffa8cc617660a9431ad)
- [Wallet activity query follow-ups](records/sdks-native/undated-001.md#record-cad5e77c5167f14c4cbb855d2b1a0ef30e3ccb33eaee64fb6d2e69135c1a89c6)
- [Alias/SNS release evidence](records/governance-identity/undated-001.md#record-dda62b5c682c2832a296e048869628f6428f1db9493210eafd2423f91bb0f91a)
- [SORA Economic Constitution](records/governance-identity/undated-001.md#record-9c962731bc03e974d717fe460bd192092ea0c5a2b2edcef9ad9820467dac1a5c)
- [SCCP Launch Scope](records/sccp-settlement/undated-001.md#record-bce712e84d1feeaef947dff99073c93e97784084fe401195d4facae0cb0dd7fb)
- [Release and Stabilization](records/vm-norito-model/undated-001.md#record-29220a05cdb26c26b15390a5b2f77f6df96907b3ddb5d8207ccff70088916db7)
- [SORA Nexus and Taira](records/operations-torii/undated-001.md#record-546855dbada73ecf30e753da8ea22b52b8ac8a22bfb4a687aba263d079377279)
- [IVM, Kotodama, and Norito](records/vm-norito-model/undated-006.md#record-85bb8b5265b2647edd37606c16afa0e740080d8518243100a32d42fc0ffcd6cf)
- [Privacy, ZK, and FHE](records/crypto-proofs/undated-004.md#record-0dce200149da4e57e5c7e6a1177d493c46d48fdd0167721f3b24ab0a1f299599)
- [Consensus, Performance, and Operations](records/consensus-network/undated-006.md#record-ccdde6451ecd2eb52e38982c9e76d00fa6a2ea28913c8b8b610f5a7b50b6750c)
- [Community and Governance](records/governance-identity/undated-001.md#record-2dfd0444e926cae83058c4f14a41cf36542aa844bdf657b0132e7fd32e483015)

## Integrity and reconstruction

From the repository root:

```sh
python3 scripts/archive_project_history.py verify --archive docs/history/2026-09-06
python3 scripts/archive_project_history.py reconstruct --archive docs/history/2026-09-06 --output-dir /tmp/iroha-original-records
```

Reconstruction writes the exact original bytes, including original relative links.
The original roots are replaced only after their hashes still match this manifest.
