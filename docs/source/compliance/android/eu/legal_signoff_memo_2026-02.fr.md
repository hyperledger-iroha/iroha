---
lang: fr
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Mémo d'approbation juridique de l'UE — 2026.1 GA (SDK Android)

## Résumé

- **Version/Formation :** 2026.1 GA (SDK Android)
- **Date de révision :** 2026-04-15
- **Conseil/Réviseur :** Sofia Martins — Conformité et droit
- **Portée :** Cible de sécurité ETSI EN 319 401, résumé DPIA RGPD, attestation SBOM, preuves d'urgence AND6 en laboratoire d'appareils
- **Tickets associés :** `_android-device-lab` / AND6-DR-202602, tracker de gouvernance AND6 (`GOV-AND6-2026Q1`)

## Liste de contrôle des artefacts

| Artefact | SHA-256 | Localisation / Lien | Remarques |
|--------------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Correspond aux identifiants de la version 2026.1 GA et aux deltas du modèle de menace (ajouts Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Références Politique de télémétrie AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | Ensemble `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | CycloneDX + provenance revue ; correspond au travail Buildkite `android-sdk-release#4821`. |
| Journal des preuves | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (ligne `android-device-lab-failover-20260220`) | Confirme les hachages de bundle capturés dans le journal + instantané de capacité + entrée de mémo. |
| Offre groupée d'urgence appareil-laboratoire | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hachage extrait de `bundle-manifest.json` ; le ticket AND6-DR-202602 a été transféré au service juridique/conformité. |

## Résultats et exceptions

- Aucun problème de blocage identifié. Les artefacts sont conformes aux exigences ETSI/RGPD ; Parité de télémétrie AND7 notée dans le résumé DPIA et aucune atténuation supplémentaire n’est requise.
- Recommandation : surveiller l'exercice DR-2026-05-Q2 programmé (ticket AND6-DR-202605) et ajouter l'ensemble résultant au journal des preuves avant le prochain point de contrôle de gouvernance.

## Approbation

- **Décision :** Approuvée
- **Signature / Horodatage :** _Sofia Martins (signé numériquement via le portail de gouvernance, 2026-04-15 14:32 UTC)_
- **Propriétaires suivants :** Device Lab Ops (livrer l'ensemble de preuves DR-2026-05-Q2 avant le 31/05/2026)