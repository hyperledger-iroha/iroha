---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: SoraFS آرکسٹریٹر کنفیگریشن
sidebar_label: آرکسٹریٹر کنفیگریشن
description: ملٹی سورس fetch آرکسٹریٹر کو کنفیگر کریں, ناکامیوں کی تشریح کریں, اور ٹیلیمیٹری آؤٹ پٹ ڈی بگ کریں۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن ریٹائر نہ ہو جائے, دونوں نقول کو ہم آہنگ رکھیں۔
:::

# ملٹی سورس fetch آرکسٹریٹر گائیڈ

SoraFS کا ملٹی سورس fetch آرکسٹریٹر گورننس سپورٹڈ anúncios میں شائع شدہ
پرووائیڈرز سیٹ سے ڈٹرمنسٹک اور متوازی ڈاؤن لوڈز چلاتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ آرکسٹریٹر کیسے کنفیگر کیا جائے, رول آؤٹس کے دوران کون سی ناکامی سگنلز
متوقع ہیں, اور کون سے ٹیلیمیٹری اسٹریمز صحت کے اشارے ظاہر کرتے ہیں۔

## 1. کنفیگریشن اوور ویو

A maneira mais fácil de fazer isso é:

| Ouro | مقصد | Não |
|------|------|------|
| `OrchestratorConfig.scoreboard` | پرووائیڈر pesos کو نارملائز کرتا ہے, ٹیلیمیٹری کی تازگی ویریفائی کرتا ہے, اور آڈٹس کے لیے JSON scoreboard محفوظ کرتا ہے۔ | `crates/sorafs_car::scoreboard::ScoreboardConfig` de alta qualidade |
| `OrchestratorConfig.fetch` | رن ٹائم حدود لگاتا ہے (repetir orçamentos, limites de simultaneidade, alternadores de verificação)۔ | `crates/sorafs_car::multi_fetch` میں `FetchOptions` سے میپ ہوتا ہے۔ |
| CLI / SDK Aplicativos | peers کی تعداد محدود کرتے ہیں, regiões de telemetria لگاتے ہیں, اور negar/aumentar پالیسیز ظاہر کرتے ہیں۔ | `sorafs_cli fetch` یہ sinalizadores براہ راست دیتا ہے؛ SDKs انہیں `OrchestratorConfig` کے ذریعے پاس کرتے ہیں۔ |

`crates/sorafs_orchestrator::bindings` contém ajudantes JSON que são usados ​​para Norito
JSON میں serialize کرتے ہیں، جس سے اسے SDK bindings e اور آٹومیشن میں پورٹیبل
بنایا جا سکے۔

### 1.1 JSON JSON کنفیگریشن

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

فائل کو معمول کے Camadas `iroha_config` (`defaults/`, usuário, real) کے ذریعے
محفوظ کریں تاکہ ڈٹرمنسٹک implantações تمام نوڈز پر ایک جیسے limites استعمال کریں۔
Implementação SNNet-5a سے ہم آہنگ fallback somente direto پروفائل کے لیے
`docs/examples/sorafs_direct_mode_policy.json` `docs/examples/sorafs_direct_mode_policy.json`
`docs/source/sorafs/direct_mode_pack.md` دیکھیں۔

### 1.2 Substituições de conformidade

