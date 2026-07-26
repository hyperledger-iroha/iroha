---
lang: fr
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Preuve d'attestation StrongBox — Déploiements au Japon

| Champ | Valeur |
|-------|-------|
| Fenêtre d'évaluation | 2026-02-10 – 2026-02-12 |
| Emplacement des artefacts | `artifacts/android/attestation/<device-tag>/<date>/` (format groupé selon `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Outillage de capture | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Réviseurs | Responsable du laboratoire matériel, conformité et juridique (JP) |

## 1. Procédure de capture

1. Sur chaque appareil répertorié dans la matrice StrongBox, générez un challenge et capturez le bundle d'attestation :
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Validez les métadonnées du bundle (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) dans l'arborescence des preuves.
3. Exécutez l'assistant CI pour revérifier tous les bundles hors ligne :
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Résumé de l'appareil (2026-02-12)

| Étiquette de périphérique | Modèle / StrongBox | Chemin du paquet | Résultat | Remarques |
|------------|--------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6/Tenseur G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Réussi (soutenu par matériel) | Défi lié, patch du système d'exploitation 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7/Tenseur G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Réussi | Candidat à la voie CI principale ; température dans les spécifications. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro/Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Réussi (retest) | Hub USB-C remplacé ; Buildkite `android-strongbox-attestation#221` a capturé le paquet de passage. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Réussi | Profil d'attestation Knox importé le 09/02/2026. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Réussi | Profil d'attestation Knox importé ; La voie CI est désormais verte. |

Les balises de périphérique correspondent à `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Liste de contrôle des évaluateurs

- [x] Vérifiez que `result.json` affiche `strongbox_attestation: true` et la chaîne de certificats vers la racine de confiance.
- [x] Confirmez que les octets de défi correspondent. Buildkite exécute `android-strongbox-attestation#219` (balayage initial) et `#221` (retest Pixel 8 Pro + capture S24).
- [x] Réexécutez la capture du Pixel 8 Pro après le correctif matériel (propriétaire : responsable du laboratoire matériel, terminé le 13/02/2026).
- [x] Capture complète du Galaxy S24 une fois l'approbation du profil Knox arrivée (propriétaire : Device Lab Ops, terminé le 13/02/2026).

## 4. Répartition

- Joignez ce résumé ainsi que le dernier fichier texte du rapport aux paquets de conformité des partenaires (liste de contrôle FISC § Résidence des données).
- Chemins de bundle de référence lors de la réponse aux audits des régulateurs ; ne transmettez pas de certificats bruts en dehors des canaux cryptés.

## 5. Journal des modifications

| Dates | Changement | Auteur |
|------|--------|--------|
| 2026-02-12 | Capture initiale du bundle JP + rapport. | Opérations du laboratoire d'appareils |