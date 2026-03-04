---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-default-lane-quickstart
título: البدء السريع لـ lane الافتراضي (NX-5)
sidebar_label: البدء السريع لـ lane الافتراضي
description: Use um fallback para lane em Nexus para substituir Torii e SDKs em lane_id em lanes العامة.
---

:::note المصدر الرسمي
Verifique o valor `docs/source/quickstart/default_lane.md`. Não se preocupe, você pode fazer isso sem problemas.
:::

# البدء السريع لـ lane الافتراضي (NX-5)

> **سياق خارطة الطريق:** NX-5 - تكامل lane العام الافتراضي. Você pode usar o fallback `nexus.routing_policy.default_lane` para usar o REST/gRPC em Torii e o SDK do Torii `lane_id` بأمان عندما تنتمي الحركة إلى lane canonical. Você pode usar um substituto para `/status`, e usar um substituto para mim Não há problema.

## المتطلبات المسبقة

- Use Sora/Nexus em `irohad` (não `irohad --sora --config ...`).
- Você pode usar o software `nexus.*` para obter mais informações.
- `iroha_cli` não funciona mais.
- `curl`/`jq` (ou melhor) para substituir `/status` por Torii.

## 1. Qual é a pista e o espaço de dados

Não há pistas e espaços de dados que não estejam disponíveis. O nome do arquivo (مقتطع من `defaults/nexus/config.toml`) é o nome das pistas para o espaço de dados:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Não há nenhum problema em `index`. Use o espaço de dados em 64-بت؛ وتستخدم الأمثلة أعلاه القيم الرقمية نفسها كفهارس lane للوضوح.

## 2. ضبط الاختيارية

O `nexus.routing_policy` é usado na pista e no local onde você pode usar a faixa de rodagem para obter mais informações. الحسابات. Você pode usar o agendador do `default_lane` e `default_dataspace`. O roteador que você usa no `crates/iroha_core/src/queue/router.rs` e o roteador não pode usar o Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

As faixas de rodagem não estão disponíveis para você. Não lane الاحتياطي يشير إلى lane العام الذي يحمل غالبية حركة المستخدمين حتى تبقى SDKs القديمة Obrigado.

##3.

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

تسجل العقدة سياسة التوجيه المشتقة أثناء الإقلاع. تظهر أي أخطاء تحقق (فهارس مفقودة, aliases مكررة, معرفات dataspace غير صالحة) para fofocas.

## 4. تأكيد حالة حوكمة pista

بمجرد أن تصبح العقدة online, استخدم أداة CLI للتحقق من أن lane الافتراضي مختوم (manifest محمّل) وجاهز Não. تعرض النظرة الملخصة صفا لكل lane:

```bash
iroha_cli app nexus lane-report --summary
```

Exemplo de saída:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Se a lane for `sealed`, use o runbook e a lane para obter mais informações. O `--fail-on-sealed` é compatível com CI.

## 5. فحص حمولة حالة Torii

Use `/status` para configurar o agendador e a pista. Use `curl`/`jq` para obter informações sobre a faixa de rodagem e a faixa de rodagem O que é isso:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Exemplo de saída:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Para configurar o agendador na pista `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يؤكد أن لقطة TEU وبيانات alias ورايات manifesto تتطابق مع الإعداد. A opção Grafana é usada para lane-ingest.

## 6. اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` e crate عميل Rust يحذفان حقل `lane_id` عندما لا تمرر `--lane-id` / `LaneSelector`. O roteador de fila é o `default_lane`. Verifique se o `--lane-id`/`--dataspace-id` está localizado na pista mais próxima.
- **JS/Swift/Android.** أحدث إصدارات SDK `laneId`/`lane_id` كاختيارية وتعود إلى القيمة المعلنة Em `/status`. حافظ على سياسة التوجيه متزامنة بين preparação e produção حتى لا تحتاج تطبيقات الهاتف لإعادة تهيئة Sim.
- **Testes de pipeline/SSE.** Testes de pipeline/SSE.** Testes de pipeline `tx_lane_id == <u32>` (`docs/source/pipeline.md`). Coloque o `/v1/pipeline/events/transactions` na faixa de rodagem da faixa de rodagem, na faixa de rodagem الاحتياطي.

## 7. المراقبة وروابط الحوكمة

- `/status` é o `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` no Alertmanager no local do manifesto da pista. Isso pode ser feito em devnets.
- خريطة القياس للـ agendador ولوحة حوكمة lanes (`dashboards/grafana/nexus_lanes.json`) تتوقع حقول alias/slug من الكتالوج. إذا اعدت تسمية alias, اعد تسمية دلائل Kura المقابلة كي يحافظ المدققون على مسارات حتمية (referência ao NX-1).
- موافقات البرلمان لـ lanes الافتراضية يجب ان تتضمن خطة rollback. سجّل hash do manifesto وأدلة الحوكمة بجانب هذا الدليل في runbook المشغل حتى لا تضطر الدورات المستقبلية لتخمين الحالة المطلوبة.