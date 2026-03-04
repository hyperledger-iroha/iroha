---
lang: es
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

## Reporte de verificacion del operador (fase T0)

- Nombre del operador: ______________________
- ID del descriptor del relay: ______________________
- Fecha de envio (UTC): ___________________
- Email / matrix de contacto: ___________________

### Resumen de checklist

| Item | Completado (S/N) | Notas |
|------|-----------------|-------|
| Hardware y red validados | | |
| Bloque de compliance aplicado | | |
| Envelope de admision verificado | | |
| Smoke test de guard rotation | | |
| Telemetria scrappeada y dashboards activos | | |
| Brownout drill ejecutado | | |
| Exito de tickets PoW dentro del objetivo | | |

### Snapshot de metricas

- Ratio PQ (`sorafs_orchestrator_pq_ratio`): ________
- Conteo de downgrade ultimas 24h: ________
- RTT promedio de circuitos (p95): ________ ms
- Tiempo mediano de resolucion PoW: ________ ms

### Adjuntos

Por favor adjunta:

1. Hash del support bundle del relay (`sha256`): __________________________
2. Capturas de dashboards (ratio PQ, exito de circuitos, histograma PoW).
3. Bundle de drill firmado (`drills-signed.json` + clave publica del firmante en hex y adjuntos).
4. Reporte de metricas SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Firma del operador

Certifico que la informacion anterior es correcta y que todos los pasos requeridos se completaron.

Firma: _________________________  Fecha: ___________________
