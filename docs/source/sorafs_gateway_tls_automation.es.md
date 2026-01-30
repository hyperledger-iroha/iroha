---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_tls_automation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b27f68f386cb284bc209a4ab7ee204a85f8df1b681fac5ac757344fe7d427025
source_last_modified: "2025-11-23T08:11:54.840116+00:00"
translation_last_reviewed: "2026-01-30"
---

# Guia de operador para TLS y ECH del gateway SoraFS

## Alcance

Esta guia documenta como operar el gateway SoraFS de Torii cuando el stack de
automatizacion de certificados SF-5b esta habilitado. Reemplaza el esquema de
planificacion anterior y ahora se enfoca en tareas diarias del operador:
provisionar secretos, configurar ACME/ECH, entender telemetria, cumplir
obligaciones de governance y ejecutar playbooks de incidentes aprobados.

La guia asume que el gateway incluye el controller de automatizacion en
`iroha_torii::sorafs::gateway` y que `cargo xtask sorafs-self-cert` esta
disponible en el bastion host.

## Checklist de quick start

1. Staggear credenciales ACME y artefactos GAR en el store de secretos sellado
   `sorafs_gateway_tls/` (KMS o Vault) y espejarlos al bastion host.
2. Completar la configuracion `torii.sorafs_gateway.acme` con preferencias de
   desafios DNS-01 y TLS-ALPN-01, habilitar ECH si esta soportado y fijar
   thresholds de renovacion.
3. Desplegar o habilitar la unidad `sorafs-gateway-acme@<environment>.service`
   para que el controller de automatizacion corra continuamente.
4. Ejecutar `cargo xtask sorafs-self-cert --check-tls` para capturar un bundle
   de atestacion baseline y verificar headers/metricas.
5. Publicar dashboards y alertas de telemetria para las metricas en la seccion
   **Referencia de telemetria** y suscribir a la rotacion on-call.
6. Subir los playbooks de incidentes a tu tooling de runbooks (PagerDuty, Notion)
   y programar los drills listados en **Cadencia de drills y evidencia**.

## Recorrido de configuracion

### Prerrequisitos

| Requisito | Por que importa |
|-----------|-----------------|
| Credenciales de cuenta ACME almacenadas en secretos sellados `sorafs_gateway_tls/` (KMS/Vault) | Requeridas para ordenes manuales, revocacion y renovacion de tokens de automatizacion. |
| Copia offline del ultimo bundle de config `torii.sorafs_gateway` | Permite a operadores parchear `ech_enabled`, listas de hosts o timing de retry sin esperar pipelines de config. |
| Acceso a manifiestos de governance (`manifest_signatures.json`, sobres GAR) | Necesario al publicar fingerprints de certificados o rotar mapeos de host canonicos. |
| Toolchain `cargo xtask` en el bastion host | Provee checks `sorafs-gateway-attest`/`sorafs-self-cert` usados en verificacion. |
| Plantilla de playbook almacenada en tooling de incidentes (PagerDuty/Notion) | Asegura que los checklists de esta guia estan a un click durante incidentes. |

### Secretos y layout de archivos

- Almacenar material de cuenta ACME bajo `/etc/iroha/sorafs_gateway_tls/` con
  ownership `iroha:iroha` y permisos `0600`.
- Mantener un backup sellado en tu namespace de KMS o Vault para que la rotacion
  manual recupere rapido.
- El controller de automatizacion escribe estado en
  `/var/lib/sorafs_gateway_tls/automation.json`; incluir esta ruta en backups
  para que jitter de renovacion y ventanas de retry persistan tras reinicios.
- Generar manifiestos SAN con `cargo xtask soradns-acme-plan`; el helper
  deriva los valores SAN canonicos wildcard + pretty-host, desafios ACME
  recomendados y labels DNS-01 por alias. Guardar el output bajo
  `fixtures/sorafs_gateway/acme_san/<alias>.san.json` para que reviewers GAR y
  el controller de automatizacion TLS compartan el mismo bundle de evidencia:

  ```bash
  cargo xtask soradns-acme-plan \
    --name docs.sora \
    --json-out fixtures/sorafs_gateway/acme_san/docs.sora.san.json
  ```

  Reusar las entradas `san` al templar ordenes ACME manuales y adjuntar el JSON
  a tickets de cambio DG-3 para que los pares canonicos + wildcard no se
  recalculen.

