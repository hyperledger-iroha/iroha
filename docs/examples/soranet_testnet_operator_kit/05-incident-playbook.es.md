---
lang: es
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

# Playbook de respuesta a brownout / downgrade

1. **Detectar**
   - Se dispara la alerta `soranet_privacy_circuit_events_total{kind="downgrade"}` o llega el webhook de brownout desde governance.
   - Confirmar via `kubectl logs soranet-relay` o systemd journal en 5 min.

2. **Estabilizar**
   - Congelar guard rotation (`relay guard-rotation disable --ttl 30m`).
   - Habilitar override direct-only para clientes afectados
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Capturar el hash actual de la config de compliance (`sha256sum compliance.toml`).

3. **Diagnosticar**
   - Recolectar el snapshot mas reciente de directorio y el bundle de metricas del relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Registrar profundidad de cola PoW, contadores de throttling y picos de categoria GAR.
   - Identificar si el evento fue causado por deficit de PQ, override de compliance o falla del relay.

4. **Escalar**
   - Notificar al puente de governance (`#soranet-incident`) con resumen y hash del bundle.
   - Abrir ticket de incidente enlazando la alerta, incluyendo timestamps y pasos de mitigacion.

5. **Recuperar**
   - Una vez resuelta la causa raiz, reactivar rotation
     (`relay guard-rotation enable`) y revertir overrides direct-only.
   - Monitorear KPIs por 30 minutos; asegurar que no aparezcan nuevos brownouts.

6. **Postmortem**
   - Enviar reporte de incidente dentro de 48 horas usando la plantilla de governance.
   - Actualizar runbooks si se descubre un nuevo modo de falla.
