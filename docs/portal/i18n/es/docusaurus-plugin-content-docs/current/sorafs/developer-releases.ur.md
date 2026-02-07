---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Proceso de liberación
resumen: puerta de lanzamiento de CLI/SDK Política de versiones de la política de versiones de CLI/SDK Notas de la versión canónicas
---

# Proceso de liberación

Binarios SoraFS (`sorafs_cli`, `sorafs_fetch`, ayudantes) y cajas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) ایک ساتھ barco ہوتے ہیں۔ Lanzamiento
bibliotecas CLI de canalización کو alineadas رکھتا ہے، pelusa/cobertura de prueba یقینی بناتا ہے،
اور consumidores intermedios کے لیے artefactos capturan کرتا ہے۔ ہر etiqueta de candidato کے لیے
نیچے دی گئی lista de verificación چلائیں۔

## 0. Aprobación de la revisión de seguridad کی تصدیق

Puerta de lanzamiento técnico چلانے سے پہلے تازہ ترین captura de artefactos de revisión de seguridad کریں:

- سب سے تازہ Nota de revisión de seguridad SF-6 ڈاؤن لوڈ کریں ([reports/sf6-security-review](./reports/sf6-security-review.md))
  اور اس کا Boleto de liberación de hash SHA256 میں درج کریں۔
- Enlace del ticket de remediación (مثلاً `governance/tickets/SF6-SR-2026.md`) منسلک کریں اور
  Ingeniería de seguridad اور Grupo de trabajo de herramientas کے aprobadores de aprobación نوٹ کریں۔
- تصدیق کریں کہ memo کی lista de verificación de remediación بند ہے؛ liberación de elementos no resueltos کو bloque کرتے ہیں۔
- Registros de arnés de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) کو
  paquete de manifiesto کے ساتھ subir کرنے کے لیے تیار رہیں۔
- یہ بھی تصدیق کریں کہ comando de firma میں `--identity-token-provider` کے ساتھ
  واضح `--identity-token-audience=<aud>` شامل ہو تاکہ Fulcio alcance evidencia de liberación میں captura ہو۔

Gobernanza کو اطلاع دیتے وقت اور lanzamiento publicar کرتے وقت ان artefactos کو شامل کریں۔## 1. Puerta de liberación/prueba چلائیں

`ci/check_sorafs_cli_release.sh` CLI auxiliar y cajas SDK para formatear, Clippy y pruebas
چلاتا ہے، اور directorio de destino local del espacio de trabajo (`.target`) استعمال کرتا ہے تاکہ CI
contenedores میں conflictos de permisos سے بچا جا سکے۔

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

یہ script درج ذیل afirmaciones کرتا ہے:

