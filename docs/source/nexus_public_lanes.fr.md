---
lang: fr
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

# Staking des lanes publics Nexus (NX-9)

Statut : En cours -> **runtime + docs operateurs alignes** (Avr 2026)
Responsables : Economics WG / Governance WG / Core Runtime
Reference roadmap : NX-9 - staking public lane et module de recompense

Cette note capture le modele de donnees canonique, la surface d'instructions, les controles
 de gouvernance et les hooks operationnels pour le programme de staking des lanes publics Nexus.
L'objectif est de permettre aux validateurs permissionless de rejoindre les lanes publics,
de bond du stake, de servir des blocs et de recevoir des recompenses tout en gardant des leviers
deterministes de slashing/runbook sous gouvernance.

Le scaffolding de code vit desormais dans :

- Types du modele de donnees : `crates/iroha_data_model/src/nexus/staking.rs`
- Definitions ISI : `crates/iroha_data_model/src/isi/staking.rs`
- Stub de l'executor core (retourne une erreur deterministe de garde jusqu'a l'arrivee de la logique NX-9) :
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs peuvent commencer a cabler les payloads Norito avant l'implementation complete du runtime ;
les instructions de stake verrouillent maintenant l'asset de staking configure en retirant de
`stake_account`/`staker` vers un compte escrow bonded (`nexus.staking.stake_escrow_account_id`).
Les slashes debitent l'escrow et creditent le sink configure (`nexus.staking.slash_sink_account_id`),
et les unbonds renvoient les fonds au compte d'origine une fois le timer expire.

## 1. Etat du ledger et types

### 1.1 Registres de validateurs

`PublicLaneValidatorRecord` suit l'etat canonique de chaque validateur :

