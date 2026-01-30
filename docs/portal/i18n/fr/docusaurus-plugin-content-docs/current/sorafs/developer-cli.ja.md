---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e582552fe301e8f6e8dd167eeecdbaa81e4f3e3015c2d73f086129b9c41486c7
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: developer-cli
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Source canonique
:::

La surface consolidée `sorafs_cli` (fournie par le crate `sorafs_car` avec la feature `cli` activée) expose chaque étape nécessaire pour préparer les artefacts SoraFS. Utilisez ce cookbook pour aller directement aux workflows courants ; associez-le au pipeline de manifest et aux runbooks de l'orchestrateur pour le contexte opérationnel.

## Empaqueter les payloads

Utilisez `car pack` pour produire des archives CAR déterministes et des plans de chunk. La commande sélectionne automatiquement le chunker SF-1 sauf si un handle est fourni.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Handle de chunker par défaut : `sorafs.sf1@1.0.0`.
- Les entrées de répertoire sont parcourues en ordre lexicographique afin que les checksums restent stables entre plateformes.
- Le résumé JSON inclut les digests de payload, les métadonnées par chunk et le CID racine reconnu par le registre et l'orchestrateur.

## Construire les manifests

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Les options `--pin-*` mappent directement vers les champs `PinPolicy` dans `sorafs_manifest::ManifestBuilder`.
- Fournissez `--chunk-plan` lorsque vous souhaitez que le CLI recalcule le digest SHA3 de chunk avant soumission ; sinon il réutilise le digest intégré au résumé.
- La sortie JSON reflète le payload Norito pour des diffs simples lors des revues.

## Signer les manifests sans clés de longue durée

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Accepte des tokens inline, des variables d'environnement ou des sources basées sur des fichiers.
- Ajoute des métadonnées de provenance (`token_source`, `token_hash_hex`, digest de chunk) sans persister le JWT brut sauf si `--include-token=true`.
- Fonctionne bien en CI : combinez avec OIDC de GitHub Actions en définissant `--identity-token-provider=github-actions`.

## Soumettre les manifests à Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Effectue le décodage Norito des alias proofs et vérifie qu'ils correspondent au digest du manifest avant POST vers Torii.
- Recalcule le digest SHA3 de chunk à partir du plan pour éviter les attaques par mismatch.
- Les résumés de réponse capturent le statut HTTP, les headers et les payloads du registre pour un audit ultérieur.

## Vérifier le contenu CAR et les proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruit l'arbre PoR et compare les digests de payload avec le résumé du manifest.
- Capture les décomptes et identifiants requis lors de la soumission des proofs de réplication à la gouvernance.

## Diffuser la télémétrie des proofs

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Émet des éléments NDJSON pour chaque proof streamé (désactivez le replay avec `--emit-events=false`).
- Agrège les décomptes succès/échec, les histogrammes de latence et les échecs échantillonnés dans le résumé JSON afin que les dashboards puissent tracer les résultats sans scruter les logs.
- Quitte avec un code non nul lorsque la gateway signale des échecs ou que la vérification PoR locale (via `--por-root-hex`) rejette des proofs. Ajustez les seuils avec `--max-failures` et `--max-verification-failures` pour les runs de répétition.
- Supporte PoR aujourd'hui ; PDP et PoTR réutilisent la même enveloppe une fois SF-13/SF-14 en place.
- `--governance-evidence-dir` écrit le résumé rendu, les métadonnées (timestamp, version du CLI, URL de la gateway, digest du manifest) et une copie du manifest dans le répertoire fourni afin que les paquets de gouvernance puissent archiver la preuve du proof-stream sans rejouer l'exécution.

## Références supplémentaires

- `docs/source/sorafs_cli.md` — documentation exhaustive des flags.
- `docs/source/sorafs_proof_streaming.md` — schéma de télémétrie des proofs et template de dashboard Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — plongée approfondie dans le chunking, la composition de manifest et la gestion des CAR.
