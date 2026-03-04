---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: اعداد مُنسِّق SoraFS
sidebar_label: Nome da barra lateral
description: اضبط مُنسِّق الجلب متعدد المصادر, وفسّر الاخفاقات, وتتبع مخرجات التليمترية.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/developer/orchestrator.md`. Certifique-se de que o produto esteja funcionando corretamente.
:::

# دليل مُنسِّق الجلب متعدد المصادر

Você pode usar o software SoraFS para obter mais informações e obter mais informações
المزوّدين المنشورة em anúncios المدعومة بالحوكمة. يشرح هذا الدليل كيفية ضبط
المُنسِّق, وما هي إشارات الفشل المتوقعة أثناء عمليات الإطلاق, وأي تدفقات
Você pode fazer isso com mais frequência.

## 1. نظرة عامة على الاعداد

يدمج المُنسِّق ثلاثة مصادر للاعداد:

| المصدر | الغرض | الملاحظات |
|--------|-------|-----------|
| `OrchestratorConfig.scoreboard` | يطبّع أوزان المزوّدين, ويتحقق من حداثة التليمترية, ويحفظ لوحة النتائج JSON المستخدمة Não. | O modelo é `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | يطبّق حدود وقت التشغيل (ميزانيات إعادة المحاولة, حدود التوازي, مفاتيح التحقق). | O `FetchOptions` é o `crates/sorafs_car::multi_fetch`. |
| Usando CLI / SDK | Você pode negar/aumentar e negar/aumentar. | `sorafs_cli fetch` يعرّض هذه الأعلام مباشرةً؛ Instale `OrchestratorConfig` em SDKs. |

Use JSON em `crates/sorafs_orchestrator::bindings` para definir o valor do arquivo
Para Norito JSON, você pode usar o SDK e o SDK.

### 1.1 para JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

احفظ الملف عبر طبقات `iroha_config` Nome de usuário (`defaults/`, usuário, real)
Verifique se há algum problema com isso. لملف fallback مباشر فقط
Implementação do rollout do SNNet-5a, راجع
`docs/examples/sorafs_direct_mode_policy.json` e o código de barras do `docs/examples/sorafs_direct_mode_policy.json`
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 تجاوزات الامتثال

Use o SNNet-9 para obter mais informações. كائن `compliance` Preto
No caso de Norito JSON, o arquivo é somente direto:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` يعلن رموز ISO‑3166 alpha‑2 التي تعمل فيها هذه
  النسخة من المُنسِّق. Verifique se o produto está funcionando corretamente.
- `jurisdiction_opt_outs` não funciona. عندما تظهر أي ولاية تشغيلية ضمن
  Descrição do produto `transport_policy=direct-only`
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` يسرد digests المانيفست (cegos CIDs بصيغة hex
  بأحرف كبيرة). التطابقات تفرض أيضاً الجدولة somente direto e وتُظهر
  `compliance_blinded_cid_opt_out` está disponível.
- `audit_contacts` يسجل عناوين URI التي تتوقع الحوكمة أن ينشرها المشغلون في
  playbooks الخاصة بـ GAR.
- `attestations` يلتقط حزم الامتثال الموقّعة الداعمة للسياسة. كل إدخال يعرّف
  `jurisdiction` ISO (ISO-3166 alpha-2), و`document_uri`, و`digest_hex`
  64 páginas, e `issued_at_ms`, e `expires_at_ms`
  Obrigado. تتدفق هذه الآثار إلى قائمة تدقيق المُنسِّق كي تربط أدوات الحوكمة
  Não há nada que você possa fazer.

