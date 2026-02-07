---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Registro de migración SoraFS
descripción: Journal des changements canonique qui suit chaque jalon de migracion, les responsables et les suivis requis.
---

> Adapte de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Registro de migración SoraFS

Este registro reporta la captura de la revista de migraciones en el RFC de arquitectura
SoraFS. Les entrees sont groupees par jalon et indicant la fenetre Effective,
les equipes impactees et les action requises. Les misses a jour du plan de
Modificador DOIVENT de migración en esta página y en el RFC
(`docs/source/sorafs_architecture_rfc.md`) para guardar los consumidores en aval
se alinea.

| Jalón | Fentre eficaz | Currículum del cambio | Equipos impactados | Acciones | Estatuto |
|-------|-------------------|---------------------|------------------|---------|--------|
| M1 | Semanas 7-12 | Le CI impone los accesorios deterministas; les preuves d'alias sont disponibles en puesta en escena; le tooling exponen des flags d'attente explicites. | Documentos, almacenamiento, gobernanza | Asegúrese de que los accesorios retengan a los firmantes, registre los alias en el registro de puesta en escena, mettre a jour les checklists de release avec l'exigence `--car-digest/--root-cid`. | ⏳ Attente |Les minutas du plan de controle de gouvernance qui referencent ces jalons vivent sous
`docs/source/sorafs/`. Les equipes doivent ajouter des puces datees sous chaque ligne
lorsque des eventements notables surviennent (ej: nouveaux enregistrements d'alias,
retrospectivas de incidentes del registro) afin de proporcionar un rastro auditable.

## Mises un día reciente

- 2025-11-01 — Difusión de `migration_roadmap.md` al consejo de gobierno y aux listas
  operadores para revisión; en attente de validación lors de la prochaine session du
  consejo (ref: suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI d'enregistrement du Pin Registry applique desormais la validación
  partagee chunker/politique vía les helpers `sorafs_manifest`, gardant les chemins
  La cadena se alinea con los controles Torii.
- 2026-02-13 — Agregar las fases de implementación del anuncio del proveedor (R0–R3) en el registro y
  Publicación de paneles de control y asociaciones de operadores de orientación.
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).