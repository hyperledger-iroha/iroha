---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-profile-authoring
כותרת: Guía de autoría de perfiles de chunker de SoraFS
sidebar_label: Guía de autoría de chunker
תיאור: רשימת רשימות עבור פרופונר nuevos perfiles y fixtures de chunker de SoraFS.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_profile_authoring.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# Guía de autoría de perfiles de chunker de SoraFS

Esta guía explica cómo proponer y publicar nuevos perfiles de chunker para SoraFS.
השלמה ל-RFC de arquitectura (SF-1) y la referencia del registro (SF-2a)
con requisitos concretos de autoría, pasos de validación y plantillas de propuesta.
Para un emplo canónico, consulta
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
y el registro de dry-run asociado en
`docs/source/sorafs/reports/sf1_determinism.md`.

## קורות חיים

דף פרופיל que entra en el registro debe:

- הודעה על פרמטרים CDC deterministas y ajustes de multihash idénticos entre
  arquitecturas;
- ניתן לשחזר גופי entregar (JSON Rust/Go/TS + corpora fuzz + testigos PoR) que
  לוס SDKs במורד הזרם puedan אימות חטא tooling a medida;
- כולל metadatos listos para gobernanza (מרחב שמות, שם, סמבר) junto con guía de rollout
  y ventanas operativas; y
- pasar la suite de diff determinista antes de la revisión del consejo.

סיגו אל רשימת הבדיקה של אבאג'ו למען הכנה לא פרופסטה que cumpla esas regglas.

## קורות חיים

Antes de redactar una propuesta, confirma que cumple la carta del registro aplicada
por `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monótona sin huecos.
- El handle canónico (`namespace.name@semver`) debe aparecer en la list de alias
  y **debe** ser la primera entrada. Siguen los alias heredados (עמ' ej., `sorafs.sf1@1.0.0`).
- Ningún alias puede colisionar עם ידית אוטו קנוניקו לא יכול להיות יותר.
- Los alias deben ser no vacíos y recortados de espacios en blanco.

Ayudas útiles de CLI:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos mantienen las propuestas alineadas con la carta del registro y proporcionan los
metadatos canónicos necesarios en las discusiones de gobernanza.

## Metadatos requeridos| קמפו | תיאור | דוגמה (`sorafs.sf1@1.0.0`) |
|-------|-------------|--------------------------------|
| `namespace` | Agrupación lógica de perfiles relacionados. | `sorafs` |
| `name` | כללי התנהגות קריא להומנוס. | `sf1` |
| `semver` | Cadena de versión semántica para el conjunto de parametros. | `1.0.0` |
| `profile_id` | Identificador numérico monotono asignado una vez que el perfil entra. שמור את מספר הזיהויים הבא ללא שימוש חוזר. | `1` |
| `profile_aliases` | מטפל באדיקונאלים אופציונליים (נומברס heredados, abreviaturas) משחרר את הלקוח לאורך זמן. כולל ידית קנוניקה דומה לראשית אנטרד. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | אורך מינימה של נתח בבתים. | `65536` |
| `profile.target_size` | אורך מטרת נתח בבתים. | `262144` |
| `profile.max_size` | אורך מקסימום של נתח בבתים. | `524288` |
| `profile.break_mask` | Máscara adaptativa usada por el rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante del polinomio ציוד (הקסדה). | `0x3da3358b4dc173` |
| `gear_seed` | Seed usada para derivar la tabla gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para digests por chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest del Bundle canónico de fixtures. | `13fa...c482` |
| `fixtures_root` | Directorio relativo que contiene los fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para el muestreo PoR determinista (`splitmix64`). | `0xfeedbeefcafebabe` (דוגמה) |

Los metadatos deben aparecer tanto en el documento de propuesta como dentro de los fixtures
regenerados para que el registro, el tooling de CLI y la automatización de gobernanza puedan
confirmar los valores sin cruces manuales. ללא שם: Si hay dudas, Ejecuta los CLIs de Chunk-Store y
מניפסט קון `--json-out=-` עבור שידור חישובי מטאטא לאס לאס דה רוויזיון.

### נקודות מגע של CLI y registro

- `sorafs_manifest_chunk_store --profile=<handle>` - הוספת מטא נתונים של נתח,
  digest de manifest y checks PoR con los parametros propuestos.
- `sorafs_manifest_chunk_store --json-out=-` - שדר את הדיווח של chunk-store a
  סטדאוט עבור השוואות אוטומטיות.
- `sorafs_manifest_stub --chunker-profile=<handle>` — אישור que manifestes Y מטוסים CAR
  embeben el handle canónico más los alias.
- `sorafs_manifest_stub --plan=-` — המשך לחיצה על `chunk_fetch_specs` הקודם לסעיף
  אימות מקזז/עכל את התוצאות.

Registra la salida de comandos (עיכובים, raíces PoR, hashes de manifest) en la propuesta para que
los revisores puedan reproducirlos textualmente.

## רשימת רשימת קביעות ואימות1. **משחקי Regenerar**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar la suite de paridad** — `cargo test -p sorafs_chunker` y el arnés diff
   entre lenguajes (`crates/sorafs_chunker/tests/vectors.rs`) deben estar en verde con los
   nuevos fixtures en su lugar.
3. **משחזר גופים מטושטש/לחץ אחורי** — ejecuta `cargo fuzz list` y el arnés de
   סטרימינג (`fuzz/sorafs_chunker`) contra los activos regenerados.
4. **אימות עדויות הוכחה לשליפה** - ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` usando el perfil propuesto y confirma que las
   raíces coinciden con el manifest de fixtures.
