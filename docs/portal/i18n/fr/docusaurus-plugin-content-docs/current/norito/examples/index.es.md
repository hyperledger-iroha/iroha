---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Exemples de Norito
description : Fragmentos de Kotodama sélectionnés avec des recorridos del libro mayor.
limace : /norito/exemples
---

Ces exemples reflètent les démarrages rapides du SDK et les enregistrements du livre principal. Chaque fragment comprend une liste de vérification du livre principal et comprend les guides de Rust, Python et JavaScript pour que vous puissiez répéter le même scénario de principe à la fin.

- **[Esquelette du point d'entrée Hajimari](./hajimari-entrypoint)** — Vous avez un minimum de contrat Kotodama avec un point d'entrée public unique et un gestionnaire d'état.
- **[Registrar dominio y acuñar activos](./register-and-mint)** — Vérifiez la création de domaines avec autorisation, le registre des actifs et la reconnaissance déterministe.
- **[Invoquer le transfert de l'hôte à partir de Kotodama](./call-transfer-asset)** — Vous pouvez indiquer comment un point d'entrée de Kotodama peut appeler les instructions de l'hôte `transfer_asset` avec validation des métadonnées en ligne.
- **[Transférer l'actif entre les comptes](./transfer-asset)** — Flux direct de transfert d'actifs qui reflète les démarrages rapides du SDK et les enregistrements du livre principal.
- **[Acuñar, transferir y quemar un NFT](./nft-flow)** — Enregistrez le cycle de vie d'un NFT de l'extrême à l'extrême : connaissance du propriétaire, transferts, étiquette des métadonnées et des choses.