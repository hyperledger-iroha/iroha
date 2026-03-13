---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Lançamento de plano e provador de anúncio de suporte SoraFS"
---

> Adaptação de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Planeje o lançamento e forneça os fornecedores de anúncios SoraFS

Este plano é coordenado por meio de anúncios permissivos provados por anúncios
управляемой поверхности `ProviderAdvertV1`, necessário para fontes múltiplas
pedaços. Sobre a concentração de três resultados:

- **Руководство оператора.** Пошаговые действия, которые провайдеры хранения должны
  выполнить до включения каждого portão.
- **Покрытие телеметрией.** Дашборды e alertas, которые Observabilidade e Ops используют,
  Isso pode ser feito para que todos os anúncios sejam exibidos.
  SDK e ferramentas podem ser aplicativos de planejamento.

Lançamento do modelo SF-2b/2c em
[roteiro de migração SoraFS](./migration-roadmap) e предполагает, esta política é enviada para
[política de admissão do provedor](./provider-admission-policy) уже действует.

##Taймлайн фаз

| Faz | Okno (цель) | Sugestão | Operadores de destino | Foco de foco |
|-------|-----------------|-----------|------------------|-------------------|

## Verifique o operador

1. **Inventarizar anúncios.** Перечислите каждый опубликованный anúncio e fechá-lo:
   - Путь к envelope governamental (`defaults/nexus/sorafs_admission/...` или produção-эквивалент).
   - Anúncio `profile_id` e `profile_aliases`.
   - Capacidades de especificação (referidas no mínimo `torii_gateway` e `chunk_range_fetch`).
   - Sinalizador `allow_unknown_capabilities` (transferido por TLV reservado pelo fornecedor).
2. **Ferramentas do fornecedor de segurança.**
   - Selecione a carga útil do anúncio do editor, colocado em:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com `max_span`
     - `allow_unknown_capabilities=<true|false>` при наличии GREASE TLV
   - Verifique a configuração `/v2/sorafs/providers` e `sorafs_fetch`; предупреждения o
     capacidades неизвестных нужно триажить.
3. **Prontidão de múltiplas fontes.**
   - Verifique `sorafs_fetch` com `--provider-advert=<path>`; CLI теперь падает,
     когда отсутствует `chunk_range_fetch`, e печатает предупреждения о
     capacidades numéricas de programação. Verifique o JSON-отчет e
     архивируйте его с registros de operações.
4. **Produto de execução.**
   - Compre envelopes `ProviderAdmissionRenewalV1` no mínimo até 30 dias até
     aplicação de gateway (R2). Продления должны сохранять канонический alça e
     capacidades adicionais; менять следует только stake, endpoints ou metadados.
5. **Comunicação com comandos de comando.**
   - Владельцы SDK должны выпускать версии, que permite a operação de avisos
     por anúncios abertos.
   - DevRel анонсирует каждую фазу; acesse as opções de painéis e lógica
     por favor não.
6. **Configurar painéis e alertas.**
   - Importar exportação Grafana e разместите его em **SoraFS / Provedor
     Implementação** com UID `sorafs-provider-admission`.
   - Убедитесь, quais regras de alerta estão disponíveis no canal oficial
     `sorafs-advert-rollout` em preparação e produção.

## Telemetria e dados

A métrica correta está disponível no `iroha_telemetry`:- `torii_sorafs_admission_total{result,reason}` — счетчики принятия, отклонения
  e avisos. Selecione `missing_envelope`, `unknown_capability`, `stale` e
  `policy_violation`.

Exportação Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe o arquivo para o repositório de arquivos padrão (`observability/dashboards`) e
обновите только UID datasource antes de ser publicado.

Adicionar pacote publicado no pacote Grafana **SoraFS / Implementação de provedor** com
UID padrão `sorafs-provider-admission`. Regras de alerta
`sorafs-admission-warn` (aviso) e `sorafs-admission-reject` (crítico)
преднастроены на política уведомлений `sorafs-advert-rollout`; entrar em contato
Coloque o ponto de especificação da especificação no lugar do arquivo JSON.

Painel de recomendação Grafana:

