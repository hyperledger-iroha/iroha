<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bfbe7de97848ee43284d448f9d80b78f68b5e95d36e0f86d2aa12c3633838867
source_last_modified: "2025-11-07T10:28:53.296738+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "SoraFS 移行ロードマップ"
---

> 次の文書を基に作成: [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# SoraFS 移行ロードマップ (SF-1)

この文書は `docs/source/sorafs_architecture_rfc.md` に記載された移行ガイダンスを
運用可能な形に落とし込む。SF-1 の成果物を実行可能なマイルストーン、ゲート基準、
担当チェックリストに展開し、storage、governance、DevRel、SDK チームがレガシー
アーティファクトのホスティングから SoraFS ベースの公開への移行を調整できるようにする。

このロードマップは意図的に決定論的である。各マイルストーンは必要なアーティファクト、
コマンド実行、アテステーション手順を明示し、下流のパイプラインが同一の出力を生成し、
ガバナンスが監査可能な履歴を保持できるようにする。

## マイルストーン概要

| マイルストーン | 期間 | 主な目標 | 必須成果物 | 担当 |
|---------------|------|----------|-----------|------|
| **M0 - Bootstrap** | Weeks 1-6 | 決定論的な chunker fixtures を公開し、アーティファクトを二重公開 (legacy + SoraFS) する。 | `sorafs_chunker` fixtures、`sorafs_manifest_stub` CLI 統合、移行台帳エントリ。 | Docs, DevRel, Storage |
| **M1 - Deterministic Enforcement** | Weeks 7-12 | 署名済み fixtures を強制し、パイプラインが expectation flags を採用する間に alias proofs を staging する。 | Nightly fixture 検証、評議会署名の manifest、alias registry の staging エントリ。 | Storage, Governance, SDKs |
| **M2 - Registry First** | Weeks 13-20 | pins を registry 経由にし、legacy bundles を凍結し、parity テレメトリを公開する。 | Pin Registry コントラクト + CLI (`sorafs pin propose/approve`)、観測ダッシュボード、運用 runbook。 | Governance, Ops, Observability |
| **M3 - Alias Only** | Week 21+ | legacy hosting を廃止し、取得に alias proofs を要求する。 | alias-only gateways、parity アラート、SDK デフォルト更新、legacy 撤去通知。 | Ops, Networking, SDKs |

マイルストーンの状態は `docs/source/sorafs/migration_ledger.md` で追跡する。
このロードマップの変更は必ず台帳を更新し、ガバナンスとリリースエンジニアリングの
同期を維持すること。

## ワークストリーム

### 1. レガシーデータの再梱包

| 手順 | マイルストーン | 説明 | 担当 | 出力 |
|------|---------------|------|------|------|
| インベントリとタグ付け | M0 | レガシーバンドルの SHA3-256 digest をエクスポートし、移行台帳に追記 (append-only) する。 | Docs, DevRel | `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid` を含む台帳エントリ。 |
| 決定論的再構築 | M0-M1 | 各リリースアーティファクトで `sorafs_manifest_stub` を実行し、CAR、manifest、署名 envelope、fetch plan を `artifacts/<team>/<alias>/<timestamp>/` に保存する。 | Docs, CI | リリースごとの再現可能な CAR + manifest bundles。 |
| 検証ループ | M1 | `sorafs_fetch` を staging gateway に対して再生し、chunk 境界/ダイジェストが fixtures と一致するかを確認する。台帳コメントに pass/fail を記録。 | Governance QA | Staging 検証レポート + drift 用 GitHub issue。 |
| Registry 切替 | M2 | manifest digest が on-chain に存在したら台帳ステータスを `Pinned` に変更し、legacy bundle を read-only 化 (配信のみ、変更不可)。 | Governance, Ops | Registry トランザクション hash、legacy storage の read-only チケット。 |
| 廃止 | M3 | 30 日の猶予期間後に legacy CDN エントリを削除し、DNS 変更承認を保管し、post-mortem を公開する。 | Ops | 廃止チェックリスト、DNS 変更記録、インシデントチケットのクローズ。 |

### 2. 決定論的 pinning の採用

| 手順 | マイルストーン | 説明 | 担当 | 出力 |
|------|---------------|------|------|------|
| Fixture リハーサル | M0 | `fixtures/sorafs_chunker` とローカルの chunk digest を比較する週次 dry-run。`docs/source/sorafs/reports/` にレポートを公開。 | Storage Providers | pass/fail マトリクス付き `determinism-<date>.md`。 |
| 署名強制 | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` は署名/manifest の逸脱で fail。開発 overrides はガバナンスの waiver を PR に添付。 | Tooling WG | CI ログ、waiver チケットリンク (該当する場合)。 |
| Expectation flags | M1 | パイプラインは `sorafs_manifest_stub` に明示的な expectations を渡して出力を固定: | Docs CI | expectation flags を参照する更新済みスクリプト (下のコマンド参照)。 |
| Registry-first pinning | M2 | `sorafs pin propose` と `sorafs pin approve` が manifest 提出をラップし、CLI デフォルトは `--require-registry`。 | Governance Ops | Registry CLI 監査ログ、失敗提案のテレメトリ。 |
| Observability parity | M3 | Prometheus/Grafana ダッシュボードが registry manifests と chunk インベントリの差分でアラートを出し、ops の on-call に接続。 | Observability | ダッシュボードリンク、アラートルール ID、GameDay 結果。 |

#### 正規の公開コマンド

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

digest、サイズ、CID の値は、該当アーティファクトの移行台帳エントリに記録された
期待値へ置き換えること。

### 3. Alias 移行とコミュニケーション

| 手順 | マイルストーン | 説明 | 担当 | 出力 |
|------|---------------|------|------|------|
| Staging の alias proofs | M1 | Pin Registry の staging 環境で alias claims を登録し、manifests に Merkle proof を付与 (`--alias`)。 | Governance, Docs | manifest 隣の proof bundle + alias 名を記した台帳コメント。 |
| Dual DNS + 通知 | M1-M2 | legacy DNS と Torii/SoraDNS を並行運用し、オペレーターと SDK チャンネルに移行通知を公開。 | Networking, DevRel | アナウンスポスト + DNS 変更チケット。 |
| Proof enforcement | M2 | Gateways は新しい `Sora-Proof` ヘッダがない manifests を拒否し、CI に `sorafs alias verify` を追加して proof を取得する。 | Networking | Gateway 設定パッチ + 検証成功を示す CI 出力。 |
| Alias-only rollout | M3 | legacy DNS を削除し、SDK デフォルトを Torii/SoraDNS + alias proofs に更新し、rollback 窓口を文書化。 | SDK Maintainers, Ops | SDK リリースノート、ops runbook 更新、rollback 計画。 |

### 4. コミュニケーションと監査

- **台帳の規律:** 状態変更 (fixtures drift、registry 提出、alias 有効化) はすべて
  `docs/source/sorafs/migration_ledger.md` に日付付きで追記すること。
- **ガバナンス議事録:** Pin Registry 変更や alias ポリシーを承認した council セッションは
  本ロードマップと台帳の両方を参照すること。
- **外部コミュニケーション:** DevRel は各マイルストーンでステータス更新 (ブログ +
  changelog 抜粋) を公開し、決定論的保証と alias タイムラインを強調する。

## 依存関係とリスク

| 依存関係 | 影響 | 緩和策 |
|----------|------|--------|
| Pin Registry コントラクトの可用性 | M2 pin-first rollout を阻害。 | M2 前に replay テスト付きでコントラクトを staging し、回帰が解消するまで envelope fallback を維持。 |
| 評議会署名鍵 | manifest envelopes と registry 承認に必要。 | 署名セレモニーを `docs/source/sorafs/signing_ceremony.md` に記載し、オーバーラップ付きで鍵をローテーションし台帳に記録。 |
| Gateway parity tooling | alias proofs と chunk parity 強制に必要。 | M1 で gateway 更新を出し、M2 基準を満たすまで legacy 挙動を feature flag の背後に保持。 |
| SDK リリース・カデンス | クライアントは M3 前に alias proofs に従う必要。 | SDK リリース窓をマイルストーンゲートに揃え、リリーステンプレートに移行チェックリストを追加。 |

残余リスクと緩和策は `docs/source/sorafs_architecture_rfc.md` にも反映されているため、
変更時は相互参照すること。

## 終了基準チェックリスト

| マイルストーン | 基準 |
|---------------|------|
| M0 | - すべての対象アーティファクトを `sorafs_manifest_stub` + expectation flags で再構築。 <br /> - 各アーティファクト群で移行台帳が埋まっている。 <br /> - 二重公開 (legacy + SoraFS) が稼働中。 |
| M1 | - Nightly fixture job が 7 日連続で green。 <br /> - Staging alias proofs が CI で検証済み。 <br /> - ガバナンスが expectation flag ポリシーを承認。 |
| M2 | - 新規 manifests の 100% が Pin Registry 経由。 <br /> - Legacy storage は read-only 化され、incident playbook が承認済み。 <br /> - Observability ダッシュボードが稼働し、アラート閾値が設定済み。 |
| M3 | - Alias-only gateways が本番稼働。 <br /> - Legacy DNS が削除され、変更チケットに反映済み。 <br /> - SDK デフォルトが更新されリリース済み。 <br /> - 最終ステータスを移行台帳に追記。 |

## 変更管理

1. このファイル **および** `docs/source/sorafs/migration_ledger.md` を更新する PR で
   調整案を提案する。
2. PR 説明にガバナンス議事録と CI 証跡をリンクする。
3. マージ後、storage + DevRel メーリングリストへ概要とオペレーター向けアクションを通知する。

この手順を守ることで、SoraFS の rollout が決定論的・監査可能・透明であり続け、
Nexus ローンチに参加するチーム間での整合が保たれる。
