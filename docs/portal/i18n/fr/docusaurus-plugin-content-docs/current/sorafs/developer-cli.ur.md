---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
titre : Livre de recettes CLI SoraFS
sidebar_label : livre de recettes CLI
description : Surface `sorafs_cli` consolidée - Procédure pas à pas axée sur les tâches
---

:::note مستند ماخذ
:::

Surface `sorafs_cli` consolidée (avec la caisse `sorafs_car` et la fonctionnalité `cli` et les artefacts SoraFS تیار کرنے کے لیے درکار ہر قدم exposer کرتا ہے۔ Il s'agit d'un livre de recettes et de flux de travail pour tous les utilisateurs. contexte opérationnel pour un pipeline de manifeste et des runbooks d'orchestrateur pour une paire de fichiers

## Charges utiles des packages

Archives CAR déterministes et plans de fragments pour `car pack` Il s'agit d'une poignée de commande et d'un chunker SF-1.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Descripteur de chunker par défaut : `sorafs.sf1@1.0.0`.
- Entrées du répertoire ordre lexicographique et marche et sommes de contrôle et sommes stables
- Résumé JSON pour les résumés de charge utile, les métadonnées de fragments et le registre/orchestrateur pour le CID racine et le CID racine.

## Construire des manifestes

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Options `--pin-*` pour les champs `sorafs_manifest::ManifestBuilder` et les champs `PinPolicy` sur la carte.
- `--chunk-plan` est utilisé pour la soumission CLI et le résumé de fragments SHA3 pour le calcul des tâches ورنہ وہ résumé میں embed شدہ digest réutilisation کرتا ہے۔
- Charge utile de sortie JSON Norito pour les commentaires et les différences entre les deux## Signer les manifestes sans clés de longue durée

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Jetons en ligne, variables d'environnement et sources basées sur des fichiers.
- Métadonnées de provenance (`token_source`, `token_hash_hex`, chunk digest) pour le JWT brut et le JWT brut pour le `--include-token=true`. ہو۔
- CI prend en charge le projet : GitHub Actions OIDC et `--identity-token-provider=github-actions` pour le projet.

## Soumettre les manifestes à Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Preuves d'alias pour le décodage Norito et Torii pour le décodage POST et le résumé du manifeste et la correspondance avec le manifeste.
- Planifier un fragment de résumé SHA3 pour calculer des attaques par discordance et des attaques par disparité
- Résumés des réponses pour l'audit et le statut HTTP, les en-têtes et les charges utiles du registre

## Vérifier le contenu et les preuves du CAR

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Arbre PoR pour les résumés de charge utile et le résumé du manifeste pour comparer les détails
- Preuves de réplication et gouvernance et soumission des décomptes et capture des identifiants.

## Télémétrie à l'épreuve des flux

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- La preuve en streaming et les éléments NDJSON émettent des émissions (`--emit-events=false` et replay)
- Nombre de réussites/échecs, histogrammes de latence, échecs échantillonnés et résumé JSON et agrégation de journaux de tableaux de bord et de scraping pour les mises à jour de données
- Un rapport de défaillance de la passerelle et une vérification PoR locale (`--por-root-hex` en anglais) les preuves rejettent une sortie non nulle et une sortie non nulle. répétitions comme `--max-failures` et `--max-verification-failures` et réglage des seuils
- Le PoR et le support sont disponibles PDP et PoTR SF-13/SF-14 pour enveloppe et réutilisation
- Résumé rendu `--governance-evidence-dir`, métadonnées (horodatage, version CLI, URL de la passerelle, résumé du manifeste) et copie du manifeste du répertoire du répertoire des paquets de gouvernance et des preuves de flux de preuves et exécution des paquets de gouvernance. کیے بغیر archive کر سکیں۔

## Références supplémentaires

- `docs/source/sorafs_cli.md` — Drapeaux de sécurité pour les drapeaux
- `docs/source/sorafs_proof_streaming.md` — schéma de télémétrie de preuve et modèle de tableau de bord Grafana۔
- `docs/source/sorafs/manifest_pipeline.md` — chunking, composition du manifeste, et traitement CAR