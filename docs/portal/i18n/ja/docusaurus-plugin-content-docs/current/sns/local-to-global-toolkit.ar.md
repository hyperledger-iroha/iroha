---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ローカル -> グローバル

عكس هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من المستودع الاحادي. **ADDR-5c** と、CLI および Runbook のバージョン。

## いいえ

- `scripts/address_local_toolkit.sh` CLI 番号 `iroha` 番号:
  - `audit.json` -- テスト `iroha tools address audit --format json`。
  - `normalized.txt` -- リテラル IH58 (المفضل) / 圧縮 (`sora`) (الخيار الثاني) セレクター ローカル。
- パスワードを取得する (`dashboards/grafana/address_ingest.json`)
  アラート マネージャー (`dashboards/alerts/address_ingest_rules.yml`) によるカットオーバー Local-8 /
  ローカル-12 時間。ローカル 8 とローカル 12 を使用します。
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、`AddressInvalidRatioSlo`
  マニフェスト。
- [アドレス表示ガイドライン](address-display-guidelines.md)
  [アドレス マニフェスト Runbook](../../../source/runbooks/address_manifest_ops.md) UX を参照してください。

## ああ

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

意味:

- `--format compressed (`sora`)` は `sora...` の IH58 です。
- `--no-append-domain` لاصدار リテラル بدون نطاق。
- `--audit-only` は。
- `--allow-errors` للاستمرار عند ظهور صفوف تالفة (مطابق لسلوك CLI)。

アーティファクトの存在。 और देखें
変更管理の変更 Grafana 変更管理
ローカル-8 ローカル-12 >=30 。

## CI

1. ジョブを実行します。
2. `audit.json` ローカル セレクター (`domain.kind = local12`)。
   テスト `true` (テスト テスト `false` テスト 開発/テスト)
   شخيص التراجعات) واضف
   `iroha tools address normalize --fail-on-warning --only-local` CI 認証
   制作を行っております。

リリースノートの抜粋
完全にカットオーバーです。