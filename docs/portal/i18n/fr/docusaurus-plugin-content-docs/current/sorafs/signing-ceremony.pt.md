---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : Substituicao da cerimonia de assinatura
description : Côme ou Parlement Sora approuve et distribue les appareils du chunker SoraFS (SF-1b).
sidebar_label : Cérémonie d'assinatura
---

> Feuille de route : **SF-1b - approbations de luminaires du Parlamento Sora.**
> Le flux du Parlement remplace l'antique "cerimonia de assinatura do conselho" hors ligne.

Le manuel rituel d'assinature utilisé pour les luminaires du chunker SoraFS a été posé.
Tous les aprovacoes agora passam pelo **Parlamento Sora**, a DAO baseada em sortio que
régit le Nexus. Les membres du Parlement bloquent XOR pour obtenir une ville, une rotation
entre paineis et vote en chaîne pour approuver, rejeter ou annuler les versions de luminaires.
Cette guide explique le processus et les outils pour les développeurs.

## Visa général du Parlement

- **Cidadania** - Les opérateurs bloquent le XOR nécessaire pour s'installer comme villes et
  se tornar elegiveis ao sortio.
- **Paineis** - As responsabilidades sao divididas entre paineis rotativos (Infraestrutura,
  Moderaçao, Tesouraria, ...). Le Painel de Infraestrutura et le dono das aprovacoes de
  les luminaires font SoraFS.
- **Sorteio e rotacao** - As cadeiras de painel sao redesenhadas na cadencia definida na
  constitution du Parlement pour que le groupe nenhum monopolise comme aprovacoes.

## Flux d'approbation des luminaires

1. **Soumettre la proposition**
   - O Tooling WG envoie le bundle candidat `manifest_blake3.json` mais le diff do luminaire
     pour le registre en chaîne via `sorafs.fixtureProposal`.
   - A proposta registra o digest BLAKE3, a versao sémantica e as notas de mudanca.
2. **Révision et vote**
   - Le Painel de Infraestrutura reçoit une contribution de la fila de tarefas do Parlamento.
   - Membros do painel inspecionam artefatos de CI, rodam testes de paridade e
     registram votos ponderados en chaîne.
3. **Finalisation**
   - Lorsque le quorum est atteint, le runtime émet un événement d'approbation qui inclut le
     digérer canonique le manifeste et le compromis pour que Merkle fasse la charge utile du luminaire.
   - L'événement et l'enregistrement ne sont pas enregistrés SoraFS pour que les clients puissent rechercher ou
     manifeste le plus récent approuvé par le Parlement.
4. **Distribution**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) puxam ou manifeste approuvé via
     Nexus RPC. Comme les constantes JSON/TS/Go font le dépôt ficam synchronisé entre autres
     réexécutez `export_vectors` et validez le résumé contre l'enregistrement en chaîne.

## Flux de travail du développeur

- Luminaires Regenere com :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utilisez l'aide de récupération du Parlement pour baisser l'enveloppe approuvée, vérifier
  assinaturas e atualizar luminaires locaux. Aponte `--signatures` pour l'enveloppe
  publié par le Parlement; o aide à résoudre o manifeste associé, recalcule o
  digérer BLAKE3 et impoe le profil canonique `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passez `--manifest` pour afficher le manifeste à l'extérieur de l'URL. Enveloppes sem assinatura
sao recusados, à moins que `--allow-unsigned` soit défini pour la fumée s'écoule localement.- Pour valider un manifeste via la passerelle de staging, disponible pour Torii dès maintenant
  charges utiles locales :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local nao exige plus une liste `signer.json`.
  `ci/check_sorafs_fixtures.sh` comparer l'état du repo avec le dernier compromis
  en chaîne et falha quando divergem.

## Notes de gouvernance

- Une constitution du Parlement régissant le quorum, la rotation et l'escalade - nao e
  configuration nécessaire sans niveau dans la caisse.
- Rollbacks d'émergence sao traités par le painel de moderacao do Parlamento. Ô
  Painel de Infraestrutura ouvre une proposition de retour en référence au résumé
  antérieur se manifeste, substituindo une libération quando aprovada.
- Aprovacoes historiques permanentes disponibles sans registre SoraFS pour replay
  forense.

##FAQ

- **Para onde foi `signer.json`?**  
  Je l'ai retiré. Aujourd'hui, l'attribution des assassinats existe en chaîne ; `manifest_signatures.json`
  Il n'y a pas de référentiel ni un appareil de développeur qui doit correspondre jusqu'à la fin
  événement d'approbation.

- **Ainda exigimos assinaturas Ed25519 locais?**  
  Nao. Comme aprovacoes do Parlamento sao armazenadas como artefatos on-chain. Calendrier
  il existe localement pour la reproduction, mais sao validé contre le résumé du Parlement.

- **Como as equipes monitoram aprovacoes?**  
  Assinem l'événement `ParliamentFixtureApproved` ou consulte le registre via Nexus RPC
  pour récupérer le digestat actuel du manifeste et la chamada du painel.