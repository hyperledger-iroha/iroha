---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: ملٹی سورس رول آؤٹ اور پرووائیڈر بلیک لسٹنگ رن بک
sidebar_label: ملٹی سورس رول آؤٹ رن بک
description: مرحلہ وار ملٹی سورس رول آؤٹس اور ہنگامی پرووائیڈر بلیک لسٹنگ کے لیے آپریشنل چیک لسٹ۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/multi_source_rollout.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن سیٹ ریٹائر نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک SRE اور آن کال انجینئرز کو دو اہم ورک فلو میں رہنمائی کرتی ہے:

1. ملٹی سورس آرکسٹریٹر کو کنٹرولڈ ویوز میں رول آؤٹ کرنا۔
2. موجودہ سیشنز کو غیر مستحکم کیے بغیر خراب کارکردگی والے پرووائیڈرز کو بلیک O que é melhor para você

یہ فرض کرتی ہے کہ SF-6 کے تحت فراہم کردہ pilha de orquestração پہلے ہی ڈیپلائے ہے (`sorafs_orchestrator`, gateway chunk-range API, e exportadores de telemetria).

> **مزید دیکھیں:** [آرکسٹریٹر آپریشنز رن بک](./orchestrator-ops.md) فی رن طریقۂ کار (captura do placar, مرحلہ وار rollout alterna، اور rollback) میں تفصیل دیتی ہے۔ لائیو تبدیلیوں کے دوران دونوں حوالوں کو ساتھ استعمال کریں۔

## 1. قبل از عمل توثیق

1. **گورننس ان پٹس کی تصدیق کریں۔**
   - تمام امیدوار پرووائیڈرز کو cargas úteis de capacidade de alcance اور orçamentos de fluxo کے ساتھ envelopes `ProviderAdvertV1` شائع کرنے چاہئیں۔ `/v1/sorafs/providers` سے ویلیڈیٹ کریں اور متوقع capacidade فیلڈز سے موازنہ کریں۔
   - taxas de latência/falha
2. **کنفیگریشن اسٹیج کریں۔**
   - آرکسٹریٹر کی JSON کنفیگریشن کو `iroha_config` em camadas ٹری میں محفوظ کریں:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Implementação de JSON سے متعلق حدود (`max_providers`, repetir orçamentos) encenação/produção میں e فائل دیں تاکہ فرق کم رہے۔
3. ** luminárias canônicas چلائیں۔ **
   - variáveis de ambiente de manifesto/token سیٹ کریں اور busca determinística چلائیں:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     variáveis de ambiente میں manifest payload digest (hex) اور ہر canary پرووائیڈر کے لیے tokens de fluxo codificados em base64 شامل ہونے چاہئیں۔
   - `artifacts/canary.scoreboard.json` کو پچھلے lançamento سے comparar کریں۔ کوئی نیا غیر اہل پرووائیڈر یا وزن میں >10% تبدیلی revisão مانگتی ہے۔
4. **ٹیلیمیٹری کی وائرنگ چیک کریں۔**
   - `docs/examples/sorafs_fetch_dashboard.json` میں Grafana exportar کھولیں۔ آگے بڑھنے سے پہلے یقینی بنائیں کہ `sorafs_orchestrator_*` preparação de métricas میں preencher ہو رہی ہیں۔

## 2. ہنگامی پرووائیڈر بلیک لسٹنگ

جب کوئی پرووائیڈر خراب pedaços فراہم کرے، مستقل tempos limite دے، یا verificações de conformidade میں فیل ہو تو یہ طریقہ کار اپنائیں۔1. **ثبوت محفوظ کریں۔**
   - تازہ ترین buscar resumo ایکسپورٹ کریں (saída `--json-out` کا)۔ ناکام índices de pedaços, پرووائیڈر aliases, اور digerir incompatibilidades ریکارڈ کریں۔
   - `telemetry::sorafs.fetch.*` alvos سے متعلقہ لاگ trechos محفوظ کریں۔
2. **فوری substituir لگائیں۔**
   - آرکسٹریٹر کو دیے گئے instantâneo de telemetria میں پرووائیڈر کو penalizado مارک کریں (`penalty=true` سیٹ کریں یا `token_health` کو `0` پر braçadeira کریں)۔ Construir placar خود بخود پرووائیڈر کو excluir کر دے گا۔
   - testes de fumaça ad-hoc کے لیے `sorafs_cli fetch` میں `--deny-provider gw-alpha` پاس کریں تاکہ propagação de telemetria کا انتظار کیے بغیر exercício de caminho de falha ہو۔
   - متاثرہ ماحول میں اپڈیٹ شدہ pacote de telemetria/configuração دوبارہ implantar کریں (preparação → canário → produção)۔ تبدیلی کو registro de incidentes میں دستاویز کریں۔
