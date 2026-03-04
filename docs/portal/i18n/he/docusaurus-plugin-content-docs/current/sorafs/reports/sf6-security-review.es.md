---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Revision de seguridad SF-6
תקציר: Hallazgos y tareas de seguimiento de la evaluacion independiente של חתימה ללא מפתח, הוכחה הזרמת y pipelines de envio de manifests.
---

# Revision de seguridad SF-6

**Ventana de evaluacion:** 2026-02-10 -> 2026-02-18  
**מוביל לעדכון:** גילדת הנדסת אבטחה (`@sec-eng`), קבוצת עבודה של כלי עבודה (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), ממשקי API להוכחה, סטרימינג של הוכחה, manejo de manifests en Torii, Sigstore/OIDC, ווים לשחרור ב-CI.  
**חפצים:**  
- Fuente del CLI y tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Handlers de manifest/proof en Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- שחרור אוטומטי (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- רתום de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Reporte de Paridad GA del Orchestrator SoraFS](./orchestrator-ga-parity.md))

## מתודולוגיה

1. **סדנאות ליצירת מודלים של איום** מאפרון טכנולוגיות מתקדמות עבור מפתחים, מערכות CI y nodos Torii.  
2. **סקירת קוד** אנפוקו שטחיות של אישורים (אינטרקמביו דה אסימונים OIDC, חתימה ללא מפתח), אימות מניפסטים Norito והזרמת לחץ לאחור והוכחה.  
3. **בדיקת דינאמיקו** שחזור מתקנים y simulo modos de falla (שידור חוזר של אסימונים, שיבוש מניפסט, הוכחה זרימת truncados) usando el reprodujo de paridad y fuzz drives a medida.  
4. **בדיקת תצורה** תוקפת ברירת המחדל של `iroha_config`, הוראות דגלים של CLI וסקריפטים לשחרור עבור ביטולים קבועים ובדיקות ביקורת.  
5. **Entrevista de processo** אישור אל flujo de remediacion, rutas de escalamiento y captura de evidencia de auditoria con los owners de release de Tooling WG.

## קורות חיים| תעודת זהות | Severidad | אזור | האלזגו | החלטה |
|----|--------|------|--------|------|
| SF6-SR-01 | אלטה | חתימה ללא מפתח | ברירת המחדל של audiencia del token OIDC נרמזת בעבר בתבניות של CI, עם הרשאות חוזרות לדיירים. | Se agrego la aplicacion explicita de `--identity-token-audience` en hooks de release y templates de CI ([תהליך שחרור](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI ahora falla si se omite la audiencia. |
| SF6-SR-02 | מדיה | הזרמת הוכחה | Los caminos de back-pressure aceptaban buffers de suscriptores sin limite, habilitando agotamiento de memoria. | `sorafs_cli proof stream` איפוס טמאנוס דה תעלת אקוטאדוס עם טרנקמיינטו דטרמיניסטה, רישום קורות חיים Norito y abortando el stream; el espejo Torii זה בפועל עבור נתחי אקוטאר תשובה (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | מדיה | Envio de manifests | El CLI aceptaba manifests sin verificar planes de chunks embebidos cuando `--plan` estaba ausente. | `sorafs_cli manifest submit` ahora recomputa y compara digests de CAR salvo que se provea `--expect-plan-digest`, rechazando mismatches and mostrando pistas de remediacion. Los tests cubren casos de exito/falla (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | באחה | מסלול ביקורת | רשימת הבדיקה של שחרור דאגה דה un log de aprobacion firmado para la revision de seguridad. | Se agrego una seccion en [תהליך שחרור](../developer-releases.md) יש צורך ב-hashes של תזכיר גרסה וכתובת URL של כרטיס כניסה לפני GA. |

Todos los hallazgos high/medium se corrigieron durante la ventana de revisie y se validaron con el harness de paridad existente. אין quedan מוציא ביקורת מאוחרת.

## אימות שליטה

- **Alcance de credenciales:** תבניות של CI ahora exigen audiencia y המנפיקים מפורשים; el CLI y el helper de release fallan rapido salvo que `--identity-token-audience` מלווה ב-`--identity-token-provider`.  
- **ההצגה החוזרת:** בודקת את המציאות המוחלטת/השליליות של המניפסטים, אסיגוראנדו מעכלת את ה-desalineados sigan siendo fallas no deterministas y se detecten antes de tocar la red.  
- **הזרמת לחץ חוזר והוכחה:** Torii ahora transmite פריטים PoR/PoTR sobre canales acotados, y el CLI retiene solo muestras truncadas de latencia + cinco ejemplos de falla, evitando crecimiento manentenien de limite resumenist.  
- **תצפית:** סטרימינג של הוכחה (`torii_sorafs_proof_stream_*`) וקורות חיים של CLI, מבצעים רצף של פרורי לחם אודיטוריה.  
- **תיעוד:** מידע על מפתחים ([אינדקס מפתחים](../developer-index.md), [הפניה ל-CLI](../developer-cli.md)) דגלים אינדיאנים חכמים ותהליכי עבודה של escalamiento.

## מידע על רשימת הבדיקה לשחרור

מנהלי שחרורים **deben** משלימים להוכחות הבאות לקידום מועמד ל-GA:1. Hash del memo mas reciente de revision de seguridad (este documento).  
2. Link al ticket de remediacion seguido (por ejemplo, `governance/tickets/SF6-SR-2026.md`).  
3. פלט של `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos explicitos de audiencia/assuer.  
4. Logs capturados del harness de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. אישור הערות השחרור של Torii כוללות תמיכה של טלמטריה להוכחה זרימה.

No recolectar los artefactos anteriores bloquea el sign-off de GA.

**Hashes de artefactos de referencia (חתימה 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos pendientes

- **מודל אקטואליזציה של איום:** חזור על עדכון טרימסטרלמנט או קדם דגלים של CLI.  
- **Cobertura de fuzzing:** קידודי העברת הוכחה לזרימה באמצעות `fuzz/proof_stream_transport`, זהות מטענים cubriendo, gzip, deflate y zstd.  
- **Ensayo de incidentes:** תוכנות או מפעילים עם פשרה סימולה של אסימון והחזרה לאחור של מניפסט, מתחייבת לביצוע פעולות תיעוד.

## סלידה

- Representante de Security Engineering Guild: @sec-eng (2026-02-20)  
- Representante de Tooling Working Group: @tooling-wg (2026-02-20)

Almacena las aprobaciones firmadas junto al bundle de artefactos de release.