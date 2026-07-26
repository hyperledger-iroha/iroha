---
lang: ja
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN 認証とローテーション

このノートは、Secret Data Node (SDN) 証明書の適用モデルをキャプチャします。
Jurisdiction Data Guardian (JDG) フローによって使用されます。

## コミットメントの形式
- `JdgSdnCommitment` はスコープ (`JdgAttestationScope`) をバインドします。
  ペイロード ハッシュ、および SDN 公開キー。印鑑はタイプされた署名です
  (`SignatureOf<JdgSdnCommitmentSignable>`) ドメインタグ付きペイロード上
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`。
- 構造検証 (`validate_basic`) は以下を強制します。
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - 有効なブロック範囲
  - 空ではないシール
  - 経由で実行する場合の証明書に対するスコープの等価性
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- 重複排除はアテステーションバリデーター (署名者+ペイロードハッシュ) によって処理されます。
  一意性）を使用して、コミットメントの保留/重複を防ぎます。

## レジストリとローテーション ポリシー
- SDN キーは `JdgSdnRegistry` に存在し、`(Algorithm, public_key_bytes)` によってキー化されます。
- `JdgSdnKeyRecord` は、アクティベーション高さ、オプションのリタイア高さ、
  およびオプションの親キー。
- 回転は `JdgSdnRotationPolicy` によって管理されます (現在: `dual_publish_blocks`)
  オーバーラップウィンドウ)。子キーを登録すると、親のリタイアメントが更新されます。
  `child.activation + dual_publish_blocks`、ガードレール付き:
  - 行方不明の両親は拒否されます
  - アクティベーションは確実に増加している必要があります
  - 猶予期間を超えるオーバーラップは拒否されます。
- レジストリ ヘルパーは、インストールされているレコード (`record`、`keys`) をステータスとして表示します。
  そしてAPIの公開。

## 検証フロー
- `JdgAttestation::validate_with_sdn_registry` は構造をラップします
  構成証明チェックと SDN 強制。 `JdgSdnPolicy` スレッド:
  - `require_commitments`: PII/秘密ペイロードの存在を強制します
  - `rotation`: 親の退職を更新するときに使用される猶予期間
- 各コミットメントは以下についてチェックされます。
  - 構造的妥当性 + 証明範囲の一致
  - 登録されたキーの存在
  - 証明されたブロック範囲をカバーするアクティブ ウィンドウ (すでにリタイア境界
    二重公開猶予を含む)
  - ドメインタグ付きコミットメント本文に有効なシールを貼る
- 安定したエラーは、オペレーターの証拠のインデックスとして表面化します。
  `MissingSdnCommitments`、`UnknownSdnKey`、`InactiveSdnKey`、`InvalidSeal`、
  または構造的な `Commitment`/`ScopeMismatch` 障害。

## オペレーターのランブック
- **プロビジョニング:** 最初の SDN キーを `activated_at` に登録するか、その前に登録します。
  第一秘密ブロックの高さ。キーのフィンガープリントを JDG オペレーターに公開します。
- **Rotate:** 後継キーを生成し、`rotation_parent` に登録します。
  現在のキーをポイントし、親のリタイアメントが等しいことを確認します。
  `child_activation + dual_publish_blocks`。ペイロードコミットメントを再封印する
  オーバーラップウィンドウ中のアクティブなキー。
- **監査:** Torii/status 経由でレジストリ スナップショット (`record`、`keys`) を公開します。
  表面に表示されるため、監査人はアクティブなキーとリタイア期間を確認できます。アラート
  証明された範囲がアクティブなウィンドウの外側にある場合。
- **回復:** `UnknownSdnKey` → レジストリにシーリング キーが含まれていることを確認します。
  `InactiveSdnKey` → 起動の高さを回転または調整します。 `InvalidSeal` →
  ペイロードを再封印し、証明書を更新します。## ランタイムヘルパー
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) ポリシーをパッケージ化 +
  レジストリを調べ、`validate_with_sdn_registry` 経由で証明書を検証します。
- レジストリは、Norito でエンコードされた `JdgSdnKeyRecord` バンドルからロードできます (「
  `JdgSdnEnforcer::from_reader`/`from_path`) または
  `from_records`、登録中に回転ガードレールを適用します。
- オペレーターは、Torii/status の証拠として Norito バンドルを永続化できます。
  同じペイロードが入場時に使用される執行者に供給される間、浮上し、
  コンセンサスガード。単一のグローバル エンフォーサは、起動時に次のように初期化できます。
  `init_enforcer_from_path`、および `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  status/Torii サーフェスのライブ ポリシー + キー レコードを公開します。

## テスト
- `crates/iroha_data_model/src/jurisdiction.rs` の回帰カバレッジ:
  `sdn_registry_accepts_active_commitment`、`sdn_registry_rejects_unknown_key`、
  `sdn_registry_rejects_inactive_key`、`sdn_registry_rejects_bad_signature`、
  `sdn_registry_sets_parent_retirement_window`、
  `sdn_registry_rejects_overlap_beyond_policy`、既存の
  構造証明/SDN 検証テスト。