---
lang: ja
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CLI & SDK - リリースノート (v0.1.0)

## ハイライト
- `sorafs_cli` はパッケージングパイプライン全体 (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) をラップするようになり、
  CI ランナーは個別のヘルパーではなく単一のバイナリを呼び出します。新しい keyless
  署名フローはデフォルトで `SIGSTORE_ID_TOKEN` を使い、GitHub Actions の OIDC プロバイダを
  理解し、署名バンドルと並んで決定的な JSON サマリを出力します。
- マルチソース fetch の *scoreboard* は `sorafs_car` の一部として提供されます。
  プロバイダのテレメトリを正規化し、能力ペナルティを適用し、JSON/Norito レポートを永続化し、
  共有 registry handle を通じてオーケストレータシミュレータ (`sorafs_fetch`) に供給します。
  `fixtures/sorafs_manifest/ci_sample/` のフィクスチャは CI/CD が差分比較すべき決定的な
  入出力を示します。
- リリース自動化は `ci/check_sorafs_cli_release.sh` と `scripts/release_sorafs_cli.sh` に
  明文化されています。各リリースは manifest bundle、署名、`manifest.sign/verify` のサマリ、
  scoreboard snapshot をアーカイブし、ガバナンスレビューアがパイプライン再実行なしで
  アーティファクトを追跡できるようにします。

## 互換性
- 破壊的変更: **なし。** CLI 追加はすべて追加的なフラグ/サブコマンドで、既存の呼び出しは
  変更不要です。
- 最小 gateway/node バージョン: Torii `2.0.0-rc.2.0` (またはそれ以降) が必要です。
  `crates/iroha_torii` が公開する chunk-range API、stream-token のクォータ、能力ヘッダを
  利用できるようにするためです。ストレージノードは commit
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` の SoraFS host stack を使用してください
  (scoreboard の新しい入力とテレメトリ配線を含みます)。
- 上流依存: ワークスペース基準以外のサードパーティ更新はありません。リリースは
  `Cargo.lock` に固定された `blake3`, `reqwest`, `sigstore` を再利用します。

## アップグレード手順
1. ワークスペース内の対象 crates を更新します:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. ローカル (または CI) でリリースゲートを再実行し、fmt/clippy/test のカバレッジを確認します:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. 厳選された config で署名済みアーティファクトとサマリを再生成します:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   リリースが canonical fixtures を更新する場合は、更新済み bundles/proofs を
   `fixtures/sorafs_manifest/ci_sample/` にコピーしてください。

## 検証
- リリースゲートのコミット: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` をゲート成功直後に取得)。
- `ci/check_sorafs_cli_release.sh` の出力: `artifacts/sorafs_cli_release/v0.1.0/ci-check.log`
  にアーカイブ (release bundle に添付)。
- マニフェストバンドルのダイジェスト: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Proof summary ダイジェスト: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- マニフェストダイジェスト (downstream の attestation cross-check 用):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json` から取得)。

## オペレーター向けメモ
- Torii gateway は `X-Sora-Chunk-Range` capability header を適用するようになりました。
  新しい stream token scopes を提示するクライアントが通るよう allowlists を更新してください。
  range claim のない古いトークンは throttled されます。
- `scripts/sorafs_gateway_self_cert.sh` はマニフェスト検証を統合します。self-cert harness を
  実行する際は、生成したばかりの manifest bundle を渡し、署名 drift があれば wrapper が
  即時に失敗するようにしてください。
- テレメトリダッシュボードは新しい scoreboard export (`scoreboard.json`) を取り込み、
  プロバイダの適格性、重み付け、拒否理由を整合させてください。
- 各 rollout で以下の 4 つの canonical summaries をアーカイブします:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. ガバナンスチケットはこれらの正確なファイルを参照します。

## 謝辞
- Storage Team - CLI の end-to-end 統合、chunk-plan renderer、scoreboard telemetry 配線。
- Tooling WG - release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) と deterministic fixtures bundle。
- Gateway Operations - capability gating、stream-token ポリシーのレビュー、更新された
  self-cert playbooks。
