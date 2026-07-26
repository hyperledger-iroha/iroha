---
lang: es
direction: ltr
source: docs/portal/docs/intro.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SORA Nexus Portal de desarrollador میں خوش آمدید

SORA Nexus Portal de desarrolladores Nexus Operadores اور Hyperledger Iroha Contribuyentes کے لئے documentación interactiva, Tutoriales de SDK, اور Referencias de API کو یکجا کرتا ہے۔ یہ sitio de documentos principal کو اس repositorio سے براہ راست especificaciones generadas اور guías prácticas سامنے لا کر مکمل کرتا ہے۔ página de inicio Puntos de entrada Norito/SoraFS temáticos, instantáneas OpenAPI firmadas, Norito dedicada Referencia de transmisión فراہم کرتا ہے تاکہ especificación raíz de los contribuyentes کھنگالے بغیر contrato de plano de control de transmisión تک پہنچ سکیں۔

## آپ یہاں کیا کر سکتے ہیں

- **Norito سیکھیں** - descripción general اور inicio rápido سے آغاز کریں تاکہ modelo de serialización اور herramientas de código de bytes سمجھ سکیں۔
- **SDKs bootstrap کریں** - JavaScript y Rust کے inicios rápidos آج فالو کریں؛ Python, Swift, y Android guían recetas para migrar ہونے کے ساتھ شامل ہوں گے۔
- **Referencias de API ** - Torii OpenAPI página تازہ ترین especificación REST render کرتا ہے، اور tablas de configuración fuentes canónicas de Markdown کی طرف enlace کرتے ہیں۔
- **Implementaciones تیار کریں** - runbooks operativos (telemetría, liquidación, superposiciones Nexus) `docs/source/` سے port ہو رہے ہیں اور migración کے ساتھ اس سائٹ پر آئیں گے۔

## موجودہ حالت- ✅ aterrizaje temático Docusaurus v3 جس میں tipografía actualizada, héroe/tarjetas basadas en gradiente, اور mosaicos de recursos شامل ہیں جو Norito Resumen de transmisión رکھتے ہیں۔
- ✅ Torii OpenAPI complemento کو `npm run sync-openapi` سے cableado کیا گیا ہے، comprobaciones de instantáneas firmadas اور CSP guardias `buildSecurityHeaders` کے ذریعے نافذ ہیں۔
- ✅ Vista previa de la cobertura de la sonda CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) Inicio rápido de SoraFS, listas de verificación de referencia y publicación de artefactos پہلے puerta کرتی ہے۔
- ✅ Norito, SoraFS, inicios rápidos del SDK کے ساتھ barras laterales de secciones de referencia میں live ہیں؛ `docs/source/` Todas las importaciones (streaming, orquestación, runbooks) جب لکھی جاتی ہیں تو یہاں شامل ہوتی ہیں۔

## شمولیت کیسے کریں

- Comandos de desarrollo de لوکل کے لئے `docs/portal/README.md` دیکھیں (`npm install`, `npm run start`, `npm run build`)۔
- Tareas de migración de contenido `DOCS-*` elementos de la hoja de ruta کے ساتھ track کی جاتی ہیں۔ Contribuciones خوش آمدید ہیں - `docs/source/` سے secciones puerto کریں اور página کو barra lateral میں شامل کریں۔
- اگر آپ کوئی artefacto generado (especificaciones, tablas de configuración) شامل کریں تو comando de compilación دستاویز کریں تاکہ آئندہ contribuyentes اسے آسانی سے actualizar کر سکیں۔