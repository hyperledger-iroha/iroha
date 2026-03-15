---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "SoraFS پرووائیڈر anúncio رول آؤٹ اور مطابقتی پلان"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے ماخوذ۔

# SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان

یہ پلان anúncios de provedores permissivos سے مکمل طور پر governados `ProviderAdvertV1`
superfície پر cut-over کو کوآرڈی نیٹ کرتا ہے جو recuperação de pedaços de múltiplas fontes کے لیے
ضروری ہے۔ Quais são as entregas do produto:

- **Guia do operador.** وہ قدم بہ قدم اقدامات جو provedores de armazenamento کو ہر portão فلپ ہونے
  سے پہلے مکمل کرنا ہیں۔
- **Cobertura de telemetria.** painéis e alertas Observabilidade e operações
  کرتے ہیں تاکہ نیٹ ورک صرف anúncios compatíveis قبول کرے۔
  SDKs e ferramentas ٹیمیں اپنی lançamentos پلان کر سکیں۔

یہ implementação [Roteiro de migração SoraFS](./migration-roadmap) کی Marcos SF-2b/2c
کے ساتھ alinhar ہے اور فرض کرتا ہے کہ [política de admissão do provedor](./provider-admission-policy)
پہلے سے نافذ ہے۔

## Cronograma da Fase

| Fase | Janela (alvo) | Comportamento | Ações do Operador | Foco na observabilidade |
|-------|-----------------|-----------|------------------|-------------------|

## Lista de verificação do operador

1. **Anúncios de inventário.** ہر anúncio publicado کی فہرست بنائیں اور ریکارڈ کریں:
   - Caminho do envelope governante (`defaults/nexus/sorafs_admission/...` یا equivalente de produção).
   - anúncio `profile_id` e `profile_aliases`.
   - lista de capacidades (کم از کم `torii_gateway` e `chunk_range_fetch`).
   - Sinalizador `allow_unknown_capabilities` (جب TLVs reservados pelo fornecedor ہوں تو ضروری ہے)۔
