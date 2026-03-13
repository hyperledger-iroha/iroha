---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
titre : Recettes CLI SoraFS
sidebar_label : Recettes CLI
description : Parcours orienté tâches de la surface consolidée `sorafs_cli`.
---

:::note Source canonique
:::

La surface consolidée `sorafs_cli` (fournie par le crate `sorafs_car` avec la fonctionnalité `cli` activée) expose chaque étape nécessaire pour préparer les artefacts SoraFS. Utilisez ce livre de recettes pour aller directement aux workflows courants ; associez-le au pipeline de manifest et aux runbooks de l'orchestrateur pour le contexte opérationnel.

## Empaqueter les payloads

Utilisez `car pack` pour produire des archives CAR déterministes et des plans de chunk. La commande sélectionne automatiquement le chunker SF-1 sauf si une poignée est fournie.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Poignée de chunker par défaut : `sorafs.sf1@1.0.0`.
- Les entrées de répertoire sont parcourues en ordre lexicographique afin que les sommes de contrôle restent stables entre les plateformes.
- Le résumé JSON inclut les digests de payload, les métadonnées par chunk et le CID racine reconnu par le registre et l'orchestrateur.

## Construire les manifestes

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Les options `--pin-*` mappent directement vers les champs `PinPolicy` dans `sorafs_manifest::ManifestBuilder`.
- Fournissez `--chunk-plan` lorsque vous souhaitez que le CLI recalcule le digest SHA3 de chunk avant soumission ; sinon il réutilise le résumé intégré au CV.
- La sortie JSON reflète le payload Norito pour des différences simples lors des revues.

## Signer les manifestes sans clés de longue durée

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Acceptez les tokens en ligne, les variables d'environnement ou les sources basées sur les fichiers.
- Ajout des métadonnées de provenance (`token_source`, `token_hash_hex`, digest de chunk) sans persister le JWT brut sauf si `--include-token=true`.
- Fonctionne bien en CI : combinez avec OIDC de GitHub Actions en définissant `--identity-token-provider=github-actions`.

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

- Effectue le décodage Norito des alias proofs et vérifie qu'ils correspondent au digest du manifest avant POST vers Torii.
- Recalculez le digest SHA3 de chunk à partir du plan pour éviter les attaques par mismatch.
- Les résumés de réponse capturent le statut HTTP, les en-têtes et les charges utiles du registre pour un audit ultérieur.

## Vérifier le contenu CAR et les preuves

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```- Reconstruire l'arbre PoR et comparer les résumés de payload avec le résumé du manifeste.
- Capturer les décomptes et identifiants requis lors de la soumission des preuves de réplication à la gouvernance.

## Diffuseur la télémétrie des preuves

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Émet des éléments NDJSON pour chaque preuve streamé (désactivez le replay avec `--emit-events=false`).
- Agréger les décomptes succès/échec, les histogrammes de latence et les échecs échantillonnés dans le CV JSON afin que les tableaux de bord puissent tracer les résultats sans scruter les logs.
- Quitte avec un code non nul lorsque la passerelle signale des échecs ou que la vérification PoR locale (via `--por-root-hex`) rejette des preuves. Ajustez les seuils avec `--max-failures` et `--max-verification-failures` pour les courses de répétition.
- Supporte PoR aujourd'hui ; PDP et PoTR réutilisent la même enveloppe une fois SF-13/SF-14 en place.
- `--governance-evidence-dir` écrit le résumé rendu, les métadonnées (timestamp, version du CLI, URL de la gateway, digest du manifest) et une copie du manifest dans le répertoire fourni afin que les paquets de gouvernance puissent archiver la preuve du proof-stream sans rejouer l'exécution.

## Références supplémentaires- `docs/source/sorafs_cli.md` — documentation exhaustive des flags.
- `docs/source/sorafs_proof_streaming.md` — schéma de télémétrie des preuves et modèle de tableau de bord Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — plongée approfondie dans le chunking, la composition de manifeste et la gestion des CAR.