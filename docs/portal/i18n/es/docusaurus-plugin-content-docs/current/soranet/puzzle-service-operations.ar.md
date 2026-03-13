---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-de-servicio-de-rompecabezas
título: دليل عمليات خدمة الالغاز
sidebar_label: عمليات خدمة الالغاز
descripción: Daemon `soranet-puzzle-service` para la misión Argon2/ML-DSA.
---

:::nota المصدر القياسي
Aquí `docs/source/soranet/puzzle_service_operations.md`. حافظ على النسختين متزامنتين حتى يتم تقاعد الوثائق القديمة.
:::

# دليل عمليات خدمة الالغاز

يقوم demonio `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) باصدار
boletos de admisión المدعومة بـ Argon2 والتي تعكس política الخاصة بالـ relé `pow.puzzle.*`
Hay tokens de admisión ML-DSA disponibles en relés de borde.
يعرض خمس نقاط نهاية HTTP:

- `GET /healthz` - فحص vivacidad.
- `GET /v2/puzzle/config` - يعيد معلمات PoW/puzzle الفعلية المسحوبة
  Relé de código JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - يصدر تذكرة Argon2; cuerpo JSON اختياري
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  يطلب TTL اقصر (يتم ضبطه ضمن نافذة política), y ويربط التذكرة بـ transcripción hash,
  ويعيد تذكرة موقعة من relevo + بصمة التوقيع عندما تكون مفاتيح التوقيع مهيئة.
- `GET /v2/token/config` - عندما `pow.token.enabled = true` يعيد سياسة
  token de admisión (huella digital del emisor, TTL/desviación del reloj, ID de retransmisión,
  ومجموعة revocación المدمجة).
- `POST /v2/token/mint` - يصدر ML-DSA token de admisión مرتبطا بـ resume hash المقدم؛
  cuerpo الطلب يقبل `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.تتم عملية التحقق من التذاكر المنتجة عبر خدمة الاختبار التكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` ، والتي تختبر ايضا
aceleradores الخاصة بالـ relé خلال سيناريوهات DoS الحجمية.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تهيئة اصدار التوكنات

Establece un relé JSON de tipo `pow.token.*` (es decir,
`tools/soranet-relay/deploy/config/relay.entry.json` (كمثال) لتمكين توكنات ML-DSA.
على الاقل قدم المفتاح العام للـ emisor وقائمة revocación اختيارية:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

خدمة الالغاز تعيد استخدام هذه القيم وتقوم باعادة تحميل ملف revocación بصيغة
Norito JSON en tiempo de ejecución. Adaptador CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) Más información
واستعراضها offline, واضافة `token_id_hex` الى ملف revocación, وتدقيق
الاعتمادات قبل دفع actualizaciones الى الانتاج.

مرر المفتاح السري للـ emisor الى خدمة الالغاز عبر banderas CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

يتوفر ايضا `--token-secret-hex` عندما يكون السر مدارا عبر tubería خارجية. يقوم
observador لملف revocación بالحفاظ على `/v2/token/config` محدثا؛ نسق التحديثات مع
امر `soranet-admission-token revoke` لتجنب تاخر حالة revocación.اضبط `pow.signed_ticket_public_key_hex` في JSON الخاص بالـ relé للاعلان عن المفتاح
العام ML-DSA-44 المستخدم للتحقق من PoW tickets الموقعة; يعكس `/v2/puzzle/config`
المفتاح وبصمته BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) para clientes
Verificador de تثبيت. يتم التحقق من التذاكر الموقعة مقابل ID de retransmisión y enlaces de transcripción
وتشارك نفس مخزن revocación؛ تظل entradas PoW الخام بحجم 74 بايت صالحة عند تهيئة
verificador de boleto firmado. مرر secreto الخاص بالموقع عبر `--signed-ticket-secret-hex`
او `--signed-ticket-secret-path` عند تشغيل خدمة الالغاز; يرفض التشغيل pares de claves
Esto se debe a que está conectado a `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` y `"signed": true` (y `"transcript_hash_hex"`) para
billete firmado مشفر بـ Norito الى جانب bytes التذكرة الخام; تضمن الردود
`signed_ticket_b64` y `signed_ticket_fingerprint_hex` permiten reproducir huellas dactilares. يتم
رفض الطلبات مع `signed = true` اذا لم يتم تهيئة secreto الموقع.

## دليل تدوير المفاتيح1. **Se confirma el descriptor.** Se confirma el descriptor de retransmisión de gobernanza.
   paquete de directorio. Conector hexadecimal `handshake.descriptor_commit_hex` interior
   تكوين JSON الخاص بالـ retransmisión المشترك مع خدمة الالغاز.
2. **مراجعة حدود سياسة الالغاز.** تاكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` المحدثة تتماشى مع خطة الاصدار. يجب
   على المشغلين ابقاء تهيئة Argon2 حتمية عبر relés (con 4 MiB ذاكرة،
   1 <= carriles <= 16).