3. **substituir ویلیڈیٹ کریں۔**
   - busca de fixture canônico دوبارہ چلائیں۔ تصدیق کریں کہ placar نے پرووائیڈر کو `policy_denied` وجہ کے ساتھ inelegível مارک کیا ہے۔
   - `sorafs_orchestrator_provider_failures_total` چیک کریں تاکہ انکار شدہ پرووائیڈر کے لیے کاؤنٹر مزید نہ بڑھے۔
4. **طویل مدتی پابندی بڑھائیں۔**
   - اگر پرووائیڈر >24 h کے لیے بلاک رہے گا تو اس کے anúncio کو girar یا suspender کرنے کے لیے ticket de governança بنائیں۔ ووٹ پاس ہونے تک lista de bloqueios میں نہ آئے۔
5. **رول بیک پروٹوکول۔**
   - پرووائیڈر بحال کرنے کے لیے اسے lista de negação سے ہٹا دیں, دوبارہ implantar کریں, اور نیا instantâneo do placar محفوظ کریں۔ تبدیلی کو incidente postmortem کے ساتھ منسلک کریں۔

## 3. مرحلہ وار رول آؤٹ پلان

| مرحلہ | دائرہ کار | لازمی سگنلز | Vá/Não-Vá
|-------|-----------|-------------|----------------|
| **Laboratório** | مخصوص integração کلسٹر | cargas úteis de fixtures کے ساتھ دستی CLI fetch | تمام pedaços کامیاب ہوں, contadores de falha do provedor صفر پر رہیں, taxa de repetição < 5%. |
| **Encenação** | Preparação de plano de controle | Painel de controle Grafana regras de alerta صرف somente aviso موڈ میں | `sorafs_orchestrator_active_fetches` ہر ٹیسٹ رن کے بعد صفر پر واپس آئے؛ کوئی `warn/critical` Nome de usuário |
| **Canário** | پروڈکشن ٹریفک کا ≤10% | pager خاموش مگر ٹیلیمیٹری em tempo real مانیٹر | taxa de novas tentativas <10%, falhas do provedor صرف معلوم peers barulhentos تک محدود, latência histograma staging linha de base ±20% کے مطابق۔ |
| **Disponibilidade Geral** | 100% Rodada | regras do pager 24 h تک `NoHealthyProviders` کی صفر erros, taxa de repetição مستحکم, painéis SLA do painel سبز۔ |

ہر مرحلے میں:

1. مطلوبہ `max_providers` para repetir orçamentos کے ساتھ آرکسٹریٹر JSON اپڈیٹ کریں۔
2. dispositivo canônico اور ماحول کے نمائندہ manifesto کے خلاف `sorafs_cli fetch` یا testes de integração SDK چلائیں۔
3. placar + artefatos de resumo محفوظ کریں اور registro de lançamento کے ساتھ منسلک کریں۔
4. اگلے مرحلے پر جانے سے پہلے de plantão انجینئر کے ساتھ painéis de telemetria ریویو کریں۔

## 4. Observabilidade e ganchos de incidentes- **Métricas:** یقینی بنائیں کہ Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total` کو مانیٹر کر رہا ہے۔ اچانک اسپائک عام طور پر یہ بتاتا ہے کہ پرووائیڈر لوڈ میں degradar ہو رہا ہے۔
- **Logs:** Alvos `telemetry::sorafs.fetch.*` کو مشترکہ لاگ ایگریگیٹر پر روٹ کریں۔ `event=complete status=failed` کے لیے محفوظ تلاشیں بنائیں تاکہ triagem تیز ہو۔
- **Placares:** ہر artefato de placar کو طویل مدتی اسٹوریج میں محفوظ کریں۔ Revisões de conformidade JSON e reversões encenadas کے لیے trilha de evidências بھی ہے۔
- **Painéis:** placa canônica Grafana (`docs/examples/sorafs_fetch_dashboard.json`) کو پروڈکشن فولڈر میں کلون کریں اور `docs/examples/sorafs_fetch_alerts.yaml` کی regras de alerta شامل کریں۔

## 5. Comunicação e Documentação

- ہر deny/boost تبدیلی کو Operations Changelog میں timestamp, آپریٹر, وجہ اور متعلقہ incidente کے ساتھ لاگ کریں۔
- جب pesos do provedor یا orçamentos de repetição بدلیں تو SDK ٹیموں کو مطلع کریں تاکہ کلائنٹ سائیڈ توقعات ہم آہنگ رہیں۔
- GA مکمل ہونے کے بعد `status.md` میں resumo de implementação اپڈیٹ کریں اور اس رن بک ریفرنس کو notas de versão میں arquivo کریں۔