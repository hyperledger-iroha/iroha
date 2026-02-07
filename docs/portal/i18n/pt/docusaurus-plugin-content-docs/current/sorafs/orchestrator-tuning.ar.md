---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: إطلاق وضبط المُنسِّق
sidebar_label: ضبط المُنسِّق
description: قيم افتراضية عملية وإرشادات ضبط ونقاط تدقيق لمواءمة المُنسِّق متعدد المصادر مع GA.
---

:::note المصدر المعتمد
É `docs/source/sorafs/developer/orchestrator_tuning.md`. Não se preocupe, você pode usar o software para evitar problemas.
:::

# دليل إطلاق وضبط المُنسِّق

Isso é feito por [مرجع الإعداد](orchestrator-config.md) e
[multi-source-rollout.md]. يشرح
كيفية ضبط المُنسِّق لكل مرحلة إطلاق, وكيفية قراءة آثار لوحة النتائج, وما هي
Verifique se há algum problema com isso. طبّق التوصيات بشكل متسق عبر
CLI e SDKs podem ser usados ​​para acessar o site e o site.

## 1. مجموعات المعلمات الأساسية

Você pode fazer isso com uma chave de fenda e usar uma chave de fenda. يلتقط
الجدول أدناه القيم الموصى بها لأكثر المراحل شيوعًا؛ وما لم يُذكر يعود إلى
Os valores são `OrchestratorConfig::default()` e `FetchOptions::default()`.

| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | الملاحظات |
|---------|-----------------|------------------------------------------|---------------------------------------|--------------------------------------------|----------------------------------------|-----------|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | Você pode fazer isso sem precisar de mais nada. Verifique se o dispositivo está funcionando corretamente. |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | Isso significa que você pode fazer isso sem problemas. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ Use `telemetry_region` para remover o problema do dispositivo. |
| **الإتاحة العامة (GA)** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | Faça o download do seu telefone e coloque-o no lugar certo para que você possa usá-lo. |

- يبقى `scoreboard.weight_scale` على القيمة الافتراضية `10_000` ما لم يتطلب نظام لاحق دقة عددية Obrigado. زيادة المقياس لا تغيّر ترتيب المزوّدين؛ Não se preocupe.
- عند الانتقال بين المراحل, احفظ حزمة JSON واستخدم `--scoreboard-out` لتوثيق مجموعة المعلمات الدقيقة في مسار التدقيق.

## 2. نظافة لوحة النتائج

تجمع لوحة النتائج بين متطلبات المانيفست وإعلانات المزوّدين والتليمترية.
Como fazer isso:1. **تحقّق من حداثة التليمترية.** تأكد من أن اللقطات المشار إليها عبر
   `--telemetry-json` é um problema que não funciona. الإدخالات الأقدم من
   `telemetry_grace_secs` é compatível com `TelemetryStale { last_updated }`.
   Verifique se o seu produto está funcionando corretamente e se você está usando o produto.
2. **راجع أسباب الأهلية.** احفظ الآثار عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل إدخال
   Use o `eligibility` para removê-lo. Não há nada melhor do que isso
   الإعلانات المنتهية; أصلح الحمولة المصدرية.
3. **راجع تغيّر الأوزان.** قارن حقل `normalised_weight` بالإصدار السابق. تغيّر
   Mais de 10% de desconto no preço do dinheiro para o dinheiro e para o dinheiro
   Isso é tudo.
4. **أرشف الآثار.** اضبط `scoreboard.persist_path` ليصدر كل تشغيل لقطة لوحة النتائج
   Não. أرفق الأثر بسجل الإصدار مع المانيفست e حزمة التليمترية.
5. **وثّق دليل خليط المزوّدين.** يجب أن تعرض ميتاداتا `scoreboard.json` e `summary.json`
   O código `provider_count` e `gateway_provider_count` e o `provider_mix` estão disponíveis.
   Eu estou usando `direct-only` ou `gateway-only` ou `mixed`. يجب أن تُظهر لقطات
   Descrição de `provider_count=0` e `provider_mix="gateway-only"`,
   Você pode fazer isso sem parar. `cargo xtask sorafs-adoption-check` `cargo xtask sorafs-adoption-check`
   (ويفشل إذا لم تتطابق العدادات/التسميات), لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` é um dispositivo de segurança para uso doméstico
   `adoption_report.json`. Você pode usar o Torii, o `gateway_manifest_id`/
   `gateway_manifest_cid` é uma ferramenta que pode ser usada para obter mais informações
   ربط مغلف المانيفست بخليط المزوّدين الملتقط.

للتعريفات التفصيلية للحقول راجع
`crates/sorafs_car/src/scoreboard.rs` é compatível com CLI
`sorafs_cli fetch --json-out`.

## مرجع أعلام CLI e SDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`) e
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) تشترك في
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالية عند التقاط أدلة الإطلاق أو
إعادة تشغيل الـ fixtures:

