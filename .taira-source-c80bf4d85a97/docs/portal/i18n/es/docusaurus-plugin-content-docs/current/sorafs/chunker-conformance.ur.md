---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: Guía de conformidad del fragmentador SoraFS
sidebar_label: conformidad del fragmento
descripción: Accesorios, SDK, perfil determinista del fragmentador SF1, requisitos y flujos de trabajo
---

:::nota مستند ماخذ
:::

یہ flujo de trabajo de regeneración, política de firma, اور pasos de verificación بھی documento کرتی ہے تاکہ SDK میں sincronización de consumidores de dispositivos میں رہیں۔

## Perfil canónico

- Semilla de entrada (hex): `0000000000dec0ded`
- Tamaño objetivo: 262144 bytes (256 KiB)
- Tamaño mínimo: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polinomio rodante: `0x3DA3358B4DC173`
- Semilla de mesa de engranajes: `sorafs-v1-gear`
- Máscara de rotura: `0x0000FFFF`

Implementación de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
کسی بھی Aceleración SIMD کو یکساں límites اور resúmenes پیدا کرنے چاہئیں۔

## Paquete de accesorios

`cargo run --locked -p sorafs_chunker --bin export_vectors` accesorios کو regenerar
کرتا ہے اور درج ذیل فائلیں `fixtures/sorafs_chunker/` میں بناتا ہے:- `sf1_profile_v1.{json,rs,ts,go}`: Rust, TypeScript y los consumidores de Go tienen límites de fragmentos canónicos. ہر فائل
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`, پھر `sorafs.sf1@1.0.0`)۔ یہ ordenar
  `ensure_charter_compliance` کے ذریعے enforce ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — Manifiesto verificado por BLAKE3 جو ہر accesorio فائل کو cubierta کرتا ہے۔
- `manifest_signatures.json` — resumen de manifiesto con firmas del consejo (Ed25519) ۔
- `sf1_profile_v1_backpressure.json` o `fuzz/` en corpus sin procesar —
  escenarios de transmisión deterministas y pruebas de contrapresión de fragmentación میں استعمال ہوتے ہیں۔

### Política de firma

Regeneración de accesorios **لازم** طور پر firma válida del consejo شامل کرے۔ salida sin signo del generador کو rechazar کرتا ہے جب تک `--allow-unsigned` واضح طور پر نہ دیا جائے (صرف مقامی تجربات کے لیے)۔ Sobres de firma de solo anexar ہوتے ہیں اور firmante کے لحاظ سے deduplicar ہوتے ہیں۔

Firma del consejo شامل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificación

Asistente de CI `ci/check_sorafs_fixtures.sh` generador کو `--locked` کے ساتھ دوبارہ چلاتا ہے۔
اگر accesorios میں deriva ہو یا firmas faltantes ہوں تو trabajo fallido ہو جاتا ہے۔ اس guión کو
flujos de trabajo nocturnos میں اور cambios de accesorios enviar کرنے سے پہلے استعمال کریں۔

Pasos de verificación manual:

1. `cargo test -p sorafs_chunker` چلائیں۔
2. `ci/check_sorafs_fixtures.sh` لوکل چلائیں۔
3. تصدیق کریں کہ `git status -- fixtures/sorafs_chunker` صاف ہے۔

## Actualizar el libro de jugadas

Este perfil de fragmentador propone کرتے وقت یا SF1 اپڈیٹ کرتے وقت:یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
requisitos de metadatos, plantillas de propuestas y listas de verificación de validación

1. نئے parámetros کے ساتھ `ChunkProfileUpgradeProposalV1` (RFC SF-1 دیکھیں) تیار کریں۔
2. `export_vectors` Los dispositivos کے ذریعے regeneran el resumen del manifiesto کریں اور نیا ریکارڈ کریں۔
3. مطلوبہ quórum del consejo کے ساتھ signo manifiesto کریں۔ firmas de تمام
   `manifest_signatures.json` میں agregar ہونی چاہئیں۔
4. متاثرہ Accesorios SDK (Rust/Go/TS) اپڈیٹ کریں اور paridad entre tiempos de ejecución یقینی بنائیں۔
5. Los parámetros de los corpus fuzz se regeneran
6. اس گائیڈ میں نیا identificador de perfil, semillas, اور digest اپڈیٹ کریں۔
7. تبدیلی کو اپڈیٹڈ pruebas اور actualizaciones de la hoja de ruta کے ساتھ enviar کریں۔

اگر اس عمل کے بغیر límites de fragmentos یا resúmenes تبدیل کیے جائیں تو وہ invalid ہیں اور merge نہیں ہونے چاہئیں۔