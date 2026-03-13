---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: دليل تشغيل عمليات العقد
sidebar_label: Nome da barra lateral
description: O valor do `sorafs-node` é igual ao Torii.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/runbooks/sorafs_node_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة توثيق Sphinx القديمة.
:::

## نظرة عامة

Para obter mais informações, consulte `sorafs-node` ou Torii. يتطابق
كل قسم مباشرةً مع مخرجات SF-3: جولات pin/fetch, واستعادة ما بعد إعادة التشغيل,
ورفض الحصص, وأخذ عينات PoR.

## 1. المتطلبات المسبقة

- Verifique o valor do arquivo em `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Verifique se o Torii é usado para substituir o `data_dir`.
- تحقّق من أن العقدة تعلن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` بعد
  Você pode fazer isso.
- عند تمكين التنعيم, تعرض لوحات المتابعة عدادات GiB·hour/PoR الخام والمُنعّمة
  Para obter mais informações, consulte o site.

### تشغيل تجريبي عبر CLI (اختياري)

Faça o download do HTTP sem precisar fazer login na CLI
المرفقة.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

تطبع الأوامر ملخصات Norito JSON وترفض عدم تطابق ملفات تعريف المقاطع أو Digest,
Você pode usar o CI no Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين إثبات PoR

يمكن للمشغلين الآن إعادة تشغيل آرتيفاكتات PoR الصادرة عن الحوكمة محليًا قبل
Resolva Torii. A CLI está disponível para download em `sorafs-node`, para que você possa fazer isso.
O código de acesso não pode ser acessado por meio de HTTP e HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يُصدر الأمر ملخص JSON (digest المانيفست, معرّف المزوّد, digest الإثبات, عدد
العينات, ونتيجة الحكم الاختيارية). O valor `--manifest-id=<hex>` é uma solução de segurança
O resumo do resumo do site e `--json-out=<path>` é um resumo completo.
Você pode fazer isso com antecedência. `--verdict` `--verdict` sem fio
حلقة التحدي → الإثبات → الحكم كاملةً دون اتصال قبل استدعاء e HTTP.

Use Torii para configurar o HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم تقديم كلا نقطتي النهاية بواسطة عامل التخزين المضمّن, لذا تبقى اختبارات
【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Buscar

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Definindo a base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Não há JSON como `manifest_b64` e `payload_b64`. تعيد الاستجابة
   O `manifest_id_hex` e o Digest الحمولة.
3. Definições de segurança:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   A base64 é baseada em `data_b64` e está disponível para download.

##3.

1. Verifique se o produto está funcionando corretamente.
2. Verifique o Torii (ou seja, o valor do arquivo).
3. Limpe o local. يجب أن تبقى الحمولة قابلة للاسترجاع e يتطابق digest
   Você pode usar o aplicativo para obter mais informações.
4. Coloque `GET /v2/sorafs/storage/state` no lugar de `bytes_used`
   Você pode fazer isso sem problemas.

## 4. اختبار رفض الحصة1. Use o `torii.sorafs.storage.max_capacity_bytes` para fazer o download
   مانيفست واحد).
2. ثبّت مانيفستًا واحدًا؛ Não, não.
3. Verifique se a máquina está funcionando corretamente. Use o Torii para HTTP `400`
   O problema é o `storage capacity exceeded`.
4. Verifique o funcionamento do aparelho.

## 5. فحص أخذ عينات PoR

1. ثبّت مانيفستًا.
2. اطلب عينة PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se o arquivo `samples` está disponível no `samples`.
   يصحّ مقابل جذر المانيفست المخزّن.

## 6. خطافات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الفحوصات المستهدفة المضافة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Qual é o `pin_fetch_roundtrip`, `pin_survives_restart` e `pin_quota_rejection`
  e`por_sampling_returns_verified_proofs`.
- يجب أن تتابع لوحات المتابعة:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - عدادات نجاح/فشل PoR المعروضة عبر `/v2/sorafs/capacity/state`
  - محاولات نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`

يضمن اتباع هذه التدريبات, أن عامل التخزين المضمّن قادر على إدخال البيانات,
PoR
Verifique se você está fazendo isso.