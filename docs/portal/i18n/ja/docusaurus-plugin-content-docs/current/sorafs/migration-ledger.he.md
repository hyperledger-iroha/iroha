---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c19db6518840b140ff18a34d988523dd513e47dbae9768ffeddd80545e8ad002
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: migration-ledger
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> 次の文書を基に作成: [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# SoraFS 移行台帳

この台帳は SoraFS Architecture RFC に記録された移行変更ログを反映する。
エントリはマイルストーンごとにまとめられ、適用期間、影響チーム、必要な
アクションを示す。移行計画の更新は、このページと RFC
(`docs/source/sorafs_architecture_rfc.md`) の両方を必ず更新し、下流の利用者との
整合を保つこと。

| マイルストーン | 有効期間 | 変更概要 | 影響チーム | アクション | 状態 |
|---------------|---------|---------|-----------|------------|------|
| M0 | Weeks 1–6 | Chunker fixtures を公開; pipelines が CAR + manifest バンドルをレガシー artefacts と並行して出力; 移行台帳のエントリを作成。 | Docs, DevRel, SDKs | 期待フラグ付きで `sorafs_manifest_stub` を採用し、この台帳にエントリを記録し、レガシー CDN を維持する。 | ✅ アクティブ |
| M1 | Weeks 7–12 | CI が決定論的 fixtures を強制; alias 証明が staging で利用可能; tooling が明示的な期待フラグを公開。 | Docs, Storage, Governance | fixtures の署名を維持し、staging registry に aliases を登録し、`--car-digest/--root-cid` 強制を release チェックリストに追加する。 | ⏳ 保留 |
| M2 | Weeks 13–20 | Registry ベースの pinning が主要経路に; レガシー artefacts は読み取り専用へ; gateways は registry 証明を優先。 | Storage, Ops, Governance | pinning を registry 経由にし、レガシー hosts を凍結し、オペレーター向け移行通知を公開する。 | ⏳ 保留 |
| M3 | Week 21+ | alias のみアクセスを強制; observability が registry パリティを警告; レガシー CDN を廃止。 | Ops, Networking, SDKs | レガシー DNS を削除し、キャッシュ URL をローテーションし、パリティダッシュボードを監視し、SDK のデフォルトを更新する。 | ⏳ 保留 |
| R0–R3 | 2025-03-31 → 2025-07-01 | Provider advert の強制フェーズ: R0 監視、R1 警告、R2 正規 handles/capabilities を強制、R3 レガシー payloads を削除。 | Observability, Ops, SDKs, DevRel | `grafana_sorafs_admission.json` を取り込み、`provider_advert_rollout.md` のオペレーターチェックリストに従い、R2 ゲートの 30+ 日前に advert 更新を前倒しで計画する。 | ⏳ 保留 |

これらのマイルストーンを参照するガバナンス制御プレーンの議事録は
`docs/source/sorafs/` 配下にある。チームは重要なイベントが発生した際に
(例: 新しい alias 登録、registry インシデントの振り返り) 各行の下に日付付き
箇条書きを追加し、監査可能な証跡を残す。

## 最近の更新

- 2025-11-01 — `migration_roadmap.md` をガバナンス評議会とオペレーターの
  メーリングリストに共有してレビュー中。次回評議会セッションで承認予定
  (参照: `docs/source/sorafs/council_minutes_2025-10-29.md` のフォローアップ)。
- 2025-11-02 — Pin Registry の register ISI が `sorafs_manifest` ヘルパーで
  chunker/ポリシーの共有検証を強制し、on-chain の経路を Torii チェックと
  整合させる。
- 2026-02-13 — provider advert の rollout フェーズ (R0–R3) を台帳に追加し、
  関連ダッシュボードとオペレーターガイドを公開
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
