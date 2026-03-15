---
lang: fr
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Liste de contrôle des contrôles de sécurité FISC — SDK Android

| Champ | Valeur |
|-------|-------|
| Version | 0,1 (2026-02-12) |
| Portée | Android SDK + outils opérateur utilisés dans les déploiements financiers japonais |
| Propriétaires | Conformité et droit (Daniel Park), responsable du programme Android |

## Matrice de contrôle

| Contrôle FISC | Détails de mise en œuvre | Preuves / Références | Statut |
|--------------|-------------|-----------------------|--------|
| **Intégrité de la configuration du système** | `ClientConfig` applique le hachage du manifeste, la validation du schéma et l'accès à l'exécution en lecture seule. Les échecs de rechargement de la configuration émettent des événements `android.telemetry.config.reload` documentés dans le runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` ; `docs/source/android_runbook.md` §1–2. | ✅ Mis en œuvre |
| **Contrôle d'accès et authentification** | Le SDK respecte les politiques TLS Torii et les requêtes signées `/v1/pipeline` ; Référence des flux de travail de l'opérateur Prise en charge du Playbook §4–5 pour l'escalade et le remplacement du déclenchement via des artefacts Norito signés. | `docs/source/android_support_playbook.md` ; `docs/source/sdk/android/telemetry_redaction.md` (flux de travail de remplacement). | ✅ Mis en œuvre |
| **Gestion des clés cryptographiques** | Les fournisseurs privilégiés par StrongBox, la validation des attestations et la couverture matricielle des appareils garantissent la conformité KMS. Sorties du faisceau d'attestation archivées sous `artifacts/android/attestation/` et suivies dans la matrice de préparation. | `docs/source/sdk/android/key_management.md` ; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Mis en œuvre |
| **Journalisation, surveillance et conservation** | La politique de rédaction de télémétrie hache les données sensibles, regroupe les attributs des appareils et applique la conservation (fenêtres de 7/30/90/365 jours). Support Playbook §8 décrit les seuils du tableau de bord ; remplacements enregistrés dans `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md` ; `docs/source/android_support_playbook.md` ; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Mis en œuvre |
| **Opérations et gestion du changement** | La procédure de basculement GA (Support Playbook §7.2) et les mises à jour `status.md` suivent l'état de préparation de la publication. Preuve de publication (SBOM, bundles Sigstore) liée via `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md` ; `status.md` ; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Mis en œuvre |
| **Réponse aux incidents et rapports** | Playbook définit la matrice de gravité, les fenêtres de réponse SLA et les étapes de notification de conformité ; les remplacements de télémétrie + les répétitions du chaos assurent la reproductibilité avant les pilotes. | `docs/source/android_support_playbook.md` §§4–9 ; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Mis en œuvre |
| **Résidence / localisation des données** | Les collecteurs de télémétrie pour les déploiements JP fonctionnent dans la région approuvée de Tokyo ; Ensembles d'attestations StrongBox stockés dans la région et référencés à partir des tickets partenaires. Le plan de localisation garantit que les documents sont disponibles en japonais avant la version bêta (AND5). | `docs/source/android_support_playbook.md` §9 ; `docs/source/sdk/android/developer_experience_plan.md` §5 ; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 En cours (localisation en cours) |

## Notes du réviseur

- Vérifiez les entrées de la matrice des appareils pour le Galaxy S23/S24 avant l'intégration des partenaires réglementés (voir les lignes du document de préparation `s23-strongbox-a`, `s24-strongbox-a`).
- Assurez-vous que les collecteurs de télémétrie dans les déploiements JP appliquent la même logique de rétention/remplacement définie dans la DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Obtenez la confirmation des auditeurs externes une fois que les partenaires bancaires ont examiné cette liste de contrôle.