---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_operator_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0b0d1b2ada1254b20926b840d8557df46f1056ef06fa28dd329d7f481e4c5a56
source_last_modified: "2025-11-02T14:42:30.682087+00:00"
translation_last_reviewed: "2026-01-30"
---

# Playbook operativo de chunk-range para el gateway SoraFS

## 1. Prerrequisitos

- Gateway actualizado al perfil trustless (`docs/source/sorafs_gateway_profile.md`).
- Enforcement de stream tokens habilitado (ver `sorafs_gateway_chunk_range.md`).
- Automatización TLS/ECH configurada (`sorafs_gateway_tls_automation.md`).

## 2. Checklist de configuración

1. Habilitar endpoints de chunk-range:
   ```toml
   [sorafs.gateway.chunk_range]
   enabled = true
   max_parallel_streams = 512
   token_ttl_secs = 900
   rate_limit_bytes_per_sec = "100 MiB"
   ```
2. Apuntar el gateway al registro de admisión (`sorafs_manifest::provider_admission`).
3. Configurar exporter de telemetría (Prometheus/OpenTelemetry).
4. Configurar agregación de logs para eventos de emisión/revocación de tokens.

## 3. Procedimientos operativos

### Emisión de tokens

- Usar `POST /token` con sobre de manifiesto firmado.
- Almacenar la respuesta (token, expiración) en el dashboard del operador.
- Rotar tokens proactivamente antes del TTL cuando se ejecuten workloads 24/7.

### Monitoreo

- Dashboards:
  - `sorafs_gateway_chunk_range_requests_total`
  - `sorafs_gateway_stream_tokens_active`
  - `sorafs_gateway_stream_token_denials_total`
  - Histogramas de latencia por handle de chunker.
- Alertas:
  - Denegaciones de token > umbral.
  - Latencia de rangos > SLO.
  - Fallos de verificación de pruebas (respuestas 422).

### Respuesta a incidentes

- Agotamiento de token: incrementar límite de tasa o emitir nuevo token; notificar a operadores del orquestador.
- Fallos de prueba: poner en cuarentena al proveedor, regenerar pruebas, re-ejecutar el harness de conformidad.
- Desajuste de admisión: sincronizar sobres de admisión desde gobernanza; actualizar el caché de Torii.

## 4. Troubleshooting

| Síntoma | Posible causa | Acción |
|---------|----------------|--------|
| 428 `required_headers_missing` | Downgrade del cliente / falta `dag-scope` | Validar versión del cliente, actualizar orquestador. |
| 429 `stream_token_exhausted` | Token fuera de cuota | Emitir nuevo token, ajustar `rate_limit_bytes_per_sec`. |
| 412 `admission_required` | Sobre faltante/expirado | Actualizar registro de admisión, verificar firmas del manifiesto. |
| 422 fallo de prueba | Chunk corrupto o mismatch de fixture | Re-ejecutar suite de conformidad, comparar raíces PoR. |

## 5. Mantenimiento

- Ejecutar el kit de auto-certificación SF-5a antes y después de upgrades mayores.
- Actualizar fixtures cuando gobernanza publique nuevos datasets.
- Revisar dashboards de observabilidad semanalmente y asegurar que el ruteo de alertas funcione.

## 6. Automatización y playbooks de incidentes

### 6.1 Script de rotación de stream tokens

Para mantener los stream tokens frescos sin intervención manual, los operadores DEBERÍAN usar
el helper `scripts/sorafs_gateway_rotate_tokens.sh` (distribuido junto con los paquetes del gateway):

```bash
./scripts/sorafs_gateway_rotate_tokens.sh \
  --provider-id "$PROVIDER_ID" \
  --manifest-catalog manifests/pin_registry.csv \
  --token-endpoint https://gateway.example.com/token \
  --ttls 3600 \
  --parallel 8 \
  --log-dir /var/log/sorafs/token_rotation
```

Comportamientos clave:

- Lee el catálogo de manifiestos emitido por el Pin Registry y agrupa solicitudes `/token`
  por proveedor/handle de manifiesto.
- Firma solicitudes con la clave de operador configurada (`SORA_FS_OPERATOR_SK` env var) y
  registra los IDs de token emitidos en el log de rotación para auditoría.
- Reintenta automáticamente con backoff exponencial cuando el gateway devuelve `429` o
  `503`, y anota fallos vía `telemetry_sorafs_token_rotation_errors_total`.

Patrón de automatización recomendado:

1. Programar el script en cada host proveedor vía cron/systemd (`*/30 * * * *`).
2. Exportar logs al pipeline central; alertas deben dispararse cuando los fallos superen
   5% en una hora.
3. Ejecutar el script manualmente después de cada actualización de admisión para asegurar
   que nuevos manifiestos reciban tokens frescos antes de abrir el gateway a orquestadores.

### 6.2 Integración con playbooks de incidentes

La rotación de tokens se integra con los playbooks existentes bajo
`docs/source/sorafs_gateway_tls_automation.md` (TLS/ECH) y `docs/source/sorafs_gateway_capability_tests.md`
 (rechazos GAR). Los operadores deben extender esos playbooks con la siguiente guía:

- **Outages TLS/ECH** – Si el flujo de automatización TLS revierte certificados, reejecutar el
  script de rotación inmediatamente después del rollout para evitar firmas de token expiradas
  por cadenas de certificado desalineadas.
- **Incidentes de Gateway Admission Rate (GAR)** – Cuando GAR dispara throttle/deny,
  el coordinador del incidente debe notificar al on-call de rotación para suspender el
  script (setear `SORA_FS_ROTATION_SUSPEND=1`) y evitar inundar el cluster, reanudando
  una vez que GAR se despeje.
- **Fallback a modo seguro de chunk-range** – Cuando el plan de failover del orquestador
  dependa de un set reducido de proveedores, actualizar `--manifest-catalog` a una lista
  filtrada para que el script emita tokens solo para proveedores activos.

Documenta los ajustes anteriores en el runbook local y asegura que los incidentes en PagerDuty
para TLS/ECH o GAR incluyan un ítem de checklist para confirmar que la automatización de tokens
se ha pausado/reanudado de forma apropiada.
