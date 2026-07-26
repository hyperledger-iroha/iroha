---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: Sora Nexus espacio de datos آپریٹر آن بورڈنگ
descripción: `docs/source/sora_nexus_operator_onboarding.md` کا آئینہ، جو Nexus آپریٹرز کے لئے ریلیز چیک لسٹ کو ٹریک کرتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/sora_nexus_operator_onboarding.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Incorporación del operador del espacio de datos

یہ گائیڈ de extremo a extremo فلو کو محفوظ کرتی ہے جس پر Sora Nexus data-space آپریٹرز کو ریلیز کے اعلان کے بعد عمل کرنا ہوتا ہے۔ Runbook de doble pista (`docs/source/release_dual_track_runbook.md`) y nota de selección de artefactos (`docs/source/release_artifact_selection.md`) لانے سے پہلے ڈاؤن لوڈ شدہ paquetes/imágenes, manifiestos اور plantillas de configuración کو عالمی expectativas de carril کے ساتھ کیسے ہم آہنگ کرنا ہے۔## سامعین اور پیشگی شرائط
- El programa Nexus incluye la asignación de espacio de datos (índice de carril, ID/alias de espacio de datos, requisitos de política de enrutamiento).
- آپ Ingeniería de lanzamiento کی شائع کردہ artefactos de lanzamiento firmados تک رسائی رکھتے ہیں (tarballs, imágenes, manifiestos, firmas, claves públicas).
- آپ نے اپنے validador/observador رول کے لئے پروڈکشن material clave تیار یا حاصل کیا ہے (identidad de nodo Ed25519, validadores کے لئے clave de consenso BLS + PoP؛ اور کوئی بھی alternancia de funciones confidenciales).
- آپ ان موجودہ Sora Nexus peers تک رسائی کر سکتے ہیں جو آپ کے نوڈ کا bootstrap کریں گے۔

## مرحلہ 1 - ریلیز پروفائل کی تصدیق
1. وہ alias de red یا ID de cadena شناخت کریں جو آپ کو دیا گیا ہے۔
2. اس ریپوزٹری کے pago پر `scripts/select_release_profile.py --network <alias>` (یا `--chain-id <id>`) چلائیں۔ ayudante `release/network_profiles.toml` دیکھ کر implementar ہونے والا پروفائل پرنٹ کرتا ہے۔ Sora Nexus کے لئے جواب `iroha3` ہونا چاہئے۔ کسی بھی دوسرے ویلیو پر رک جائیں اور Release Engineering سے رابطہ کریں۔
3. ریلیز اعلان میں دیا گیا etiqueta de versión نوٹ کریں (مثلاً `iroha3-v3.2.0`); اسی سے آپ artefactos اور manifiestos حاصل کریں گے۔## مرحلہ 2 - artefactos حاصل کریں اور ویریفائی کریں
1. Paquete `iroha3` (`<profile>-<version>-<os>.tar.zst`) y archivos complementarios ڈاؤن لوڈ کریں (`.sha256`, اختیاری `.sig/.pub`, `<profile>-<version>-manifest.json`, اور `<profile>-<version>-image.json` اگر آپ contenedores ڈپلائے کر رہے ہیں)۔
2. ان پیک کرنے سے پہلے integridad چیک کریں:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   اگر آپ KMS respaldado por hardware استعمال کرتے ہیں تو `openssl` کو ادارہ منظور شدہ verifier سے بدل دیں۔
3. tarball con formato `PROFILE.toml` y manifiestos JSON con formato:
   - `profile = "iroha3"`
   - `version`, `commit`, y `built_at` فیلڈز ریلیز اعلان سے ملتے ہیں۔
   - SO/arquitectura آپ کے objetivo de implementación سے coincidencia کرتی ہے۔
4. اگر آپ imagen del contenedor کرتے ہیں تو `<profile>-<version>-<os>-image.tar` کے لئے hash/firma دوبارہ verificar کریں اور `<profile>-<version>-image.json` میں درج image ID کنفرم کریں۔## مرحلہ 3 - plantillas سے configuración تیار کریں
1. extracto del paquete کریں اور `config/` کو اس جگہ کاپی کریں جہاں نوڈ اپنی configuración پڑھے گا۔
2. `config/` کے تحت فائلوں کو plantillas سمجھیں:
   - `public_key`/`private_key` کو اپنے پروڈکشن Ed25519 teclas سے بدلیں۔ اگر نوڈ claves HSM سے لے گا تو claves privadas کو disco سے ہٹا دیں؛ config کو Conector HSM کی طرف پوائنٹ کریں۔
   - `trusted_peers`, `network.address` o `torii.address` para interfaces de pares bootstrap y para pares de arranque. کریں۔
   - `client.toml` Punto final Torii orientado al operador (configuración TLS سمیت، اگر لاگو ہو) اور آپ کی aprovisionamiento کردہ credenciales کے ساتھ اپ ڈیٹ کریں۔
