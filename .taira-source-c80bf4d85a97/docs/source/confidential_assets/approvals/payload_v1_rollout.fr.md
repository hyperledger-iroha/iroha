---
lang: fr
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Approbation du déploiement de Payload v1 (SDK Council, 2026-04-28).
//!
//! Capture la note de décision du Conseil SDK requise par `roadmap.md:M1` afin que le
//! Le déploiement de la charge utile chiffrée v1 a un enregistrement vérifiable (livrable M1.4).

# Décision de déploiement de Payload v1 (2026-04-28)

- **Président :** Responsable du conseil du SDK (M. Takemiya)
- **Membres votants :** Swift Lead, CLI Mainteneur, Confidential Assets TL, DevRel WG
- **Observateurs :** Gestion des programmes, opérations de télémétrie

## Entrées examinées

1. **Liaisons et soumissionnaires Swift** — `ShieldRequest`/`UnshieldRequest`, les soumissionnaires asynchrones et les assistants du générateur Tx ont atterri avec des tests de parité et docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **Ergonomie CLI** — L'assistant `iroha app zk envelope` couvre les flux de travail d'encodage/d'inspection ainsi que les diagnostics de panne, alignés sur les exigences ergonomiques de la feuille de route.【crates/iroha_cli/src/zk.rs:1256】
3. **Appareils déterministes et suites de parité** — appareil partagé + validation Rust/Swift pour conserver Norito octets/surfaces d'erreur aligné.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Décision

- **Approuver le déploiement de la charge utile v1** pour les SDK et CLI, permettant aux portefeuilles Swift de créer des enveloppes confidentielles sans plomberie sur mesure.
- **Conditions :** 
  - Gardez les appareils de parité sous les alertes de dérive CI (liées à `scripts/check_norito_bindings_sync.py`).
  - Documenter le playbook opérationnel dans `docs/source/confidential_assets.md` (déjà mis à jour via le Swift SDK PR).
  - Enregistrer les preuves d'étalonnage et de télémétrie avant de retourner les drapeaux de production (suivis sous M2).

## Éléments d'action

| Propriétaire | Article | À payer |
|-------|------|-----|
| Lead rapide | Annoncer la disponibilité de GA + extraits README | 2026-05-01 |
| Mainteneur CLI | Ajouter l'assistant `iroha app zk envelope --from-fixture` (facultatif) | Backlog (non bloquant) |
| Groupe de travail DevRel | Mettre à jour les démarrages rapides du portefeuille avec les instructions de charge utile v1 | 2026-05-05 |

> **Remarque :** Cette note remplace l'appel temporaire « en attente d'approbation du conseil » dans `roadmap.md:2426` et satisfait à l'élément de suivi M1.4. Mettez à jour `status.md` chaque fois que les éléments d’action de suivi se terminent.