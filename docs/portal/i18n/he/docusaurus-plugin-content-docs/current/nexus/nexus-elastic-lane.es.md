---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-elastic-lane
כותרת: Provisionamiento de lane elastico (NX-7)
sidebar_label: Provisionamiento de lane elastico
תיאור: Flujo de bootstrap para crear manifests de lane Nexus, entradas de catalogo y Evidencia de rollout.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/nexus_elastic_lane.md`. Manten ambas copias alineadas hasta que el barrido de localizacion llegue al portal.
:::

# Kit de provisionamiento de lane elastico (NX-7)

> **אלמנט מפת הדרכים:** NX-7 - Tooling de provisionamiento de lane elastico  
> **Estado:** השלמה של כלים - מניפסטים מסוגים, קטעי קטלוג, מטענים Norito, פרועבאס דה הומו,
> y el helper de bundle de load-test ahora combina gating de latencia por slot + manifests de evidencia para que las corridas de carga de validadores
> se publicquen sin scripting a medida.

Esta guia lleva a los operadores por el nuevo helper `scripts/nexus_lane_bootstrap.sh` que automatize la generacion de manifest de lane, snippets of catalogo de lane/dataspace y Evidencia de rollout. El objetivo es facilitar el alta de nuevas lanes Nexus (publicas o privadas) sin editar a mano multiples archivos ni redrivar la geometria del catalogo a mano.

## 1. דרישות מוקדמות

1. Aprobacion de governance para el alias de lane, dataspace, conjunto de validadores, tolerancia a fallos (`f`) y Politica de Settlement.
2. רשימה סופית של validadores (מזהים של cuenta) y una רשימה של מרחבי שמות protegidos.
3. גישה למאגר תצורה של רכיבים עבור מקורות מידע נוספים.
4. Rutas para el registro de manifests de lane (ver `nexus.registry.manifest_directory` y `cache_directory`).
5. מגע של טלמטריה / ידיות של PagerDuty עבור ליין, מודם que las alertas se conecten en cuanto el lane este online.

## 2. Genera artefactos de lane

Ejecuta el helper desde la raiz del repositorio:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

דגלים קלאב:

- `--lane-id` debe coincidir con el index de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` y `--dataspace-id/hash` controlan la entrada del catalogo de dataspace (por defecto usa el id del lane cuando se omite).
- `--validator` puede repetirse o leerse desde `--validators-file`.
- `--route-instruction` / `--route-account` משחררים את הרשימה הרשמית.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) צור קשרים של Runbook עבור לוחות מחוונים מוסטרן לאוס תקנות.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` משדרגים את הוק לשדרוג זמן ריצה עם גילוי נאות של הנתיב מצריך שליטה על הרחבת ההפעלה.
- `--encode-space-directory` invoca `cargo xtask space-directory encode` אוטומטי. Combinelo con `--space-directory-out` cuando quiera que el archivo `.to` codificado vaya a un lugar distinto del default.

התסריט ייצור את הממצאים המוצגים ב-`--output-dir` (באמצעות ה-Defecto el Directorio בפועל), mas un cuarto optional cuando se habilita el קידוד:1. `<slug>.manifest.json` - מניפסט של ליין que contiene el quorum de validadores, מרחבי שמות מוגנים ו-metadatos אופציונליים של הוק לשדרוג זמן ריצה.
2. `<slug>.catalog.toml` - un snippet TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` y cualquier regla de enrutamiento solicitada. Asegura que `fault_tolerance` מוגדר ב-Entrada de dataspace למען מימדים ל-comite de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoria que describe la geometria (שבלול, segmentos, metadatos) mas los pasos de rollout requeridos y el comando exacto de `cargo xtask space-directory encode` (bajo `space_directory_encode.command`). Adjunta este JSON לכל כרטיס כניסה למטוס como evidencia.
4. `<slug>.manifest.to` - emitido cuando `--encode-space-directory` esta activado; listo para el flujo `iroha app space-directory manifest publish` de Torii.

ארה"ב `--dry-run` לפרויזואליזר של JSON/קטעי ארכיון, ו-`--force` עבור תיאורי חפצים קיימים.

