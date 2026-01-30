---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f5c64884331b8b0e60480d1a98664ca21105f7d5aa7af0a1b5be9cfd20b046c3
source_last_modified: "2025-11-14T04:43:22.430185+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Канонический источник
:::

SNNet-16G завершает post-quantum rollout для транспорта SoraNet. Knobs `rollout_phase` позволяют операторам координировать детерминированный promotion от текущего Stage A guard requirement до Stage B majority coverage и Stage C strict PQ posture без редактирования сырого JSON/TOML для каждой поверхности.

Этот playbook покрывает:

- Определения phase и новые configuration knobs (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) в codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Маппинг SDK и CLI flags, чтобы каждый client мог отслеживать rollout.
- Ожидания canary scheduling для relay/client и governance dashboards, которые gate-ят promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Rollback hooks и ссылки на fire-drill runbook ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Карта фаз

| `rollout_phase` | Эффективная anonymity stage | Эффект по умолчанию | Типичное использование |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | Требовать как минимум один PQ guard на circuit, пока флот прогревается. | Baseline и ранние недели canary. |
| `ramp`          | `anon-majority-pq` (Stage B) | Смещать выбор в сторону PQ relays для >= двух третей coverage; классические relays остаются fallback. | Региональные relay canaries; SDK preview toggles. |
| `default`       | `anon-strict-pq` (Stage C) | Принудить PQ-only circuits и усилить downgrade alarms. | Финальный promotion после завершения telemetry и sign-off governance. |

Если поверхность также задает явный `anonymity_policy`, он переопределяет phase для этого компонента. Отсутствие явного stage теперь defer-ится к значению `rollout_phase`, чтобы операторы могли переключить phase один раз на среду и дать clients унаследовать его.

## Reference конфигурации

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Loader orchestrator решает fallback stage во время выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и отображает его через `sorafs_orchestrator_policy_events_total` и `sorafs_orchestrator_pq_ratio_*`. См. `docs/examples/sorafs_rollout_stage_b.toml` и `docs/examples/sorafs_rollout_stage_c.toml` для готовых snippets.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь сохраняет разобранную phase (`crates/iroha/src/client.rs:2315`), чтобы helper команды (например `iroha_cli app sorafs fetch`) могли сообщать текущую phase вместе с default anonymity policy.

## Automation

Два `cargo xtask` helper автоматизируют генерацию расписания и capture artefacts.

1. **Сгенерировать региональный schedule**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Durations принимают суффиксы `s`, `m`, `h` или `d`. Команда выпускает `artifacts/soranet_pq_rollout_plan.json` и Markdown summary (`artifacts/soranet_pq_rollout_plan.md`), который можно приложить к change request.

2. **Capture drill artefacts с подписями**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Команда копирует указанные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет BLAKE3 digests для каждого artefact и пишет `rollout_capture.json` с metadata и Ed25519 подписью поверх payload. Используйте тот же private key, которым подписаны fire-drill minutes, чтобы governance могла быстро валидировать capture.