3. **تجهيز اعادة التشغيل.** اعد تحميل وحدة systemd او الحاوية عند اعلان gobernanza
   عن cutover الخاص بالتدوير. الخدمة لا تدعم recarga en caliente؛ اعادة التشغيل مطلوبة
   لالتقاط confirmación del descriptor الجديد.
4. **التحقق.** Haga clic en `POST /v2/puzzle/mint` y en `difficulty` y `expires_at`.
   يطابقان السياسة الجديدة. Remojar (`docs/source/soranet/reports/pow_resilience.md`)
   يلتقط حدود الكمون المتوقعة للمرجعية. عندما تكون التوكنات مفعلة، اجلب
   `/v2/token/config` Revocación de la huella digital del emisor
   يطابقان القيم المتوقعة.

## اجراء التعطيل الطارئ

1. Utilice `pow.puzzle.enabled = false` para conectar el relé. Nombre de `pow.required = true`
   Aquí está el recurso de respaldo de hashcash.
2. اختياريا، نفذ ادخالات `pow.emergency` لرفض descriptores القديمة بينما بوابة Argon2
   fuera de línea.
3. اعد تشغيل كل من relevo وخدمة الالغاز لتطبيق التغيير.
4. راقب `soranet_handshake_pow_difficulty` para obtener hashcash
   Utilice el `/v2/puzzle/config` y el `puzzle = null`.

## المراقبة والتنبيهات- **Latencia SLO:** Seleccione `soranet_handshake_latency_seconds` y P95 entre 300 ms.
  Esto compensa el remojo del acelerador y la protección.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Presión de cuota:** استخدم `soranet_guard_capacity_report.py` مع métricas de relé
  Los tiempos de reutilización son `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alineación del rompecabezas:** يجب ان يطابق `soranet_handshake_pow_difficulty` الصعوبة
  المعادة من `/v2/puzzle/config`. يشير الاختلاف الى تكوين relé قديم او فشل اعادة
  التشغيل.
- **Preparación del token:** نبه اذا انخفض `/v2/token/config` الى `enabled = false`
  Haga clic en el botón `revocation_source` para obtener marcas de tiempo. يجب على
  Para obtener más información, consulte la revocación Norito o la CLI para acceder a ella.
  Aquí está el punto final.
- **Salud del servicio:** افحص `/healthz` وفق cadence liveness المعتادة ونبه اذا
  Utilice `/v2/puzzle/mint` para HTTP 500 (para obtener información sobre Argon2 y
  فشل RNG). تظهر اخطاء token acuñación عبر استجابات HTTP 4xx/5xx على `/v2/token/mint`؛
  تعامل مع الاخفاقات المتكررة كحالة paginación.

## الامتثال وتسجيل التدقيقLos relés están conectados al acelerador `handshake` y están activados por el acelerador y el tiempo de reutilización.
تاكد من ان tubería الامتثال الموصوفة في `docs/source/soranet/relay_audit_pipeline.md`
تستوعب هذه السجلات حتى تبقى تغييرات سياسة الالغاز قابلة للتدقيق. عندما تكون
بوابة الالغاز مفعلة, ارشف عينات التذاكر الصادرة وsnapshot تكوين Norito مع
Implementación de تذكرة للمدققين المستقبليين. يجب تتبع tokens de admisión الصادرة قبل
نوافذ الصيانة عبر قيم `token_id_hex` وادراجها في ملف revocación عند انتهائها او
الغائها.