## 3. Aplica los cambios

1. Copia el Manifest JSON en el `nexus.registry.manifest_directory` configurado (y en el directorio cache si el registro refleja bundles remotos). Commitea el archivo si los manifests se versionan en tu repo de configuracion.
2. Anexa el snippet de catalogo a `config/config.toml` (o al `config.d/*.toml` correspondiente). Asegura que `nexus.lane_count` sea al menos `lane_id + 1` y actualiza cualquier `nexus.routing_policy.rules` que deba apuntar al nuevo lane.
3. Codifica (שמטה `--encode-space-directory`) y publica el manifest en el Space Directory usando el comando capturado en el summary (`space_directory_encode.command`). Esto produce el payload `.manifest.to` que Torii espera y registra la evidencia para auditores; envia con `iroha app space-directory manifest publish`.
4. Ejecuta `irohad --sora --config path/to/config.toml --trace-config` y archiva la salida de trace en el ticket de rollout. Esto prueba que la nueva geometria עולה בקנה אחד con el slug/segmentos de Kura generados.
5. Reinicia los validadores asignados al lane una vez que los cambios de manifest/catalogo esten desplegados. שמירת תקציר JSON וכרטיס עבור עתידיים.

## 4. בניית חבילה של הפצת רישום

Empaqueta el manifest generado y el overlay para que los operadores puedan distribuir datas de gobernanza de lanes syn configs editar in cada host. העוזר של חבילת עותק מניפסטים ב-el layout canonico, הפקת שכבת-על אופציונלית של קטלוג ה-gobernanza עבור `nexus.registry.cache_directory`, והעברה לא מקוונת עבור העברות:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

סלידאס:

1. `manifests/<slug>.manifest.json` - copia estos archivos en el `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - deja esto en `nexus.registry.cache_directory`. מארז ה-`--module` ניתן להגדיר בצורה מודפסת, ניתנות להחלפה של מודולים של גוברננזה (NX-2) כדי לממש את שכבת המטמון עם עריכה `config.toml`.
3. `summary.json` - כולל גיבוב, מטדי שכבת-על והוראות להפעלה.
4. אופציונלי `registry_bundle.tar.*` - listo para SCP, S3 או trackers de artefactos.סינכרוניזה לעשות את המדריך (או אל הארכיון) קוד אימות, אקסטרה מארח אוויר-gaped y copia los manifests + שכבת מטמון על גבי קוד קוד רישום אנטה של ​​reiniciar Torii.

## 5. Pruebas de humo para validadores

Despues de reiniciar Torii, ejecuta el nuevo helper de smoke para verificar que el lane reporte `manifest_ready=true`, que las metricas expongan el conteo esperado de lanes y que el gauge de sealed este limpio. Las lanes que requieren manifests deben exponer un `manifest_path` no vacio; el helper ahora falla de inmediato cuando falta la ruta para que cada despliegue NX-7 incluya la evidencia del manifest firmado:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agrega `--insecure` cuando pruebes entornos con certificados בחתימה עצמית. El script termina con codigo no cero si el lane falta, esta sealed o las metricas/telemetria se desalinean de los valores esperados. Usa los knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` para mantener la telemetria por lane (altura de bloque/finalidad/backlog/headroom) `--max-slot-p95` / `--max-slot-p99` (mas `--min-slot-samples`) para imponer los objetivos de duracion de slot NX-18 sin salir del helper.

Para validaciones air-gapped (o CI) puedes reproducir una respuesta Torii capturada in lugar de golpear un point vivo:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Los fixtures grabados bajo `fixtures/nexus/lanes/` reflejan los artefactos producidos por el helper de bootstrap para que los nuevos manifests se puedan lint-ear sin scripting a medida. CI ejecuta el mismo flujo via `ci/check_nexus_lane_smoke.sh` y `ci/check_nexus_lane_registry_bundle.sh` (כינוי: `make check-nexus-lanes`) para demostrar que el helper de smoke NX-7 siga estando alineado con el formato deload payload publicado/man paradelay ניתנים לשחזור.