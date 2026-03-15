---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Валидация маркетплейса емкости SoraFS
タグ: [SF-2c、受け入れ、チェックリスト]
概要: Чеклист приемки、покрывающий онбординг プロバイダー、потоки споров и сверку казначейства、которые закрывают готовность маркетплейса емкости SoraFS к GA.
---

# Чеклист валидации маркетплейса емкости SoraFS

**Окно проверки:** 2026-03-18 -> 2026-03-24  
**Владельцы программы:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**Область:** Пайплайны онбординга プロバイダー、потоки рассмотрения споров и процессы сверки казначейства、необходимые для GA SF-2c。

Чеклист ниже должен быть проверен до включения маркетплейса для внезних операторов. Каждая строка ссылается на детерминированные доказательства (テスト、フィクスチャ или документацию)、которые аудиторы могутだ。

## Чеклист приемки

### プロバイダー

| Проверка | Валидация | Доказательство |
|------|-----------|----------|
|レジストリ канонические декларации емкости | `/v2/sorafs/capacity/declare` アプリ API、メタデータ、レジストリの情報ね。 | `crates/iroha_torii/src/routing.rs:7654` |
|スマート コントラクト ペイロード |ユニットは、ID プロバイダーとコミットされた GiB を使用して、最新の情報を提供します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI のアーティファクト онбординга | канонические CLI ハーネスは、Norito/JSON/Base64 の往復接続、および接続を実行します。オフラインで。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|ワークフローとガードレールの管理 |議会のデフォルトポリシーを確認してください。 | `../storage-capacity-marketplace.md` |

### 最高です

| Проверка | Валидация | Доказательство |
|------|-----------|----------|
|ダイジェスト ペイロード |ユニットは、保留中の台帳、ペイロード、保留中の台帳を保持します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Генератор споров CLI соответствует канонической схеме | Генератор споров CLI соответствует канонической схеме | CLI は Base64/Norito と JSON-сводки для `CapacityDisputeV1`、ハッシュ証拠バンドルを提供します。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|再生-тест доказывает детерминизм споров/пенализаций |テレメトリの証明失敗、スナップショット台帳、クレジットと紛争、ピアのスラッシュ。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook の概要 |ワークフロー評議会、ロールバックを実行します。 | `../dispute-revocation-runbook.md` |

### Сверка казначейства

| Проверка | Валидация | Доказательство |
|------|-----------|----------|
|見越元帳 совпадает с 30-dayневной проекцией soak |プロバイダーを 30 日以内に決済し、台帳を確認してください。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Сверка экспорта 元帳 записывается каждую ночь | `capacity_reconcile.py` は料金台帳を表示し、XOR エクスポート、ゲートウェイを表示します。アラートマネージャー。 | `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|請求ダッシュボードとテレメトリの発生率 | Grafana は、見越 GiB 時間、ストライク、保税担保のオンコール保証を保証します。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Опубликованный отчет архивирует методологию soak и команды 再生 |ソーク、可観測性フックを使用します。 | `./sf2c-capacity-soak.md` |

## Примечания по выполнению

サインオフの手順:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

ペイロードを取得するには、`sorafs_manifest_stub capacity {declaration,dispute}` を使用してください。 JSON/Norito バイトのガバナンス チケットです。

## Артефакты サインオフ

| Артефакт |パス |ブレイク2b-256 |
|----------|------|---------------|
|プロバイダーをサポート | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Пакет одобрения разрезения споров | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Пакет одобрения сверки казначейства | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

リリースバンドルがリリースされ、リリースバンドルがリリースされます。

## Подписи

- ストレージ チーム リーダー — @storage-tl (2026-03-24)  
- ガバナンス評議会書記 — @council-sec (2026-03-24)  
- 財務業務責任者 — @treasury-ops (2026-03-24)