SNNet-9 آرکسٹریٹر میں گورننس ڈرائیون کمپلائنس شامل کرتا ہے۔ Norito JSON
کنفیگریشن میں ایک نیا `compliance` آبجیکٹ carve-outs محفوظ کرتا ہے جو fetch
پائپ لائن کو direct-only موڈ پر مجبور کرتے ہیں:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` ISO‑3166 alpha‑2 کوڈز ڈکلیئر کرتا ہے جہاں یہ
  آرکسٹریٹر انسٹینس آپریٹ کرتا ہے۔ análise کے دوران کوڈز maiúsculas میں normalizar
  ہوتے ہیں۔
- `jurisdiction_opt_outs` گورننس رجسٹر کو espelho کرتا ہے۔ جب کوئی آپریٹر
  jurisdição فہرست میں ہو، آرکسٹریٹر `transport_policy=direct-only` نافذ
  کرتا ہے اور razão de fallback `compliance_jurisdiction_opt_out` emitir کرتا ہے۔
- `blinded_cid_opt_outs` resumos de manifesto (CIDs cegos, hexadecimal maiúsculo)
  ہے۔ agendamento somente direto de cargas úteis correspondentes
  `compliance_blinded_cid_opt_out` fallback دکھاتی ہیں۔
- `audit_contacts` URI کو ریکارڈ کرتا ہے جنہیں گورننس آپریٹرز کے GAR
  manuais
- `attestations` کمپلائنس کی سائنڈ پیکٹس محفوظ کرتا ہے۔ ہر entrada ایک opcional
  `jurisdiction` (ISO-3166 alfa-2), `document_uri`, `digest_hex` de 64 caracteres,
  carimbo de data / hora de emissão `issued_at_ms`, ou opcional `expires_at_ms` بیان کرتی ہے۔
  یہ artefatos آرکسٹریٹر کی lista de verificação de auditoria میں جاتے ہیں تاکہ ferramentas de governança
  substituições

کمپلائنس بلاک کو معمول کے کنفیگریشن camadas کے ذریعے دیں تاکہ آپریٹرز کو
ڈٹرمنسٹک substitui ملیں۔ آرکسٹریٹر کمپلائنس کو dicas de modo de gravação کے _بعد_
O código é: O SDK `upload-pq-only` é um arquivo de jurisdição e manifesto
opt-outs پھر بھی transporte somente direto پر لے جاتے ہیں اور جب کوئی compatível
provedor نہ ہو تو فوری فیل کرتے ہیں۔

Catálogos de cancelamento canônicos `governance/compliance/soranet_opt_outs.json` میں
موجود ہیں؛ Lançamentos marcados pelo Conselho de Governança کے ذریعے اپڈیٹس شائع کرتی ہے۔
Atestados سمیت مکمل مثال `docs/examples/sorafs_compliance_policy.json` میں
دستیاب ہے, اور آپریشنل پروسیس
[Manual de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md)
میں درج ہے۔

### 1.3 CLI e botões SDK| Bandeira/Campo | Mais |
|--------------|-----|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | اس بات کی حد مقرر کرتا ہے کہ کتنے پرووائیڈر placar فلٹر سے گزر سکیں۔ `None` پر سیٹ کریں تاکہ تمام elegível پرووائیڈرز استعمال ہوں۔ |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | فی-chunk retry حد مقرر کرتا ہے۔ Este é o `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | instantâneos de latência/falha e construtor de placar `telemetry_grace_secs` سے پرانی ٹیلیمیٹری پرووائیڈرز کو inelegível کر دیتی ہے۔ |
| `--scoreboard-out` | حساب شدہ placar (elegível + inelegível) کو پوسٹ رن معائنے کے لیے محفوظ کرتا ہے۔ |
| `--scoreboard-now` | carimbo de data/hora do placar (segundos Unix) substituição کرتا ہے تاکہ fixture captura determinística رہیں۔ |
| `--deny-provider` / gancho de política de pontuação | anúncios ڈیلیٹ کیے بغیر پرووائیڈرز کو determinístico طور پر excluir کرتا ہے۔ فوری بلیک لسٹنگ کے لیے مفید۔ |
| `--boost-provider=name:delta` | گورننس pesos برقرار رکھتے ہوئے پرووائیڈر کے créditos round-robin ponderados ایڈجسٹ کرتا ہے۔ |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | میٹرکس اور logs estruturados کو لیبل دیتا ہے تاکہ ڈیش بورڈز جغرافیہ یا rollout wave کے لحاظ سے pivot کر سکیں۔ |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | اب ڈیفالٹ `soranet-first` ہے کیونکہ ملٹی سورس آرکسٹریٹر linha de base ہے۔ downgrade یا diretiva de conformidade کے لیے `direct-only` استعمال کریں, اور pilotos somente PQ کے لیے `soranet-strict` محفوظ رکھیں؛ substituições de conformidade |

