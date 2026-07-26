---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
título: خطة اطلاق ما بعد الكم SNNet-16G
sidebar_label: خطة اطلاق PQ
description: Você usa o handshake do X25519+ML-KEM no SoraNet com o canary padrão, com relés, clientes e SDKs.
---

:::note المصدر القياسي
Verifique o valor `docs/source/soranet/pq_rollout_plan.md`. Não se preocupe, o problema pode ser alterado.
:::

SNNet-16G é compatível com SoraNet. مفاتيح `rollout_phase` تسمح للمشغلين بتنسيق ترقية حتمية من متطلب guard no Stage A no تغطية الاغلبية في O Estágio B é o caminho PQ do Estágio C, mas o JSON/TOML não está disponível.

Manual de instruções:

- تعريفات المراحل ومفاتيح التهيئة الجديدة (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) O código base da base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Os sinalizadores são usados ​​no SDK e CLI para serem implementados no cliente durante o lançamento.
- Agendamento de retransmissão/cliente e governança de retransmissão (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos para reversão e treinamento de incêndio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## خريطة المراحل

| `rollout_phase` | مرحلة اخفاء الهوية الفعلية | الاثر الافتراضي | الاستخدام المعتاد |
|-----------------|--------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Estágio A) | O guarda PQ e o guarda PQ não funcionam no circuito. | linha de base e canário. |
| `ramp` | `anon-majority-pq` (Estágio B) | تحيز الاختيار نحو relés PQ لتحقيق تغطية >= ثلثين؛ Os relés são usados ​​como fallback. | canários حسب المناطق للـ relés; alterna o SDK. |
| `default` | `anon-strict-pq` (Estágio C) | Os circuitos PQ estão configurados para fazer downgrade. | الترقية النهائية بعد اكتمال telemetria e governança. |

A chave de segurança `anonymity_policy` é a chave para a operação do produto. Você pode usar o `rollout_phase` para obter mais informações sobre o produto. Procure clientes e clientes em todo o mundo.

## مرجع التهيئة

### Orquestrador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O loader é o orquestrador que contém o fallback e o `crates/sorafs_orchestrator/src/lib.rs:2229` e o `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. راجع `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` são necessários.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` é um dispositivo de teste (`crates/iroha/src/client.rs:2315`) que não é compatível com `iroha_cli app sorafs fetch`. Isso significa que você pode fazer isso sem problemas.

## الاتمتة

ادوات `cargo xtask` اثنان تقوم باتمتة توليد الجدول والتقاط artefatos.

1. **توليد الجدول الاقليمي**

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

   Limpe o `s` e `m` e `h` e `d`. O valor `artifacts/soranet_pq_rollout_plan.json` e o Markdown (`artifacts/soranet_pq_rollout_plan.md`) podem ser alterados.

2. **Artefatos de التقاط للتمرين مع التوقيعات**

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

   يقوم الامر بنسخ الملفات المزودة الى `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, ويحسِب digests من نوع BLAKE3 لكل artefato, ويكتب `rollout_capture.json` الذي Os metadados estão disponíveis no Ed25519. استخدم نفس private key التي توقع محاضر fire-drill كي تتمكن governança من التحقق بسرعة.## Usando sinalizadores para SDK e CLI

| السطح | Canário (Fase A) | Rampa (Etapa B) | Padrão (Fase C) |
|---------|------------------|----------------|------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` e código de barras | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuração do orquestrador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuração do cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (padrão) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos assinados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ajudantes do orquestrador JavaScript | `rolloutPhase: "canary"` e `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

O toggles do SDK é o analisador de estágio do orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`), mas o analisador de estágio é o orquestrador (`crates/sorafs_orchestrator/src/lib.rs:365`). Isso é algo que você pode fazer.

## قائمة agendamento للكاناري

1. **Pré-voo (T menos 2 semanas)**

- تاكد ان معدل brownout no Estágio A é de 1%, خلال الاسبوعين السابقين e تغطية PQ >=70% para منطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - جدولة slot لمراجعة governança يوافق نافذة canário.
   - Use `sorafs.gateway.rollout_phase = "ramp"` no staging (o JSON é o orquestrador e o orquestrador) e o dry-run é executado.

2. **Retransmissão canário (dia T)**

   - ترقية منطقة واحدة في كل مرة عبر ضبط `rollout_phase = "ramp"` على orquestrador وmanifests الخاصة بالـ relés المشاركة.
   - As opções "Eventos de política por resultado" e "Taxa de quedas de energia" no PQ Ratchet (em termos de implementação do programa) são usadas para proteger o cache de proteção TTL.
   - Os instantâneos de `sorafs_cli guard-directory fetch` são exibidos para serem exibidos.

