---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Nome de usuário Nexus
description: O nome do produto é Nexus, ou `docs/source/nexus_operations.md`.
---

Verifique se o dispositivo está conectado a `docs/source/nexus_operations.md`. فهي تلخص قائمة التشغيل, ونقاط إدارة التغيير, ومتطلبات تغطية القياس التي يجب على مشغلي Nexus é compatível.

## قائمة دورة الحياة

| المرحلة | Produtos | الأدلة |
|-------|--------|----------|
| ما قبل الإقلاع | Você pode usar o `profile = "iroha3"` e o cabo de alimentação. | مخرجات `scripts/select_release_profile.py`, soma de verificação, حزمة بيانات موقعة. |
| مواءمة الكتالوج | Você pode usar o `[nexus]`, mas também o `--trace-config`. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع تذكرة onboarding. |
| Produtos e serviços | Se você usar `irohad --sora --config ... --trace-config`, você pode usar o CLI (`FindNetworkStatus`) para fazer isso. | سجل فحص الدخان + تأكيد Alertmanager. |
| حالة مستقرة | راقب لوحات/تنبيهات, دوّر المفاتيح حسب وتيرة الحوكمة, وحدث configs/runbooks عند تغير البيانات. | Você pode usar um dispositivo de armazenamento de dados, para que você possa usá-lo corretamente. |

O serviço de onboarding (em inglês) está disponível no `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة التغيير

1. **تحديثات الإصدار** - تتبع الإعلانات em `status.md`/`roadmap.md`; أرفق قائمة onboarding مع كل PR للإصدار.
2. **تغييرات بيانات lane** - تحقق من الحزم الموقعة من Space Directory e وأرشفها تحت `docs/source/project_tracker/nexus_config_deltas/`.
3. **تغييرات الإعداد** - Na área de `config/config.toml`, você precisa se livrar da pista/espaço de dados. Verifique se o produto está funcionando corretamente ou se você está usando o produto.
4. **تمارين الرجوع** - درّب ربع سنوي على إجراءات الإيقاف/الاستعادة/فحص الدخان؛ Este código é `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال** - يجب أن تحصل lanes الخاصة/CBDC على موافقة امتثال قبل تعديل سياسة DA أو Verifique o valor do arquivo (`docs/source/cbdc_lane_playbook.md`).

## Números e SLOs

- Códigos: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, que podem ser instalados no SDK (como `android_operator_console.json`).
- Nome: `dashboards/alerts/nexus_audit_rules.yml` e Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- المقاييس التي يجب مراقبتها:
  - `nexus_lane_height{lane_id}` - Verifique se há algum problema.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل lane (الافتراضي 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - تنبيه عندما يتجاوز P99 900 ms (público) ou 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - A taxa de juros é de 5% a 2%.
  - `telemetry_redaction_override_total` - 2 de setembro de setembro Verifique se há algum problema com isso.
- Não há nenhum problema em [Nexus](./nexus-telemetry-remediation). وأرفق النموذج المكتمل بملاحظات مراجعة التشغيل.

## مصفوفة الحوادث

| الشدة | التعريف | الاستجابة |
|----------|------------|----------|
| 1º de setembro | Não há espaço de dados, mas o espaço de dados é de 15 dias e o espaço de dados é de 15 dias. | نادِ Nexus Primário + Engenharia de Liberação + Conformidade, جمّد القبول, اجمع الأدلة, انشر تواصل <=60 دقيقة, RCA <=5 أيام عمل. |
| 2 de setembro | خرق SLA لتراكم lane, نقطة عمياء في القياس >30 dias, durante o rollout للبيانات. | نادِ Nexus Primary + SRE, عالج <=4 ساعات, سجّل المتابعات خلال يومي عمل. |
| 3 de setembro | انحراف غير معطل (docs, تنبيهات). | Deixe-os entrar e saia do lugar. |

Isso significa que você pode identificar IDs de faixa/espaço de dados, além de IDs de faixa/espaço de dados e والمقاييس/السجلات الداعمة, ومهام المتابعة والمالكين.

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
- Coloque a chave de fenda + caixa `--trace-config` no lugar.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد, أو البيانات.
- Coloque a chave Prometheus no lugar de 12 polegadas.
- Você pode usar o `docs/source/project_tracker/nexus_config_deltas/README.md` para obter mais informações.

## مواد ذات صلة

- Configuração: [Visão geral Nexus](./nexus-overview)
- Nome: [especificação Nexus] (./nexus-spec)
- Pista هندسة: [modelo de pista Nexus] (./nexus-lane-model)
- Notas de transição وشيمات التوجيه: [Nexus notas de transição](./nexus-transition-notes)
- método de integração: [integração do operador Sora Nexus] (./nexus-operator-onboarding)
- معالجة القياس: [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation)