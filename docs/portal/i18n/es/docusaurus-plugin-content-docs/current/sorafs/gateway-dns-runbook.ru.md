---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска Gateway y DNS SoraFS

Esta copia en el portal de contenido canónico en
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Las barreras de seguridad operativas de la tecnología para DNS y puerta de enlace descentralizados,
чтобы лиды по networking, ops и документации могли отрепетировать стек
автоматизации перед kickoff 2025-03.

## Область и entregables

- Cambie el DNS (SF-4) y la puerta de enlace (SF-5) cuando se repitan los parámetros
  деривации хостов, релизов каталога resolvers, автоматизации TLS/GAR y
  сбора доказательств.
- Держать kickoff-артефакты (agenda, invitación, seguimiento de asistencia, instantánea
  телеметрии GAR) sincronizados con los propietarios actuales.
- Подготовить аудируемый paquete de artefactos para los revisores de gobernanza: notas de la versión
  resolutores de catálogo, sondas de puerta de enlace de circuitos, arnés de conformidad y productos
  Documentos/DevRel.

## Роли и ответственности| Flujo de trabajo | Ответственности | Требуемые артефакты |
|------------|-----------|---------------------|
| Redes TL (pila DNS) | Puede configurar archivos de planes determinados, descargar versiones del directorio RAD y publicar solucionadores de televisores. | `artifacts/soradns_directory/<ts>/`, diferencias para `docs/source/soradns/deterministic_hosts.md`, metadatos RAD. |
| Líder de automatización de operaciones (puerta de enlace) | Выполнять taladros автоматизации TLS/ECH/GAR, запускать `sorafs-gateway-probe`, обновлять ganchos PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonda JSON, записи в `ops/drill-log.md`. |
| Grupo de trabajo sobre herramientas y gremio de control de calidad | Descargue `ci/check_sorafs_gateway_conformance.sh`, compre accesorios, compre paquetes de autocertificación Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Фиксировать actas, обновлять diseño prelectura + apéndices, публиковать resumen de evidencia en el portal. | Обновленные `docs/source/sorafs_gateway_dns_design_*.md` y notas de lanzamiento. |

## Входы и requisitos previos

- Especificaciones de los dispositivos de configuración (`docs/source/soradns/deterministic_hosts.md`) y
  atestación de tarjeta de resolución (`docs/source/soradns/resolver_attestation_directory.md`).
- Артефакты gateway: manual del operador, ayudantes de automatización TLS/ECH,
  orientación en modo directo y flujo de trabajo de autocertificación en `docs/source/sorafs_gateway_*`.
- Útiles: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` y ayudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secretos: clave de acceso GAR, credenciales DNS/TLS ACME, clave de enrutamiento PagerDuty,
  Torii token de autenticación para buscar resolutores.

## Lista de verificación previa al vuelo1. Подтвердите участников и agenda, обновив
   `docs/source/sorafs_gateway_dns_design_attendance.md` y разослав текущую
   agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовьте корни артефактов, например
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Actualizar accesorios (manifiestos GAR, pruebas RAD, puerta de enlace de conformidad de paquetes) y
   убедитесь, что состояние `git submodule` соответствует последнему rehearsal tag.
4. Проверьте secretos (clave de liberación Ed25519, archivo de cuenta ACME, token PagerDuty) y
   соответствие sumas de comprobación en bóveda.
5. Proporcionar objetivos de telemetría de prueba de humo (punto final Pushgateway, placa GAR Grafana)
   antes del taladro.

## Шаги репетиции автоматизации

### Детерминированная карта хостов y lanzamiento del catálogo RAD

1. Запустите helper детерминированной деривации хостов на предложенном наборе
   manifiestos y подтвердите отсутствие deriva относительно
   `docs/source/soradns/deterministic_hosts.md`.
2. Сгенерируйте paquetes de resolución de catálogos:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Utilice el catálogo de identificación personal, SHA-256 y máquinas de escribir actualizadas
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и в minutos de inicio.

### Захват телеметрии DNS

- Registros de transparencia del solucionador de cola en una duración de ≥10 minutos
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exportar parámetros de Pushgateway y almacenar instantáneas de NDJSON en el sitio
  Directorios ejecute ID.

### Taladros автоматизации puerta de enlace

1. Utilice la sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```2. Arnés de conformidad de ajuste (`ci/check_sorafs_gateway_conformance.sh`) y
   ayudante de autocertificación (`scripts/sorafs_gateway_self_cert.sh`) para la actualización
   Paquete de atestación Norito.
3. Utilice la función PagerDuty/Webhook para lograr un funcionamiento de extremo a extremo
   пути автоматизации.

### Упаковка доказательств

- Обновите `ops/drill-log.md` con marcas de tiempo, участниками y sondas hashes.
- Actualizar artefactos en directorios ejecutar ID y publicar resumen ejecutivo
  â minutos Docs/DevRel.
- Solicite un paquete de evidencia en el ticket de gobernanza para la revisión inicial.

## Модерация сессии и передача доказательств- **Moderador de la línea de tiempo:**
  - T-24 h — Gestión de programas публикует напоминание + instantánea de agenda/asistencia en `#nexus-steering`.
  - T-2 h — Networking TL обновляет snapshot телеметрии GAR and фиксирует deltas в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation proporciona sondas de arranque y activa ID de ejecución en `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор делится этим ранбуком и назначает live scribe; Docs/DevRel contiene elementos de acción en este momento.
- **Шаблон minutos:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (se incluye en el paquete del portal)
  и коммитьте заполненный экземпляр на каждую сессию. Включите список участников,
  решения, elementos de acción, pruebas hashes y открытые риски.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из ensayo,
  приложите отрендеренный PDF minutas, запишите SHA-256 hashes в minutas + agenda,
  затем уведомите alias del revisor de gobernanza после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (inicio de marzo de 2025)

Последние rehearsal/live артефакты, упомянутые в roadmap and minutes, лежат в bucket
`s3://sora-governance/sorafs/gateway_dns/`. Хэши ниже отражают канонический
manifiesto (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Ejecución en seco — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Paquete Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Acta PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Taller en vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Carga pendiente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel incluye SHA-256 después de publicar PDF en paquete)._

## Materiales nuevos

- [Guía de operaciones de la puerta de enlace](./operations-playbook.md)
- [План наблюдаемости SoraFS](./observability-plan.md)
- [Трекер децентрализованного DNS y puerta de enlace](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)