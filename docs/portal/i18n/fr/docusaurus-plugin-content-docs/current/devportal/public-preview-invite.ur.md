---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پبلک پریویو دعوتی پلے بک

## پروگرام کے مقاصد

یہ پلے بک وضاحت کرتی ہے کہ ریویور آن بورڈنگ ورک فلو فعال ہونے کے بعد پبلک پریویو کیسے اعلان اور چلایا جائے۔
DOCS-SORA s'intéresse aux artefacts et aux artefacts سیکیورٹی رہنمائی،
اور واضح feedback راستہ شامل ہونا یقینی بنایا جاتا ہے۔

- **آڈیئنس:** Les responsables des mainteneurs sont sélectionnés pour l'aperçu d'utilisation acceptable.
- **سیلنگز:** taille d'onde par défaut <= 25 heures 14 heures de fenêtre d'accès, et 24 heures de réponse aux incidents

## لانچ گیٹ چیک لسٹ

Il y a des gens qui sont en contact avec eux:

1. تازہ ترین aperçu des artefacts CI میں اپلوڈ ہوں (`docs-portal-preview`,
   Manifeste de somme de contrôle, descripteur, bundle SoraFS)۔
2. `npm run --prefix docs/portal serve` (checksum-gated) pour la balise pour votre compte
3. Les utilisateurs approuvent la vague d'invitation et la vague d'invitation.
4. L'observabilité et l'incident sont validés.
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))۔
5. commentaires sur le modèle de problème (gravité, étapes de reproduction, captures d'écran, informations sur l'environnement et informations sur l'environnement)
6. اعلان کی کاپی Docs/DevRel + Governance نے ریویو کی ہو۔

## دعوتی پیکیج

ہر دعوت میں شامل ہونا چاہیے:1. **Artéfacts vérifiés** — Manifeste/plan SoraFS et artefact GitHub en cours de réalisation
   ساتھ میں checksum manifest et اور descriptor بھی دیں۔ vérification کمانڈ واضح طور پر لکھیں تاکہ
   ریویورز site لانچ کرنے سے پہلے اسے چلا سکیں۔
2. **Instructions de service** — aperçu contrôlé par somme de contrôle.

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels de sécurité** — Les jetons et les jetons expirent avant la date d'expiration.
   اور incidents فوراً رپورٹ کیے جائیں۔
4. **Canal de rétroaction** — modèle/formulaire de problème pour répondre aux attentes en matière de temps de réponse et de réponse
5. **Dates du programme** — dates de début/fin, heures de bureau et synchronisations, et fenêtre d'actualisation.

نمونہ ای میل
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
میں دستیاب ہے اور یہ exigences پوری کرتا ہے۔ Espaces réservés بھیجنے سے پہلے (dates, URL, contacts)
اپ ڈیٹ کریں۔

## پریویو hôte et exposer کریں

جب تک onboarding مکمل نہ ہو اور modifier le ticket منظور نہ ہو تب تک aperçu de l'hôte کو promouvoir نہ کریں۔
Voici comment créer/publier/vérifier les étapes de bout en bout
[aperçu du guide d'exposition de l'hôte] (./preview-host-exposure.md) دیکھیں۔

1. **Construire des objets déterministes :** tampon de balise de sortie pour les artefacts déterministes

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

   script de broche `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   اور `portal.dns-cutover.json` et `artifacts/sorafs/` میں لکھتا ہے۔ ان فائلوں کو inviter la vague
   کے ساتھ attacher کریں تاکہ ہر ریویور وہی bits check کر سکے۔2. **Publication de l'alias d'aperçu :** Il s'agit d'un `--skip-submit` qui est en cours de publication.
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` et preuve d'alias émise par la gouvernance)۔
   `docs-preview.sora` pour la liaison manifeste pour un ensemble de preuves
   `portal.manifest.submit.summary.json` et `portal.pin.report.json` نکالے گا۔

3. **Sonde de déploiement :** invite la personne à résoudre l'alias et la somme de contrôle à la balise et à la correspondance.
   یقینی بنائیں۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) est une solution de secours pour un outil pratique
   Aperçu du bord en cours de réalisation

## کمیونیکیشن ٹائم لائن

| دن | ایکشن | Propriétaire |
| --- | --- | --- |
| J-3 | Comment finaliser les artefacts actualiser la vérification et l'exécution à sec | Docs/DevRel |
| J-2 | Approbation de la gouvernance + ticket de modification | Docs/DevRel + Gouvernance |
| J-1 | modèle de suivi de suivi de liste de destinataires | Docs/DevRel |
| D | appel de coup d'envoi / heures de bureau, tableaux de bord de télémétrie مانیٹر کریں | Docs/DevRel + Sur appel |
| J+7 | résumé des commentaires à mi-parcours, problèmes de blocage et triage | Docs/DevRel |
| J+14 | wave بند کریں، عارضی رسائی révoquer کریں، `status.md` میں خلاصہ شائع کریں | Docs/DevRel |

## Suivi des accès et télémétrie1. Destinataire, horodatage de l'invitation, date de révocation et aperçu de l'enregistreur de commentaires.
   (دیکھیں [`preview-feedback-log`](./preview-feedback-log)) تاکہ ہر vague ایک ہی piste de preuves شیئر کرے :

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Événements pris en charge ہیں `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, et `access-revoked`۔ journal ڈیفالٹ طور پر
   `artifacts/docs_portal_preview/feedback_log.json` میں موجود ہے؛ اسے invite wave ٹکٹ کے ساتھ
   formulaires de consentement سمیت joindre کریں۔ clôture نوٹ سے پہلے assistant récapitulatif استعمال کریں تاکہ
   Voici un roll-up vérifiable :

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   résumé JSON et vague d'invitations, destinataires, nombre de commentaires, et événement d'événement
   timestamp کو énumérer کرتا ہے۔ aide
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   Il s'agit d'un flux de travail et d'un flux de travail complet pour CI. récapituler شائع کرتے وقت
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   والا digest template استعمال کریں۔
2. tableaux de bord de télémétrie et wave میں استعمال ہونے والے `DOCS_RELEASE_TAG` کے ساتھ tag کریں تاکہ
   pointes et inviter des cohortes en corrélation
3. Déployez l'environnement de prévisualisation de l'environnement de prévisualisation `npm run probe:portal -- --expect-release=<tag>`
   درست publier des métadonnées annoncer کرے۔
4. Un incident et un modèle de runbook pour capturer une cohorte et un lien

## Commentaires sur la clôture1. commentaires sur les documents partagés et sur le forum de discussion articles `docs-preview/<wave>` et tag article
   les propriétaires de la feuille de route انہیں آسانی سے requête کر سکیں۔
2. Aperçu de l'enregistreur et sortie récapitulative du rapport d'onde pour la cohorte `status.md` pour résumer l'enregistrement
   (participants, résultats et correctifs prévus) Pour le jalon DOCS-SORA et pour `roadmap.md`, il s'agit d'un jalon
3. [`reviewer-onboarding`](./reviewer-onboarding.md) Les étapes de désintégration suivent : révocation de l'accès.
   archives des demandes pour les participants
4. La vague d'artefacts rafraîchit les portes de somme de contrôle et le modèle d'invitation pour les dates et les dates.

Le playbook est un livre de lecture et un aperçu du livre auditable dans Docs/DevRel.
اسکیل کرنے کا répétable طریقہ ملتا ہے جیسے جیسے پورٹل GA کے قریب آتا ہے۔