### Configuracion Torii

Agregar o actualizar la siguiente seccion en el bundle de configuracion Torii
(p. ej., `configs/production.toml`):

```toml
[torii.sorafs_gateway.acme]
account_email = "tls-ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
dns_provider = "route53"
renewal_threshold_seconds = 2_592_000  # 30 days
retry_backoff_seconds = 900
retry_jitter_seconds = 120
use_tls_alpn_challenge = true
ech_enabled = true
```

- `dns_provider` mapea a la implementacion registrada en `acme::DnsProvider`; Route53 viene in-tree, otros providers pueden agregarse sin recompilar el gateway.
- `ech_enabled = true` hace que el flujo de automatizacion genere configs ECH junto a los certificados; setear `false` si el CDN upstream o clientes no soportan ECH (ver el playbook de fallback abajo).
- `renewal_threshold_seconds` controla cuando el loop de automatizacion dispara una orden nueva; el default sigue en 30 dias pero puede bajar para despliegues de alto riesgo.
- El parser de configuracion vive en `iroha_torii::sorafs::gateway::config`, y los errores de validacion se ven en logs de Torii durante el arranque.

### Referencia de configuracion

| Clave | Default | Expectativa en produccion | Vinculo de compliance |
|------|---------|----------------------------|-----------------------|
| `torii.sorafs_gateway.acme.enabled` | `true` | Mantener habilitado salvo durante drills de incidentes. | Deshabilitar automatizacion debe registrarse en el change log con ID de incidente/ticket. |
| `torii.sorafs_gateway.acme.directory_url` | Let’s Encrypt v2 | Override solo al cambiar de entorno CA. | Governance requiere publicar la CA seleccionada en manifiestos GAR. |
| `torii.sorafs_gateway.acme.dns_provider` | `route53` | Proveer un nombre concreto registrado en el crate de automatizacion. | La politica IAM del provider debe revisarse trimestralmente. |
| `torii.sorafs_gateway.acme.renewal_threshold_seconds` | `2_592_000` (30 dias) | Ajustar segun postura de riesgo (>=14 dias). | Ajustes requieren aprobacion del risk owner. |
| `torii.sorafs_gateway.acme.retry_backoff_seconds` | `900` | Mantener <= 900 para cumplir SLAs de recuperacion GAR. | Backoff >900 requiere waiver en el log de governance. |
| `torii.sorafs_gateway.acme.ech_enabled` | `true` | Togglear solo cuando corre el playbook de fallback. | Cada toggle debe documentarse con razon/evidencia en el tracker de compliance. |
| `torii.sorafs_gateway.acme.telemetry_topic` | `sorafs-tls` | Override opcional para el topic de agregacion de logs. | Si se cambia, registrar el nuevo topic con observabilidad para retencion. |

Al ajustar configuracion a mano, stagear el cambio en
`iroha_config::actual::sorafs_gateway` (o tu sistema de config), capturar el
 diff en el ticket de change-control y adjuntar un bundle self-cert fresco una
vez despleguen los nuevos settings.

### Servicio de automatizacion

- Habilitar la unidad systemd template que viene con el bundle de automatizacion:

  ```bash
  systemctl enable --now sorafs-gateway-acme@production.service
  ```

- Los logs van al target `sorafs_gateway_tls_automation` del journal. Enviarlos
  a tu agregador de logs y etiquetarlos con el environment para triage rapido.
- Estado de implementacion: `iroha_torii::sorafs::gateway::acme::AcmeAutomation`
  provee handling determinista de desafios DNS-01/TLS-ALPN-01, jitter/backoff
  configurable y persistencia del estado de renovacion.

### Verificacion y atestacion

1. Luego de aplicar la configuracion, recargar Torii:

   ```bash
   systemctl reload iroha-torii.service
   ```

2. Ejecutar el harness de self-cert para confirmar headers, metricas y
   integracion de politica GAR:

   ```bash
   cargo xtask sorafs-self-cert --check-tls \
     --gateway https://gateway.example.com \
     --out artifacts/sorafs_gateway_self_cert
   ```

