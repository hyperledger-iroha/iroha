---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist y digests esperados para validar el perfil chunker canonico `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 Determinism Dry-Run

Este reporte captura el dry-run base para el perfil chunker canonico
`sorafs.sf1@1.0.0`. Tooling WG debe re-ejecutar el checklist de abajo cuando
valide refreshes de fixtures o nuevos pipelines de consumidores. Registra el
resultado de cada comando en la tabla para mantener un trail auditable.

## Checklist

| Paso | Comando | Resultado esperado | Notas |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos los tests pasan; el test de paridad `vectors` tiene exito. | Confirma que los fixtures canonicos compilan y coinciden con la implementacion Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | El script sale con 0; reporta los digests de manifest de abajo. | Verifica que los fixtures se regeneren limpiamente y que las firmas permanezcan adjuntas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | La entrada de `sorafs.sf1@1.0.0` coincide con el descriptor del registry (`profile_id=1`). | Asegura que la metadata del registry siga sincronizada. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La regeneracion tiene exito sin `--allow-unsigned`; los archivos de manifest y firma no cambian. | Provee prueba de determinismo para limites de chunk y manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Reporta que no hay diff entre fixtures TypeScript y Rust JSON. | Helper opcional; asegurar paridad entre runtimes (script mantenido por Tooling WG). |

## Digests esperados

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Sign-Off Log

| Fecha | Engineer | Resultado del checklist | Notas |
|------|----------|-------------------------|-------|
| 2026-02-12 | Tooling (LLM) | OK | Fixtures regenerados via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, produciendo handle canonico + alias lists y un manifest digest fresco `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado con `cargo test -p sorafs_chunker` y una corrida limpia de `ci/check_sorafs_fixtures.sh` (fixtures preparados para la verificacion). Paso 5 pendiente hasta que llegue el helper de paridad Node. |
| 2026-02-20 | Storage Tooling CI | OK | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) obtenido via `ci/check_sorafs_fixtures.sh`; el script regenero fixtures, confirmo el manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, y re-ejecuto el harness Rust (pasos Go/Node se ejecutan cuando estan disponibles) sin diffs. |

Tooling WG debe anexar una fila fechada despues de ejecutar el checklist. Si
algun paso falla, abre un issue enlazado aqui e incluye detalles de remediacion
antes de aprobar nuevos fixtures o perfiles.
