---
id: deal-engine
title: SoraFS V1 Ledger Economics
sidebar_label: Ledger Economics
description: Canonical V1 orderbook, reserve/rent, and billing authority.
---

# SoraFS V1 ledger-authoritative economics

The retired process-local agreement service is not a V1 surface. Provider and client custody, matching, usage accrual, and settlement are authoritative only through native orderbook and reserve/rent instructions, finalized typed queries and events, and the supervised hedging/billing projection. Clients submit signed transactions and reconcile committed ledger state.

Pre-release local checkpoints and HTTP balance-mutation endpoints are intentionally unsupported. Development state created by the retired service must be discarded and reseeded.