3. Archivar el reporte generado junto al sobre GAR para trazabilidad.
4. Estado de implementacion: `iroha_torii::sorafs::gateway::telemetry` exporta
   `SORA_TLS_STATE_HEADER`, `Metrics::set_sorafs_tls_state` y
   `Metrics::record_sorafs_tls_renewal` consumidos por el harness.

### Probe de headers y GAR

Ejecutar el probe determinista de headers antes de cada rollout (y cuando la
monitoria sintetica dispare) para asegurar que los gateways staplean los
headers Sora requeridos, coinciden con metadata GAR y anuncian el estado de
cache/TLS esperado:

```bash
cargo xtask sorafs-gateway-probe \
  --gateway https://gw.example.com/car/bafy... \
  --gar artifacts/gar/self-cert.gar.jws \
  --gar-key council-key-1=8b9c...c5 \
  --header "Accept: application/vnd.ipld.car; dag-scope=full" \
  --timeout-secs 15
```

- `--gar` apunta al GAR JWS compacto. Proveer las llaves publicas Ed25519
  correspondientes via flags repetidos `--gar-key kid=hex` para que el probe
  verifique el JWS.
- `--gateway` obtiene headers en vivo; alternativamente usa `--headers-file`
  (con `--host`) para inspeccionar dumps capturados con `curl -i`.
- El probe valida consistencia `Sora-Name`/`Sora-Proof`, cobertura GAR de
  host/manifiesto, `Cache-Control: max-age=600, stale-while-revalidate=120`,
  `Content-Security-Policy`, `Strict-Transport-Security` y `X-Sora-TLS-State`.
- `--report-json <path|->` escribe el resumen legible por maquina consumido por
  los helpers de automatizacion. Cuando se apunta a stdout, el probe imprime
  resultados humanos a stderr para mantener parseable el stream JSON.
- Sale con codigo no-cero en mismatches para que hooks de CI/paging fallen
  rapido. Aun no establece el header de estado TLS.

#### Automatizacion de paging y drills de rollback

El probe ahora expone hooks nativos para logging de drills, resuenes JSON y
payloads PagerDuty, asi que puedes ejecutarlo directo desde CI o bastion hosts:

- `--drill-scenario <name>` con `--drill-log`, `--drill-ic`, `--drill-scribe`,
  `--drill-notes` y `--drill-link` agrega una fila a `ops/drill-log.md` usando
  las mismas reglas de escaping que `scripts/telemetry/log_sorafs_drill.sh`.
- `--summary-json <path>` emite un registro estructurado (hallazgos, metadata
  GAR, timestamps) que puede adjuntarse al bundle de atestacion o evidencia de
  drill.
- `--pagerduty-routing-key <key>` habilita integracion PagerDuty. Combinalo con
  `--pagerduty-payload <path>` (por defecto
  `artifacts/sorafs_gateway_probe/pagerduty_event.json`) y, cuando estes listo,
  `--pagerduty-url https://events.pagerduty.com/v2/enqueue` para publicar el
  evento. Flags adicionales (`--pagerduty-component`, `--pagerduty-group`,
  `--pagerduty-link text=url`, `--pagerduty-dedup-key`, etc.) mapean directo a
  la payload de la Events API.

Ejemplo de invocacion de drill:

```bash
cargo xtask sorafs-gateway-probe \
  --gateway https://gw.example.com/car/bafy... \
  --gar artifacts/gar/self-cert.gar.jws \
  --gar-key council-key-1=8b9c...c5 \
  --drill-scenario tls-renewal \
  --drill-ic "Automation Harness" \
  --drill-scribe "Ops Bot" \
  --drill-notes "Quarterly TLS rotation drill" \
  --summary-json artifacts/sorafs_gateway_probe/tls_probe.json \
  --pagerduty-routing-key "$PAGERDUTY_ROUTING_KEY" \
  --pagerduty-link "Rollback plan=https://git.example.com/sorafs/tls-rotation" \
  --pagerduty-url https://events.pagerduty.com/v2/enqueue
```

