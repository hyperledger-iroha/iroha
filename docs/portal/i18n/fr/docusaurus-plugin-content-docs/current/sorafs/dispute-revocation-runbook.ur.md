---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : litige-révocation-runbook
titre : SoraFS تنازع اور منسوخی رن بک
sidebar_label : تنازع اور منسوخی رن بک
description: SoraFS Pièce de rechange pour appareil photo et appareil photo ڈٹرمنسٹک طور پر خالی کرنے کے لیے گورننس ورک فلو۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ Il s'agit d'un Sphinx en vente libre.
:::

## مقصد

Il s'agit d'un produit SoraFS qui est en mesure de fournir une solution à votre problème. آہنگی، اور ڈیٹا کے ڈٹرمنسٹک انخلا کو یقینی بنانے میں رہنمائی کرتی ہے۔

## 1. واقعے کا جائزہ

- **Prise en charge :** SLA en cours de fonctionnement (échec de disponibilité/PoR), déficit de réplication, ou désaccord de facturation en cours
- **ٹیلیمیٹری کی تصدیق:** پرووائیڈر کے لیے `/v1/sorafs/capacity/state` et `/v1/sorafs/capacity/telemetry` snapshots حاصل کریں۔
- **اسٹیک ہولڈرز کو مطلع کریں :** Équipe de stockage (opérations du fournisseur) ، Conseil de gouvernance (organe de décision) ، Observabilité (mises à jour du tableau de bord) ۔

## 2. شواہد کا پیکج تیار کریں

1. Voici les artefacts (télémétrie JSON, journaux CLI, notes de l'auditeur)
2. Archiver les archives (archive tar) pour normaliser les fichiers درج کریں:
   - Résumé BLAKE3-256 (`evidence_digest`)
   - type de média (`application/zip`, `application/jsonl` et)
   - URI d'hébergement (stockage d'objets, broche SoraFS, point de terminaison accessible Torii)
3. Le seau de collecte de preuves est un seau de collecte de preuves à écriture unique et un seau de collecte de preuves en écriture unique.## 3. تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` pour les spécifications JSON :

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

2. CLI چلائیں :

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

3. `dispute_summary.json` ریویو کریں (type de résumé de preuves, horodatages)۔
4. L'application Torii `/v1/sorafs/capacity/dispute` est basée sur JSON. جواب کی قدر `dispute_id_hex` محفوظ کریں؛ یہی بعد کی منسوخی کارروائیوں اور آڈٹ رپورٹس کا اینکر ہے۔

## 4. انخلا اور منسوخی

1. **Fenêtre de grâce :** پرووائیڈر کو متوقع منسوخی سے آگاہ کریں؛ Les données épinglées sont associées à des données épinglées.
2. **`ProviderAdmissionRevocationV1` Article :**
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخط اور résumé de révocation ویریفائی کریں۔
3. **منسوخی شائع کریں:**
   - Le lecteur Torii est en ligne
   - یقینی بنائیں کہ پرووائیڈر adverts بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے)۔
4. **Tableaux de bord Tableau de bord :** L'ID de litige est révoqué et l'ID du litige est également disponible pour un ensemble de preuves. کریں۔

## 5. Post-mortem اور فالو اپ

- Recherche de la cause première et de la remédiation ainsi que du suivi des incidents et du suivi des incidents.
- restitution (réduction des mises, récupération des frais, remboursement des clients)
- سیکھے گئے اسباق دستاویز کریں؛ ضرورت ہو تو Seuils SLA et alertes de surveillance

## 6. حوالہ جاتی مواد

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (section litige)
- `docs/source/sorafs/provider_admission_policy.md` (workflow de révocation)
- Tableau de bord d'observabilité : `SoraFS / Capacity Providers`

## چیک لسٹ- [ ] paquet de preuves حاصل کر کے hash کر لیا گیا۔
- [ ] contester la charge utile مقامی طور پر valider کیا گیا۔
-[ ] Torii litige ٹرانزیکشن قبول ہوئی۔
- [ ] منسوخی نافذ کی گئی (اگر منظور ہو)۔
- [ ] tableaux de bord/runbooks اپڈیٹ ہوئے۔
- [ ] Post-mortem گورننس کونسل میں جمع کرایا گیا۔