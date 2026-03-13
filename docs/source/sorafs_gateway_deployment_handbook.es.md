---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_deployment_handbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 430747b85661780558424403c8f633279d00b2cb96f28654f594da00cd6d49b3
source_last_modified: "2025-11-21T13:41:16.011700+00:00"
translation_last_reviewed: "2026-01-30"
---

# Manual de despliegue y operaciones del gateway SoraFS

Este manual entrega a los equipos de infra un unico playbook para publicar y
operar el servicio de gateway SoraFS de Torii. Cubre estrategia de despliegue
blue/green, hooks de observabilidad requeridos, y los runbooks/artefactos de
cumplimiento que los operadores deben mantener.

## 1. Alcance y audiencia

- **Audiencia:** equipos de plataforma/Ops que hacen onboarding de nuevos gateways, SREs responsables de operaciones dia-dos, y reviewers de governance que validan evidencia de cumplimiento.
- **Fuera de alcance:** economia de providers SoraFS, diseno de muestreo PoR, o procedimientos de upgrade del core de Torii. Ver `docs/source/sorafs_*.md` para notas de diseno complementarias.

## 2. Prerrequisitos

| Requisito | Notas |
|-----------|-------|
| Build de Torii >= `2026-02-18` | Debe incluir enforcement de stream-token y superficie de config `torii.sorafs_gateway`. |
| Artefactos de admision GAR | Sobre de admision del gateway + manifiesto firmado via tooling de governance. |
| Key de firmado de stream token | Guardada en `sorafs_gateway_secrets/token_signing_sk`. Rotar trimestralmente (ver §5.4). |
| Stack de observabilidad | Dashboards Prometheus + Grafana (`grafana_sorafs_gateway_*`) entregados en `docs/source/`. |
| Tooling de smoke | CLI `sorafs-fetch` mas reciente (`cargo run -p sorafs_fetch -- --help`) con las opciones de gateway descritas abajo. |

## 3. Checklist de configuracion

1. Poblar `torii.sorafs_gateway` en la config del nodo:
   ```toml
   [torii.sorafs_gateway]
   enable = true
   require_manifest_envelope = true
   enforce_admission = true
   direct_mode.default_mapping = "vanity"
   stream_tokens.signing_key_path = "/etc/iroha/sorafs_gateway_secrets/token_signing_sk"
   stream_tokens.default_ttl_secs = 900
   stream_tokens.default_max_streams = 8
   ```
2. Asegurar que el controller de automatizacion TLS este activo (seccion `tls_automation`) para que el header `X-Sora-TLS-State` se pueble.
3. Configurar exporters de observabilidad (scrape Prometheus del endpoint `torii_metrics`). Los dashboards referenciados en §4 esperan nombres de metricas `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_stream_token_denials_total{reason=…}`, etc.

## 4. Procedimiento de rollout blue/green

### 4.1 Validacion pre-flight

1. Generar un stream token para el host de gateway (stage) nuevo:
   ```bash
   curl -sS -X POST https://stage-gw.example/token \
     -H "X-SoraFS-Client: stage-orchestrator" \
     -H "X-SoraFS-Nonce: $(uuidgen)" \
     --data-binary @admission_envelope.to \
     | jq .
   ```
2. Ejecutar un dry-run de fetch desde un orquestador de staging usando el nuevo flag de CLI:
   ```bash
   sorafs-fetch \
     --plan=plan.json \
     --gateway-provider=name=stage-gw,provider-id=<hex>,base-url=https://stage-gw.example/,stream-token=<base64> \
     --gateway-manifest-id=<manifest_id_hex> \
     --gateway-chunker-handle=sorafs.sf1@1.0.0 \
     --gateway-client-id=stage-orchestrator \
     --output=/tmp/stage.bin \
     --json-out=/tmp/stage_report.json
   ```
   El reporte debe mostrar cero retries y `provider_reports[].metadata.capabilities` debe incluir `chunk_range_fetch`.
