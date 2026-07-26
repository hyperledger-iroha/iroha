---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operator-onboarding
כותרת: Incorporacion de operadores de data-space de Sora Nexus
תיאור: Espejo de `docs/source/sora_nexus_operator_onboarding.md`, ראה רשימת בדיקה מקצה לקצה עבור מפעילי Nexus.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/sora_nexus_operator_onboarding.md`. Manten ambas copias alineadas hasta que las ediciones localizadas lleguen al portal.
:::

# Incorporacion de operadores de data-space de Sora Nexus

Esta guia captura el flujo מקצה לקצה que deben seguir los operations the data-space de Sora Nexus וזו הודעה על שחרור. משלים ל-Runbook de Doble באמצעות (`docs/source/release_dual_track_runbook.md`) ו-La Not de Seleccion de Artefactos (`docs/source/release_artifact_selection.md`) אל מתאר כמו חבילות לינאריות/תמונות הורדות, ביטויים y plantillas de configuracion con las expectativas and no de las expectativas.

## דרישות מוקדמות לצופים
- האם יש סיפונה ל-Programma Nexus y recibiste tu assignacion de data-space (index de lane, data-space ID/כינוי y requisitos de politica de routing).
- Puedes acceder a los artefactos firmados del release publicados por Release Engineering (tarballs, imagenes, manifests, firmas, llaves publicas).
- יש לו חומרי ייצור ליצירת חומרי אימות/מתבוננים (identidad de nodo Ed25519; llave de consenso BLS + PoP para validators; mas cualquier toggle de funciones confidenciales).
- Puedes alcanzar and los peers existentes de Sora Nexus que bootstrapean tu nodo.

## פסו 1 - אישור הפרסום
1. מזהה כינוי דה אדום או מזהה שרשרת que te dieron.
2. Ejecuta `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en un checkout de este repositorio. El helper consulta `release/network_profiles.toml` e imprime el perfil a desplegar. Para Sora Nexus la respuesta debe ser `iroha3`. עבור חילופי דברים אחרים, צור קשר עם הנדסת שחרור.
3. Anota el tag de version que referencio el anuncio del release (por ejemplo `iroha3-v3.2.0`); lo usaras para descargar artefactos y manifests.

