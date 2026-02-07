---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : litige-révocation-runbook
titre : Ранбук споров и отзывов SoraFS
sidebar_label : Annonces et commentaires
description : Processus de gouvernance pour les activités liées aux biens SoraFS, coordination des opérations d'évacuation et des mesures d'évacuation.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/dispute_revocation_runbook.md`. Vous pouvez obtenir des copies de synchronisation si vous souhaitez créer une documentation sur Sphinx qui ne vous concerne pas.
:::

## Назначение

Ce projet assure la gouvernance des opérateurs en ce qui concerne les activités liées aux coûts SoraFS, en coordonnant les opérations et l'observation des décisions. эвакуации данных.

## 1. Оценить инцидент

- **Déclencheur :** permet de déterminer le SLA (temps de disponibilité/temps de disponibilité), de définir la réplication ou de définir la facturation.
- **Modifiez le téléphone :** indiquez les instantanés `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` pour le fournisseur.
- **Уведомить стейкхолдеров:** Équipe de stockage (операции провайдера), Conseil de gouvernance (орган решения), Observabilité (обновления дашбордов).

## 2. Ajouter un paquet de documents1. Sélectionnez les éléments d'art sélectionnés (télémétrie JSON, logiciel CLI, paramètres d'auditeur).
2. Нормализуйте в детерминированный архив (par exemple, tarball); зафиксируйте:
   - résumé BLAKE3-256 (`evidence_digest`)
   - type de support (`application/zip`, `application/jsonl` et т.д.)
   - Définition de l'URI (stockage d'objets, broche SoraFS ou point de terminaison, disponible à partir de Torii)
3. Placez le paquet dans le compartiment de preuves de gouvernance en écriture unique.

## 3. Suivre le sport

1. Téléchargez les spécifications JSON pour `sorafs_manifest_stub capacity dispute` :

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

2. Cliquez sur CLI :

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

3. Vérifiez le `dispute_summary.json` (selon le conseil, résumé des documents et des méthodes).
4. Déployez JSON dans Torii `/v1/sorafs/capacity/dispute` pour trouver la transition de gouvernance. Téléchargez la réponse `dispute_id_hex` ; Je viens de réaliser mon projet d'écoute et d'écoute.

## 4. Évacuation et évacuation1. **Окно льготы:** уведомите провайдера о грядущем отзыве; Les mesures d'évacuation sont prises dans le cadre de ces journées, mais c'est une politique double.
2. **Générez `ProviderAdmissionRevocationV1` :**
   - Utilisez le `sorafs_manifest_stub provider-admission revoke` avec l'inverse.
   - Проверьте подписи и digest отзыва.
3. **Опубликуйте отзыв:**
   - Ouvrez la porte sur Torii.
   - Vérifiez que les annonces du fournisseur sont bloquées (en utilisant la liste `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Обновите дашbordы:** отметьте провайдера как отозванного, укажите ID спора и приложите ссылку на пакет доказательств.

## 5. Post-mortem et développement ultérieur

- Assurez-vous de prendre soin de vous et de prendre des mesures correctives en cas d'incidents de gouvernance.
- Определите реституцию (réduction de la mise, récupération de la commission, возвраты клиентам).
- Документируйте выводы; Vérifiez les SLA ou les alertes de surveillance en cas de problème.

## 6. Matériel de nettoyage

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (pour les événements)
- `docs/source/sorafs/provider_admission_policy.md` (flux de travail supprimé)
- Nom du tableau de bord : `SoraFS / Capacity Providers`

## Checklist

- [ ] Le paquet contient des documents et des informations.
- [ ] Payload спора валидирован локально.
- [ ] Torii-transmission sporadique.
- [ ] Отзыв выполнен (если одобрен).
- [ ] Дашборды/ранбуки обновлены.
- [ ] Post-mortem оформлен в совете gouvernance.