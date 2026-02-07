---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: Guía de creación de perfiles fragmentados SoraFS
sidebar_label: guía de creación de fragmentos
descripción: SoraFS Perfiles fragmentados y accesorios تجویز کرنے کے لیے checklist۔
---

:::nota مستند ماخذ
:::

# SoraFS Guía de creación de perfiles fragmentados

یہ گائیڈ وضاحت کرتی ہے کہ SoraFS کے لیے نئے perfiles fragmentados کیسے تجویز اور publicar کیے جائیں۔
یہ arquitectura RFC (SF-1) اور referencia de registro (SF-2a) کو
requisitos de creación concretos, pasos de validación y plantillas de propuesta
Canonical مثال کے لیے دیکھیں
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
اور متعلقہ registro de funcionamiento en seco
`docs/source/sorafs/reports/sf1_determinism.md` Mیں۔

## Descripción general

ہر perfil جو registro میں داخل ہوتی ہے اسے یہ کرنا ہوگا:

- parámetros CDC deterministas اور configuración multihash کو arquitecturas کے درمیان یکساں طور پر publicidad کرنا؛
- dispositivos rejugables (Rust/Go/TS JSON + fuzz corpora + testigos PoR) SDK descendentes de SDK de bajada Herramientas personalizadas Verificar کر سکیں؛
- revisión del consejo سے پہلے conjunto de diferencias deterministas پاس کرنا۔

نیچے دی گئی lista de verificación پر عمل کریں تاکہ ایک ایسا propuesta تیار ہو جو ان قواعد پر پورا اترے۔

## Instantánea de la carta del registro

