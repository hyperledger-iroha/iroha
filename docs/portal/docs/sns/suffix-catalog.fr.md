---
lang: fr
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d23c9d6a77942b5918b933631b890addbc0ecdfef51e9ff427a4069d2cc37902
source_last_modified: "2025-11-15T16:27:31.089720+00:00"
translation_last_reviewed: 2026-01-01
---

# Catalogue des suffixes du Sora Name Service

Le roadmap SNS suit chaque suffixe approuve (SN-1/SN-2). Cette page reflete le
catalogue source de verite afin que les operateurs executant des registrars, des
DNS gateways ou des outils de wallet puissent charger les memes parametres sans
scraper les docs de statut.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consommateurs:** `iroha sns policy`, kits d'onboarding SNS, dashboards KPI, et
  scripts de release DNS/Gateway lisent le meme bundle JSON.
- **Statuts:** `active` (enregistrements autorises), `paused` (temporairement gate),
  `revoked` (annonce mais pas disponible actuellement).

## Schema du catalogue

| Champ | Type | Description |
|-------|------|-------------|
| `suffix` | string | Suffixe lisible par l'humain avec point initial. |
| `suffix_id` | `u16` | Identifiant stocke on-ledger dans `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` ou `revoked` decrivant la preparation au lancement. |
| `steward_account` | string | Compte responsable du stewardship (correspond aux hooks de politique du registrar). |
| `fund_splitter_account` | string | Compte qui recoit les paiements avant le routage selon `fee_split`. |
| `payment_asset_id` | string | Actif utilise pour le settlement (`61CtjvNd9T3THAR65GsMVHr82Bjc` pour la cohorte initiale). |
| `min_term_years` / `max_term_years` | integer | Bornes de terme d'achat depuis la politique. |
| `grace_period_days` / `redemption_period_days` | integer | Fenetres de securite de renouvellement appliquees par Torii. |
| `referral_cap_bps` | integer | Carve-out maximal de referral autorise par la gouvernance (basis points). |
| `reserved_labels` | array | Objets de label proteges par la gouvernance `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Objets de tier avec `label_regex`, `base_price`, `auction_kind`, et bornes de duree. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` repartition en basis points. |
| `policy_version` | integer | Compteur monotone incremente quand la gouvernance edite la politique. |

## Catalogue actuel

| Suffixe | ID (`hex`) | Steward | Fund splitter | Statut | Actif de paiement | Plafond referral (bps) | Terme (min - max ans) | Grace / Redemption (jours) | Tiers de prix (regex -> prix de base / enchere) | Labels reserves | Repartition des fees (T/S/R/E bps) | Version de politique |
|---------|------------|---------|---------------|--------|------------------|------------------------|------------------------|----------------------------|------------------------------------------------|----------------|-----------------------------------|--------------------|
| `.sora` | `0x0001` | `soraカタカナ...` | `soraカタカナ...` | Actif | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> soraカタカナ...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `soraカタカナ...` | `soraカタカナ...` | En pause | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> soraカタカナ...`, `guardian -> soraカタカナ...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `soraカタカナ...` | `soraカタカナ...` | Revoque | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## Extrait JSON

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "soraカタカナ...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## Notes d'automatisation

1. Charger le snapshot JSON et le hasher/signer avant distribution aux operateurs.
2. Les outils du registrar doivent exposer `suffix_id`, les limites de terme et la
   tarification du catalogue quand une requete atteint `/v1/sns/*`.
3. Les helpers DNS/Gateway lisent les metadonnees des labels reserves lors de la
   generation des templates GAR pour que les reponses DNS restent alignees avec les
   controles de gouvernance.
4. Les jobs d'annexes KPI taguent les exports de dashboards avec des metadonnees de
   suffixe pour que les alertes correspondent a l'etat de lancement enregistre ici.
