---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 796a1a1096b23064f49f98cbabc61759dfbb0324feeabff8dddba0ffb896f6c2
source_last_modified: "2025-11-09T09:53:46.358746+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: signing-ceremony
title: Remplacement de la ceremony de signature
description: Comment le Parlement Sora approuve et distribue les fixtures du chunker SoraFS (SF-1b).
sidebar_label: Ceremonie de signature
---

> Roadmap : **SF-1b — approbations des fixtures du Parlement Sora.**
> Le workflow du Parlement remplace l'ancienne « ceremonie de signature du conseil » hors ligne.

Le rituel manuel de signature des fixtures du chunker SoraFS est retire. Toutes les
approbations passent desormais par le **Parlement Sora**, la DAO basee sur le tirage
au sort qui gouverne Nexus. Les membres du Parlement bloquent du XOR pour obtenir la
citoyennete, tournent entre panels et votent on-chain pour approuver, rejeter ou
revenir sur des releases de fixtures. Ce guide explique le processus et le tooling
pour les developers.

## Vue d'ensemble du Parlement

- **Citoyennete** — Les operateurs immobilisent le XOR requis pour s'inscrire comme
  citoyens et devenir eligibles au tirage au sort.
- **Panels** — Les responsabilites sont reparties entre des panels rotatifs
  (Infrastructure, Moderation, Tresorerie, ...). Le Panel Infrastructure detient
  les approbations de fixtures SoraFS.
- **Tirage au sort et rotation** — Les sieges de panel sont reattribues selon la
  cadence specifiee dans la constitution du Parlement afin qu'aucun groupe ne
  monopolise les approbations.

## Flux d'approbation des fixtures

1. **Soumission de proposition**
   - Le Tooling WG televerse le bundle candidat `manifest_blake3.json` et le diff
     de fixture dans le registre on-chain via `sorafs.fixtureProposal`.
   - La proposition enregistre le digest BLAKE3, la version semantique et les notes
     de changement.
2. **Revue et vote**
   - Le Panel Infrastructure recoit l'affectation via la file de taches du Parlement.
   - Les membres inspectent les artefacts CI, executent des tests de parite et
     emettent des votes ponderes on-chain.
3. **Finalisation**
   - Une fois le quorum atteint, le runtime emet un evenement d'approbation incluant
     le digest canonique du manifest et l'engagement Merkle du payload de fixture.
   - L'evenement est duplique dans le registry SoraFS afin que les clients puissent
     recuperer le dernier manifest approuve par le Parlement.
4. **Distribution**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) recuperent le manifest
     approuve via Nexus RPC. Les constantes JSON/TS/Go du depot restent synchronisees
     en relancant `export_vectors` et en validant le digest par rapport a l'enregistrement
     on-chain.

## Workflow developpeur

- Regenerer les fixtures avec :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utiliser le helper de fetch du Parlement pour telecharger l'enveloppe approuvee,
  verifier les signatures et rafraichir les fixtures locales. Pointer `--signatures`
  vers l'enveloppe publiee par le Parlement ; le helper resout le manifest associe,
  recalcule le digest BLAKE3 et impose le profil canonique `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passer `--manifest` si le manifest se trouve a une autre URL. Les enveloppes non
signees sont refusees sauf si `--allow-unsigned` est active pour des smoke runs locaux.

- Pour valider un manifest via un gateway de staging, cibler Torii plutot que des
  payloads locaux :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local n'exige plus un roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` compare l'etat du repo avec le dernier engagement
  on-chain et echoue lorsqu'ils divergent.

## Notes de gouvernance

- La constitution du Parlement gouverne le quorum, la rotation et l'escalade ;
  aucune configuration au niveau du crate n'est necessaire.
- Les rollbacks d'urgence sont geres via le panel de moderation du Parlement. Le
  Panel Infrastructure depose une proposition de revert qui reference le digest
  precedent du manifest, et la release est remplacee une fois approuvee.
- Les approbations historiques restent disponibles dans le registry SoraFS pour
  un replay forensique.

## FAQ

- **Ou est passe `signer.json` ?**  
  Il a ete supprime. Toute attribution de signature vit on-chain ; `manifest_signatures.json`
  dans le depot n'est qu'un fixture developpeur qui doit correspondre au dernier
  evenement d'approbation.

- **Faut-il encore des signatures Ed25519 locales ?**  
  Non. Les approbations du Parlement sont stockees comme artefacts on-chain. Les fixtures
  locales existent pour la reproductibilite mais sont validees contre le digest du Parlement.

- **Comment les equipes surveillent-elles les approbations ?**  
  Abonnez-vous a l'evenement `ParliamentFixtureApproved` ou interrogez le registry via
  Nexus RPC pour obtenir le digest actuel du manifest et la liste des membres du panel.
