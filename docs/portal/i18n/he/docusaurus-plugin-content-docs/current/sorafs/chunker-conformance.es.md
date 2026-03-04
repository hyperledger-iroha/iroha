---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-conformance
כותרת: Guía de conformidad del chunker de SoraFS
sidebar_label: Conformidad de chunker
תיאור: דרישות ושירותים לשמירה על הגדרות של chunker SF1 עם מתקנים ו-SDKs.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_conformance.md`. Mantén ambas versiones sincronizadas hasta que se retiren los docs heredados.
:::

Esta guía codifica los requisitos que toda implementación debe seguir para mantenerse
alineada con el perfil determinista de chunker de SoraFS (SF1). טמביאן
דוקומנטה אל פלוג'ו דה ריגנרציון, לה פוליטיקה דה פירמס y los pasos de verificación para que
los consumidores de fixtures en los SDKs permanezcan sincronizados.

## פרופיל קנוני

- ידית פרופיל: `sorafs.sf1@1.0.0` (כינוי heredado `sorafs.sf1@1.0.0`)
- Seed de entrada (hex): `0000000000dec0ded`
- מטרה: 262144 בתים (256 KiB)
- Minimo: 65536 בייטים (64 KiB)
- כמות גדולה: 524288 בייטים (512 KiB)
- Polinomio de rolling: `0x3DA3358B4DC173`
- Seed de la tabla ציוד: `sorafs-v1-gear`
- מסיכת שבירה: `0x0000FFFF`

יישום הפניה: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Cualquier Acceleración SIMD debe producir Limites y digests idénticos.

## חבילת מתקנים

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera los
מתקנים y emite los suientes archivos bajo `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — גבולות הנתח קנוניקוס לצריכה
  Rust, TypeScript ו-Go. Cada archivo anuncia el handle canónico como la primera
  entrada en `profile_aliases`, seguido por cualquier alias heredado (עמוד ej.,
  `sorafs.sf1@1.0.0`, luego `sorafs.sf1@1.0.0`). El orden se impone por
  `ensure_charter_compliance` y NO DEBE alterarse.
- `manifest_blake3.json` - אימות מניפסט עם BLAKE3 que cubre cada archivo de fixtures.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sobre el digest del manifest.
- `sf1_profile_v1_backpressure.json` y corpora en bruto dentro de `fuzz/` —
  תרשימי הסטרימינג של ארה"ב על ידי שימוש בלחץ אחורי של צ'אנקר.

### פוליטיקה דה פירמס

La regeneración de fixtures **debe** כולל את מבנה המבנה. אל גנרדור
rechaza la salida sin firmar a menos que se pase explícitamente `--allow-unsigned` (pensado
solo para experimentación local). Los sobres de firma son append-only y se
deduplican por firmante.

עבור אגרגר una firma del consejo:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## אימות

El helper de CI `ci/check_sorafs_fixtures.sh` reejecuta el generador con
`--locked`. אביזרי ההתקן משתנים או פנטן פירמס, אל ג'וב פאלה. ארה"ב
es script en זרימות עבודה לילה ו-antes de enviar cambios de fixtures.

הוראות אימות:

1. Ejecuta `cargo test -p sorafs_chunker`.
2. Invoca `ci/check_sorafs_fixtures.sh` localmente.
3. אשר את `git status -- fixtures/sorafs_chunker` esté limpio.

## Playbook de actualización

הצג פרופיל חדש של chunker או מציאות SF1:

פירושו: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) עבור
דרישות המטאדאטוס, הצמחים של הפרופסטה ורשימות הבדיקה.1. Redacta un `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) con nuevos parametros.
2. אביזרי Regenera vía `export_vectors` y registra el nuevo digest del manifest.
3. Firma el manifest con el quórum del consejo requerido. Todas las firmas deben
   anexarse a `manifest_signatures.json`.
4. אקטואליזציה של מכשירי SDK (Rust/Go/TS) וזמן ריצה חוצה.
5. Regenera corpora fuzz si cambian los parametros.
6. Actualiza esta guía con el nuevo handle de perfil, זרעים y לעכל.
7. Envía el cambio junto con pruebas actualizadas y actualizaciones del roadmap.

Los cambios que afecten los límites de chunk o los digests sin seguir este processo
son inválidos y no deben fusionarse.