| Painel | Consulta | Notas |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pilha para visualizar aceitar vs avisar vs rejeitar. Alerta por avisar > 0,05 * total (aviso) ou rejeitar > 0 (crítico). |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporal definida, питающая por pager (taxa de aviso de 5% na escola 15 minutos de outubro). |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Para triagem no runbook; contratar soluções para mitigação. |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Указывает на provedores, prazo de atualização do пропустивших; сверяйте слогами cache de descoberta. |

CLI артефакты для ручных дашбордов:

- `sorafs_fetch --provider-metrics-out` пишет счетчики `failures`, `successes` e
  `disabled` para o provedor. Importar para painéis ad-hoc, itens
  мониторить orquestrador de simulação перед переключением fornecedores de produção.
- Por `chunk_retry_rate` e `provider_failure_rate` em formato JSON
  usar estrangulamento ou sintomas de cargas obsoletas, isso é o que acontece
  admissão отклонениям.

### Раскладка Grafana дашборда

Observabilidade placa публикует отдельный — **SoraFS Admissão do Provedor
Lançamento** (`sorafs-provider-admission`) — в **SoraFS / Lançamento do provedor**
quais são os IDs do painel canônicos:

- Painel 1 — *Taxa de resultado de admissão* (área empilhada, единица "ops/min").
- Painel 2 — *Taxa de advertência* (série única), выражение
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 — *Motivos de rejeição* (série temporal, сгруппированные по `reason`), сортировка по
  `rate(...[5m])`.
- Painel 4 — *Atualizar dívida* (stat), отражает запрос из таблицы выше и
  prazo de atualização atualizado do livro-razão de migração.

Скопируйте (ou создайте) Esqueleto JSON em repositórios de informações
`observability/dashboards/sorafs_provider_admission.json`, para obter o valor desejado
Fonte de dados UID; IDs de painel e regras de alerta são usadas em runbooks não, mas não
перенумеровывайте их без обновления этой документации.

Para usar o repositório, você precisa definir a definição do painel de referência em
`docs/source/grafana_sorafs_admission.json`; скопируйте его вашу Grafana pacote,
Não há nenhuma variante inicial para teste local.

### Alerta de alerta PrometheusAdicione o grupo de trabalho em
`observability/prometheus/sorafs_admission.rules.yml` (configure o arquivo, exceto este
первая группа правил SoraFS) e coloque-o na configuração Prometheus.
Coloque `<pagerduty>` em uma etiqueta de roteamento real para suas rotas de plantão.

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

Запустите `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de abrir a configuração, isso é usado, esse processo de sintaxe
`promtool check rules`.

## Matriz de Sustentabilidade

| Anúncio de Haracterística | R0 | R1 | R2 | R3 |
|-----|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, aliases canônicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Capacidade de rede `chunk_range_fetch` | ⚠️ Avisar (ingestão + telemetria) | ⚠️Avisar | ❌ Rejeitar (`reason="missing_capability"`) | ❌ Rejeitar |
| Capacidade de capacidade TLV sem `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rejeitar | ❌ Rejeitar |
| Teste `refresh_deadline` | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar |
| `signature_strict=false` (dispositivos de diagnóstico) | ✅ (desenvolvimento de только) | ⚠️Avisar | ⚠️Avisar | ❌ Rejeitar |

Qual horário será exibido no UTC. A aplicação de dados foi executada no registro de migração e não
будут изменены без голосования conselho; любые изменения требуют обновления этого
fatura e razão em PR.

> **Lembre-se da realidade:** R1 está na série `result="warn"` em
> `torii_sorafs_admission_total`. Ingestão de pacote Torii, novo rótulo,
> отслеживается вместе с задачами телеметрии SF-2; fazer isso para ser usado

## Eventos de comunicação e operação

- **Еженедельная рассылка статуса.** DevRel рассылает краткое резюме métrica
  admissão, avisos técnicos e prazos de entrega.
- **Resposta a incidentes.** Если срабатывают alertas `reject`, alertas de plantão:
  1. Resolva o problema do anúncio da descoberta Torii (`/v2/sorafs/providers`).
  2. Valide o anúncio no pipeline do provedor e crie-o
     `/v2/sorafs/providers`, este é o valor de referência.
  3. Organize a rotação do anúncio antes do prazo de atualização.
- **Exibição de recursos.** Capacidade de configuração de recursos de esquema em R1/R2, exceto
  a implementação do комитет não foi executada; GREASE испытания проводите только в еженедельное
  окно обслуживания и фиксируйте в migração ledger.

## Ссылки

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)