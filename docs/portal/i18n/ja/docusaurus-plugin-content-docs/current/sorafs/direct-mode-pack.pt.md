---
lang: pt
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e200c21fceb03de598cf4c91ca306121afa2cdf2f5c0d7a00f8f34855804838
source_last_modified: "2025-11-07T10:33:21.917878+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正規ソース
このページは `docs/source/sorafs/direct_mode_pack.md` を反映しています。レガシーの Sphinx セットが退役するまで両方を同期してください。
:::

SoraNet 回線は SoraFS の既定トランスポートですが、ロードマップ項目 **SNNet-5a** では匿名化の展開が完了するまで、決定的な読み取りアクセスを維持できるよう規制されたフォールバックが必要になります。このパックは、CLI/SDK のノブ、設定プロファイル、コンプライアンステスト、デプロイチェックリストをまとめ、プライバシー転送を触らずに Torii/QUIC 直通モードで SoraFS を運用するための情報を提供します。

このフォールバックは SNNet-5 から SNNet-9 が準備ゲートを通過するまで、staging と規制対象の本番環境に適用されます。匿名モードと直通モードを必要に応じて切り替えられるよう、下記の成果物を通常の SoraFS デプロイ資料と一緒に保持してください。

## 1. CLI と SDK のフラグ

- `sorafs_cli fetch --transport-policy=direct-only ...` はリレーのスケジューリングを無効化し、Torii/QUIC トランスポートを強制します。CLI ヘルプには `direct-only` が許容値として表示されます。
- SDK は直通モードのトグルを公開する場合、`OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` を設定する必要があります。`iroha::ClientOptions` と `iroha_android` の生成バインディングも同じ enum を渡します。
- Gateway ハーネス (`sorafs_fetch`, Python バインディング) は共有 Norito JSON ヘルパー経由で direct-only トグルを解析できるため、オートメーションでも同一の挙動になります。

フラグはパートナー向けランブックに記載し、機能トグルは環境変数ではなく `iroha_config` 経由で配線してください。

## 2. Gateway ポリシープロファイル

Norito JSON を使って決定的なオーケストレータ設定を保持します。`docs/examples/sorafs_direct_mode_policy.json` の例プロファイルは次をエンコードします:

- `transport_policy: "direct_only"` — SoraNet リレーのみを広告するプロバイダを拒否します。
- `max_providers: 2` — 直通ピアを最も信頼できる Torii/QUIC エンドポイントに制限します。地域のコンプライアンス要件に合わせて調整してください。
- `telemetry_region: "regulated-eu"` — 発行メトリクスにラベルを付け、ダッシュボードと監査でフォールバック実行を区別できるようにします。
- 保守的なリトライ予算 (`retry_budget: 2`, `provider_failure_threshold: 3`) で、誤設定の gateway を隠さないようにします。

JSON は `sorafs_cli fetch --config` (自動化) または SDK バインディング (`config_from_json`) 経由で読み込み、オペレータに提示する前にポリシーを適用します。監査のため `persist_path` に scoreboard 出力を保存してください。

Gateway 側の強制ノブは `docs/examples/sorafs_gateway_direct_mode.toml` にまとめられています。このテンプレートは `iroha app sorafs gateway direct-mode enable` の出力を反映し、envelope/admission チェックの無効化、rate-limit の既定値の配線、計画由来のホスト名と manifest digest での `direct_mode` テーブルの充填を行います。構成管理にコミットする前にプレースホルダ値をロールアウト計画で置き換えてください。

## 3. コンプライアンステストスイート

直通モードの準備状況には、オーケストレータと CLI クレートの両方でのテストが含まれます:

- `direct_only_policy_rejects_soranet_only_providers` は `TransportPolicy::DirectOnly` が候補 advert が SoraNet リレーのみをサポートする場合に素早く失敗することを保証します。【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` は Torii/QUIC トランスポートが存在するときに使用され、SoraNet リレーがセッションから除外されることを保証します。【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` は `docs/examples/sorafs_direct_mode_policy.json` をパースし、ドキュメントがヘルパーと整合していることを保証します。【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` は `sorafs_cli fetch --transport-policy=direct-only` をモックの Torii gateway に対して実行し、直通トランスポートを固定する規制環境向けのスモークテストを提供します。【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` は同じコマンドをポリシー JSON と scoreboard 保存で包み、ロールアウト自動化に使います。

更新を公開する前にフォーカスしたスイートを実行してください:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

上流の変更で workspace のビルドが失敗する場合は、阻害エラーを `status.md` に記録し、依存が追いついたら再実行してください。

## 4. 自動スモークラン

CLI のカバレッジだけでは環境固有のリグレッション (例: gateway ポリシーのドリフトや manifest の不一致) を検出できません。専用のスモークヘルパーは `scripts/sorafs_direct_mode_smoke.sh` にあり、`sorafs_cli fetch` を直通モードのオーケストレータポリシー、scoreboard の永続化、サマリー収集でラップします。

