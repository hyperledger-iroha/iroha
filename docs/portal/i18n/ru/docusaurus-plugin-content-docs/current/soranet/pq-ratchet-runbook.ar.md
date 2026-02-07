---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pq-ratchet-runbook
название: Упражнение PQ Ratchet в SoraNet
Sidebar_label: Руководство по PQ Ratchet
Описание: шаги для сменного упражнения по обновлению или понижению уровня политики поэтапной анонимности PQ с детерминированной проверкой телеметрии.
---

:::note Стандартный источник
Эта страница отражает `docs/source/soranet/pq_ratchet_runbook.md`. Сохраняйте две копии идентичными до тех пор, пока старые документы не будут удалены.
:::

##Цель

В этом руководстве описывается последовательность действий на случай непредвиденных обстоятельств для политики поэтапной постквантовой (PQ) анонимизации SoraNet. Операторы практикуют модернизацию (Этап A -> Этап B -> Этап C) и контролируемый переход на этап B/A, когда запасы PQ заканчиваются. В ходе упражнения проверяются перехватчики телеметрии (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) и собираются артефакты для журнала детализации инцидентов.

## Требования

- Новейший двоичный файл для `sorafs_orchestrator` с взвешиванием возможностей (фиксация в или после ссылки на упражнение, отображаемой в `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к пакету Prometheus/Grafana, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- снимок Мое имя для охранного каталога. Принесите и проверьте копию перед тренировкой:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Если исходный каталог публикует только JSON, перекодируйте его в двоичный файл Norito через `soranet-directory build` перед запуском ротации помощников.

- Соберите метаданные и подготовьте ротацию эмитента артефактов через CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Окно изменений одобрено дежурными сетевыми группами и группами мониторинга.

## Этапы обновления

1. **Этап проверки**

   Запись начальной школы:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Перед обновлением ожидайте `anon-guard-pq`.

2. **Переход на этап B (большинство PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   – Подождите >=5 минут, пока манифесты обновятся.
   - В Grafana (панель `SoraNet PQ Ratchet Drill`) убедитесь, что на панели «События политики» отображается `outcome=met` для `stage=anon-majority-pq`.
   - Сделайте снимок экрана или JSON панели управления и прикрепите его к журналу инцидентов.

3. **Переход на этап C (строгий PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Убедитесь, что диаграммы `sorafs_orchestrator_pq_ratio_*` соответствуют 1,0.
   - Убедитесь, что счетчик провалов остается постоянным; В противном случае следуйте инструкциям по сокращению.

## Упражнение на снижение напряжения

1. **Вызывание искусственного дефицита PQ**

   Отключите реле PQ в среде игровой площадки, сократив каталог защиты только до классических записей, а затем перезагрузив кеш оркестратора:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Отслеживание телеметрии отключения**

   - Панель мониторинга: панель «Уровень затемнения» поднимается выше 0.
   - ПромQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщать `anonymity_outcome="brownout"` с `anonymity_reason="missing_majority_pq"`.

3. **Понижение до этапа B/этапа A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Если питания PQ остается недостаточным, уменьшите его до `anon-guard-pq`. Упражнение заканчивается, когда показания счетчиков провалов стабилизируются и обновление можно будет перезапустить.

4. **Восстановить каталог охраны**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметрия и артефакты

- **Панель управления:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Предупреждения Prometheus:** Убедитесь, что предупреждение о снижении напряжения для `sorafs_orchestrator_policy_events_total` остается ниже утвержденного SLO (<5 % в течение любого 10-минутного окна).
- **Журнал происшествий:** Прикрепите выдержки из телеметрии и примечания оператора к `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Захват местоположения:** используйте `cargo xtask soranet-rollout-capture`, чтобы скопировать журнал тренировки и табло в `artifacts/soranet_pq_rollout/<timestamp>/`, рассчитать дайджесты BLAKE3 и создать местоположение `rollout_capture.json`.

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

Прикрепите сгенерированные данные и подпись к управлению.

## ОткатЕсли упражнение выявит истинный недостаток PQ, оставайтесь на этапе A, уведомите Networking TL и прикрепите собранные метрики с отклонениями в каталоге защиты к системе отслеживания инцидентов. Используйте ранее захваченный экспорт защитного каталога для восстановления нормального обслуживания.

:::tip Регрессионное покрытие
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` обеспечивает синтетическую проверку, которая поддерживает это упражнение.
:::