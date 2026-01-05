---
lang: ja
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7236dfe86175ff89f660be4cb4dd2c90df20a05f9606d2707407465f639b1a1c
source_last_modified: "2025-12-05T06:21:36.529838+00:00"
translation_last_reviewed: 2026-01-01
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

このドキュメントは Iroha の初期 bridge finality proof サーフェスを説明します。
目的は、外部チェーンや light client が、オフチェーン計算や信頼されたリレーなしで
Iroha ブロックが final であることを検証できるようにすることです。

## Proof 形式

`BridgeFinalityProof` (Norito/JSON) は以下を含みます:

- `height`: ブロック高。
- `chain_id`: クロスチェーンリプレイを防ぐための Iroha chain identifier。
- `block_header`: canonical `BlockHeader`。
- `block_hash`: header の hash (クライアントが再計算して検証)。
- `commit_certificate`: ブロックを確定させた validator set + signatures。

Proof は自己完結です。外部 manifest や不透明な blob は不要です。
Retention: Torii は直近の commit-certificate ウィンドウに対して finality proof を提供します
(設定された history cap により制限。デフォルト 512 件で
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP` で設定)。
長い期間が必要な場合はクライアント側で proof をキャッシュ/アンカーしてください。
canonical tuple は `(block_header, block_hash, commit_certificate)` です。header の hash は
commit certificate 内の hash と一致する必要があり、chain id が proof を単一の ledger に束縛します。
サーバは certificate が異なる block hash を指している場合 `CommitCertificateHashMismatch` を
拒否してログに記録します。

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) は基本 proof を明示的な commitment と justification で拡張します:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: commitment payload に対する authority set の署名
  (commit certificate の署名を再利用)。
- `block_header`, `commit_certificate`: 基本 proof と同じ。

現状の placeholder: `mmr_root`/`mmr_peaks` は block-hash MMR をメモリ上で再計算して導出します。
inclusion proofs はまだ返されません。現時点でも commitment payload で同じ hash を検証できます。

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON)。

検証は基本 proof と同様です: header から `block_hash` を再計算し、commit certificate の署名を検証し、
commitment フィールドが certificate と block hash に一致することを確認します。bundle は bridge
プロトコルが分離を好む場合の commitment/justification ラッパーを追加します。

## 検証手順

1. `block_header` から `block_hash` を再計算し、不一致なら拒否。
2. `commit_certificate.block_hash` が再計算した `block_hash` と一致することを確認。
   不一致の header/commit certificate ペアは拒否。
3. `chain_id` が期待する Iroha chain と一致することを確認。
4. `commit_certificate.validator_set` から `validator_set_hash` を再計算し、記録された hash/version と一致することを確認。
5. commit certificate の署名を header hash に対して検証。validator 公開鍵と index を使用し、
   quorum (`n>3` のとき `2f+1`、それ以外は `n`) を適用し、重複/範囲外 index を拒否。
6. 任意で validator set hash をアンカー値と比較し、trusted checkpoint に結び付ける (weak-subjectivity anchor)。
7. 任意で epoch anchor を期待値に合わせ、意図的に rotation されるまで古い/新しい epoch の proof を拒否。

`BridgeFinalityVerifier` (`iroha_data_model::bridge` 内) はこれらのチェックを適用し、
quorum 計数の前に chain-id/height drift、validator-set hash/version の不一致、
重複/範囲外の署名者、無効署名、予期しない epoch を拒否するため、light client は
単一の verifier を再利用できます。

## Reference verifier

`BridgeFinalityVerifier` は期待する `chain_id` と任意の validator-set/epoch anchors を受け取ります。
header/block-hash/commit-certificate の tuple を強制し、validator-set hash/version を検証し、
広告された validator roster に対して署名/quorum を検証し、最新 height を追跡して stale/skip proof を拒否します。
anchors がある場合は `UnexpectedEpoch`/`UnexpectedValidatorSet` で epoch/roster 間の replay を拒否します。
anchors がない場合は最初の proof の validator-set hash と epoch を採用し、その後は重複/範囲外/
不十分な署名に対して決定論的なエラーで拒否を続けます。

## API サーフェス

- `GET /v1/bridge/finality/{height}` - 指定のブロック高に対する `BridgeFinalityProof` を返します。
  `Accept` による content negotiation で Norito または JSON をサポートします。
- `GET /v1/bridge/finality/bundle/{height}` - 指定のブロック高に対する `BridgeFinalityBundle`
  (commitment + justification + header/certificate) を返します。

## Notes and follow-ups

- Proof は現在、保存された commit certificate から導出されます。履歴は commit certificate の retention
  ウィンドウに従って制限されるため、長期が必要ならクライアント側で anchor proof をキャッシュしてください。
  ウィンドウ外の要求は `CommitCertificateNotFound(height)` を返すので、エラーを表示してアンカー済み
  checkpoint にフォールバックしてください。
- `block_hash` の不一致 (header と certificate) を持つ replay/偽造 proof は `CommitCertificateHashMismatch`
  で拒否されます。クライアントも署名検証の前に同じ tuple チェックを行い、不一致 payload を破棄すべきです。
- 将来的には MMR/authority-set の commitment chain を追加して、長い履歴での proof サイズを削減できます。
  フォーマットは commit certificate をより豊かな commitment envelope に包むことで後方互換を維持します。