3. paquete میں فراہم کردہ ID de cadena برقرار رکھیں، الا یہ کہ Gobernanza واضح طور پر ہدایت دے - carril global ایک واحد identificador de cadena canónica چاہتا ہے۔
4. نوڈ کو Sora پروفائل فلیگ کے ساتھ اسٹارٹ کرنے کا ارادہ رکھیں: `irohad --sora --config <path>`. اگر فلیگ نہ ہو تو cargador de configuración SoraFS یا multicarril سیٹنگز کو rechazar کر دے گا۔## مرحلہ 4 - metadatos del espacio de datos اور enrutamiento ہم آہنگ کریں
1. `config/config.toml` ایڈٹ کریں تاکہ `[nexus]` سیکشن Nexus Consejo کے فراہم کردہ catálogo de espacio de datos سے coincidencia کرے:
   - `lane_count` موجودہ epoch میں فعال lanes کی مجموعی تعداد کے برابر ہونا چاہئے۔
   - `[[nexus.lane_catalog]]` اور `[[nexus.dataspace_catalog]]` کی ہر انٹری میں منفرد `index`/`id` اور متفقہ alias ہونے چاہئیں۔ موجودہ entradas globales نہ ہٹائیں؛ اگر consejo نے اضافی espacios de datos دیئے ہیں تو اپنے alias delegados شامل کریں۔
   - Espacio de datos انٹری میں `fault_tolerance (f)` شامل ہونا یقینی بنائیں؛ comités de relevo de carril کا سائز `3f+1` ہوتا ہے۔
2. `[[nexus.routing_policy.rules]]` کو اپنی دی گئی پالیسی کے مطابق اپ ڈیٹ کریں۔ instrucciones de gobierno de plantilla predeterminadas carril `1` implementaciones de contrato carril `2` ruta ruta قواعد شامل یا تبدیل کریں تاکہ آپ کے espacio de datos کی ٹریفک درست carril اور alias پر جائے۔ قواعد کی ترتیب بدلنے سے پہلے Ingeniería de lanzamiento کے ساتھ ہم آہنگی کریں۔
3. Umbrales `[nexus.da]`, `[nexus.da.audit]`, y `[nexus.da.recovery]` ریویو کریں۔ آپریٹرز سے توقع ہے کہ وہ aprobado por el consejo ویلیوز رکھیں؛ صرف اسی وقت بدلیں جب نئی پالیسی منظور ہو۔
4. Configuración del rastreador de operaciones کو اپنے میں ریکارڈ کریں۔ ticket de incorporación del runbook de versión de doble vía کے ساتھ موثر `config.toml` (secretos redactados) منسلک کرنے کا تقاضا کرتا ہے۔## مرحلہ 5 - پری فلائٹ ویلیڈیشن
1. نیٹ ورک میں شامل ہونے سے پہلے validador de configuración incorporado چلائیں:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   یہ configuración resuelta پرنٹ کرتا ہے اور اگر entradas de catálogo/enrutamiento میں تضاد ہو یا genesis اور config نہ ملیں تو جلدی fail ہو جاتا ہے۔
2. Los contenedores implementan کرتے ہیں تو `docker load -i <profile>-<version>-<os>-image.tar` کے بعد وہی کمانڈ image کے اندر چلائیں ( `--sora` شامل کرنا نہ بھولیں)۔
3. registros میں marcador de posición identificadores de carril/espacio de datos کے advertencias دیکھیں۔ اگر ملیں تو مرحلہ 4 پر واپس جائیں - پروڈکشن implementaciones کو plantillas کے ID de marcador de posición پر انحصار نہیں کرنا چاہئے۔
4. اپنا procedimiento de humo local چلائیں (مثلاً `iroha_cli` سے `FindNetworkStatus` consulta بھیجیں، تصدیق کریں کہ puntos finales de telemetría `nexus_lane_state_total` expone کرتے ہیں، اور claves de transmisión کی rotación/importación کی تصدیق کریں)۔## مرحلہ 6 - Transición y transferencia
1. تصدیق شدہ `manifest.json` اور artefactos de firma کو boleto de liberación میں محفوظ کریں تاکہ auditores آپ کی controles دوبارہ کر سکیں۔
2. Operaciones Nexus کو اطلاع دیں کہ نوڈ متعارف کرنے کے لئے تیار ہے؛ شامل کریں:
   - Identidad del nodo (ID del par, nombres de host, punto final Torii).
   - Catálogo de carriles/espacios de datos y política de enrutamiento.
   - Binarios/imágenes verificados کے hashes۔
3. حتمی admisión de pares (semillas de chismes اور asignación de carril) کو `@nexus-core` کے ساتھ کوآرڈینیٹ کریں۔ منظوری ملنے سے پہلے نیٹ ورک unirse a نہ کریں؛ Sora Nexus ocupación de carril determinista نافذ کرتا ہے اور manifiesto de admisiones actualizado چاہتا ہے۔
4. Live ہونے کے بعد اپنے runbooks میں کی گئی anula اپ ڈیٹ کریں اور etiqueta de lanzamiento نوٹ کریں تاکہ اگلی iteración اسی línea de base سے شروع ہو۔

## ریفرنس چیک لسٹ
- [] Perfil de versión `iroha3` کے طور پر validar ہو چکا ہے۔
- [] Paquete/imagen کے hashes اور firmas verificar ہو چکے ہیں۔
- [] Claves, direcciones de pares اور Torii puntos finales پروڈکشن ویلیوز پر اپ ڈیٹ ہیں۔
- [] Nexus Catálogo de carril/espacio de datos اور asignación del consejo de políticas de enrutamiento سے coincidencia کرتی ہے۔
- [] Validador de configuración (`irohad --sora --config ... --trace-config`) بغیر advertencias کے پاس کرتا ہے۔
- [ ] Manifiestos/firmas del billete de embarque میں آرکائیو اور Ops کو اطلاع دے دی گئی ہے۔Nexus fases de migración اور expectativas de telemetría کے وسیع تر سیاق کے لئے [notas de transición Nexus](./nexus-transition-notes) دیکھیں۔