---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: آرکسٹریٹر رول آؤٹ اور ٹیوننگ
sidebar_label: آرکسٹریٹر ٹیوننگ
description: ملٹی سورس آرکسٹریٹر کو GA تک لے جانے کے لیے عملی ڈیفالٹس, ٹیوننگ رہنمائی, اور آڈٹ چیک پوائنٹس۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator_tuning.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن مکمل طور پر ریٹائر نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

# آرکسٹریٹر رول آؤٹ اور ٹیوننگ گائیڈ

یہ گائیڈ [کنفیگریشن ریفرنس](orchestrator-config.md) اور
[ملٹی سورس رول آؤٹ رن بک](multi-source-rollout.md) پر مبنی ہے۔ یہ وضاحت کرتی ہے
کہ ہر رول آؤٹ مرحلے میں آرکسٹریٹر کو کیسے ٹیون کیا جائے, placar کے
آرٹیفیکٹس کو کیسے سمجھا جائے, اور ٹریفک بڑھانے سے پہلے کون سے ٹیلیمیٹری سگنلز
لازمی ہیں۔ No CLI, SDKs e SDKs podem ser adquiridos por meio de downloads gratuitos
ہر نوڈ ایک ہی ڈٹرمنسٹک fetch پالیسی پر عمل کرے۔

## 1. بنیادی پیرامیٹر سیٹس

ایک مشترکہ کنفیگریشن ٹیمپلیٹ سے آغاز کریں اور رول آؤٹ کے ساتھ چند منتخب botões
کو ایڈجسٹ کریں۔ نیچے جدول عام مراحل کے لیے تجویز کردہ اقدار دکھاتا ہے؛
O nome do produto é `OrchestratorConfig::default()` ou `FetchOptions::default()`
کے ڈیفالٹس پر واپس جاتی ہیں۔

| مرحلہ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Não |
|-------|-----------------|------------------------------------------|---------------------------------------|-------------------------------------------|--------------------------------------------------|-------|
| **Laboratório / CI** | `3` | `2` | `2` | `2500` | `300` | سخت limite de latência اور چھوٹی janela de graça شور والی ٹیلیمیٹری کو فوراً ظاہر کرتی ہے۔ novas tentativas کم رکھیں تاکہ غلط manifesta جلد سامنے آئیں۔ |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | پروڈکشن ڈیفالٹس کو منعکس کرتا ہے اور pares exploratórios کے لیے جگہ چھوڑتا ہے۔ |
| **Canário** | `6` | `3` | `3` | `5000` | `900` | ڈیفالٹس کے مطابق؛ `telemetry_region` سیٹ کریں تاکہ ڈیش بورڈز کینری ٹریفک الگ دکھا سکیں۔ |
| **Disponibilidade geral (GA)** | `None` (تمام elegível استعمال کریں) | `4` | `4` | `5000` | `900` | عارضی خرابیوں کو جذب کرنے کے لیے tentar novamente ou limites de falha رکھیں۔ |

- `scoreboard.weight_scale` ڈیفالٹ `10_000` پر رہتا ہے جب تک downstream سسٹم کسی اور resolução inteira کا تقاضا نہ کرے۔ اسکیل بڑھانے سے pedido do provedor نہیں بدلتی؛ صرف زیادہ گھنا distribuição de crédito بنتا ہے۔
- مراحل کے درمیان منتقلی میں JSON bundle محفوظ کریں اور `--scoreboard-out` استعمال کریں تاکہ آڈٹ ٹریل میں درست پیرامیٹر سیٹ ریکارڈ ہو۔

## 2. Placar

Manifesto do placar کی ضروریات، anúncios de provedores اور ٹیلیمیٹری کو یکجا کرتا ہے۔
O que fazer:1. **ٹیلیمیٹری کی تازگی چیک کریں۔** یقینی بنائیں کہ `--telemetry-json` میں حوالہ شدہ instantâneos
   Janela de graça مقررہ کے اندر ریکارڈ ہوئے ہوں۔ `telemetry_grace_secs` سے پرانی entradas
   `TelemetryStale { last_updated }` کے ساتھ فیل ہوں گی۔ Parada brusca
   ٹیلیمیٹری ایکسپورٹ اپڈیٹ کیے بغیر آگے نہ بڑھیں۔
