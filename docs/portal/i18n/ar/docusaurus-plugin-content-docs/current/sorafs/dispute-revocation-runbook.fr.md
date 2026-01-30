---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: dispute-revocation-runbook
title: Runbook des litiges et révocations SoraFS
sidebar_label: Runbook litiges et révocations
description: Flux de gouvernance pour déposer des litiges de capacité SoraFS, coordonner les révocations et évacuer les données de manière déterministe.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/dispute_revocation_runbook.md`. Gardez les deux copies synchronisées jusqu’à ce que la documentation Sphinx héritée soit retirée.
:::

## Objectif

Ce runbook guide les opérateurs de gouvernance dans la création de litiges de capacité SoraFS, la coordination des révocations et la garantie d’une évacuation déterministe des données.

## 1. Évaluer l’incident

- **Conditions de déclenchement :** détection d’une violation du SLA (disponibilité/échec PoR), déficit de réplication ou désaccord de facturation.
- **Confirmer la télémétrie :** capturer les snapshots `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` pour le fournisseur.
- **Notifier les parties prenantes :** Storage Team (opérations du fournisseur), Governance Council (organe décisionnel), Observability (mises à jour des dashboards).

## 2. Préparer le bundle de preuves

1. Collecter les artefacts bruts (telemetry JSON, logs CLI, notes d’audit).
2. Normaliser dans une archive déterministe (par exemple, un tarball) ; consigner :
   - digest BLAKE3-256 (`evidence_digest`)
   - type de média (`application/zip`, `application/jsonl`, etc.)
   - URI d’hébergement (object storage, pin SoraFS ou endpoint accessible via Torii)
3. Stocker le bundle dans le bucket de collecte des preuves de gouvernance avec un accès write-once.

## 3. Déposer le litige

1. Créez un JSON spec pour `sorafs_manifest_stub capacity dispute` :

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Lancez la CLI :

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. Vérifiez `dispute_summary.json` (confirmez le type, le digest des preuves, les timestamps).
4. Soumettez le JSON de requête à Torii `/v1/sorafs/capacity/dispute` via la file de transactions de gouvernance. Capturez la valeur de réponse `dispute_id_hex` ; elle ancre les actions de révocation suivantes et les rapports d’audit.

## 4. Évacuation et révocation

1. **Fenêtre de grâce :** avertissez le fournisseur de la révocation imminente ; autorisez l’évacuation des données épinglées lorsque la politique le permet.
2. **Générez `ProviderAdmissionRevocationV1` :**
   - Utilisez `sorafs_manifest_stub provider-admission revoke` avec la raison approuvée.
   - Vérifiez les signatures et le digest de révocation.
3. **Publiez la révocation :**
   - Soumettez la requête de révocation à Torii.
   - Assurez-vous que les adverts du fournisseur sont bloqués (attendez une hausse de `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Mettez à jour les dashboards :** signalez le fournisseur comme révoqué, référencez l’ID du litige et liez le bundle de preuves.

## 5. Post-mortem et suivi

- Enregistrez la chronologie, la cause racine et les actions de remédiation dans le tracker d’incidents de gouvernance.
- Déterminez la restitution (slashing du stake, clawbacks de frais, remboursements clients).
- Documentez les leçons ; mettez à jour les seuils SLA ou les alertes de monitoring si nécessaire.

## 6. Documents de référence

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (section litiges)
- `docs/source/sorafs/provider_admission_policy.md` (workflow de révocation)
- Dashboard d’observabilité : `SoraFS / Capacity Providers`

## Checklist

- [ ] Bundle de preuves capturé et haché.
- [ ] Payload de litige validé localement.
- [ ] Transaction de litige Torii acceptée.
- [ ] Révocation exécutée (si approuvée).
- [ ] Dashboards/runbooks mis à jour.
- [ ] Post-mortem déposé auprès du conseil de gouvernance.