Borrador de propuesta کرنے سے پہلے تصدیق کریں کہ یہ carta de registro کے مطابق ہے جسے
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` aplicar کرتا ہے:- ID de perfil مثبت números enteros ہوتے ہیں جو بغیر espacios کے monótonos طور پر بڑھتے ہیں۔
- کوئی alias کسی دوسرے mango canónico سے colisionar نہیں کر سکتا اور ایک سے زیادہ بار ظاہر نہیں ہو سکتا۔
- Alias ​​no vacíos ہوں اور espacios en blanco سے recortar ہوں۔

Prácticos ayudantes de CLI:

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

یہ propuestas de comandos کو carta de registro کے مطابق رکھتے ہیں اور discusiones de gobernanza کے لیے درکار metadatos canónicos فراہم کرتے ہیں۔

## Metadatos requeridos| Campo | Descripción | Ejemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | متعلقہ perfiles کے لیے agrupación lógica۔ | `sorafs` |
| `name` | etiqueta legible por humanos۔ | `sf1` |
| `semver` | conjunto de parámetros کے لیے cadena de versión semántica۔ | `1.0.0` |
| `profile_id` | Identificador numérico monótono جو perfil کے tierra ہونے پر asignar ہوتا ہے۔ اگلا reserva de identificación کریں مگر موجودہ نمبرز reutilización نہ کریں۔ | `1` |
| `profile.min_size` | longitud del fragmento کی mínimo حد bytes میں۔ | `65536` |
| `profile.target_size` | longitud del fragmento کی destino حد bytes میں۔ | `262144` |
| `profile.max_size` | longitud del fragmento کی máximo حد bytes میں۔ | `524288` |
| `profile.break_mask` | hash rodante کے لیے máscara adaptativa (hexadecimal)۔ | `0x0000ffff` |
| `profile.polynomial` | constante polinómica del engranaje (hexadecimal) ۔ | `0x3da3358b4dc173` |
| `gear_seed` | Tabla de engranajes de 64 KiB deriva کرنے کے لیے semilla۔ | `sorafs-v1-gear` |
| `chunk_multihash.code` | resúmenes por fragmento کے لیے código multihash۔ | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | paquete de accesorios canónicos کا digest۔ | `13fa...c482` |
| `fixtures_root` | accesorios regenerados رکھنے والی directorio relativo۔ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | muestreo determinista PoR کے لیے semilla (`splitmix64`)۔ | `0xfeedbeefcafebabe` (ejemplo) |Metadatos, documento de propuesta, accesorios generados, herramientas CLI, automatización de gobernanza, referencias cruzadas manuales, confirmación de valores. اگر شک ہو تو chunk-store اور manifest CLIs کو `--json-out=-` کے ساتھ چلائیں تاکہ notas de revisión de metadatos computados میں flujo ہو سکے۔

### CLI y puntos de contacto del registro

- `sorafs_manifest_chunk_store --profile=<handle>`: parámetros propuestos, metadatos de fragmentos, resumen de manifiesto y comprobaciones de PoR دوبارہ چلائیں۔
- `sorafs_manifest_chunk_store --json-out=-` — informe de almacén de fragmentos, salida estándar, flujo, comparaciones automáticas y datos
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmar کریں کہ manifiestos اور CAR planes canonical handle اور alias incrustar کرتے ہیں۔
- `sorafs_manifest_stub --plan=-` — پچھلا `chunk_fetch_specs` واپس feed کریں تاکہ cambiar کے بعد compensaciones/resúmenes verificar ہوں۔

Salida del comando (resúmenes, raíces PoR, hashes de manifiesto)

## Lista de verificación de determinismo y validación1. **Los accesorios se regeneran کریں**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Parity suite چلائیں** — `cargo test -p sorafs_chunker` اور arnés de diferenciación entre idiomas (`crates/sorafs_chunker/tests/vectors.rs`) نئے accesorios کے ساتھ verde ہونا چاہیے۔
3. **Reproducción de corpus de fuzz/contrapresión کریں** — `cargo fuzz list` y arnés de transmisión (`fuzz/sorafs_chunker`) کو activos regenerados کے خلاف چلائیں۔
4. **Los testigos de prueba de recuperabilidad verifican کریں** — `sorafs_manifest_chunk_store --por-sample=<n>` perfil propuesto کے ساتھ چلائیں اور raíces کو manifiesto de instalación سے coincidencia کریں۔
5. **Funcionamiento en seco de CI** — `ci/check_sorafs_fixtures.sh` لوکل چلائیں؛ script کو نئے accesorios اور موجودہ `manifest_signatures.json` کے ساتھ tener éxito ہونا چاہیے۔
6. **Confirmación entre tiempos de ejecución**: los enlaces Go/TS regenerados JSON consumen límites de fragmentos idénticos y los resúmenes emiten

Comandos اور resúmenes resultantes کو propuesta میں دستاویزی کریں تاکہ Herramientas WG بغیر conjeturas کے انہیں دوبارہ چلا سکے۔

### Manifiesto / confirmación de PoR

Los accesorios regeneran la canalización de manifiesto de la canalización de manifiesto de los metadatos de CAR y las pruebas de PoR consistentes:

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Archivo de entrada کو اپنے aparatos میں استعمال ہونے والے کسی corpus representativo سے بدلیں
(Transmisión determinista de 1 GiB) اور resúmenes resultantes کو propuesta کے ساتھ adjuntar کریں۔

## Plantilla de propuesta

Propuestas کو `ChunkerProfileProposalV1` Norito registros کے طور پر `docs/source/sorafs/proposals/` میں check in کیا جاتا ہے۔ Se espera una plantilla JSON nueva (los valores de اپنی reemplazan کریں):Informe de Markdown coincidente (`determinism_report`) فراہم کریں جس میں salida de comando, resúmenes de fragmentos اور validación کے دوران پائی گئی desviaciones شامل ہوں۔

## Flujo de trabajo de gobernanza

1. **Propuesta + accesorios کے ساتھ PR enviar کریں۔** Activos generados, Norito propuesta, اور `chunker_registry_data.rs` actualizaciones شامل کریں۔
2. **Revisión del GT de herramientas۔** Lista de verificación de validación de los revisores دوبارہ چلاتے ہیں اور confirmar کرتے ہیں کہ reglas de registro de propuestas کے مطابق ہے (reutilización de identificación نہیں، determinismo satisfecho)۔
3. **Sobre del consejo ۔** Aprobar el resumen de propuesta de ہونے کے بعد miembros del consejo (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) پر firmar کرتے ہیں اور firmas کو sobre de perfil میں agregar کرتے ہیں جو accesorios کے ساتھ رکھا جاتا ہے۔
4. **Publicar registro۔** Fusionar registro, documentos y actualización de accesorios ہوتے ہیں۔ CLI predeterminado پچھلے perfil پر رہتا ہے جب تک migración de gobernanza کو listo قرار نہ دے۔

## Consejos de autoría

- Potencia de dos کی límites pares ترجیح دیں تاکہ comportamiento de fragmentación de casos extremos کم ہو۔
- Semillas de la tabla de engranajes کو legibles por humanos مگر globalmente únicas رکھیں تاکہ pistas de auditoría آسان ہوں۔
- Artefactos de evaluación comparativa (comparaciones de rendimiento) کو `docs/source/sorafs/reports/` میں محفوظ کریں تاکہ مستقبل میں reference ہو سکے۔

Lanzamiento کے دوران expectativas operativas کے لیے libro mayor de migración دیکھیں
(`docs/source/sorafs/migration_ledger.md`) ۔ Reglas de conformidad en tiempo de ejecución کے لیے
`docs/source/sorafs/chunker_conformance.md` دیکھیں۔