Probes fallidos disparan PagerDuty inmediatamente (omite la URL durante
entrenamiento si solo necesitas el payload) y aun salen no-cero para que CI
pueda detenerse. El script helper bajo
`scripts/telemetry/run_sorafs_gateway_probe.sh` sigue disponible para bundles
preconfigurados, y CI ejercita el workflow via
`ci/check_sorafs_gateway_probe.sh` usando los fixtures demo en
`fixtures/sorafs_gateway/probe_demo/`; reusa ese script (o copia su command
line) al cablear drills de paging periodicos. Los flags nativos significan que
la mayoria de equipos pueden llamar el probe directo sin agregar glue
personalizado de logging o PagerDuty.

### Plan de promocion de rutas y rollback

Ejecutar el nuevo route planner una vez que el manifiesto de release esta listo
para que el ticket de cutover lleve un bloque determinista de headers `Sora-*`
y metadata explicita de rollback. El helper ahora viene dentro del CLI
(`iroha app sorafs gateway route-plan`) y como wrapper cuando necesitas
automatizar desde CI:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json artifacts/sorafs_cli/portal.manifest.json \
  --hostname docs.sora.link \
  --alias sora:docs \
  --route-label docs@2026-03-21 \
  --release-tag v2026.03.21 \
  --cutover-window 2026-03-21T15:00Z/2026-03-21T15:30Z \
  --rollback-manifest-json artifacts/sorafs_cli/portal.manifest.previous.json \
  --rollback-route-label docs@previous
