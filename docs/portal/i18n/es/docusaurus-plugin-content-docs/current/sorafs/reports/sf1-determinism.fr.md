---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS Ejecución en seco del determinismo SF1
resumen: Lista de verificación y resúmenes asistentes para validar el perfil fragmentador canónico `sorafs.sf1@1.0.0`.
---

# SoraFS Funcionamiento en seco del determinismo SF1

Esta relación captura el ensayo de base para el perfil fragmentador canónico
`sorafs.sf1@1.0.0`. Tooling WG debe relanzar la lista de verificación ci-dessous lors de la
validación de actualizaciones de accesorios o de nuevas tuberías de consumidores.
Consignez le résultat de chaque commande dans le tableau afin de maintenir une
traza auditable.

## Lista de verificación| Étape | comando | Resultados de asistencia | Notas |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todas las pruebas pasaron; La prueba de paridad `vectors` se vuelve a utilizar. | Confirme que los dispositivos canónicos están compilados y correspondientes a la implementación de Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | El script ordena en 0; rapporte les digests de manifest ci-dessous. | Verifique que los accesorios estén regénèrent proprement y que las firmas estén agregadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | La entrada para `sorafs.sf1@1.0.0` corresponde al descriptor de registro (`profile_id=1`). | Asegúrese de que los metadatos del registro estén sincronizados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La regeneración se reanudará sin `--allow-unsigned`; Los archivos de manifiesto y de firma permanecen incambiados. | Fournit une preuve de determinismo pour les limites de fragment et les manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | No hay diferencias entre los dispositivos TypeScript y JSON Rust. | Opción de ayuda; Garantizar la paridad entre tiempos de ejecución (mantenimiento del script por Tooling WG). |

## Resúmenes de asistencia

- Resumen de fragmentos (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Diario de cierre de sesión| Fecha | Ingeniero | Resultados de la lista de verificación | Notas |
|------|----------|-----------------------|-------|
| 2026-02-12 | Herramientas (LLM) | ✅ Réussi | Accesorios regénérées a través de `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, produciendo la lista canónica + alias y un resumen manifiesto fresco `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` propre (accesorios en etapa de verificación). Etapa 5 en atención justo cuando llegó el asistente de paridad Node. |
| 2026-02-20 | CI de herramientas de almacenamiento | ✅ Réussi | Sobre del Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) recuperado vía `ci/check_sorafs_fixtures.sh`; El script cambió los accesorios, confirmó el resumen de manifiesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` y relanzó el arnés Rust (las etapas Go/Node se ejecutan cuando están disponibles) sin diferencias. |

Tooling WG debe agregar una línea fechada después de la ejecución de la lista de verificación. si une
Étape échoue, solucione un problema aquí e incluya los detalles de reparación.
Avant d'approuver de nouveaux accesorios o perfiles.