---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : Sustitución de la ceremonia de firma
description : Le Parlement de Sora ouvre et distribue les appareils du chunker de SoraFS (SF-1b).
sidebar_label : Cérémonie de l'entreprise
---

> Feuille de route : **SF-1b — approbations de luminaires du Parlement de Sora.**
> Le flux du Parlement remplace l'ancienne « cérémonie du conseil d'administration » hors ligne.

Le manuel rituel des entreprises est utilisé pour les luminaires du chunker de SoraFS queda retraité. Tous
les autorisations sont maintenant passées par le **Parlamento de Sora**, la DAO est basée en quelque sorte que gobierna
Nexus. Les membres du Parlement ont financé XOR pour obtenir la ville, tourner entre les panneaux et
émettent des votes en chaîne qui aprueban, rechazan ou revierten releases de luminaires. Cette guia explicative
le processus et les outils pour les développeurs.

## Résumé du Parlement

- **Ciudadania** — Les opérateurs fianzan el XOR requis pour s'inscrire comme citoyens et
  volverse elegibles para sorteo.
- **Panneaux** — Las responsabilités se divisent en panneaux rotatifs (Infraestructura,
  Moderacion, Tesoreria, …). Le Panel de Infraestructura est le duo des autorisations de
  luminaires de SoraFS.
- **Sorteo y rotacion** — Les positions du panneau sont réaffectées avec la cadence spécifiée
  la constitution du Parlement pour que le groupe ait le monopole des autorisations.

## Flux d'approbation des luminaires

1. **Envoi de propuesta**
   - Le WG de Tooling sous le bundle candidat `manifest_blake3.json` est le différentiel du luminaire
     au registre en chaîne via `sorafs.fixtureProposal`.
   - La proposition d'enregistrer le digest BLAKE3, la version sémantique et les notes de changement.
2. **Révision et vote**
   - Le Panel de Infraestructura reçoit la désignation à travers la tête de tareas del
     Parlement.
   - Les membres du panneau inspectent les artefacts de CI, corren tests de parité et d'émission
     votes pondérés en chaîne.
3. **Finalisation**
   - Une fois le quorum atteint, le runtime émet un événement d'approbation qui inclut le
     digérer canoniquement le manifeste et le compromis Merkle de la charge utile du luminaire.
   - L'événement est réfléchi dans le registre de SoraFS pour que les clients puissent obtenir le
     manifestement plus récemment approuvé par le Parlement.
4. **Distribution**
   - Les assistants de CLI (`cargo xtask sorafs-fetch-fixture`) traitent le manifeste approuvé à partir de
     Nexus RPC. Les constantes JSON/TS/Go du référentiel sont maintenues synchronisées avec
     Réexécutez `export_vectors` et validez le résumé contre le registre en chaîne.

## Flux de travail pour les développeurs

- Luminaires Regenera avec :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utilisez l'aide pour récupérer le Parlement pour télécharger l'enveloppe approuvée, vérifier
  firmas y refrescar luminaires locaux. Apunta `--signatures` à l'enveloppe publiée par le
  Parlement; l'assistant résout le manifeste accompagnant, recalcule le résumé BLAKE3 et
  impone el profil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Pasa `--manifest` si le manifeste est présent dans une autre URL. Los enveloppes sin firma se rechazan
salvo qui se configure `--allow-unsigned` pour les paramètres régionaux Smoke Runs.

- Pour valider un manifeste à travers une passerelle de staging, en apposant sur Torii en présence de charges utiles
  paramètres régionaux :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local ne nécessite pas une liste `signer.json`.
  `ci/check_sorafs_fixtures.sh` comparer l'état du repo avec le dernier compromis
  on-chain et falla quando divergen.

## Notes de gouvernement

- La constitution du Parlamento Gobierna quorum, rotation et escalade ; non, je n'en ai pas besoin
  configuration au niveau de la caisse.
- Les rollbacks d'émergence se produisent à travers le panneau de modération du Parlement.
  Le Panel de Infraestructura présente une proposition de retour qui fait référence au résumé
  avant le manifeste, remplacez la version une fois approuvée.
- Les autorisations historiques permanentes disponibles dans le registre SoraFS pour
  rejouer forense.

##FAQ

- **A donde se fue `signer.json`?**  
  Il faut l'éliminer. Aujourd'hui, l'attribution des entreprises est vive en chaîne ; `manifest_signatures.json`
  dans le dépôt, c'est seulement un appareil de développeur qui doit coïncider avec le dernier
  evento de aprobacion.

- **Nous avons besoin des locales Ed25519 ?**  
  Non. Les autorisations du Parlement sont enregistrées comme des artefacts en chaîne. Calendrier de Los
  des lieux existent pour la reproductibilité, mais ils sont validés contre le résumé du Parlement.

- **Comment surveiller les autorisations des équipes ?**  
  Abonnez-vous à l'événement `ParliamentFixtureApproved` ou consultez le registre via Nexus RPC
  pour récupérer le résumé actuel du manifeste et l'appel du panneau.