## Матрица флагов SDK & CLI

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` или опираться на phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` или `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Все SDK toggles маппятся на тот же stage parser, что и orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), поэтому multi-language deployments остаются в lock-step с настроенной phase.

## Canary scheduling checklist

1. **Preflight (T minus 2 weeks)**

- Убедитесь, что Stage A brownout rate <1% за предыдущие две недели и PQ coverage >=70% на регион (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте governance review slot, который утверждает окно canary.
   - Обновите `sorafs.gateway.rollout_phase = "ramp"` в staging (отредактируйте orchestrator JSON и redeploy) и выполните dry-run promotion pipeline.

2. **Relay canary (T day)**

   - Продвигайте по одному региону, задавая `rollout_phase = "ramp"` на orchestrator и manifests участвующих relays.
   - Наблюдайте "Policy Events per Outcome" и "Brownout Rate" на PQ Ratchet dashboard (с rollout panel) в течение удвоенного TTL guard cache.
   - Снимайте snapshots `sorafs_cli guard-directory fetch` до и после для audit storage.

3. **Client/SDK canary (T plus 1 week)**

   - Переключите `rollout_phase = "ramp"` в client configs или передайте `stage-b` overrides для целевых SDK cohorts.
   - Захватите telemetry diffs (`sorafs_orchestrator_policy_events_total`, сгруппированные по `client_id` и `region`) и приложите их к rollout incident log.

4. **Default promotion (T plus 3 weeks)**

   - После sign-off governance переключите orchestrator и client configs на `rollout_phase = "default"` и ротуйте подписанный readiness checklist в release artefacts.

## Governance & evidence checklist

| Phase change | Promotion gate | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Stage-A brownout rate <1% за последние 14 дней, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 по регионам promotion, Argon2 ticket verify p95 < 50 ms и слот governance под promotion забронирован. | Пара JSON/Markdown от `cargo xtask soranet-rollout-plan`, парные snapshots `sorafs_cli guard-directory fetch` (до/после), подписанный bundle `cargo xtask soranet-rollout-capture --label canary`, и canary minutes с ссылкой на [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), telemetry references в `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | 30-дневный SN16 telemetry burn-in выполнен, `sn16_handshake_downgrade_total` плоский на baseline, `sorafs_orchestrator_brownouts_total` ноль во время client canary, и proxy toggle rehearsal записан. | Транскрипт `sorafs_cli proxy set-mode --mode gateway|direct`, вывод `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, лог `sorafs_cli guard-directory verify`, подписанный bundle `cargo xtask soranet-rollout-capture --label default`. | Тот же PQ Ratchet board плюс SN16 downgrade panels, описанные в `docs/source/sorafs_orchestrator_rollout.md` и `dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | Триггерится при всплесках downgrade counters, провале guard-directory verification или устойчивых downgrade events в буфере `/policy/proxy-toggle`. | Checklist из `docs/source/ops/soranet_transport_rollback.md`, логи `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, incident tickets и notification templates. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` и оба alert packs (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Храните каждый artefact в `artifacts/soranet_pq_rollout/<timestamp>_<label>/` вместе с сгенерированным `rollout_capture.json`, чтобы governance packets включали scoreboard, promtool traces и digests.
- Приложите SHA256 digests загруженных доказательств (minutes PDF, capture bundle, guard snapshots) к promotion minutes, чтобы Parliament approvals можно было воспроизвести без доступа к staging cluster.
- Ссылайтесь на telemetry plan в promotion ticket, чтобы подтвердить, что `docs/source/soranet/snnet16_telemetry_plan.md` остается каноническим источником для downgrade vocabularies и alert thresholds.

## Dashboard & telemetry updates

`dashboards/grafana/soranet_pq_ratchet.json` теперь содержит annotation panel "Rollout Plan", который ссылается на этот playbook и отображает текущую phase, чтобы governance reviews могли подтвердить активную stage. Держите описание panel синхронным с будущими изменениями knobs.

Для alerting убедитесь, что существующие rules используют label `stage`, чтобы canary и default phases триггерили отдельные policy thresholds (`dashboards/alerts/soranet_handshake_rules.yml`).

## Rollback hooks

### Default -> Ramp (Stage C -> Stage B)

1. Demote orchestrator через `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (и отзеркалить ту же phase в SDK configs), чтобы Stage B восстановился по всему флоту.
2. Принудить clients в безопасный transport profile через `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, сохранив transcript для auditability workflow `/policy/proxy-toggle`.
3. Запустить `cargo xtask soranet-rollout-capture --label rollback-default` для архивации guard-directory diffs, promtool output и dashboard screenshots под `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. Импортировать guard-directory snapshot, захваченный перед promotion, через `sorafs_cli guard-directory import --guard-directory guards.json` и повторно запустить `sorafs_cli guard-directory verify`, чтобы demotion packet включал hashes.
2. Установить `rollout_phase = "canary"` (или override `anonymity_policy stage-a`) в orchestrator и client configs, затем повторить PQ ratchet drill из [PQ ratchet runbook](./pq-ratchet-runbook.md), чтобы доказать downgrade pipeline.
3. Приложить обновленные PQ Ratchet и SN16 telemetry screenshots плюс результаты alert outcomes к incident log перед уведомлением governance.

### Guardrail reminders

- Ссылайтесь на `docs/source/ops/soranet_transport_rollback.md` при каждом demotion и фиксируйте временные mitigation как `TODO:` в rollout tracker для последующей работы.
- Держите `dashboards/alerts/soranet_handshake_rules.yml` и `dashboards/alerts/soranet_privacy_rules.yml` под покрытием `promtool test rules` до и после rollback, чтобы alert drift был задокументирован вместе с capture bundle.