مرّر كتلة الامتثال عبر طبقات الاعداد المعتادة كي يحصل المشغلون على تجاوزات
Bem. Use o modo de gravação: no SDK do SDK
`upload-pq-only`, é um recurso de configuração direta e somente direto
وتفشل سريعاً عندما لا يوجد مزوّدون متوافقون.توجد كتالوجات opt-out المعتمدة في
`governance/compliance/soranet_opt_outs.json`; وينشر مجلس الحوكمة التحديثات عبر
إصدارات موسومة. يتوفر مثال كامل للاعداد (بما في ذلك atestados) aqui
`docs/examples/sorafs_compliance_policy.json`;
[manual de instruções GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 CLI e SDK

| العلم / الحقل | الاثر |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Não há nenhum problema com o dinheiro. Verifique se `None` está funcionando corretamente. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Não faça isso. Verifique o `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Não há latência/falha no sistema operacional. O código `telemetry_grace_secs` é usado para remover o problema. |
| `--scoreboard-out` | يحفظ لوحة النتائج المحسوبة (مزوّدون مؤهلون + غير مؤهلين) للفحص بعد التشغيل. |
| `--scoreboard-now` | Use o sistema de jogos (Unix) para definir os fixtures حتمية. |
| `--deny-provider` / pontuação de gancho سياسة | يستبعد المزوّدين بشكل حتمي دون حذف anúncios. مفيد للمنع السريع. |
| `--boost-provider=name:delta` | يضبط أرصدة round-robin الموزونة لمزوّد مع الإبقاء على أوزان الحوكمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | يوسم المقاييس والسجلات المهيكلة لتمكين لوحات المتابعة من التجميع حسب الجغرافيا, e موجة الإطلاق. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | O código `soranet-first` não pode ser usado para evitar problemas. O `direct-only` é um dispositivo de teste e um `soranet-strict` somente PQ; Limpe a máquina de lavar roupa. |

SoraNet-first é um site de rede social e de rede SNNet
المعني. بعد تخرّج SNNet-4/5/5a/5b/6a/7/8/12/13 ستشدّد الحوكمة الوضع المطلوب
(`soranet-strict`), o que significa que você pode usar o `direct-only`
A máquina de lavar roupa pode ser usada em qualquer lugar.

Você pode usar o `--` para o `sorafs_cli fetch` e
`sorafs_fetch`. Os SDKs não são suportados pelos construtores.

### 1.4 ادارة ذاكرة cache للحراس

O CLI não permite que o SoraNet seja usado para retransmitir relés.
É necessário implementar o lançamento do SNNet-5. ثلاثة أعلام جديدة تتحكم
Mais:

| العلم | الغرض |
|------|-------|
| `--guard-directory <PATH>` | O JSON não pode ser usado para retransmitir (ou não). تمرير الدليل يحدّث cache الحراس قبل الجلب. |
| `--guard-cache <PATH>` | Use `GuardSet` como padrão. Você pode usar o cache para obter mais informações. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | تجاوزات اختيارية لعدد حراس الدخول المثبتين (3) e 30 dias). |
| `--guard-cache-key <HEX>` | مفتاح اختياري بطول 32 بايت لوضع MAC Blake3 على cache الحراس حتى يمكن التحقق منها قبل إعادة الاستخدام. |

تستخدم حمولة دليل الحراس مخططاً مدمجاً:

Use o `--guard-directory` para substituir o `GuardDirectorySnapshotV2`.
تحتوي اللقطة الثنائية على:- `version` — Nome de usuário (`2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  Você pode fazer isso sem problemas.
- `validation_phase` — بوابة سياسة الشهادات (`1` = número de telefone Ed25519 واحد,
  `2` = تفضيل توقيعات مزدوجة, `3` = اشتراط التوقيعات المزدوجة).
- `issuers` — `fingerprint`, `ed25519_public`,
  `mldsa65_public`. تُحسب البصمات كالتالي:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — é um arquivo SRCv2 (nome `RelayCertificateBundleV2::to_cbor()`). تحمل
  Para relés e relés e relés e ML-KEM e Ed25519/ML-DSA-65
  المزدوجة.

A CLI pode ser usada para configurar o cache do cache
الحراس. Para obter mais informações sobre JSON Resolva o SRCv2.

Use CLI com `--guard-directory` para armazenar o cache do servidor.
يحافظ المحدد على الحراس المثبتين ضمن نافذة الاحتفاظ والمؤهلين في الدليل؛
وتستبدل relés sem fio. بعد نجاح الجلب تُكتب cache
O código `--guard-cache` está localizado no local onde está o problema.
O SDK não está disponível para download
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
`GuardSet` é igual a `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` é um recurso de implementação de implementação de PQ
SNNet-5. تعمل مفاتيح المراحل (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`).
PQ يقوم المحدد بإسقاط الدبابيس الكلاسيكية الزائدة لتفضيل المصافحات الهجينة.
A instalação do CLI/SDK é baseada em `anonymity_status`/`anonymity_reason`,
`anonymity_effective_policy`, `anonymity_pq_selected`, `anonymity_classical_selected`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` e código de barras/caixa/caixa
Isso pode causar quedas de energia e fallbacks e quedas.

A solução SRCv2 é `certificate_base64`. sim
O código de barras do modelo Ed25519/ML-DSA está disponível no site Ed25519/ML-DSA.
المحللة بجانب cache الحراس. Você pode fazer isso com PQ
وتفضيلات المصافحة والترجيح؛ وتُهمل الشهادات المنتهية ويعود المحدد إلى حقول
الوصف القديمة. تنتشر الشهادات عبر إدارة دورة حياة الدوائر وتُعرض عبر
`telemetry::sorafs.guard` e `telemetry::sorafs.circuit` são usados para obter mais informações
وحزم المصافحة وما إذا لوحظت توقيعات مزدوجة لكل حارس.

Use o CLI para configurar a interface do CLI:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` `fetch` O SRCv2 é um recurso que pode ser usado para evitar problemas
`verify` permite que você crie um arquivo JSON com o mesmo valor
É possível usar o CLI/SDK.

### 1.5 مدير دورة حياة الدوائر

عندما يتوفر دليل relés e cache الحراس معاً;
Você pode usar o SoraNet para obter mais informações. يقع الاعداد ضمن
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) é um problema:

- `relay_directory`: يحمل لقطة دليل SNNet-3 كي يتم اختيار middle/exit hops
  Sim.
- `circuit_manager`: O código de segurança (não especificado) é transferido para o TTL.

Norito JSON é igual a `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Baixar SDKs sem problemas
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) , e a CLI está disponível para download
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).O ponto de extremidade ou o ponto de extremidade PQ ou o ponto de extremidade
(não disponível) ou TTL. Código de erro `refresh_circuits`
Cabo de segurança (`crates/sorafs_orchestrator/src/lib.rs:1346`)
`CircuitEvent` é um dispositivo de teste que pode ser usado para evitar problemas. Mergulhe
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`).
دورات تبديل للحراس؛ راجع التقرير في
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 e QUIC محلي

يمكن للمُنسِّق اختيارياً تشغيل e QUIC محلي بحيث لا تضطر إضافات المتصفح
Além disso, o SDK não armazena o cache nem o cache. يرتبط الوكيل بعنوان
loopback, وينهي اتصالات QUIC, ويعيد manifesto de Norito يصف الشهادة ومفتاح
cache الاختياري إلى العميل. تُعد أحداث النقل التي يصدرها الوكيل ضمن
`sorafs_orchestrator_transport_events_total`.

O valor do arquivo `local_proxy` é definido como JSON:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` é um dispositivo de armazenamento de dados (`0` é usado para substituir o `0`).
- `telemetry_label` não é compatível com o software.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض cache الحراس بالمفتاح ذاته
  Ao usar CLI/SDKs, você pode usar o recurso de gerenciamento de arquivos.
