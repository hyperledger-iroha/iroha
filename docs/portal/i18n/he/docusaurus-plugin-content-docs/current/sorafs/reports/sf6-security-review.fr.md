---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Revue de sécurité SF-6
תקציר: Constatations et actions de suivi de l'évaluation indépendante de la signature sans clé, du proof streaming et des pipelines d'envoi de manifests.
---

# Revue de sécurité SF-6

**Fenêtre d'évaluation:** 2026-02-10 → 2026-02-18  
**Leads de revue:** גילדת הנדסת אבטחה (`@sec-eng`), קבוצת עבודה של כלי עבודה (`@tooling-wg`)  
**Périmètre:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), ממשקי API להוכחה, סטרימינג של הוכחה, גילויי תנועה ב-Sigstore Sigstore/OIDC, ווים לשחרור CI.  
**חפצים:**  
- מקור CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- מניפסט/הוכחה של מטפלים Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- שחרור אוטומציה (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- רתם דה פריטה דטרמיניסט (`crates/sorafs_car/tests/sorafs_cli.rs`, [Rapport de parité GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## מתודולוגיה

1. **Ateliers de threat modeling** ont cartographié les capacités d'attaque pour les postes devs, les systèmes CI et les nœuds Torii.  
2. **סקירת קוד** a ciblé les surfaces d'identifiants (échange de tokens OIDC, signature sans clé), la validation de manifests Norito ו-back-pressure du proof streaming.  
3. **בדיקות דינמיות** ont rejoué des manifests de fixtures et simulé des modes de panne (שידור חוזר של אסימונים, שיבוש מניפסט, הוכחה זרימה tronqués) דרך רתמת ה-parité et des fuzz drives dédiés.  
4. **בדיקת תצורה** תוקפת ברירת המחדל `iroha_config`, הדרכה של דגלים CLI ו-scripts de release pour garant des exécutions déterministes et auditables.  
5. **Entretien de processus** אישור לשטף תיקון, les chemins d'escalade et la capture d'evidence d'audit avec les owners de release du Tooling WG.

## קורות חיים| תעודת זהות | Sévérité | אזור | קונסטט | החלטה |
|----|--------|------|--------|------|
| SF6-SR-01 | Élevée | חתימה sans clé | ברירת המחדל של אסימונים של הקהל OIDC משתמעת בתבניות CI, עם סיכון של משחק חוזר בין דייר. | Ajout d'une exigence מפורש `--identity-token-audience` ב-Hooks de release et templates CI ([תהליך שחרור](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI échoue désormais si l'audience est misise. |
| SF6-SR-02 | מויין | הזרמת הוכחה | Les chemins de back-pressure acceptaient des buffers de subscribes sans limite, permettant L'épuisement mémoire. | `sorafs_cli proof stream` להטיל des tails de channel bornées avec un truncation déterministe, journalise des résumés Norito et abort le stream ; le miroir Torii a été mis à jour pour borner les chunks response (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | מויין | Envoi de manifests | Le CLI acceptait des manifests sans vérifier les chunk plans embarqués quand `--plan` était נעדר. | `sorafs_cli manifest submit` חישוב מחדש והשווה את תמונת ה-CAR עם `--expect-plan-digest`, דחיית אי-התאמות ורמזים לתיקון. Des tests couvrent succès/échecs (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | פייבל | מסלול ביקורת | La checklist de release n'avait pas de log d'approbation signné pour la revue de sécurité. | Ajout d'une section [תהליך שחרור](../developer-releases.md) exigeant l'attachement des hashes du memo de revue et l'URL du ticket de sign-off avant GA. |

Tous les constats גבוה/בינוני ont été corrigés תליון la fenêtre de revue et validés par le harness de parité existant. Aucun בעיה ביקורת סמויה ne reste.

## אימות שליטה

- **Portée des identifiants :** תבניות CI מחייבות קהל ומנפיק מפורש; le CLI et le helper de release échouent fast si `--identity-token-audience` n'accompagne pas `--identity-token-provider`.  
- **הצגה חוזרת:** Les tests mis à jour couvrent les flux positifs/négatifs d'envoi de manifests, garantissant que les digests and mismatch restent des échecs non déterministes et sont signalés avant de toucher le réseau.  
- **הזרמת עמידה בלחץ אחורי :** Torii מפוזר פריטי פריטים PoR/PoTR דרך ערוצי bornés, et le CLI ne conserve que des échantillons de latence tronqués + cinq exemples d'échec, évitant la croissant évitant la limite דטרמיניסטים.  
- **תצפית :** הזרמת הוכחה למחשבים (`torii_sorafs_proof_stream_*`) וקורות חיים של CLI ללכוד את מקורות הביטול, על רקע פירורי לחם של אופרטורים.  
- **תיעוד :** Les guides devs ([אינדקס מפתחים](../developer-index.md), [הפניה CLI](../developer-cli.md)) מסמנים לס דגלים sensibles et les workflows d'escalade.

## Ajouts à la checklist de release

מנהלי השחרור **דואינט** מצטרפים לקידום המועמדים GA:1. Hash du memo de revue de sécurité le plus récent (מסמך ce).  
2. Lien vers le ticket de remédiation suivi (לדוגמה `governance/tickets/SF6-SR-2026.md`).  
3. פלט de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` montrant les arguments קהל/מנפיק מפורש.  
4. Logs capturés du harness de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. אישור לגבי הערות שחרור Torii כוללות את מחשבי ה-Télémétrie de proof streaming borné.

לא אספן חפצי אמנות ci-dessus bloque le sign-off GA.

**Hashes des artefacts de référence (יציאה 2026-02-20) :**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis en attente

- **דגם Mise à jour du threat :** Repéter cette revue chaque trimestre ou avant des ajouts majeurs de flags CLI.  
- **Couverture fuzzé :** Les encodages de transport proof streaming sont fuzzés via `fuzz/proof_stream_transport`, couvrant identity, gzip, deflate et zstd.  
- **Repétition d'incident :** Planifier un exercice operator simulant une compromission de token and un rollback de manifest, pour s'assurer que la documentation reflète les procédures pratiquées.

## אישור

- נציג הנדסת אבטחה גילדת: @sec-eng (2026-02-20)  
- קבוצת עבודה של נציג כלי עבודה: @tooling-wg (2026-02-20)

שומר על ההסכמות החתומות עם חבילת חפצי האמנות לשחרור.