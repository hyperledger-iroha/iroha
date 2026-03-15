---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de serviço de quebra-cabeça
título: دليل عمليات خدمة الالغاز
sidebar_label: عمليات خدمة الالغاز
description: Instale o daemon `soranet-puzzle-service` para instalar o Mission Argon2/ML-DSA.
---

:::note المصدر القياسي
É `docs/source/soranet/puzzle_service_operations.md`. Verifique se o produto está funcionando corretamente.
:::

# دليل عمليات خدمة الالغاز

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) está instalado
ingressos de admissão المدعومة بـ Argon2 والتي تعكس política الخاصة بالـ relé `pow.puzzle.*`
Os tokens de admissão ML-DSA são usados ​​em retransmissões de borda.
يعرض خمس نقاط نهاية HTTP:

- `GET /healthz` - فحص vivacidade.
- `GET /v1/puzzle/config` - يعيد معلمات PoW/puzzle الفعلية المسحوبة
  É um relé JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - يصدر تذكرة Argon2; corpo JSON
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  يطلب TTL اقصر (يتم ضبطه ضمن نافذة política), ويربط التذكرة بـ transcrição hash,
  ويعيد تذكرة موقعة من relé + بصمة التوقيع عندما تكون مفاتيح التوقيع مهيئة.
- `GET /v1/token/config` - `pow.token.enabled = true`
  token de admissão (impressão digital do emissor, TTL/inclinação do relógio, ID de retransmissão,
  ومجموعة revogação المدمجة).
- `POST /v1/token/mint` - Token de admissão ML-DSA مرتبطا بـ resume hash المقدم؛
  corpo é `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

تتم عملية التحقق من التذاكر المنتجة عبر خدمة الاختبار التكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`;
aceleradores الخاصة بالـ relé خلال سيناريوهات DoS الحجمية.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## تهيئة اصدار التوكنات

Defina o JSON como relé do relé `pow.token.*` (
`tools/soranet-relay/deploy/config/relay.entry.json` é compatível com ML-DSA.
على الاقل قدم المفتاح العام للـ emissor e revogação اختيارية:

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

خدمة الالغاز تعيد استخدام هذه القيم وتقوم باعادة تحميل ملف revogação بصيغة
Norito JSON no tempo de execução. CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) Nome de usuário
واستعراضها offline, واضافة `token_id_hex` revogação de revogação, وتدقيق
As atualizações serão atualizadas.

مرر المفتاح السري للـ issuer الى خدمة الالغاز عبر flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

Use o `--token-secret-hex` para que o pipeline esteja conectado. يقوم
observador لملف revogação بالحفاظ على `/v1/token/config` محدثا؛ نسق التحديثات مع
امر `soranet-admission-token revoke` لتجنب تاخر حالة revogação.Use `pow.signed_ticket_public_key_hex` em JSON para retransmitir para o local de trabalho
العام ML-DSA-44 المستخدم للتحقق من PoW tickets الموقعة; `/v1/puzzle/config`
A solução BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) para clientes
Meu verificador. يتم التحقق من التذاكر الموقعة مقابل ID de retransmissão e ligações de transcrição
وتشارك نفس مخزن revogação؛ Obter ingressos PoW em 74 dias por semana
verificador de bilhetes assinados. مرر secret الخاص بالموقع عبر `--signed-ticket-secret-hex`
E `--signed-ticket-secret-path` é um arquivo de texto; يرفض التشغيل pares de chaves
Verifique se o produto está em `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` é `"signed": true` (e `"transcript_hash_hex"`) mais alto
ticket assinado مشفر بـ Norito الى جانب bytes التذكرة الخام; تتضمن الردود
`signed_ticket_b64` e `signed_ticket_fingerprint_hex` permitem reproduzir impressões digitais. sim
رفض الطلبات مع `signed = true` اذا لم يتم تهيئة secret الموقع.

## دليل تدوير المفاتيح

1. **جمع descriptor commit الجديد.** É o commit do descritor de retransmissão de governança aqui
   pacote de diretório. Use hex para `handshake.descriptor_commit_hex`
   O JSON é usado para retransmitir o problema.
2. **مراجعة حدود سياسة الالغاز.** تاكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` é um dispositivo que pode ser danificado. sim
   O Argon2 está usando relés (ou seja, 4 MiB de largura,
   1 <= pistas <= 16).
3. **تجهيز اعادة التشغيل.** اعد تحميل وحدة systemd او الحاوية عند اعلان governança
   Não há cortes. Como usar hot-reload; اعادة التشغيل مطلوبة
   O descritor commit é o seguinte.
4. **التحقق.** Você pode usar o `POST /v1/puzzle/mint` e o `difficulty` e `expires_at`
   يطابقان السياسة الجديدة. Mergulhe em água (`docs/source/soranet/reports/pow_resilience.md`)
   يلتقط حدود الكمون المتوقعة للمرجعية. عندما تكون التوكنات مفعلة, اجلب
   `/v1/token/config` para a impressão digital do emissor e para a revogação
   يطابقان القيم المتوقعة.

## اجراء التعطيل الطارئ

1. Use `pow.puzzle.enabled = false` no relé de segurança. `pow.required = true`
   Não há nenhum recurso de hashcash substituto.
2. اختياريا, نفذ ادخالات `pow.emergency` لرفض descritores القديمة بينما بوابة Argon2
   off-line.
3. Verifique se o relé está funcionando corretamente.
4. Use `soranet_handshake_pow_difficulty` para obter hashcash
   O nome do produto é `/v1/puzzle/config` ou `puzzle = null`.

## المراقبة والتنبيهات- **SLO de latência:** `soranet_handshake_latency_seconds` e P95 tem duração de 300 ms.
  توفر compensações الخاصة باختبار absorver بيانات معايرة لthrottles الـ guarda.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Pressão de cota:** استخدم `soranet_guard_capacity_report.py` com métricas de relé
  Resfriamentos para `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alinhamento do quebra-cabeça:** يجب ان يطابق `soranet_handshake_pow_difficulty` الصعوبة
  O nome é `/v1/puzzle/config`. يشير الاختلاف الى تكوين relé é e não é
  Não.
- **Prontidão do token:** نبه اذا انخفض `/v1/token/config` ou `enabled = false`
  Você pode usar o `revocation_source` com carimbos de data/hora. يجب على
  A revogação da versão Norito da CLI é feita por meio de uma chamada de revogação Norito
  Este é o endpoint.
- **Saúde do serviço:** افحص `/healthz` e cadência vivacidade المعتادة ونبه اذا
  Use `/v1/puzzle/mint` para HTTP 500 (não use o Argon2 ou
  no RNG). A criação de token minting é feita através de HTTP 4xx/5xx em `/v1/token/mint`;
  تعامل مع الاخفاقات المتكررة كحالة paginação.

## الامتثال وتسجيل التدقيق

Os relés são usados ​​para `handshake` para reduzir o acelerador e o resfriamento.
Obtenha um pipeline de pipeline em `docs/source/soranet/relay_audit_pipeline.md`
Certifique-se de que o produto esteja funcionando corretamente. عندما تكون
بوابة الالغاز مفعلة, ارشف عينات التذاكر الصادرة e instantâneo تكوين Norito مع
Implementar rollout للمدققين المستقبلين. يجب تتبع tokens de admissão
نوافذ الصيانة عبر قيم `token_id_hex` وادراجها في ملف revogação عند انتهائها او
الغائها.