---
lang: fr
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2026-01-03T18:08:00.500077+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Agenda de synchronisation mensuel Docs/DevRel

Cet agenda formalise la synchronisation mensuelle Docs/DevRel qui est référencée dans
`roadmap.md` (voir « Ajouter un examen du personnel de localisation aux documents mensuels/DevRel
sync ») et le forfait Android AND5 i18n. Utilisez-la comme liste de contrôle canonique, et
mettez-le à jour chaque fois que les livrables de la feuille de route ajoutent ou suppriment des points à l’ordre du jour.

## Cadence et logistique

- **Fréquence :** mensuelle (généralement le deuxième jeudi, 16h00 UTC)
- **Durée :** 45 minutes + 15 minutes de suspension en option pour les plongées profondes
- **Emplacement :** Zoom (`https://meet.sora.dev/docs-devrel-sync`) avec partage
  notes dans HackMD ou `docs/source/docs_devrel/minutes/<yyyy-mm>.md`
- **Public :** Responsable Docs/DevRel (président), ingénieurs Docs, localisation
  gestionnaire de programme, SDK DX TLs (Android, Swift, JS), Product Docs, Release
  Délégué ingénierie, observateurs Support/QA
- **Facilitateur :** Responsable Docs/DevRel ; nommer un scribe tournant qui
  valider les minutes dans le repo dans les 24 heures

## Liste de contrôle préalable aux travaux

| Propriétaire | Tâche | Artefact |
|-------|------|--------------|
| Scribe | Créez le fichier de notes du mois (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) en utilisant le modèle ci-dessous. | Fichier de notes |
| Localisation PM | Actualiser `docs/source/sdk/android/i18n_plan.md#translation-status` et le journal du personnel ; pré-remplir les décisions proposées. | plan i18n |
| DX TL | Exécutez `ci/check_android_docs_i18n.sh` ou `scripts/sync_docs_i18n.py --dry-run` et joignez des résumés pour discussion. | Artefacts CI |
| Outils de documentation | Exportez les résumés `docs/i18n/manifest.json` + la liste des tickets en attente de `docs/source/sdk/android/i18n_requests/`. | Résumé du manifeste et du ticket |
| Assistance/Version | Rassemblez toutes les escalades qui nécessitent une action Docs/DevRel (par exemple, des invitations en attente d'aperçu, le blocage des commentaires des réviseurs). | Status.md ou document d'escalade |

## Blocs d'agenda1. **Appel et objectifs (5min)**
   - Confirmer le quorum, le scribe et la logistique.
   - Mettez en surbrillance tout incident urgent (panne d'aperçu des documents, blocage de localisation).
2. **Revue du personnel de localisation (15 min)**
   - Examiner le journal des décisions de dotation en personnel
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Confirmer l'état des bons de commande ouverts (`DOCS-L10N-*`) et la couverture provisoire.
   - Comparez la sortie de fraîcheur du CI avec le tableau d'état de la traduction ; appeler n'importe qui
     doc dont le SLA local (> 5 jours ouvrables) sera violé avant le prochain
     synchroniser.
   - Décider si une escalade est nécessaire (Product Ops, Finance, entrepreneur)
     gestion). Consigner la décision dans le journal de dotation et dans le rapport mensuel
     minutes, y compris propriétaire + date d'échéance.
   - Si le personnel est sain, documentez la confirmation afin que l'action de la feuille de route puisse
     revenons à 🈺/🈴 avec des preuves.
3. **Mises à jour des documents/feuille de route (10 min)**
   - État du travail du portail DOCS-SORA, du proxy Try-It et de la publication SoraFS
     préparation.
   - Mettez en surbrillance la dette de documents ou les réviseurs nécessaires pour les trains de versions actuels.
4. **Points forts du SDK (10 min)**
   - Préparation de la documentation Android AND5/AND7, parité Swift IOS5, progression JS GA.
   - Capturez les appareils partagés ou les différences de schéma qui affecteront les documents.
5. **Revue d'action et parking (5min)**
   - Revisitez les éléments ouverts de la synchronisation précédente ; confirmer les fermetures.
   - Enregistrez les nouvelles actions dans le fichier de notes avec les propriétaires et les délais explicites.

## Modèle d'examen du personnel de localisation

Incluez le tableau suivant dans les minutes de chaque mois :

| Paramètres régionaux | Capacité (ETP) | Engagements & PO | Risques / Escalades | Décision et propriétaire |
|--------|----------------|---------|-----------|------------------|
| JP | par exemple, 0,5 entrepreneur + 0,1 sauvegarde de documents | PO `DOCS-L10N-4901` (en attente de signature) | « Contrat non signé d'ici le 04/03/2026 » | « Passer aux opérations produit — @docs-devrel, prévu le 02/03/2026 » |
| IL | par exemple, 0.1 Ingénieur Docs | La rotation entre en prise de force 2026-03-18 | « Besoin d'un réviseur de sauvegarde » | « @docs-lead identifiera la sauvegarde d'ici le 05/03/2026 » |

Enregistrez également un court récit couvrant :

- **Perspectives SLA :** Tout document susceptible de manquer le SLA de cinq jours ouvrables et le
  atténuation (priorité d'échange, recrutement d'un fournisseur de sauvegarde, etc.).
- **Santé des tickets et des actifs :** Entrées exceptionnelles dans
  `docs/source/sdk/android/i18n_requests/` et si les captures d'écran/actifs sont
  prêt pour les traducteurs.

### Journalisation des examens du personnel de localisation

- **Procès-verbal :** Copiez le tableau des effectifs + le récit dans
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (tous les paramètres régionaux reflètent le
  minutes en anglais via des fichiers localisés dans le même répertoire). Lier l'entrée
  retour à l'ordre du jour (`docs/source/docs_devrel/monthly_sync_agenda.md`) donc
  la gouvernance peut retracer les preuves.
- **Plan i18n :** Mettre à jour le journal des décisions de dotation en personnel et le tableau d'état de la traduction
  dans `docs/source/sdk/android/i18n_plan.md` immédiatement après la réunion.
- **Statut :** Lorsque les décisions en matière de personnel affectent les portes de la feuille de route, ajoutez une courte entrée dans
  `status.md` (section Docs/DevRel) référençant le fichier des minutes et le plan i18n
  mise à jour.

## Modèle de procès-verbal

Copiez ce squelette dans `docs/source/docs_devrel/minutes/<yyyy-mm>.md` :

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```Publiez les notes via PR peu de temps après la réunion et liez-les à partir de `status.md`
lorsqu’on fait référence à des risques ou à des décisions en matière de dotation.

## Attentes de suivi

1. **Minutes engagées :** dans les 24 heures (`docs/source/docs_devrel/minutes/`).
2. **Plan i18n mis à jour :** ajustez le journal des effectifs et le tableau de traduction pour
   refléter de nouveaux engagements ou des escalades.
3. **Entrée Status.md :** résumez toutes les décisions à haut risque pour conserver la feuille de route
   en synchronisation.
4. **Escalades classées :** lorsque l'examen demande une escalade, créez/actualisez
   le ticket concerné (par exemple, Opérations produit, approbation financière, intégration du fournisseur)
   et référencez-le à la fois dans le procès-verbal et dans le plan i18n.

En suivant cet agenda, l'exigence de la feuille de route d'inclure la localisation
les examens du personnel dans la synchronisation mensuelle Docs/DevRel restent vérifiables et en aval
les équipes savent toujours où trouver les preuves.