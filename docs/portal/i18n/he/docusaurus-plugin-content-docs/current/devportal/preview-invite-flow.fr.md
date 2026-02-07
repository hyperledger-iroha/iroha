---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# תצוגה מקדימה של Flux d'invitation

## Objectif

L'element de roadmap **DOCS-SORA** ציין את ה-Onboarding des relecteurs et le program d'invitations תצוגה מקדימה ציבורית של comme derniers bloqueurs avant la sortie de beta. Cette page decrit comment ouvrir chaque vague d'invitations, quels artefacts doivent etre livres Avant d'envoyer les invites and comment prouver que le flux est ניתנים לביקורת. Utilisez-la avec:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour la gestion par relecteur.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour les garanties de checksum.
- [`devportal/observability`](./observability.md) pour les exports de telemetrie et hooks d'alerting.

## תוכנית מעורפלת

| מעורפל | קהל | קריטריונים ד'עיקרי | קריטריונים דה מיון | הערות |
| --- | --- | --- | --- | --- |
| **W0 - ליבת תחזוקה** | מנהלי מסמכים/SDK תקפים לתוכן שלך. | Equipe GitHub `docs-portal-preview` peuplee, gate checksum `npm run serve` en vert, Alertmanager silencieux 7 jours. | Tous les docs P0 relus, backlog tague, aucun incident bloquant. | Sert a valider le flux; pas d'email d'invitation, seulement partage des artefacts תצוגה מקדימה. |
| **W1 - שותפים** | מפעילי SoraFS, אינטגרטורים Torii, רלקטורים ממשל sos NDA. | W0 termine, termes juridiques approuves, proxy Try-it en staging. | שותפים להחתמה אוספים (נושא או חתימה על הנוסחאות), טלמטריה מחודשת <=10 חוזרים במקביל, תליון רגרסיות בטוח 14 ימים. | Imposer template d'invitation + tickets de demande. |
| **W2 - Communaute** | תורמים selectnes depuis la list d'attente communaute. | סיום W1, תרגילי תקריות חוזרים על עצמם, שאלות נפוצות פרסום mise a jour. | משוב, >=2 משחרר מסמכים באמצעות תצוגה מקדימה של צינור ללא החזרה לאחור. | Limiter מזמין במקביל (<=25) ו-batcher chaque semaine. |

Documentez quelle vague est active dans `status.md` et dans le tracker des demandes preview afin que la governance voie le statut d'un coup d'oeil.

## רשימת בדיקה מוקדמת

Terminez ces actions **avant** de planifier des invitations pour une vague:

1. **חפצים CI disponibles**
   - Dernier `docs-portal-preview` + תשלום מתאר par `.github/workflows/docs-portal-preview.yml`.
   - הערה סיכה SoraFS ב-`docs/portal/docs/devportal/deploy-guide.md` (מתאר דה חיתוך קיים).
2. **בקשת בדיקה**
   - Invoque `docs/portal/scripts/serve-verified-preview.mjs` דרך `npm run serve`.
   - הוראות `scripts/preview_verify.sh` נבדקים על macOS + Linux.
3. **טלמטריה בסיסית**
   - `dashboards/grafana/docs_portal.json` montre un trafic Try it sain et l'alerte `docs.preview.integrity` est au vert.
   - Derniere annexe de `docs/portal/docs/devportal/observability.md` mise a jour avec des liens Grafana.
4. **שלטון חפצים**
   - Issue du invite tracker prete (une issue par vague).
   - עותק תבנית רשומים של רלקטורים (voir [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - אישורים משפטיים ו-SRE מצריכים בעיה.

Enregistrez l'achevement du preflight dans le invite tracker avant d'envoyer le moindre אימייל.

## Etapes du flux1. **מבחר מועמדים**
   - Tier depuis la liste d'attente ou la שותפים לקובץ.
   - S'assurer que chaque candidat a un template de demande complete.
2. **Approuver l'acces**
   - מקצה un approbateur a l'issue du invite tracker.
   - Verifier les prerequis (CLA/contrat, שימוש מקובל, אבטחה קצרה).
3. **הזמנות של שליחים**
   - Completer les placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, אנשי קשר).
   - הצטרף לתיאור + הארכיון הארכיון, כתובת האתר לשלב נסה את זה ותמיכה ב-les canaux.
   - Stocker l'email final (או תמליל Matrix/Slack) בעניין.
4. **Suivre l'onboarding**
   - Mettre a jour le invite tracker avec `invite_sent_at`, `expected_exit_at`, et le statut (`pending`, `active`, `complete`, Grafana).
   - Lier la demande d'entree du relecteur pour auditabilite.
5. **Surveiller la telemetrie**
   - Surveiller `docs.preview.session_active` et les התראות `TryItProxyErrors`.
   - אובר על תקרית סי לה Telemetrie devie du baseline ו-rescribe le resultat a cote de l'entree d'invitation.
6. **אספן משוב ומסתור**
   - משוב מגיע אליכם ב-`expected_exit_at` יש תשומת לב.
   - Mettre a jour l'issue de vague avec un corresume של בית המשפט (קונסטטס, תקריות, פעולות קדומות) avant de passer au cohort suivant.

## עדויות ודיווחים

| חפץ | או סטוקר | Cadence de mise a jour |
| --- | --- | --- |
| Issue du invite tracker | Projet GitHub `docs-portal-preview` | הזמנה אפר צ'אק. |
| ייצוא דואר רוסטר | הרשמה שקר ב-`docs/portal/docs/devportal/reviewer-onboarding.md` | Hebdomadaire. |
| טלמטריה | `docs/source/sdk/android/readiness/dashboards/<date>/` (משמש מחדש le bundle telemetrie) | תקריות מעורפלות + אפרה. |
| תקציר משוב | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (creer un dossier par vague) | Dans les 5 jours suivant la sortie de vague. |
| Note de reunion governance | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | דגם DOCS-SORA של סינכרון אוונט צ'אק. |

Lancez `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
apres chaque batch pour produire un digest lisible par machine. Joignez le JSON redu a l'issue de vague pour que les relecteurs governance confirment les comptes d'invitations sans rejouer tout le log.

Joignez la list de preuves a `status.md` a chaque fin de vague afin que l'entree roadmap puisse etre mise a jour rapidement.

## קריטריונים להחזרה והפסקה

Mettez en pause le flux d'invitations (et notifiez la governance) lorsque l'un des cas suivants survivant:

- Proxy לאירועים נסה זאת ללא צורך בהחזרה לאחור (`npm run manage:tryit-proxy`).
- התרעות עייפות: >3 דפי התראה יוצקים נקודות קצה בתצוגה מקדימה בלבד במשך 7 ימים.
- Ecart de conformite: נגר הזמנה sans termes signes או sans enregistrement du template de demande.
- Risque d'integrite: חוסר התאמה של checksum detecte par `scripts/preview_verify.sh`.

הייחודיות של Reprenez נמשכת עד הסוף לתיקון בזמן הגשש הזמנת ותאשר כי לוח המחוונים הטלמטרי הוא יציב במשך 48 שעות.