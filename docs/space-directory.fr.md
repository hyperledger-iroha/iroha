---
lang: fr
direction: ltr
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/space-directory.md (Space Directory Operator Playbook) -->

# Playbook Opérateur du Space Directory

Ce playbook explique comment rédiger, publier, auditer et faire tourner les entrées du
**Space Directory** pour les dataspaces Nexus. Il complète les notes d’architecture dans
`docs/source/nexus.md` et le plan d’onboarding CBDC
(`docs/source/cbdc_lane_playbook.md`) en fournissant des procédures pratiques, des
fixtures et des modèles de gouvernance.

> **Périmètre.** Le Space Directory sert de registre canonique pour les manifests de
> dataspace, les politiques de capacité UAID (Universal Account ID) et la piste d’audit
> sur laquelle s’appuient les régulateurs. Même si le contrat sous‑jacent est encore en
> développement actif (NX‑15), les fixtures et processus décrits ci‑dessous sont prêts à
> être intégrés dans des outils et des tests d’intégration.

## 1. Concepts clés

| Terme | Description | Références |
|-------|-------------|------------|
| Dataspace | Contexte d’exécution/lane exécutant un ensemble de contrats approuvé en gouvernance. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (hash blake2b‑32) utilisé pour ancrer les permissions cross‑dataspace. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` décrivant des règles déterministes allow/deny pour un couple UAID/dataspace (deny prévaut). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | Métadonnées de gouvernance + DA publiées avec les manifests pour permettre aux opérateurs de reconstruire les sets de validateurs, les whitelists de composabilité et les hooks d’audit. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Événements encodés en Norito émis lors de l’activation, l’expiration ou la révocation de manifests. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Cycle de vie des manifests

Le Space Directory applique une **gestion de cycle de vie basée sur les epochs**.
Chaque changement produit un bundle de manifest signé ainsi qu’un événement :

| Événement | Déclencheur | Actions requises |
|----------|-------------|------------------|
| `ManifestActivated` | Un nouveau manifest atteint `activation_epoch`. | Diffuser le bundle, mettre à jour les caches, archiver l’approbation de gouvernance. |
| `ManifestExpired` | `expiry_epoch` est dépassé sans renouvellement. | Avertir les opérateurs, nettoyer les handles UAID, préparer un manifest de remplacement. |
| `ManifestRevoked` | Décision d’urgence deny‑wins avant expiration. | Révoquer immédiatement l’UAID, émettre un rapport d’incident, planifier une revue de gouvernance. |

Les abonnés doivent utiliser `DataEventFilter::SpaceDirectory` pour suivre certains
dataspaces ou UAIDs. Exemple de filtre (Rust) :

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. Workflow opérateur

| Phase | Propriétaire(s) | Étapes | Évidence |
|-------|-----------------|--------|----------|
| Draft | Propriétaire du dataspace | Cloner le fixture, éditer droits/gouvernance, lancer `cargo test -p iroha_data_model nexus::manifest`. | Diff Git, log de tests. |
| Review | Governance WG | Valider le JSON du manifest + les bytes Norito, signer le journal de décision. | Procès‑verbal signé, hash du manifest (BLAKE3 + Norito `.to`). |
| Publish | Lane ops | Soumettre via le CLI (`iroha app space-directory manifest publish`) en utilisant un payload Norito `.to` ou du JSON brut **ou** effectuer un POST sur `/v1/space-directory/manifests` avec le JSON du manifest + une raison optionnelle ; vérifier la réponse Torii et capturer `SpaceDirectoryEvent`. | Reçu CLI/Torii, log d’événements. |
| Expire | Lane ops / Gouvernance | Lancer `iroha app space-directory manifest expire` (UAID, dataspace, epoch) lorsqu’un manifest atteint sa fin de vie, vérifier `SpaceDirectoryEvent::ManifestExpired`, archiver les preuves de nettoyage de bindings. | Sortie CLI, log d’événements. |
| Revoke | Gouvernance + Lane ops | Lancer `iroha app space-directory manifest revoke` (UAID, dataspace, epoch, reason) **ou** POST `/v1/space-directory/manifests/revoke` avec le même payload vers Torii, vérifier `SpaceDirectoryEvent::ManifestRevoked`, mettre à jour le bundle d’évidence. | Reçu CLI/Torii, log d’événements, note dans le ticket. |
| Monitor | SRE/Compliance | Suivre la télémétrie + les logs d’audit, définir des alertes sur les révocations/expirations. | Capture Grafana, logs archivés. |
| Rotate/Revoke | Lane ops + Gouvernance | Préparer un manifest de remplacement (nouvel epoch), faire un tabletop, ouvrir un incident (en cas de revoke). | Ticket de rotation, post‑mortem d’incident. |

Tous les artefacts d’un déploiement sont conservés sous
`artifacts/nexus/<dataspace>/<timestamp>/` avec un manifest de checksums afin de
satisfaire les demandes d’évidence des régulateurs.

## 4. Template de manifest et fixtures

Utilisez les fixtures fournis comme référence canonique de schéma. L’exemple CBDC
wholesale (`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) contient
des entrées allow et deny :

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

Une règle deny‑wins typique pour l’onboarding CBDC pourrait ressembler à :

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

