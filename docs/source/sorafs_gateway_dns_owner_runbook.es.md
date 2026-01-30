---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_owner_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19f43d387ad49962c51a8d9565cc7c92bdff1f3d1af23b11392e66e120af92d4
source_last_modified: "2025-11-18T09:03:32.952411+00:00"
translation_last_reviewed: "2026-01-30"
---

# Runbook de owners de SoraFS Gateway y DNS

Este runbook cierra el follow-up de **Decentralized DNS & Gateway** trazado en
`roadmap.md` al detallar cómo los tres owners responsables coordinan la
automatización DNS, promoción de alias, muestreo de telemetría y drills de
rollback antes del kickoff del 2025‑03‑03. Complementa el tracker de asistencia
(`docs/source/sorafs_gateway_dns_design_attendance.md`), el pre-read y el
snapshot de telemetría GAR, y debe actualizarse cuando cambien las herramientas
referenciadas o los buckets de evidencia.

## 1. Roles y alcance

| Alcance | Owner primario | Backup | Inputs / tooling |
|-------|---------------|--------|------------------|
| Automatización de zonefile SoraDNS y pinning GAR | Ops Lead (`ops.lead@soranet`) | Networking TL (`networking.tl@soranet`) | `tools/soradns-resolver/`, `docs/source/sns/governance_playbook.md`, `promtool`, `ops/drill-log.md` |
| Alias de gateway y metadatos de cutover SoraFS | Tooling WG (`tooling.wg@sorafs`) | Docs/DevRel (`docs.devrel@sora`) | `docs/portal/scripts/sorafs-pin-release.sh`, `docs/portal/scripts/generate-dns-cutover-plan.mjs`, guía de despliegue del portal |
| Snapshots de telemetría y automatización de rollback | QA Guild (`qa.guild@sorafs`) | Security Engineering (`security@soranet`) | `scripts/telemetry/run_soradns_transparency_tail.sh`, `scripts/telemetry/schedule_soradns_ir_drill.sh`, notas de telemetría GAR |

## 2. Timeline y checklist de evidencia

| Ventana | Owner(s) | Checklist | Evidencia |
|--------|----------|-----------|----------|
| T‑14 días | Tooling WG + Docs/DevRel | Build del portal, correr el packaging script en modo dry-run y emitir el descriptor de cutover DNS.<br>`npm ci && npm run build && ./docs/portal/scripts/sorafs-pin-release.sh --skip-submit --dns-change-ticket OPS-XXXX --dns-cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z --dns-hostname docs.sora.link --dns-zone sora.link --ops-contact ops.lead@soranet --cache-purge-endpoint https://cache.api/purge --cache-purge-auth-env CACHE_PURGE_TOKEN --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json` | `artifacts/sorafs/portal.pin.report.json`, `artifacts/sorafs/portal.additional_assets.json`, `artifacts/sorafs/portal.dns-cutover.json`, `artifacts/sorafs/portal.gateway.binding.json`, `artifacts/sorafs/portal.gateway.headers.txt`, `artifacts/sorafs/gateway.route_plan.json`, `artifacts/sorafs/checksums.sha256`, ticket de deploy del portal |
| T‑10 días | Ops Lead | Actualizar el skeleton del zonefile según `docs/source/sns/governance_playbook.md`, guardar en `artifacts/sns/zonefiles/<zone>/<version>.json`, y verificar que el resolver tome los nuevos registros:<br>`cargo run -p soradns-resolver -- --config ops/soradns/resolver.staging.json`<br>`python3 scripts/sns_zonefile_skeleton.py --cutover-plan artifacts/sorafs/portal.dns-cutover.json --out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json --resolver-snippet-out ops/soradns/static_zones.docs.json --ipv4 <gateway-ip> --ttl 600 --zonefile-version 20250303.docs.sora --effective-at 2025-03-03T16:00Z --gar-digest <gar-digest-hex> --freeze-state soft --freeze-ticket SNS-DF-XXXX --freeze-expires-at 2025-03-10T12:00Z --freeze-note "guardian review" --txt ChangeTicket=OPS-XXXX` | JSON de zonefile, log de sync del resolver, commit hash de `ops/soradns/resolver.staging.json` |

