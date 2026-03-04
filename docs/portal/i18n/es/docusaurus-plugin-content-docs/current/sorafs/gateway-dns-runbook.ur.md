---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS گیٹ وے اور DNS کک آف رن بک

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) میں ہے۔
یہ DNS y puerta de enlace descentralizados ورک اسٹریم کے آپریشنل گارڈ ریلز کو سمیٹتی ہے تاکہ
نیٹ ورکنگ، آپس، اور ڈاکیومنٹیشن لیڈز 2025-03 کے کک آف سے پہلے آٹومیشن اسٹیک کی
مشقیق کر سکیں۔

## اسکوپ اور ڈلیوریبلز

- Hitos de DNS (SF-4) y puerta de enlace (SF-5) y derivación de host determinista
  Lanzamientos del directorio de resolución, automatización TLS/GAR, captura de evidencia کی مشق شامل ہو۔
- کک آف ان پٹس (agenda, invitación, rastreador de asistencia, instantánea de telemetría GAR) کو
  تازہ ترین asignaciones de propietarios کے ساتھ ہم آہنگ رکھنا۔
- گورننس ریویورز کے لیے قابلِ آڈٹ paquete de artefactos تیار کرنا: directorio de resolución
  notas de la versión, registros de sonda de puerta de enlace, salida del arnés de conformidad, resumen de Docs/DevRel ۔

