---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-default-lane-quickstart
titre : البدء السريع لـ lane الافتراضي (NX-5)
sidebar_label : Nom de la voie ou voie
description : Il s'agit d'un repli pour la voie de secours avec Nexus et d'un Torii et de SDK pour lane_id dans la fonction voies.
---

:::note المصدر الرسمي
Il s'agit de la référence `docs/source/quickstart/default_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح التوطين إلى البوابة.
:::

# البدء السريع لـ lane الافتراضي (NX-5)

> **سياق خارطة الطريق :** NX-5 - تكامل voie العام الافتراضي. Utilisez la solution de repli `nexus.routing_policy.default_lane` pour utiliser REST/gRPC avec Torii et le SDK. `lane_id` est une voie canonique. يوجه هذا الدليل المشغلين لإعداد سلوك العميل من البداية للنهاية.

## المتطلبات المسبقة

- Remplacer Sora/Nexus par `irohad` (voir `irohad --sora --config ...`).
- وصول إلى مستودع الإعدادات حتى تتمكن من تعديل أقسام `nexus.*`.
- `iroha_cli` مهيأ للتحدث مع عنقود الهدف.
- `curl`/`jq` (pour moi) pour `/status` à Torii.

## 1. La voie et l'espace de données

Il y a des voies et des espaces de données également disponibles. La description (`defaults/nexus/config.toml`) des voies est associée aux alias de l'espace de données :

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
```يجب أن يكون كل `index` فريدا ومتتاليا. معرفات dataspace en 64-بت؛ وتستخدم الأمثلة أعلاه القيم الرقمية نفسها كفهارس voie للوضوح.

## 2. ضبط افتراضات التوجيه والتجاوزات الاختيارية

قسم `nexus.routing_policy` يتحكم في lane الاحتياطي ويسمح بتجاوز التوجيه لتعليمات محددة أو بادئات الحسابات. Vous devez utiliser le planificateur `default_lane` et `default_dataspace`. Le routeur est compatible avec `crates/iroha_core/src/queue/router.rs` et est compatible avec Torii REST/gRPC.

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

عند إضافة lanes جديدة لاحقا، حدّث الكتالوج أولا ثم وسّع قواعد التوجيه. يجب أن يظل lane الاحتياطي يشير إلى lane العام الذي يحمل غالبية حركة المستخدمين حتى تبقى SDK القديمة متوافقة.

## 3. إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

تسجل العقدة سياسة التوجيه المشتقة أثناء الإقلاع. Il s'agit d'un alias (un alias utilisé dans l'espace de données d'un utilisateur) pour les potins.

## 4. تأكيد حالة حوكمة voie

بمجرد أن تصبح العقدة online,، استخدم أداة CLI للتحقق من أن lane الافتراضي مختوم (manifest محمّل) وجاهز للحركة. تعرض النظرة الملخصة صفا لكل voie:

```bash
iroha_cli app nexus lane-report --summary
```

Exemple de sortie :

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Vous pouvez également utiliser le runbook `sealed` pour Lane. علم `--fail-on-sealed` مفيد لـ CI.

## 5. فحص حمولة حالة Toriiاستجابة `/status` تعرض سياسة التوجيه ولقطة planificateur de voie. استخدم `curl`/`jq` لتأكيد الافتراضات المضبوطة والتحقق من أن lane الاحتياطي ينتج القياس عن Par:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Exemple de sortie :

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

Pour le planificateur de la voie `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يؤكد أن لقطة TEU et alias ورايات manifeste تتطابق مع الإعداد. Il s'agit d'un Grafana pour l'ingestion de voie.

## 6. اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` et crate pour Rust est également compatible avec `lane_id` pour `--lane-id` / `LaneSelector`. Utilisez le routeur de file d'attente `default_lane`. استخدم الأعلام الصريحة `--lane-id`/`--dataspace-id` فقط عند استهداف voie غير افتراضي.
- **JS/Swift/Android.** Le SDK est disponible pour `laneId`/`lane_id` et est également disponible `/status`. Il s'agit d'une question de mise en scène et de production, ainsi que de questions liées à la mise en scène et à la production.
- **Tests Pipeline/SSE.** Les tests effectués sont `tx_lane_id == <u32>` (`docs/source/pipeline.md`). اشترك في `/v1/pipeline/events/transactions` بهذا الشرط لإثبات أن الكتابات المرسلة بدون lane صريح تصل تحت معرف lane الاحتياطي.

## 7. المراقبة وروابط الحوكمة- `/status` est associé à `nexus_lane_governance_sealed_total` et `nexus_lane_governance_sealed_aliases` dans Alertmanager pour le manifeste de voie. ابق هذه التنبيهات مفعلة حتى في devnets.
- خريطة القياس للـ scheduler ولوحة حوكمة lanes (`dashboards/grafana/nexus_lanes.json`) تتوقع حقول alias/slug من الكتالوج. إذا اعدت تسمية alias، اعد تسمية دلائل Kura المقابلة كي يحافظ المدققون على مسارات حتمية (متابعة تحت NX-1).
- موافقات البرلمان لـ lanes الافتراضية يجب ان تتضمن خطة rollback. Vous pouvez utiliser le hash manifest et le runbook pour obtenir des informations supplémentaires sur le runbook. المطلوبة.