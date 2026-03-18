---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71585d0aa92f516519d69df408cf4255b90f5bce89ce8022f6c7b60871dc057b
source_last_modified: "2025-11-11T09:31:19.097792+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Exemples Norito
description: Extraits Kotodama sélectionnés avec des parcours du registre.
slug: /norito/examples
---

Ces exemples reflètent les quickstarts des SDK et les parcours du registre. Chaque extrait regroupe une liste de vérification du registre et renvoie vers les guides Rust, Python et JavaScript afin que vous puissiez rejouer le même scénario de bout en bout.

- **[Squelette du point d'entrée Hajimari](./hajimari-entrypoint)** — Structure minimale de contrat Kotodama avec un seul point d'entrée public et un gestionnaire d'état.
- **[Enregistrer un domaine et frapper des actifs](./register-and-mint)** — Démontre la création de domaines avec autorisations, l'enregistrement d'actifs et la frappe déterministe.
- **[Invoquer le transfert hôte depuis Kotodama](./call-transfer-asset)** — Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
- **[Transférer un actif entre comptes](./transfer-asset)** — Flux de transfert d'actifs simple qui reflète les quickstarts des SDK et les parcours du registre.
- **[Frapper, transférer et brûler un NFT](./nft-flow)** — Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.
