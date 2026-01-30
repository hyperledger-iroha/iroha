---
lang: es
direction: ltr
source: docs/source/sorafs/migration_roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 15434e006f72dee4a5b47bba791c2ecb7c026ca2f27419f14be16725daa4e513
source_last_modified: "2025-11-02T04:40:40.141904+00:00"
translation_last_reviewed: "2026-01-30"
---

# Roadmap de migracion SoraFS (SF-1)

Este documento operacionaliza la guia de migracion capturada en
`docs/source/sorafs_architecture_rfc.md`. Expande los entregables SF-1 en
hitos listos para ejecucion, criterios de gate y checklists de owners para que
storage, hosting de artefactos y la publicacion respaldada por SoraFS queden
alineados.

El roadmap es intencionalmente determinista: cada hito nombra los artefactos
requeridos, invocaciones de comandos y pasos de atestacion para que los
pipelines downstream produzcan outputs identicos y governance retenga un rastro
auditado.

## Overview de hitos

| Hito | Ventana | Objetivos primarios | Must Ship | Owners |
|------|---------|--------------------|-----------|--------|
| **M1 – Deterministic Enforcement** | Semanas 7-12 | Forzar fixtures firmados y stagear pruebas de alias mientras los pipelines adoptan flags de expectativa. | Verificacion nightly de fixtures, manifiestos firmados por el council, entradas de staging del alias registry. | Storage, Governance, SDKs |

El estado de los hitos se rastrea en `docs/source/sorafs/migration_ledger.md`. Todo
cambio a este roadmap DEBE actualizar el ledger para mantener en sync a
governance y release engineering.

## Workstreams

### 2. Adopcion de pinning determinista

| Step | Hito | Descripcion | Owner(s) | Output |
|------|------|-------------|----------|--------|
| Fixture rehearsals | M0 | Dry-runs semanales comparando digests locales de chunk contra `fixtures/sorafs_chunker`. Publicar reporte bajo `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` con matriz pass/fail. |
| Enforce signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` fallan si firmas o manifiestos hacen drift. Overrides de desarrollo requieren waiver de governance adjunto al PR. | Tooling WG | Log de CI, link de ticket de waiver (si aplica). |
| Expectation flags | M1 | Pipelines llaman `sorafs_manifest_stub` con expectativas explicitas para pinnear outputs: | Docs CI | Scripts actualizados referenciando flags de expectativa (ver bloque de comando abajo). |
| Registry-first pinning | M2 | `sorafs pin propose` y `sorafs pin approve` envuelven submissions de manifiesto; el CLI default a `--require-registry`. | Governance Ops | Audit log del CLI de registry, telemetria de proposals fallidas. |
| Observability parity | M3 | Dashboards Prometheus/Grafana alertan cuando los inventarios de chunks divergen de manifiestos del registry; alertas cableadas al on-call de ops. | Observability | Link a dashboard, IDs de reglas de alerta, resultados de GameDay. |

#### Comando canonico de publicacion

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Reemplaza los valores de digest, size y CID con las referencias esperadas
registradas en la entrada del migration ledger para el artefacto.

### 3. Transicion de alias y comunicaciones

| Step | Hito | Descripcion | Owner(s) | Output |
|------|------|-------------|----------|--------|
| Alias proofs en staging | M1 | Registrar alias claims en el entorno staging del Pin Registry y adjuntar pruebas Merkle a manifiestos (`--alias`). | Governance, Docs | Bundle de pruebas guardado junto al manifiesto + comentario en el ledger con el nombre de alias. |
| Enforce proofs | M2 | Los gateways rechazan manifiestos sin headers `Sora-Proof` recientes; CI gana un paso `sorafs alias verify` para obtener pruebas. | Networking | Patch de config del gateway + output de CI capturando verificacion exitosa. |

### 4. Comunicacion y auditoria

- **Disciplina de ledger:** cada cambio de estado (drift de fixtures, submission
  al registry, activacion de alias) debe adjuntar una nota con fecha en
  `docs/source/sorafs/migration_ledger.md`.
- **Minutas de governance:** sesiones del council que aprueben cambios del pin
  registry o politicas de alias deben referenciar este roadmap y el ledger.
- **Comunicaciones externas:** DevRel publica status updates en cada hito (blog +
  extracto de changelog) destacando garantias deterministas y timelines de alias.

## Dependencias y riesgos

| Dependencia | Impacto | Mitigacion |
|-------------|---------|-----------|
| Disponibilidad del contrato Pin Registry | Bloquea rollout M2 pin-first. | Stagear el contrato antes de M2 con replay tests; mantener fallback de sobres hasta estar libre de regresiones. |
| Llaves de firmado del council | Requeridas para sobres de manifiesto y approvals del registry. | Ceremonia de firmado documentada en `docs/source/sorafs/signing_ceremony.md`; rotar llaves con overlap y nota en el ledger. |
| Cadencia de release de SDK | Los clientes deben honrar pruebas de alias antes de M3. | Alinear ventanas de release de SDK con gates de hito; agregar checklists de migracion a templates de release. |

Riesgos residuales y mitigaciones se espejan en `docs/source/sorafs_architecture_rfc.md`
y deben cross-referenciarse al hacer ajustes.

## Checklist de criterios de salida

| Hito | Criterio |
|------|----------|
| M1 | - Nightly fixture job verde por siete dias consecutivos. <br> - Pruebas de alias en staging verificadas en CI. <br> - Governance ratifica la politica de expectation flags. |

## Gestion de cambios

1. Proponer ajustes via PR actualizando este archivo **y**
   `docs/source/sorafs/migration_ledger.md`.
2. Enlazar minutas de governance y evidencia de CI en la descripcion del PR.
3. Al mergear, notificar a las listas de correo de storage + DevRel con un
   resumen y acciones esperadas para operadores.

Seguir este procedimiento asegura que el rollout SoraFS permanezca determinista,
auditable y transparente entre equipos que participan en el lanzamiento Nexus.