2. **Eligibilidade وجوہات دیکھیں۔** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   کے ذریعے آرٹیفیکٹس محفوظ کریں۔ ہر entrada میں `eligibility` بلاک ہوتا ہے جو ناکامی
   کی درست وجہ بتاتا ہے۔ incompatibilidade de capacidade یا anúncios expirados کو اوور رائیڈ نہ کریں؛
   carga útil upstream
3. **Peso ڈیلٹاز کا جائزہ لیں۔** `normalised_weight` کو پچھلے liberação سے موازنہ کریں۔
   10% de desconto em anúncios/telemetria por anúncio/telemetria
   Para o log de implementação
4. **آرٹیفیکٹس آرکائیو کریں۔** `scoreboard.persist_path` سیٹ کریں تاکہ ہر رن میں
   فائنل scoreboard snapshot محفوظ ہو۔ اسے release ریکارڈ کے ساتھ manifesto اور
   pacote de telemetria کے ساتھ جوڑیں۔
5. **Provider mix کا ثبوت ریکارڈ کریں۔** `scoreboard.json` کی metadados اور متعلقہ
   `summary.json` `provider_count`, `gateway_provider_count` ou `provider_mix`
   لیبل لازماً ہو تاکہ جائزہ لینے والے ثابت کر سکیں کہ رن `direct-only`, `gateway-only`
   یا `mixed` تھا۔ Capturas de gateway `provider_count=0` e `provider_mix="gateway-only"`
   ہونا چاہیے, جبکہ misturado رنز میں دونوں سورسز کے لیے غیر صفر conta ضروری ہیں۔
   `cargo xtask sorafs-adoption-check` é um valor que não é compatível (incompatibilidade entre پر فیل ہوتا ہے),
   لہٰذا اسے ہمیشہ `ci/check_sorafs_orchestrator_adoption.sh` یا اپنے script de captura کے ساتھ
   Pacote de evidências `adoption_report.json` Os gateways Torii são instalados
   `gateway_manifest_id`/`gateway_manifest_cid` کو metadados do placar میں رکھیں تاکہ adoção
   envelope de manifesto de portão کو mix de provedor capturado سے جوڑ سکے۔

فیلڈز کی تفصیلی تعریف کے لیے
`crates/sorafs_car/src/scoreboard.rs` e estrutura de resumo CLI (ou `sorafs_cli fetch --json-out`
سے نکلتی ہے) دیکھیں۔

## CLI e SDK para instalação