5. **Dry run de CI** - invoca `ci/check_sorafs_fixtures.sh` localmente; תסריט אל
   debe tener éxito con los nuevos fixtures y el `manifest_signatures.json` existente.
6. **זמן ריצה צולב** - בדיקה לגבי חיבורים Go/TS consumen el JSON regenerado
   y emiten límites y digests idénticos.

Documenta los comandos y los digests resultantes en la propuesta para que el Tooling WG pueda
repetirlos sin conjeturas.

### אישור מניפסט / PoR

Después de regenerar fixtures, ejecuta el pipeline completo de manifest para asegurar que los
metadatos CAR y las pruebas PoR sigan siendo עקביות:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Reemplaza el archivo de entrada con cualquier corpus representativo usado por tus גופי
(עמ' ej., el stream determinista de 1 GiB) y adjunta los digests resultantes a la propuesta.

## Plantilla de propuesta

Las propuestas se envían como registros Norito `ChunkerProfileProposalV1` registrados en
`docs/source/sorafs/proposals/`. La plantilla JSON de abajo ilustra la forma esperada
(sustituye tus valores según sea cecesario):


Proporciona un reporte Markdown correspondiente (`determinism_report`) que capture la
salida de comandos, los digests de chunk y cualquier desviación encontrada durante la validación.

## Flujo de gobernanza

1. **Enviar PR con propuesta + fixtures.** Incluye los assets generados, la propuesta
   Norito y las actualizaciones a `chunker_registry_data.rs`.
2. **Revisión del Tooling WG.** Los revisores reejecutan el checklist de validación y
   confirman que la propuesta se alinea con las regglas del registro (sin reutilización de id,
   determinismo satisfecho).
3. **Sobre del consejo.** Una vez aprobado, los miembros del consejo firman el digest de
   la propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) y anexan sus
   פירמס אל סובר דל פרפיל guardado junto a los fixtures.
4. **פרסום הרישום.** מיזוג מציאות של רישום, מסמכים ומשחקים. אל
   CLI por defecto permanece en el perfil previo hasta que la gobernanza declare la migración
   רשימה.
5. **Seguimiento deprecación.** Después de la ventana de migración, actualiza el registro
   ספר הגירה.

## Consejos de autoría- Prefiere límites de potencia de dos pares para minimizar comportamiento de chunking in casos bord.
- Evita cambiar el código multihash sin coordinar consumidores de manifest y gateway; כולל אחת
  nota operativa cuando lo hagas.
- Mantén las seeds de la tabla gear קריא para humanos pero globalmente únicas para simplificar
  auditorias.
- Guarda cualquier artefacto de benchmarking (עמ' ej., comparaciones de throughput) bajo
  `docs/source/sorafs/reports/` עבור התייחסות עתידית.

עבור פעולות צפויות לאורך זמן ההפצה, ייעוץ ופנקס ההגירה
(`docs/source/sorafs/migration_ledger.md`). Para regglas de conformidad en runtime ver
`docs/source/sorafs/chunker_conformance.md`.