---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : litige-révocation-runbook
titre : Runbook des litiges et des révisions par SoraFS
sidebar_label : Runbook des litiges et des commentaires
description : Flux de gouvernance pour l'enregistrement des différends de capacité da SoraFS, coordonner les revogacoes et évacuer les données de forme déterministe.
---

:::note Fonte canonica
Cette page reflète `docs/source/sorafs/dispute_revocation_runbook.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx herdada seja retirada.
:::

## Proposition

Ce runbook guide les opérateurs de gouvernance pour l'ouverture des litiges de capacité de SoraFS, qui coordonne les changements et garantit que l'évacuation des données soit conclue de manière déterministe.

## 1. Avaliar ou incident

- **Conditions de sécurité :** détection de violation de SLA (uptime/falha de PoR), déficit de réplication ou divergence de cobranca.
- **Confirmer la télémétrie :** capturez des instantanés de `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` du fournisseur.
- **Notificar parts interessadas:** Storage Team (operacoes do provenor), Governance Council (orgao decisor), Observability (actualisation des tableaux de bord).

## 2. Préparez le paquet de preuves1. Colete artefatos brutos (télémétrie JSON, journaux de CLI, notes d'auditoire).
2. Normalisez-les dans un fichier déterministe (par exemple, une archive tar) ; enregistrer:
   - résumé BLAKE3-256 (`evidence_digest`)
   - type de midi (`application/zip`, `application/jsonl`, etc.)
   - URI de l'hébergement (stockage d'objets, broche par SoraFS ou point de terminaison passif via Torii)
3. Armazene ou pacote no bucket de evidencias da gouvernance com acesso write-once.

## 3. Enregistrer un litige

1. Criez une spécification JSON pour `sorafs_manifest_stub capacity dispute` :

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

2. Exécutez une CLI :

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. Révisez `dispute_summary.json` (confirmez le type, digérez les preuves et les horodatages).
4. Envie du JSON requis pour Torii `/v1/sorafs/capacity/dispute` via le fil des transactions de gouvernance. Capturer la valeur de la réponse `dispute_id_hex` ; ele ancora comme acoes de revogacao posteriores et os relatorios de auditoria.

## 4. Évacuation et révocation1. **Janela de graca:** notifique o provenor sobre a revogacao iminente; permettez l'évacuation des données fixées lorsque la politique autorise.
2. **Voir `ProviderAdmissionRevocationV1` :**
   - Utilisez `sorafs_manifest_stub provider-admission revoke` avec le motif approuvé.
   - Vérifiez les assinaturas et le digest de revogacao.
3. **Publique à revogacao :**
   - Envie d'une demande de revogacao pour Torii.
   - Garanta que os adverts do provenor estejam bloqueados (espéra-se que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` augmente).
4. **Actualiser les tableaux de bord :** marque le fournisseur comme revogado, référence à l'ID du litige et à la conclusion du paquet de preuves.

## 5. Post-mortem et accompagnement

- Enregistrez la ligne du tempo, la cause en cause et les mesures correctives dans le suivi des incidents de gouvernance.
- Déterminer une restitucao (réduction de la participation, récupération des taxas, reembolsos aos clients).
- Documentez les étudiants ; actualiser les limites de SLA ou les alertes de surveillance si nécessaire.

## 6. Matériel de référence

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (secao de contestas)
- `docs/source/sorafs/provider_admission_policy.md` (flux de revogacao)
- Tableau de bord d'observabilité : `SoraFS / Capacity Providers`

## Liste de contrôle

- [ ] Pacote de preuves capturées et hasheado.
- [ ] Charge utile du litige validé localement.
- [ ] Transacao de disputa no Torii aceita.
- [ ] Revogacao execuda (se aprovada).
- [ ] Tableaux de bord/runbooks actualisés.
- [ ] Post-mortem archivé conjointement avec le conseil de gouvernance.