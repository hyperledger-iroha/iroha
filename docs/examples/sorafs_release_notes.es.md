---
lang: es
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CLI y SDK - Notas de release (v0.1.0)

## Destacados
- `sorafs_cli` ahora envuelve toda la canalizacion de empaquetado (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) para que los runners de CI invoquen un
  solo binario en lugar de helpers ad-hoc. El nuevo flujo de firma sin llaves usa por defecto
  `SIGSTORE_ID_TOKEN`, entiende proveedores OIDC de GitHub Actions y emite un resumen JSON deterministico
  junto con el bundle de firmas.
- El *scoreboard* de fetch multi-source se entrega como parte de `sorafs_car`: normaliza
  la telemetria de proveedores, aplica penalizaciones de capacidad, persiste reportes JSON/Norito, y
  alimenta el simulador del orquestador (`sorafs_fetch`) mediante el registry handle compartido.
  Los fixtures en `fixtures/sorafs_manifest/ci_sample/` demuestran las entradas y salidas deterministicas
  que CI/CD debe comparar.
- La automatizacion de releases esta codificada en `ci/check_sorafs_cli_release.sh` y
  `scripts/release_sorafs_cli.sh`. Cada release ahora archiva el bundle de manifiesto,
  la firma, los resumenes `manifest.sign/verify` y el snapshot del scoreboard para que reviewers de
  governance puedan rastrear artefactos sin re-ejecutar el pipeline.

## Compatibilidad
- Cambios breaking: **Ninguno.** Todas las adiciones de CLI son flags/subcommands aditivos; las
  invocaciones existentes siguen funcionando sin cambios.
- Versiones minimas de gateway/node: requiere Torii `2.0.0-rc.2.0` (o mas nuevo) para que la
  API de rango de chunks, cuotas de stream-token, y headers de capacidad expuestos por
  `crates/iroha_torii` esten disponibles. Los nodos de storage deben ejecutar el stack host de SoraFS del
  commit `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (incluye los nuevos inputs del scoreboard y el cableado
  de telemetria).
- Dependencias upstream: No hay bumps de terceros mas alla de la base del workspace; el release
  reutiliza las versiones fijadas de `blake3`, `reqwest`, y `sigstore` en `Cargo.lock`.

## Pasos de actualizacion
1. Actualiza los crates alineados en tu workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Re-ejecuta la gate de release localmente (o en CI) para confirmar cobertura de fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Regenera artefactos firmados y resumenes con la config curada:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Copia los bundles/proofs renovados en `fixtures/sorafs_manifest/ci_sample/` si el release
   actualiza los fixtures canonicos.

## Verificacion
- Commit de release gate: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` justo despues de que el gate paso).
- Salida de `ci/check_sorafs_cli_release.sh`: archivada en
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (adjunta al bundle de release).
- Digest del bundle de manifiesto: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Digest del resumen de proof: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Digest del manifiesto (para cruces de attestation downstream):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (de `manifest.sign.summary.json`).

## Notas para operadores
- El gateway Torii ahora aplica el header de capacidad `X-Sora-Chunk-Range`. Actualiza
  allowlists para que los clientes que presenten los nuevos scopes de stream token sean admitidos;
  los tokens antiguos sin el claim de rango seran throttled.
- `scripts/sorafs_gateway_self_cert.sh` integra la verificacion de manifiestos. Al ejecutar el
  harness de self-cert, entrega el bundle de manifiesto recien generado para que el wrapper falle
  rapido si hay drift de firmas.
- Los dashboards de telemetria deben ingerir el nuevo export del scoreboard (`scoreboard.json`) para
  conciliar elegibilidad de proveedores, asignacion de pesos y razones de rechazo.
- Archiva los cuatro resumenes canonicos con cada rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Los tickets de governance hacen referencia a estos archivos
  exactos durante la aprobacion.

## Agradecimientos
- Storage Team - consolidacion end-to-end del CLI, renderer de chunk-plan, y cableado de
  telemetria del scoreboard.
- Tooling WG - pipeline de release (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) y bundle deterministico de fixtures.
- Gateway Operations - capability gating, revision de politica de stream-token, y playbooks
  actualizados de self-certificacion.
