---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: عمليات Pin Registry
sidebar_label: Registro de Pins
description: Você pode usar o Pin Registry em SoraFS e usar o SLA para isso.
---

:::note المصدر المعتمد
Modelo `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على النسختين متزامنتين حتى تقاعد وثائق Esfinge القديمة.
:::

## نظرة عامة

Você pode usar o Pin Registry no SoraFS e usar o Pin Registry no SoraFS para fazer o download. Use o `iroha_torii` e o Prometheus para obter o `torii_sorafs_*`. O Torii é usado para registrar o registro em 30 dias no site, para que você possa usar o registro. Você pode usar o produto `/v1/sorafs/pin/*` para obter mais informações. Use o código de barras (`docs/source/grafana_sorafs_pin_registry.json`) para obter mais informações sobre Grafana. الأقسام أدناه مباشرة.

## مرجع المقاييس

| المقياس | Etiquetas | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | مخزون manifesta على السلسلة بحسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | Você pode usar aliases para o manifesto do registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | تراكم أوامر التكرار مقسما حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | Medidor مرجعي يعكس أوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA de referência: `met` تحسب الأوامر المكتملة ضمن المهلة, `missed` يجمع الإكمال المتاخر + الانتهاء, `pending` não é compatível. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | كمون الإكمال المجمع (عدد الايبوكات بين الاصدار والاكتمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Não há nenhum problema em relação a isso. |

Os medidores são usados ​​para capturar o snapshot, o que significa que você pode usar o snapshot e o `1m` E sim.

## Para Grafana

Use o JSON para definir o valor do arquivo. Não se preocupe, você pode fazer isso sem problemas.

1. **Manifestos de دورة حياة** – `torii_sorafs_registry_manifests_total` (mesmo que `status`).
2. **Alias ​​do nome** – `torii_sorafs_registry_aliases_total`.
3. **طابور الاوامر حسب الحالة** – `torii_sorafs_registry_orders_total` (referência a `status`).
4. **Backlog de registro de pendências** – `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` são usados.
5. **SLA do Novo** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Como usar o produto** – do modelo `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` para `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use o Grafana para usar o `min_over_time` para obter mais informações Veja mais:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الاوامر الفائتة (cerca de 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه- **SLA  0**
  - Nome: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الاجراء: افحص manifesta الخاصة بالحوكمة لتاكيد churn de provedores.
- **p95 للاكتمال > متوسط هامش المهلة**
  - Nome: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الاجراء: تحقق ان provedores يلتزمون قبل المهلة؛ Isso é tudo.

### امثلة قواعد Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "هبوط SLA لتكرار SoraFS تحت الهدف"
          description: "ظلت نسبة نجاح SLA اقل من 95% لمدة 15 دقيقة."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "تراكم تكرار SoraFS فوق العتبة"
          description: "تجاوزت اوامر التكرار المعلقة ميزانية التراكم المضبوطة."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "انتهاء اوامر تكرار SoraFS"
          description: "انتهى امر تكرار واحد على الاقل في الخمس دقائق الماضية."
```

## سير عمل الفرز

1. **تحديد السبب**
   - اذا زادت اخفاقات SLA بينما بقي التراكم منخفضا, ركز على اداء provedores (فشل PoR, الاكتمال المتاخر).
   - اذا زاد التراكم مع اخفاقات مستقرة, افحص القبول (`/v1/sorafs/pin/*`) لتاكيد manifests التي تنتظر موافقة sim.
2. **Fornecedores de serviços de terceiros**
   - O `iroha app sorafs providers list` deve ser removido do computador.
   - Medidores de medição `torii_sorafs_capacity_*` para GiB e PoR.
3. **اعادة اسناد التكرار**
   - اصدر اوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما ينخفض هامش التراكم (`stat="avg"`) em 5 etapas (تغليف manifest/CAR يستخدم `iroha app sorafs toolkit pack`).
   - اخطر الحوكمة اذا كانت aliases تفتقر لربط manifest نشط (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - سجل ملاحظات الحادث في سجل عمليات SoraFS مع الطوابع الزمنية وdigests المتاثرة.
   - حدّث هذا الدليل عند ظهور اوضاع فشل جديدة او لوحات جديدة.

## خطة الاطلاق

اتبع هذا الاجراء المرحلي عند تفعيل او تشديد سياسة cache للـ alias في الانتاج:1. **تحضير الاعدادات**
   - حدّث `torii.sorafs_alias_cache` em `iroha_config` (user -> actual) باستخدام TTLs ونوافذ السماح المتفق عليها: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. O código de segurança é instalado em `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لـ SDKs, وزع نفس القيم عبر طبقات الاعداد (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` para ligações Rust / NAPI / Python) não é possível العميل البوابة.
2. **Teste de teste**
   - انشر تغيير الاعدادات على عنقود staging يعكس طوبولوجيا الانتاج.
   - شغّل `cargo xtask sorafs-pin-fixtures` لتاكيد ان fixtures الكنسية للـ alias ما زالت تفك التشفير وتقوم بعملية ida e volta؛ Não há deriva que você possa encontrar.
   - Os endpoints `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` devem ser armazenados em fresco e janela de atualização e expirado e expirado. Você pode usar HTTP e cabeçalhos (`Sora-Proof-Status`, `Retry-After`, `Warning`) e usar JSON como exemplo.
3. **التفعيل في الانتاج**
   - اطرح الاعدادات الجديدة no final do processo. Use Torii para configurar gateways/SDK sem usar gateways Aqui está.
   - Coloque `docs/source/grafana_sorafs_pin_registry.json` no Grafana (empurre o cache para o alias) مساحة عمل NOC.
4. **التحقق بعد النشر**
   - `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` por 30 dias. Você pode usar o produto `error`/`expired` para obter mais informações. النمو غير المتوقع يعني ان على المشغلين فحص ادلة alias e provedores قبل المتابعة.
   - Você não pode atualizar os SDKs (SDKs obsoletos ou expirados). Certifique-se de que o produto esteja funcionando corretamente.
5. **Substituição**
   - اذا تاخر اصدار alias وتكرر تجاوز نافذة التحديث, خفف السياسة مؤقتا بزيادة `refresh_window` و `positive_ttl` está fora do lugar. A `hard_expiry` é uma ferramenta que pode ser usada para remover o problema.
   - عد الى الاعداد السابق باستعادة snapshot السابق من `iroha_config` اذا استمرت التليمتري في اظهار اعداد `error` مرتفعة, ثم افتح حادثة لتتبع تاخير توليد alias.

## مواد ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين, تكمل هذا الدليل.