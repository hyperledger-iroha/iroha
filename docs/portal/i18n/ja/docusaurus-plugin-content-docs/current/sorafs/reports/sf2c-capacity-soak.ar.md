---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ソーク لتراكم سعة SF-2c

日: 2026-03-21

## ああ

液体を浸し、液体を浸し、SoraFS を使用してください。 SF-2c。

- **اختبار 浸漬 متعدد المزوّدين لمدة 30 يوما:** يتم عبر
  `capacity_fee_ledger_30_day_soak_deterministic` 年
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  يقوم harness بإنشاء خمسة مزودين، ويمتد عبر 30 نافذة تسوية، ويتحقق من أن إجماليات
  元帳は、オンラインで管理されています。ブレイク 3 のダイジェスト
  (`capacity_soak_digest=...`) حتى تتمكن CI من التقاط اللقطة القياسية ومقارنتها。
- ** 評価:** 評価
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (๑فس الملف)攻撃、クールダウン、斬撃、攻撃
  元帳。

## ああ

浸す方法:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

試合は、試合の準備をするために行われます。

## いいえ

يعرض Torii المزوّدين جنبًا إلى جنب مع 料金台帳の管理
ペナルティストライキ:

- 残り: `GET /v2/sorafs/capacity/state` يعيد إدخالات `credit_ledger[*]` التي
  台帳を浸す必要があります。やあ
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana インポート: `dashboards/grafana/sorafs_capacity_penalties.json`
  ストライキは、ストライキを攻撃します。
  ベースラインがかなり浸み込みます。

## ああ

- 煙を浸す（煙層）。
- Grafana は Torii をスクレイピングし、テレメトリを取得します。