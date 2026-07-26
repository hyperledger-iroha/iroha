---
lang: es
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Usa este brief para desplegar la configuracion de compliance SNNet-9 con un proceso repetible y amigable para auditoria. Emparejalo con la revision jurisdiccional para que cada operador use los mismos digests y el mismo layout de evidencia.

## Pasos

1. **Armar configuracion**
   - Importa `governance/compliance/soranet_opt_outs.json`.
   - Mezcla tus `operator_jurisdictions` con los digests de atestacion publicados
     en la [revision jurisdiccional](gar-jurisdictional-review).
2. **Validar**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Opcional: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Capturar evidencia**
   - Guarda bajo `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (bloque de compliance final)
     - `attestations.json` (URIs + digests)
     - logs de validacion
     - referencias a PDFs/Norito envelopes firmados
4. **Activar**
   - Etiqueta el rollout (`gar-opt-out-<date>`), redeploya configs de orchestrator/SDK,
     y confirma que los eventos `compliance_*` se emiten en los logs donde corresponde.
5. **Cierre**
   - Archiva el paquete de evidencia con Governance Council.
   - Registra la ventana de activacion y aprobadores en el GAR logbook.
   - Programa las fechas de proxima revision segun la tabla de revision jurisdiccional.

## Checklist rapido

- [ ] `jurisdiction_opt_outs` coincide con el catalogo canonico.
- [ ] Digests de atestacion copiados exactamente.
- [ ] Comandos de validacion ejecutados y archivados.
- [ ] Paquete de evidencia guardado en `artifacts/soranet/compliance/<date>/`.
- [ ] Tag de rollout y GAR logbook actualizados.
- [ ] Recordatorios de proxima revision configurados.

## Ver tambien

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