## פסו 2 - Recuperar y validar artefactos
1. הורד את החבילה `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos companeros (`.sha256`, אופציונלי `.sig/.pub`, Nexus, 18NI000000303X, 1400000030Ni מתמודדים).
2. Valida la integridad antes de descomprimir:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Reemplaza `openssl` con el verificador aprobado por la organizacion si usas un KMS con respaldo de hardware.
3. Inspecciona `PROFILE.toml` dentro del tarball y los manifests JSON לאישור:
   - `profile = "iroha3"`
   - Los campos `version`, `commit` y `built_at` בצירוף מקרים עם הודעת שחרור.
   - El OS/arquitectura coinciden con tu objetivo despliegue.
4. Si usas la imagen de contenedor, repite la verificacion de hash/firma para `<profile>-<version>-<os>-image.tar` y confirma el image ID registrado en `<profile>-<version>-image.json`.## פסו 3 - הכן את התצורה של הצמחים
1. Extrae el bundle y copia `config/` a la ubicacion donde el nodo leera su configuracion.
2. Trata los archivos bajo `config/` como plantillas:
   - Reemplaza `public_key`/`private_key` מתמשכת עד 25519 דה produccion. Elimina llaves privadas del disco si el nodo las obtiene de un HSM; קונפיגורציה בפועל עבור חיבור ל-HSM.
   - Ajusta `trusted_peers`, `network.address` y `torii.address` עבור ממשקים נגישים ובעלי עמיתים ל-bootstrap assignados.
   - אקטואליזה `client.toml` עם נקודת קצה Torii של המפעיל (כולל תצורת TLS באפליקציה) ואישורים עבור הפעלת כלי עבודה.
3. תעודת זהות בשרשרת ניתנת ל-Bundle a menos que Governance לא מפורש: el Lane Global Espera un identificador de cadena canonico unico.
4. Planea iniciar el nodo con el flag de perfil Sora: `irohad --sora --config <path>`. ה-loader de configuracion rechazara justes de SoraFS או ריבוי נתיבים או הדגל esta ausente.

## פסו 4 - מטא נתונים ליניאריים של ניתוב נתונים-מרחב
1. Edita `config/config.toml` para que la seccion `[nexus]` coincida con el catalogo de data-spaces proporcionado por el Nexus Council:
   - `lane_count` debe igualar el total de lanes habilitados en la epoca actual.
   - Cada entrada en `[[nexus.lane_catalog]]` y `[[nexus.dataspace_catalog]]` debe contener un `index`/`id` unico y los alias acordados. No elimines las entradas globales existentes; agrega tus alias delegados si el consejo asigno data-spaces addictionales.
   - Asegura que cada entrada de dataspace incluya `fault_tolerance (f)`; ממסר נתיב לוס comites ב-`3f+1`.
2. Actualiza `[[nexus.routing_policy.rules]]` para capturar la politica que te asignaron. La plantilla por defecto enruta instrucciones de gobernanza al lane `1` y despliegues de contratos al lane `2`; agrega o modifica regglas para que el trafico destinado a tu data-space vaya al lane y alias correctos. קואורדינה עם הנדסת שחרור לפני המלחמה.
3. Revisa los umbrales de `[nexus.da]`, `[nexus.da.audit]` y `[nexus.da.recovery]`. Se espera que los operadores mantengan los valores aprobados por el consejo; ajustalos solo si se ratifico una politica actualizada.
4. Registra la Configuracion Final en tu Tracker de Operaciones. ספר ריצה לשחרור כפול דרך דרישת תוספת ל-`config.toml` יעילה (con secretos redactados) לכרטיס כניסה למטוס.## פסו 5 - Validacion previa
1. Ejecuta el validador de configuracion integrado antes de unirte a la red:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Esto imprime la configuracion resuelta y falla temprano si las entradas de catalogo/routing son inconsistentes o si genesis y config לא מקרי.
2. Si despliegas contenedores, ejecuta el mismo comando dentro de la imagen despues de cargarla con `docker load -i <profile>-<version>-<os>-image.tar` (recuerda incluir `--sora`).
3. Revisa logs para advertencias sobre identificadores placeholder de lane/data-space. Si aparecen, regresa al Paso 4: los despliegues de produccion no deben depender de los IDs placeholder que vienen con las plantillas.
4. Ejecuta tu procedimiento local de smoke (עמ' ej., enviar una consulta `FindNetworkStatus` con `iroha_cli`, confirmar que los endpoints de telemetria exponen `nexus_lane_state_total` y verificar de que rotilla correspondar o importing se rotlla).

## פאסו 6 - ניתוק y מסירה
1. Guarda el `manifest.json` verificado y los artefactos de firma en el ticket de release para que los auditores puedan reproducir tus verificaciones.
2. הודעה על Nexus פעולות que el nodo esta listo para ser introducido; כולל:
   - Identidad del nodo (מזהה עמיתים, שמות מארח, נקודת קצה Torii).
   - השפעות הקטלוג של נתיב/מרחב נתונים ופוליטיקה של ניתוב.
   - Hashes de los binarios/imagenes que verificaste.
3. Coordina la admision final de peers (זרעי רכילות y asignacion de lane) con `@nexus-core`. No te unas a la red hasta recibir aprobacion; Sora Nexus אפליקציית תעסוקה קובעת את הנתיבים ומחייבת את ההגשה.
4. Despues de que el nodo este en vivo, actualiza tus runbooks con cualquier override que introdujiste y anota el tag de release para que la suuiente iteracion arranque desde esta baseline.

## רשימת אזכור
- [ ] מסמך אישור שחרור como `iroha3`.
- [ ] Hashes y firmas del bundle/imagen verificados.
- [ ] Llaves, הנחיות עמיתים ונקודות קצה Torii ממשיכות להפקה.
- [ ] קטלוג הנתיבים/מרחב הנתונים והפוליטיקה של הניתוב של Nexus עולים בקנה אחד עם ה-Asignacion del Consejo.
- [ ] Validador de configuracion (`irohad --sora --config ... --trace-config`) pasa sin advertencias.
- [ ] Manifests/firmas archivados en el ticket de onboarding y Ops notificado.

בהקשר נוסף של שלבי מיגרציה של Nexus וציפיות טלמטריה, עדכון [Nexus הערות מעבר](./nexus-transition-notes).