`sorafs_cli fetch` (دیکھیں `crates/sorafs_car/src/bin/sorafs_cli.rs`)
Invólucro `iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) Branco
superfície de configuração do orquestrador شیئر کرتے ہیں۔ evidência de implementação یا canônica
replay de jogos کرنے کے لیے یہ flags استعمال کریں:

Referência de sinalizador de várias fontes compartilhada (ajuda CLI e documentos کو صرف اسی فائل میں ترمیم کر کے sincronização رکھیں):- Provedores elegíveis `--max-peers=<count>` کی تعداد محدود کرتا ہے جو placar فلٹر سے گزرتے ہیں۔ خالی چھوڑیں تاکہ تمام provedores qualificados سے stream ہو، اور `1` صرف تب سیٹ کریں جب جان بوجھ کر substituto de fonte única آزمانا ہو۔ SDKs são o botão `maxPeers` سے ہم آہنگ (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` کے limite de novas tentativas por bloco تک فارورڈ ہوتا ہے۔ تجویز کردہ اقدار کے لیے ٹیوننگ گائیڈ کے implementação جدول کو استعمال کریں؛ evidência جمع کرنے والی CLI رنز کو padrões do SDK سے میچ ہونا چاہیے۔
- `--telemetry-region=<label>` Prometheus `sorafs_orchestrator_*` سیریز (relés OTLP) پر região/env لیبل لگاتا ہے تاکہ ڈیش بورڈز laboratório, teste, canário اور GA ٹریفک الگ کر سکیں۔
- Placar `--telemetry-json=<path>` کے لیے injeção de instantâneo referenciada کرتا ہے۔ JSON کو scoreboard کے ساتھ محفوظ کریں تاکہ آڈیٹرز رن replay کر سکیں (اور `cargo xtask sorafs-adoption-check --require-telemetry` یہ ثابت کر سکے کہ کون سا captura de fluxo OTLP کو feed کر رہا تھا)۔
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) ganchos de observação de ponte کو فعال کرتے ہیں۔ جب سیٹ ہو تو orquestrador مقامی Norito/Kaigi proxy کے ذریعے chunks stream کرتا ہے تاکہ clientes de navegador, guarda caches اور salas Kaigi کو وہی recibos ملیں جو Rust emite کرتا ہے۔
- `--scoreboard-out=<path>` (اختیاری `--scoreboard-now=<unix_secs>` کے ساتھ) instantâneo de elegibilidade محفوظ کرتا ہے۔ محفوظ شدہ JSON کو ہمیشہ ticket de liberação میں telemetria referenciada اور artefatos de manifesto کے ساتھ جوڑیں۔
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` metadados de anúncio کے اوپر determinístico ایڈجسٹمنٹس لگاتے ہیں۔ ان bandeiras کو صرف ensaios کے لیے استعمال کریں؛ پروڈکشن downgrades کو artefatos de governança کے ذریعے جانا چاہیے تاکہ ہر نوڈ وہی pacote de políticas نافذ کرے۔
- `--provider-metrics-out` / `--chunk-receipts-out` métricas de integridade por provedor e recebimentos de pedaços محفوظ کرتے ہیں؛ evidência de adoção جمع کرتے وقت دونوں artefatos ضرور شامل کریں۔

مثال (شائع شدہ fixture استعمال کرتے ہوئے):

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

SDKs para configuração e `SorafsGatewayFetchOptions` para cliente Rust
(`crates/iroha/src/client.rs`), Ligações JS (`javascript/iroha_js/src/sorafs.js`) e
Swift SDK (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`) میں استعمال کرتے ہیں۔
ان ajudantes کو padrões CLI کے ساتھ lock-step رکھیں تاکہ آپریٹرز automação میں
پالیسیوں کو بغیر sob medida ترجمہ لیئرز کے کاپی کر سکیں۔

## 3. Buscar پالیسی ٹیوننگ

Novas tentativas `FetchOptions`, simultaneidade e verificação کو کنٹرول کرتا ہے۔ O que fazer:- **Retentativas:** `per_chunk_retry_limit` کو `4` سے اوپر بڑھانے سے recuperação وقت بڑھتا ہے مگر falhas do provedor چھپ سکتے ہیں۔ بہتر ہے `4` کو teto رکھا جائے اور rotação do provedor پر بھروسہ کیا جائے تاکہ کمزور artistas سامنے آئیں۔
- **Limite de falha:** `provider_failure_threshold` طے کرتا ہے کہ provedor کب سیشن کے باقی حصے کے لیے desativar ہو۔ اس ویلیو کو tentar novamente پالیسی سے ہم آہنگ رکھیں: اگر orçamento de repetição de limite سے کم ہو تو orquestrador تمام novas tentativas ختم ہونے سے پہلے peer کو نکال دیتا ہے۔
- **Simultaneidade:** `global_parallel_limit` کو `None` رکھیں جب تک مخصوص ماحول intervalos anunciados کو saturar نہ کر سکے۔ اگر سیٹ کریں تو ویلیو provedores کے orçamentos de fluxo کے مجموعے سے ≤ ہو تاکہ fome نہ ہو۔
- **Alternativas de verificação:** `verify_lengths` e `verify_digests` پروڈکشن میں فعال رہنے چاہئیں۔ یہ frotas de fornecedores mistos میں determinismo کی ضمانت دیتے ہیں؛ انہیں صرف ambientes fuzzing isolados میں ہی بند کریں۔

## 4. ٹرانسپورٹ اور انانیمٹی اسٹیجنگ

