---
lang: fr
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Attestation SBOM et de provenance — SDK Android

| Champ | Valeur |
|-------|-------|
| Portée | SDK Android (`java/iroha_android`) + exemples d'applications (`examples/android/*`) |
| Propriétaire du flux de travail | Ingénierie des versions (Alexei Morozov) |
| Dernière vérification | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Workflow de génération

Exécutez le script d'assistance (ajouté pour l'automatisation AND6) :

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Le script effectue les opérations suivantes :

1. Exécute `ci/run_android_tests.sh` et `scripts/check_android_samples.sh`.
2. Appelle le wrapper Gradle sous `examples/android/` pour créer des SBOM CycloneDX pour
   `:android-sdk`, `:operator-console` et `:retail-wallet` avec le
   `-PversionName`.
3. Copie chaque SBOM dans `artifacts/android/sbom/<sdk-version>/` avec les noms canoniques
   (`iroha-android.cyclonedx.json`, etc.).

## 2. Provenance et signature

Le même script signe chaque SBOM avec `cosign sign-blob --bundle <file>.sigstore --yes`
et émet `checksums.txt` (SHA-256) dans le répertoire de destination. Définissez le `COSIGN`
variable d'environnement si le binaire réside en dehors de `$PATH`. Une fois le script terminé,
enregistrez les chemins du bundle/somme de contrôle ainsi que l'identifiant d'exécution de Buildkite dans
`docs/source/compliance/android/evidence_log.csv`.

## 3. Vérification

Pour vérifier un SBOM publié :

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Comparez le SHA de sortie à la valeur répertoriée dans `checksums.txt`. Les réviseurs comparent également le SBOM à la version précédente pour garantir que les deltas de dépendance sont intentionnels.

## 4. Aperçu des preuves (2026-02-11)

| Composant | SBOM | SHA-256 | Ensemble Sigstore |
|---------------|------|---------|-----------------|
| SDK Android (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | Bundle `.sigstore` stocké à côté de SBOM |
| Exemple de console opérateur | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Échantillon de portefeuille de vente au détail | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Les hachages capturés à partir de Buildkite exécutent `android-sdk-release#4821` ; reproduisez via la commande de vérification ci-dessus.)*

## 5. Travail remarquable

- Automatisez les étapes SBOM + cosign dans le pipeline de publication avant GA.
- Mettre en miroir les SBOM dans le compartiment d'artefacts publics une fois que AND6 marque la liste de contrôle comme terminée.
- Coordonnez-vous avec Docs pour lier les emplacements de téléchargement SBOM à partir des notes de version destinées aux partenaires.