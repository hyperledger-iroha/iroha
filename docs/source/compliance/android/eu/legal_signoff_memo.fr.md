---
lang: fr
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Modèle de mémo de signature juridique de l'UE

Cette note enregistre la révision juridique requise par l'élément de la feuille de route **AND6** avant la
Le paquet d’artefacts de l’UE (ETSI/GDPR) est soumis aux régulateurs. L'avocat devrait cloner
ce modèle par version, remplissez les champs ci-dessous et stockez la copie signée
aux côtés des artefacts immuables référencés dans le mémo.

## Résumé

- **Libération / Entraînement :** `<e.g., 2026.1 GA>`
- **Date de révision :** `<YYYY-MM-DD>`
- **Conseil/Réviseur :** `<name + organisation>`
- **Portée :** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Billets associés :** `<governance or legal issue IDs>`

## Liste de contrôle des artefacts

| Artefact | SHA-256 | Localisation / Lien | Remarques |
|--------------|---------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + archives de gouvernance | Confirmez les identifiants de version et les ajustements du modèle de menace. |
| `gdpr_dpia_summary.md` | `<hash>` | Même répertoire/miroirs de localisation | Assurez-vous que les références à la politique de rédaction correspondent à `sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | Même répertoire + ensemble de cosignature dans le compartiment de preuves | Vérifiez les signatures de provenance CycloneDX +. |
| Ligne du journal des preuves | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Numéro de ligne `<n>` |
| Offre groupée d'urgence appareil-laboratoire | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Confirme la répétition de basculement liée à cette version. |

> Joignez des lignes supplémentaires si le paquet contient plus de fichiers (par exemple, confidentialité
> annexes ou traductions DPIA). Chaque artefact doit référencer son immuable
> télécharger la cible et le travail Buildkite qui l'a produit.

## Résultats et exceptions

- `None.` *(Remplacer par une liste à puces couvrant les risques résiduels, compensant
  contrôles ou actions de suivi requises.)*

## Approbation

- **Décision :** `<Approved / Approved with conditions / Blocked>`
- **Signature / Horodatage :** `<digital signature or email reference>`
- **Propriétaires suivants :** `<team + due date for any conditions>`

Téléchargez le mémo final dans le compartiment de preuves de gouvernance, copiez le SHA-256 dans
`docs/source/compliance/android/evidence_log.csv` et liez le chemin de téléchargement dans
`status.md`. Si la décision est « Bloquée », remontez vers le pilotage AND6.
comité et documenter les étapes de remédiation dans la liste critique de la feuille de route et dans le
journal d'urgence du laboratoire de l'appareil.