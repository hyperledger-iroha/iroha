---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : Remplacement de la cérémonie de signature
description : Commentez le Parlement Sora approuve et distribue les luminaires du chunker SoraFS (SF-1b).
sidebar_label : Cérémonie de signature
---

> Feuille de route : **SF-1b — approbations des luminaires du Parlement Sora.**
> Le workflow du Parlement remplace l'ancienne « cérémonie de signature du conseil » hors ligne.

Le rituel manuel de signature des luminaires du chunker SoraFS est retiré. Tous les
approbations passent désormais par le **Parlement Sora**, la DAO basee sur le tirage
au sort qui gouverne Nexus. Les membres du Parlement bloquent du XOR pour obtenir la
citoyennete, tourne entre panels et votent en chaîne pour approuver, rejeter ou
Revenez sur les releases de luminaires. Ce guide explique le processus et le outillage
pour les développeurs.

## Vue d'ensemble du Parlement

- **Citoyennete** — Les opérateurs immobilisent le XOR requis pour s'inscrire comme
  citoyens et devenir éligibles au tirage au sort.
- **Panels** — Les responsabilités sont réparties entre des panneaux rotatifs
  (Infrastructure, Modération, Trésorerie, ...). Détenteur du Panel Infrastructure
  les approbations de luminaires SoraFS.
- **Tirage au sort et rotation** — Les sièges de panel sont reattribués selon la
  cadence spécifiée dans la constitution du Parlement afin qu'aucun groupe ne
  monopoliser les approbations.

## Flux d'approbation des luminaires

1. **Soumission de proposition**
   - Le Tooling WG téléverse le bundle candidat `manifest_blake3.json` et le diff
     de luminaire dans le registre en chaîne via `sorafs.fixtureProposal`.
   - La proposition enregistre le digest BLAKE3, la version sémantique et les notes
     de changement.
2. **Revue et vote**
   - Le Panel Infrastructure recit l'affectation via le fichier de taches du Parlement.
   - Les membres inspectent les artefacts CI, exécutent des tests de parité et
     emettent des votes pondérés en chaîne.
3. **Finalisation**
   - Une fois le quorum atteint, le runtime emet un événement d'approbation incluant
     le digest canonique du manifeste et l'engagement Merkle du payload de luminaire.
   - L'événement est dupliqué dans le registre SoraFS afin que les clients puissent
     récupérer le dernier manifeste approuvé par le Parlement.
4. **Distribution**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) récupèrent le manifeste
     approuver via Nexus RPC. Les constantes JSON/TS/Go du dépôt restent synchronisées
     en relancant `export_vectors` et en validant le digest par rapport à l'enregistrement
     en chaîne.

## Développeur de workflow

- Régénérer les luminaires avec :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utiliser l'aide de fetch du Parlement pour télécharger l'enveloppe approuvée,
  vérifier les signatures et rafraichir les luminaires locaux. Pointeur `--signatures`
  vers l'enveloppe publiée par le Parlement ; le helper résultat le manifest associé,
  recalcule le digest BLAKE3 et impose le profil canonique `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Passer `--manifest` si le manifeste se trouve à une autre URL. Les enveloppes non
les signataires sont refusés sauf si `--allow-unsigned` est actif pour des fumées locales.

- Pour valider un manifest via une gateway de staging, cibler Torii plutôt que des
  charges utiles locales :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local n'exige plus un roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` comparer l'état du repo avec le dernier engagement
  on-chain et echoue lorsqu'ils divergent.

## Notes de gouvernance

- La constitution du Parlement régit le quorum, la rotation et l'escalade ;
  Aucune configuration au niveau du crate n'est nécessaire.
- Les rollbacks d'urgence sont geres via le panel de modération du Parlement. Le
  Panel Infrastructure dépose une proposition de revert qui référence le digest
  précédent du manifeste, et la libération est remplacée une fois approuvée.
- Les approbations historiques restent disponibles dans le registre SoraFS pour
  une replay forensique.

##FAQ

- **Ou est passé `signer.json` ?**  
  Il a été supprimé. Toute attribution de signature vit en chaîne ; `manifest_signatures.json`
  dans le dépôt n'est qu'un développeur de luminaires qui doit correspondre au dernier
  événement d'approbation.

- **Faut-il encore des signatures Ed25519 locales ?**  
  Non. Les approbations du Parlement sont stockées comme artefacts en chaîne. Les calendriers
  locales existantes pour la reproductibilite mais sont validees contre le digest du Parlement.

- **Comment les équipes surveillent-elles les approbations ?**  
  Abonnez-vous à l'événement `ParliamentFixtureApproved` ou interrogez le registre via
  Nexus RPC pour obtenir le résumé actuel du manifeste et la liste des membres du panel.