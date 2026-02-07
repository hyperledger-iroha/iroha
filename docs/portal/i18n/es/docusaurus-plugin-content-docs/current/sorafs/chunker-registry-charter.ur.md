---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: carta-registro-fragmento
título: Carta de registro de fragmentos SoraFS
sidebar_label: carta de registro de fragmentos
descripción: Envíos de perfiles de Chunker y aprobaciones کے لیے carta de gobernanza۔
---

:::nota مستند ماخذ
:::

# SoraFS Carta de gobernanza del registro fragmentado

> **Ratificado:** 2025-10-29 por el Panel de Infraestructura del Parlamento de Sora (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). کسی بھی ترمیم کے لیے باضابطہ gobernanza ووٹ درکار ہے؛
> implementación ٹیموں کو اس دستاویز کو اس وقت تک normativo سمجھنا ہوگا جب تک کوئی نئی carta منظور نہ ہو۔

یہ charter SoraFS registro fragmentador کو evolucionar کرنے کے لیے proceso اور roles definen کرتی ہے۔
یہ [Guía de creación de perfiles de Chunker](./chunker-profile-authoring.md) کی تکمیل کرتی ہے اور یہ بیان کرتی ہے کہ نئے
perfiles کیسے proponer, revisar, ratificar اور بالآخر desaprobar ہوتے ہیں۔

## Alcance

یہ carta `sorafs_manifest::chunker_registry` کی ہر entrada پر لاگو ہے اور
ہر اس herramientas پر بھی جو registro استعمال کرتا ہے (CLI de manifiesto, CLI de anuncio de proveedor,
SDK). یہ alias اور manejar invariantes aplicar کرتی ہے جنہیں
`chunker_registry::ensure_charter_compliance()` چیک کرتا ہے:

- ID de perfil: números enteros, ہوتے ہیں جو monotonic طور پر بڑھتے ہیں۔
- Mango canónico `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- Recorte de cadenas de alias کی جاتی ہیں، único ہوتی ہیں، اور دوسری entradas کے identificadores canónicos سے colisionan نہیں کرتیں۔

## Roles- **Autor(es)** – propuesta تیار کرتے ہیں، accesorios regeneran کرتے ہیں، اور evidencia de determinismo جمع کرتے ہیں۔
- **Grupo de Trabajo de Herramientas (TWG)** – listas de verificación publicadas کے ذریعے propuesta validar کرتا ہے اور invariantes de registro برقرار رکھتا ہے۔
- **Consejo de Gobernanza (GC)** – Informe del GTT کا revisión کرتا ہے، sobre de propuesta پر firmar کرتا ہے، اور plazos de publicación/desuso aprobar کرتا ہے۔
- **Equipo de almacenamiento** – implementación del registro mantiene کرتا ہے اور actualizaciones de documentación publica کرتا ہے۔

## Flujo de trabajo del ciclo de vida

1. **Envío de propuesta**
   - Guía de creación del autor کی lista de verificación de validación چلاتا ہے اور
     `docs/source/sorafs/proposals/` کے تحت `ChunkerProfileProposalV1` JSON بناتا ہے۔
   - Salida CLI شامل کریں:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Accesorios, propuesta, informe de determinismo, actualizaciones de registro پر مشتمل PR enviar کریں۔

2. **Revisión de herramientas (TWG)**
   - Lista de verificación de validación دوبارہ چلائیں (accesorios, fuzz, manifiesto/canalización PoR).
   - `cargo test -p sorafs_car --chunker-registry` چلائیں اور یقینی بنائیں کہ
     `ensure_charter_compliance()` Entrada نئی کے ساتھ پاس ہو۔
   - Comportamiento CLI (`--list-profiles`, `--promote-profile`, transmisión
     `--json-out=-`) کی توثیق کریں کہ یہ alias actualizados اور maneja دکھاتا ہے۔
   - Hallazgos y estado de aprobación/rechazo پر مشتمل مختصر رپورٹ تیار کریں۔3. **Aprobación del Consejo (CG)**
   - Informe del GTT sobre los metadatos de la propuesta y la revisión.
   - Resumen de propuesta (`blake3("sorafs-chunker-profile-v1" || bytes)`) پر sign کریں اور
     firmas کو sobre del consejo میں شامل کریں جو accesorios کے ساتھ رکھا جاتا ہے۔
   - Resultado de la votación کو actas de gobierno میں ریکارڈ کریں۔

4. **Publicación**
   - Fusión de relaciones públicas کریں، اور اپڈیٹ کریں:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`, guías de creación/conformidad).
     - Calendario y informes de determinismo.
   - Operadores اور equipos SDK کو نئے perfil اور implementación planificada کے بارے میں اطلاع دیں۔

5. **Desaprobación/Puesta de sol**
   - جو propuestas موجودہ perfil کو reemplazar کرتی ہیں ان میں ventana de publicación dual
     (períodos de gracia) اور plan de actualización شامل ہونا چاہیے۔
     اور libro mayor de migración کو actualización کریں۔

6. **Cambios de emergencia**
   - Eliminación یا revisiones کے لیے votación del consejo اور اکثریتی aprobación درکار ہے۔
   - TWG کو documento de pasos de mitigación de riesgos کرنے اور actualización del registro de incidentes کرنے ہوں گے۔

## Expectativas de herramientas- `sorafs_manifest_chunk_store` y `sorafs_manifest_stub` exponen کرتے ہیں:
  - Inspección de registro کے لیے `--list-profiles`.
  - Promoción de perfil کرتے وقت bloque de metadatos canónicos بنانے کے لیے `--promote-profile=<handle>`.
  - Informes کو stdout پر stream کرنے کے لیے `--json-out=-`, تاکہ registros de revisión reproducibles ممکن ہوں۔
- `ensure_charter_compliance()` binarios relevantes کے inicio پر چلایا جاتا ہے
  (`manifest_chunk_store`, `provider_advert_stub`). Las pruebas de CI fallan ہونا چاہیے اگر
  نئی carta de entradas کی خلاف ورزی کریں۔

## Mantenimiento de registros

- Informes de determinismo تمام کو `docs/source/sorafs/reports/` میں محفوظ کریں۔
- Chunker فیصلوں کا حوالہ دینے والی actas del consejo
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- ہر بڑے cambio de registro کے بعد `roadmap.md` اور `status.md` اپڈیٹ کریں۔

## Referencias

- Guía de creación: [Guía de creación de perfiles de Chunker](./chunker-profile-authoring.md)
- Lista de verificación de conformidad: `docs/source/sorafs/chunker_conformance.md`
- Referencia de registro: [Registro de perfil de Chunker](./chunker-registry.md)