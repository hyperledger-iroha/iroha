---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: disputa-revocación-runbook
título: SoraFS تنازع اور منسوخی رن بک
sidebar_label: تنازع اور منسوخی رن بک
descripción: SoraFS کپیسٹی تنازعات جمع کرانے، منسوخیوں کی ہم آہنگی، اور ڈیٹا کو ڈٹرمنسٹک طور پر خالی کرنے کے لیے گورننس ورک فلو۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک گورننس آپریٹرز کو SoraFS کپیسٹی تنازعات جمع کرانے، منسوخیوں کی ہم آہنگی، اور ڈیٹا کے ڈٹرمنسٹک انخلا کو یقینی بنانے میں رہنمائی کرتی ہے۔

## 1. واقعے کا جائزہ

- **ٹرگر شرائط:** SLA کی خلاف ورزی (fallo de tiempo de actividad/PoR), déficit de replicación, یا desacuerdo de facturación کی نشاندہی۔
- **ٹیلیمیٹری کی تصدیق:** پرووائیڈر کے لیے `/v1/sorafs/capacity/state` اور `/v1/sorafs/capacity/telemetry` instantáneas حاصل کریں۔
- **اسٹیک ہولڈرز کو مطلع کریں:** Equipo de almacenamiento (operaciones del proveedor), Consejo de gobierno (órgano de decisión), Observabilidad (actualizaciones del panel) ۔

## 2. شواہد کا پیکج تیار کریں

1. خام artefactos جمع کریں (telemetría JSON, registros CLI, notas del auditor) ۔
2. ڈٹرمنسٹک archivo (مثلاً tarball) میں normalizar کریں؛ درج کریں:
   - Resumen BLAKE3-256 (`evidence_digest`)
   - tipo de medio (`application/zip`, `application/jsonl` y otros)
   - URI de alojamiento (almacenamiento de objetos, pin SoraFS, punto final accesible Torii)
3. گورننس depósito de recopilación de pruebas میں escritura única رسائی کے ساتھ پیکج محفوظ کریں۔## 3. تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` Archivo de especificación JSON:

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

3. `dispute_summary.json` ریویو کریں (tipo, resumen de evidencia, marcas de tiempo) ۔
4. گورننس ٹرانزیکشن کیو کے ذریعے Torii `/v1/sorafs/capacity/dispute` کو ریکوئسٹ JSON بھیجیں۔ جواب کی قدر `dispute_id_hex` محفوظ کریں؛ یہی بعد کی منسوخی کارروائیوں اور آڈٹ رپورٹس کا اینکر ہے۔

## 4. انخلا اور منسوخی

1. **Ventana de gracia:** پروائیڈر کو متوقع منسوخی سے آگاہ کریں؛ پالیسی اجازت دے تو datos anclados کے انخلا کی اجازت دیں۔
2. **`ProviderAdmissionRevocationV1` Número:**
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخط اور resumen de revocación ویریفائی کریں۔
3. **منسوخی شائع کریں:**
   - منسوخی ریکوئسٹ Torii کو جمع کریں۔
   - یقینی بنائیں کہ پرووائیڈر anuncios بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے)۔
4. **Paneles de control:** پرووائیڈر کو revocado کے طور پر فلیگ کریں، ID de disputa کا حوالہ دیں، اور paquete de pruebas لنک کریں۔

## 5. Post-mortem اور فالو اپ

- Búsqueda de causa raíz, reparación y seguimiento de incidentes.
- restitución طے کریں (reducción de la participación, recuperación de tarifas, reembolsos a los clientes) ۔
- سیکھے گئے اسباق دستاویز کریں؛ ضرورت ہو تو Umbrales SLA یا alertas de monitoreo اپڈیٹ کریں۔

## 6. حوالہ جاتی مواد

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (sección de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (flujo de trabajo de revocación)
- Panel de observabilidad: `SoraFS / Capacity Providers`

## چیک لسٹ- [] paquete de evidencia حاصل کر کے hash کر لیا گیا۔
- [] disputar carga útil مقامی طور پر validar کیا گیا۔
- [] Disputa Torii ٹرانزیکشن قبول ہوئی۔
- [] منسوخی نافذ کی گئی (اگر منظور ہو)۔
- [] paneles/runbooks اپڈیٹ ہوئے۔
- [] Post-mortem گورننس کونسل میں جمع کرایا گیا۔