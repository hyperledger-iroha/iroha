---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro fragmentador
título: Registro de perfil fragmentador SoraFS
sidebar_label: registro de fragmentos
descripción: registro fragmentador SoraFS con ID de perfil, parámetros y plan de negociación
---

:::nota مستند ماخذ
:::

## Registro de perfil fragmentador SoraFS (SF-2a)

SoraFS Comportamiento de fragmentación de pila کو ایک چھوٹے registro con espacio de nombres کے ذریعے negociar کرتا ہے۔
ہر parámetros CDC deterministas del perfil, metadatos semver اور resumen esperado/asignación multicodec کرتا ہے جو manifiestos اور archivos CAR میں استعمال ہوتا ہے۔

Autores del perfil کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
میں مطلوبہ metadatos, lista de verificación de validación اور plantilla de propuesta دیکھنا چاہیے قبل اس کے کہ وہ نئی entradas enviadas کریں۔
جب gobernanza تبدیلی aprobar کر دے تو
[lista de verificación de implementación del registro](./chunker-registry-rollout-checklist.md)
[libro de jugadas del manifiesto de puesta en escena](./staging-manifest-playbook) کے مطابق accesorios کو puesta en escena اور producción میں promover کریں۔

### Perfiles

| Espacio de nombres | Nombre | SemVer | ID de perfil | Mín. (bytes) | Destino (bytes) | Máx. (bytes) | Romper máscara | Multihash | Alias ​​| Notas |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Accesorios SF-1 میں استعمال ہونے والا perfil canónico |El código de registro میں `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) gobierna کرتا ہے)۔ ہر entrada ایک `ChunkerProfileDescriptor` کے طور پر ظاہر ہوتی ہے جس میں:

* `namespace` – Múltiples perfiles کی agrupación lógica (مثلاً `sorafs`)۔
* `name` – انسان کے لیے etiqueta de perfil legible (`sf1`, `sf1-fast`,…)۔
* `semver` – conjunto de parámetros کے لیے cadena de versión semántica۔
* `profile` – como `ChunkProfile` (min/target/max/mask)۔
* `multihash_code` – resúmenes de fragmentos بناتے وقت استعمال ہونے والا multihash (`0x1f`
  SoraFS predeterminado کے لیے)۔

Manifiesto `ChunkingProfileV1` کے ذریعے perfiles کو serializar کرتا ہے۔ یہ estructura de metadatos del registro
(espacio de nombres, nombre, semver) کو parámetros CDC sin procesar اور اوپر دکھائی گئی lista de alias کے ساتھ registro کرتا ہے۔
Los consumidores usan `profile_id`, buscan en el registro, usan ID desconocidos, tienen parámetros en línea y tienen respaldo, tienen ID desconocidos.

Herramientas de registro سے inspeccionar کرنے کے لیے CLI auxiliar چلائیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

CLI کے وہ تمام flags جو JSON لکھتے ہیں (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) ruta کے طور پر `-` قبول کرتے ہیں، جس سے carga útil stdout پر stream ہوتا ہے بجائے فائل بنانے کے۔
یہ herramientas میں canalización de datos کرنا آسان بناتا ہے جبکہ informe principal کو پرنٹ کرنے والا comportamiento predeterminado برقرار رہتا ہے۔```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```

Perfil de puerta de enlace mutuamente compatible منتخب کرتے ہیں (predeterminado `sorafs.sf1@1.0.0`) اور فیصلہ `Content-Chunker` encabezado de respuesta کے ذریعے refleja کرتے ہیں۔ Manifiestos منتخب perfil incrustado کرتے ہیں تاکہ nodos posteriores Negociación HTTP پر انحصار کیے بغیر diseño de fragmentos validar کر سکیں۔



* **Ruta principal**: CARv2, resumen de carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`، اور perfil de fragmento اوپر کے مطابق registro ہوتا ہے۔


### Conformidad

* `sorafs.sf1@1.0.0` accesorios públicos de perfil (`fixtures/sorafs_chunker`) اور `fuzz/sorafs_chunker` کے تحت registrar corpus سے coincidencia کرتا ہے۔ Paridad de extremo a extremo Rust, Go اور Node میں دیے گئے tests سے ejercicio کی جاتی ہے۔
* `chunker_registry::lookup_by_profile` afirmar کرتا ہے کہ parámetros del descriptor `ChunkProfile::DEFAULT` سے coincide کریں تاکہ divergencia accidental سے بچا جا سکے۔
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` سے بنے manifiesta میں metadatos de registro شامل ہوتی ہے۔