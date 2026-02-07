---
lang: fr
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2026-01-03T18:07:59.250950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Journal d'urgence du laboratoire d'appareils

Enregistrez ici chaque activation du plan d’urgence du laboratoire d’appareils Android.
Incluez suffisamment de détails pour les examens de conformité et les futurs audits de préparation.

| Dates | Déclencheur | Mesures prises | Suivis | Propriétaire |
|------|---------|---------------|------------|-------|
| 2026-02-11 | La capacité est tombée à 78 % après la panne de la voie Pixel8 Pro et le retard de la livraison du Pixel8a (voir `android_strongbox_device_matrix.md`). | Promotion de la voie Pixel7 au rang de cible principale de CI, emprunt d'une flotte Pixel6 partagée, planification de tests de fumée du Firebase Test Lab pour un échantillon de portefeuille de vente au détail et engagement d'un laboratoire StrongBox externe conformément au plan AND6. | Remplacez le hub USB-C défectueux pour Pixel8 Pro (dû le 15/02/2026) ; confirmer l'arrivée du Pixel8a et le rapport de capacité de rebaseline. | Responsable du laboratoire de matériel |
| 2026-02-13 | Hub Pixel8 Pro remplacé et approuvé GalaxyS24, rétablissant la capacité à 85 %. | Retour de la voie Pixel7 au secondaire, réactivation du travail `android-strongbox-attestation` Buildkite avec les balises `pixel8pro-strongbox-a` et `s24-strongbox-a`, matrice de préparation mise à jour + journal des preuves. | Surveiller l’ETA de livraison du Pixel8a (toujours en attente) ; conserver l'inventaire des moyeux de rechange documenté. | Responsable du laboratoire de matériel |