3. Capturar un resumen de verificacion trustless para el CAR staged:
   ```bash
   cargo run -p sorafs_car --bin soranet_trustless_verifier --features cli --locked -- \
     --manifest fixtures/sorafs_gateway/1.0.0/manifest_v1.to \
     --car fixtures/sorafs_gateway/1.0.0/gateway.car \
     --json-out=artifacts/gateway/stage_trustless_summary.json --quiet
   ```
   Adjuntar el JSON al paquete de evidencia blue/green; registra digests de manifiesto/payload, el digest SHA3-256 del plan de chunks y la raiz PoR usando la configuracion compartida del verifier (`configs/soranet/gateway_m0/gateway_trustless_verifier.toml`).

### 4.2 Cutover blue/green

| Paso | Descripcion | Criterio de aceptacion |
|------|-------------|------------------------|
| 1 | Desplegar nuevas instancias de gateway (green) junto al pool blue existente. | healthz + `/v2/sorafs/storage/status` pasan. |
| 2 | Registrar hosts green via update de registro de admision; publicar sobre de governance. | `torii.sorafs_gateway.direct_mode.plan` dry-run muestra el mapping vanity esperado. |
| 3 | Desviar una pequena porcion de trafico (5%) usando tokens de orquestacion apuntados a `stage-gw`. | Dashboard `grafana_sorafs_gateway_load_tests.json` muestra <=1% de rechazo. |
| 4 | Expandir al 50% cuando la telemetria permanezca dentro de SLOs por 30 minutos. | La pendiente de `torii_sorafs_stream_token_denials_total` cerca de cero; `scripts/sorafs_direct_mode_smoke.sh` pasa contra endpoints de gateway staging. |
| 5 | Drenar instancias blue, revocar sus stream tokens y remover del registro de admision. | `sorafs_gateway_token_revocations_total` incrementa y los hosts antiguos dejan de servir trafico. |

