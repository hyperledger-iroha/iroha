---
lang: ru
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

# Ранбук репетиции запуска Nexus в режиме multi‑lane

Этот ранбук ведёт репетицию Phase B4 Nexus. Он проверяет, что согласованный
governance‑пакет `iroha_config` и multi‑lane genesis‑манифест детерминированно
работают в телеметрии, маршрутизации и rollback‑drills.

## Область

- Прогнать все три lane Nexus (`core`, `governance`, `zk`) с смешанным ingress Torii
  (транзакции, деплой контрактов, действия governance), используя подписанную seed
  `NEXUS-REH-2026Q1`.
- Собрать телеметрические/trace‑артефакты, требуемые для B4 acceptance (Prometheus
  scrape, OTLP export, структурированные логи, Norito admission traces, RBC‑метрики).
- Выполнить rollback‑drill `B4-RB-2026Q1` сразу после dry‑run и подтвердить, что
  single‑lane профиль пере‑применяется корректно.

## Предусловия

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` отражает
   утверждение GOV-2026-03-19 (подписанные манифесты + инициалы ревьюеров).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, с
   `nexus.enabled = true` встроенным) и `defaults/nexus/genesis.json` совпадают с
   утверждёнными хэшами; `kagami genesis bootstrap --profile nexus` выдаёт тот же
   digest, что в трекере.
3. Каталог lanes соответствует утверждённой трёхlane‑структуре; `irohad --sora
   --config defaults/nexus/config.toml` должен вывести баннер Nexus router.
4. Multi‑lane CI зелёная: `ci/check_nexus_multilane_pipeline.sh`
   (запускает `integration_tests/tests/nexus/multilane_pipeline.rs` через
   `.github/workflows/integration_tests_multilane.yml`) и
   `ci/check_nexus_multilane.sh` (router coverage) проходят, чтобы профиль Nexus
   оставался multi‑lane‑готовым (`nexus.enabled = true`, хэши каталога Sora
   целы, storage по lane в `blocks/lane_{id:03}_{slug}` и merge logs
   подготовлены). Фиксируйте digests артефактов в трекере при изменении defaults‑bundle.
5. Dashboards + алерты для Nexus‑метрик импортированы в Grafana‑папку репетиции;
   маршрутизация алертов направлена на PagerDuty службы репетиции.
6. Lanes Torii SDK настроены по таблице routing‑policy и могут локально
   воспроизводить rehearsal‑нагрузку.

## План‑таймлайн

| Фаза | Окно | Владельцы | Критерий выхода |
|-------|---------------|----------|---------------|
| Подготовка | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed опубликован, дашборды подготовлены, узлы репетиции развернуты. |
| Freeze staging | Apr 8 2026 18:00 UTC | @release-eng | Hashes config/genesis перепроверены; уведомление о freeze отправлено. |
| Исполнение | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Чек‑лист выполнен без блокирующих инцидентов; телеметрический пакет архивирован. |
| Rollback drill | Сразу после выполнения | @sre-core | Чек‑лист `B4-RB-2026Q1` выполнен; rollback‑телеметрия собрана. |
| Ретроспектива | До Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Документ retro/lessons learned + tracker блокеров опубликован. |

## Чек‑лист выполнения (Apr 9 2026 15:00 UTC)

1. **Аттестация конфига** — `iroha_cli config show --actual` на каждом узле;
   подтвердить совпадение хэшей с записью трекера.
2. **Прогрев lanes** — воспроизвести seed‑нагрузку 2 слота, проверить
   `nexus_lane_state_total` на активность всех трёх lanes.
3. **Сбор телеметрии** — записать Prometheus `/metrics` snapshots, OTLP‑образцы,
   структурированные логи Torii (по lane/dataspace) и RBC‑метрики.
4. **Governance‑hooks** — выполнить поднабор governance‑транзакций и проверить
   lane‑routing + телеметрические теги.
5. **Incident drill** — смоделировать насыщение lane по плану; убедиться, что
   алерты срабатывают и реакция зафиксирована.
6. **Rollback drill `B4-RB-2026Q1`** — применить single‑lane профиль, повторить
   чек‑лист rollback, собрать телеметрию и пере‑включить Nexus bundle.
7. **Загрузка артефактов** — выгрузить телеметрический пакет, Torii traces и drill log
   в Nexus evidence bucket; сослаться в `docs/source/nexus_transition_notes.md`.
8. **Манифест/валидация** — запустить `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` для генерации
   `telemetry_manifest.json` + `.sha256`, затем приложить манифест к записи трекера.
   Хелпер нормализует границы слотов (как целые в манифесте) и падает при
   отсутствии обязательных меток, сохраняя детерминизм governance‑артефактов.

## Выходные артефакты

- Подписанный checklist rehearsal + incident drill log.
- Телеметрический пакет (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Telemetry manifest + digest от скрипта проверки.
- Ретроспективный документ с блокерами, митигациями и ответственными.

## Итог выполнения — Apr 9 2026

- Репетиция прошла 15:00 UTC–16:12 UTC с seed `NEXUS-REH-2026Q1`; все три lane
  держали ~2.4k TEU на слот, а `nexus_lane_state_total` показал балансировку
  envelope‑ов.
- Телеметрический пакет архивирован в `artifacts/nexus/rehearsals/2026q1/`
  (включает `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`,
  incident log и rollback‑evidence). Checksums зафиксированы в
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- Rollback‑drill `B4-RB-2026Q1` завершён в 16:18 UTC; single‑lane профиль
  пере‑применён за 6m42s без зависших lanes, затем Nexus‑bundle включён после
  подтверждения телеметрии.
- Инцидент насыщения lane, введённый на слоте 842 (forced headroom clamp), дал
  ожидаемые алерты; playbook mitigation закрыл page за 11m с документированным
  PagerDuty‑таймлайном.
- Блокеров не выявлено; follow‑up задачи (автоматизация логирования TEU headroom,
  скрипт валидации телеметрического пакета) отмечены в ретроспективе Apr 15.

## Эскалация

- Блокирующие инциденты или регрессии телеметрии останавливают rehearsal и требуют
  эскалации в governance в течение 4 рабочих часов.
- Любое отклонение от утверждённого пакета config/genesis требует перезапуска
  rehearsal после повторного одобрения.

## Валидация телеметрического пакета (Выполнено)

Запускайте `scripts/telemetry/validate_nexus_telemetry_pack.py` после каждой
репетиции, чтобы доказать, что телеметрический bundle содержит канонические
артефакты (Prometheus export, OTLP NDJSON, Torii structured logs, rollback log)
и зафиксировать их SHA‑256 digests. Хелпер пишет `telemetry_manifest.json` и
соответствующий `.sha256`, чтобы governance могла ссылаться на hashes в retro‑пакете.

Для репетиции Apr 9 2026 валидированный манифест лежит рядом с артефактами в
`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` и его digest —
`telemetry_manifest.json.sha256`. Приложите оба файла к записи трекера при
публикации ретро.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

Используйте `--require-slot-range` / `--require-workload-seed` в CI, чтобы
блокировать загрузки без этих аннотаций. Применяйте `--expected <name>` для
добавления дополнительных артефактов (например, DA receipts), если это требует
план rehearsal.