使用例:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- スクリプトは CLI フラグと key=value 設定ファイルの両方を尊重します (`docs/examples/sorafs_direct_mode_smoke.conf` を参照)。実行前に manifest digest とプロバイダ advert エントリを本番値で埋めてください。
- `--policy` の既定は `docs/examples/sorafs_direct_mode_policy.json` ですが、`sorafs_orchestrator::bindings::config_to_json` が生成する任意のオーケストレータ JSON を指定できます。CLI は `--orchestrator-config=PATH` でポリシーを受け取り、手動でフラグを調整せずに再現可能な実行を可能にします。
- `sorafs_cli` が `PATH` にない場合、ヘルパーは `sorafs_orchestrator` クレート (release プロファイル) からビルドし、スモークランが出荷される直通モードの配線を検証します。
- 出力:
  - 組み立て済み payload (`--output`, 既定は `artifacts/sorafs_direct_mode/payload.bin`).
  - fetch サマリー (`--summary`, 既定は payload の隣) で、ロールアウト証跡に使うテレメトリリージョンとプロバイダレポートを含みます。
  - ポリシー JSON に宣言されたパスへ保存される scoreboard スナップショット (例: `fetch_state/direct_mode_scoreboard.json`). 変更チケットにはサマリーと一緒に保管してください。
- 採用ゲートの自動化: fetch 完了後、ヘルパーは保存された scoreboard と summary のパスを使って `cargo xtask sorafs-adoption-check` を実行します。必要クオラムはデフォルトでコマンドラインに渡したプロバイダ数です。より大きなサンプルが必要なら `--min-providers=<n>` で上書きしてください。採用レポートはサマリーの隣に書き出され (`--adoption-report=<path>` で場所を指定可能)、ヘルパーは既定で `--require-direct-only` (フォールバックに一致) と、該当フラグを渡した場合は `--require-telemetry` を付与します。追加の xtask 引数は `XTASK_SORAFS_ADOPTION_FLAGS` で渡してください (例: 承認済みのダウングレード中に `--allow-single-source` を渡し、ゲートがフォールバックを許容しつつ強制するようにする)。ローカル診断以外で `--skip-adoption-check` を使ってゲートをスキップしないでください。ロードマップでは、規制された直通モード実行ごとに採用レポートのバンドルが必須です。

## 5. ロールアウトチェックリスト

1. **構成フリーズ:** 直通モードの JSON プロファイルを `iroha_config` リポジトリに保存し、変更チケットにハッシュを記録します。
2. **Gateway 監査:** 直通モードへ切り替える前に Torii エンドポイントが TLS、能力 TLV、監査ログを強制していることを確認します。gateway ポリシープロファイルをオペレータに公開してください。
3. **コンプライアンス承認:** 更新したプレイブックをコンプライアンス/規制レビューアに共有し、匿名化オーバーレイ外での運用承認を記録します。
4. **ドライラン:** コンプライアンステストスイートと、信頼できる Torii プロバイダへの staging fetch を実行します。scoreboard 出力と CLI サマリーを保管してください。
5. **本番切替:** 変更ウィンドウを告知し、(既に `soranet-first` を選んでいる場合は) `transport_policy` を `direct_only` に切り替え、直通モードのダッシュボード (`sorafs_fetch` のレイテンシ、プロバイダ失敗カウンタ) を監視します。`roadmap.md:532` の SNNet-4/5/5a/5b/6a/7/8/12/13 が卒業したら SoraNet-first に戻せるよう、ロールバック計画を文書化してください。
6. **変更後レビュー:** scoreboard スナップショット、fetch サマリー、監視結果を変更チケットに添付します。`status.md` に有効化日と異常を記録してください。

チェックリストは `sorafs_node_ops` ランブックと並べて保管し、オペレータが本番切替前に手順を練習できるようにします。SNNet-5 が GA に到達したら、運用テレメトリの同等性を確認してフォールバックを退役させてください。

## 6. 証跡と採用ゲート要件

直通モードのキャプチャでも SF-6c 採用ゲートを満たす必要があります。scoreboard、summary、manifest envelope、採用レポートを各実行で束ね、`cargo xtask sorafs-adoption-check` がフォールバック姿勢を検証できるようにしてください。欠落フィールドがあるとゲートが失敗するため、変更チケットに期待メタデータを記録してください。

- **トランスポートメタデータ:** `scoreboard.json` は `transport_policy="direct_only"` を宣言する必要があります (ダウングレードを強制した場合は `transport_policy_override=true` に切り替える)。既定値を継承する場合でも匿名性ポリシーフィールドのペアを埋めて、段階的な匿名性計画からの逸脱が見えるようにします。
- **プロバイダカウンタ:** gateway-only セッションでは `provider_count=0` を保持し、Torii プロバイダ数を `gateway_provider_count=<n>` に設定します。JSON を手で編集しないでください。CLI/SDK がカウントを導出し、採用ゲートは分割がないキャプチャを拒否します。
- **manifest 証跡:** Torii gateway が関与する場合、署名済みの `--gateway-manifest-envelope <path>` (または SDK 相当) を渡し、`gateway_manifest_provided` と `gateway_manifest_id`/`gateway_manifest_cid` が `scoreboard.json` に記録されるようにします。`summary.json` にも同じ `manifest_id`/`manifest_cid` を含めてください。どちらかが欠けていると採用チェックは失敗します。
- **テレメトリ要件:** テレメトリが同伴する場合は `--require-telemetry` を付けてゲートを実行し、採用レポートがメトリクス送信を証明できるようにします。エアギャップのリハーサルではフラグを省略できますが、CI と変更チケットでは不在を記録してください。

例:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

`adoption_report.json` を scoreboard、summary、manifest envelope、スモークログバンドルと一緒に添付してください。これらの成果物は CI の採用ジョブ (`ci/check_sorafs_orchestrator_adoption.sh`) が強制する内容を反映し、直通モードのダウングレードを監査可能にします。
