---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# תצוגה מקדימה של Playbook d'invitation ציבורית

## Objectifs du program

הערה מפורשת של ספר המשחקים פרסמה את התצוגה המקדימה לציבור
זרימת עבודה לכניסה לבודקים est actif. Il garde la roadmap DOCS-SORA honnete en
s'assurant que chaque invitation part avec des artefacts verifiables, des consignes de securite
et un chemin clair pour le feedback.

- **קהל:** רשימת מרפאות קהילתיות, שותפים ואנשי שמירה
  signe la politique d'usage קבילה du preview.
- **Plafonds:** taille de vague par defaut <= 25 סוקרים, fenetre d'acces de 14 jours,
  תגובה נוספת לאירועים 24 שעות.

## רשימת רשימת השערים

Terminez ces taches avant d'envoyer une הזמנה:

1. Derniers artefacts de preview charges en CI (`docs-portal-preview`,
   Manifest de Checksum, Descriptor, Bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gate par checksum) teste sur le meme tag.
3. כרטיסים ל-onboarding des reviewers approuves et lies a la vague d'invitation.
4. מסמכים מאובטחים, ניתנים לצפייה ותקפים
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. נוסחאות משוב או הכנת תבנית (כולל את champs de severite,
   תפיסות רפרודוקציה, צילומי מסך ומידע על הסביבה).
6. Copy d'annonce revise par Docs/DevRel + Governance.

## Paquet d'invitation

הזמנה של Chaque doit כוללת:

1. **חפצי אומנות מאמתים** - Fournir les liens vers le manifest/plan SoraFS ou les artefacts
   GitHub, בתוספת le manifest de checksum et le descriptor. Referencer explicitement la commande
   de verification pour que les reviewers puissent l'executer avant de lancer le site.
2. **הוראות לשרת** - כלול שער תצוגה מקדימה של la commande par checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels securite** - Indiquer que les tokens expirent automatiquement, que les liens
   ne doivent pas etre partages, et que les incidents doivent etre signales מיידי.
4. **Canal de feedback** - Lier la template/formulaire et clarifier les attentes de temps de reponse.
5. **תאריכי התוכנית** - פורניר תאריכי הבכורה/סיום, שעות המשרד או סינכרון, et la prochaine
   fenetre de refresh.

L'email exemple dans
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
couvre ces exigences. מספר מצייני מיקום (תאריכים, כתובות אתרים, אנשי קשר)
avant l'envoi.

## תצוגה מקדימה של Exposer l'hote

Ne promouvoir l'hote תצוגה מקדימה qu'une fois l'onboarding termine et le ticket de changement approuve.
Voir le [guide d'exposition de l'hote preview](./preview-host-exposure.md) pour les etapes מקצה לקצה
de build/publish/verify utilisees dans cette סעיף.

1. **בנה ואריזה:** סמן את תג השחרור והמוצרים המוכרים.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   תסריט ה-pin ecrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` sous `artifacts/sorafs/`. Joindre ces fichiers a la vague
   d'invitation afin que chaque מבקר puisse verifier les memes bits.2. **תצוגה מקדימה של Publier l'alias:** Relancer la commande sans `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et la preuve d'alias emise
   par la governance). התסריט הוא המניפסט של `docs-preview.sora` et emet
   `portal.manifest.submit.summary.json` פלוס `portal.pin.report.json` pour le bundle de preuves.

3. **Prober le deploiement:** Confirmer que l'alias se resout et que le checksum correspond au tag
   avant d'envoyer les הזמנות.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) sous la main comme fallback pour
   que les reviewers puissent lancer une copie locale si le edge preview flanche.

## קו זמן לתקשורת

| Jour | פעולה | בעלים |
| --- | --- | --- |
| D-3 | הגמר להעתקת הזמנה, חפצי אמנות, אימות יבשה | Docs/DevRel |
| D-2 | סימון שלטון + כרטיס החלפת | Docs/DevRel + ממשל |
| D-1 | שליחת הזמנות דרך le template, mettre a jour le tracker avec la list des destinataires | Docs/DevRel |
| ד | שיחת בעיטה / שעות עבודה, צג לוחות מחוונים של טלמטריה | Docs/DevRel + כוננות |
| D+7 | תקכל משוב ומעורפל, טריאז' של בעיות בלוקוואנטות | Docs/DevRel |
| D+14 | Clore la vague, revoquer l'acces temporaire, publier un resume dans `status.md` | Docs/DevRel |

## Suivi d'acces et telemetrie

1. רושם chaque destinataire, חותמת זמן של הזמנה ותאריך ביטול ביטול
   לוגר משוב לתצוגה מקדימה (voir
   [`preview-feedback-log`](./preview-feedback-log)) afin que chaque vague partage la meme
   trace de preuves:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Les evenements pris en charge sont `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, et `access-revoked`. Le log se trouve a
   `artifacts/docs_portal_preview/feedback_log.json` par defaut; joignez-le au ticket de
   הזמנה מעורפלת עם הסכמה לפורמולציות. Utilisez l'helper de סיכום
   pour produire un roll-up auditable avant la note de cloture:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   התקציר של JSON מונה את ההזמנות מעורפלות, les destinataires ouverts, les
   comptors de feedback et le timestamp du dernier evenement. L'helper repose sur
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   זרימת עבודה של donc le meme peut tourner localement ou en CI. Utiliser le template de digest
   dans [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   lors de la publication du recap de la vague.
2. Tagger les Dashboards de telemetrie avec le `DOCS_RELEASE_TAG` use pour la vague afin que
   les pics puissent etre correles aux cohortes d'invitation.
3. מפעל `npm run probe:portal -- --expect-release=<tag>` לאחר פריסה עבור אישור que
   l'environnement תצוגה מקדימה annonce la bonne metadata de release.
4. השולח מציג תקרית ב-Template de runbook et le lier a la cohorte.

## משוב ולבוש1. Agreger le feedback dans un doc partage או un board d'issues. Tagger les items avec
   `docs-preview/<wave>` עבור הבעלים של מפת הדרכים puissent les retrouver facilement.
2. Utiliser la sortie summary du preview logger pour remplir le rapport de vague, puis
   resumer la cohorte dans `status.md` (משתתפים, תקצירים, תיקונים קודמים) et
   mettre a jour `roadmap.md` si le jalon DOCS-SORA שינוי.
3. Suivre les etapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoquer l'acces, Archiver les demandes et
   מחזיר למשתתפים.
4. Preparer la prochaine vague en rafraichissant les artefacts, en relancant les gates de checksum
   et en mettant a jour le template d'invitation avec de nouvelles תאריכים.

Appliquer ce playbook de facon consistente garde le program previewable auditable et donne a
Docs/DevRel un moyen repetable de faire grandir les invitations a mesure que le portail approche de GA.