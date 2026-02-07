---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ספר ההפעלה של Gateway y DNS de SoraFS

Esta copia del portal refleja el runbook canónico en
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Recoge las salvaguardas operativas del workstream de DNS y Gateway descentralizados
para que los leads de networking, ops y documentación puedan ensayar la pila de
אוטומטיזציה לפני הפתיחה 2025-03.

## Alcance y entregables

- Vincular los hitos de DNS (SF-4) y gateway (SF-5) ensayando la derivación
  קביעת מארחים, מהדורות של מנהלי פתרונות, אוטומטיזציה של TLS/GAR
  y captura de evidencias.
- Mantener los insumos del kickoff (סדר יום, הזמנה, tracker de asistencia, תמונת מצב
  de telemetría GAR) sincronizados con las últimas asignaciones de owners.
- מפיק חבילה של חפצי אמנות לביקורת עבור בודקים דה גוברננזה: notas de
  שחרור מדריכי פתרונות, יומני שער, סלידה רתמה
  de conformidad y el resumen de Docs/DevRel.

## תפקידים ובעלי אחריות

| זרם עבודה | אחריות אחריות | Artefactos requeridos |
|------------|----------------|------------------------|
| רשת TL (מחסנית DNS) | ניהול התוכנית קובעת את המארחים, הוצאת מהדורות של מדריך RAD, תשומות פומביות של טלמטריה של פותרים. | `artifacts/soradns_directory/<ts>/`, הבדלים של `docs/source/soradns/deterministic_hosts.md`, מטא נתונים RAD. |
| Ops Automation Lead (שער) | מקדחות אוטומטיות של TLS/ECH/GAR, מתקן `sorafs-gateway-probe`, ווי אקטואליזציה של PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de probe, entradas en `ops/drill-log.md`. |
| QA Guild & Tooling WG | Ejecutar `ci/check_sorafs_gateway_conformance.sh`, אביזרי קוראר, חבילות ארכיון של אישור עצמי Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | דקות הרשם, בפועל קריאה מוקדמת של דיזינו + apéndices y publicar el resumen de evidencias en este portal. | Archivos actualizados `docs/source/sorafs_gateway_dns_design_*.md` y notes de rollout. |

## תנאים מוקדמים

- Especificación de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) y
  el andamiaje de atestación de resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefactos del gateway: ידני של מפעיל, עוזרי אוטומט TLS/ECH,
  מצב ישיר ופעולה של אישור עצמי `docs/source/sorafs_gateway_*`.
- כלי עבודה: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, y helpers de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- סודות: clave de release GAR, אישורי ACME DNS/TLS, מפתח ניתוב של PagerDuty,
  token de auth de Torii עבור רזולורים.

## רשימת רשימת טרום-ווילו1. אשר את התוכנית בפועל
   `docs/source/sorafs_gateway_dns_design_attendance.md` y circulando la agenda
   vigente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepara raíces de artefactos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. אביזרי Refresca (מציג GAR, pruebas RAD, bundles de conformidad del gateway) y
   asegura que el estado de `git submodule` coincida con el último tag de ensayo.
4. Verifica secretos (clave de release Ed25519, archivo de cuenta ACME, token de PagerDuty)
   y que coincidan con checksums de vault.
5. Haz smoke-test de los targets de telemetria (נקודת קצה של Pushgateway, tablero GAR Grafana)
   מקדחה אנטי.

## Pasos de ensayo de automatización

### מפה מוגדרת של המארחים ושחרור ה-RAD

1. Ejecuta el helper de derivación determinista de hosts contra el set de manifests
   propuesto y confirma que no haya drift respecto de
   `docs/source/soradns/deterministic_hosts.md`.
2. Genera un bundle de directorio de resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registra el ID del directorio, el SHA-256 y las rutas de salida impresas dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` y en las minutas del kickoff.

### Captura de telemetria DNS

- Haz tail de los logs de transparencia de resolvers durante ≥10 דקות usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- ייצוא מדדי Pushgateway וארכיון תמונות מצב NDJSON junto al
  מזהה מנהל הפעלה.

### תרגילים אוטומטיים של השער

1. Ejecuta la sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Ejecuta el harness de conformidad (`ci/check_sorafs_gateway_conformance.sh`) y
   el helper de self-cert (`scripts/sorafs_gateway_self_cert.sh`) עבור רענון
   el bundle de atestaciones Norito.
3. Captura eventos de PagerDuty/Webhook עבור הדגמה לאוטומציה
   funciona de extremo a extremo.

### Empaquetado de evidencias

- Actualiza `ops/drill-log.md` עם חותמות זמן, משתתפים ו-hashes de probes.
- Guarda los artefactos bajo los directorios de run ID y publica un resume ejecutivo
  en las minutas de Docs/DevRel.
- Enlaza el bundle de evidencias en el ticket de gobernanza antes de la revisión
  דל בעיטה.

## Facilitacion de sesion y entrega de evidencias- **Linea de tiempo del moderator:**
  - T-24 שעות — ניהול תוכניות publica el recordatorio + תמונת מצב של agenda/asistencia en `#nexus-steering`.
  - T-2 h — רשת TL מחודשת תמונת מצב של טלמטריה GAR ורישום דלתות ב-`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 מ' — אימות אוטומציה של אוטומציה הפעלה להכנה של בדיקות וכתובת זיהוי הפעלה של `artifacts/sorafs_gateway_dns/current`.
  - Durante la llamada - El moderador comparte este runbook y asigna un escriba en vivo; מסמכי Docs/DevRel מציבים פריטים ב-Accion en linea.
- **Plantilla de minutas:** Copia el esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (tambien espejado en el bundle
  del portal) y comitea una instancia completada por sesion. כולל רשימה של
  אסיסטנטס, החלטות, פריטים דה אציון, תקצירים של הוכחות y riesgos pendientes.
- **טעינת הוכחות:** הורד את המדריך `runbook_bundle/` del ensayo,
  תוספת ל-PDF de minutas renderizado, registra hashes SHA-256 in las minutas +
  אג'נדה y luego notifica alias de reviewers de gobernanza cuando las cargas
  aterricen en `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## תמונת Snapshot de evidencias (בעיטת מרץ 2025)

Los ultimos artefactos de ensayo/produccion referenciados en el מפת הדרכים y las minutas
viven en el bucket `s3://sora-governance/sorafs/gateway_dns/`. Los hashes abajo
reflejan el manifest canónico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **ריצה יבשה — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball del bunt: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - דקת PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **סדנה en vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(טעינה תלויה: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara el SHA-256 cuando el PDF renderizado llegue al bundle.)_

## relacionado חומרי

- [Operations Playbook del gateway](./operations-playbook.md)
- [Plan de observabilidad de SoraFS](./observability-plan.md)
- [מעקב אחר DNS descentralizado y gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)