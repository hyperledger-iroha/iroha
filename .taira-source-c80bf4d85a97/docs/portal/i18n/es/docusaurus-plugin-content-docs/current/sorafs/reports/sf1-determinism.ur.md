---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS Ejecución en seco del determinismo SF1
resumen: perfil de fragmentación canónico `sorafs.sf1@1.0.0` para validar la lista de verificación de los resúmenes esperados.
---

# SoraFS Funcionamiento en seco del determinismo SF1

یہ رپورٹ perfil de fragmentación canónico `sorafs.sf1@1.0.0` کے ejecución en seco de línea base کو
capturar کرتی ہے۔ Los accesorios de Tooling WG کو actualizan یا نئے canales de consumo کی
validación کے وقت نیچے والا lista de verificación دوبارہ چلانا چاہیے۔ ہر کمانڈ کا نتیجہ
ٹیبل میں ریکارڈ کریں تاکہ rastro auditable برقرار رہے۔

## Lista de verificación| Paso | Comando | Resultado esperado | Notas |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تمام pruebas پاس ہوں؛ `vectors` prueba de paridad کامیاب ہو۔ | تصدیق کرتا ہے کہ accesorios canónicos compilar ہوتے ہیں اور Implementación de Rust سے coincidencia کرتے ہیں۔ |
| 2 | `ci/check_sorafs_fixtures.sh` | اسکرپٹ 0 کے ساتھ salir کرے؛ نیچے والے resúmenes de manifiestos رپورٹ کرے۔ | verificar کرتا ہے کہ accesorios صاف طور پر regenerar ہوں اور firmas adjuntas رہیں۔ |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` کا descriptor de registro de entrada (`profile_id=1`) سے coincide con کرے۔ | یقینی بناتا ہے کہ sincronización de metadatos del registro رہے۔ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | regeneración `--allow-unsigned` کے بغیر tener éxito کرے؛ manifiesto اور firma فائلیں sin cambios رہیں۔ | límites de fragmentos اور manifiesta کے لئے prueba de determinismo فراہم کرتا ہے۔ |
| 5 | `node scripts/check_sf1_vectors.mjs` | Accesorios de TypeScript اور Rust JSON کے درمیان کوئی diff report نہ کرے۔ | ayudante opcional؛ tiempos de ejecución کے درمیان paridad یقینی بنائیں (script Tooling WG mantiene کرتا ہے)۔ |

## Resúmenes esperados

- Resumen de fragmentos (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de cierre de sesión| Fecha | Ingeniero | Resultado de la lista de verificación | Notas |
|------|----------|------------------|-------|
| 2026-02-12 | Herramientas (LLM) | ✅ Aprobado | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` کے ذریعے los accesorios regeneran ہوئیں، identificador canónico + listas de alias اور نیا resumen de manifiesto `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` بنا۔ `cargo test -p sorafs_chunker` اور صاف `ci/check_sorafs_fixtures.sh` ejecute سے verificar کیا (los accesorios verifican کے لئے تھیں en etapas). Paso 5 تب تک pendiente ہے جب تک Ayudante de paridad de nodo نہ آ جائے۔ |
| 2026-02-20 | CI de herramientas de almacenamiento | ✅ Aprobado | Sobre del Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` کے ذریعے حاصل ہوا؛ Los accesorios de اسکرپٹ نے regeneran کیے، resumen de manifiesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` confirma کیا، اور Rust arnés دوبارہ چلایا (pasos de Ir/Nodo دستیاب ہوں تو چلتے ہیں) بغیر diferencias کے۔ |

Lista de verificación de herramientas WG چلانے کے بعد تاریخ کے ساتھ قطار شامل کرنی چاہیے۔ اگر
کوئی قدم falla ہو تو یہاں problema vinculado فائل کریں اور remediación تفصیلات شامل
کریں، پھر نئے accesorios یا perfiles aprobados کریں۔