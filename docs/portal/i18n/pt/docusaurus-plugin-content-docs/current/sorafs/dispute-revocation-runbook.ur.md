---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: SoraFS تنازع اور منسوخی رن بک
sidebar_label: تنازع اور منسوخی رن بک
description: SoraFS کپیسٹی تنازعات جمع کرانے, منسوخیوں کی ہم آہنگی, اور ڈیٹا کو ڈٹرمنسٹک طور پر خالی کرنے کے لیے گورننس ورک فلو۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانی Esfinge ڈاکیومنٹیشن ریٹائر نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک گورننس آپریٹرز کو SoraFS کپیسٹی تنازعات جمع کرانے, منسوخیوں کی ہم آہنگی, اور ڈیٹا کے ڈٹرمنسٹک انخلا کو یقینی بنانے میں رہنمائی کرتی ہے۔

## 1. واقعے کا جائزہ

- **ٹرگر شرائط:** SLA کی خلاف ورزی (tempo de atividade/falha PoR), déficit de replicação, یا discordância de faturamento کی نشاندہی۔
- **ٹیلیمیٹری کی تصدیق:** پرووائیڈر کے لیے `/v2/sorafs/capacity/state` e `/v2/sorafs/capacity/telemetry` snapshots حاصل کریں۔
- **اسٹیک ہولڈرز کو مطلع کریں:** Equipe de armazenamento (operações do provedor), Conselho de Governança (órgão de decisão), Observabilidade (atualizações do painel)۔

## 2. شواہد کا پیکج تیار کریں

1. Seus artefatos جمع کریں (JSON de telemetria, logs CLI, notas do auditor)۔
2. Arquivo ڈٹرمنسٹک (مثلاً tarball) para normalizar کریں؛ O que fazer:
   - Resumo BLAKE3-256 (`evidence_digest`)
   - tipo de mídia (`application/zip`, `application/jsonl` e وغیرہ)
   - URI de hospedagem (armazenamento de objetos, pino SoraFS e endpoint acessível Torii)
3. Balde de coleta de evidências گورننس میں gravação única رسائی کے ساتھ پیکج محفوظ کریں۔

## 3. تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` کے لیے JSON spec بنائیں:

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

2. CLI چلائیں:

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

3. `dispute_summary.json` ریویو کریں (tipo, resumo de evidências, carimbos de data e hora)۔
4. گورننس ٹرانزیکشن کیو کے ذریعے Torii `/v2/sorafs/capacity/dispute` کو ریکوئسٹ JSON بھیجیں۔ جواب کی قدر `dispute_id_hex` محفوظ کریں؛ یہی بعد کی منسوخی کارروائیوں اور آڈٹ رپورٹس کا اینکر ہے۔

## 4. انخلا اور منسوخی

1. **Janela de graça:** پرووائیڈر کو متوقع منسوخی سے آگاہ کریں؛ پالیسی اجازت دے تو dados fixados کے انخلا کی اجازت دیں۔
2. **`ProviderAdmissionRevocationV1` Nota:**
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخط اور resumo de revogação ویریفائی کریں۔
3. **منسوخی شائع کریں:**
   - منسوخی ریکوئسٹ Torii کو جمع کریں۔
   - یقینی بنائیں کہ پرووائیڈر anúncios بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے)۔
4. **Painéis اپڈیٹ کریں:** پرووائیڈر کو revogado کے طور پر فلیگ کریں, ID de disputa کا حوالہ دیں, اور pacote de evidências لنک کریں۔

## 5. Post-mortem de فالو اپ

- ٹائم لائن, causa raiz, remediação اقدامات گورننس rastreador de incidentes میں ریکارڈ کریں۔
- restituição طے کریں (corte de participação, recuperação de taxas, reembolso de clientes)۔
- سیکھے گئے اسباق دستاویز کریں؛ Limites de SLA e alertas de monitoramento

## 6. حوالہ جاتی مواد

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (seção de disputa)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de trabalho de revogação)
- Painel de observabilidade: `SoraFS / Capacity Providers`

## چیک لسٹ- [] pacote de evidências حاصل کر کے hash کر لیا گیا۔
- [] disputa de carga útil مقامی طور پر validar کیا گیا۔
- [] Torii disputa ٹرانزیکشن قبول ہوئی۔
- [ ] منسوخی نافذ کی گئی (اگر منظور ہو)۔
- [ ] painéis/runbooks اپڈیٹ ہوئے۔
- [] Post-mortem گورننس کونسل میں جمع کرایا گیا۔