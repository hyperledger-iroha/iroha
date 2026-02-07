---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry-charter
כותרת: Carta del registro de chunker de SoraFS
sidebar_label: Carta del registro de chunker
תיאור: Carta de gobernanza para presentaciones y aprobaciones de perfiles de chunker.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry_charter.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# Carta de gobernanza del registro de chunker de SoraFS

> **Ratificado:** 2025-10-29 por el Sora Parliament Infrastructure Panel (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Cualquier enmienda requiere un
> voto de gobernanza פורמלי; los equipos de implementación deben tratar este documento como
> normativo hasta que se apruebe una carta que lo sustituya.

אסטרטגיות מגדירים את התפקידים האל-פרוסיאו וה-לוסים עבור אבולוציונאר אל רישום ה-chunker de SoraFS.
Complementa la [Guía de autoría de perfiles de chunker](./chunker-profile-authoring.md) al decribir cómo nuevos
פרפילים הם פרופונן, revisan, ratifican y eventualmente se deprecian.

## אלקנס

La carta aplica a cada entrada en `sorafs_manifest::chunker_registry` y
כלי עזר לצרכן רישום (CLI de manifest, CLI de provider-advert,
ערכות SDK). Impone las invariantes de alias y handle verificadas por
`chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monótona.
- ידית El canónico `namespace.name@semver` **debe** aparecer como primera
  entrada en `profile_aliases`. סיגואן לוס alias heredados.
- Las cadenas de alias se recortan, son únicas y no colisionan con handles canónicos
  de otras entradas.

## תפקידים

- **הכותבים** - מכינים את הפרופסטה, מתקנים מחודשים וחידושים
  evidencia de determinismo.
- **קבוצת עבודה בכלים (TWG)** - valida la propuesta usando las checklists
  publicadas y asegura que las invariantes del registro se cumplan.
- **מועצת הממשל (GC)** - revisa el reporte del TWG, firma el sobre de la propuesta
  y aprueba los plazos de publicación/deprecación.
- **צוות אחסון** - mantiene la implementación del registro y publica
  actualizaciones de documentación.

## Flujo del ciclo de vida

1. **Presentación de propuesta**
   - El autor ejecuta la checklist de validación de la guía de autoría y crea
     un JSON `ChunkerProfileProposalV1` en
     `docs/source/sorafs/proposals/`.
   - כולל:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envía un PR que contenga גופי, פרופווסטה, reporte determinismo y
     אקטואליזציוני רישום.

2. **Revision de tooling (TWG)**
   - שחזר את רשימת התיוג של אימות (תקנים, fuzz, pipeline de manifest/PoR).
   - Ejecuta `cargo test -p sorafs_car --chunker-registry` y asegura que
     `ensure_charter_compliance()` pase con la nueva entrada.
   - Verifica que el comportamiento del CLI (`--list-profiles`, `--promote-profile`, סטרימינג
     `--json-out=-`) refleje los alias y מטפל ב-actualizados.
   - הפק un reporte breve que resuma los hallazgos y el estado de aprobación/rechazo.3. **Aprobación del consejo (GC)**
   - Revisa el reporte del TWG y la metadata de la propuesta.
   - Firma el digest de la propuesta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     y añade las firmas al sobre del consejo mantenido junto a los fixtures.
   - Registra el resultado de la votación en las actas de gobernanza.

4. **פרסום**
   - Fusiona el PR, actualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`, guías de autoría/conformidad).
     - מתקנים y reportes determinismo.
   - הודעה על הפעלה וציוד SDK של פרופיל חדש לתוכנית השקה.

5. **Deprecación / Retiro**
   - Las propuestas que sustituyen un perfil existente deben incluir una ventana de publicación
     כפול (periodos de gracia) y un plan de actualización.
     en el registro y actualiza el ספר דה מיגרציון.

6. **Cambios de emergencia**
   - ביטולי התיקונים החמים דורשים את ההצבעה על ההסכמה על ידי אישור מאיוריה.
   - El TWG debe documentar los pasos de mitigación de riesgos y actualizar el registro de incidentes.

## ציפיות של כלי עבודה

- `sorafs_manifest_chunk_store` y `sorafs_manifest_stub` הסבר:
  - `--list-profiles` עבור בדיקת רישום.
  - `--promote-profile=<handle>` עבור כללי אל בלוקו דה מטאטאטוס קנוניקו בארה"ב
    al promotor un perfil.
  - `--json-out=-` עבור שידור מדווח על סטדout, יומני עדכון
    ניתנים לשחזור.
- `ensure_charter_compliance()` se invoca al inicio en los binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Las pruebas CI deben fallar si
  nuevas entradas וילן לה קארטה.

## רישום

- Guarda todos los reportes de determinismo en `docs/source/sorafs/reports/`.
- Las actas del consejo que referencian decisiones de chunker viven en
  `docs/source/sorafs/migration_ledger.md`.
- Actualiza `roadmap.md` y `status.md` después de cada cambio mayor del registro.

## אסמכתאות

- Guía de autoría: [Guía de autoría de perfiles de chunker](./chunker-profile-authoring.md)
- רשימת תאימות: `docs/source/sorafs/chunker_conformance.md`
- התייחסות לרישום: [רישום פרופילי chunker](./chunker-registry.md)