- `cargo fmt --all -- --check` (espacio de trabajo)
- `cargo clippy --locked --all-targets` `sorafs_car` کے لیے (característica `cli` کے ساتھ),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` انہی cajas کے لیے

اگر کوئی قدم fallar ہو تو etiquetado سے پہلے regresión درست کریں۔ Versiones de lanzamiento کو principal
کے ساتھ continuo رہنا چاہیے؛ liberar ramas میں corrige la selección de cereza نہ کریں۔ puerta
یہ بھی چیک کرتا ہے کہ banderas de firma sin llave (`--identity-token-issuer`, `--identity-token-audience`)
جہاں ضروری ہوں فراہم کیے گئے ہوں؛ argumentos faltantes ejecutar کو fail کر دیتے ہیں۔

## 2. Política de versiones لاگو کریں

Cajas CLI/SDK SoraFS en SemVer استعمال کرتے ہیں:

- `MAJOR`: پہلی 1.0 versión میں introduce ہوتا ہے۔ 1.0 سے پہلے `0.y` golpe menor
  **cambios importantes** کو ظاہر کرتا ہے، چاہے وہ Superficie CLI میں ہوں یا Norito esquemas میں۔
- `PATCH`: Corrección de errores, versiones solo de documentación, actualizaciones de dependencias y comportamiento observable.

`sorafs_car`, `sorafs_manifest` اور `sorafs_chunker` کو ہمیشہ ایک ہی versión پر رکھیں تاکہ
Consumidores posteriores del SDK ایک cadena de versión alineada پر depende کر سکیں۔ Mejora de versión کرتے وقت:1. ہر caja کے `Cargo.toml` میں `version =` campos اپڈیٹ کریں۔
2. `cargo update -p <crate>@<new-version>` کے ذریعے `Cargo.lock` regenera کریں (las versiones explícitas del espacio de trabajo aplican کرتا ہے)۔
3. Puerta de liberación دوبارہ چلائیں تاکہ artefactos obsoletos باقی نہ رہیں۔

## 3. Notas de la versión تیار کریں

ہر lanzamiento کو registro de cambios de rebajas شائع کرنا چاہیے جو CLI، SDK اور cambios que impactan la gobernanza
کو resaltar کرے۔ `docs/examples/sorafs_release_notes.md` Plantilla کا استعمال کریں (versión اسے
directorio de artefactos میں کاپی کریں اور secciones کو detalles concretos سے بھر دیں)۔

Contenido mínimo:

- **Aspectos destacados**: CLI y SDK para consumidores incluyen titulares de funciones
- **Pasos de actualización**: las dependencias de carga aumentan y los accesorios deterministas vuelven a ejecutar los comandos TL;DR۔
- **Verificación**: hashes de salida del comando یا sobres اور `ci/check_sorafs_cli_release.sh` کی revisión exacta جو ejecutar ہوئی۔

بھری ہوئی notas de la versión کو etiqueta کے ساتھ adjuntar کریں (مثلاً Cuerpo de la versión de GitHub) اور انہیں deterministamente
artefactos generados کے ساتھ محفوظ کریں۔

## 4. Suelte los ganchos چلائیں

`scripts/release_sorafs_cli.sh` Paquete de firmas y resumen de verificación generado y lanzamiento
کے ساتھ barco ہوتے ہیں۔ Wrapper instalado en la compilación CLI instalado en `sorafs_cli manifest sign` instalado en el servidor
`manifest verify-signature` repetición کرتا ہے تاکہ etiquetado سے پہلے fallas سامنے آ جائیں۔ Nombre:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Consejos:- Entradas de lanzamiento (carga útil, planes, resúmenes, hash de token esperado) کو repositorio یا configuración de implementación میں track کریں تاکہ script reproducible رہے۔ `fixtures/sorafs_manifest/ci_sample/` y diseño canónico del paquete de accesorios دکھاتا ہے۔
- Automatización CI کو `.github/workflows/sorafs-cli-release.yml` پر base کریں؛ یہ puerta de liberación چلاتا ہے، اوپر والا script چلاتا ہے، اور paquetes/firmas کو artefactos de flujo de trabajo کے طور پر archivo کرتا ہے۔ دوسرے Sistemas CI میں بھی وہی orden de comando (liberar puerta -> firmar -> verificar) رکھیں تاکہ registros de auditoría hashes generados کے ساتھ alinear رہیں۔
- `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`, y `manifest.verify.summary.json` کو اکٹھا رکھیں؛ یہی notificación de gobernanza de paquetes میں consulte ہوتا ہے۔
- Lanzamiento de accesorios canónicos, manifiesto actualizado, plan de fragmentos, resúmenes y `fixtures/sorafs_manifest/ci_sample/`, actualización de manifiesto (`docs/examples/sorafs_ci_sample/manifest.template.json`). کریں) etiquetado سے پہلے۔ Los operadores intermedios han comprometido accesorios پر dependen کرتے ہیں تاکہ paquete de lanzamiento reproducen کر سکیں۔
- `sorafs_cli proof stream` verificación de canal delimitado کا ejecutar captura de registro کریں اور liberar paquete کے ساتھ adjuntar کریں تاکہ pruebas de seguridad de transmisión فعال رہنے کا ثبوت ملے۔
- Firma de notas de la versión `--identity-token-audience` exactas میں درج کریں؛ Gobernanza Política de Fulcio کے خلاف audiencia کو verificación cruzada کرتی ہے۔

جب lanzamiento میں implementación de puerta de enlace بھی شامل ہو تو `scripts/sorafs_gateway_self_cert.sh` استعمال کریں۔ اسی paquete de manifiesto کی طرف punto کریں تاکہ artefacto candidato de atestación سے coincidencia ہو:```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetar y publicar

Comprobaciones پاس ہونے اور ganchos مکمل ہونے کے بعد:

1. `sorafs_cli --version` اور `sorafs_fetch --version` چلائیں تاکہ binarios نئی informe de versión کریں۔
2. Configuración de versión registrada `sorafs_release.toml` (preferido) یا کسی اور archivo de configuración میں تیار کریں جو آپ کے repositorio de implementación میں track ہو۔ Variables de entorno ad-hoc پر انحصار نہ کریں؛ CLI کو `--config` (یا equivalente) کے ذریعے rutas دیں تاکہ entradas de liberación واضح اور رہیں۔
3. Etiqueta firmada (preferida) یا etiqueta anotada بنائیں:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artefactos (paquetes CAR, manifiestos, resúmenes de pruebas, notas de la versión, resultados de certificación) کو registro de proyectos میں cargar کریں اور lista de verificación de gobernanza (guía de implementación: [guía de implementación](./developer-deployment.md)) siga کریں۔ اگر lanzamiento نے نئی accesorios بنائیں تو انہیں repositorio de accesorios compartidos یا almacén de objetos میں push کریں تاکہ auditoría automatización paquete publicado کا control de fuente کے ساتھ diff کر سکے۔
5. Canal de gobernanza کو etiqueta firmada, notas de la versión, paquete de manifiesto/hashes de firma, resúmenes `manifest.sign/verify` archivados, اور sobres de certificación کے enlaces کے ساتھ مطلع کریں۔ URL del trabajo de CI (archivo de registro) شامل کریں جس نے `ci/check_sorafs_cli_release.sh` اور `scripts/release_sorafs_cli.sh` چلایا۔ Boleto de gobernanza اپڈیٹ کریں تاکہ aprobaciones de auditores کو artefactos سے rastreo کر سکیں؛ جب `.github/workflows/sorafs-cli-release.yml` publicación de notificaciones de trabajo کرے تو resúmenes ad-hoc کے بجائے enlace de hashes registrados کریں۔

## 6. Seguimiento posterior al lanzamiento- Versión actual کی طرف اشارہ کرنے والی documentación (inicios rápidos, plantillas CI) اپڈیٹ کریں یا تصدیق کریں کہ کوئی تبدیلی درکار نہیں۔
- Liberar registros de salida de puerta کو auditores کے لیے archivo کریں - انہیں artefactos firmados کے ساتھ محفوظ رکھیں۔

اس pipeline کی پیروی ہر ciclo de lanzamiento میں CLI، SDK cajas اور gobernanza colateral کو lock-step میں رکھتی ہے۔