3. **Cliente/SDK canário (T mais 1 semana)**

   - O `rollout_phase = "ramp"` é usado no cliente e substitui o `stage-b` pelo SDK do SDK.
   - A telemetria do sistema (`sorafs_orchestrator_policy_events_total` مجمعة حسب `client_id` e `region`) e a implementação da implementação.

4. **Promoção padrão (T mais 3 semanas)**

   - بعد موافقة governança, بدّل اعدادات orquestrador e cliente الى `rollout_phase = "default"` e lista de verificação الاستعداد الموقع ضمن artefatos الاصدار.

## قائمة governança e والادلة| تغيير المرحلة | بوابة الترقية | حزمة الادلة | Painéis e Alertas |
|--------------|----------------|-----------------|---------------------|
| Canário -> Rampa *(prévia do Estágio B)* | معدل brownout لمرحلة Stage A اقل من 1% خلال 14 يوما, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 لكل منطقة تمت ترقيتها, تحقق Argon2 ticket p95  Padrão *(aplicação do Estágio C)* | Burn-in de 30 minutos para SN16, e `sn16_handshake_downgrade_total` de linha de base, e ` sorafs_orchestrator_brownouts_total` para usar canary وتوثيق ensaio para alternar proxy. | O `sorafs_cli proxy set-mode --mode gateway|direct`, o conjunto `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, o `sorafs_cli guard-directory verify` e o `cargo xtask soranet-rollout-capture --label default`. | O PQ Ratchet está disponível para downgrade SN16 em `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. |
| Preparação para rebaixamento/reversão de emergência | Você pode fazer o downgrade, e usar o guard-directory, e o buffer `/policy/proxy-toggle` para fazer o downgrade Muito bem. | Checklist من `docs/source/ops/soranet_transport_rollback.md`, سجلات `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, تذاكر الحوادث, وقوالب الاشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, e um código de barras (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- خزّن كل artefato تحت `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` الناتج بحيث تحتوي حزم governança على scoreboard وpromtool traces وdigests.
- ارفق digests SHA256 للادلة المرفوعة (minutos PDF, pacote de captura, instantâneos de proteção) A encenação é importante.
- Transfira a telemetria para o `docs/source/soranet/snnet16_telemetry_plan.md` e faça o downgrade e faça o downgrade التنبيه.

## تحديثات painel e telemetria

`dashboards/grafana/soranet_pq_ratchet.json` يتضمن الان لوحة تعليقات "Plano de implementação" تربط بهذا playbook وتعرض المرحلة الحالية حتى تتمكن Governança de مراجعات من تاكيد المرحلة النشطة. Você pode usar os botões de ajuste e ajustar os botões.

A solução de problemas `stage` é um dispositivo de teste `stage`. canary e padrão é o valor padrão (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos para reversão

### Padrão -> Rampa (Estágio C -> Estágio B)

1. Configure o orquestrador com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (e o SDK do SDK) no Estágio B sem o estágio B.
2. Defina os clientes para o fluxo de trabalho `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` para o fluxo de trabalho `/policy/proxy-toggle`. Não.
3. Use `cargo xtask soranet-rollout-capture --label rollback-default` para diffs no guard-directory, promtool e dashboards no `artifacts/soranet_pq_rollout/`.

### Rampa -> Canário (Estágio B -> Estágio A)1. Faça o snapshot do diretório de guarda no diretório de proteção do diretório `sorafs_cli guard-directory import --guard-directory guards.json` ou `sorafs_cli guard-directory verify` Ele contém hashes de pacote.
2. Use `rollout_phase = "canary"` (ou substitua `anonymity_policy stage-a`) no orquestrador e nas configurações do cliente, para que você possa usar a broca de catraca PQ com [PQ catraca runbook](./pq-ratchet-runbook.md) não permite downgrade.
3. ارفق لقطات PQ Ratchet e telemetria SN16 المحدثة مع نتائج التنبيهات في سجل الحوادث قبل اخطار governança.

### Guardrail

- A implementação do `docs/source/ops/soranet_transport_rollback.md` é um recurso de mitigação e mitigação para o `TODO:` no rollout tracker.
- `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` são usados para reverter `promtool test rules` e rollback para drift. Você pode capturar a captura.