Para obter mais informações sobre a CLI (você pode usar a CLI e usar a interface de usuário CLI para fazer isso):- `--max-peers=<count>` يحدّ عدد المزوّدين المؤهلين الذين ينجون من فلتر لوحة النتائج. اتركه فارغًا لتدفق جميع المزوّدين المؤهلين, واضبطه على `1` فقط عند اختبار الرجوع لمصدر واحد عمدًا. Use `maxPeers` em SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يمرّر حد إعادة المحاولة لكل chunk الذي يطبقه `FetchOptions`. Faça o download do cartão de crédito no site da empresa Não use o CLI para instalar o SDK do SDK.
- `--telemetry-region=<label>` يوسم سلاسل Prometheus `sorafs_orchestrator_*` (ومرحّلات OTLP) بوسم المنطقة/البيئة حتى تتمكن لوحات المتابعة من فصل حركة المختبر وstaging والكناري وGA.
- `--telemetry-json=<path>` não é compatível com o software. O JSON é usado para definir o valor do arquivo `cargo xtask sorafs-adoption-check --require-telemetry` ou `cargo xtask sorafs-adoption-check --require-telemetry`. OTLP não é compatível).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) não use ganchos. Você pode usar pedaços de proxy Norito/Kaigi para proteger caches e proteger caches وغرف Kaigi نفس الإيصالات التي يصدرها Rust.
- `--scoreboard-out=<path>` (ou seja, `--scoreboard-now=<unix_secs>`) não funciona. Você pode usar o JSON para definir o valor do arquivo e usá-lo no site.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يطبّقان تعديلات حتمية فوق ميتاداتا الإعلانات. استخدم هذه الأعلام للتجارب فقط؛ Você pode usar artefatos que não sejam úteis para você.
- `--provider-metrics-out` / `--chunk-receipts-out` يحتفظان بمقاييس صحة المزوّدين وإيصالات الـ chunks المرجعية في قائمة التحقق؛ Verifique se o produto está funcionando corretamente.

مثال (باستخدام الـ fixture المنشور):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Baixar SDKs do `SorafsGatewayFetchOptions` para Rust
(`crates/iroha/src/client.rs`) Escrita JS
(`javascript/iroha_js/src/sorafs.js`) e SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
A CLI está disponível para download no site da CLI.
طبقات ترجمة مخصصة.

## 3. ضبط سياسة الجلب

Use `FetchOptions` em qualquer lugar e local. Mais informações:

- **إعادة المحاولة:** رفع `per_chunk_retry_limit` فوق `4` يزيد زمن الاستعادة لكنه
  Isso não é verdade. Você pode usar o `4` para obter mais informações sobre o produto
  لإظهار الأداء الضعيف.
- **عتبة الفشل:** تحدد `provider_failure_threshold` متى يتم تعطيل المزوّد لبقية الجلسة.
  يجب أن تتوافق هذه القيمة مع سياسة إعادة المحاولة: عتبة أقل من ميزانية المحاولة
  Verifique se há algum problema com isso.
- **التوازي:** اترك `global_parallel_limit` غير مضبوط (`None`) ما لم تكن بيئة محددة
  Verifique se há algum problema com isso. عند ضبطه, تأكد أن القيمة ≤ مجموع ميزانيات
  Faça o download corretamente.
- **مفاتيح التحقق:** يجب إبقاء `verify_lengths` e `verify_digests` مفعّلين في الإنتاج.
  فهي تضمن الحتمية عندما تعمل أساطيل مزوّدين مختلطة؛ عطّلها فقط في بيئات fuzzing المعزولة.

## 4. مراحل النقل والخصوصية

Para definir o `rollout_phase`, `anonymity_policy` e `transport_policy`, siga os passos abaixo:- فضّل `rollout_phase="snnet-5"` ودع سياسة الخصوصية الافتراضية تتبع معالم SNNet-5. استعمل
  `anonymity_policy_override` é um dispositivo que pode ser usado para evitar problemas.
- أبقِ `transport_policy="soranet-first"` كخيار أساس بينما SNNet-4/5/5a/5b/6a/7/8/12/13 في 🈺
  (راجع `roadmap.md`). Use `transport_policy="direct-only"` para obter informações sobre o produto e a solução de problemas
  وانتظر مراجعة تغطية PQ قبل الترقية إلى `transport_policy="soranet-strict"`—هذا المستوى يفشل سريعًا
  Você pode fazer isso com cuidado.
- A versão `write_mode="pq-only"` é uma solução de segurança para arquivos de software (SDK, software de gerenciamento de arquivos)
  تلبية متطلبات PQ. A instalação do `write_mode="allow-downgrade"` é um problema de segurança
  Você pode usar o aplicativo para obter mais informações.
- يعتمد اختيار الحراس eتجهيز الدوائر على دليل SoraNet. Como usar `relay_directory`
  O cache `guard_set` é usado para remover o cache do dispositivo. بصمة الكاش
  O `sorafs_cli fetch` deve ser instalado no lugar certo.

## 5. ganchos

يساعد نظامان فرعيان في المُنسِّق على فرض السياسة دون تدخل يدوي:

