---
id: reserve-ledger-digest
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


Политика Reserve+Rent (пункт roadmap **SFM-6**) теперь поставляет CLI helpers `sorafs reserve`
и транслятор `scripts/telemetry/reserve_ledger_digest.py`, чтобы treasury-прогоны могли
эмитировать детерминированные переводы rent/reserve. Эта страница отражает поток,
определенный в `docs/source/sorafs_reserve_rent_plan.md`, и объясняет, как подключить
новый transfer feed к Grafana + Alertmanager, чтобы ревьюеры экономики и governance
могли аудировать каждый цикл биллинга.

## Сквозной процесс

1. **Квота + проекция реестра**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account <i105-account-id> \
     --treasury-account <i105-account-id> \
     --reserve-account <i105-account-id> \
     --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Helper реестра добавляет блок `ledger_projection` (rent due, reserve shortfall,
   top-up delta, underwriting booleans) и Norito `Transfer` ISIs, необходимые для
   перемещения XOR между treasury и reserve аккаунтами.

2. **Сгенерировать дайджест + выводы Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Digest helper нормализует итоги micro-XOR в XOR, фиксирует соответствие underwriting
   и выпускает метрики transfer feed `sorafs_reserve_ledger_transfer_xor` и
   `sorafs_reserve_ledger_instruction_total`. Если нужно обработать несколько ledgers
   (например, пакет провайдеров), повторите пары `--ledger`/`--label`, и helper запишет
   единый файл NDJSON/Prometheus, содержащий каждый дайджест, чтобы dashboards могли
   поглотить весь цикл без bespoke glue. Файл `--out-prom` нацелен на textfile collector
   node-exporter - поместите `.prom` в каталог, который отслеживает exporter, или загрузите
   его в telemetry bucket, используемый задачей Reserve dashboard, тогда как `--ndjson-out`
   передает те же payloads в data pipelines.

3. **Публикация artefacts + evidence**
   - Сохраните дайджесты в `artifacts/sorafs_reserve/ledger/<provider>/` и добавьте ссылку
     на Markdown-сводку из еженедельного экономического отчета.
   - Приложите JSON digest к rent burn-down (чтобы аудиторы могли пересчитать математику)
     и включите checksum в governance evidence packet.
   - Если digest сигнализирует top-up или нарушение underwriting, укажите alert IDs
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) и отметьте,
     какие transfer ISIs были применены.

## Метрики → дашборды → алерты

| Исходная метрика | Панель Grafana | Алерт / policy hook | Примечания |
|------------------|----------------|---------------------|------------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” в `dashboards/grafana/sorafs_capacity_health.json` | Подкормите weekly treasury digest; всплески reserve flow пробрасываются в `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (тот же dashboard) | Сопоставьте с ledger digest, чтобы доказать, что выставленное хранилище соответствует переводам XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + статусные карточки в `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` срабатывает при `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` срабатывает при `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” и coverage cards в `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` и `SoraFSReserveLedgerTopUpTransferMissing` предупреждают, когда transfer feed отсутствует или нулевой, хотя rent/top-up требуется; coverage cards падают до 0% в тех же случаях. |

Когда цикл rent завершен, обновите Prometheus/NDJSON snapshots, убедитесь, что панели Grafana
подхватили новый `label`, и приложите скриншоты + Alertmanager IDs к governance пакету rent.
Это доказывает, что CLI проекция, телеметрия и governance artefacts происходят из **одного**
transfer feed и сохраняет экономические dashboards roadmap в соответствии с автоматизацией
Reserve+Rent. Coverage cards должны показывать 100% (или 1.0), а новые алерты должны
сняться, когда в digest присутствуют переводы rent и reserve top-up.
