---
lang: es
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de rollback de incentivos de relay

Usa este playbook para deshabilitar pagos automaticos de relay si governance solicita un
halt o si se disparan los guardrails de telemetria.

1. **Congelar automatizacion.** Deten el daemon de incentivos en cada host de orchestrator
   (`systemctl stop soranet-incentives.service` o el deployment de contenedor equivalente) y confirma que el proceso ya no corre.
2. **Drenar instrucciones pendientes.** Ejecuta
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   para asegurar que no haya instrucciones de payout pendientes. Archiva los payloads Norito resultantes para auditoria.
3. **Revocar aprobacion de governance.** Edita `reward_config.json`, fija
   `"budget_approval_id": null` y redeploya la configuracion con
   `iroha app sorafs incentives service init` (o `update-config` si el daemon es de larga duracion). El motor de payout ahora falla cerrado con
   `MissingBudgetApprovalId`, por lo que el daemon se niega a mintear payouts hasta que se restaure un nuevo hash de aprobacion. Registra el commit git y el SHA-256 de la config modificada en el log del incidente.
4. **Notificar al Parlamento de Sora.** Adjunta el ledger de payouts drenado, el reporte shadow-run y un resumen corto del incidente. Las minutas del Parlamento deben registrar el hash de la configuracion revocada y la hora en que se detuvo el daemon.
5. **Validacion de rollback.** Mantener el daemon deshabilitado hasta que:
   - las alertas de telemetria (`soranet_incentives_rules.yml`) esten en verde por >=24 h,
   - el reporte de reconciliacion de tesoreria muestre cero transferencias faltantes, y
   - el Parlamento apruebe un nuevo hash de presupuesto.

Una vez que governance re-emita un hash de aprobacion de presupuesto, actualiza `reward_config.json`
con el nuevo digest, vuelve a ejecutar el comando `shadow-run` sobre la telemetria mas reciente
y reinicia el daemon de incentivos.
