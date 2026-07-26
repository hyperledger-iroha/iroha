---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-決済-faq
タイトル: よくある質問と決済
説明: Ответы для операторов о марлерутизации 決済、конвертации в XOR、телеметрии и аудиторских доказательствах。
---

よくある質問、決済 (`docs/source/nexus_settlement_faq.md`)、解決済みのメッセージを確認してください。モノレポの詳細。 Здесь объясняется、как Settlement Router обрабатывает выплаты、какие метрики отслеживать и как SDK должны интегрировать полезные нагрузки Norito。

## Основные моменты

1. **レーン** — каждый データスペース объявляет `settlement_handle` (`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` または `xor_dual_fund`)。 Смотрите актуальный каталог lane в `docs/source/project_tracker/nexus_config_deltas/`.
2. **Детерминированная конвертация** — 解決策と XOR через источники ликвидности, утвержденныеそうです。レーン заранее пополняют XOR-буферы;ヘアカット применяются только когда буферы выходят за пределы политики。
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации и датчики ヘアカット。 `dashboards/grafana/nexus_settlement.json`、`dashboards/alerts/nexus_audit_rules.yml` のいずれかです。
4. **Доказательства** — архивируйте конфиги, логи роутера, экспорт телеметрии и отчеты по сверке для аудитов.
5. **Обязанности SDK** — каждый SDK の決済、ID レーン、ペイロード Norito、чтобы最高です。

## Примеры потоков

| Тип レーン | Какие доказательства собрать | Что это подтверждает |
|----------|-----------|----------------|
| Приватная `xor_hosted_custody` | Логроутера + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC は、XOR を使用したヘアカットを提供します。 |
| Публичная `xor_global` | Логроутера + ссылка на DEX/TWAP + метрики латентности/конвертации |ヘアカットをするためにTWAPを使用します。 |
| Гибридная `xor_dual_fund` |パブリック vs シールド + の比較 |シールド付き/パブリック クラスのヘアカットを選択してください。 |

## Нужно бользе деталей?

- よくある質問: `docs/source/nexus_settlement_faq.md`
- 決済ルーター: `docs/source/settlement_router.md`
- CBDC: `docs/source/cbdc_lane_playbook.md`
- ランブック: [Nexus](./nexus-operations)