```

El comando produce un descriptor JSON
(`artifacts/sorafs_gateway/route_plan.json` por defecto) mas plantillas de
headers (`gateway.route.headers.txt` y, cuando se suministra
`--rollback-manifest-json`, `gateway.route.rollback.headers.txt`). Cada plan
incrusta:

- el `Sora-Content-CID` resuelto,
- los headers `Sora-Name`/`Sora-Proof` renderizados y plantillas CSP/HSTS,
- el string canonico `Sora-Route-Binding` (`host=…;cid=…;generated_at=…;label=…`),
- metadata opcional de rollback que ata el bloque previo de manifiesto/headers
  a una etiqueta legible.

Adjuntar el plan JSON y las plantillas de headers al ticket de release junto al
descriptor de cutover DNS para que reviewers comparen el binding nuevo con el
rollback registrado. El descriptor espeja el bloque `gateway_binding` usado por
`docs/portal/scripts/sorafs-pin-release.sh`, asegurando que los pipelines de DNS
y gateway promuevan headers identicos.

## Referencia de telemetria

| Superficie | Nombre | Descripcion | Alerta / Accion |
|------------|--------|-------------|-----------------|
| Metricas | `sorafs_gateway_tls_cert_expiry_seconds` | Segundos hasta la expiracion del certificado activo. | Pager on-call cuando `< 1_209_600` (14 dias). |
| Metricas | `sorafs_gateway_tls_renewal_total{result}` | Contador de intentos de renovacion etiquetado por `success`/`error`. | Investigar si la tasa de error excede 5% en una hora. |
| Metricas | `sorafs_gateway_tls_ech_enabled` | Gauge (`0`/`1`) que refleja el estado ECH actual. | Alertar cuando baja a `0` inesperadamente. |
| Metricas | `torii_sorafs_gar_violations_total{reason,detail}` | Contador de violaciones de politica expuesto por GAR enforcement. | Escalar a governance inmediatamente; adjuntar logs de violacion. |
| Header | `X-Sora-TLS-State` | Embebido en respuestas del gateway (p. ej., `ech-enabled;expiry=2025-06-12T12:00:00Z`). | Monitoreo sintetico; ante `ech-disabled` o `degraded`, seguir los playbooks abajo. |
| Logs | `journalctl -u sorafs-gateway-acme@*.service` | Trazas de renovacion, errores de challenge y overrides manuales. | Capturar logs con tickets de incidente y durante drills. |

Exponer las metricas via Prometheus/OpenTelemetry, cablear dashboards para
expiracion y tendencias de renovacion, y crear probes sinteticos que verifiquen
el header `X-Sora-TLS-State` cada hora.

### Cableado de alertas

- **Runway de expiracion:** Alertar cuando
  `sorafs_gateway_tls_cert_expiry_seconds` baja de 14 dias (warning) y 7 dias
  (critical). Pagear al on-call del gateway y enlazar al playbook de
  **Emergency Certificate Rotation**.
- **Fallas de renovacion:** Disparar incidente cuando
  `sorafs_gateway_tls_renewal_total{result="error"}` incremente mas de tres
  veces en una ventana de seis horas. Adjuntar logs de automatizacion al ticket.
- **Violaciones GAR:** Enrutar alarmas `torii_sorafs_gar_violations_total`
  directo al canal del council de governance para permitir waivers o desviar trafico.
- **Drift de estado ECH:** Alertar la rotacion de developer experience cuando
  `sorafs_gateway_tls_ech_enabled` cambia de `1` a `0` fuera de un play
  programado; SDKs downstream deben ser notificados para ajustar expectativas.

## Hooks de politica GAR

- Las denegaciones de politica del gateway incrementan
  `torii_sorafs_gar_violations_total{reason,detail}` para que Prometheus/Alertmanager
  disparen playbooks de governance automaticamente.
- Torii ahora emite mensajes `DataEvent::Sorafs(GarViolation)` con identificadores
  de providers, metadata de denylist y contexto de rate-limit. Suscribirse via
  los adapters SSE/webhook existentes para alimentar pipelines de compliance.
- Los logs de requests agregan campos estructurados `policy_reason`,
  `policy_detail` y `provider_id_hex`, simplificando el triage forense y la
  recoleccion de evidencia de auditoria.

## Compliance y governance

Los operadores deben cumplir con las siguientes obligaciones para mantenerse en
buen estado con Nexus governance:

- **Alineacion GAR:** publicar fingerprints de certificados actualizados en
  manifiestos GAR cada vez que una renovacion completa. Enviar evidencia (logs
  de automatizacion, bundle self-cert, fingerprint de atestacion) al council de
  governance.
- **Policy logging:** retener logs de violaciones GAR por al menos 180 dias e
  incluirlos en reportes trimestrales de compliance.
- **Retencion de atestaciones:** archivar cada output de
  `cargo xtask sorafs-self-cert` bajo `artifacts/sorafs_gateway_tls/<YYYYMMDD>/`
  y dar acceso read-only a auditores.
- **Gestion de cambios de config:** registrar cambios `torii.sorafs_gateway` en
  el sistema de change-control, incluyendo la razon de togglear `ech_enabled` o
  ajustar thresholds de renovacion.
- **Ejecucion de drills:** correr los drills definidos en esta guia y documentar
  resultados dentro de tres dias habiles.

### Checklist de evidencia de compliance

| Obligacion | Evidencia a recolectar | Retencion | Owner |
|------------|-------------------------|----------|-------|
| Alineacion GAR | Manifiesto GAR actualizado, bundle firmado de fingerprint de certificado, link de ticket de incidente/cambio. | 3 anos | Governance liaison |
| Policy logging | Exportes `torii_sorafs_gar_violations_total`, extractos de logs estructurados, notificaciones Alertmanager. | 180 dias | Observabilidad |
| Retencion de atestaciones | Reporte JSON `cargo xtask sorafs-self-cert`, snapshot de header TLS, output de fingerprint OpenSSL. | 3 anos | Operaciones de gateway |
| Gestion de cambios | Diff de config (`torii.sorafs_gateway`), registro de aprobacion, timestamp de despliegue. | 2 anos | Change manager |
| Documentacion de drills | Entrada en tracker de drills, lista de participantes, issues de seguimiento. | 2 anos | Chaos coordinator |
| Eventos de toggle ECH | Log de cambio de config, boletin firmado a lista SDK/ops, snapshot de telemetria antes/despues. | 2 anos | Developer experience |

## Playbooks operativos

### Rotacion de certificado de emergencia

**Criterios de disparo**
- `sorafs_gateway_tls_cert_expiry_seconds` < 1,209,600 (14 dias).
- `sorafs_gateway_tls_renewal_total{result="failure"}` dispara dos veces en la ventana de renovacion.
- `X-Sora-TLS-State` anuncia `last-error=` o clientes reportan mismatch de certificados/fallas de handshake.

**Estabilizar**
1. Pagear al on-call de TLS del gateway SoraFS y abrir un incidente en `#sorafs-incident`.
2. Pausar rollouts de configuracion y registrar el commit de config actual en el ticket de incidente.
3. Deshabilitar automatizacion seteando `torii.sorafs_gateway.acme.enabled = false`, commitear el cambio y reiniciar el despliegue del gateway.