SoraNet-first اب padrão de envio ہے، اور reversões کو متعلقہ bloqueador SNNet کا
حوالہ دینا چاہیے۔ SNNet-4/5/5a/5b/6a/7/8/12/13 کے گریجویٹ ہونے کے بعد گورننس
postura necessária کو مزید سخت کرے گی (`soranet-strict` کی طرف)؛ تب تک صرف
substituições orientadas a incidentes کو `direct-only` ترجیح دینی چاہیے, اور انہیں implementação
log میں ریکارڈ کرنا لازمی ہے۔

Os sinalizadores são `sorafs_cli fetch` e `sorafs_fetch` voltado para o desenvolvedor.
میں `--` اسٹائل sintaxe قبول کرتے ہیں۔ SDKs são construtores digitados por tipo
ایکسپوز کرتے ہیں۔

### 1.4 Guard cache مینجمنٹ

CLI e seletor de proteção SoraNet com fio کرتی ہے تاکہ آپریٹرز SNNet-5 کے مکمل
implementação de transporte سے پہلے relés de entrada کو determinístico انداز میں pino کر سکیں۔
Aqui estão os sinalizadores que estão disponíveis para você:

| Bandeira | مقصد |
|------|------|
| `--guard-directory <PATH>` | ایک JSON فائل کی طرف اشارہ کرتا ہے جو تازہ ترین consenso de retransmissão بیان کرتی ہے (نیچے subconjunto دکھایا گیا ہے)۔ diretório پاس کرنے سے fetch سے پہلے guarda atualização de cache ہوتی ہے۔ |
| `--guard-cache <PATH>` | Norito codificado `GuardSet` محفوظ کرتا ہے۔ بعد کے رنز بغیر نئے diretório کے بھی reutilização de cache کرتے ہیں۔ |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | guardas de entrada کی تعداد (ڈیفالٹ 3) اور janela de retenção (ڈیفالٹ 30 دن) کے لیے substituições opcionais۔ |
| `--guard-cache-key <HEX>` | chave opcional de 32 bytes جو Blake3 MAC کے ذریعے guard cache کو tag کرتی ہے تاکہ reutilizar سے پہلے فائل verificar ہو سکے۔ |

Guardar cargas úteis do diretório ایک esquema compacto استعمال کرتی ہیں:

