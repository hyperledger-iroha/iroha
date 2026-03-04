---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74b0ef4843c441003cd6630f35e0deac4a736adad450270047a739c1b1d0a6fc
source_last_modified: "2025-11-21T13:08:42.404970+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Onboarding des operateurs GAR
sidebar_label: Onboarding GAR
description: Checklist pour activer les politiques de compliance SNNet-9 avec digests d'attestation et capture d'evidence.
---

Utilisez ce brief pour deployer la configuration de compliance SNNet-9 avec un processus repetable et favorable aux audits. Associez-le a la revue juridictionnelle afin que chaque operateur utilise les memes digests et le meme format d'evidence.

## Etapes

1. **Assembler la configuration**
   - Importez `governance/compliance/soranet_opt_outs.json`.
   - Fusionnez vos `operator_jurisdictions` avec les digests d'attestation publies
     dans la [revue juridictionnelle](gar-jurisdictional-review).
2. **Valider**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Optionnel: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Capturer l'evidence**
   - Stocker sous `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (bloc de compliance final)
     - `attestations.json` (URIs + digests)
     - logs de validation
     - references vers PDFs/Norito envelopes signes
4. **Activer**
   - Taguer le rollout (`gar-opt-out-<date>`), redeployer les configs orchestrator/SDK,
     et confirmer que les evenements `compliance_*` apparaissent dans les logs attendus.
5. **Cloture**
   - Deposer le bundle d'evidence aupres du Governance Council.
   - Journaliser la fenetre d'activation et les approbateurs dans le GAR logbook.
   - Programmer les prochaines dates de revue depuis la table de revue juridictionnelle.

## Checklist rapide

- [ ] `jurisdiction_opt_outs` correspond au catalogue canonique.
- [ ] Digests d'attestation copies exactement.
- [ ] Commandes de validation executees et archivees.
- [ ] Bundle d'evidence stocke sous `artifacts/soranet/compliance/<date>/`.
- [ ] Tag de rollout + GAR logbook mis a jour.
- [ ] Rappels de prochaine revue configures.

## Voir aussi

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