**Emitir y desplegar bundle de reemplazo**
1. Capturar el estado actual para auditoria:
   ```bash
   curl -sD - https://gateway.example/status \
     | grep -i '^x-sora-tls-state'
   openssl s_client -connect gateway.example:443 -servername gateway.example \
     < /dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256
   ```
2. Generar un bundle de reemplazo con el wrapper del repo (escribe PEMs al directorio pending):
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   El comando emite `fullchain.pem`, `privkey.pem` y `ech.json` bajo el directorio pending.
   Si la unidad systemd de automatizacion gestiona renovaciones en tu entorno, reiniciala
   despues de correr el CLI para reanudar polling en background:
   ```bash
   sudo systemctl restart sorafs-gateway-acme@production.service
   journalctl -fu sorafs-gateway-acme@production.service
   ```
   Los clientes ACME de produccion siguen disponibles para cuentas validadas, pero el CLI
   de arriba provee el bundle self-signed determinista requerido para drills de staging.
3. Stagear el bundle en secretos y recargar Torii:
   ```bash
   install -m 600 /var/lib/sorafs_gateway_tls/pending/fullchain.pem /etc/iroha/sorafs_gateway_tls/fullchain.pem
   install -m 600 /var/lib/sorafs_gateway_tls/pending/privkey.pem  /etc/iroha/sorafs_gateway_tls/privkey.pem
   install -m 640 /var/lib/sorafs_gateway_tls/pending/ech.json     /etc/iroha/sorafs_gateway_tls/ech.json
   systemctl reload iroha-torii.service
   ```

