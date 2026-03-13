---
lang: fr
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Disponibilité des données Politique de loyer et d'incitation (DA-7)

_Statut : Rédaction — Propriétaires : Groupe de travail sur l'économie / Trésorerie / Équipe de stockage_

L'élément **DA-7** de la feuille de route introduit un loyer explicite libellé en XOR pour chaque blob
soumis à `/v2/da/ingest`, plus des bonus qui récompensent l'exécution PDP/PoTR et
la sortie servait à aller chercher les clients. Ce document définit les paramètres initiaux,
leur représentation du modèle de données et le workflow de calcul utilisé par Torii,
SDK et tableaux de bord de trésorerie.

## Structure de la politique

La stratégie est codée comme [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
au sein du modèle de données. Torii et les outils de gouvernance maintiennent la politique dans
Charges utiles Norito afin que les devis de loyer et les registres d'incitation puissent être recalculés
déterministe. Le schéma expose cinq boutons :

| Champ | Descriptif | Par défaut |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR facturé par GiB et par mois de rétention. | `250_000` micro-XOR (0,25 XOR) |
| `protocol_reserve_bps` | Part du loyer acheminée vers la réserve protocolaire (points de base). | `2_000` (20%) |
| `pdp_bonus_bps` | Pourcentage de bonus par évaluation PDP réussie. | `500` (5%) |
| `potr_bonus_bps` | Pourcentage de bonus par évaluation PoTR réussie. | `250` (2,5%) |
| `egress_credit_per_gib` | Crédit payé lorsqu'un fournisseur sert 1 Go de données DA. | `1_500` micro-XOR |

Toutes les valeurs en points de base sont validées par rapport à `BASIS_POINTS_PER_UNIT` (10 000).
Les mises à jour de stratégie doivent transiter par la gouvernance, et chaque nœud Torii expose le
politique active via la section de configuration `torii.da_ingest.rent_policy`
(`iroha_config`). Les opérateurs peuvent remplacer les valeurs par défaut dans `config.toml` :

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

L'outil CLI (`iroha app da rent-quote`) accepte les mêmes entrées de stratégie Norito/JSON.
et émet des artefacts qui reflètent le `DaRentPolicyV1` actif sans atteindre
revenir à l'état Torii. Fournissez l'instantané de stratégie utilisé pour une exécution d'ingestion afin que
la citation reste reproductible.

### Artéfacts persistants des devis de loyer

Exécutez `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` pour
émettre à la fois le résumé à l'écran et un joli artefact JSON imprimé. Le dossier
enregistre `policy_source`, l'instantané `DaRentPolicyV1` en ligne, le calcul
`DaRentQuote` et un `ledger_projection` dérivé (sérialisé via
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)), ce qui le rend adapté aux tableaux de bord de trésorerie et au grand livre ISI
canalisations. Lorsque `--quote-out` pointe sur un répertoire imbriqué, la CLI crée n'importe quel
parents manquants, afin que les opérateurs puissent normaliser les emplacements tels que
`artifacts/da/rent_quotes/<timestamp>.json` aux côtés d’autres ensembles de preuves DA.
Joignez l'artefact aux approbations de location ou aux exécutions de rapprochement afin que le XOR
la répartition (loyer de base, réserve, bonus PDP/PoTR et crédits de sortie) est
reproductible. Passez `--policy-label "<text>"` pour remplacer automatiquement le
description `policy_source` dérivée (chemins de fichiers, valeur par défaut intégrée, etc.) avec un
une balise lisible par l'homme telle qu'un ticket de gouvernance ou un hachage de manifeste ; les garnitures CLI
cette valeur et rejette les chaînes vides/espaces uniquement afin que les preuves enregistrées
reste vérifiable.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```La section de projection du grand livre alimente directement les ISI du grand livre des loyers DA : elle
définit les deltas XOR destinés à la réserve de protocole, aux paiements du fournisseur et
les pools de bonus par épreuve sans nécessiter de code d'orchestration sur mesure.

### Générer des plans de grand livre de loyers

Exécutez `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
pour convertir un devis de loyer persistant en transferts de grand livre exécutables. La commande
analyse le `ledger_projection` intégré, émet les instructions Norito `Transfer`
qui collectent le loyer de base dans le trésor, achemine la réserve/le fournisseur
portions et préfinance les pools de bonus PDP/PoTR directement auprès du payeur. Le
la sortie JSON reflète les métadonnées de la cotation afin que les outils CI et de trésorerie puissent raisonner
à propos du même artefact :

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