Sinalizador `--guard-directory` e carga útil `GuardDirectorySnapshotV2` codificada em Norito
espere کرتا ہے۔ instantâneo binário میں شامل ہے:- `version` — esquema do esquema (como `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso جو ہر certificado incorporado سے correspondência کرنا ضروری ہے۔
- `validation_phase` — porta de política de certificado (`1` = assinatura Ed25519 única
  permitir, `2` = preferência por assinaturas duplas, `3` = assinaturas duplas exigem).
- `issuers` — emissores de governança como `fingerprint`, `ed25519_public`, `mldsa65_public`.
  impressões digitais اس طرح بنائے جاتے ہیں:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — Pacotes SRCv2 کی فہرست (saída `RelayCertificateBundleV2::to_cbor()`).
  Descritor de retransmissão de pacote, sinalizadores de capacidade, política ML-KEM, Ed25519/ML-DSA-65
  assinaturas duplas رکھتا ہے۔

CLI ہر pacote کو chaves de emissor declaradas کے خلاف verificar کرتی ہے اور پھر diretório کو
instantâneos لازمی ہیں۔

`--guard-directory` کے ساتھ CLI چلائیں تاکہ تازہ ترین consenso کو موجودہ cache
کے ساتھ mesclar کیا جا سکے۔ protetores fixados do seletor کو برقرار رکھتا ہے جو ابھی
janela de retenção کے اندر اور diretório میں elegível ہیں؛ Os relés expiraram
entradas کو substituir کرتے ہیں۔ کامیاب buscar کے بعد cache atualizado `--guard-cache`
کے caminho پر لکھ دی جاتی ہے، جس سے اگلی سیشنز رہتی ہیں۔ SDKs
comportamento `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
کو کال کر کے اور نتیجہ `GuardSet` کو `SorafsGatewayFetchOptions` میں پاس کر کے
reproduzir کر سکتی ہیں۔

Seletor `ml_kem_public_hex` کو Protetores com capacidade PQ کو ترجیح دینے دیتا ہے جب
Lançamento SNNet-5 alternadores de estágio (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) اب relés clássicos کو خودکار طور پر rebaixar کرتے ہیں: جب PQ
guarda دستیاب ہو تو seletor اضافی pinos clássicos ہٹا دیتا ہے تاکہ اگلی سیشنز
apertos de mão híbridos Resumos CLI/SDK نتیجہ خیز mix کو
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` اور متعلقہ candidato/déficit/oferta delta فیلڈز کے
ذریعے ظاہر کرتے ہیں, جس سے quedas de energia e fallbacks clássicos واضح ہو جاتے ہیں۔

Guarde diretórios اب `certificate_base64` کے ذریعے مکمل SRCv2 bundle embed کر
سکتی ہیں۔ آرکسٹریٹر ہر pacote کو decodificar کرتا ہے, assinaturas Ed25519/ML-DSA کو
revalidar o certificado analisado e o cache de proteção.
ہے۔ جب certificado موجود ہو تو وہ Chaves PQ, preferências de handshake, e ponderação
کا fonte canônica بن جاتا ہے؛ certificados expirados descartados کر دیے جاتے ہیں اور
gerenciamento کے ذریعے propagar ہوتے ہیں اور `telemetry::sorafs.guard` اور
`telemetry::sorafs.circuit` کے ذریعے superfície ہوتے ہیں، جو janela de validade،
suítes de handshake, com assinaturas duplas کے مشاہدے کو ریکارڈ کرتے ہیں۔

Ajudantes CLI سے snapshots کو editores کے ساتھ sincronização رکھیں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

Instantâneo SRCv2 `fetch`
دوسری ٹیموں سے آئے artefatos کے لیے pipeline de validação دوبارہ چلاتا ہے اور
Saída do seletor de proteção CLI/SDK سے match کرنے والا JSON summary emit کرتا ہے۔

### 1.5 Gerenciador de ciclo de vida do circuitoجب diretório de retransmissão e cache de proteção دونوں فراہم ہوں تو آرکسٹریٹر ciclo de vida do circuito
gerente کو فعال کرتا ہے تاکہ ہر fetch سے پہلے Circuitos SoraNet کو pré-construído اور
renovar کیا جائے۔ کنفیگریشن `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) میں دو نئے فیلڈز کے ذریعے ہوتی ہے:

- `relay_directory`: instantâneo do diretório SNNet-3 رکھتا ہے تاکہ saltos intermediários/de saída
  determinístico
- `circuit_manager`: opcional کنفیگریشن (ڈیفالٹ طور پر habilitado) ou circuito TTL
  کو کنٹرول کرتی ہے۔

Norito JSON e `circuit_manager` são os seguintes:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Dados do diretório SDKs
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) کے ذریعے forward کرتی ہیں, اور CLI اسے اس وقت
Um cabo com fio com fio `--guard-directory` sem fio
(`crates/iroha_cli/src/commands/sorafs.rs:365`).