2. **Regenerar com ferramentas do fornecedor.**
   - اپنے fornecedor editor de anúncios سے carga útil دوبارہ بنائیں، اور یقینی بنائیں:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` e `max_span`
     - GREASE TLVs کی صورت میں `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` e `sorafs_fetch` سے validar کریں؛ desconhecido
     capacidades کی avisos کو triagem کریں۔
3. **Valide a prontidão de múltiplas fontes.**
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں؛ Sobre `chunk_range_fetch`
     نہ ہونے پر CLI falha کرتا ہے اور recursos desconhecidos ignorados کے لیے avisos دیتا ہے۔
     Relatório JSON محفوظ کریں اور registros de operações کے ساتھ arquivo کریں۔
4. **Renovações de etapas.**
   - imposição de gateway (R2) سے کم از کم 30 دن پہلے `ProviderAdmissionRenewalV1`
     envelopes renovações میں identificador canônico اور conjunto de capacidade برقرار
     رہنا چاہئے؛ صرف stake, endpoints e metadados بدلے۔
5. **Comunique-se com equipes dependentes.**
   - Proprietários de SDK کو ایسی lançamentos دینا ہوں گی جو anúncios rejeitados ہونے پر operadores کو avisos دکھائیں۔
   - Anúncio de transição de fase DevRel ہر کرے؛ links do painel e lógica de limite
6. **Instale painéis e alertas.**
   - Exportação Grafana امپورٹ کریں اور **SoraFS / Implementação do provedor** کے تحت رکھیں, UID
     `sorafs-provider-admission` رکھیں۔
   - یقینی بنائیں کہ preparação de regras de alerta e produção میں compartilhada
     Canal de notificação `sorafs-advert-rollout`

## Telemetria e painéis

یہ métricas پہلے ہی `iroha_telemetry` کے ذریعے دستیاب ہیں:- `torii_sorafs_admission_total{result,reason}` — aceito, rejeitado ou aviso
  resultados razões: `missing_envelope`, `unknown_capability`, `stale`
  O `policy_violation` é compatível

Exportação Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
فائل کو repositório de painéis compartilhados (`observability/dashboards`) میں import کریں اور
شائع کرنے سے پہلے صرف datasource UID اپڈیٹ کریں۔

Pasta Grafana **SoraFS / Implementação do provedor ** com UID estável
`sorafs-provider-admission` کے ساتھ publicar ہوتا ہے۔ regras de alerta
`sorafs-admission-warn` (aviso) ou `sorafs-admission-reject` (crítico)
پہلے سے `sorafs-advert-rollout` política de notificação استعمال کرنے کے لیے configurada ہیں؛
Lista de destinos بدلے تو painel JSON کو editar کرنے کے بجائے ponto de contato
atualizar کریں۔

Painéis Grafana recomendados:

| Painel | Consulta | Notas |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pilha - aceitar vs avisar vs rejeitar avisar > 0,05 * total (aviso) یا rejeitar > 0 (crítico) پر alerta۔ |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporal de linha única جو limite de pager کو feed کرتی ہے (15 meses de taxa de aviso de 5%)۔ |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | triagem de runbook کے لیے؛ etapas de mitigação |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | atualizar prazo perdido کرنے والے provedores کو ظاہر کرتا ہے؛ logs de cache de descoberta کے ساتھ referência cruzada کریں۔ |

Painéis manuais com artefatos CLI:

- `sorafs_fetch --provider-metrics-out` ہر provedor کے لیے `failures`, `successes`,
  Contadores `disabled` لکھتا ہے۔ simulações do orquestrador کو monitor کرنے کے لیے
  painéis ad-hoc میں importação کریں۔
- Relatório JSON کے `chunk_retry_rate` e `provider_failure_rate` otimização de campos یا
  sintomas de carga útil obsoleta دکھاتے ہیں جو اکثر admissão rejeitada

### Layout do painel Grafana

Observabilidade ایک placa dedicada - **SoraFS Admissão do Provedor
Implementação** (`sorafs-provider-admission`) — **SoraFS / Implementação do provedor** کے تحت
publicar کرتا ہے، اور اس کے IDs de painel canônico یہ ہیں:

- Painel 1 — *Taxa de resultado de admissão* (área empilhada, یونٹ "ops/min").
- Painel 2 — *Taxa de advertência* (série única)، اظہار
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 — *Motivos de rejeição* (`reason` کے حساب سے série temporal), `rate(...[5m])`
  کے مطابق classificar کی گئی۔
- Painel 4 — *Atualizar dívida* (estatísticas), اوپر والی consulta de tabela کو espelho کرتا ہے اور
  registro de migração سے حاصل کردہ prazos de atualização de anúncios کے ساتھ anotado ہے۔

Esqueleto JSON e repositório de painéis de infraestrutura `observability/dashboards/sorafs_provider_admission.json`
پر copy یا create کریں, پھر صرف datasource UID اپڈیٹ کریں؛ IDs de painel e regras de alerta
نیچے runbooks میں referenciados ہیں، اس لیے انہیں renumerar کرنے سے پہلے docs اپڈیٹ کریں۔

سہولت کے لیے ریپو `docs/source/grafana_sorafs_admission.json` Painel de referência de referência
definição لوکل teste کے لیے اسے اپنے Grafana pasta میں copiar کر لیں۔

### Regras de alerta PrometheusSelecione o grupo de regras `observability/prometheus/sorafs_admission.rules.yml`
میں شامل کریں (اگر یہ پہلا SoraFS grupo de regras ہے تو فائل بنائیں) اور Prometheus
configuração سے inclui کریں۔ `<pagerduty>` کو اپنے rótulo de roteamento de chamada سے بدلیں۔

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
چلا کر تصدیق کریں کہ sintaxe `promtool check rules` پاس کر رہا ہے۔

## Comunicação e tratamento de incidentes

- ** Mailer de status semanal. ** Métricas de admissão DevRel, avisos pendentes e prazos کا خلاصہ شیئر کرتا ہے۔
- **Resposta a incidentes.** Alertas `reject` فائر ہوں تو de plantão انجینئر:
  1. Descoberta Torii (`/v2/sorafs/providers`) ou busca de anúncio ofensivo کریں۔
  2. pipeline do provedor میں validação de anúncio دوبارہ چلائیں اور `/v2/sorafs/providers` سے comparar کریں تاکہ reproduzir erro ہو۔
  3. provedor کے ساتھ coordenada کریں تاکہ اگلی prazo de atualização سے پہلے rotação do anúncio ہو جائے۔
- **Alteração congela.** R1/R2 کے دوران esquema de capacidade میں تبدیلیاں نہ کریں جب تک rollout کمیٹی منظوری نہ دے؛ Testes GREASE کو ہفتہ وار janela de manutenção میں agendamento کریں اور registro de migração میں log کریں۔

## Referências

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)