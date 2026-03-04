---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
titre : Guide de conformité du chunker da SoraFS
sidebar_label : Conformité du chunker
description : Conditions requises et flux pour préserver le profil déterminé du chunker SF1 dans les luminaires et les SDK.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas comme copies synchronisées.
:::

Este guia codifica os requisitos que toda Implementacao doit seguir para permanecer
compatible avec le profil déterministe du chunker de SoraFS (SF1). Ele aussi
documenta o fluxo de regeneracao, a politica de assinaturas e os pasos de verificacao para que
Les consommateurs de luminaires et de SDK sont synchronisés.

## Profil canonique

- Poignée de profil : `sorafs.sf1@1.0.0`
- Graine d'entrée (hex) : `0000000000dec0ded`
- Tamanho alvo : 262 144 octets (256 Ko)
- Taille minimale : 65 536 octets (64 Ko)
- Tamanho maximo : 524 288 octets (512 Ko)
- Polinomio de roulage: `0x3DA3358B4DC173`
- Engrenage de la table : `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Implémentation de référence : `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qu'importe l'accélération SIMD doit produire des limites et des digestions identiques.

## Bundle de luminaires

`cargo run --locked -p sorafs_chunker --bin export_vectors` se régénère comme
les luminaires émettent des fichiers suivants dans `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` - limites canoniques de chunk pour les consommateurs
  Rust, TypeScript et Go. Chaque fois que vous annoncez la poignée canonique comme la première
  entrada em `profile_aliases`, suivi de quaisquer alias alternatifs (ex.,
  `sorafs.sf1@1.0.0`, après `sorafs.sf1@1.0.0`). A ordem e imposta por
  `ensure_charter_compliance` et NAO DEVE seront modifiés.
- `manifest_blake3.json` - manifeste vérifié par BLAKE3 cobrindo cada arquivo de luminaires.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) sobre o digest do manifest.
- `sf1_profile_v1_backpressure.json` et les corps bruts à l'intérieur de `fuzz/` -
  scénarios déterminants de streaming utilisés par les testes de contre-pression du chunker.

### Politique des assassinats

La régénération des luminaires **deve** inclut une assinatura valide du conseil. Ô gérador
rejeita a dit sem assinatura a menos que `--allow-unsigned` seja passado explicitamente (destinado
apenas para experimentalacao local). Os enveloppes d'assinatura sao append-only e
sao déduplicados por signatario.

Pour ajouter une Assinature du Conselho :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

L'assistant de CI `ci/check_sorafs_fixtures.sh` réexécute le générateur avec
`--locked`. Si les luminaires divergirem ou assinaturas faltarem, o job falha. Utiliser
ce script dans les flux de travail est prévu et avant d'envoyer des modifications aux luminaires.

Manuel des étapes de vérification :

1. Exécutez `cargo test -p sorafs_chunker`.
2. Exécutez `ci/check_sorafs_fixtures.sh` localement.
3. Confirmez que `git status -- fixtures/sorafs_chunker` est vide.## Playbook de mise à niveau

Pour créer un nouveau profil de chunker ou actualiser SF1 :

Voir également : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour
exigences de métadonnées, modèles de proposition et listes de contrôle de validation.

1. Remplacez le `ChunkProfileUpgradeProposalV1` (voir RFC SF-1) avec de nouveaux paramètres.
2. Régénérez les appareils via `export_vectors` et enregistrez le nouveau résumé du manifeste.
3. Assine o manifest com o quorum do conselho exigido. Todas as assinaturas devem ser
   anexadas a `manifest_signatures.json`.
4. Actualisez au fur et à mesure les appareils SDK compatibles (Rust/Go/TS) et garantissez la parité cross-runtime.
5. Régénérez le corps en fonction des paramètres modifiés.
6. Actualisez cette guide avec la nouvelle poignée de profil, les graines et le digest.
7. Envie d'un changement avec les tests actualisés et actualisés de la feuille de route.

Mudancas que afetem limites de chunk ou digests seguir ce processus
Sao Invalidas et Nao devraient être fusionnés.