Circuitos gerenciadores کو اس وقت renovar کرتا ہے جب metadados de proteção (endpoint, chave PQ یا
timestamp fixado) بدل جائے یا TTL ختم ہو۔ ہر fetch سے پہلے invocar ہونے والا
ajudante `refresh_circuits` (`crates/sorafs_orchestrator/src/lib.rs:1346`) `CircuitEvent`
لاگز emitir کرتا ہے تاکہ آپریٹرز ciclo de vida فیصلوں کو ٹریس کر سکیں۔ teste de imersão
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) Rotações de guarda میں مستحکم
latência رپورٹ `docs/source/soranet/reports/circuit_stability.md:1` میں
دیکھیں۔

### 1.6 para proxy QUIC

آرکسٹریٹر ایک لوکل QUIC proxy بھی چلا سکتا ہے تاکہ extensões de navegador e SDK
adaptadores کو certificados یا guarda chaves de cache gerenciam نہ کرنا پڑیں۔ loopback de proxy
ایڈریس پر bind ہوتا ہے, conexões QUIC ختم کرتا ہے, اور کلائنٹ کو Norito
manifesto واپس کرتا ہے جو certificado اور chave de cache de proteção opcional بیان کرتا ہے۔
proxy کے emite کردہ eventos de transporte کو `sorafs_orchestrator_transport_events_total`
میں شمار کیا جاتا ہے۔

آرکسٹریٹر JSON میں نئے `local_proxy` بلاک سے proxy enable کریں:

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
```- `bind_addr` کنٹرول کرتا ہے کہ proxy کہاں ouvir کرے (پورٹ `0` سے efêmero
  پورٹ مانگیں)۔
- `telemetry_label` میٹرکس میں propagate ہوتا ہے تاکہ dashboards proxies اور
  buscar sessões
- `guard_cache_key_hex` (اختیاری) proxy کو وہی cache de proteção com chave دکھانے دیتا ہے
  Não CLI/SDKs استعمال کرتے ہیں, تاکہ sincronização de extensões do navegador رہیں۔
- `emit_browser_manifest` اس بات کو toggle کرتا ہے کہ handshake ایسی manifesto
  واپس کرے جسے extensões ذخیرہ/verificar کر سکیں۔
- `proxy_mode` طے کرتا ہے کہ proxy مقامی طور پر bridge کرے (`bridge`) یا صرف
  metadados emitem کرے تاکہ SDKs ou circuitos SoraNet کھولیں (`metadata-only`).
  ڈیفالٹ `bridge` ہے؛ O manifesto do دینا ہو تو `metadata-only` é válido
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs`
  dicas دیتے ہیں تاکہ براؤزر fluxos paralelos بجٹ کر سکے اور proxy کے circuito
  reutilizar رویے کو سمجھ سکے۔
- `car_bridge` (اختیاری) مقامی CAR archive cache کی طرف اشارہ کرتا ہے۔ `extension`
  اس sufixo کو کنٹرول کرتا ہے جو `*.car` غائب ہونے پر لگایا جاتا ہے؛ `allow_zst = true`
  سے `*.car.zst` براہ راست servir ہو سکتا ہے۔
