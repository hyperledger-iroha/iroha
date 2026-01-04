---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sns/address_checksum_failure_runbook.md`. Actualiza el archivo fuente primero y luego sincroniza esta copia.
:::

Los fallos de checksum aparecen como `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) en Torii, SDKs y clientes de wallet/explorer. Los items de roadmap ADDR-6/ADDR-7 ahora requieren que los operadores sigan este runbook cuando se activen alertas de checksum o tickets de soporte.

## Cuando ejecutar el play

- **Alertas:** `AddressInvalidRatioSlo` (definida en `dashboards/alerts/address_ingest_rules.yml`) se dispara y las anotaciones listan `reason="ERR_CHECKSUM_MISMATCH"`.
- **Deriva de fixtures:** El textfile Prometheus `account_address_fixture_status` o el dashboard de Grafana reporta un checksum mismatch para cualquier copia de SDK.
- **Escalaciones de soporte:** Equipos de wallet/explorer/SDK citan errores de checksum, corrupcion de IME o escaneos del portapapeles que ya no decodifican.
- **Observacion manual:** Los logs de Torii muestran repetidamente `address_parse_error=checksum_mismatch` para endpoints de produccion.

Si el incidente es especificamente sobre colisiones Local-8/Local-12, sigue los playbooks `AddressLocal8Resurgence` o `AddressLocal12Collision`.

## Checklist de evidencia

| Evidencia | Comando / Ubicacion | Notas |
|----------|---------------------|-------|
| Snapshot de Grafana | `dashboards/grafana/address_ingest.json` | Captura el desglose de razones invalidas y endpoints afectados. |
| Payload de alerta | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Incluye etiquetas de contexto y marcas de tiempo. |
| Salud de fixtures | `artifacts/account_fixture/address_fixture.prom` + Grafana | Prueba si las copias de SDK se desviaron de `fixtures/account/address_vectors.json`. |
| Consulta PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Exporta CSV para el doc de incidente. |
| Logs | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (o agregacion de logs) | Limpia PII antes de compartir. |
| Verificacion de fixture | `cargo xtask address-vectors --verify` | Confirma que el generador canonico y el JSON comprometido coinciden. |
| Chequeo de paridad SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Ejecuta para cada SDK reportado en alertas/tickets. |
| Sanidad de portapapeles/IME | `iroha address inspect <literal>` | Detecta caracteres ocultos o reescrituras IME; cita `address_display_guidelines.md`. |

## Respuesta inmediata

1. Reconoce la alerta, enlaza snapshots de Grafana + salida PromQL en el hilo de incidente, y anota los contextos de Torii afectados.
2. Congela promociones de manifest / releases de SDK que toquen el parseo de direcciones.
3. Guarda snapshots de dashboard y los artefactos textfile de Prometheus generados en la carpeta de incidente (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. Extrae muestras de logs que muestren payloads `checksum_mismatch`.
5. Notifica a los owners de SDK (`#sdk-parity`) con payloads de ejemplo para que puedan triage.

## Aislamiento de causa raiz

### Deriva de fixture o generador

- Vuelve a ejecutar `cargo xtask address-vectors --verify`; regenera si falla.
- Ejecuta `ci/account_fixture_metrics.sh` (o `scripts/account_fixture_helper.py check` individual) para cada SDK y confirma que los fixtures incluidos coinciden con el JSON canonico.

### Regresiones de codificadores de cliente / IME

- Inspecciona literales proporcionados por usuarios via `iroha address inspect` para encontrar joins de ancho cero, conversiones kana o payloads truncados.
- Cruza flujos de wallet/explorer con `docs/source/sns/address_display_guidelines.md` (objetivos de copia dual, advertencias, helpers QR) para asegurar que siguen la UX aprobada.

### Problemas de manifest o registro

- Sigue `address_manifest_ops.md` para revalidar el ultimo manifest bundle y asegurar que no reaparecieron selectores Local-8.

### Trafico malicioso o malformado

- Desglosa IPs/app IDs ofensores via logs de Torii y `torii_http_requests_total`.
- Conserva al menos 24 horas de logs para seguimiento de Security/Governance.

## Mitigacion y recuperacion

| Escenario | Acciones |
|----------|----------|
| Deriva de fixture | Regenera `fixtures/account/address_vectors.json`, vuelve a ejecutar `cargo xtask address-vectors --verify`, actualiza bundles de SDK, y adjunta snapshots de `address_fixture.prom` al ticket. |
| Regresion de SDK/cliente | Abre issues referenciando el fixture canonico + salida de `iroha address inspect`, y bloquea releases con la CI de paridad SDK (por ejemplo `ci/check_address_normalize.sh`). |
| Corrupcion de manifest | Reconstruye el manifest siguiendo `address_manifest_ops.md`, vuelve a ejecutar `cargo xtask address-manifest verify`, y mantiene `torii.strict_addresses=true` hasta que la telemetria se limpie. |
| Envios maliciosos | Aplica rate-limit o bloquea principals ofensores, escala a Governance si se requiere tombstone de selectores. |

Una vez que aterricen las mitigaciones, vuelve a ejecutar la consulta PromQL anterior para confirmar que `ERR_CHECKSUM_MISMATCH` se mantiene en cero (excluyendo `/tests/*`) durante al menos 30 minutos antes de bajar el incidente.

## Cierre

1. Archiva snapshots de Grafana, CSV de PromQL, extractos de logs y `address_fixture.prom`.
2. Actualiza `status.md` (seccion ADDR) y la fila del roadmap si cambiaron herramientas/docs.
3. Registra notas post-incidente en `docs/source/sns/incidents/` cuando surjan nuevas lecciones.
4. Asegura que las notas de release de SDK mencionen correcciones de checksum cuando aplique.
5. Confirma que la alerta se mantiene verde por 24h y que los checks de fixture siguen verdes antes de cerrar.