پرائیویسی postura کو ظاہر کرنے کے لیے `rollout_phase`, `anonymity_policy`, اور `transport_policy` استعمال کریں:

- `rollout_phase="snnet-5"` کو ترجیح دیں اور política de anonimato padrão کو marcos SNNet-5 کے ساتھ چلنے دیں۔ `anonymity_policy_override` صرف تب استعمال کریں جب governança نے diretiva assinada جاری کیا ہو۔
- `transport_policy="soranet-first"` کو linha de base رکھیں جب تک SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 ہوں
  (دیکھیں `roadmap.md`). `transport_policy="direct-only"` صرف دستاویزی downgrades یا exercícios de conformidade کے لیے استعمال کریں, اور revisão de cobertura PQ کے بعد ہی `transport_policy="soranet-strict"` پر جائیں—اس سطح پر صرف relés clássicos رہیں تو فوری falhar ہوگا۔
- `write_mode="pq-only"` é um caminho de gravação (SDK, orquestrador, ferramentas de governança) PQ. rollouts کے دوران `write_mode="allow-downgrade"` رکھیں تاکہ ہنگامی ردعمل rotas diretas پر انحصار کر سکے جبکہ downgrade de telemetria کو نشان زد کرے۔
- Seleção de guarda e diretório SoraNet de teste de circuito پر منحصر ہیں۔ Instantâneo `relay_directory` assinado فراہم کریں اور `guard_set` cache محفوظ کریں تاکہ guarda churn طے شدہ janela de retenção میں رہے۔ `sorafs_cli fetch` کے ذریعے لاگ ہونے والا evidência de implementação de impressão digital em cache کا حصہ ہے۔

## 5. Downgrade e ganchos de conformidade

O que você precisa saber sobre o seu negócio:

- **Remediação de downgrade** (`downgrade_remediation`): `handshake_downgrade_total` ایونٹس کی نگرانی کرتا ہے اور `window_secs` کے اندر `threshold` تجاوز ہونے پر local proxy کو `target_mode` پر مجبور کرتا ہے (ڈیفالٹ somente metadados)۔ ڈیفالٹس (`threshold=3`, `window=300`, `cooldown=900`) برقرار ریویوز مختلف پیٹرن نہ بتائیں۔ کسی بھی substituir کو log de implementação
- **Política de conformidade** (`compliance`): jurisdição اور exclusões manifestas listas de exclusão gerenciadas pela governança کے ذریعے آتے ہیں۔ Pacote کنفیگریشن میں substituições ad-hoc مت ڈالیں؛ اس کے بجائے `governance/compliance/soranet_opt_outs.json` کے لیے atualização assinada طلب کریں اور gerado JSON دوبارہ implantar کریں۔دونوں سسٹمز کے لیے, نتیجہ خیز کنفیگریشن pacote محفوظ کریں اور اسے liberar evidências میں شامل کریں Quais são os gatilhos de downgrade?

## 6. ٹیلیمیٹری اور ڈیش بورڈز

رول آؤٹ بڑھانے سے پہلے تصدیق کریں کہ درج ذیل سگنلز alvo ماحول میں فعال ہیں:

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  کینری مکمل ہونے کے بعد صفر ہونا چاہیے۔
- `sorafs_orchestrator_retries_total`
  `sorafs_orchestrator_retry_ratio` — کینری کے دوران 10% سے کم پر مستحکم ہوں اور GA کے بعد 5% سے کم رہیں۔
- `sorafs_orchestrator_policy_events_total` — Estágio de implementação do estágio de implementação کی تصدیق کرتا ہے (etiqueta `stage`) اور `outcome` کے ذریعے quedas de energia ریکارڈ کرتا ہے۔
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — پالیسی توقعات کے مقابلے میں Relé PQ سپلائی ٹریک کرتے ہیں۔
- `telemetry::sorafs.fetch.*` لاگ ٹارگٹس — مشترکہ لاگ ایگریگیٹر پر جائیں اور `status=failed` کے لیے محفوظ تلاشیں ہوں۔

