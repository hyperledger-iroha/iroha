---
lang: fr
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2026-01-03T18:07:57.641039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Outils d'évaluation d'impact (MINFO‑4b)

Référence de la feuille de route : **MINFO‑4b — Outils d'évaluation d'impact.**  
Propriétaire : Conseil de gouvernance / Analytics

Cette note documente la commande `cargo xtask ministry-agenda impact` qui désormais
produit la différence automatisée de famille de hachage requise pour les paquets référendaires. Le
L'outil consomme les propositions validées du Conseil de l'Ordre du jour, le registre en double et
un instantané facultatif de liste de refus/de politique afin que les réviseurs puissent voir exactement lequel
les empreintes digitales sont nouvelles, qui entrent en conflit avec la politique existante et combien d'entrées
chaque famille de hachage contribue.

## Entrées

1. **Propositions d'ordre du jour.** Un ou plusieurs dossiers qui suivent
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Transmettez-les explicitement avec `--proposal <path>` ou pointez la commande vers un
   répertoire via `--proposal-dir <dir>` et chaque fichier `*.json` sous ce chemin
   est inclus.
2. **Registre en double (facultatif).** Un fichier JSON correspondant
   `docs/examples/ministry/agenda_duplicate_registry.json`. Les conflits sont
   signalé sous `source = "duplicate_registry"`.
3. **Instantané de stratégie (facultatif).** Un manifeste léger qui répertorie chaque
   empreinte digitale déjà appliquée par la politique du GAR/du Ministère. Le chargeur attend le
   schéma ci-dessous (voir
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   pour un échantillon complet) :

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

Toute entrée dont l'empreinte `hash_family:hash_hex` correspond à une cible de proposition est
signalé sous `source = "policy_snapshot"` avec la référence `policy_id`.

## Utilisation

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Des propositions supplémentaires peuvent être ajoutées via des indicateurs `--proposal` répétés ou en
fournir un répertoire contenant un lot référendaire complet :

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

La commande imprime le JSON généré sur la sortie standard lorsque `--out` est omis.

## Sortie

Le rapport est un artefact signé (enregistrez-le sous le dossier référendaire).
`artifacts/ministry/impact/`) avec la structure suivante :

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Joignez ce JSON à chaque dossier référendaire aux côtés du résumé neutre afin
les panélistes, les jurés et les observateurs de la gouvernance peuvent voir le rayon d'explosion exact de
chaque proposition. La sortie est déterministe (triée par famille de hachage) et sûre à
inclure dans CI/runbooks ; si le registre en double ou l'instantané de politique change,
réexécutez la commande et attachez l'artefact actualisé avant l'ouverture du vote.

> **Étape suivante :** introduisez le rapport d'impact généré dans
> [`cargo xtask ministry-panel packet`](referendum_packet.md) donc le
> Le dossier `ReferendumPacketV1` contient à la fois la répartition des familles de hachage et le
> liste détaillée des conflits pour la proposition en cours d'examen.