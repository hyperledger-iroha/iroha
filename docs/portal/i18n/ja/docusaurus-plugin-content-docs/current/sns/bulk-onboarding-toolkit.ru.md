---
lang: ja
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/bulk_onboarding_toolkit.md`, чтобы внезние
SN-3b が 1 つあります。
:::

# SNS バルク オンボーディング ツールキット (SN-3b)

**ロードマップ:** SN-3b「バルクオンボーディングツール」  
**Артефакты:** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

レジストラの名前は、`.sora` です。
`.nexus` は決済レールを通過します。 Ручная сборка
JSON ペイロードは、CLI のメソッド、SN-3b などの機能を備えています。
детерминированный CSV-to-Norito ビルダー、который готовит структуры
`RegisterNameRequestV1` と Torii または CLI。 Хелпер заранее валидирует каждую строку,
マニフェストと опциональный построчный JSON、および может отправлять
ペイロードと領収書を受け取ります。

## 1. CSV を作成する

Парсер требует следующую строку заголовка (порядок гибкий):

| Колонка | Обязательно | Описание |
|----------|---------------|----------|
| `label` | Да | Запрозенная метка (допускается 混合大文字、инструмент нормализует по Norm v1 и UTS-46)。 |
| `suffix_id` | Да | Числовой идентификатор суффикса (десятичный или `0x` hex)。 |
| `owner` | Да | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Да | Целое число `1..=255`。 |
| `payment_asset_id` | Да | Актив 和解 (например `xor#sora`)。 |
| `payment_gross` / `payment_net` | Да | Беззнаковые целые、представляющие единицы актива。 |
| `settlement_tx` | Да | JSON は、ハッシュを要求します。 |
| `payment_payer` | Да | AccountId、авторизовавлий платеж。 |
| `payment_signature` | Да | JSON は財務省のスチュワードを表します。 |
| `controllers` |オプション |コントローラー、`;` または `,` を参照してください。 По умолчанию `[owner]`。 |
| `metadata` |オプション |インライン JSON または `@path/to/file.json` のリゾルバー ヒント、TXT と同じです。 д。 По умолчанию `{}`。 |
| `governance` |オプション |インライン JSON または `@path` または `GovernanceHookV1`。 `--require-governance` は、今日。 |

Любая колонка может ссылаться на внезний файл, если добавить `@` в начале значения.
CSV を使用してください。

## 2. Запуск хелпера

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

説明:

- `--require-governance` отклоняет строки без ガバナンス フック (полезно для)
  プレミアム аукционов или 予約済み割り当て)。
- `--default-controllers {owner,none}` резает、будут ли пустые コントローラー ячейки
  オーナーです。
- `--controllers-column`、`--metadata-column`、および `--governance-column` です。
  上流への輸出も可能です。

マニフェストの作成:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Если указан `--ndjson`, каждый `RegisterNameRequestV1` также записывается как
однострочный JSON документ, чтобы автоматизация могла стримить запросы прямо в
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Автоматизированные отправки

### 3.1 Режим Torii REST

Укажите `--submit-torii-url` または `--submit-token`、`--submit-token-file`、
マニフェスト напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Хелпер делает один `POST /v1/sns/registrations` на запрос и останавливается при
  HTTP サービス。 Ответы добавляются в лог как NDJSON записи.
- `--poll-status` は、`/v1/sns/registrations/{selector}` を表示します
  каждой отправки (до `--poll-attempts`, по умолчанию 5), чтобы подтвердить
  ありがとうございます。 Укажите `--suffix-map` (JSON による `suffix_id` の説明)
  "suffix")、чтобы инструмент мог вывести `{label}.{suffix}` для ポーリング。
- Настройки: `--submit-timeout`、`--poll-attempts`、`--poll-interval`。

### 3.2 iroha CLI の説明

マニフェストを作成する CLI、マニフェストを作成する:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- コントローラ `Account` エントリ (`controller_type.kind = "Account"`)、
  CLI はアカウントベースのコントローラーに対応します。
- メタデータとガバナンス BLOB の統合
  `iroha sns register --metadata-json ... --governance-json ...` を参照してください。
- Stdout/stderr および коды выхода CLI логируются; ненулевые коды прерывают запуск。

Оба режима отправки можно запускать вместе (Torii и CLI) перекрестной проверки
レジストラのデプロイメントとフォールバックの両方。

### 3.3 Квитанции отправки

`--submission-log <path>` の NDJSON のバージョン:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Успезные ответы Torii включают структурированные поля из `NameRecordV1` или
`RegisterNameResponseV1` (`record_status`、`record_pricing_class`、
`record_owner`、`record_expires_at_ms`、`registry_event_version`、`suffix_id`、
`label`)、чтобы далеборды и отчеты управления могли парсить лог без свободного
текста。レジストラのマニフェストを確認する
Поспроизводимого доказательства。

## 4. Автоматизация релизов портала

CI およびポータル ジョブ `docs/portal/scripts/sns_bulk_release.sh`、который
`artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

概要:

1. `registrations.manifest.json`、`registrations.ndjson` および копирует
   これは、CSV と互換性があります。
2. マニフェスト Torii および CLI (при настройке)、записывая
   `submissions.log` を確認してください。
3. `summary.json` を使用して、Torii URL、CLI を実行します。
   タイムスタンプ)、バンドルを確認する
   ああ。
4. `metrics.prom` (`--metrics` をオーバーライド) Prometheus-совместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   и результатов отправки.概要 JSON を使用してください。

ワークフローは、最新のワークフローを使用して実行できます。
ありがとうございます。

## 5. Телеметрия и даборды

Файл метрик、сгенерированный `sns_bulk_release.sh`、содержит следующие серии:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` と Prometheus サイドカー (Promtail またはバッチ)
輸入者）、レジストラ、スチュワード、ガバナンスピア
バルク機能を備えています。 Grafana
`dashboards/grafana/sns_bulk_release.json` メッセージ: количество
必要に応じて、必要な情報を取得してください。ああ
фильтруется по `release`, чтобы аудиторы могли изучить один CSV прогон.

## 6. Валидация и режимы отказа

- **ラベルの正規化:** входы нормализуются Python IDNA + 小文字 и
  Norm v1 を更新しました。 Невалидные метки быстро падают до сетевых вызовов.
- **数値ガードレール:** サフィックス ID、期間年数、価格設定のヒント
  `u16` または `u8`。 Поля платежей принимают десятичные или hex числа до
  `i64::MAX`。
- **メタデータ/ガバナンス解析:** インライン JSON 解析。 Ссылки на файлы
  CSV を使用します。メタデータ не-объект приводит к озибке валидации.
- **コントローラー:** は `--default-controllers` です。 Указывайте
  явные списки コントローラー (например `i105...;i105...`) の所有者です。

Овибки сообщаются с контекстными номерами строк (например)
`error: row 12 term_years must be between 1 and 255`)。 Скрипт выходит с кодом `1`
`2` を確認すると、CSV が表示されます。

## 7. Тестирование и происхождение

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` の CSV ファイル、
  NDJSON、施行ガバナンスと CLI および Torii を参照してください。
- Python (パイソン パイソン) と работает
  `python3` をご覧ください。 История коммитов отслеживается рядом с CLI в
  основном репозитории для воспроизводимости.

マニフェストと NDJSON バンドル к を更新します。
レジストラ тикету、чтобы スチュワード могли воспроизвести точные ペイロード、отправленные
× Torii。