- `kaigi_bridge` (اختیاری) Kaigi rotas کو proxy پر expor کرتا ہے۔ `room_policy`
  بتاتا ہے کہ bridge `public` ہے یا `authenticated` para clientes de navegador
  Etiquetas GAR چن سکیں۔
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only`
  `--local-proxy-norito-spool=PATH` substitui o valor do JSON
  بدلے بغیر modo de tempo de execução یا spool تبدیل ہو سکتے ہیں۔
- `downgrade_remediation` (اختیاری) خودکار gancho de downgrade کو کنفیگر کرتا ہے۔
  Habilitado ہو تو آرکسٹریٹر telemetria de relé میں rajadas de downgrade دیکھتا ہے اور
  `window_secs` é um proxy local `threshold` que é um proxy local `target_mode`
  (ڈیفالٹ `metadata-only`) é um cartão de crédito válido. جب downgrades رک جائیں تو proxy
  `cooldown_secs` کے بعد `resume_mode` پر واپس آتا ہے۔ Matriz `modes` کو específico
  funções de relé

جب modo de ponte proxy میں چلتا ہے تو دو serviços de aplicativos فراہم کرتا ہے:

- **`norito`** — کلائنٹ کا stream target `norito_bridge.spool_dir` کے نسبت resolve
  ہوتا ہے۔ alvos کو higienizar کیا جاتا ہے (travessia e caminhos absolutos نہیں)
  اور اگر فائل میں extensão نہیں تو sufixo configurado لگایا جاتا ہے، پھر payload
  براؤزر کو stream کیا جاتا ہے۔
- **`car`** — destinos de fluxo `car_bridge.cache_dir` کے اندر resolver ہوتے ہیں،
  herança de extensão padrão کرتے ہیں، اور cargas úteis compactadas کو رد کرتے ہیں جب تک
  `allow_zst` não é compatível Pontes کامیاب `STREAM_ACK_OK` کے ساتھ جواب دیتے ہیں
  Como armazenar bytes de arquivo بھیجتے ہیں تاکہ کلائنٹس verificação کو pipeline کر سکیں۔

دونوں صورتوں میں proxy cache-tag HMAC مہیا کرتا ہے (جب handshake de chave de cache de proteção
کے دوران موجود ہو) اور `norito_*` / `car_*` códigos de razão de telemetria ریکارڈ کرتا ہے
تاکہ painéis کامیابی, arquivos ausentes, falhas de higienização e falhas
سمجھ سکیں۔`Orchestrator::local_proxy().await` چلتے ہوئے identificador کو expor کرتا ہے تاکہ
کالرز certificado PEM پڑھ سکیں, manifesto do navegador حاصل کر سکیں, یا ایپلیکیشن
ختم ہونے پر desligamento normal

جب habilitado ہو تو proxy اب **manifest v2** ریکارڈز سرور کرتا ہے۔ Certificado de dinheiro
Para proteger a chave de cache کے علاوہ v2 یہ شامل کرتا ہے:

- `alpn` (`"sorafs-proxy/1"`) e `capabilities` array تاکہ کلائنٹس stream
  پروٹوکول کی تصدیق کر سکیں۔
- ہر handshake کے لیے `session_id` e `cache_tagging` bloco تاکہ por sessão
  guarda afinidades e tags HMAC مشتق کیے جا سکیں۔
- circuito e dicas de seleção de guarda (`circuit`, `guard_selection`, `route_hints`)
  تاکہ fluxos de integração do navegador کھلنے سے پہلے UI mais rica دکھا سکیں۔
- `telemetry_v2` مقامی instrumentação کے لیے botões de amostragem / privacidade کے ساتھ۔
- ہر `STREAM_ACK_OK` میں `cache_tag_hex` شامل ہوتا ہے۔ کلائنٹس اسے
  Cabeçalho `x-sorafs-cache-tag` میں espelho کرتے ہیں تاکہ seleções de guarda em cache
  resto میں criptografado رہیں۔

O subconjunto v1 é definido como um subconjunto v1

## 2. Semântica de falha

آرکسٹریٹر ایک بائٹ منتقل ہونے سے پہلے سخت capacidade e verificações de orçamento نافذ کرتا
ہے۔ O que você precisa fazer:

1. **Falhas de elegibilidade (pré-voo).** capacidade de alcance expirou
   anúncios, telemetria obsoleta e provedores کو artefato de placar میں log کیا
   جاتا ہے اور agendamento سے نکال دیا جاتا ہے۔ Resumos CLI `ineligible_providers`
   matriz میں motivos بھر دیتی ہیں تاکہ raspagem de logs de operadores کیے بغیر governança
   deriva
2. **Esgotamento do tempo de execução.** Provedor مسلسل falhas کو faixa کرتا ہے۔ sim
   `provider_failure_threshold` پہنچ جائے تو provedor کو سیشن کے باقی حصے کے لیے
   `disabled` کر دیا جاتا ہے۔ Fornecer provedores `disabled` ہو جائیں تو آرکسٹریٹر
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` e cartão de crédito
3. **Abortos determinísticos.** سخت حدود erros estruturados کے طور پر ظاہر ہوتی ہیں:
   - `MultiSourceError::NoCompatibleProviders` — manifesto ایسی chunk span یا
     alinhamento مانگتا ہے جسے باقی fornecedores honram نہیں کر سکتے۔
   - `MultiSourceError::ExhaustedRetries` — orçamento de repetição por bloco ختم ہو گیا۔
   - `MultiSourceError::ObserverFailed` — observadores downstream (ganchos de streaming)
     نے pedaço verificado رد کر دیا۔

