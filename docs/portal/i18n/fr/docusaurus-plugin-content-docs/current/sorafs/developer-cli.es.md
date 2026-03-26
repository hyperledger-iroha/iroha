---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
titre : Recetario de CLI de SoraFS
sidebar_label : Récapitulatif de la CLI
description : Recorrido orienté vers les tareas de la surface consolidée de `sorafs_cli`.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/cli.md`. Mantén est une copie synchronisée.
:::

La surface consolidée de `sorafs_cli` (proportionnée par la caisse `sorafs_car` avec la fonctionnalité `cli` autorisée) expose chaque étape nécessaire pour préparer les artefacts de SoraFS. Utilisez ce récepteur pour saler directement les municipalités de Flujos ; combiné avec le pipeline de manifeste et les runbooks de l'explorateur pour le contexte opérationnel.

## Charges utiles Empaquetar

Usa `car pack` pour produire des archives CAR déterministes et des avions de chunk. La commande sélectionne automatiquement la salve du chunker SF-1 qui fournit une poignée.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Poignée de chunker prédéfinie : `sorafs.sf1@1.0.0`.
- Les entrées du répertoire sont enregistrées dans un ordre lexicográfico pour que les sommes de contrôle soient stables entre les plates-formes.
- Le résumé JSON comprend les résumés de la charge utile, les métadonnées du morceau et le CID identifié par le registre et l'explorateur.

## Construir manifeste

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Les options `--pin-*` sont attribuées directement aux champs `PinPolicy` et `sorafs_manifest::ManifestBuilder`.
- Utilisez `--chunk-plan` lorsque vous souhaitez que la CLI recalcule le résumé SHA3 du morceau avant l'envoi ; de lo contrario réutilise le digest incrusté dans le curriculum vitae.
- La sortie JSON reflète la charge utile Norito pour des différences simples lors des révisions.

## Firmar se manifeste sans clés de longue durée

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Acceptez les jetons en ligne, les variables d'entrée ou les sources basées dans les archives.
- Ajouter les métadonnées de procédure (`token_source`, `token_hash_hex`, digest de chunk) sans conserver le JWT dans la salve brute de `--include-token=true`.
- Fonctionne bien en CI : combiné avec OIDC de GitHub Actions configuré `--identity-token-provider=github-actions`.

## Enviar manifeste un Torii

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

- Réalisation du décodification Norito pour les preuves d'alias et vérification qui coïncident avec le résumé du manifeste avant POSTear avec Torii.
- Recalculez le résumé SHA3 du morceau à partir du plan pour prévenir les attaques de désajustement.
- Les résultats de la réponse capturent l'état HTTP, les en-têtes et les charges utiles du registre pour les auditeurs postérieurs.

## Vérifier le contenu de CAR et les preuves

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruire l'arbol PoR et comparer les résumés de la charge utile avec le résumé du manifeste.
- Capturer les conteos et les identifiants requis pour envoyer des preuves de réplication à l'administration.## Transmitir telemetría de proofs

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

- Émettre des éléments NDJSON par transmission de preuve (désactiver la relecture avec `--emit-events=false`).
- Ajoutez des contes de réussite/d'échec, d'histogrammes de latence et d'échecs affichés dans le CV JSON pour que les tableaux de bord puissent afficher les résultats sans lire les journaux.
- Vente avec un code distinct de zéro lorsque la passerelle signale des erreurs ou la vérification PoR locale (vía `--por-root-hex`) rechaza preuves. Ajustez les parapluies avec `--max-failures` et `--max-verification-failures` pour les opérations d'échantillonnage.
- Soporta PoR aujourd'hui ; PDP et PoTR réutilisent le même appareil lorsque vous arrivez au SF-13/SF-14.
- `--governance-evidence-dir` décrit le rendu du résumé, les métadonnées (horodatage, version de CLI, URL de la passerelle, résumé du manifeste) et une copie du manifeste dans le répertoire géré pour que les paquets de gouvernance archivent les preuves du flux de preuve sans répéter l'exécution.

## Références supplémentaires

- `docs/source/sorafs_cli.md` — documentation exhaustive des drapeaux.
- `docs/source/sorafs_proof_streaming.md` — esquema de télémétrie de preuves et plante de tableau de bord Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — profondeur en chunking, composition de manifeste et gestion de CAR.