- **معالجة التخفيض** (`downgrade_remediation`): تراقب أحداث `handshake_downgrade_total`, وبعد تجاوز
  `threshold` ou `window_secs` transfere o proxy para `target_mode` (somente metadados).
  Use o código de barras (`threshold=3`, `window=300`, `cooldown=900`) para obter mais informações
  Não há problema. Você não pode substituir no seu computador e não precisa substituir o software
  `sorafs_proxy_downgrade_state`.
- **سياسة الامتثال** (`compliance`): تمر استثناءات الولاية القضائية والمانيفست عبر قوائم opt‑out التي
  تديرها الحوكمة. O que você precisa fazer é substituir o que você quer saber اطلب تحديثًا موقعًا
  O `governance/compliance/soranet_opt_outs.json` é um arquivo JSON.

بالنسبة لكلا النظامين, احفظ حزمة الإعداد الناتجة e أرفقها بأدلة الإصدار حتى يتمكن
Você pode fazer isso sem problemas.

## 6. التليمترية ولوحات المتابعة

قبل توسيع الإطلاق, تأكد, أن الإشارات التالية نشطة في البيئة المستهدفة:

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  Não há nenhum problema com isso.
- `sorafs_orchestrator_retries_total` e
  `sorafs_orchestrator_retry_ratio` — A taxa de juros é de 10%
  Mais 5% do GA.
- `sorafs_orchestrator_policy_events_total` — يتحقق من أن مرحلة الإطلاق المطلوبة
  فعالة (etiqueta `stage`) e a queda de energia é `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — Verifique se o PQ está funcionando corretamente.
- أهداف سجلات `telemetry::sorafs.fetch.*` — يجب بثها إلى مجمّع السجلات المشترك مع
  Você pode usar o `status=failed`.

Como usar o Grafana no site
`dashboards/grafana/sorafs_fetch_observability.json` (`dashboards/grafana/sorafs_fetch_observability.json`)
**SoraFS → Buscar Observabilidade**)
O problema é que o SRE está queimando.
Use o Alertmanager em `dashboards/alerts/sorafs_fetch_rules.yml` e instale-o
O Prometheus é o `scripts/telemetry/test_sorafs_fetch_alerts.sh`
`promtool test rules` é compatível ou Docker). تتطلب عمليات تسليم التنبيه نفس كتلة
Você pode fazer isso com uma chave de fenda.

### تدفق burn-in للتليمتريةيتطلب بند خارطة الطريق **SF-6e** Tempo de burn-in للتليمترية لمدة 30 dias após a instalação
المُنسِّق متعدد المصادر إلى افتراضاته GA. استخدم سكربتات المستودع لالتقاط حزمة
آثار قابلة لإعادة الإنتاج لكل يوم في النافذة:

1. Verifique `ci/check_sorafs_orchestrator_adoption.sh` para evitar queimaduras. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Use o código `fixtures/sorafs_orchestrator/multi_peer_parity_v1`, e use-o
   `scoreboard.json` e `summary.json` e `provider_metrics.json` e `chunk_receipts.json`
   و`adoption_report.json` ou `artifacts/sorafs_orchestrator/<timestamp>/`،
   A chave de segurança é `cargo xtask sorafs-adoption-check`.
2. Verifique se há burn-in, use o `burn_in_note.json` para fazer o download.
   وفهرس اليوم, ومعرّف المانيفست, ومصدر التليمترية, وملخصات الآثار. Como usar JSON
   بسجل الإطلاق لتوضيح أي لقطة استوفت كل يوم ضمن نافذة 30 dias.
3. Use o código Grafana (`dashboards/grafana/sorafs_fetch_observability.json`)
   Em termos de encenação/produção, e burn-in, e de qualquer lugar do mundo.
   للمانيفست/المنطقة قيد الاختبار.
4. Modelo `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   Use o `dashboards/alerts/sorafs_fetch_rules.yml` para obter mais informações
   يطابق المقاييس المصدرة أثناء burn-in.
5. أرشف لقطة لوحة المتابعة, ومخرجات اختبار التنبيهات, وذيل السجلات لعمليات البحث
   `telemetry::sorafs.fetch.*` é um dispositivo de armazenamento de dados de alta qualidade
   الأدلة دون الاعتماد على أنظمة حية.

## 7. قائمة تحقق الإطلاق

1. Os placares de pontuação são definidos no CI باستخدام الإعداد المرشح e no CI.
2. شغّل جلب fixtures الحتمي في كل بيئة (المختبر، staging, الكناري, الإنتاج) وأرفق
   Verifique `--scoreboard-out` e `--json-out`.
3. راجع لوحات التليمترية مع مهندس المناوبة, وتأكد أن كل المقاييس أعلاه لديها عينات حية.
4. Execute o comando git (ou seja, `iroha_config`) e confirme o git para executar o procedimento.
   Não use nada.
5. Faça o download do SDK e do SDK do seu computador para que você possa usá-lo Obrigado.

اتباع هذا الدليل يحافظ على إطلاقات المُنسِّق حتمية وقابلة للتدقيق, مع توفير حلقات
تغذية راجعة واضحة لضبط ميزانيات إعادة المحاولة, وسعة المزوّدين, ووضع الخصوصية.