ہر erro que ofende o índice de pedaços اور جہاں ممکن ہو motivo da falha do provedor final
کے ساتھ آتا ہے۔ انہیں bloqueadores de liberação سمجھیں — انہی entradas کے ساتھ novas tentativas
تب تک falha دہراتی رہیں گے جب تک anúncio, telemetria یا provedor de saúde نہ بدلے۔

### 2.1 Persistência do placar

جب `persist_path` کنفیگر ہو تو آرکسٹریٹر ہر رن کے بعد placar final لکھتا ہے۔
JSON pode ser definido como:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (اس رن کے لیے peso normalizado atribuído).
- Metadados `provider` (identificador, endpoints, orçamento de simultaneidade).

Instantâneos do placar کو artefatos de liberação کے ساتھ arquivo کریں تاکہ lista negra
O lançamento é capaz de auditar رہیں۔

## 3. Telemetria e depuração

### 3.1 Métricas Prometheus

آرکسٹریٹر `iroha_telemetry` کے ذریعے درج ذیل میٹرکس emite کرتا ہے:| Métrica | Etiquetas | Descrição |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | buscas orquestradas em voo کا medidor۔ |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | latência de busca ponta a ponta ریکارڈ کرنے والا histograma۔ |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | falhas de terminal کا contador (novas tentativas esgotadas, nenhum provedor, falha do observador)۔ |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | فی-novas tentativas do provedor کا contador۔ |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | falhas do provedor em nível de sessão کا contador جو desativação تک لے جائیں۔ |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | política de anonimato فیصلوں کی contagem (meet vs brownout) estágio اور motivo de fallback کے مطابق۔ |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | منتخب Conjunto SoraNet میں Compartilhamento de relé PQ کا histograma۔ |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | instantâneo do placar میں Relações de fornecimento de relé PQ کا histograma۔ |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | déficit de política (meta e participação PQ real) ou histograma |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | ہر سیشن میں compartilhamento de relé clássico کا histograma۔ |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | فی سیشن منتخب relés clássicos کی تعداد کا histograma۔ |

Botões de produção آن کرنے سے پہلے métricas کو painéis de preparação
تجویز کردہ layout Plano de observabilidade SF-6 کو siga کرتا ہے:

1. **Buscas ativas** — اگر conclusões do medidor کے بغیر بڑھے تو الرٹ۔
2. **Proporção de novas tentativas** — جب Contadores `retry` تاریخی linhas de base سے بڑھیں تو warning۔
3. **Falhas do provedor** — 15 منٹ میں کسی provedor کے `session_failure > 0` پر
   alertas de pager۔

### 3.2 Destinos de log estruturados

Definir metas determinísticas e eventos estruturados.

- `telemetry::sorafs.fetch.lifecycle` — `start` e `complete` marcadores de ciclo de vida
  کے ساتھ contagens de pedaços, novas tentativas, e duração total۔