El helper ahora estampa metadata `zonefile.{name,version,ttl,effective_at,cid,gar_digest,proof}` junto a
`static_zone`. Apunta siempre `--effective-at` al inicio de la ventana de cutover, fija
`--zonefile-version` al ticket/tag de change control y captura el digest GAR emitido por
la automatización de firmado (o pásalo manualmente una vez el sobre GAR esté stageado). Deja
`--zonefile-proof` vacío salvo que la automatización no pueda exponer el header `Sora-Proof`—en
ese caso copia el literal de proof desde la entrada del runbook del gateway para que la evidencia
Torii/resolver coincida con el bundle GAR.
| T‑7 días | QA Guild + Security | Capturar snapshot de telemetría GAR, regenerar scrape Prometheus y actualizar la nota de telemetría GAR:<br>`scripts/telemetry/run_soradns_transparency_tail.sh --log /var/log/soradns/transparency.log --metrics-output docs/source/sorafs_gateway_dns_design_metrics_$(date +%Y%m%d).prom --format jsonl` | `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` actualizado, nuevo artefacto `.prom`, screenshot de alerta |
| T‑5 días | Ops Lead + Docs | Agendar el ensayo DNS/GAR vía el drill logger:<br>`scripts/telemetry/schedule_soradns_ir_drill.sh --date 2025-02-27 --log ops/drill-log.md --notes "DNS automation rehearsal for kickoff"` | Entrada en `ops/drill-log.md`, invitación de calendario |
| T‑2 días | Todos los owners | Revisar `artifacts/sorafs/portal.dns-cutover.json`, confirmar `change_ticket`, `cutover_window`, alias namespace/name, bloque de purge (endpoint/payload/auth var), metadata de rollback, el `gateway_binding` embebido (CID de contenido + plantilla de headers) y el nuevo stanza `route_plan` (path, headers primarios + rollback). Re-ejecutar `iroha app sorafs gateway route-plan …` (o `scripts/sorafs-gateway route plan …` desde CI) para que `artifacts/sorafs_gateway/route_plan.json` más las plantillas de headers (`gateway.route.headers.txt`, `gateway.route.rollback.headers.txt` si aplica) reflejen el manifiesto final. Adjuntar el descriptor + bundle de headers/route plan al ticket de rollout y al hilo de Slack. | Comentario del ticket de rollout con diff del descriptor + route plan |

> **Nuevo paso de validación SN-7:** El script de packaging ejecuta
> `cargo xtask soradns-verify-binding --binding artifacts/sorafs/portal.gateway.binding.json`
> (más flags de alias/hostname/manifest) automáticamente, pero debes re-ejecutarlo
> cuando el archivo binding se ajuste fuera de CI. El helper decodifica `Sora-Proof`,
> verifica que el route binding coincide con el CID del manifiesto + hostname y
> falla si los headers cambian. Captura el output de consola en el ticket para
> que los reviewers vean la evidencia en línea.
| Cutover (T) | Ops + Tooling + QA | Aplicar zonefile, reiniciar resolvers y correr el probe del portal listado en el descriptor:<br>`npm run probe:portal -- --base-url=https://docs.sora.link --expect-release=$(jq -r '.release.tag' artifacts/sorafs/portal.dns-cutover.json)`.<br>Monitorear `torii_sorafs_gar_violations_total`/`torii_sorafs_gateway_refusals_total`. | Bundle `artifacts/sorafs/dns-cutover/<timestamp>/` con logs del resolver, output del probe del portal, screenshot Grafana |
| T + 1 día | QA Guild + Security | Re-ejecutar el transparency tailer con upload a Pushgateway, archivar update post-cutover de telemetría y guardar evidencia firmada en Governance DAG. | `artifacts/soradns/transparency.prom`, link de log governance, nota en `status.md` |

## 3. Procedimientos de automatización

### 3.1 Packaging de release y metadata de alias (Tooling WG / Docs)

1. Construir el portal docs (`npm ci && npm run build`), asegurando que
   `build/checksums.sha256` exista (generado por `docs/portal/scripts/write-checksums.mjs` vía `postbuild`).
2. Ejecutar `docs/portal/scripts/sorafs-pin-release.sh`, suministrando metadata DNS vía flags
   o env vars `DNS_*`. Incluir `--skip-submit` para ensayo; omitirlo al promocionar
   a Torii. El script emite:
   - `artifacts/sorafs/portal.pin.report.json`
   - `artifacts/sorafs/portal.additional_assets.json`
   - `artifacts/sorafs/portal.manifest.to` + `.json`
   - `artifacts/sorafs/portal.dns-cutover.json` (vía llamada embebida a
     `docs/portal/scripts/generate-dns-cutover-plan.mjs`)
   - `artifacts/sorafs/portal.gateway.binding.json`
   - `artifacts/sorafs/portal.gateway.headers.txt`