Le champ final `egress_credit_per_gib_micro_xor` permet les tableaux de bord et les paiements
les planificateurs alignent les remboursements de sortie sur la politique de loyer qui a produit le
citer sans recalculer les calculs de politique dans la colle de script.

## Exemple de devis

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

Le devis est reproductible sur les nœuds Torii, les SDK et les rapports de trésorerie, car
il utilise des structures déterministes Norito au lieu de mathématiques ad hoc. Les opérateurs peuvent
joindre le code JSON/CBOR `DaRentPolicyV1` aux propositions de gouvernance ou louer
des audits pour prouver quels paramètres étaient en vigueur pour un blob donné.

## Bonus et réserves

- **Réserve de protocole :** `protocol_reserve_bps` finance la réserve XOR qui soutient
  réplication d’urgence et réduction des remboursements. Le Trésor suit ce compartiment
  séparément pour garantir que les soldes du grand livre correspondent au taux configuré.
- **Bonus PDP/PoTR :** Chaque évaluation de preuve réussie reçoit un supplément
  paiement dérivé de `base_rent × bonus_bps`. Lorsque le planificateur DA émet une preuve
  reçus, il comprend les balises de points de base afin que les incitations puissent être rejouées.
- **Crédit de sortie :** Les fournisseurs enregistrent les GiB servis par manifeste, multiplient par
  `egress_credit_per_gib` et soumettez les reçus via `iroha app da prove-availability`.
  La politique de loyer maintient le montant par Go en phase avec la gouvernance.

## Flux opérationnel

1. **Ingérer :** `/v2/da/ingest` charge le `DaRentPolicyV1` actif, indique le loyer
   en fonction de la taille et de la rétention du blob, et intègre le devis dans le Norito
   manifeste. Le demandeur signe une déclaration faisant référence au hachage du loyer et
   l’identifiant du ticket de stockage.
2. **Comptabilité :** Les scripts d'ingestion du Trésor décodent le manifeste, appellent
   `DaRentPolicyV1::quote`, et renseigner les livres de loyers (loyer de base, réserve,
   bonus et crédits de sortie attendus). Tout écart entre le loyer enregistré
   et les cotations recalculées échouent à CI.
3. **Récompenses de preuve :** Lorsque les planificateurs PDP/PoTR marquent un succès, ils émettent un reçu
   contenant le résumé du manifeste, le type de preuve et le bonus XOR dérivé de
   la politique. La gouvernance peut auditer les paiements en recalculant le même devis.
4. **Remboursement de sortie :** Les orchestrateurs de récupération soumettent des résumés de sortie signés.
   Torii multiplie le nombre de GiB par `egress_credit_per_gib` et émet le paiement
   instructions contre le séquestre du loyer.

## TélémétrieLes nœuds Torii exposent l'utilisation des loyers via les métriques Prometheus suivantes (étiquettes :
`cluster`, `storage_class`) :

- `torii_da_rent_gib_months_total` — Gio-mois cités par `/v2/da/ingest`.
- `torii_da_rent_base_micro_total` — loyer de base (micro XOR) accumulé lors de l'absorption.
- `torii_da_protocol_reserve_micro_total` — contributions à la réserve de protocole.
- `torii_da_provider_reward_micro_total` — paiements de loyer du côté du fournisseur.
- `torii_da_pdp_bonus_micro_total` et `torii_da_potr_bonus_micro_total` —
  Pools de bonus PDP/PoTR provenant du devis d'ingestion.

Les tableaux de bord économiques s'appuient sur ces compteurs pour garantir les ISI du grand livre, les prélèvements de réserves,
et les barèmes de bonus PDP/PoTR correspondent tous aux paramètres politiques en vigueur pour chaque
cluster et classe de stockage. La carte SoraFS Capacité Santé Grafana
(`dashboards/grafana/sorafs_capacity_health.json`) affiche désormais des panneaux dédiés
pour la distribution des loyers, l'accumulation de bonus PDP/PoTR et la capture de GiB-mois, permettant
Treasury doit filtrer par cluster ou classe de stockage Torii lors de l'examen de l'ingestion
volume et paiements.

## Étapes suivantes

- ✅ Les reçus `/v2/da/ingest` intègrent désormais `rent_quote` et les surfaces CLI/SDK affichent le prix cité.
  loyer de base, part de réserve et bonus PDP/PoTR afin que les soumissionnaires puissent consulter les obligations XOR avant
  engager des charges utiles.
- Intégrer le grand livre des loyers aux prochains flux de réputation/carnet de commandes DA
  pour prouver que les fournisseurs de haute disponibilité reçoivent les paiements corrects.