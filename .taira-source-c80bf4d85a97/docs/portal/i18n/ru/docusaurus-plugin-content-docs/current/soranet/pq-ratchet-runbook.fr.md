---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-ratchet-runbook
Название: Моделирование PQ Ratchet SoraNet
sidebar_label: Ratchet Runbook PQ
описание: Репетиционные действия по вызову для повышения или понижения уровня политики анонимности PQ и проверки детерминированной телеметрии.
---

:::примечание Канонический источник
Эта страница отражает `docs/source/soranet/pq_ratchet_runbook.md`. Сохраняйте обе копии выровненными до тех пор, пока старый комплект документации не будет удален.
:::

## Цель

В этом учебном пособии описана последовательность противопожарных учений для политики многоуровневой постквантовой (PQ) анонимности SoraNet. Операторы повторяют повышение (Этап A -> Этап B -> Этап C), а также контролируемое понижение до уровня B/A, когда предложение PQ снижается. Инструмент проверки проверяет перехватчики телеметрии (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) и собирает артефакты для журнала репетиции инцидентов.

## Предварительные условия

- Последний двоичный файл `sorafs_orchestrator` с взвешиванием возможностей (фиксация равна или позже ссылки на детализацию в `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к стеку Prometheus/Grafana, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- Номинальный снимок охранного каталога. Соберите и проверьте копию перед сверлением:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Если исходный каталог публикует только JSON, перекодируйте его в двоичный формат Norito с помощью `soranet-directory build` перед запуском помощников ротации.

- Собирайте метаданные и предварительные артефакты ротации эмитентов с помощью CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Окно изменений одобрено дежурными группами по сети и наблюдению.

## Этапы продвижения

1. **Стажировка по аудиту**

   Запишите стартовый курс:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Перед продвижением дождитесь `anon-guard-pq`.

2. **Переход на этап B (PQ большинства)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   – Подождите >=5 минут, пока манифесты обновятся.
   - В Grafana (панель мониторинга `SoraNet PQ Ratchet Drill`) убедитесь, что на панели «События политики» отображается `outcome=met` для `stage=anon-majority-pq`.
   - Сделайте снимок экрана или JSON панели и прикрепите его к журналу сбоев.

3. **Переход на этап C (строгий PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Убедитесь, что гистограммы `sorafs_orchestrator_pq_ratio_*` стремятся к 1,0.
   - Убедитесь, что счетчик снижения напряжения остается на одном уровне; в противном случае следуйте инструкциям по переходу на более раннюю версию.

## Упражнение по понижению/отсутствию напряжения

1. **Вызвать нехватку синтетического PQ**

   Отключите реле PQ в среде игровой площадки, сократив каталог защиты только до классических записей, а затем перезагрузите кеш оркестратора:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Наблюдайте телеметрию падения напряжения**

   - Панель мониторинга: панель «Уровень затемнения» поднимается выше 0.
   - ПромQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщать `anonymity_outcome="brownout"` с `anonymity_reason="missing_majority_pq"`.

3. **Переход на этап B/этап A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Если питания PQ остается недостаточным, понизьте версию до `anon-guard-pq`. Упражнение заканчивается, когда счетчики провалов стабилизируются и можно будет повторно применить повышения.

4. **Восстановить каталог охраны**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия и артефакты- **Панель управления:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Предупреждения Prometheus:** убедитесь, что предупреждение о снижении напряжения для `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (&lt;5% в течение любого 10-минутного окна).
- **Журнал происшествий**: добавьте фрагменты телеметрии и примечания оператора в `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Захват со знаком:** используйте `cargo xtask soranet-rollout-capture`, чтобы скопировать журнал тренировки и табло в `artifacts/soranet_pq_rollout/<timestamp>/`, вычислить дайджесты BLAKE3 и создать подписанный `rollout_capture.json`.

Пример:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Прикрепите сгенерированные метаданные и подпись к файлу управления.

## Откат

Если проверка выявит истинную нехватку PQ, оставайтесь на этапе A, уведомите сетевого TL и прикрепите собранные метрики, а также различия защитного каталога к средству отслеживания инцидентов. Используйте экспорт записи защитного каталога ранее, чтобы восстановить нормальное обслуживание.

:::tip Регрессионное покрытие
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` обеспечивает синтетическую проверку, которая поддерживает это упражнение.
:::