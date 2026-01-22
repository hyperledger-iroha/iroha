---
lang: ja
direction: ltr
source: docs/source/sorafs_gateway_dns_design_attendance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20e4dbd95067574ead4e1e9afc8875739e83813b2ff6b7b1e4850655906021c5
source_last_modified: "2025-11-20T07:16:03.636926+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_gateway_dns_design_attendance.md -->

# 出席トラッカー

会議: **SoraFS Gateway & DNS Design Kickoff**（2025-03-03 16:00 UTC）

## ステータス概要

- 招待は 2025-02-21 に `docs/source/sorafs_gateway_dns_design_invite.txt` を使って送付。
- 返信期限は **2025-02-26**。以下のステータスは確認が届き次第更新。
- **2025-02-26** までに全員の確認が揃い、キックオフ前に RSVP リストをクローズした。

## 回答ログ

| 役割 | 連絡先 | ステータス | メモ／フォローアップ |
|------|--------|------------|-----------------------|
| Networking TL（ファシリテーター） | `networking.tl@soranet` | ✅ 確認済み 2025-02-21 | アジェンダ担当。ブリッジを 10 分前に開く。 |
| Ops リード | `ops.lead@soranet` | ✅ 確認済み 2025-02-23 | ロールアウトのランブックを担当。事前読込用デッキを依頼済み。 |
| Storage チーム代表 | `storage.rep@sorafs` | ✅ 確認済み 2025-02-21 | 最新の chunker フィクスチャ状況を持参。 |
| Tooling WG 代表 | `tooling.wg@sorafs` | ✅ 確認済み 2025-02-21 | コンフォーマンス・ハーネスの更新を準備中。 |
| ガバナンス連絡役 | `governance@sora` | ✅ 確認済み（代理）2025-02-24 | 代理: `governance.alt@sora`。本担当は OOO のまま。 |
| QA ギルドリード | `qa.guild@sorafs` | ✅ 確認済み 2025-02-21 | 会議前のテレメトリ・スナップショットが必要。 |
| Docs/DevRel オブザーバー | `docs.devrel@sora` | ✅ 確認済み 2025-02-21 | 共有メモ文書をドラフト中。 |
| Torii プラットフォーム代表 | `torii.platform@soranet` | ✅ 確認済み 2025-02-21 | GAR メトリクスのエクスポートを収集中。 |
| セキュリティエンジニアリング オブザーバー | `security@soranet` | ✅ 確認済み 2025-02-25 | スライドデッキは 2025-02-24 に共有済み。リモート参加。 |

## DNS 自動化オーナー

ロードマップ項目 **Decentralized DNS & Gateway** は、キックオフ前に自動化オーナーを
指名することを求めている。以下の表はスコープ、責任リード、バックアップを記録し、
フォローアップタスクがリポジトリ内のコード／資産に直接紐づくようにする。

| スコープ | 主担当 | バックアップ | 責務 |
|---------|--------|--------------|------|
| SoraDNS ゾーンファイル自動化 & GAR ピン留め | Ops リード（`ops.lead@soranet`） | Networking TL（`networking.tl@soranet`） | `tools/soradns-resolver/` の自動化を維持し、`docs/source/sns/governance_playbook.md` に記載された署名済みゾーンファイルのスケルトンを公開し、各カットオーバー前に SPKI/GAR データをローテーションする。 |
| ゲートウェイ別名 & SoraFS 切替メタデータ | Tooling WG 代表（`tooling.wg@sorafs`） | Docs/DevRel（`docs.devrel@sora`） | `docs/portal/scripts/sorafs-pin-release.sh` と `docs/portal/scripts/generate-dns-cutover-plan.mjs`（テストは `docs/portal/scripts/__tests__/dns-cutover-plan.test.mjs`）を運用し、生成したマニフェストを `docs/portal/docs/devportal/deploy-guide.md` に添付し、別名変更を pin registry に告知する。 |
| テレメトリ・スナップショット & ロールバック自動化 | QA ギルドリード（`qa.guild@sorafs`） | セキュリティエンジニアリング（`security@soranet`） | GAR メトリクス（`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`, `docs/source/sorafs_gateway_dns_design_metrics_*.prom`）を収集・保存し、アラートフックがキックオフのダッシュボードに接続されたままであることを確認し、GA 前にセキュリティ担当とロールバック手順をリハーサルする。 |

## フォローアップアクション

1. **デッキ配布** — 2025-02-27 までに最終スライドを全員へ配布（Networking TL）。
2. **代理向けブリーフ** — セッション前にガバナンス代理へ DNS ポリシー付録を共有（Docs/DevRel）。
3. **セッション録画の段取り** — Docs/DevRel が共有メモ文書と録画チェックリストを準備。
4. **自動化オーナーの同期** — 上記オーナーが 2025-03-04 までにランブックのチェックポイント
   （ゾーンファイル公開、別名昇格、テレメトリ採取）を確認し、キックオフの責任分界を明確化。
5. **オーナー向けランブック** — 共有 SOP は `docs/source/sorafs_gateway_dns_owner_runbook.md` に格納。
   キックオフのデッキと一緒に配布し、リハーサル進捗の追跡に参照する。
6. **クローズアウト確認** — すべてのフォローアップは `docs/source/sorafs_gateway_dns_design_minutes.md`
   の 2025-03-04 クローズアウト記録に完了として記載済み。

## カレンダーと成果物

- カレンダー招待は 2025-02-25 に Google Calendar で送付。イベントリンク:
  `https://calendar.google.com/calendar/event?eid=c29yYWZzLWdhdGV3YXktZG5zLTIwMjUwMzAz`（SoraNet 内部）。
- 共有メモ文書のスタブ: `docs/source/sorafs_gateway_dns_design_agenda.md`
  （セッション後にアクションレジスタを追記）。
