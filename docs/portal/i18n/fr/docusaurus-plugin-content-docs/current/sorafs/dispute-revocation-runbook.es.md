---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : litige-révocation-runbook
titre : Runbook des litiges et des révocations de SoraFS
sidebar_label : Runbook des litiges et des révocations
description : Flux de gouvernement pour présenter les différends de capacité de SoraFS, coordonner les révocations et évacuer les données de forme déterministe.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/dispute_revocation_runbook.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que la documentation héritée de Sphinx soit retirée.
:::

## Proposition

Ce runbook guide les opérateurs de gouvernance pour présenter les litiges de capacité de SoraFS, coordonner les révocations et garantir que l'évacuation des données est complète de forme déterministe.

## 1. Évaluer l'incident

- **Conditions d'activation :** détection de l'absence de SLA (temps d'activité/chute de PoR), déficit de réplication ou désacuerdo de facturación.
- **Confirmer la télémétrie :** capture les instantanés de `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` pour le fournisseur.
- **Notificar a las partes interesadas :** Équipe de stockage (operaciones del provenedor), Conseil de gouvernance (órgano decisorio), Observabilité (actualisation des tableaux de bord).

## 2. Préparer le paquet de preuves1. Recopier les artefacts en brut (télémétrie JSON, journaux de CLI, notes d'audit).
2. Normaliser un fichier déterministe (par exemple, une archive tar) ; inscription :
   - résumé BLAKE3-256 (`evidence_digest`)
   - type de média (`application/zip`, `application/jsonl`, etc.)
   - URI d'hébergement (stockage d'objets, broche de SoraFS ou point de terminaison accessible par Torii)
3. Gardez le paquet dans le seau de collecte des preuves de gouvernement avec accès à l'écriture unique.

## 3. Présenter le litige

1. Créez une spécification JSON pour `sorafs_manifest_stub capacity dispute` :

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

2. Exécutez la CLI :

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Révision `dispute_summary.json` (confirmation du type, résumé des preuves et horodatages).
4. Envoyez le JSON de la sollicitation au Torii `/v1/sorafs/capacity/dispute` en passant par le colis de transactions d'État. Capturer la valeur de réponse `dispute_id_hex` ; ainsi que les actions de révocation postérieures et les informations de l'auditoire.

## 4. Évacuation et révocation1. **Vente de grâce :** notifier au fournisseur la révocation imminente ; permettre l'évacuation des données fijados lorsque la politique le permet.
2. **Généres `ProviderAdmissionRevocationV1` :**
   - Utilisez `sorafs_manifest_stub provider-admission revoke` avec la raison approuvée.
   - Verifica firmas y el digest de revocación.
3. **Publica la révocación:**
   - Envoyez la demande de révocation au Torii.
   - Assurez-vous que les publicités du fournisseur sont bloquées (en espérant que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` augmente).
4. **Actualiser les tableaux de bord :** marquer le fournisseur comme déclaré, faire référence à l'ID du litige et joindre le paquet de preuves.

## 5. Post-mortem et suivi

- Enregistrez la ligne de temps, la cause raisonnable et les actions de remédiation dans le suivi des incidents de gouvernement.
- Détermination de la restitución (réduction de la participation, récupération des commissions, remboursement des clients).
- Documents d'apprentissage ; Actualisez les ombrelles de SLA ou les alertes de surveillance si nécessaire.

## 6. Matériaux de référence

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (section des litiges)
- `docs/source/sorafs/provider_admission_policy.md` (flux de révocation)
- Tableau de bord d'observabilité : `SoraFS / Capacity Providers`

## Liste de contrôle- [ ] Paquet de preuves capturées et hasheado.
- [ ] Charge utile de litige validé localement.
- [ ] Transaction de litige en Torii acceptée.
- [ ] Revocación ejecutada (si fue aprobada).
- [ ] Tableaux de bord/runbooks actualisés.
- [ ] Présentation post-mortem avant le conseil d'administration.