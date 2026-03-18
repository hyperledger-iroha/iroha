---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS Ejecución en seco del determinismo SF1
resumen: Чеклист и ожидаемые resúmenes для проверки канонического fragmentador perfil `sorafs.sf1@1.0.0`.
---

# SoraFS Funcionamiento en seco del determinismo SF1

Esta es la función de funcionamiento en seco del programa de procesamiento de archivos canónicos.
`sorafs.sf1@1.0.0`. Tooling WG должна повторять чеклист ниже при проверке
обновлений accesorios или новых tuberías de consumo. Записывайте результат каждой
команды в таблицу, чтобы сохранить rastro auditable.

## Cheklist| Шаг | Comandante | Respuestas recientes | Примечания |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Все тесты проходят; Prueba de paridad `vectors`. | Además, qué accesorios canónicos están compilados y compatibles con Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Script завершается 0; сообщает digiere manifiestos ниже. | Asegúrese de que los accesorios se regeneren y se ajusten a sus necesidades. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | El archivo `sorafs.sf1@1.0.0` incluye el descriptor de registro (`profile_id=1`). | Garantía del registro de metadatos establecido de forma sincronizada. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La regeneración se realiza según `--allow-unsigned`; файлы manifiesto y firma не меняются. | Esta es la determinación de los límites de fragmentos y los manifiestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Сообщает отсутствие diff entre accesorios de TypeScript y Rust JSON. | Ayudante opcional; обеспечьте паритет между runtime (script поддерживает Tooling WG). |

## Ожидаемые resúmenes

- Resumen de fragmentos (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de cierre de sesión| Datos | Ingeniero | Lista de resultados | Примечания |
|------|----------|--------------------|-------|
| 2026-02-12 | Herramientas (LLM) | ✅ Aprobado | Los accesorios están regidos por `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, con identificadores canónicos + alias específicos y un nuevo resumen de manifiesto `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Проверено `cargo test -p sorafs_chunker` и чистым прогоном `ci/check_sorafs_fixtures.sh` (accesorios disponibles para proveedores). Ejemplo 5 de ayuda para la paridad de nodos. |
| 2026-02-20 | CI de herramientas de almacenamiento | ✅ Aprobado | Sobre del Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) получен через `ci/check_sorafs_fixtures.sh`; El script regeneriza los accesorios, incluye el resumen del manifiesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` y el antiguo hardware Rust (con Go/Node utilizado anteriormente) entre las diferencias. |

Tooling WG должна добавить датированную строку после выполнения чеклиста. Если
какой-либо шаг падает, заведите problema со ссылкой здесь y добавьте detalles
remediación до утверждения новых accesorios или профилей.