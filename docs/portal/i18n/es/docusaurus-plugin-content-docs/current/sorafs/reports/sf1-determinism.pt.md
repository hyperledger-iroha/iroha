---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS Ejecución en seco del determinismo SF1
resumen: Checklist e digests esperados para validar o perfil chunker canonico `sorafs.sf1@1.0.0`.
---

# SoraFS Funcionamiento en seco del determinismo SF1

Este relatorio captura o dry-run base para el perfil chunker canonico
`sorafs.sf1@1.0.0`. Tooling WG debe reejecutar la lista de verificación a continuación y validar
actualiza los accesorios o las nuevas tuberías de consumo. Regístrese o resultado de cada
comando na tabela para manter um trail auditavel.

## Lista de verificación| Paso | comando | Resultado esperado | Notas |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todas las pruebas pasaron; o teste de paridade `vectors` tem sucesso. | Confirma que los accesorios canónicos se compilan y corresponden a la implementación de Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | O guión sai com 0; reporta os digests de manifest abaixo. | Verifica que los accesorios se regeneran limpios y que las assinaturas permanecen anexadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Una entrada `sorafs.sf1@1.0.0` corresponde a un descriptor de registro (`profile_id=1`). | Garantía de que los metadatos del registro permanecerán sincronizados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneracao ocorre sem `--allow-unsigned`; arquivos de manifest e assinatura nao mudam. | Fornece prueba de determinismo para límites de trozos y manifiestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Informe sobre diferencias entre dispositivos TypeScript y Rust JSON. | Ayudante opcional; Garantizar la paridad entre tiempos de ejecución (script mantido por Tooling WG). |

## Resúmenes esperados

- Resumen de fragmentos (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de cierre de sesión| Datos | Ingeniero | Resultado de la lista de verificación | Notas |
|------|----------|------------------------|-------|
| 2026-02-12 | Herramientas (LLM) | Aceptar | Accesorios regenerados a través de `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, produciendo manejadores canónicos + listas de alias y un resumen manifiesto novo `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado con `cargo test -p sorafs_chunker` y un `ci/check_sorafs_fixtures.sh` limpio (accesorios preparados para verificación). Passo 5 pendente ate o helper de paridade Node chegar. |
| 2026-02-20 | CI de herramientas de almacenamiento | Aceptar | Sobre del Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) obtenido vía `ci/check_sorafs_fixtures.sh`; El script regenera los accesorios, confirma el resumen del manifiesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` y vuelve a ejecutar el arnés Rust (pasos de ejecución de Go/Node cuando están disponibles) sin diferencias. |

Tooling WG debe agregar una línea de datos para ejecutar la lista de verificación. Se algum
passo falhar, abra un problema ligado aquí e incluya detalles de remediación antes
de aprovar novos fixes ou perfis.