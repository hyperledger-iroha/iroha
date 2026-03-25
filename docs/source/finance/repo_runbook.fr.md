---
lang: fr
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Runbook de règlement des pensions

Ce guide documente le flux déterministe des accords de pension et de prise en pension dans Iroha.
Il couvre l'orchestration CLI, les assistants SDK et les boutons de gouvernance attendus afin que les opérateurs puissent
initier, marger et conclure des accords sans écrire de charges utiles Norito brutes. Pour la gouvernance
listes de contrôle, capture de preuves et procédures de fraude/annulation, voir
[`repo_ops.md`](./repo_ops.md), qui satisfait à l'élément F1 de la feuille de route.

## Commandes CLI

La commande `iroha app repo` regroupe les assistants spécifiques au dépôt :

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` et `repo unwind` respectent `--input/--output` donc le `InstructionBox` généré
  les charges utiles peuvent être redirigées vers d’autres flux CLI ou soumises immédiatement.
* Transmettez `--custodian <account>` pour acheminer la garantie vers un dépositaire tripartite. Lorsqu'il est omis, le
  la contrepartie reçoit le gage directement (repo bilatéral).
* `repo margin` interroge le grand livre via `FindRepoAgreements` et rapporte la prochaine marge attendue
  horodatage (en millisecondes) indiquant si un rappel de marge est actuellement dû.
* `repo margin-call` ajoute une instruction `RepoMarginCallIsi`, enregistrant le point de contrôle de marge et
  émettre des événements pour tous les participants. Les appels sont rejetés si la cadence n'est pas écoulée ou si le
  l’instruction est soumise par un non-participant.

## Assistants du SDK Python

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="i105...",
    counterparty="i105...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* Les deux assistants normalisent les quantités numériques et les champs de métadonnées avant d'appeler les liaisons PyO3.
* `RepoAgreementRecord` reflète le calcul du calendrier d'exécution afin que l'automatisation hors grand livre puisse
  déterminer quand les rappels sont dus sans recalculer la cadence manuellement.

## Règlements DvP / PvP

La commande `iroha app settlement` étape les instructions de livraison contre paiement et de paiement contre paiement :

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from i105... \
  --payment-to i105... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* Les quantités de jambe acceptent des valeurs intégrales ou décimales et sont validées par rapport à la précision de l'actif.
* `--atomicity` accepte `all-or-nothing`, `commit-first-leg` ou `commit-second-leg`. Utilisez ces modes
  avec `--order` pour exprimer quelle branche reste validée si le traitement ultérieur échoue (`commit-first-leg`
  maintient la première jambe appliquée ; `commit-second-leg` conserve le second).
* Les invocations CLI émettent aujourd'hui des métadonnées d'instruction vides ; utiliser les assistants Python au niveau du règlement
  les métadonnées doivent être jointes.
* Voir [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) pour le mappage de champ ISO 20022 qui
  soutient ces instructions (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Transmettez `--iso-xml-out <path>` pour que la CLI émette un aperçu XML canonique aux côtés du Norito.
  instruction; le fichier suit le mappage ci-dessus (`sese.023` pour DvP, `sese.025` pour PvP`). Associez le
  indicateur avec `--iso-reference-crosswalk <path>` afin que la CLI vérifie `--delivery-instrument-id` par rapport au
  même instantané que Torii utilise lors de l’admission à l’exécution.

Les assistants Python reflètent la surface CLI :

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## Déterminisme et attentes en matière de gouvernance

Les instructions Repo reposent exclusivement sur les types numériques codés Norito et sur les valeurs partagées.
Logique `RepoGovernance::with_defaults`. Gardez à l’esprit les invariants suivants :* Les quantités sont sérialisées avec des valeurs déterministes `NumericSpec` : utilisation des branches de trésorerie
  `fractional(2)` (deux décimales), les branches collatérales utilisent `integer()`. Ne pas soumettre
  valeurs avec une plus grande précision : les gardes d'exécution les rejetteront et les pairs divergeront.
* Les dépôts tripartites conservent l'identifiant du compte dépositaire dans `RepoAgreement`. Événements de cycle de vie et de marge
  émettre une charge utile `RepoAccountRole::Custodian` afin que les dépositaires puissent s'abonner et rapprocher l'inventaire.
* Les coupes de cheveux sont limitées à 10 000 bps (100 %) et les fréquences de marge sont des secondes entières. Fournir
  les paramètres de gouvernance dans ces unités canoniques pour rester alignés sur les attentes d’exécution.
* Les horodatages sont toujours en millisecondes Unix. Tous les assistants les transmettent tels quels au Norito
  charge utile afin que les pairs obtiennent des horaires identiques.
* Les instructions d'initiation et de déroulement réutilisent le même identifiant d'accord. Le runtime rejette
  dupliquer les identifiants et se déroule pour les accords inconnus ; Les assistants CLI/SDK détectent ces erreurs plus tôt.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` renvoie la cadence canonique. Toujours
  consultez cet instantané avant de déclencher des rappels pour éviter de rejouer des plannings obsolètes.