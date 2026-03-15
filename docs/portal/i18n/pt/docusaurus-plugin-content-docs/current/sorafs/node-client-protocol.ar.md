---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول عقدة ↔ عميل SoraFS

يلخص هذا الدليل التعريف المعتمد للبروتوكول في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use o upstream para Norito para obter informações e dados
Você pode usar runbooks para executar o SoraFS.

## إعلانات المزوّد والتحقق

O modelo SoraFS é compatível com `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`).
تثبت الإعلانات بيانات الاكتشاف والضوابط التي يفرضها المُنسِّق متعدد المصادر
أثناء التشغيل.

- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ينبغي على
  A data é de 12 dias.
- **TLV القدرات** — تسرد قائمة TLV ميزات النقل (Torii, QUIC+Noise, ترحيلات
  SoraNet, امتدادات مزوّد). يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` é uma graxa com graxa.
- **تلميحات QoS** — طبقة `availability` (Quente/Quente/Frio), أقصى كمون للاسترجاع,
  حد التزامن, وميزانية stream اختيارية. Qual é a qualidade do QoS no sistema
  Não há nada que você possa fazer aqui.
- **Endpoints e rendezvous** — عناوين خدمة محددة مع بيانات TLS/ALPN إضافة
  Para que você possa obter mais informações sobre o assunto, não há problema em usá-lo.
- **سياسة تنوع المسارات** — `min_guard_weight` e fan-out do AS/pool e
  `provider_failure_threshold` é compatível com multi-peer.
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل
  `sorafs.sf1@1.0.0`)؛ Use `profile_aliases` para obter mais informações
  الترحيل.

ترفض قواعد التحقق stake الصفري, أو قوائم القدرات/العناوين/المواضيع الفارغة,
Não há transferência de QoS ou QoS. تقارن أظرف القبول بين محتوى الإعلان
O item (`compare_core_fields`) é um problema.

### امتدادات الجلب بالنطاقات

تتضمن المزوّدات الداعمة للنطاق البيانات التالية:

| الحقل | الغرض |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | Você pode usar `max_chunk_span` e `min_granularity`. |
| `StreamBudgetV1` | Verifique se há algum problema/referência (`max_in_flight`, `max_bytes_per_sec`, e `burst`). يتطلب قدرة نطاق. |
| `TransportHintV1` | Não use nenhum produto (como `torii_http_range`, `quic_stream`, `soranet_relay`). O código de barras é `0–15`. |

Veja mais:

- يجب على خطوط إعلانات المزوّدين التحقق من قدرة النطاق وميزانية stream وتلميحات
  Certifique-se de que o produto esteja funcionando corretamente.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  luminárias para `fixtures/sorafs_manifest/provider_admission/`.
- Verifique o valor do código `stream_budget` ou `transport_hints`
  بواسطة محمّلات CLI/SDK قبل الجدولة, ما يحافظ على توافق مسار multi-source مع
  É o Torii.

## نقاط نهاية نطاقات البوابة

Verifique se o HTTP não está funcionando corretamente.

###`GET /v1/sorafs/storage/car/{manifest_id}`| المتطلب | التفاصيل |
|--------|----------|
| **Cabeçalhos** | `Range` (`dag-scope: block`), `X-SoraFS-Chunker`, `X-SoraFS-Nonce` Base64 e `X-SoraFS-Stream-Token` base64. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, و`Content-Range` يصف النافذة المقدمة, وبيانات `X-Sora-Chunk-Range`, وإعادة إرسال O chunker/token. |
| **Modos de falha** | `416` para um dispositivo de armazenamento, `401` para um dispositivo de armazenamento/referência, `429` تجاوز ميزانيات stream/byte. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب شريحة واحدة بنفس الرؤوس بالإضافة إلى digest الحتمي للشريحة. مفيد لإعادة
O carro ou o carro estão funcionando corretamente.

## سير عمل المُنسِّق متعدد المصادر

Você pode usar o SF-6 para usar (CLI Rust عبر `sorafs_fetch`, وSDKs عبر
`sorafs_orchestrator`):

1. **جمع المدخلات** — فك خطة شرائح المانيفست, وجلب أحدث الإعلانات, وتمرير
   Verifique o valor do produto (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Placar de pontuação** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل أسباب الرفض؛ É `sorafs_fetch --scoreboard-out` como JSON.
3. **جدولة الشرائح** — يفرض `fetch_with_scoreboard` (ou `--plan`) قيود النطاق,
   ميزانيات stream, وحدود إعادة المحاولة/الأقران (`--retry-budget`, `--max-peers`)
   O token de fluxo é definido como um token de fluxo.
4. **التحقق من الإيصالات** — تشمل المخرجات `chunk_receipts` e `provider_reports`;
   A CLI está usando `provider_reports` e `chunk_receipts` e `ineligible_providers`
   لحزم الأدلة.

أخطاء شائعة تصل إلى المشغلين/SDKs:

| الخطأ | الوصف |
|-------|-------|
| `no providers were supplied` | Não há necessidade de fazer isso. |
| `no compatible providers available for chunk {index}` | عدم توافق نطاق أو ميزانية لشريحة محددة. |
| `retry budget exhausted after {attempts}` | Você pode usar `--retry-budget` ou não. |
| `no healthy providers remaining` | Não se preocupe, você pode fazer isso sem problemas. |
| `streaming observer failed` | Coloque o carro no lugar certo. |
| `orchestrator invariant violated` | O placar, o placar, o painel de controle e o JSON estão na CLI. |

## التليمترية والأدلة

- المقاييس الصادرة عن المُنسِّق:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (o nome é manifesto/região/provedor). Use `telemetry_region` em qualquer lugar
  A CLI não permite que você use o software.
- تتضمن ملخصات الجلب في CLI/SDK ملف scoreboard JSON المحفوظ وإيصالات الشرائح
  Verifique se o SF-6/SF-7 está funcionando corretamente.
- تكشف معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  Você pode usar o SRE para obter mais informações.

## مساعدات CLI e REST

- `iroha app sorafs pin list|show` e `alias list` e `replication list` para REST
  Os pinos Norito JSON são usados para atestação.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` manifestos
  بنمط Norito e JSON com provas اختيارية للـ alias e sucessor; Provas
  `400`, e provas de `503`, `Warning: 110`, بينما تعيد
  provas `412`.
- RESTO (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  تتضمن هياكل atestado حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  رؤوس الكتل قبل التنفيذ.## المراجع

- المواصفة المعتمدة:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Nome Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI de referência: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Nome do arquivo: `crates/sorafs_orchestrator`
- Nome de usuário: `dashboards/grafana/sorafs_fetch_observability.json`