## رولز اور ذمہ داریاں| ورک اسٹریم | ذمہ داریاں | Objetos de arte |
|------------|------------|------------------|
| Redes TL (pila DNS) | plan de host determinista برقرار رکھنا، Lanzamientos de directorio RAD چلانا، entradas de telemetría de resolución شائع کرنا۔ | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` Diferencias y metadatos RAD |
| Líder de automatización de operaciones (puerta de enlace) | Taladros automáticos TLS/ECH/GAR چلانا، `sorafs-gateway-probe` چلانا، Ganchos PagerDuty اپڈیٹ کرنا۔ | `artifacts/sorafs_gateway_probe/<ts>/`, sonda JSON, entradas `ops/drill-log.md`۔ |
| Grupo de trabajo sobre herramientas y gremio de control de calidad | `ci/check_sorafs_gateway_conformance.sh` Dispositivos de autocertificación Norito Paquetes de autocertificación آرکائیو کرنا۔ | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | minutos ریکارڈ کرنا، diseño prelectura + apéndices اپڈیٹ کرنا، اور اسی پورٹل میں resumen de evidencia شائع کرنا۔ | اپڈیٹ شدہ `docs/source/sorafs_gateway_dns_design_*.md` فائلز اور notas de lanzamiento۔ |

## ان پٹس اور پری ریکوائرمنٹس

- especificación de host determinista (`docs/source/soradns/deterministic_hosts.md`) para resolver
  andamio de certificación (`docs/source/soradns/resolver_attestation_directory.md`).
- artefactos de puerta de enlace: manual del operador, ayudantes de automatización TLS/ECH, guía en modo directo,
  Un flujo de trabajo de autocertificación como `docs/source/sorafs_gateway_*` کے تحت ہے۔
- Útiles: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, ayudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secretos: clave de liberación GAR, credenciales DNS/TLS ACME, clave de enrutamiento PagerDuty,
  El solucionador obtiene el token de autenticación Torii

## پری فلائٹ چیک لسٹ1. `docs/source/sorafs_gateway_dns_design_attendance.md` اپڈیٹ کر کے شرکاء اور agenda
   کنفرم کریں اور موجودہ agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`) شیئر کریں۔
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/` جیسے raíces de artefactos تیار کریں۔
3. accesorios (manifiestos GAR, pruebas RAD, paquetes de conformidad de puerta de enlace) ریفریش کریں اور
   یقینی بنائیں کہ `git submodule` کی حالت تازہ ترین etiqueta de ensayo سے میچ کرتی ہے۔
4. secretos (clave de versión Ed25519, archivo de cuenta ACME, token PagerDuty) ویریفائی کریں اور
   sumas de comprobación de bóveda کے ساتھ میچ کریں۔
5. Perforar objetivos de telemetría (punto final Pushgateway, placa GAR Grafana) y prueba de humo

## pasos de ensayo de آٹومیشن

### Mapa de host determinista y lanzamiento del directorio RAD

1. تجویز کردہ manifiesta سیٹ کے خلاف ayudante determinista de derivación de host چلائیں اور
   تصدیق کریں کہ `docs/source/soradns/deterministic_hosts.md` کے مقابلے میں کوئی deriva نہیں۔
2. paquete de directorio de resolución تیار کریں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. پرنٹ شدہ ID de directorio, SHA-256, اور rutas de salida کو
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور minutos de inicio میں ریکارڈ کریں۔

### Captura de telemetría DNS

- registros de transparencia del solucionador کو ≥10 منٹ تک tail کریں:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exportación de métricas de Pushgateway کریں اور NDJSON instantáneas کو ejecutar directorio ID کے ساتھ آرکائیو کریں۔

### Simulacros de automatización de puertas de enlace

1. Sonda TLS/ECH Modo:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. arnés de conformidad (`ci/check_sorafs_gateway_conformance.sh`) y ayudante de autocertificación
   (`scripts/sorafs_gateway_self_cert.sh`) چلائیں تاکہ Paquete de atestación Norito ریفریش ہو۔
3. Captura de eventos de PagerDuty/Webhook کریں تاکہ آٹومیشن پاتھ de extremo a extremo ثابت ہو۔

### Embalaje de pruebas

- `ops/drill-log.md` Marcas de tiempo, participantes y hashes de sonda
- ejecutar directorios de ID میں artefactos محفوظ کریں اور Docs/DevRel minutos میں resumen ejecutivo شائع کریں۔
- revisión inicial سے پہلے boleto de gobernanza میں paquete de evidencia لنک کریں۔

## سیشن فیسلیٹیشن اور entrega de pruebas- **Cronología del moderador:**
  - T-24 h — Gestión de programas `#nexus-steering` Recordatorio de میں + instantánea de agenda/asistencia پوسٹ کرے۔
  - T-2 h — Instantánea de telemetría TL GAR de red ریفریش کر کے `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں deltas ریکارڈ کرے۔
  - T-15 m — Preparación de la sonda de automatización de operaciones ویریفائی کر کے فعال run ID کو `artifacts/sorafs_gateway_dns/current` میں لکھے۔
  - کال کے دوران — Moderador یہ رن بک شیئر کرے اور asignación de escriba en vivo کرے؛ Elementos de acción en línea de Docs/DevRel ریکارڈ کرے۔
- **Plantilla de minutos:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` سے esqueleto کاپی کریں (paquete de portal میں بھی ہے) اور ہر سیشن کے لیے ایک مکمل confirmación de instancia کریں۔ lista de asistentes, decisiones, elementos de acción, hashes de evidencia, riesgos pendientes شامل کریں۔
- **Carga de evidencia:** ensayo کے `runbook_bundle/` directorio کو zip کریں، minutos renderizados PDF adjuntar کریں، minutos + agenda میں SHA-256 hashes لکھیں، اور cargas کے بعد alias del revisor de gobernanza کو ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## Instantánea de la evidencia (inicio en marzo de 2025)

hoja de ruta اور minutos میں حوالہ دیے گئے تازہ ترین ensayo/artefactos en vivo
`s3://sora-governance/sorafs/gateway_dns/` cucharón میں ہیں۔ نیچے دیے گئے hashes
manifiesto canónico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کو reflejar کرتے ہیں۔- **Ejecución en seco — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Paquete tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Acta PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Taller en vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Carga pendiente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF آنے پر SHA-256 شامل کرے گا۔)_

## Material relacionado

- [Guía de operaciones de puerta de enlace](./operations-playbook.md)
- [Plan de observabilidad SoraFS](./observability-plan.md)
- [Rastreador de DNS y puerta de enlace descentralizado] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)