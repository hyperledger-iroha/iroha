---
lang: he
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding des relecteurs de preview

## Vue d'ensemble

DOCS-SORA חליפת חליפה לעיצוב פורטיל. Les בונה avec gate de checksum
(`npm run serve`) et les flux נסה את זה durcis debloquent le prochain jalon:
onboarding de relecteurs מאמת את האוונג que la preview publique ne s'ouvre largement. מדריך סי
מגדיר אספן הערות לדרישות, מוודא זכאות, גישה לגישה וחוץ
המשתתפים מאובטחים. Reportez-vous au
[זרימת הזמנת תצוגה מקדימה](./preview-invite-flow.md) pour la planification des cohortes, la cadence
d'invitation et les exports de telemetrie; les etapes ci-dessous se concentrent sur les actions
a prendre une fois qu'un relecteur a ete selectionne.

- **Perimetre:** relecteurs qui ont besoin d'acces au preview docs (`docs-preview.sora`,
  בונה דפי GitHub או חבילות SoraFS) avant GA.
- **היקף סוס:** מפעילים Torii או SoraFS (couverts par leurs propres kits d'onboarding)
  et deploiements de portail en production (voir
  [`devportal/deploy-guide`](./deploy-guide.md)).

## תפקידים ותנאים מוקדמים

| תפקיד | טיפוסי אובייקטים | חפצי אמנות דרושים | הערות |
| --- | --- | --- | --- |
| מתחזק ליבה | מדריכי מוודא לס נובו, מבצע בדיקות עשן. | טפל ב-GitHub, צור קשר עם Matrix, CLA חתם au dossier. | Souvent deja dans l'equipe GitHub `docs-preview`; המפקיד quand meme une demande pour que l'acces soit auditable. |
| סוקר שותף | Valider des snippets SDK או תוכן ניהול אוונט פרסום פרסום. | אימייל תאגידי, POC משפטי, סימני תצוגה מקדימה של מונחים. | Doit reconnaitre les exigences de telemetrie + traitement des donnees. |
| מתנדב קהילתי | Fournir du feedback d'utilisabilite sur les guides. | טיפול ב-GitHub, עדיף ליצור קשר, לזמן קצר, לקבל את ה-CoC. | Garder les cohortes petites; prioriser les relecteurs ayant signe l'accord de תרומה. |

ישנם סוגים שונים של סוקרים:

1. Reconnaitre la politique d'usage מקובל pour les artefacts de preview.
2. Lire les annexes securite/observabilite
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. S'engager a executer `docs/portal/scripts/preview_verify.sh` avant de servir un
   מיקום תמונת מצב.

## זרימת עבודה1. Demander au demandeur de remplir le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   נוסחא (ou le copier/coller dans une issue). לוכד לפחות: identite, moyen de contact,
   לטפל ב-GitHub, תאריכים מקדימים של ריוויו, ואישור que les docs de securite on ete lues.
2. Enregistrer la demande dans le tracker `docs-preview` (הנפקת GitHub או כרטיס ניהול)
   et assigner un appprobateur.
3. מאמת התנאים המוקדמים:
   - CLA / accord de bidrag au dossier (ou reference de contrat partner).
   - Accuse d'usage acceptable stocke dans la demande.
   - Evaluation des risques terminee (דוגמה: שותפי סוקרים מאשרים לחוק).
4. L'approbateur signe la demande et lie l'issue de suivi a toute entree de change-management
   (דוגמה: `DOCS-SORA-Preview-####`).

## אספקה ויציאה

1. **Partager les artefacts** - Fournir le dernier descriptor + archive de preview depuis
   זרימת העבודה CI ou le pin SoraFS (חפץ `docs-portal-preview`). Rappeler aux reviewers
   המוציא לפועל:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec enforcement de checksum** - Orienter les reviewers vers la commande gatee:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela reuse `scripts/serve-verified-preview.mjs` pour qu'aucun build non verifie
   ne soit lance par accident.

3. **Donner l'acces GitHub (אופציונלי)** - Si les reviewers ont besoin de branches non publiees,
   les ajouter a l'equipe GitHub `docs-preview` pour la duree de la revue et consigner le changement
   de membership dans la demande.

4. **Communiquer les canaux de support** - Partager le contact on-call (Matrix/Slack) et la
   נוהל תקרית [`incident-runbooks`](./incident-runbooks.md).

5. **טלמטריה + משוב** - Rappeler aux reviewers que des analytics anonymisees sont collectees
   (voir [`observability`](./observability.md)). Fournir le formulaire de feedback ou le template
   d'issue mentionne dans l'invitation et journaliser l'evenement avec l'helper
   [`preview-feedback-log`](./preview-feedback-log) pour que le resume de vague reste a jour.

## רשימת ביקורת של המבקר

Avant d'acceder au preview, les reviewers doivent completer:

1. Verifier les artefacts telecharges (`preview_verify.sh`).
2. Lancer le portail via `npm run serve` (ou `serve:verified`) pour s'assurer que le guard checksum est actif.
3. ראה הערות מאובטחות ותצפיות ב-ci-dessus.
4. בודק את המסוף OAuth/נסה זאת באמצעות התחברות לקוד התקן (במידה רלוונטית) ו-eviter de reutiliser des tokens de production.
5. Deposer les constats dans le tracker convenu (גיליון, מסמך חלק או נוסחה) et les tagger
   avec le tag de release תצוגה מקדימה.

## האחריות לתחזוקה ויציאה מהמטוס| שלב | פעולות |
| --- | --- |
| בעיטה | Confirmer que la checklist d'intake est jointe a la demande, partager les artefacts + הוראות, ajouter une entree `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), et planifier un point mi-parcours si la revue. |
| ניטור | Surveiller la Telemetrie Preview (Trafic Try it inhabituel, echec de probe) ו-suivre le runbook d'incident סי quelque chose parait חשוד. עיתונאים לאירועים `feedback-submitted`/`issue-opened` או פרווה ומספר תוצאות של תוצאות מעורפלות. |
| יציאה למטוס | Revoquer l'acces temporaire GitHub ou SoraFS, שולח `access-revoked`, Archiver la demande (כולל קורות חיים של משוב + פעולות בתשומת לב), et mettre a jour le registre des reviewers. Demander au reviewer de purger les builds locaux et joindre le digest genere a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez le meme processus lors de la rotation des reviewers entre vagues. Garder la trace dans le repo
(גיליון + תבניות) aide DOCS-SORA a rester auditable et permet a la governance de confirmer que l'acces
תצוגה מקדימה של מסמכים נוספים.

## תבניות הזמנה ועוד

- Commencer chaque outreach avec le
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  עלוב יותר. ללכוד את המינימום בשפה המשפטית, את ההוראות לתצוגה מקדימה של בדיקה ותשומת לב
  המבקרים מבינים את הפוליטיקה המקובלת.
- Lors de l'edition du template, remplacer les placeholders pour `<preview_tag>`, `<request_ticket>`
  et les canaux de contact. Stocker une copie du message final dans le ticket d'intake עבור סוקרים,
  מבקרים ומבקרים מציינים את הטקסט המדויק.
- Apres l'envoi de l'invitation, mettre a jour le spreadsheet de suivi ou l'issue avec le timestamp
  `invite_sent_at` et la date de fin attendue pour que le rapport
  [תצוגה מקדימה של הזמנת זרימת](./preview-invite-flow.md) puisse capturer la cohorte automatiquement.