Reusa `scripts/sorafs_direct_mode_smoke.sh` para la compuerta de modo directo. Provee adverts de providers via
CLI o apunta el script a `docs/examples/sorafs_direct_mode_smoke.conf` (actualiza el hash del manifiesto
y las entradas de stream-token antes de correr):

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=green-1,provider-id=<hex>,base-url=https://gw-green.example/direct/,stream-token=<base64>
```

El wrapper persiste el scoreboard declarado en
`docs/examples/sorafs_direct_mode_policy.json`, registra el resumen de fetch y
aprovecha el nuevo flag `sorafs_cli fetch --orchestrator-config=…` para que los operadores
referencien el mismo JSON de politica usado durante la planificacion.

### 4.3 Tareas post-despliegue

- Archivar evidencia del rollout: comandos CLI, snapshots Grafana, logs de tokens.
- Actualizar `status.md` con notas de completitud si governance lo requiere.

## 5. Operaciones y respuesta a incidentes

### 5.1 Deck de monitoreo

Fijar los siguientes dashboards (JSON disponible bajo `docs/source/`):
- `grafana_sorafs_gateway_load_tests.json` — latencia, tasa de rechazo, throughput.
- `grafana_sorafs_gateway_direct_mode.json` — estado de mapeo de hosts, hits de denylist.
- `grafana_sorafs_gateway_tokens.json` — tokens activos, conteos de emision/denegacion.

Alertas recomendadas:
- `torii_sorafs_chunk_range_requests_total{result="5xx"}` sobre 1% por 5 minutos.
- `torii_sorafs_stream_token_denials_total{reason="rate_limited"}` > 10/min.
- `gateway_policy_denials_total{reason="admission_unavailable"}` > 0.

### 5.2 Incidentes comunes y playbooks

| Incidente | Deteccion | Accion inmediata | Seguimiento |
|-----------|-----------|------------------|-------------|
| Agotamiento de stream token | Alerta: denegaciones `rate_limited` | Emitir nuevo token con `max_streams` mas alto via `/token`; ajustar concurrencia del orquestador. | Revisar presupuesto del cliente; actualizar `torii.sorafs_gateway.stream_tokens.default_max_streams`. |
| GAR mismatch / provider no admitido | Spikes en metricas de denegacion de politica | Verificar sync del registro de admision; ejecutar `iroha_cli app sorafs direct-mode status`. | Re-firmar manifiesto/sobre; documentar en el log de governance. |
| Falla de automatizacion TLS | `X-Sora-TLS-State` pasa a `degraded` | Disparar renovacion ACME manual (`sorafs-gateway tls renew`); fallback a cert almacenado. | Registrar incidente con timeline de certs. |
| Hit de denylist / takedown de governance | `gateway_policy_denials_total{reason="denylisted"}` | Confirmar metadata del request, informar a governance, bloquear provider/alias ofensivo. | Actualizar documentacion de denylist; retener logs para compliance. |

### 5.3 Requisitos de logs y auditoria

- Retener logs de emision de stream-token (cuerpo JSON + headers) por >=90 dias.
- Forwardear violaciones de politica al SIEM con contexto: manifest ID, alias del provider, reason code.
- Guardar snapshots Grafana para decisiones go/no-go en el ticket de release.

### 5.4 Rotacion de llaves

1. Ejecutar `sorafs-gateway key rotate --kind token-signing`.
2. Actualizar el manifiesto de admision (`gateway.token_signing_pk`) y publicar el sobre.
3. Notificar a orquestadores via el topic de telemetria `sorafs.gateway.token_pk_update`.
4. Mantener la key antigua 24h, luego purgarla del secrets store.

### 5.5 Pack de politica WAF

Los rollouts del gateway deben incluir el pack WAF/rate compartido emitido en `configs/soranet/gateway_m0/gateway_waf_policy.yaml` (version `2`). El pack fija una ventana shadow de 300 s, incorpora el harness de FP (`ci/check_sorafs_gateway_conformance.sh`) y enlaza este manual como playbook de rollback. Adjunta el pack junto al resumen trustless y la evidencia TLS en cada envio a governance.

## 6. Checklist de cumplimiento y governance

- **Evidence pack**: sobres de admision, logs de rotacion de tokens, tickets de cambio.
- **Validacion de self-cert**: ejecutar workflows de `docs/source/sorafs_gateway_self_cert.md` antes de onboarding de operadores.
- **Alineacion regulatoria**: asegurar que los providers de storage firmen el SLA de operaciones del gateway que cubre retencion de stream-token, cadencia de rotacion TLS y tiempo de respuesta de denylist.

## 7. Apendice: referencia CLI

### 7.1 Uso del gateway en `sorafs-fetch`

```
sorafs-fetch \
  --plan=chunk_fetch_plan.json \
  --gateway-provider=name=gw-a,provider-id=e1ab...,base-url=https://gw-a.example/,stream-token=<b64> \
  --gateway-provider=name=gw-b,provider-id=f2cd...,base-url=https://gw-b.example/,stream-token=<b64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --gateway-client-id=stage-orchestrator \
  --json-out=gateway_report.json
```

Los outputs incluyen metadata del provider (`metadata.capabilities`, `metadata.range_capability`) para que operadores puedan archivar evidencia de conformidad junto al registro de rollout.

Los fetches con guard reutilizan los mismos entry guards que el stack cliente SoraNet. Agrega los siguientes flags cuando el consenso de directorio esta disponible:

- `--guard-directory=consensus.json` — payload Norito JSON que contiene los descriptores de relay publicados por SNNet-3. El CLI refresca los guards pinneados antes de ejecutar el fetch.
- `--guard-cache=/var/lib/sorafs/guards.norito` — `GuardSet` codificado en Norito persistido tras una corrida exitosa; los reruns reusan el cache aun sin un directorio nuevo.
- `--guard-target 3` / `--guard-retention-days 30` — override del conteo de pins por defecto (3) y ventana de retencion (30 dias) al hacer staging de rollouts regionales.

El archivo de cache vive junto a la configuracion del orquestador para que corridas multi-source se mantengan deterministas a traves de tooling CLI/SDK.

---

**Control de cambios:** Actualiza este manual cuando cambien configs de gateway, etiquetas de telemetria o politicas de stream-token. Referencia este documento en futuras entradas del roadmap para tareas SF-5d/SF-7.