- `telemetry::sorafs.fetch.retry` — repetir eventos (`provider`, `reason`, `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — erros repetidos کی وجہ سے desativados
  provedores۔
- `telemetry::sorafs.fetch.error` — `reason` اور metadados de provedor opcionais کے
  ساتھ falhas de terminal۔

Os fluxos são o pipeline de log Norito e o encaminhamento de resposta a incidentes
کو ایک ہی fonte da verdade ملے۔ eventos de ciclo de vida PQ/mix clássico کو
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
Contadores کے ذریعے ظاہر کرتے ہیں, جس سے dashboards کو métricas scrape
Fio de fio کیے بغیر کرنا آسان ہوتا ہے۔ Lançamentos do GA para eventos de ciclo de vida/repetição
Nível de log `info` رکھیں e erros de terminal کے لیے `warn` استعمال کریں۔

### 3.3 Resumos JSON

`sorafs_cli fetch` اور Rust SDK دونوں resumo estruturado واپس کرتے ہیں، جس میں:

- `provider_reports` com contagens de sucesso/falha e desabilitação do provedor ہونے کی حالت۔
- `chunk_receipts` جو دکھاتے ہیں کہ کس provedor نے کون سا pedaço پورا کیا۔
- Matrizes `retry_stats` e `ineligible_providers`۔مشکوک provedores کو depuração کرتے وقت arquivo de arquivo de resumo کریں - recibos اوپر
دیے گئے metadados de log سے براہ راست جڑتے ہیں۔

## 4. Lista de verificação operacional

1. **CI میں کنفیگریشن estágio کریں۔** `sorafs_fetch` کو configuração de destino کے ساتھ چلائیں،
   visualização de elegibilidade کے لیے `--scoreboard-out` دیں، اور پچھلے release سے diff کریں۔
   کوئی غیر متوقع promoção de provedor inelegível روک دیتا ہے۔
2. **Telemetria ویلیڈیٹ کریں۔** buscas de múltiplas fontes habilitam کرنے سے پہلے یقینی
   Implementar métricas de implantação `sorafs.fetch.*` e exportação de logs estruturados
   ہے۔ métricas کی عدم موجودگی عموماً اس بات کا اشارہ ہے کہ fachada do orquestrador
   invocar نہیں ہوئی۔
3. **Substituições دستاویزی بنائیں۔** ہنگامی `--deny-provider` یا `--boost-provider`
   O JSON e o JSON (invocação CLI) e o log de alterações e o commit reversões کو
   substituir reverter کرنا اور نیا scoreboard snapshot capture کرنا چاہیے۔
4. **Testes de fumaça دوبارہ چلائیں۔** repetir orçamentos یا limites de provedor بدلنے کے بعد
   luminária canônica (`fixtures/sorafs_manifest/ci_sample/`) para buscar fetch کریں
   اور verificar کریں کہ pedaços de recibos determinísticos رہیں۔

اوپر دیے گئے مراحل آرکسٹریٹر کے رویے کو lançamentos graduais میں reproduzíveis رکھتے
ہیں اور resposta a incidentes کے لیے ضروری telemetria فراہم کرتے ہیں۔

### 4.1 Substituições de políticas

Estágio de transporte/anonimato dos operadores کو configuração base تبدیل کیے بغیر pin
Cartão de crédito `policy_override.transport_policy`
`policy_override.anonymity_policy` کو اپنے `orchestrator` JSON میں سیٹ کریں (یا
`sorafs_cli fetch` ou `--transport-policy-override=` / `--anonymity-policy-override=`
پاس کریں)۔ جب substituir موجود ہو تو آرکسٹریٹر substituto de brownout usual کو pular
کرتا ہے: اگر مطلوبہ PQ tier فراہم نہ ہو تو fetch `no providers` کے ساتھ فیل
ہوتا ہے بجائے خاموشی سے downgrade ہونے کے۔ comportamento padrão
Você também pode substituir campos