---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر anúncios e اور شیڈولنگ

یہ صفحہ درج ذیل دستاویز میں A especificação canônica é a seguinte:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Esquemas Norito e registros de alterações portal
Você pode usar o SDK do SDK e executar os runbooks do SoraFS com runbooks رکھتی ہے۔

## Esquema Norito میں اضافے

### Capacidade de alcance (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – O número de bits é menor que o tamanho do span (bytes), `>= 1`.
- `min_granularity` – procure ریزولوشن, `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – ایک درخواست میں غیر مسلسل compensações کی اجازت دیتا ہے۔
- `requires_alignment` – اگر true ہو تو offsets کو `min_granularity` کے مطابق alinhar ہونا لازم ہے۔
- `supports_merkle_proof` – Testemunha PoR

Codificação canônica `ProviderCapabilityRangeV1::to_bytes` / `from_bytes` نافذ کرتے ہیں
تاکہ cargas úteis de fofoca determinísticas رہیں۔

### `StreamBudgetV1`
- Números: `max_in_flight`, `max_bytes_per_sec`, اختیاری `burst_bytes`.
- Regras de validação (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہیے۔

###`TransportHintV1`
- Nomes: `protocol: TransportProtocol`, `priority: u8` (0-15 pontos
  `TransportHintV1::validate` não é válido).
-Protocolos principais: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- فی provedor ڈپلیکیٹ entradas de protocolo مسترد کی جاتی ہیں۔

### `ProviderAdvertBodyV1` میں اضافے
- Versão `stream_budget: Option<StreamBudgetV1>`.
- Versão `transport_hints: Option<Vec<TransportHintV1>>`.
- دونوں فیلڈز اب `ProviderAdmissionProposalV1`, envelopes de governança, dispositivos CLI, e JSON telemétrico میں بہاؤ رکھتے ہیں۔

## Validação e vinculação de governança

`ProviderAdvertBodyV1::validate` ou `ProviderAdmissionProposalV1::validate`
خراب metadados کو مسترد کرتے ہیں:

- Capacidades de alcance کو decodificação ہونا چاہیے اور limites de amplitude/granularidade پوری کرنی چاہئیں۔
- Transmitir orçamentos / dicas de transporte کے لیے `CapabilityType::ChunkRangeFetch` TLV اور lista de dicas não vazia لازم ہے۔
- ڈپلیکیٹ protocolos de transporte اور غیر درست prioridades fofocas سے پہلے erros de validação پیدا کرتے ہیں۔
- Envelopes de admissão `compare_core_fields` کے ذریعے proposta/anúncios کے metadados de intervalo کو comparar کرتے ہیں تاکہ incompatibilidade de cargas úteis de fofoca جلدی مسترد ہوں۔

Cobertura de regressão یہاں موجود ہے:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Ferramentas e acessórios

- Cargas úteis de anúncio do provedor `range_capability`, `stream_budget`, اور `transport_hints` شامل ہونا لازم ہے۔
  Respostas `/v2/sorafs/providers` e luminárias de admissão کے ذریعے validar کریں؛ Resumos JSON میں capacidade analisada, orçamento de fluxo, اور matrizes de dicas شامل ہونے چاہئیں تاکہ ingestão de telemetria ہو سکے۔
- `cargo xtask sorafs-admission-fixtures` Artefatos JSON میں orçamentos de fluxo e dicas de transporte دکھاتا ہے تاکہ painéis apresentam faixa de adoção کر سکیں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت fixtures اب شامل کرتے ہیں:
  - anúncios canônicos de várias fontes,
  - `multi_fetch_plan.json` تاکہ Conjuntos SDK determinísticos de repetição do plano de busca multi-peer کر سکیں۔

## Orquestrador Torii Orquestrado- Torii `/v2/sorafs/providers` metadados de capacidade de intervalo analisado
  provedores جب نئی metadados چھوڑ دیں تو avisos de downgrade چلتے ہیں, اور terminais de intervalo de gateway براہ راست clientes کے لیے یہی restrições نافذ کرتے ہیں۔
- Orquestrador multifonte (`sorafs_car::multi_fetch`) اب limites de intervalo, alinhamento de capacidade, اور orçamentos de fluxo کو atribuição de trabalho کے دوران impor کرتا ہے۔ Testes de unidade são pedaços muito grandes, busca esparsa e cenários de limitação.
- Sinais de downgrade `sorafs_car::multi_fetch` (falhas de alinhamento, solicitações limitadas) fluxo کرتا ہے تاکہ operadores دیکھ سکیں کہ پلانگ کے دوران مخصوص provedores کیوں pular ہوئے۔

## Referência de telemetria

Torii کی instrumentação de busca de faixa **SoraFS Observabilidade de busca** Painel Grafana
(`dashboards/grafana/sorafs_fetch_observability.json`) اور متعلقہ regras de alerta
(`dashboards/alerts/sorafs_fetch_rules.yml`) کو feed کرتی ہے۔

| Métrica | Tipo | Etiquetas | Descrição |
|--------|------|--------|------------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Recursos de capacidade de alcance anunciam provedores de کرنے والے۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Política کے مطابق tentativas de busca de faixa limitada۔ |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | — | Orçamento de simultaneidade compartilhada استعمال کرنے والی fluxos protegidos ativos۔ |

Exemplo de trechos do PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Aplicação de cota کی تصدیق کے لیے contador de otimização استعمال کریں اس سے پہلے کہ padrões de orquestrador multi-fonte فعال کریں, اور جب simultaneidade آپ کے frota کے fluxo máximo de orçamento کے قریب ہو تو alerta کریں۔