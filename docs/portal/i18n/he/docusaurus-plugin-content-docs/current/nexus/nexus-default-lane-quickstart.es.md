---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-default-lane-quickstart
כותרת: Guia rapida del lane predeterminado (NX-5)
sidebar_label: Guia rapida del lane predeterminado
תיאור: Configura y Verifica el Fallback del lane predeterminado de Nexus para que Torii y los SDK puedan omitir lane_id in lanes publicas.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/quickstart/default_lane.md`. Manten ambas copias alineadas hasta que el barrido de localizacion llegue al portal.
:::

# Guia rapida del lane predeterminado (NX-5)

> **הקשר של מפת הדרכים:** NX-5 - אינטגרציה של נתיב ציבורי מראש. זמן ריצה אחר חשיפה ו-fallback `nexus.routing_policy.default_lane` עבור נקודות קצה REST/gRPC de Torii y cada SDK puedan omitir con seguridad un `lane_id` cuando el trafico public canonic al la. Esta guia lleva a los מפעילים את הקטלוג, בדוק את ה-fallback en `/status` ואת הבעלים של רכיבי לקוח אקסטרים.

## דרישות מוקדמות

- Un build de Sora/Nexus de `irohad` (ejecuta `irohad --sora --config ...`).
- עזר למאגר תצורה עבור עריכת סעיפים `nexus.*`.
- `iroha_cli` קונפיגורציית עבור חפצי אשכול.
- `curl`/`jq` (o equivalente) לבדיקת מטען `/status` de Torii.

## 1. תאר את קטלוג הנתיבים ומרחבי הנתונים

Declara los lanes y dataspaces que deben existir en la red. El fragmento suuiente (recortado de `defaults/nexus/config.toml`) registra tres lanes publicas mas los alias de dataspace correspondientes:

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

Cada `index` debe ser unico y contiguo. Los ids de dataspace בן ערך של 64 סיביות; los ejemplos anteriores usan los mismos valores numericos que los indices de lane para mayor claridad.

## 2. Configura los valores predeterminados de enrutamiento y las sobreescrituras opcionales

La seccion `nexus.routing_policy` controla el lane de fallback y te permite sobrescribir el enrutamiento para instrucciones especificas o prefijos de cuenta. אם זה קורה בקנה אחד, מתזמן מתזמן לביצוע פעולות `default_lane` ו-`default_dataspace`. La logica del router vive en `crates/iroha_core/src/queue/router.rs` y aplica la politica de forma transparente a las superficies REST/gRPC de Torii.

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

Cuando mas adelante מסכים עם ניואבס ליין, אקטואליזה ראשונית אל קטלוג y luego extiende las reglas de enrutamiento. El lane de fallback debe seguir apuntando al lane publico que concentra la mayor parte del trafico de usuarios para que los SDKs heredados sigan funcionando.

## 3. Arranca un nodo con la politica aplicada

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

El nodo registra la politica de enrutamiento derivada durante el arranque. Cualquier error de validacion (מדדים faltantes, alias duplicados, IDs de dataspace invalidos) se muestra antes de que comience el gossip.

## 4. אשר את אסטדו דה גוברננסה דל לייןUna vez que el nodo este en linea, usa el helper del CLI para verificar que el lane predeterminado este sellado (manifest cargado) y listo para trafico. La vista de resumen imprime una fila por lane:

```bash
iroha_cli app nexus lane-report --summary
```

פלט לדוגמה:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Si el lane predeterminado muestra `sealed`, seue el runbook de gobernanza de lanes antes de permitir trafico externo. El flag `--fail-on-sealed` es util para CI.

## 5. Inspecciona los payloads de estado de Torii

La respuesta `/status` expone tanto la politica de enrutamiento como la instantanea del scheduler por lane. ארה"ב `curl`/`jq` עבור אישורי הגדרות קבועות מראש של מערכות הפעלה ותקשורת עבור מסלול החזרה ליצירת טלמטריה:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

פלט לדוגמה:

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

עבור בדיקה של מתזמנים ב-Vivo עבור אל ליין `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto confirma que la instantanea de TEU, los metadatos de alias y los flags de manifest se alinean con la configuracion. מטען הטעינה המיועד הוא שימוש בלוח המחוונים של Grafana עבור לוח המחוונים לכניסת נתיב.

## 6. Ejercita los valores predeterminados del cliente

- **Rust/CLI.** `iroha_cli` y el crate cliente de Rust omiten el campo `lane_id` cuando no pasas `--lane-id` / `LaneSelector`. El Router de Colas por lo tanto recurre a `default_lane`. ארה"ב דגלים מפורשים `--lane-id`/`--dataspace-id` סולו cuando apuntes a una lane no predeterminada.
- **JS/Swift/Android.** גרסאות אולטימטיביות של לוס SDK tratan `laneId`/`lane_id` כמו אופציונליות y hacen fallback al valor anunciado por `/status`. Manten la politica de enrutamiento sincronizada entre staging y produccion para que las apps moviles no cecesiten reconfiguraciones de emergencia.
- **בדיקות Pipeline/SSE.** Los filtros de eventos de transacciones aceptan predicados `tx_lane_id == <u32>` (ver `docs/source/pipeline.md`). הירשם ל-`/v2/pipeline/events/transactions` עם זה פילטר עבור הדגמה que las escrituras enviadas sin un lane explicito llegan bajo el id de lane de fallback.

## 7. Observabilidad y ganchos de gobernanza

- `/status` tambien publica `nexus_lane_governance_sealed_total` y `nexus_lane_governance_sealed_aliases` para que Alertmanager pueda avisar cuando una lane pierde su manifest. Manten esas alertas habilitadas incluso and devnets.
- מפת הטלמטריה של לוח הזמנים ולוח המחוונים של ה-Gobernanza de lanes (`dashboards/grafana/nexus_lanes.json`) esperan los campos alias/slug del catalogo. אם כינוי זה, ראה את כללי הליווי של הכתוביות.
- Las aprobaciones parlamentarias para lanes predeterminadas deben incluir un plan de rollback. רשם אל האש דל מניפסט y la evidencia de gobernanza Junto con este quickstart en tu runbook de operador para que las rotaciones futuras no tengan que adivinar el estado requerido.Una vez que estas comprobaciones pasen puedes tratar `nexus.routing_policy.default_lane` como la fuente de verdad para la configuracion de los SDK y empezar a deshabilitar las rutas de codigo heredadas de lane unico en la red.