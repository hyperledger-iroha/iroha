---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-ratchet-runbook
Название: SoraNet PQ Ratchet Simulacrum
Sidebar_label: Учебник PQ Ratchet
описание: Этапы тестирования по вызову для продвижения или понижения политики анонимности PQ во время стажировок с детерминированной проверкой телеметрии.
---

:::примечание Канонический источник
Эта страница отражает `docs/source/soranet/pq_ratchet_runbook.md`. Синхронизируйте обе копии.
:::

## Цель

В этом модуле Runbook описана последовательность моделирования политики постквантовой (PQ) анонимности на этапах SoraNet. Операторы репетируют повышение (Этап A -> Этап B -> Этап C) и контролируемое понижение обратно на этап B/A, когда предложение PQ падает. Симулякр проверяет перехватчики телеметрии (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) и собирает артефакты для журнала репетиции инцидентов.

## Предварительные условия

- Последний двоичный файл `sorafs_orchestrator` с весом возможностей (фиксация равна или превышает ссылку детализации, показанную в `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к стеку Prometheus/Grafana, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- Номинальный снимок охранного каталога. Найдите и проверьте копию перед симуляцией:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Если исходный каталог публикует только JSON, перекодируйте его в двоичный файл Norito с помощью `soranet-directory build` перед запуском помощников ротации.

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

1. **Поэтапный аудит**

   Зарегистрируйте начальный этап:

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
   - Сделайте снимок экрана или JSON панели мониторинга и прикрепите его к журналу инцидентов.

3. **Переход на этап C (строгий PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Проверить, что гистограммы `sorafs_orchestrator_pq_ratio_*` стремятся к 1,0.
   - Убедитесь, что счетчик снижения напряжения остается на одном уровне; в противном случае выполните шаги по понижению в должности.

## Упражнение на вылет/отключение

1. **Вызвать дефицит синтетического PQ**

   Отключите реле PQ в среде игровой площадки, сократив каталог защиты до только классических записей, а затем перезагрузите кеш оркестратора:

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

   Если питания PQ по-прежнему недостаточно, понизьте версию до `anon-guard-pq`. Моделирование заканчивается, когда счетчики отключения питания стабилизируются и можно будет повторно применить повышение.

4. **Восстановить каталог охраны**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия и артефакты- **Панель управления:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Предупреждения Prometheus:** убедитесь, что значение предупреждения о снижении напряжения `sorafs_orchestrator_policy_events_total` ниже настроенного SLO (&lt;5 % в любом 10-минутном окне).
- **Журнал происшествий**: прикрепите фрагменты телеметрии и примечания оператора к `docs/examples/soranet_pq_ratchet_fire_drill.log`.
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

Прикрепите сгенерированные метаданные и подпись к пакету управления.

## Откат

Если моделирование выявляет реальные недостатки PQ, оставайтесь на этапе A, уведомите сетевого TL и прикрепите собранные метрики вместе с различиями защитных каталогов к средству отслеживания инцидентов. Используйте ранее захваченный экспорт защитного каталога для восстановления нормального обслуживания.

:::tip Регрессионное покрытие
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` обеспечивает синтетическую проверку, поддерживающую этот симулякр.
:::