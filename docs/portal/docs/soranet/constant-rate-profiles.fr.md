---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e41e5ed0d7b74fe8ea1ac5bda290088ebb54572db240ae9c2546d719e0c6815f
source_last_modified: "2025-11-19T13:44:41.615366+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: constant-rate-profiles
title: Profils de taux constant SoraNet
sidebar_label: Profils a taux constant
description: Catalogue de presets SNNet-17B1 pour relays core/home en production et profil null de dogfood SNNet-17A2, avec maths tick->bandwidth, helpers CLI et garde-fous MTU.
---

:::note Source canonique
:::

SNNet-17B introduit des lanes de transport a taux fixe afin que les relays deplacent le trafic en cellules de 1,024 B quel que soit la taille du payload. Les operateurs choisissent parmi trois presets:

- **core** - relays en data-center ou heberges professionnellement pouvant dedier >=30 Mbps au trafic.
- **home** - operateurs residentiels ou a faible uplink qui ont besoin de fetches anonymes pour les circuits sensibles a la vie privee.
- **null** - preset dogfood SNNet-17A2. Il conserve les memes TLVs/envelope mais allonge le tick et le ceiling pour du staging a faible bande passante.

## Resume des presets

| Profil | Tick (ms) | Cellule (B) | Cap de lanes | Plancher dummy | Payload par lane (Mb/s) | Payload plafond (Mb/s) | % uplink au plafond | Uplink recommande (Mb/s) | Cap de voisins | Declencheur d'auto-desactivation (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Cap de lanes** - nombre maximal de voisins a taux constant en parallele. Le relay rejette les circuits excedentaires une fois le cap atteint et incremente `soranet_handshake_capacity_reject_total`.
- **Plancher dummy** - nombre minimum de lanes maintenues en vie avec du trafic dummy meme lorsque la demande reelle est plus faible.
- **Payload plafond** - budget d'uplink dedie aux lanes a taux constant apres application de la fraction de plafond. Les operateurs ne doivent jamais depasser ce budget meme si de la bande passante supplementaire est disponible.
- **Declencheur d'auto-desactivation** - pourcentage de saturation soutenu (moyenne par preset) qui fait redescendre le runtime au plancher dummy. La capacite est restauree apres le seuil de recovery (75% pour `core`, 60% pour `home`, 45% pour `null`).

**Important:** le preset `null` est reserve au staging et au dogfooding; il ne satisfait pas les garanties de vie privee requises pour les circuits de production.

## Table tick -> bandwidth

Chaque cellule payload transporte 1,024 B, donc la colonne KiB/sec equivaut au nombre de cellules emises par seconde. Utilisez le helper pour etendre la table avec des ticks personnalises.

| Tick (ms) | Cellules/sec | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

Formule:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

Helper CLI:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` emet des tables style GitHub pour le resume des presets et la table de ticks optionnelle afin que vous puissiez coller la sortie deterministe dans le portail. Associez-le a `--json-out` pour archiver les donnees rendues comme preuve de gouvernance.

## Configuration et overrides

`tools/soranet-relay` expose les presets via les fichiers de configuration et les overrides runtime:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

La cle de configuration accepte `core`, `home` ou `null` (defaut `core`). Les overrides CLI sont utiles pour les drills de staging ou les demandes SOC qui reduisent temporairement le duty cycle sans re-ecrire les configs.

## Garde-fous MTU

- Les cellules payload utilisent 1,024 B plus ~96 B de framing Norito+Noise et les en-tetes QUIC/UDP minimaux, ce qui maintient chaque datagramme sous le MTU IPv6 minimum de 1,280 B.
- Lorsque des tunnels (WireGuard/IPsec) ajoutent une encapsulation supplementaire, vous **devez** reduire `padding.cell_size` pour que `cell_size + framing <= 1,280 B`. Le validateur du relay impose `padding.cell_size <= 1,136 B` (1,280 B - 48 B d'overhead UDP/IPv6 - 96 B de framing).
- Les profils `core` doivent epingler >=4 voisins meme a l'etat idle afin que les lanes dummy couvrent toujours un sous-ensemble de guards PQ. Les profils `home` peuvent limiter les circuits a taux constant aux wallets/aggregators mais doivent appliquer une back-pressure lorsque la saturation depasse 70% pendant trois fenetres de telemetry.

## Telemetry et alertes

Les relays exportent les metriques suivantes par preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alerter lorsque:

1. Le ratio dummy reste sous le plancher du preset (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) pendant plus de deux fenetres.
2. `soranet_constant_rate_ceiling_hits_total` augmente plus vite qu'un hit toutes les cinq minutes.
3. `soranet_constant_rate_degraded` passe a `1` en dehors d'un drill planifie.

Consignez le label du preset et la liste des voisins dans les rapports d'incident afin que les auditeurs puissent prouver que les politiques a taux constant respectent les exigences de la roadmap.