**Validar y restaurar automatizacion**
1. Ejecutar el harness de self-cert:
   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --gateway https://gateway.example \
     --cert /etc/iroha/sorafs_gateway_tls/fullchain.pem \
     --ech-config /etc/iroha/sorafs_gateway_tls/ech.json \
     --out artifacts/sorafs_gateway_self_cert
   ```
2. Confirmar recuperacion de telemetria:
   - `sorafs_gateway_tls_cert_expiry_seconds` > 2,592,000 (30 dias).
   - `sorafs_gateway_tls_renewal_total{result="success"}` incrementa una vez.
   - `X-Sora-TLS-State` reporta `ech-enabled;expiry=…;renewed-at=…` sin `last-error`.
3. Actualizar artefactos de governance con el nuevo fingerprint (manifiesto GAR + bundle de atestacion) y adjuntarlos al ticket del incidente.
4. Re-habilitar automatizacion seteando `torii.sorafs_gateway.acme.enabled = true` y recargando Torii. Reanudar pipelines pausados y entregar el post-mortem dentro de tres dias habiles.

### Fallback ECH / modo degradado

Usar este play cuando CDNs o clientes fallan al consumir ECH.

1. Detectar via `sorafs_gateway_tls_ech_enabled == 0`, incidentes de clientes citando `GREASE_ECH_MISMATCH` o directivas de governance.
2. Deshabilitar ECH:
   ```toml
   [torii.sorafs_gateway.acme]
   ech_enabled = false
   ```
   Aplicar la config (o usar `iroha_cli config apply`), luego reiniciar Torii. El header `X-Sora-TLS-State` ahora debe anunciar `ech-disabled`.
3. Difundir el downgrade a operadores de storage, equipos de SDK y governance, incluyendo la ventana de revision esperada.
4. Monitorear `sorafs_gateway_tls_renewal_total{result="failure"}` por churn TLS adicional mientras ECH permanece deshabilitado.
5. Una vez que los servicios upstream se recuperen, reiniciar el servicio de automatizacion (o invocar tu cliente ACME manualmente) para producir un bundle fresco con configs ECH, re-ejecutar el harness self-cert, togglear `ech_enabled = true` y compartir snippets de telemetria actualizados.

### Revocacion por llave comprometida

Aplicar este playbook cuando llaves privadas se exponen o la CA reporta mis-issuance.

1. Mantener automatizacion deshabilitada y rotar llaves de firmado de stream-token segun el manual de operaciones para prevenir reuso de credenciales.
2. Revocar el bundle actual con el wrapper del repo (archiva artefactos bajo `revoked/`):
   ```bash
   scripts/sorafs-gateway tls revoke \
     --out /var/lib/sorafs_gateway_tls \
     --reason keyCompromise
   ```
   El comando mueve `fullchain.pem`, `privkey.pem` y `ech.json` a un backup con timestamp y registra un JSON de auditoria junto a ellos.
3. Emitir un bundle fresco via `scripts/sorafs-gateway tls renew --out /var/lib/sorafs_gateway_tls/pending --force` (o tu workflow ACME de produccion), luego seguir los pasos de validacion del playbook de rotacion.
4. Publicar sobres GAR actualizados y `manifest_signatures.json` para que nodos downstream adopten el nuevo fingerprint.
5. Notificar al council de governance y a equipos SDK con ID de incidente, timestamp de revocacion e instrucciones de remediacion.

## Cadencia de drills y evidencia

| Drill | Frecuencia | Escenario | Criterio de exito |
|-------|------------|----------|-------------------|
| `tls-renewal` | Trimestral | Ejecutar el playbook de rotacion completo en staging (automatizacion off/on). | La renovacion completa en < 15 min, telemetria actualizada, artefactos archivados. |
| `ech-fallback` | Dos veces al ano | Deshabilitar y restaurar ECH por una hora. | Clientes reciben boletines, `X-Sora-TLS-State` refleja ambos estados, cero alertas persistentes. |
| `tls-revocation` | Anual | Simular key compromise y revocar cert de staging. | Revocacion confirmada, bundle de reemplazo desplegado, update GAR publicado. |

Despues de cada drill:
- Archivar logs de automatizacion, snapshots Prometheus y output de self-cert en `artifacts/sorafs_gateway_tls/<YYYYMMDD>/`.
- Actualizar el tracker de chaos drills (`docs/source/sorafs_chaos_plan.md`) con participantes, duracion, observaciones y tareas de seguimiento.
- Registrar regresiones de automatizacion como follow-ups de roadmap (SF-5, SF-7) con evidencia enlazada.

## Troubleshooting

- **Log de automatizacion muestra fallas DNS-01 repetidas:** verificar permisos IAM para el `dns_provider` configurado y confirmar propagacion TXT con `dig _acme-challenge.gateway.example.com txt`.
- **`X-Sora-TLS-State` ausente o malformado:** asegurar que el servicio Torii fue recargado tras copiar certificados y que `Metrics::set_sorafs_tls_state` sea exitoso (revisar logs Torii para fallas `warn`).
- **Contadores de violacion GAR incrementan:** inspeccionar logs estructurados emitidos en `torii_sorafs_gar_violations_total`, remediar el provider o manifiesto ofensivo y notificar governance antes de re-habilitar trafico.
- **Clientes ECH siguen fallando tras el toggle:** confirmar que caches se invalidaron (Cloudflare/CloudFront) y compartir hostnames de fallback con integradores.

## Notas de implementacion

- **Libreria cliente ACME:** el despliegue usa
  [`letsencrypt-rs`](https://crates.io/crates/letsencrypt-rs) para desafios DNS-01
  y TLS-ALPN-01. Se integra con el executor async ya presente en el gateway y
  se distribuye bajo Apache-2.0.
- **Abstraccion de provider DNS:** soporte Route53 esta disponible por defecto.
  La automatizacion expone un trait `DnsProvider` para que equipos implementen
  providers Cloudflare o Google Cloud DNS sin tocar el controller. Configurar
  el provider via `acme.dns_provider = "<provider>"`.
- **Alineacion de nombres de telemetria:** Observabilidad aprobo los nombres de
  metricas en la tabla de telemetria; las duraciones se exponen en segundos.
  Revisar dashboards tras upgrades para asegurar que cambios de esquema se
  reflejen.
- **Integracion self-cert:** el flujo de automatizacion TLS invoca el kit
  self-cert tras cada renovacion. `sorafs_gateway_tls_automation.yml` llama a
  `cargo xtask sorafs-self-cert --check-tls` antes de notificar a operadores,
  haciendo que la validacion TLS/ECH sea parte del rollout automatizado.
