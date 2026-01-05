---
lang: ja
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

# Connect セッションプレビュー・ランブック (IOS7 / JS4)

このランブックは、ロードマップの **IOS7** と **JS4** (`roadmap.md:1340`, `roadmap.md:1656`) に必要な Connect プレビューセッションの準備、検証、撤収までの手順をまとめる。Connect のストローマンをデモする場合 (`docs/source/connect_architecture_strawman.md`)、SDK ロードマップで約束したキュー/テレメトリのフックを検証する場合、または `status.md` 向けの証跡を集める場合は、この手順に従うこと。

## 1. 事前チェックリスト

| 項目 | 詳細 | 参照 |
|------|---------|------------|
| Torii エンドポイント + Connect ポリシー | Torii のベース URL、`chain_id`、Connect ポリシー (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) を確認する。JSON スナップショットをランブックのチケットに保存する。 | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| フィクスチャ + bridge のバージョン | Norito フィクスチャのハッシュと使用する bridge ビルドを記録する (Swift は `NoritoBridge.xcframework` が必要、JS は `bootstrapConnectPreviewSession` を含む `@iroha/iroha-js` のバージョン以上が必要)。 | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| テレメトリのダッシュボード | `connect.queue_depth`、`connect.queue_overflow_total`、`connect.resume_latency_ms`、`swift.connect.session_event` などを可視化するダッシュボードが到達可能であることを確認する (Grafana の `Android/Swift Connect` ボード + Prometheus エクスポートのスナップショット)。 | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| 証跡フォルダ | `docs/source/status/swift_weekly_digest.md` (週次ダイジェスト) と `docs/source/sdk/swift/connect_risk_tracker.md` (リスクトラッカー) のような保存先を選ぶ。ログ、メトリクスのスクリーンショット、承認記録を `docs/source/sdk/swift/readiness/archive/<date>/connect/` に保存する。 | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. プレビューセッションのブートストラップ

1. **ポリシーとクォータを検証する。** 次を実行する:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   `queue_max` または TTL が予定している設定と異なる場合は実行を失敗扱いにする。
2. **SID/URI を決定論的に生成する。** `@iroha/iroha-js` の `bootstrapConnectPreviewSession` ヘルパーは SID/URI の生成を Torii のセッション登録に結び付ける。Swift が WebSocket 層を担当する場合でも必ず使う。
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false` を指定して QR/deep-link の dry-run を行う。
   - 返ってきた `sidBase64Url`、deeplink URL、`tokens` の blob を証跡フォルダに保存する。ガバナンスのレビューではこれらの成果物が必要。
3. **シークレットを配布する。** deeplink URI をウォレット運用者 (Swift dApp サンプル、Android ウォレット、QA ハーネス) に共有する。トークンをチャットに貼り付けないこと。enablement packet に記載された暗号化済みの保管庫を使う。

## 3. セッションを駆動する

1. **WebSocket を開く。** Swift クライアントは通常次のように使う:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   追加の設定 (bridge の import、並行処理アダプタ) は `docs/connect_swift_integration.md` を参照。
2. **承認と署名フロー。** dApp は `ConnectSession.requestSignature(...)` を呼び、ウォレットは `approveSession` / `reject` で応答する。各承認は、Connect ガバナンスのチャーターに合わせてハッシュ化された alias と権限を記録すること。
3. **キューと再開経路を検証する。** ネットワーク接続を切り替えたりウォレットをサスペンドしたりして、制限付きキューと replay フックがエントリを記録することを確認する。JS/Android SDK はフレームを落とすときに `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` を出す。Swift でも IOS7 のキュー基盤が入ったら同じ挙動になるはず (`docs/source/connect_architecture_strawman.md`)。少なくとも 1 回の再接続を記録したら、
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   を実行し (または `ConnectSessionDiagnostics` が返すエクスポートディレクトリを渡し)、生成された表/JSON をランブックのチケットに添付する。CLI は `ConnectQueueStateTracker` が生成する `state.json` / `metrics.ndjson` のペアを読み取るため、ガバナンスのレビュー担当は特別なツールなしで drill の証跡を追える。

## 4. テレメトリと可観測性

- **収集するメトリクス:**
  - `connect.queue_depth{direction}` gauge (ポリシーの上限より下に保つ)。
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (障害注入時のみ非ゼロ)。
  - `connect.resume_latency_ms` histogram (再接続を強制した後の p95 を記録)。
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift 固有 `swift.connect.session_event` と `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **ダッシュボード:** Connect ボードのブックマークに注釈マーカーを追加する。キャプチャ (または JSON exports) を証跡フォルダに保存し、テレメトリエクスポータ CLI で取得した OTLP/Prometheus スナップショットも添付する。
- **アラート:** Sev 1/2 のしきい値が発火した場合 (`docs/source/android_support_playbook.md` の第5節)、SDK Program Lead に連絡し、PagerDuty のインシデント ID を続行前にランブックのチケットへ記録する。

## 5. クリーンアップとロールバック

1. **ステージングされたセッションを削除する。** キュー深度のアラームが意味を持ち続けるよう、プレビューセッションは必ず削除する:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Swift のみのテストでは、Rust/CLI helper から同じエンドポイントを呼ぶ。
2. **ジャーナルを削除する。** 永続化されたキュージャーナル (`ApplicationSupport/ConnectQueue/<sid>.to`、IndexedDB ストアなど) を削除し、次回の実行がクリーンに始まるようにする。replay 問題を調査する必要がある場合は、削除前にファイルのハッシュを記録する。
3. **インシデントのメモを残す。** 実行結果を次にまとめる:
   - `docs/source/status/swift_weekly_digest.md` (deltas ブロック),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (テレメトリが整備されたら CR-2 を解除または格下げ),
   - 新しい挙動が検証された場合は JS SDK の changelog かレシピ。
4. **失敗のエスカレーション:**
   - 障害注入なしのキューオーバーフロー => Torii とポリシーが食い違う SDK にバグを起票する。
   - 再開エラー => `connect.queue_depth` + `connect.resume_latency_ms` のスナップショットをインシデント報告に添付する。
   - ガバナンス不一致 (トークン再利用、TTL 超過) => SDK Program Lead に共有し、次回更新時に `roadmap.md` に注記する。

## 6. 証跡チェックリスト

| 成果物 | 保存先 |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| クリーンアップ確認 (Torii delete, journal wipe) | `.../cleanup.log` |

このチェックリストを満たすことで IOS7/JS4 の "docs/runbooks updated" 終了条件を満たし、ガバナンスのレビュー担当に対して各 Connect プレビューセッションの決定論的な証跡を提供できる。
