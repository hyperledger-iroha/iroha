---
lang: es
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

# Paquete del Parlamento de incentivos de relay SoraNet

Este bundle captura los artefactos requeridos por el Parlamento de Sora para aprobar pagos automaticos de relay (SNNet-7):

- `reward_config.json` - configuracion del motor de recompensas serializable con Norito, lista para ser ingerida por `iroha app sorafs incentives service init`. El `budget_approval_id` coincide con el hash listado en las minutas de governance.
- `shadow_daemon.json` - mapeo de beneficiarios y bonds consumido por el harness de replay (`shadow-run`) y el daemon de produccion.
- `economic_analysis.md` - resumen de fairness para la simulacion shadow 2025-10 -> 2025-11.
- `rollback_plan.md` - playbook operativo para deshabilitar pagos automaticos.
- Artefactos de soporte: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Chequeos de integridad

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

Compara los digests con los valores registrados en las minutas del Parlamento. Verifica la firma shadow-run como se describe en
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Actualizacion del paquete

1. Actualiza `reward_config.json` cuando cambien los pesos de recompensa, el pago base o el hash de aprobacion.
2. Re-ejecuta la simulacion shadow de 60 dias, actualiza `economic_analysis.md` con los nuevos hallazgos y commitea el JSON y la firma separada.
3. Presenta el bundle actualizado al Parlamento junto con exports de dashboards de Observatory al solicitar renovacion.
