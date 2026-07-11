---
lang: ja
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
translation_last_reviewed: 2026-07-12
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proof

この文書は初回リリースの bridge finality 形式を定義します。proof は
Sumeragi v2 が永続化した正確な finality evidence を運びます。proof envelope の
schema version は `1`、内部の consensus protocol version は `2` です。
Sumeragi v1 certificate への投影、decoder、fallback はありません。

## 正確な proof 形式

Norito または Norito JSON の `BridgeFinalityProof` は、次の 3 フィールドだけを
持ちます。

```text
{ version, block_header, finality_artifact }
```

- `version` は `1` でなければなりません。
- `block_header` は要求された高さの canonical `BlockHeader` です。
- `finality_artifact` はその block に対して保存された正確な
  `V2FinalityArtifact` です。height-context roster と同じ順序で、各 validator の
  BLS-normal PoP (`validator_set_pops`) を永続的に内包します。

artifact は、完全で不変な `HeightContext`、正確な `BlockSubject`、block hash、
CommitQC、および roster に対応する PoP を保持します。`HeightContext` は chain、
epoch、roster、`DualQuorum`、DA layout、leader seed などを固定します。epoch を
終了する親 block の context には任意の `next_epoch_snapshot` も含まれます。
snapshot は context id の一部なので、子 roster を許可する前に親 CommitQC によって
認証されます。finalized snapshot は次 epoch のパラメータに加え、`epoch_end_height` と
次 roster に対応する `validator_set_pops` も認証します。

## 永続化と検証

Sumeragi v2 の apply path は artifact を検証し、不変な Kura sidecar として保存します。
proof builder は canonical block とその sidecar を読み、現在の可変 world state から
過去の PoP や certificate を再構成しません。sidecar の欠落、破損、競合、検証失敗は
fail closed です。最近の in-memory history window による保持制限はありません。

stateless verifier は version、chain、高さ、header hash、canonical predecessor、view、
context、subject、CommitQC を
厳密に対応させ、artifact 内の全 PoP を検証します。signer index は昇順かつ範囲内で、
CommitQC は人数と voting power の両方の quorum を満たし、正確な Sumeragi v2 vote
preimage に対する BLS aggregate signature が有効でなければなりません。

## Trust anchor と successor

単独の proof は、proof が運ぶ roster の下での自己整合性だけを示します。
`BridgeFinalityVerifier` は最初の proof より前に、明示的に信頼された
`HeightContextId` を要求します。その後は直後の高さだけを受け入れ、子 context の
parent CommitQC を前の固定 roster と PoP で検証します。epoch 内では子 artifact が前の
artifact の PoP をコピーし、epoch 境界では epoch、roster、quorum、seed、PoP が、認証済み
`epoch_end_height` を含む前の親 context の `next_epoch_snapshot` と一致しなければなりません。
古い高さ、飛ばされた高さ、リンクされていない successor は拒否されます。

SCCP は同じ `BridgeFinalityProof` を使います。message が提供した roster の署名だけを
信頼せず、governance で固定した checkpoint context/artifact から message artifact まで
直後の successor chain を検証する必要があります。

## Bundle と API

`BridgeFinalityBundle` は正確に `{ commitment, finality_proof }` です。
commitment は正確に
`{ chain_id, height_context_id, block_height, block_hash }` です。

- `GET /v1/bridge/finality/{height}` は `BridgeFinalityProof` を返します。
- `GET /v1/bridge/finality/bundle/{height}` は `BridgeFinalityBundle` を返します。

block または正確な永続 v2 artifact が存在しないか無効なら、両 endpoint は fail closed
になります。未知の field、version、retired proof shape は拒否しなければなりません。
