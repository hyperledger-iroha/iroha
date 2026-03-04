---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Intégration des réviseurs avec aperçu

## Visa général

DOCS-SORA accompagne le lancement dans les phases du portail des utilisateurs. Construit avec la porte de la somme de contrôle
(`npm run serve`) et flux Essayez-le renforcés à proximité du cadre :
l'intégration des réviseurs validés avant l'aperçu public s'ouvrira également. Est-ce guide
décrire comment collaborer avec les sollicitations, vérifier l'éligibilité, fournir l'accès et faciliter le départ
de participants com seguranca. Consultez o
[prévisualiser le flux d'invitation](./preview-invite-flow.md) pour le plan de coordonnées, un
cadence des invités et exportations de télémétrie ; os passos abaixo focam nas acoes
à tomar quand un réviseur a été sélectionné.

- **Escopo :** révise avec précision l'accès à l'aperçu des documents (`docs-preview.sora`,
  builds des pages GitHub ou des bundles de SoraFS) avant GA.
- **Fora do escopo :** opérateurs de Torii ou SoraFS (cobertos por seus proprios kits de onboarding)
  les implants font le portail dans la production (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Papeis et prérequis| Papier | Objets typiques | Artefatos requis | Notes |
| --- | --- | --- | --- |
| Responsable du noyau | Vérifiez de nouveaux guides, exécutez des tests de fumée. | Poignée GitHub, contact Matrix, CLA supprimé dans l'archive. | En général, c'est pas le moment GitHub `docs-preview`; ainda assim enregistrer une sollicitation pour que l'accès soit auditavel. |
| Réviseur partenaire | Valider les extraits du SDK ou le contenu de la gouvernance avant la publication. | Email corporativo, POC legal, termos de preview assinados. | Deve reconhecer requisitos de telemetria + tratamento de dados. |
| Bénévole communautaire | Fornecer feedback d’utilisabilité sur les guides. | GitHub handle, contact préféré, fuso horario, Aceitacao do CoC. | Mantenha coortes pequenas; donner la priorité aux réviseurs qui assinaram o acordo de contribuicao. |

Tous les types de réviseurs doivent :

1. Reconnaître la politique d'utilisation de l'huile pour les articles de prévisualisation.
2. Voir les annexes de sécurité/observabilité
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Concordar em executar `docs/portal/scripts/preview_verify.sh` avant de servir quoi que ce soit
   instantané localement.

## Flux d'admission1. Demandez à la sollicitante que vous preencha o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (ou copier/coller dans un numéro). Capturer à moindre coût : identité, méthode de contact,
   Poignée GitHub, données préliminaires de révision et de confirmation des documents de sécurité pour le moment.
2. Enregistrer une sollicitation sans tracker `docs-preview` (émettre GitHub ou ticket de gouvernance)
   et attribuer un fournisseur.
3. Conditions préalables à la validation :
   - CLA / accord de contribution à l'archive (ou référence du partenaire contractuel).
   - Reconhecimento de uso aceitavel armazenado na sollicitacao.
   - Avaliação de risco completa (par exemple, réviseurs partenaires agréés par Legal).
4. Le fournisseur doit signer la sollicitation et résoudre le problème du suivi de toute entrée
   gestion du changement (exemple : `DOCS-SORA-Preview-####`).

## Provisionnement et ferramentas

1. **Compartilhar artefatos** - Fornecer o descriptor + archivage de prévisualisation plus récent du workflow
   de CI ou do pin SoraFS (artefato `docs-portal-preview`). Lire les réviseurs de l'exécutant :

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec l'application de la somme de contrôle** - Afficher les réviseurs pour la commande avec la porte de la somme de contrôle :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Est-ce que je réutilise `scripts/serve-verified-preview.mjs` pour que nenhum build ne soit pas vérifié
   seja iniciado por acidente.3. **Concevoir l'accès à GitHub (facultatif)** - Réviser précisément les branches non publiées,
   Ajouter le temps GitHub `docs-preview` lors de la révision et de l'enregistrement de la gestion des adhésions
   sur sollicitation.

4. **Communiquer les canaux d'assistance** - Partager le contact de garde (Matrix/Slack) et la procédure
   de incidents de [`incident-runbooks`](./incident-runbooks.md).

5. **Télémétrie + feedback** - Afficher les réviseurs que les analyses sont anonymisées et collectées
   (version [`observability`](./observability.md)). Fornecer le formulaire de commentaires ou le modèle de problème
   cité no convite e registrar o evento com o helper
   [`preview-feedback-log`](./preview-feedback-log) pour maintenir le résumé de l'onde actualisée.

## Liste de contrôle du réviseur

Avant d'accéder à l'aperçu, les révisions doivent être complètes :

1. Vérifiez les artefatos baixados (`preview_verify.sh`).
2. Lancez le portail via `npm run serve` (ou `serve:verified`) pour garantir que la garde de somme de contrôle est active.
3. Lire les notes de sécurité et d'observation des valeurs acima.
4. Testez une console OAuth/Try it en utilisant la connexion par code de périphérique (selon l'application) et évitez de réutiliser les jetons de production.
5. Le registraire achados no tracker acordado (issue, doc compartilhado ou formulario) et taguea-los com
   o tag de release faire un aperçu.

## Responsabilités des mainteneurs et des départs| Phase | Acoès |
| --- | --- |
| Coup d'envoi | Confirmez qu'une liste de contrôle d'admission est incluse dans la sollicitation, partagez les articles + instructions, ajoutez une entrée `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), et programmez une synchronisation de ma période pour une révision qui dure plus d'une semaine. |
| Surveillance | Surveiller la télémétrie de prévisualisation (procure trafego Try it incomum, falhas de sonde) et suivre le runbook des incidents si quelque chose est suspecté. Les événements du registraire `feedback-submitted`/`issue-opened` sont conformes aux achats effectués pour maintenir les mesures de chaque précision. |
| Débarquement | Accédez temporairement à GitHub ou SoraFS, registraire `access-revoked`, obtenez une sollicitation (incluant un CV de commentaires + des liens pendants), et actualisez le registre des réviseurs. Solliciter le réviseur qui supprime les builds locaux et annexer le résumé généré à partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez le même processus pour effectuer une rotation des réviseurs entre les différentes équipes. Manter o rastro no repo (numéro + modèles)
ajuste le DOCS-SORA à un audit permanent et permet à la gouvernance de confirmer l'accès à l'aperçu
suivez les contrôles documentés.

## Modèles de convocation et de suivi- Inicie todo outreach com o archive
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Il capture le minimum de langue légale, instruit la somme de contrôle de l'aperçu et l'attente de ce
  les réviseurs reconhecam a politica de uso aceitavel.
- Pour éditer le modèle, remplacer les espaces réservés de `<preview_tag>`, `<request_ticket>` et les canaux de contact.
  Gardez une copie du message final sans ticket d'admission pour que les réviseurs, les aménageurs et les auditeurs puissent le faire
  référence au texte exato envoyé.
- Après avoir envoyé ou invité, actualisez un plan de suivi ou émettez un horodatage `invite_sent_at` et des données
  de encerramento attendu pour que le rapport
  [Aperçu du flux d'invitation](./preview-invite-flow.md) vous pouvez identifier automatiquement la côte.