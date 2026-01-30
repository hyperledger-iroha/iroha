---
lang: fr
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2b0d1ce7934e2d7ef2f19156dd5ba9ab8e94bf51d4441652a12398cea62b2b19
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: developer-cli
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/developer/cli.md` を反映しています。レガシーの Sphinx セットが退役するまで両方を同期してください。
:::

統合された `sorafs_cli` サーフェス（`cli` feature を有効化した `sorafs_car` crate が提供）は、SoraFS アーティファクトの準備に必要なすべての手順を公開します。このクックブックで一般的なワークフローに直行し、運用コンテキストとして manifest パイプラインと orchestrator の runbook を併用してください。

## ペイロードのパッケージング

`car pack` を使って決定的な CAR アーカイブと chunk プランを生成します。ハンドルが指定されない限り、コマンドは自動的に SF-1 chunker を選択します。

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- デフォルトの chunker ハンドル: `sorafs.sf1@1.0.0`。
- ディレクトリ入力は辞書順に走査されるため、チェックサムはプラットフォーム間で安定します。
- JSON サマリーには payload digest、chunk ごとのメタデータ、レジストリとオーケストレーターが認識するルート CID が含まれます。

## マニフェストの構築

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` オプションは `sorafs_manifest::ManifestBuilder` の `PinPolicy` フィールドに直接マッピングされます。
- 送信前に chunk の SHA3 digest を再計算したい場合は `--chunk-plan` を指定します。指定しない場合はサマリーに埋め込まれた digest を再利用します。
- JSON 出力は Norito ペイロードを反映するため、レビュー時の diff が容易になります。

## 長期鍵なしでマニフェストを署名

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- インラインのトークン、環境変数、ファイルベースのソースを受け付けます。
- `--include-token=true` を指定しない限り、raw JWT を保存せずに provenance メタデータ（`token_source`、`token_hash_hex`、chunk digest）を追加します。
- CI で有効: `--identity-token-provider=github-actions` を設定して GitHub Actions の OIDC と組み合わせます。

## Torii へのマニフェスト送信

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- alias proof を Norito デコードし、Torii に POST する前に manifest digest と一致することを検証します。
- プランから chunk の SHA3 digest を再計算して mismatch 攻撃を防ぎます。
- レスポンスのサマリーには、後の監査のために HTTP ステータス、ヘッダー、レジストリの payload が含まれます。

## CAR 内容と proof の検証

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR ツリーを再構築し、payload digest を manifest サマリーと比較します。
- ガバナンスにレプリケーション proof を提出する際に必要な件数と識別子を記録します。

## proof テレメトリのストリーム

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- 各 proof のストリームに対して NDJSON を出力します（`--emit-events=false` でリプレイを無効化）。
- 成功/失敗の件数、レイテンシのヒストグラム、サンプリングされた失敗を JSON サマリーに集約し、ログをスクレイピングせずにダッシュボードで可視化できます。
- gateway が失敗を報告するか、ローカル PoR 検証（`--por-root-hex`）が proofs を拒否した場合は非ゼロで終了します。リハーサル実行では `--max-failures` と `--max-verification-failures` でしきい値を調整してください。
- 現在は PoR をサポートしています。PDP と PoTR は SF-13/SF-14 の導入後に同じエンベロープを再利用します。
- `--governance-evidence-dir` はレンダリング済みのサマリー、メタデータ（タイムスタンプ、CLI バージョン、gateway URL、manifest digest）、および manifest のコピーを指定ディレクトリに書き込み、ガバナンスパケットが proof-stream の証跡を再実行なしで保管できるようにします。

## 追加リソース

- `docs/source/sorafs_cli.md` — すべてのフラグを網羅したドキュメント。
- `docs/source/sorafs_proof_streaming.md` — proof テレメトリのスキーマと Grafana ダッシュボードのテンプレート。
- `docs/source/sorafs/manifest_pipeline.md` — chunking、manifest の構成、CAR 取扱いの詳細解説。
