---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Proceso de lanzamiento
תקציר: Ejecuta el gate de lanzamiento de CLI/SDK, aplica la política de versionado compartida y publica notas de lanzamiento canónicas.
---

# תהליך לנזמיינטו

Los binarios de SoraFS (`sorafs_cli`, `sorafs_fetch`, עוזרים) y los boxes de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) se publican juntos. צינור אל
de lanzamiento mantiene el CLI y las bibliotecas alineados, asegura cobertura de
מוך/בדיקה ותפיסת חפצים לצריכה במורד הזרם. Ejecuta la list de
Verificación de Abajo para cada tag candidato.

## 0. אישור להחלפת הדיווח

Antes de ejecutar el gate técnico de lazamiento, captura los artefactos más
recientes de la revisión de seguridad:

- הורד את התזכיר más reciente de revisión de seguridad SF-6 ([דוחות/sf6-security-review](./reports/sf6-security-review.md))
  y registra su hash SHA256 en el ticket de lanzamiento.
- Adjunta el enlace del ticket de remediación (por ejemplo, `governance/tickets/SF6-SR-2026.md`) y anota
  los aprobadores de Security Engineering y del Tooling Working Group.
- Verifica que la List de remediación del memo esté cerrada; los ítems sin resolver bloquean el lanzamiento.
- הכנה למען סוביר לוס לוגים דל רתום דה פארידאד (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto con el bundle del manifest.
- אישור que el comando de firma que planeas ejecutar incluya tanto `--identity-token-provider` como
  un `--identity-token-audience=<aud>` explícito para que el alcance de Fulcio quede capturado en la evidencia del release.

כולל אמנות חפצים אל הודעה על הגוברננזה y publicar el lanzamiento.

## 1. Ejecutar el gate de lanzamiento/pruebas

El helper `ci/check_sorafs_cli_release.sh` ejecuta formateo, Clippy y tests
sobre los crates de CLI y SDK con un directorio target local al space work (`.target`)
para evitar conflictos de permisos al ejecutarse dentro de contenedores CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

התסריט הממש את התוצאות הבאות:

- `cargo fmt --all -- --check` (סביבת עבודה)
- `cargo clippy --locked --all-targets` עבור `sorafs_car` (עם תכונה `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` עבור ארגזי esos mismos

אם יש לך רקע, יש צורך בתקשורת. Los builds de release
deben estar continuos con main; אין hagas cherry-pick de fixes en ramas de release.
El gate también comprueba que los flags de firma sin claves (`--identity-token-issuer`,
`--identity-token-audience`) se proporcionen donde corresponda; los argumentos
faltantes hacen fallar la ejecución.

## 2. אפליקציית מדיניות הגרסה

מטלות לארגזי CLI/SDK de SoraFS usan SemVer:- `MAJOR`: ראה הצג עבור מהדורת פריימר 1.0. Antes de 1.0 el incremento
  menor `0.y` **indica cambios con ruptura** en la superficie del CLI o en los
  esquemas Norito.
- `MINOR`: Trabajo de funciones sin ruptura (נואבוס קומנדוס/דגלים, נואבוס
  campos Norito protegidos por política optional, adiciones de telemetria).
- `PATCH`: תיקוני באגים, משחרר סולו של תיעוד ואקטואליזציה
  dependencias que no cambian el comportamiento לצפייה.

Siempre mantén `sorafs_car`, `sorafs_manifest` y `sorafs_chunker` en la misma version
para que los consumidores de SDK במורד הזרם puedan depender de una única cadena
אלינאדה. גרסאות מצטברות:

1. Actualiza los campos `version =` en cada `Cargo.toml`.
2. Regenera el `Cargo.lock` vía `cargo update -p <crate>@<new-version>` (el workspace
   גרסאות מוצהרות).
3. Ejecuta el gate de lanzamiento otra vez para asegurar que no queden artefactos
   מיושנים.

## 3. הכנת מסמכים דה לנזמיינטו

שחרור קודמים פורסם ב-Changelog ב-Markdown כדי להחזיר את התמונות
השפעה על CLI, SDK ו-gobernanza. Usa la plantilla en
`docs/examples/sorafs_release_notes.md` (cópiala a tu directorio de artefactos de
release y completa las secciones con detalles concretos).

טווח מינימלי:

- **Destacados**: titulares de funciones para consumidores de CLI y SDK.
- **Impacto**: cambios con ruptura, שדרוגים של פוליטיקה, דרישות
  מינימוס דה gateway/nodo.
- **Pasos de actualización**: comandos TL;DR para actualizar dependencias cargo y
  מכשירי reejecutar deterministas.
- **אימות**: hashes o envoltorios de salida de comandos y la revisión exacta
  de `ci/check_sorafs_cli_release.sh` ejecutada.

Adjunta las notas de lanzamiento completas al tag (por ejemplo, el cuerpo del release
en GitHub) y guárdalas junto a los artefactos generados de forma determinista.

## 4. Ejecutar los hooks de release

Ejecuta `scripts/release_sorafs_cli.sh` para generar el bundle de firmas y el
קורות חיים של אימות que se envía con cada release. El עטיפה קומפילה אל CLI
cuando es necesario, llama a `sorafs_cli manifest sign` y reproducere de inmediato
`manifest verify-signature` para que los fallos aparezcan antes de etiquetar.
דוגמה:

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

יתרונות:- רישום כניסות לשחרור (מטען, תוכניות, סיכומים, אספרדו hash de token)
  en tu repo o config de despliegue para que el script ים לשחזור. צרור אל
  de fixtures en `fixtures/sorafs_manifest/ci_sample/` muestra el layout canónico.
- Basa la automatización de CI en `.github/workflows/sorafs-cli-release.yml`; ejecuta
  el gate de release, invoca el script anterior y archiva bundles/firmas como
  חפצי זרימת עבודה. Refleja el mismo orden de comandos (שער שחרור → firmar
  → verificar) en otros sistemas CI para que los logs de auditoría coincidan con los
  hashes generados.
- Mantén juntos `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  y `manifest.verify.summary.json`; forman el paquete referenciado en la notificación
  דה גוברננסה.
- אביזרי פרסום אקטואליים, עותק של מציאות,
  el chunk plan y los summaries en `fixtures/sorafs_manifest/ci_sample/` (y actualiza
  `docs/examples/sorafs_ci_sample/manifest.template.json`) לפני הנימוס.
  מערכת ההפעלה במורד הזרם תלויה בגרסת התקנים לשחזור
  el bundle de release.
- Captura el log de ejecución de la verificación de canales acotados de
  `sorafs_cli proof stream` y adjúntalo al paquete del release para demostrar que
  לאס salvaguardas דה הוכחה הזרמת סיואן פעיל.
- Registra el `--identity-token-audience` exacto usado durante la firma en las
  notas de lanzamiento; gobernanza verifica el audience contra la política de Fulcio
  antes de aprobar la publicación.

ארה"ב `scripts/sorafs_gateway_self_cert.sh` cuando el release también incluya un
השקת שער. Apunta al mismo bundle de manifest para probar que la
atestación coincide con el artefacto candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. כללי התנהגות ופרסום

Después de que los checks pasen y los hooks se completen:1. Ejecuta `sorafs_cli --version` y `sorafs_fetch --version` para confirmar que los binarios
   reportan la nueva version.
2. הכנה לתצורת גרסה בגרסה `sorafs_release.toml` (מועדף)
   o en otro archivo de config rastreado por tu repo de despliegue. Evita depender de
   משתנים de entorno אד-הוק; pasa rutas al CLI con `--config` (o equivalente) para que
   תשומות לשחרור שון מפורשים ופרטים לשחזור.
3. יצירת תג פיראדו (מועדף) או תג אנוטדו:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Sube los artefactos (חבילות CAR, מניפסטים, resúmenes de proofs, notas de release,
   פלטים של אטסטציון) אל הרישום דל פרויקטו סיואינדו אל רשימת הבדיקה של גוברננזה
   en la [guía de despliegue](./developer-deployment.md). זה שחרור גדול
   אביזרי, מאגר אביזרי מתקנים או חנות חפצים
   automatización de auditoría pueda השוואת אל צרור פרסום עם אל בקרה דה
   גרסאות.
5. הודעה על תעלת גוברננצה עם תג פיראדו, הודעות שחרור, חשיש
   del bundle/firmas del manifest, resúmenes archivados de `manifest.sign/verify` y
   cualquier envoltorio de atestación. כולל כתובת CI של עבודה (או ארכיון יומנים) que
   ejecutó `ci/check_sorafs_cli_release.sh` y `scripts/release_sorafs_cli.sh`. אקטואליזה
   el ticket de gobernanza para que los auditores puedan trazar las aprobaciones a los
   חפצים; cuando el job `.github/workflows/sorafs-cli-release.yml` publique
   notificaciones, enlaza los hashes registrados en lugar de pegar resúmenes ad-hoc.

## 6. Seguimiento שחרור אחורי

- Asegura que la documentación que apunta a la nueva version (התחלות מהירות, plantillas de CI)
  esté actualizada o confirma que no se requieren cambios.
- Registra entradas de roadmap si se requiere trabajo posterior (por ejemplo, flags de
- Archiva los logs de salida del gate de release para auditoría: guárdalos junto a los
  artefactos firmados.

Seguir este pipeline mantiene el CLI, los cartes del SDK y el material de gobernanza
alineados para cada ciclo de release.