| Champ | Description |
|-------|-------------|
| `lane_id: LaneId` | Lane que le validateur sert. |
| `validator: AccountId` | Compte qui signe les messages de consensus. |
| `stake_account: AccountId` | Compte qui fournit le self-bond (peut differer de l'identite du validateur). |
| `total_stake: Numeric` | Self stake + delegations approuvees. |
| `self_stake: Numeric` | Stake fourni par le validateur. |
| `metadata: Metadata` | Commission %, ids telemetrie, flags juridiction, info de contact. |
| `status: PublicLaneValidatorStatus` | Cycle de vie (pending/active/jailed/exiting/etc.). Le payload `PendingActivation` encode l'epoch cible. |
| `activation_epoch: Option<u64>` | Epoch ou le validateur est devenu actif (fixe a l'activation). |
| `activation_height: Option<u64>` | Hauteur de bloc enregistree a l'activation. |
| `last_reward_epoch: Option<u64>` | Epoch du dernier payout. |

`PublicLaneValidatorStatus` enumere les phases du cycle de vie :

- `PendingActivation(epoch)` - en attente de l'epoch d'activation specifie par la gouvernance ; le payload
  de tuple stocke l'epoch d'activation la plus tot calculee comme `current_epoch + 1` (genesis bootstrap uses `current_epoch`)
  (les epochs derivent de `epoch_length_blocks`).
- `Active` - participe au consensus et peut percevoir des recompenses.
- `Jailed { reason }` - suspendu temporairement (downtime, breach telemetrie, etc.).
- `Exiting { releases_at_ms }` - unbonding ; les recompenses cessent de s'accumuler.
- `Exited` - retire du set.
- `Slashed { slash_id }` - evenement de slashing enregistre pour audit.

Les metadonnees d'activation sont monotones : `activation_epoch`/`activation_height` sont fixes la premiere
fois qu'un validateur pending devient actif et toute tentative de reactivation a un epoch/height anterieur
est rejetee. Les validateurs pending sont promus automatiquement au debut du premier bloc dont l'epoch
atteint la limite planifiee, et le compteur de metriques d'activation
(`nexus_public_lane_validator_activation_total`) enregistre la promotion en meme temps que le changement de statut.

Les deployments permissioned gardent le roster de peers genesis actif meme avant qu'un stake de validateur
public-lane existe : tant que les peers ont des cles de consensus actives, le runtime se rabat sur les peers
genesis pour le set de validateurs. Cela evite un deadlock de bootstrap tant que l'admission staking est
inactive ou en cours de rollout.

### 1.2 Parts de stake et unbonding

Les delegators (et les validateurs qui augmentent leur propre bond) sont modeles via
`PublicLaneStakeShare` :

- `bonded: Numeric` - montant bonded actif.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` - retraits en attente, cles par un `request_id`
  fourni par le client.
- `metadata` stocke des indications UX/back-office (ex. numeros de reference de desk custody).

`PublicLaneUnbonding` contient le calendrier deterministe de retrait (`amount`, `release_at_ms`).
Torii expose maintenant les shares actives et les retraits en attente via
`GET /v1/nexus/public_lanes/{lane}/stake` afin que les wallets affichent les timers sans RPCs bespoke.

Hooks de cycle de vie (forces par runtime) :

- Les entrees `PendingActivation(epoch)` basculent automatiquement a `Active` lorsque l'epoch courant atteint
  `epoch`. L'activation enregistre `activation_epoch` et `activation_height`, et les regressions sont rejetees
  a la fois pour l'auto-activation et les appels explicites `ActivatePublicLaneValidator`.
- Les entrees `Exiting(releases_at_ms)` passent a `Exited` lorsque le timestamp de bloc depasse `releases_at_ms`,
  en nettoyant les lignes stake-share pour que la capacite des validateurs soit recuperee sans nettoyage manuel.
- L'enregistrement des recompenses rejette les shares de validateurs sauf si le validateur est `Active`,
  evitant que des validateurs pending/exiting/jailed accumulent des payouts.

### 1.3 Registres de recompenses

Les distributions de recompenses utilisent `PublicLaneRewardRecord` et `PublicLaneRewardShare` :

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "xor#wonderland",
  "total_reward": "250.0000",
  "shares": [
    { "account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe...",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Les enregistrements fournissent aux auditeurs et dashboards une evidence deterministe pour chaque payout.
La structure de recompense alimente l'ISI `RecordPublicLaneRewards`.

Gardes runtime :

- Les builds Nexus doivent etre actives ; les builds offline/stub rejettent l'enregistrement des recompenses.
- Les epochs de recompense avancent de facon monotone par lane ; les epochs stale ou dupliques sont rejetes.
- Les assets de recompense doivent correspondre au fee sink configure (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) et le solde du sink doit couvrir integralement `total_reward`.
- Chaque share doit etre positive et respecter la spec numerique de l'asset ; les totaux de shares doivent
  egaler `total_reward`.

## 2. Catalogue d'instructions

Toutes les instructions vivent sous `iroha_data_model::isi::staking`. Elles derivent des encoders/decoders
Norito pour que les SDKs soumettent les payloads sans codecs bespoke.

### 2.1 `RegisterPublicLaneValidator`

Enregistre un validateur et bond un stake initial :

```norito
{
  "lane_id": 1,
  "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "stake_account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Regles de validation :

- `initial_stake` >= `min_self_stake` (parametre de gouvernance).
- Metadata DOIT inclure des hooks contact/telemetrie avant l'activation.
- La gouvernance approuve/refuse l'entree ; jusque-la le statut est `PendingActivation` et le runtime
  promeut le validateur en `Active` a la prochaine frontiere d'epoch une fois l'epoch cible atteinte
  (`current_epoch + 1` (genesis bootstrap uses `current_epoch`) a l'enregistrement).

### 2.2 `BondPublicLaneStake`

Bond du stake additionnel (self-bond du validateur ou contribution de delegator).

Champs cles : `staker`, `amount`, metadata optionnelle pour les statements. Le runtime doit faire respecter
les limites par lane (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Demarre le timer d'unbonding. Les submitters fournissent un `request_id` deterministe
(recommandation : `blake2b(invoice)`), `amount`, et `release_at_ms`. Le runtime doit verifier
`amount` <= stake bonded et ajuster `release_at_ms` a la periode d'unbonding configuree.

### 2.4 `FinalizePublicLaneUnbond`

Apres expiration du timer, cet ISI deverrouille le stake pending et le renvoie a `staker`.
L'executor valide le request id, verifie que le timestamp de unlock est passe, emet une mise a jour
`PublicLaneStakeShare` et enregistre la telemetrie.

### 2.5 `SlashPublicLaneValidator`

La gouvernance utilise cette instruction pour debiter le stake et jail/eject des validateurs.

- `slash_id` lie l'evenement a la telemetrie + docs d'incident.
- `reason_code` est une string enum stable (ex. `double_sign`, `downtime`, `safety_violation`).
- `metadata` stocke des hashes de bundles de preuve, des pointeurs de runbook ou des IDs de regulateur.

Les slashes se propagent aux delegators selon la policy de gouvernance (perte proportionnelle ou
validator-first). La logique runtime emettra des annotations `PublicLaneRewardRecord` une fois NX-9 en place.

### 2.6 `RecordPublicLaneRewards`

Enregistre le payout pour un epoch. Champs :

- `reward_asset` : asset distribue (defaut `xor#nexus`).
- `total_reward` : total mint/transfer.
- `shares` : vecteur d'entrees `PublicLaneRewardShare`.
- `metadata` : references aux transactions de payout, hashes racine ou dashboards.

Cet ISI est idempotent par `(lane_id, epoch)` et sous-tend la comptabilite nocturne.

## 3. Operations, cycle de vie et tooling

- **Cycle de vie + modes :** les lanes stake-elected sont actives via
  `nexus.staking.public_validator_mode = stake_elected` tandis que les lanes restreintes restent
  admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`). Les deployments permissioned
  gardent les peers genesis actifs jusqu'a ce que le stake existe ; pour les lanes stake-elected nous
  exigeons toujours un peer enregistre avec une cle de consensus active dans la topologie de commit
  avant que `RegisterPublicLaneValidator` ne reussisse. Les fingerprints genesis et
  `use_stake_snapshot_roster` determinent si le runtime derive le roster depuis les snapshots de stake
  ou se rabat sur les peers genesis.
- **Operations activation/sortie :** les registrations arrivent en `PendingActivation` pour
  `current_epoch + 1` (genesis bootstrap uses `current_epoch`) et sont auto-promues au premier bloc dont l'epoch atteint la limite planifiee
  (`epoch_length_blocks`). Les operateurs peuvent
  aussi appeler `ActivatePublicLaneValidator` apres la limite pour forcer la promotion. Les sorties
  deplacent les validateurs en `Exiting(release_at_ms)` et ne liberent de capacite que lorsque le timestamp
  de bloc atteint `release_at_ms` ; une re-registration apres slash requiert encore une sortie pour que
  l'enregistrement soit marque `Exited` et que la capacite soit recuperee. Les checks de capacite utilisent
  `nexus.staking.max_validators` et s'executent apres le finalizer de sortie, donc les sorties futures
  bloquent les nouvelles registrations jusqu'a expiration du timer.
- **Config knobs :** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`,
  `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, et les switches de mode ci-dessus.
  Les passer via `iroha_config::parameters::actual::Nexus` et les exposer dans `status.md` une fois les
  valeurs GA ratifiees.
- **Torii/CLI quickstart :**
  - `iroha app nexus lane-report --summary` affiche les entrees du catalogue lane, la readiness des manifests,
    et les modes de validateurs (stake-elected vs admin-managed) pour que les operateurs confirment si
    l'admission staking est activee pour une lane.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary] [--address-format {ih58,compressed}]`
    expose les marqueurs de cycle de vie/activation (epoch cible pending, `activation_epoch` /
    `activation_height`, release de sortie, slash id) avec le stake bonded/self.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator ih58...] [--summary]`
    reflete le endpoint `/stake` avec des hints pending-unbond par paire `(validator, staker)`.
  - Snapshots Torii pour dashboards et SDKs :
    - `GET /v1/nexus/public_lanes/{lane}/validators` - metadata, statut
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), epoch/height d'activation,
      timers de release, stake bonded, dernier epoch de reward.
      `address_format=ih58|compressed` controle le rendu des litteraux.
    - `GET /v1/nexus/public_lanes/{lane}/stake` - shares de stake (`validator`,
      `staker`, montant bonded) plus timers pending unbond. `?validator=ih58...`
      filtre la reponse pour les dashboards focalises sur un validateur ; `address_format`
      s'applique a tous les litteraux.
  - Les ISIs de cycle de vie utilisent le chemin transaction standard (Torii
    `/v1/transactions` ou le pipeline d'instructions CLI). Exemples de payloads Norito JSON :

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetrie + runbooks :** les metriques exposent le nombre de validateurs, le stake bonded et pending,
  les totaux de rewards, et les compteurs de slash sous la famille `nexus_public_lane_*`. Connecter les
  dashboards au meme set de donnees utilise par les tests d'acceptance NX-9 pour que les deltas de
  validateurs et les preuves de reward/slash restent auditable. Les instructions de slashing restent
  gouvernance-only ; l'enregistrement des rewards doit prouver les totaux de payout (hash du batch de payout).

## 4. Alignement avec le roadmap

- OK Runtime et storages WSV implementent le cycle de vie des validateurs NX-9 ; les regressions couvrent
  le timing d'activation, les prerequis de peers, les sorties differees, et la re-registration apres slashes.
- OK Torii expose `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` en Norito JSON pour
  que SDKs et dashboards surveillent l'etat de la lane sans RPCs custom.
- OK Les config knobs et la telemetrie sont documentees ; les deployments mixtes maintiennent les lanes
  stake-elected et admin-managed isolees pour que les rosters de validateurs restent deterministes.

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