- `emit_browser_manifest` é um arquivo do manifesto do `emit_browser_manifest`
  للامتدادات حفظه والتحقق منه.
- `proxy_mode` é um dispositivo de armazenamento de dados (`bridge`) ou outro.
  O software de gerenciamento de software (`metadata-only`) está disponível para SDKs do SoraNet.
  Nome `bridge`; Use `metadata-only` para exibir o manifesto.
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs`
  Certifique-se de que o produto esteja funcionando corretamente e que você não tenha problemas com isso.
- `car_bridge` (اختياري) يشير إلى cache محلي لأرشيفات CAR. يتحكم حقل `extension`
  O código de barras é `*.car`; Placa `allow_zst = true`
  `*.car.zst` Código de erro.
- `kaigi_bridge` (اختياري) يعرّض مسارات Kaigi للوكيل. `room_policy` mais barato
  Você pode usar `public` ou `authenticated` para obter mais informações sobre GAR
  Não há problema.
- `sorafs_cli fetch` ou `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH` bobinas e bobinas de aço
  Ele é definido como JSON.
- `downgrade_remediation` (`downgrade_remediation`). عند التفعيل
  يراقب المُنسِّق تليمترية relé لرصد اندفاعات downgrades, وبعد تجاوز
  `threshold` é um `window_secs` que pode ser usado para `target_mode`
  (`metadata-only`). Como fazer downgrades no `resume_mode`
  É `cooldown_secs`. Use o relé `modes` para substituir o relé do carro
  (relés adicionais).

عندما يعمل الوكيل بوضع ponte يخدم خدمتين للتطبيق:

- **`norito`** — Você pode fazer isso sem problemas
  `norito_bridge.spool_dir`. تتم تنقية الأهداف (para travessia e مسارات مطلقة),
  وعندما لا يحتوي الملف على امتداد يتم تطبيق اللاحقة المحددة قبل بث الحمولة.
- **`car`** — تُحل أهداف البث داخل `car_bridge.cache_dir`, وترث الامتداد
  Certifique-se de que o dispositivo esteja conectado a `allow_zst`. الجسور
  O código de barras `STREAM_ACK_OK` é o mais importante para você.
  التحقق بالأنابيب.No site do HMAC, o cache-tag (ou seja, o cache أثناء)
Nome do produto: `norito_*` / `car_*` / `car_*`
المتابعة بين النجاح, وفقدان الملفات, وفشل التنقية بسرعة.

`Orchestrator::local_proxy().await` يعرّض المقبض التشغيلي حتى يتمكن المستدعون
من قراءة شهادة PEM, أو جلب manifest للمتصفح, أو طلب إيقاف لطيف عند خروج
التطبيق.

عند التفعيل, يقدم الوكيل الآن سجلات **manifest v2**. إضافة إلى الشهادة e مفتاح
cache الحراس، تضيف v2 ما يلي:

- `alpn` (`"sorafs-proxy/1"`) e `capabilities` não são compatíveis.
- `session_id` para afinidade e afinidade `cache_tagging` para afinidade
  HMAC não é compatível.
- تلميحات الدوائر واختيار الحراس (`circuit`, `guard_selection`, `route_hints`)
  واجهة أغنى قبل فتح البث.
- `telemetry_v2` pode ser usado para evitar problemas e problemas.
- O `STREAM_ACK_OK` é o `cache_tag_hex`. يعكس العميل القيمة في ترويسة
  `x-sorafs-cache-tag` é uma solução para HTTP ou TCP que não é confiável
  مشفرة عند التخزين.

هذه الحقول متوافقة للخلف — يمكن للعملاء الأقدم تجاهل المفاتيح الجديدة
والاعتماد على مجموعة v1.

## 2. دلالات الفشل

Você pode fazer isso com uma chave de fenda e uma chave de fenda. تقع
Mais informações no site:

1. **إخفاقات الأهلية (قبل التنفيذ).** المزوّدون الذين يفتقرون لقدرة النطاق, أو
   anúncios منتهية, أو تليمترية قديمة يتم تسجيلهم في لوحة النتائج ويُستبعدون من
   sim. O CLI مصفوفة `ineligible_providers` está disponível para download
   Não há nada de errado com isso.
2. **الاستنزاف أثناء التشغيل.** يتتبع كل مزوّد الإخفاقات المتتالية. عند بلوغ
   `provider_failure_threshold` é um problema que `disabled` é um problema. sim
   Você pode usar o `disabled` para obter mais informações.
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **إجهاضات حتمية.** تظهر الحدود الصارمة كأخطاء مهيكلة:
   - `MultiSourceError::NoCompatibleProviders` — يتطلب المانيفست مدى شرائح أو
     Verifique se você está usando o software.
   - `MultiSourceError::ExhaustedRetries` — تم استهلاك ميزانية إعادة المحاولة لكل
     Bem.
   - `MultiSourceError::ObserverFailed` — رفض المراقبون downstream (خطاطيف البث)
     شريحة تم التحقق منها.

Certifique-se de que o produto esteja funcionando corretamente e que ele esteja funcionando corretamente. تعامل
مع هذه الأخطاء كمعوّقات إصدار — إعادة المحاولة بذات المدخلات ستعيد الفشل حتى
يتغير anúncio أو التليمترية, أو صحة المزوّد الأساسي.

### 2.1 حفظ لوحة النتائج

Use `persist_path` para remover o problema. يحتوي
Usando JSON aqui:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (não disponível).
- Use `provider` (pontos finais, pontos de extremidade, dados de conexão).

أرشِف لقطات لوحة النتائج مع آثار الإطلاق حتى تبقى قرارات الحظر والrollout
قابلة للتدقيق.

##3.

### 3.1 Módulo Prometheus

Use o código `iroha_telemetry`:| المقياس | Etiquetas | الوصف |
|--------|--------|-------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | مقياس عدّاد للجلب الجاري. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Você pode fazer isso sozinho. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | عدّاد للإخفاقات النهائية (استنزاف إعادة المحاولة, عدم وجود مزوّدين, فشل المراقب). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Você pode fazer isso com cuidado. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Verifique se o produto está funcionando corretamente. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | عدد قرارات سياسة الإخفاء (تحقق/تدهور) بحسب مرحلة rollout وسبب التراجع. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Você pode usar relés PQ usando SoraNet المختارة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | هيستوغرام لنسب توفر retransmite PQ no placar. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | هيستوغرام لعجز السياسة (الفجوة بين الهدف والحصة الفعلية). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Você pode usar relés em qualquer lugar. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Os relés devem ser usados ​​​​em qualquer lugar. |

A preparação do teste para o teste é uma tarefa simples. التخطيط الموصى به
Melhor observabilidade do SF-6:

1. **الجلب النشط** — تنبيه إذا ارتفع القياس دون اكتمالات مقابلة.
2. **نسبة إعادة المحاولة** — تحذير عند تجاوز عدادات `retry` للخطوط الاساسية.
3. **إخفاقات المزوّد** — تشغيل pager عند تجاوز أي مزوّد `session_failure > 0`
   Até 15 minutos.

### 3.2 اهداف السجل المهيكل

ينشر المُنسِّق أحداثاً مهيكلة إلى اهداف حتمية:

- `telemetry::sorafs.fetch.lifecycle` — علامات `start` e `complete` مع عدد الشرائح
  والمحاولات والمدة الاجمالية.
- `telemetry::sorafs.fetch.retry` — `provider`, `reason`,
  `attempts`) é uma triagem.
- `telemetry::sorafs.fetch.provider_failure` — مزوّدون تم تعطيلهم بسبب تكرار
  الاخطاء.
- `telemetry::sorafs.fetch.error` — `reason` e `reason`
  اختيارية.

وجّه هذه التدفقات إلى مسار السجلات Norito الحالي لكي تمتلك الاستجابة للحوادث
مصدر حقيقة واحد. تُظهر أحداث دورة الحياة مزيج PQ/الكلاسيكي عبر
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
ومعاداتها المرافقة, ما يجعل ربط لوحات المتابعة سهلاً دون كشط المقاييس. أثناء
rollouts بـ GA, ثبّت مستوى السجلات على `info` لأحداث دورة الحياة/إعادة
O código `warn` é compatível com o produto.

### 3.3 JSON

Você pode usar `sorafs_cli fetch` e SDK Rust para obter o seguinte:

- `provider_reports` pode ser usado para remover/desinstalar o dispositivo.
- `chunk_receipts` توضح المزوّد الذي لبّى كل شريحة.
- Exemplos `retry_stats` e `ineligible_providers`.

أرشِف ملف الملخص عند تتبع مزوّدين سيئين — تتطابق الإيصالات مباشرة مع بيانات
Não.

## 4. قائمة تشغيل تشغيلية1. **حضّر الاعداد في CI.** شغّل `sorafs_fetch` بالاعداد المستهدف, ومرر
   `--scoreboard-out` é um dispositivo de segurança que pode ser instalado e removido. أي مزوّد
   Isso significa que você pode fazer isso sem problemas.
2. **تحقق من التليمترية.** تأكد من أن النشر يصدر مقاييس `sorafs.fetch.*` وسجلات
   Isso pode ser feito por meio de uma mensagem de erro. غياب المقاييس يشير عادةً
   Não há nada que você possa fazer.
3. **وثّق التجاوزات.** عند تطبيق إعدادات طارئة `--deny-provider` ou
   `--boost-provider`, é JSON (ou CLI) na rede. يجب أن تعكس
   عمليات الرجوع إزالة التجاوز والتقاط لقطة placar جديدة.
4. **أعد تشغيل اختبارات fumaça.** بعد تعديل ميزانيات إعادة المحاولة, أو حدود
   O dispositivo de fixação do dispositivo elétrico (`fixtures/sorafs_manifest/ci_sample/`) está instalado
   Não há nada que você possa fazer.

اتباع الخطوات أعلاه يحافظ على قابلية إعادة الإنتاج عبر rollouts المرحلية ويوفر
Faça o download do seu telefone.

### 4.1 تجاوزات السياسة

يمكن للمشغلين تثبيت مرحلة النقل/الاخفاء النشطة دون تعديل الاعداد الاساسي عبر
Use `policy_override.transport_policy` e `policy_override.anonymity_policy` em
JSON é baseado em `orchestrator` (ou seja, `--transport-policy-override=` /
`--anonymity-policy-override=` ou `sorafs_cli fetch`). Não e substituir,
يتجاوز المُنسِّق fallback brownout المعتاد: إذا تعذر تحقيق طبقة PQ المطلوبة,
Use o `no providers` para obter mais informações. الرجوع للسلوك الافتراضي
Não é possível substituir a substituição.