Une fois publié, le couple `uaid` + `dataspace` devient la clé primaire du registre de
permissions. Les manifests suivants sont chaînés par `activation_epoch`, et le registre
garantit que la dernière version active respecte toujours la sémantique “deny gagne”.

## 5. API Torii

### Publication de manifest

Les opérateurs peuvent publier des manifests directement via Torii, sans dépendre du CLI.

```
POST /v1/space-directory/manifests
```

| Champ | Type | Description |
|-------|------|-------------|
| `authority` | `AccountId` | Compte qui signe la transaction de publication. |
| `private_key` | `ExposedPrivateKey` | Clé privée encodée en base64, utilisée par Torii pour signer au nom de `authority`. |
| `manifest` | `SpaceDirectoryManifest` | Manifest complet (JSON) qui sera encodé en Norito côté serveur. |
| `reason` | `Option<String>` | Message d’audit optionnel, stocké avec les données de cycle de vie. |

Exemple de corps JSON :

```jsonc
{
  "authority": "ih58...",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

Torii renvoie `202 Accepted` dès que la transaction est mise en file. Lorsque le bloc est
exécuté, `SpaceDirectoryEvent::ManifestActivated` est émis (en fonction de
`activation_epoch`), les bindings sont reconstruits automatiquement et l’endpoint
d’inventaire des manifests reflète le nouveau payload. Les contrôles d’accès reflètent
les autres APIs d’écriture du Space Directory (gates CIDR/token d’API/politique de frais).

### API de révocation de manifest

Les révocations d’urgence ne nécessitent plus d’appeler le CLI : les opérateurs peuvent
effectuer un POST directement vers Torii pour enqueuer l’instruction canonique
`RevokeSpaceDirectoryManifest`. Le compte émetteur doit posséder
`CanPublishSpaceDirectoryManifest { dataspace }`, comme dans le flux CLI.

```
POST /v1/space-directory/manifests/revoke
```

| Champ | Type | Description |
|-------|------|-------------|
| `authority` | `AccountId` | Compte qui signe la transaction de révocation. |
| `private_key` | `ExposedPrivateKey` | Clé privée base64 que Torii utilise pour signer au nom de `authority`. |
| `uaid` | `String` | Littéral UAID (`uaid:<hex>` ou digest hex 64 caractères, LSB=1). |
| `dataspace` | `u64` | Identifiant du dataspace hébergeant le manifest. |
| `revoked_epoch` | `u64` | Epoch (inclusif) à partir duquel la révocation doit prendre effet. |
| `reason` | `Option<String>` | Message d’audit optionnel stocké avec les données de cycle de vie. |

Exemple de corps JSON :

```jsonc
{
  "authority": "ih58...",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii renvoie `202 Accepted` lorsque la transaction entre en file. À l’exécution du bloc,
`SpaceDirectoryEvent::ManifestRevoked` est émis, `uaid_dataspaces` est reconstruit
automatiquement et `/portfolio` ainsi que l’inventaire de manifests reflètent
immédiatement l’état révoqué. Les gates CIDR et les règles de politique de frais sont
identiques à celles des endpoints de lecture.

## 6. Template de profil de dataspace

Les profils capturent tout ce dont un nouveau validateur a besoin avant de se connecter.
Le fixture `profile/cbdc_lane_profile.json` documente :

- L’émetteur/quorum de gouvernance (`ih58...` + ID du ticket d’évidence).
- Le set de validateurs + quorum et les namespaces protégés (`cbdc`, `gov`).
- Le profil DA (classe A, liste d’attesteurs, cadence de rotation).
- L’ID de groupe de composabilité et la whitelist reliant UAIDs et manifests de capacité.
- Les hooks d’audit (liste d’événements, schéma de logs, service PagerDuty).

Réutilisez le JSON comme point de départ pour de nouveaux dataspaces et adaptez les
chemins de whitelist pour cibler les manifests de capacité pertinents.

## 7. Publication & rotation

1. **Encoder l’UAID.** Dériver le digest blake2b‑32 et le préfixer par `uaid:` :

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Encoder le payload Norito.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Ou utiliser le helper CLI (qui écrit à la fois le fichier `.to` et un `.hash` avec
   le digest BLAKE3‑256) :

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Publier via Torii.**

   ```bash
   # Si le manifest a déjà été encodé en Norito :
   iroha app space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Ou utilisez l’API HTTP décrite précédemment. Dans tous les cas, archivez :
   - le JSON original du manifest ;
   - le payload Norito `.to` et le hash BLAKE3 ;
   - la réponse Torii et l’événement `SpaceDirectoryEvent`.

4. **Rotation ou révocation.**

   - Pour une rotation planifiée : préparez un manifest de remplacement avec un
     `activation_epoch` futur, réalisez un tabletop et coordonnez l’activation avec les
     opérateurs de tous les dataspaces concernés.
   - Pour une révocation d’urgence : suivez le playbook de révocation (section “Revoke”
     du tableau de workflow), incluez l’ID d’incident dans le champ `reason` et
     conservez les logs et snapshots pertinents.

En suivant ce playbook, le Space Directory fournit un registre canonique, vérifiable et
adapté aux régulateurs sur la façon dont les capacités sont accordées, restreintes et
révoquées dans l’ensemble des dataspaces Nexus.
