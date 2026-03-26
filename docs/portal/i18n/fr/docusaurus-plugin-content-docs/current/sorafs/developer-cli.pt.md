---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
titre : Receitas de CLI da SoraFS
sidebar_label : Recettes de CLI
description : Guia focado em tarefas da superficie consolidada de `sorafs_cli`.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/developer/cli.md`. Mantenha ambas comme copies synchronisées.
:::

La surface consolidée `sorafs_cli` (fornecida pelo crate `sorafs_car` com o recurso `cli` habilité) expose toute l'étape nécessaire pour préparer les artefatos da SoraFS. Utilisez ce livre de recettes directement sur les flux de travail communs ; combinez le pipeline de manifeste et les runbooks de l'explorateur pour le contexte opérationnel.

## Charges utiles Empacotar

Utilisez `car pack` pour produire des archives CAR déterministes et des plans de bloc. La commande sélectionne automatiquement le chunker SF-1, à moins qu'une poignée ne soit fournie.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Poignée de chunker padrao : `sorafs.sf1@1.0.0`.
- Entrées du répertoire sao percorridas dans l'ordre lexicographique pour que les sommes de contrôle soient enregistrées entre les plates-formes.
- Le résumé JSON inclut les résumés de la charge utile, les métadonnées du morceau et le CID qui a été reconnu par l'enregistreur et par l'explorateur.

## Construir manifeste

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Opcoes `--pin-*` mapeiam directement pour les champs `PinPolicy` à `sorafs_manifest::ManifestBuilder`.
- Forneca `--chunk-plan` lorsque vous souhaitez que la CLI recalcule le résumé SHA3 du morceau avant l'envoi ; caso contrario, il réutilise le digest embutido no CV.
- JSON a déclaré la charge utile Norito pour les différences simples lors des révisions.

## Assinar manifeste sem chaves de longa duracao

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Jetons Aceita en ligne, variables d'ambiance ou polices basées sur les archives.
- Ajout de métadonnées de procédure (`token_source`, `token_hash_hex`, digest de chunk) qui persiste sur le brut JWT, à moins que `--include-token=true`.
- Fonctionne avec CI : combinez avec OIDC et les actions GitHub définissent `--identity-token-provider=github-actions`.

## Enviar manifeste para o Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Faz décodificacao Norito pour les preuves d'alias et vérifiez leur correspondance avec le résumé du manifeste avant d'envoyer via POST vers Torii.
- Recalculez le résumé SHA3 du morceau à partir du plan pour éviter les attaques de divergence.
- Les résumés de réponse capturent le statut HTTP, les en-têtes et les charges utiles à enregistrer pour les auditoires postérieurs.

## Vérifier les conteudos de CAR et les preuves

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruisez le PoR et comparez les résumés de la charge utile avec le résumé du manifeste.
- Capturer les contagiens et les identifiants requis pour envoyer des preuves de réplication pour le gouvernement.

## Transmettre la télémétrie des preuves

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Émettez des éléments NDJSON pour chaque transmission de preuve (désatif ou replay avec `--emit-events=false`).
- Regrouper les taux de succès/échecs, les histogrammes de latence et les échecs enregistrés sans résumé JSON pour que les tableaux de bord puissent tracer les résultats dans leurs journaux.
- Sai com codigo nao zero quando o gateway reporta falhas ou quando a verificacao PoR local (via `--por-root-hex`) rejeita proofs. Ajustez les limites avec `--max-failures` et `--max-verification-failures` pour l'exécution des analyses.
- Soutenir PoR aujourd'hui ; PDP et PoTR réutilisent l'enveloppe mémo lorsque vous utilisez SF-13/SF-14.
- `--governance-evidence-dir` contient le rendu du résumé, les métadonnées (horodatage, version de la CLI, URL de la passerelle, résumé du manifeste) et une copie du manifeste dans le répertoire fourni pour que les paquets de gouvernance fournissent des preuves du flux de preuve lors de l'exécution.

## Références supplémentaires

- `docs/source/sorafs_cli.md` - documentation exhaustive des drapeaux.
- `docs/source/sorafs_proof_streaming.md` - esquema de télémétrie de preuves et modèle de tableau de bord Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - fusion profonde dans le chunking, composition du manifeste et de la gestion de la CAR.