3. Cuando el descriptor requiera edits (p. ej., ventana actualizada), re-ejecutar el generador directamente:

```bash
node docs/portal/scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --change-ticket OPS-4242 \
  --cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact ops.lead@soranet \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json \
  --out artifacts/sorafs/portal.dns-cutover.json
```

4. Adjuntar `portal.dns-cutover.json`, `portal.pin.report.json`, `portal.additional_assets.json`,
   `portal.gateway.binding.json`, `portal.gateway.headers.txt` y el bundle de manifiesto firmado
   al ticket de rollout. Resaltar el comando `cache_invalidation` y el bloque `rollback`, y
   actualizar la guía de despliegue (`docs/portal/docs/devportal/deploy-guide.md`) con uso de
   flags nuevos o metadata del release-tag.

### 3.2 Automatización de zonefile y resolver (Ops Lead / Networking TL)

1. Generar el skeleton de zonefile siguiendo §4.5 de
   `docs/source/sns/governance_playbook.md` con el helper:
   ```bash
   python3 scripts/sns_zonefile_skeleton.py \
     --cutover-plan artifacts/sorafs/portal.dns-cutover.json \
     --out artifacts/sns/zonefiles/<zone>/<timestamp>.<alias>.json \
     --resolver-snippet-out ops/soradns/static_zones.<alias>.json \
     --ipv4 <gateway-ip> --ipv6 <gateway-ipv6> --ttl 600 \
     --freeze-state <soft|hard|thawing|monitoring|emergency> \
     --freeze-ticket SNS-DF-XXXX \
     --freeze-expires-at 2025-03-10T12:00Z \
     --freeze-note "guardian review" \
     --txt ChangeTicket=OPS-XXXX --spki-pin <base64pin>
   ```
   Los flags de freeze (`--freeze-ticket`, `--freeze-expires-at`, `--freeze-note`)
   requieren `--freeze-state`; el helper ahora falla rápido si falta metadata de
   freeze, para mantener evidencia consistente. Archiva el JSON bajo
   `artifacts/sns/zonefiles/<zone>/<YYYYMMDD>.json` y anota los hashes de selector.
   Cuando hay freeze en efecto, el helper registra metadata bajo `freeze` y emite
   TXT `Sora-Freeze-*` iguales para que la automatización del resolver, dashboards
   y comms de clientes referencien el mismo ticket y expiración. El snippet del resolver
   escrito en `ops/soradns/static_zones.<alias>.json` refleja el bloque `freeze` para
   que guardians prueben propagación—`soradns-resolver` rehúsa queries DNS cuando el
   selector está `soft`, `hard` o `emergency` frozen (retornando `SERVFAIL` o `REFUSED`),
   mientras que `thawing` y `monitoring` siguen sirviendo registros para rollbacks
   escalonados. El skeleton de zonefile solo define registros de la zona autoritativa.
   No configura delegación NS/DS del parent-zone en el registrador; esa delegación se
   configura por separado para resolución pública.
2. Actualizar la configuración del resolver (ruta ejemplo `ops/soradns/resolver.staging.json`)
   para que el nuevo bundle y las fuentes RAD apunten a los artefactos stageados.
3. Validar la configuración localmente:

```bash
cargo run -p soradns-resolver -- --config ops/soradns/resolver.staging.json
```

Esto realiza un sync pass, logueando conteos de bundle y RAD. Captura el log y
archívalo junto al artefacto de zonefile.

4. Tail del log de transparencia del resolver para asegurar que los bundles llegaron:

```bash
scripts/telemetry/run_soradns_transparency_tail.sh \
  --log /var/log/soradns/resolver.transparency.log \
  --metrics-output artifacts/soradns/transparency.prom \
  --format jsonl
```

5. Ejecutar los tests de alertas antes de cada cutover:

```bash
promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml
```

6. Commitear diffs del resolver/event-log y referenciarlos en `ops/drill-log.md`
   al cerrar ensayos.

### 3.3 Evidencia de telemetría y readiness de rollback (QA Guild / Security)

1. Refrescar el archivo snapshot de telemetría GAR (`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`)
   scrapeando métricas de staging en un nuevo artefacto `.prom` (ver §2 timeline).
2. Verificar que los dashboards GAR/telemetría referenciados en el roadmap siguen verdes
   y exportar PNGs para el bundle de evidencia.
