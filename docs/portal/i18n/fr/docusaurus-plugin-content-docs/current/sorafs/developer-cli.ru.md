---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-cli
titre : Книга рецептов CLI SoraFS
sidebar_label : Lire les réceptions CLI
description : Rasoir pratique pour la console de puissance `sorafs_cli`.
---

:::note Канонический источник
:::

La console de configuration `sorafs_cli` (préparée à la caisse `sorafs_car` avec la fonction intégrée `cli`) prend en charge le système, Non disponible pour les articles d'art SoraFS. Utilisez ce livre de recettes pour découvrir les types de flux de travail ; Connectez-vous au manifeste de pipeline et à l'opérateur de runbooks pour le contexte d'exploitation.

## Charges utiles Упаковка

Utilisez `car pack` pour déterminer les pièces d'architecture et de plan de voiture. La commande automatique active le chunker SF-1, sans utiliser de poignée.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Morceau de poignée standard : `sorafs.sf1@1.0.0`.
- Les répertoires établis dans le cadre du dictionnaire permettent d'établir les sommes de contrôle de la plate-forme stable.
- JSON inclut les résumés de charge utile, les métadonnées de chunk et le CID actuel, la restauration et l'organisation.

## Сборка manifeste

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Les instructions `--pin-*` vous permettent d'utiliser les polymères `PinPolicy` dans `sorafs_manifest::ManifestBuilder`.
- Téléchargez `--chunk-plan`, si vous êtes à la recherche d'une CLI qui a supprimé le résumé SHA3 pour le morceau avant l'ouverture ; иначе он использует digest из résumé.
- JSON ajoute la charge utile Norito pour les différences enregistrées avant la publication.## Подписание manifestes без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Présence de jetons en ligne, d'achats permanents ou d'enregistrements de fichiers.
- Ajoutez les métadonnées de provenance (`token_source`, `token_hash_hex`, fragment de digestion) sans la sauvegarde du JWT brut, si vous ne trouvez pas `--include-token=true`.
- Utilisé pour CI : utilisez GitHub Actions OIDC, en utilisant `--identity-token-provider=github-actions`.

## Ouvrir les manifestes dans Torii

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

- Vous avez défini les preuves d'alias Norito et vérifié le manifeste résumé du POST dans Torii.
- Sélectionnez le fragment de résumé SHA3 dans le plan pour prédire les données nécessaires.
- Réglez l'état HTTP, les en-têtes et les charges utiles pour la restauration de l'audio ultérieur.

## Проверка содержимого CAR и preuves

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Перестраивает дерево PoR и сравнивает payload digests с резюме манифеста.
- Fixez des collections et des identifiants, nécessaires pour extraire des preuves de réplication dans la gouvernance.

## Потоковая телеметрия preuves

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Générez des éléments NDJSON pour chaque preuve préalable (ajoutez la relecture à partir de `--emit-events=false`).
- Ajoutez des groupes d'utilisateurs/ordinateurs, des historiques de latence et des liens dans le résumé JSON, les tableaux de bord peuvent créer des graphiques sans logos.
- Si le robot est connecté à un nouveau code, cette passerelle doit être utilisée par les autorités ou par la province locale PoR (à l'intérieur de `--por-root-hex`) pour fournir des preuves. Sélectionnez les paramètres `--max-failures` et `--max-verification-failures` pour la répétition.
- Сегодня подддерживается PoR ; PDP et PoTR ont créé cette enveloppe après le SF-13/SF-14.
- `--governance-evidence-dir` permet d'obtenir un résumé de rendu, des métadonnées (horodatage, version CLI, passerelle URL, résumé du manifeste) et de copier le manifeste dans le répertoire d'achat, Les paquets de gouvernance peuvent archiver la documentation proof-stream avant l'entrée en vigueur.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — исчерпывающая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — épreuves télémétriques et tableau de bord Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — подробный разбор chunking, сборки manifest и обработки CAR.