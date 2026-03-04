---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo de convites fait un aperçu

## Objet

L'élément de la feuille de route **DOCS-SORA** prévoit l'intégration des réviseurs et le programme des invités pour l'aperçu public comme les derniers bloqueurs avant le portail en version bêta. Cette page décrit comment ouvrir chaque onda de convites, quais artefatos doivent être envoyés antes de mandat convites et comment prouver que le flux et l'audit. Utilisez Juntocom :

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) pour le travail du réviseur.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) pour garantir la somme de contrôle.
-[`devportal/observability`](./observability.md) pour les exportations de télémétrie et de crochets d'alerte.

## Plan d'onde| Onde | Audience | Critères d'entrée | Critères de dite | Notes |
| --- | --- | --- | --- | --- |
| **W0 - Noyau des responsables** | Les responsables de Docs/SDK valident le contenu de cette journée. | Heure GitHub `docs-portal-preview` remplie, porte de contrôle `npm run serve` verte, Alertmanager silencieux pendant 7 jours. | Tous les documents P0 révisés, backlog tagueado, sem incidents bloqueadores. | Utilisé pour valider le flux ; Dans l'e-mail de la personne invitée, il suffit de partager les articles d'aperçu. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii, réviseurs de gouvernance sur NDA. | W0 encerrado, termos legais aprovados, proxy Try-it em staging. | Signature des partenaires coletado (issue ou formulario assinado), telemetria mostra =2 versions de la documentation envoyées via le pipeline d'aperçu sans restauration. | Limitar convites concorrentes (<=25) et agrupar semanalement. |

Documentez l'onde actuelle sur `status.md` et aucun suivi des demandes de prévisualisation pour que le gouvernement voie l'état rapidement.

## Checklist de contrôle en amontConclua estas acoes **ante** de agenda convites para uma onda:

1. **Artefatos de CI disponiveis**
   - Ultimo `docs-portal-preview` + descripteur envoyé par `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS annoté sur `docs/portal/docs/devportal/deploy-guide.md` (descripteur de basculement présent).
2. **Application de la somme de contrôle**
   - `docs/portal/scripts/serve-verified-preview.mjs` invoqué via `npm run serve`.
   - Instructions de `scripts/preview_verify.sh` testées sur macOS + Linux.
3. **Base de télémétrie**
   - `dashboards/grafana/docs_portal.json` montre le trafic Essayez-le, saudavel et l'alerte `docs.preview.integrity` est verte.
   - Dernière annexe de `docs/portal/docs/devportal/observability.md` actualisée avec les liens vers Grafana.
4. **Artefatos de gouvernance**
   - Le problème est d'inviter le tracker immédiatement (un problème par ici).
   - Modèle de registre des réviseurs copiés (version [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovacoes legais e de SRE requeridas anexadas a issue.

Enregistrez la conclusion du contrôle en amont sans suivi d'invitation avant d'envoyer n'importe quel e-mail.

## Étapes du flux1. **Candidats sélectionnés**
   - Puxar da planilha de espera ou fila de partenaires.
   - Garantir que chaque candidat ait un modèle de sollicitation complet.
2. **Accès Aprovar**
   - Attribuer à un fournisseur un problème de suivi des invitations.
   - Vérifier les prérequis (CLA/contrato, uso aceitavel, brief de seguranca).
3. **Enviar invite**
   - Préencher les espaces réservés de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contacts).
   - Anexar o descriptor + hash do archive, URL de staging do Try it, et canais de support.
   - Garder l'e-mail final (ou la transcription de Matrix/Slack) du problème.
4. **Intégration d'Acompanhar**
   - Activer le tracker d'invitation avec `invite_sent_at`, `expected_exit_at`, et le statut (`pending`, `active`, `complete`, `revoked`).
   - Linkar a sollicitacao de entrada do revisor para auditabilidade.
5. **Surveiller la télémétrie**
   - Observez `docs.preview.session_active` et alertez `TryItProxyErrors`.
   - Ouvrir un incident en utilisant la télémétrie de base et en enregistrant le résultat à l'entrée de l'invité.
6. **Coletar feedback et encerrar**
   - Encerrar convites quando o feedback chegar ou `expected_exit_at` expirer.
   - Atualizar a issue da onda com um reumo curto (achados, incidents, proximas acoes) antes de passar para a proxima coorte.

## Preuves et rapports| Artefato | Onde armazenar | Cadence de mise à jour |
| --- | --- | --- |
| Problème de suivi des invitations | Projet GitHub `docs-portal-preview` | Mettre à jour après chaque convite. |
| Exporter la liste des réviseurs | Registre confirmé par `docs/portal/docs/devportal/reviewer-onboarding.md` | Sémanal. |
| Instantanés de télémétrie | `docs/source/sdk/android/readiness/dashboards/<date>/` (réutiliser le bundle de télémétrie) | Por onda + apos incidents. |
| Résumé des retours d'expérience | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (crier des pâtes par onda) | Dentro de 5 dias apos a saya da onda. |
| Note de réunion de gouvernance | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Préencher avant chaque synchronisation DOCS-SORA. |

Exécuter `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
Apos cada lote para produzir um digest legivel por maquina. Anexe o JSON renderizado a issue onda pour que les réviseurs de gouvernance confirment qu'ils sont contagieux et invités à reproduire tout le journal.

Annexe à la liste des preuves du `status.md` toujours qu'une seule chose se termine pour que l'entrée de la feuille de route puisse être mise à jour rapidement.

## Critères de restauration et de pause

Mettez en pause le flux des invités (et notifiez la gouvernance) lorsque l'un des deux articles tombe en panne :

- Incident de proxy Essayez-le qui nécessite une restauration (`npm run manage:tryit-proxy`).
- Fadiga d'alertes : >3 pages d'alerte pour les points finaux, aperçus en 7 jours.
- Écart de conformité : convite enviado sem termos assinados ou sem registrar o template de sollicitacao.
- Risque d'intégrité : non-concordance de la somme de contrôle détectée par `scripts/preview_verify.sh`.Revenez ensuite sur la résolution du problème du tracker d'invitation et confirmez que le tableau de bord de télémétrie est en cours dans moins de 48 heures.