3. Preparar comandos de rollback registrando:
   - Invocación `sorafs_cli manifest revert` con el digest del manifiesto previo (disponible en
     `rollback.manifest_digest_hex` dentro de `portal.dns-cutover.json`).
   - Plan DNS inverso derivado del `rollback.plan_path` del descriptor (cambiar hostname a
     `docs.staging.sora.link`, reutilizar `npm run probe:portal` para confirmar rollback).
4. Mantener `scripts/telemetry/inject_redaction_failure.sh` listo para drills de caos
   y documentar anomalías en el drill log.

## 4. Ensayo de automatización y drills de incidentes

- Programar ensayos con
  `scripts/telemetry/schedule_soradns_ir_drill.sh --log ops/drill-log.md`.
  Incluir etiqueta del escenario DNS/GAR, IC, scribe y notas sobre rotaciones de
  artefactos.
- Durante ensayos, correr la toolchain completa end-to-end (packaging del portal, publish
  de zonefile, sync del resolver, scrape de telemetría) y registrar:
  - Artefactos generados bajo `artifacts/sorafs/dns-rehearsal/<date>/`
  - Entrada en `ops/drill-log.md` con links a artefactos y tickets JIRA de seguimiento.
- Confirmar que `docs/portal/scripts/__tests__/dns-cutover-plan.test.mjs` está verde
  para detectar regresiones del descriptor en CI.

## 5. Checklist del día de cutover

1. **Congelar inputs:** bloquear el bundle de manifiesto y artefactos de zonefile en Git
   (`git tag dns-cutover-20250303`) y compartir el SHA en `#sorafs-gateway`.
2. **Aplicar zonefile:** publicar el skeleton firmado al bucket del resolver y recargar
   los resolvers. Vigilar `soradns_bundle_proof_age_seconds` por regresiones.
3. **Verificar resolver + DNS:** ejecutar `cargo run -p soradns-resolver -- --config ...`
   en al menos un nodo de producción para confirmar que los nuevos registros sirven.
   Luego ejecutar `dig +dnssec docs.sora.link` apuntando a cada endpoint del resolver.
4. **Probe del portal:** ejecutar el comando de verificación embebido en
   `portal.dns-cutover.json` (típicamente `npm run probe:portal ...`) y adjuntar el
   output JSON al ticket de cambios.
5. **Headers del gateway:** aplicar `gateway_binding.headers_template` (o el adjunto
   regenerado `gateway.route.headers.txt`) a la automatización de edge para que el
   bundle `Sora-Name/Sora-Proof/CSP` coincida con el release. Capturar el snippet
   renderizado y guardarlo con los artefactos. Si el route plan incluye metadata de
   rollback, archivar `gateway.route.rollback.headers.txt` con el ticket para poder
   volver sin recalcular headers.
6. **Purgado de caché:** si el descriptor incluye un bloque `cache_invalidation`,
   ejecutar el `curl` registrado (sustituyendo el token real) y archivar el output.
7. **Vigilancia de telemetría:** mantener en vista los dashboards
   `torii_sorafs_gar_violations_total`, `torii_sorafs_gateway_refusals_total` y
   `torii_sorafs_tls_cert_expiry_seconds` por ≥30 min. Exportar snapshots PNG y
   añadirlos al bundle.
8. **Triggers de rollback:** si las violaciones GAR suben o falla el probe,
   - Revertir la entrada DNS usando el skeleton de zonefile previo.
   - Ejecutar `npm run probe:portal` contra el hostname anterior.
   - Abrir un incidente según `docs/portal/docs/devportal/incident-runbooks.md`.

## 6. Evidencia y reportes

- Empaquetar artefactos de cada run bajo
  `artifacts/sorafs/dns-cutover/<YYYYMMDD>/` con subdirectorios para
  `portal/`, `zonefiles/`, `telemetry/` y `drills/`.
- Actualizar `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` después de
  cada snapshot y referenciar el archivo `.prom` nuevo.
- Registrar completion en `status.md` (Latest Updates) con referencia al roadmap
  para que governance vea la trazabilidad.
- Proveer un resumen en el refresh semanal de `status.md` y en el deck del kickoff
  una vez cambie el runbook.

Mantener este documento al día es obligatorio para la preparación SF‑4/SF‑5; si
algún flag de CLI, ruta de evidencia u owner cambia, actualizar este runbook y
linkear el cambio en el ticket de rollout asociado.
