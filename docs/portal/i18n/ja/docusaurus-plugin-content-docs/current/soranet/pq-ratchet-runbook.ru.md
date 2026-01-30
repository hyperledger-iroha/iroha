---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-ratchet-runbook
title: Учебная тревога PQ Ratchet SoraNet
sidebar_label: Runbook PQ Ratchet
description: Шаги on-call rehearsal для повышения или понижения стадийной PQ anonymity policy с детерминированной telemetry валидацией.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_ratchet_runbook.md`. Держите обе копии синхронизированными, пока наследуемые docs не будут выведены из эксплуатации.
:::

## Назначение

Этот runbook описывает последовательность fire-drill для стадийной post-quantum (PQ) anonymity policy SoraNet. Operators отрабатывают как promotion (Stage A -> Stage B -> Stage C), так и controlled demotion обратно к Stage B/A при падении supply PQ. Drill валидирует telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) и собирает artefacts для incident rehearsal log.

## Prerequisites

- Самый свежий `sorafs_orchestrator` binary с capability-weighting (commit равен или позже reference drill из `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Доступ к Prometheus/Grafana stack, который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`.
- Номинальный guard directory snapshot. Получите и проверьте копию до drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Если source directory публикует только JSON, перекодируйте его в Norito binary через `soranet-directory build` перед запуском rotation helpers.

- Захватите metadata и pre-stage artefacts ротации issuer с помощью CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Change window одобрен on-call командами networking и observability.

## Promotion steps

1. **Stage audit**

   Зафиксируйте стартовый stage:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Перед promotion ожидайте `anon-guard-pq`.

2. **Promotion в Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 минут для обновления manifests.
   - В Grafana (dashboard `SoraNet PQ Ratchet Drill`) убедитесь, что панель "Policy Events" показывает `outcome=met` для `stage=anon-majority-pq`.
   - Захватите screenshot или JSON панели и приложите к incident log.

3. **Promotion в Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Проверьте, что histogram `sorafs_orchestrator_pq_ratio_*` стремится к 1.0.
   - Убедитесь, что brownout counter остается плоским; иначе выполните шаги demotion.

## Demotion / brownout drill

1. **Индуцируйте синтетический дефицит PQ**

   Отключите PQ relays в playground среде, обрезав guard directory до классических entries, затем перезагрузите orchestrator cache:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Наблюдайте telemetry brownout**

   - Dashboard: панель "Brownout Rate" подскакивает выше 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщить `anonymity_outcome="brownout"` с `anonymity_reason="missing_majority_pq"`.

3. **Demotion до Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Если supply PQ все еще недостаточен, понизьте до `anon-guard-pq`. Drill завершен, когда brownout counters стабилизируются и promotions можно повторно применить.

4. **Восстановление guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry и artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** убедитесь, что brownout alert `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (&lt;5% в любом 10-минутном окне).
- **Incident log:** приложите telemetry snippets и заметки оператора в `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Signed capture:** используйте `cargo xtask soranet-rollout-capture`, чтобы скопировать drill log и scoreboard в `artifacts/soranet_pq_rollout/<timestamp>/`, вычислить BLAKE3 digests и создать подписанный `rollout_capture.json`.

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

Приложите сгенерированные metadata и signature к пакету governance.

## Rollback

Если drill выявляет реальную нехватку PQ, оставайтесь на Stage A, уведомите Networking TL и приложите собранные metrics вместе с guard directory diffs к incident tracker. Используйте ранее захваченный guard directory export, чтобы восстановить нормальный сервис.

:::tip Regression Coverage
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` предоставляет synthetic validation, которая поддерживает этот drill.
:::