`dashboards/grafana/sorafs_fetch_observability.json` سے painel canônico Grafana لوڈ کریں
(پورٹل میں **SoraFS → Fetch Observability** کے تحت ایکسپورٹ شدہ), تاکہ seletores de região/manifesto,
mapa de calor de repetição do provedor, histogramas de latência de blocos e contadores de paralisação وہی ہوں ou SRE
burn-ins Alertmanager rules کو `dashboards/alerts/sorafs_fetch_rules.yml`
Sintaxe `scripts/telemetry/test_sorafs_fetch_alerts.sh` ou Prometheus
ویلیڈیٹ کریں (ajudante `promtool test rules` کو لوکل یا Docker میں چلاتا ہے)۔ Transferências de alerta
Roteamento de roteamento
ٹکٹ سے جوڑ سکیں۔

### ٹیلیمیٹری burn-in ورک فلو

Item de roteiro **SF-6e** کے تحت 30 دن کی burn-in de telemetria درکار ہے اس سے پہلے کہ
ملٹی سورس آرکسٹریٹر GA ڈیفالٹس پر چلا جائے۔ ریپو اسکرپٹس کے ذریعے ہر دن کے لیے
ریپروڈیوس ایبل آرٹیفیکٹس بنائیں:

1. `ci/check_sorafs_orchestrator_adoption.sh` کو botões de ambiente de gravação کے ساتھ چلائیں۔ Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Ajudante `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ری پلے کرتا ہے،
   `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`,
   O `adoption_report.json` e o `artifacts/sorafs_orchestrator/<timestamp>/` são iguais.
   اور `cargo xtask sorafs-adoption-check` کے ذریعے کم از کم provedores qualificados impõem کرتا ہے۔
2. Burn-in ویری ایبلز موجود ہوں تو اسکرپٹ `burn_in_note.json` بھی بناتا ہے, جس میں rótulo،
   índice do dia, ID do manifesto, fonte de telemetria e resumos de artefatos محفوظ ہوتے ہیں۔ Implementação
   log میں شامل کریں تاکہ واضح ہو کہ 30 روزہ ونڈو کا کون سا دن کس capturar سے پورا ہوا۔
3. Grafana Grafana (`dashboards/grafana/sorafs_fetch_observability.json`) para preparação/produção
   espaço de trabalho میں امپورٹ کریں, rótulo de gravação لگائیں, اور تصدیق کریں کہ ہر پینل ٹیسٹ شدہ
   manifesto/região کے نمونے دکھاتا ہے۔
4. Nome `dashboards/alerts/sorafs_fetch_rules.yml` ou `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   (یا `promtool test rules …`) چلائیں تاکہ métricas de burn-in de roteamento de alerta کے مطابق ہونا دستاویزی ہو۔
5. Faça o snapshot, saída de teste de alerta e `telemetry::sorafs.fetch.*` para obter o valor desejado
   آرٹیفیکٹس کے ساتھ محفوظ کریں تاکہ governança بغیر sistemas ativos سے métricas اٹھائے evidências
   ری پلے کر سکے۔

## 7. Rodar o jogo1. CI میں configuração do candidato کے ساتھ placares دوبارہ بنائیں اور artefatos کو controle de versão میں رکھیں۔
2. ہر ماحول (laboratório, teste, canário, produção) میں busca determinística de fixture چلائیں اور `--scoreboard-out` اور `--json-out` artefatos کو rollout ریکارڈ کے ساتھ منسلک کریں۔
3. de plantão دیے گئے تمام métricas کے amostras ao vivo موجود ہیں۔
4. حتمی کنفیگریشن پاتھ (عام طور پر `iroha_config` کے ذریعے) اور registro de governança کے git commit کو ریکارڈ کریں جو anúncios اور compliance کے لیے استعمال ہوا۔
5. rastreador de lançamento انٹیگریشنز سیدھ میں رہیں۔

اس گائیڈ کی پیروی آرکسٹریٹر ڈیپلائمنٹس کو ڈٹرمنسٹک اور آڈٹ ایبل رکھتی ہے جبکہ
orçamentos de repetição, capacidade do provedor, postura de privacidade کو ٹیون کرنے کے لیے واضح
فیڈبیک لوپس فراہم کرتی ہے۔