---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_design_gar_telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ede73438cc4f9013ee34e9047353195be2e4273c7df94b5497fc21d2da2dcc70
source_last_modified: "2025-11-09T13:59:51.449597+00:00"
translation_last_reviewed: "2026-01-30"
---

# Snapshot de telemetría GAR (2025-02-21 14:05 UTC)

Esta nota captura la telemetría más reciente de staging utilizada para briefear
stakeholders antes del kickoff de SoraFS Gateway & DNS. El scrape crudo está en
`docs/source/sorafs_gateway_dns_design_metrics_20250221.prom`.

## Hallazgos clave — Snapshot 2025-02-21

- **Violaciones GAR siguen bajas.** Se registraron dos desajustes de digest de
  manifiesto en `gw-stage-1` durante las últimas 24 h, ambos ligados a
  manifiestos desactualizados. Las violaciones por rate-limit se aíslan a
  agotamiento de cuota de stream-token.
- **Refusals alineadas con expectativas.** Todas las negativas provienen de
  headers requeridos ausentes (set `X-SoraFS-*`). No se observaron handles de
  chunker no soportados.
- **Paridad de versión de fixtures.** Ambos gateways de staging anuncian
  `torii_sorafs_gateway_fixture_version{version="1.0.0"} == 1`, coincidiendo
  con el bundle de conformidad actual.
- **Buffer de expiración TLS.** Los certificados expiran en ~7 días
  (604 800 s), dentro del SLA de automatización; no se requiere rotación inmediata.

## Desglose de métricas — 2025-02-21

| Métrica | Labels | Valor | Interpretación |
|--------|--------|-------|----------------|
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `2` | Envelopes antiguos golpearon el gateway; resuelto tras refresh de caché. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `1` | El cliente excedió la cuota de stream token asignada; resalta la necesidad de mejores mensajes de retry. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="missing_headers"}` | `3` | Requests sin `X-SoraFS-Manifest-Envelope` y headers de nonce; los triggers permanecen deterministas. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-2", profile="sf1", reason="missing_headers"}` | `1` | Una negativa, ya reconocida por Tooling WG como test del harness. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-1", version="1.0.0"}` | `1` | Confirma paridad de fixtures; la alerta dispararía si fuera cero. |
| `torii_sorafs_tls_cert_expiry_seconds` | _global_ | `604 800` | Rotación de certs planificada para 2025-02-28; Ops lead confirmará la corrida de automatización. |

## Acciones

1. **Higiene de caché de manifiestos:** Asegurar que la agenda DNS/gateway cubra el busting determinista de caché para minimizar desajustes de digest.
2. **UX de validación de headers:** QA Guild verificará que los mensajes de error muestren con claridad los headers ausentes en el harness de conformidad.
3. **Recordatorio de rotación TLS:** Ops Lead incluirá la rotación próxima en el deck del kickoff; la automatización está programada para 2025-02-28.

## Refresh — 2025-03-02 15:10 UTC

Según el checklist del kickoff, se volvió a scrape de telemetría 24 horas antes
la sesión. Las métricas crudas viven en
`docs/source/sorafs_gateway_dns_design_metrics_20250302.prom`.

### Hallazgos clave — Snapshot 2025-03-02

- **Desajustes de manifiesto resueltos.** `torii_sorafs_gar_violations_total`
  reporta `0` eventos `manifest_digest_mismatch` en ambos gateways de staging
  tras el flush de caché del 2025-02-25.
- **Rate limiting estable.** `gw-stage-1` sigue registrando una sola
  violación `stream_token_quota_exceeded` (el test de regresión del harness),
  mientras que `gw-stage-2` continúa en `0`.
- **Refusals por headers bajan a una.** Solo queda un `missing_headers` en
  ambos gateways, confirmando que las actualizaciones del harness aterrizaron.
- **Paridad de fixtures intacta.** Ambos gateways anuncian
  `torii_sorafs_gateway_fixture_version{version="1.0.0"} == 1`.
- **Buffer TLS saludable.** `torii_sorafs_tls_cert_expiry_seconds` reporta
  `518 400` (~6 días), manteniendo la rotación de 2025-02-28 en horario.

### Desglose de métricas — 2025-03-02

| Métrica | Labels | Valor | Interpretación |
|--------|--------|-------|----------------|
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `0` | El flush de caché eliminó digests de manifiesto obsoletos. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `1` | Regresión esperada del harness: confirma que el enforcement de cuota sigue activo. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-2", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `0` | El gateway dos permanece limpio post-flush. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-2", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `0` | No hay eventos de rate-limit en el gateway dos. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="missing_headers"}` | `1` | Una negativa capturada durante validación del harness. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="unsupported_chunker"}` | `0` | Confirma que los clientes solo anuncian perfil `sf1`. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-2", profile="sf1", reason="missing_headers"}` | `0` | Sin issues de headers en el gateway dos. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-1", version="1.0.0"}` | `1` | Paridad de fixtures intacta; alarmas se disparan en `0`. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-2", version="1.0.0"}` | `1` | Refleja el estado del gateway uno. |
| `torii_sorafs_tls_cert_expiry_seconds` | _global_ | `518 400` | Buffer de seis días antes de la rotación planificada del 2025-02-28. |

Las acciones del snapshot 2025-02-21 siguen vigentes; no se identificaron
riesgos adicionales durante este refresh.
