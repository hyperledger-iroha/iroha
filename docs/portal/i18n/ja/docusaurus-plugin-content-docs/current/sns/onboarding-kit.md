---
lang: ja
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SNS Metrics & Onboarding Kit

ロードマップ項目 **SN-8** には 2 つの約束が含まれます:

1. `.sora`, `.nexus`, `.dao` の登録、更新、ARPU、紛争、freeze window を可視化する dashboards を公開する。
2. どの suffix も公開前に DNS/価格/API を統一的に接続できるよう、registrars と stewards 向けの onboarding kit を提供する。

このページは
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
の内容を反映し、外部レビューが同じ手順で確認できるようにしています。

## 1. Metric bundle

### Grafana dashboard と portal embed

- `dashboards/grafana/sns_suffix_analytics.json` を Grafana (または別の analytics host) に
  標準 API でインポートします:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- 同じ JSON がポータルページの iframe (**SNS KPI Dashboard**) に使われます。
  dashboard を更新したら `docs/portal` で
  `npm run build && npm run serve-verified-preview` を実行し、Grafana と embed の同期を確認してください。

### Panels と evidence

| Panel | Metrics | Governance evidence |
|-------|---------|---------------------|
| Registrations & renewals | `sns_registrar_status_total` (success + renewal resolver labels) | suffix ごとの throughput + SLA トラッキング。 |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance が registrar manifests と revenue を照合できる。 |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | freeze の状況、仲裁の頻度、guardian の負荷を可視化。 |
| SLA/error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | API 回帰を顧客影響前に検知。 |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, `manifest_id` labels の payment metrics | CSV drops と settlement tickets を結びつける。 |

月次 KPI レビューでは Grafana (または iframe) から PDF/CSV を出力し、
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md` の該当 annex に添付します。
また stewards は `docs/source/sns/reports/` に SHA-256 を記録します
(例: `steward_scorecard_2026q1.md`)。これにより audit が evidence path を再現できます。

### Annex automation

Dashboard export から直接 annex ファイルを生成し、レビュー向けの要約を統一します:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- helper は export を hash し、UID/tags/panel count を記録して
  `docs/source/sns/reports/.<suffix>/<cycle>.md` に Markdown annex を出力します。
- `--dashboard-artifact` は export を
  `artifacts/sns/regulatory/<suffix>/<cycle>/` にコピーし、annex の evidence path を固定します。
  外部アーカイブを使う場合のみ `--dashboard-label` を指定します。
- `--regulatory-entry` は regulatory memo を指定します。helper は `KPI Dashboard Annex` ブロックを
  挿入/置換し、annex path、dashboard artefact、digest、timestamp を記録します。
- `--portal-entry` は Docusaurus 側のコピー
  (`docs/portal/docs/sns/regulatory/*.md`) を同期します。
- `--regulatory-entry`/`--portal-entry` を省略する場合も、PDF/CSV snapshots をアップロードしてください。
- 定期 export は `docs/source/sns/regulatory/annex_jobs.json` に suffix/cycle を登録し、
  `python3 scripts/run_sns_annex_jobs.py --verbose` を実行します。
- `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports`
  (または `make check-sns-annex`) で jobs の整合性、`sns-annex` マーカー、annex stub の存在を検証します。
これにより copy/paste を排除し、CI で schedule/marker/localization の drift を防ぎます。

## 2. Onboarding kit components

### Suffix wiring

- Registry schema + selector rules:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  と [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)。
- DNS skeleton helper:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  と [gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md) の rehearsal 手順。
- registrar の各ローンチで `docs/source/sns/reports/` に short note を残し、selector サンプル、GAR 証明、DNS hashes を記録します。

### Pricing cheatsheet

| Label length | Base fee (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Suffix coefficients: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Term multipliers: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). 交渉した例外は registrar ticket に記録します。

### Premium auctions vs renewals

1. **Premium pool** -- sealed-bid commit/reveal (SN-3)。`sns_premium_commit_total` で bids を追跡し、
   manifest を `docs/source/sns/reports/` に公開します。
2. **Dutch reopen** -- grace + redemption 終了後、10x から 1 日 15% 減衰する 7-day Dutch sale を開始。
   `manifest_id` を付けて dashboard で進捗を追えるようにします。
3. **Renewals** -- `sns_registrar_status_total{resolver="renewal"}` を監視し、autorenew チェックリスト
   (通知、SLA、fallback payment rails) を registrar ticket に残します。

### Developer APIs & automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)。
- Bulk helper & CSV schema:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)。
- 例:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

`--submission-log` の出力にある manifest ID を KPI dashboard filter に含め、finance が release ごとの revenue を照合できるようにします。

### Evidence bundle

1. 連絡先、suffix スコープ、payment rails を含む registrar ticket。
2. DNS/resolver evidence (zonefile skeletons + GAR proofs)。
3. Pricing worksheet + governance 承認 overrides。
4. API/CLI smoke-test artefacts (`curl` samples, CLI transcripts)。
5. KPI dashboard screenshot + CSV export (月次 annex へ添付)。

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Dashboard imported | Product Analytics | Grafana API response + dashboard UID |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` submissions log |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

registrar または suffix を有効化する前に checklist を完了してください。署名済み bundle は SN-8 ゲートを通過させ、marketplace のローンチ確認時に監査が参照できる単一の evidence を提供します。
