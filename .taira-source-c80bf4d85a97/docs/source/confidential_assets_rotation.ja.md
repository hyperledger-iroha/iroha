---
lang: ja
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M3` によって参照される機密資産ローテーション プレイブック。

# 機密資産のローテーション ランブック

このプレイブックでは、オペレーターが機密資産をスケジュールおよび実行する方法について説明します
ローテーション (パラメータ セット、キーの検証、およびポリシーの移行)
ウォレット、Torii クライアント、およびメモリプール ガードが決定的なままであることを保証します。

## ライフサイクルとステータス

機密パラメータセット (`PoseidonParams`、`PedersenParams`、キーの検証)
特定の高さでの有効ステータスを取得するために使用されるラティスとヘルパーが存在します。
`crates/iroha_core/src/state.rs:7540` ～ `7561`。ランタイムヘルパーのスイープ保留中
目標の高さに達するとすぐに移行し、失敗を後で記録します。
再放送 (`crates/iroha_core/src/state.rs:6725` ～ `6765`)。

資産ポリシーの埋め込み
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
そのため、ガバナンスは次の方法でアップグレードをスケジュールできます。
`ScheduleConfidentialPolicyTransition` を選択し、必要に応じてそれらをキャンセルします。参照
`crates/iroha_data_model/src/asset/definition.rs:320` および Torii DTO ミラー
(`crates/iroha_torii/src/routing.rs:1539` ～ `1580`)。

## 回転ワークフロー

1. **新しいパラメータ バンドルを公開します。** オペレータが送信します。
   `PublishPedersenParams`/`PublishPoseidonParams` 命令 (CLI
   `iroha app zk params publish ...`) メタデータを使用して新しいジェネレーター セットをステージングします。
   アクティブ化/非推奨ウィンドウ、およびステータス マーカー。遺言執行者が拒否
   重複した ID、増加しないバージョン、または不正なステータス遷移
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499` ～ `2635`、および
   レジストリ テストでは、障害モード (`crates/iroha_core/tests/confidential_params_registry.rs:93` ～ `226`) がカバーされます。
2. **キー更新の登録/検証** `RegisterVerifyingKey` はバックエンドを強制します。
   キーが入力される前に、コミットメント、および回線/バージョンの制約が適用されます。
   レジストリ (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067` ～ `2137`)。
   キーを更新すると、古いエントリが自動的に非推奨になり、インライン バイトが消去されます。
   `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` によって行使されます。
3. **資産ポリシーの移行をスケジュールします。** 新しいパラメータ ID が有効になったら、
   ガバナンスは、希望の内容で `ScheduleConfidentialPolicyTransition` を呼び出します。
   モード、移行ウィンドウ、および監査ハッシュ。遺言執行者は抵触を拒否する
   透明性の高い供給を伴う移行または資産。などのテスト
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` を確認してください
   中止された遷移は `pending_transition` をクリアしますが、
   `confidential_policy_transition_reaches_shielded_only_on_schedule` で
   行 385 ～ 433 は、スケジュールされたアップグレードが正確に `ShieldedOnly` に切り替わることを確認します。
   有効高さ。
4. **ポリシー アプリケーションとメモリプール ガード。** ブロック エグゼキュータは保留中のすべてをスイープします。
   各ブロックの開始時に遷移し (`apply_policy_if_due`)、出力します。
   移行が失敗した場合にテレメトリが送信されるため、オペレーターはスケジュールを変更できます。入学時
   mempool は、有効なポリシーがブロックの途中で変更されるトランザクションを拒否します。
   移行期間全体にわたって決定的な包含を保証する
   (`docs/source/confidential_assets.md:60`)。

## ウォレットと SDK の要件- Swift およびその他のモバイル SDK は、アクティブなポリシーを取得するための Torii ヘルパーを公開します。
  さらに保留中の移行も含まれるため、ウォレットは署名前にユーザーに警告できます。参照
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) および関連する
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591` でテストします。
- CLI は、`iroha ledger assets data-policy get` (ヘルパー
  `crates/iroha_cli/src/main.rs:1497`–`1670`)、オペレーターが
  ポリシー/パラメータ ID をアセット定義に関連付けます。
  ブロックストア。

## テストとテレメトリーの範囲

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` はそのポリシーを検証します
  トランジションはメタデータ スナップショットに伝播され、適用されるとクリアされます。
- `crates/iroha_core/tests/zk_dedup.rs:1` は、`Preverify` キャッシュであることを証明します。
  二重支出/二重プルーフを拒否します。これには、ローテーション シナリオも含まれます。
  コミットメントが異なります。
- `crates/iroha_core/tests/zk_confidential_events.rs` および
  `zk_shield_transfer_audit.rs` エンドツーエンドのシールドをカバー → 転送 → シールド解除
  フローを実行し、パラメータのローテーションが行われても監査証跡が確実に残るようにします。
- `dashboards/grafana/confidential_assets.json` および
  `docs/source/confidential_assets.md:401` CommitmentTree を文書化し、
  すべてのキャリブレーション/ローテーションの実行に伴う検証キャッシュ ゲージ。

## ランブックの所有権

- **DevRel / ウォレット SDK リード:** SDK スニペットと以下を示すクイックスタートを維持します。
  保留中のトランジションを表面化し、ミント→転送→公開をリプレイする方法
  ローカルでテストします (`docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` で追跡されます)。
- **プログラム管理 / 機密資産 TL:** 移行リクエストを承認し、維持します
  `status.md` は今後のローテーションで更新され、免除 (存在する場合) が確実に行われます。
  校正台帳と一緒に記録されます。