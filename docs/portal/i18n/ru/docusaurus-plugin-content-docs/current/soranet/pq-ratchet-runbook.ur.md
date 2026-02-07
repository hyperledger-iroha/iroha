---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-ratchet-runbook
Название: Пожарная дрель с храповым механизмом SoraNet PQ
Sidebar_label: Учебник PQ Ratchet
Описание: Этапы репетиции по вызову и детерминированная проверка телеметрии для пошагового продвижения или понижения политики анонимности PQ.
---

:::обратите внимание на канонический источник
Эта страница отражает `docs/source/soranet/pq_ratchet_runbook.md`. Синхронизируйте обе копии до тех пор, пока старый набор документации не будет удален.
:::

## Цель

В этом учебном пособии описана последовательность противопожарных учений для политики поэтапной постквантовой (PQ) анонимности SoraNet. Операторы репетируют как повышение (Этап A -> Этап B -> Этап C), так и контролируемое понижение обратно на этап B/A, когда предложение PQ низкое. Проверяет перехватчики телеметрии тренировки (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) и собирает артефакты для журнала репетиций инцидентов.

## Предварительные условия

- Последний двоичный файл `sorafs_orchestrator` с весом возможностей (равный или более поздний, чем ссылка на детализация фиксации, показанная в `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к стеку Prometheus/Grafana, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- Номинальный снимок каталога защиты. скопируйте выборку и проверьте перед тренировкой:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Если исходный каталог публикует только JSON, перекодируйте его из `soranet-directory build` в двоичный файл Norito перед запуском помощников ротации.

- Захват метаданных из CLI и артефактов предварительной ротации эмитентов:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Сеть и возможность наблюдения за дежурными командами, утвержденными окнами изменений.

## Этапы продвижения

1. **Поэтапный аудит**

   Запишите начальный этап:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Ожидайте `anon-guard-pq` перед продвижением по службе.

2. **Переход на этап B (PQ большинства)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 минут, пока манифесты обновятся.
   - Проверьте `outcome=met` на наличие `stage=anon-majority-pq` на панели «События политики» в Grafana (панель мониторинга `SoraNet PQ Ratchet Drill`).
   - Сделайте снимок экрана или панель в формате JSON и прикрепите его к журналу инцидентов.

3. **Переход на этап C (строгий PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Убедитесь, что гистограммы `sorafs_orchestrator_pq_ratio_*` приближаются к 1,0.
   - Убедитесь, что счетчик снижения напряжения остается неизменным; В противном случае выполните шаги по понижению в должности.

## Упражнение по понижению в должности / отключению электроэнергии

1. **Создать синтетический дефицит качества**

   Отключите ретрансляторы PQ, сократив каталог защиты только до классических записей в среде Playground, а затем перезагрузите кеш оркестратора:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Наблюдайте телеметрию падения напряжения**

   - Панель мониторинга: значение панели «Уровень затемнения» должно подняться выше 0.
   - ПромQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщать `anonymity_outcome="brownout"` и `anonymity_reason="missing_majority_pq"`.

3. **Переход на этап B/этап A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Если питания PQ все еще недостаточно, понизьте уровень до `anon-guard-pq`. Учения завершаются, когда счетчики отключения электроэнергии стабилизируются и можно снова применять повышения.

4. **Восстановить каталог Guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия и артефакты- **Панель управления:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Предупреждения Prometheus:** Убедитесь, что предупреждение о снижении напряжения `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (&lt;5% в любом 10-минутном окне).
- **Журнал инцидентов.** Добавляйте фрагменты телеметрии и примечания оператора в `docs/examples/soranet_pq_ratchet_fire_drill.log`.
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

Если при тренировке выяснится истинная нехватка PQ, оставайтесь на этапе A, уведомите Networking TL и прикрепите различия каталогов охраны с собранными метриками к системе отслеживания инцидентов. Восстановите нормальный сервис, используя экспорт защитного каталога, записанный ранее.

:::tip Покрытие регрессии
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` обеспечивает синтетическую проверку, поддерживающую это упражнение.
:::