---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: Runbook para lançamento de vários recursos e testes de verificação confiáveis
sidebar_label: implementação de vários runbooks
description: Lista de operação para implementação de múltiplas operações e demonstração de configuração чёрный список.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/runbooks/multi_source_rollout.md`. Faça cópias de sincronização, mas não será necessário instalá-lo.
:::

## Abençoado

Este runbook fornece SRE e muitos desenvolvedores fornecem seu processo crítico:

1. Abra o controle de vários controles do orquestrador.
2. Faça uma verificação de segurança ou verifique a prioridade de solução de problemas sem a tecnologia de desinstalação sessão.

Por favor, esta seção de orquestração é postada na rede SF-6, usando a configuração (`sorafs_orchestrator`, API de gateway configurada чанков, экспортеры телеметрии).

> **См. Então:** [Runbook no эксплуатации оркестратора](./orchestrator-ops.md) pode ser usado para definir o desempenho do programa (placar de pontuação, переключатели поэтапного implementação, reversão). Use a solução sempre que você precisar.

## 1. Validação prévia

1. **Confira a governança financeira.**
   - Seus candidatos-prováveis ​​dolжны публиковать конверты `ProviderAdvertV1` com carga útil'ами диапазонных возможностей и бюджетами потоков. Verifique o `/v2/sorafs/providers` e desligue-o com um fio limpo.
   - Снимки телеметрии с метриками латентности/сбоев должны быть не старше 15 minutos antes de каждым canary-proгоном.
2. **Configuração de configuração.**
   - Сохраните JSON-конфиг оркестратора no local desejado `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Ative o lançamento do limite JSON (`max_providers`, que é retornado). Use o que você precisa para a preparação/produção, com uma taxa de execução mínima.
3. **Equipamentos canônicos.**
   - Abra o manifesto/token e verifique a busca:

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

     A configuração inicial do gerenciador de carga útil de resumo (hex) e base64-кодированные stream-токены para o provedor, участвующего no canário.
   - Registre `artifacts/canary.scoreboard.json` com o código do produto. A nova taxa de amostragem é maior ou menor que 10% da taxa de retorno.
4. **Proveritь, что телеметрия подключена.**
   - Abra o transporte Grafana em `docs/examples/sorafs_fetch_dashboard.json`. Verifique se esta métrica `sorafs_orchestrator_*` é colocada no teste durante o processo de produção.

## 2. Teste padrão de segurança em uma página específica

Certifique-se de que este procedimento seja feito, o que o provedor deve fazer é não produzir nenhum problema проверки соответствия.1. **Faça o download.**
   - Selecione o recurso fetch-свод (exibido `--json-out`). Запишите индексы сбойных чанков, алиасы провайдеров e несовпадения digest.
   - Сохраните релевантные фрагменты логов из таргетов `telemetry::sorafs.fetch.*`.
2. **Substituição desnecessária.**
   - Отметьте провайдера как penalized в снимке телеметрии, передаваемом оркестратору (установите `penalty=true` ou ограничьте `token_health` para `0`). O placar do placar será exibido automaticamente.
   - Para a fumaça ad-hoc-тестов передайте `--deny-provider gw-alpha` em `sorafs_cli fetch`, чтобы отработать путь отказа без ожидания распространения телеметрии.
   - Переразверните обновленный pacет телеметрии/конфигурации в затронутой среде (preparação → canário → produção). Verifique as informações do incidente no diário.
3. **Substituição de substituição.**
   - Повторите buscar fixação канонического. Claro, este placar pode ser testado como novo com o fornecedor `policy_denied`.
   - Verifique `sorafs_orchestrator_provider_failures_total`, чтобы убедиться, что счетчик перестал расти para отклоненного провайдера.
4. **Explicar um bloco duplo.**
   - Se você fornecer um orçamento >24 horas, envie um bilhete de governança para uma rota ou um anúncio. Para criar uma lista de negação e abrir um telefone, isso não será exibido no placar.
5. **Protocol otката.**
   - Чтобы вернуть провайдера, удалите его из deny-lista, переразверните и снимите новый placar instantâneo. Verifique a informação do incidente postmortem.

## 3. Implementação do plano de expansão

| Faz | Baixar | Sinais de trânsito | Critérios Go/No-Go |
|------|-------|-------------------|-------------------|
| **Laboratório** | Vidro integrado integrado | Clique em CLI fetch na imagem de payloads | Quanto mais você produz, a probabilidade de retorno é de 0, com retorno < 5%. |
| **Encenação** | Plano de controle de preparação | Painel Grafana подключен; alertas de aviso no programa somente aviso | `sorafs_orchestrator_active_fetches` não é compatível com nenhum programa de teste; não há alertas `warn/critical`. |
| **Canário** | ≤10% de tráfego | Pager não monitorado em tempo real | Доля ретраев <10%, провайдерские сбои ограничены известными шумными пирами, гистограмма латентности совпадает со linha de base do estadiamento ±20%. |
| **Envio gratuito** | Implementação 100% | Atividades de pager público | Não use `NoHealthyProviders` por 24 horas, para retornar à posição estável, painel SLA no dia anterior. |

Para fazer isso:

1. Abra o organizador JSON com o planejamento `max_providers` e execute-o.
2. Abra `sorafs_cli fetch` ou teste SDK integrado no dispositivo de fixação canônico e represente o manifesto do site.
3. Сохраните placar de artefatos + resumo e приложите их к записи о релизе.
4. Verifique se o telefone está funcionando corretamente durante a operação correta.

## 4. Ganchos de segurança e incidentes- **Métricas:** Observe que o Alertmanager monitora `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Резкий всплеск обычно означает деградацию провайдера под нагрузкой.
- **Leitura:** Abra os alvos `telemetry::sorafs.fetch.*` no log de registro original. Se você instalar uma chave de segurança para `event=complete status=failed`, este será o teste.
- **Placares:** Сохраняйте каждый placar de artefatos em долговременное хранилище. JSON também é uma opção de transferência para verificação de conformidade e verificação de conformidade.
- **Painéis:** Клонируйте канонический Grafana-дэшборд (`docs/examples/sorafs_fetch_dashboard.json`) no pacote de produção com alertas de venda `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicação e documentação

- Логируйте каждое изменение deny/boost no changelog de operação com отметкой времени, оператором, причиной и связанным incidente.
- Use comandos SDK para configurar seus testes ou para que você possa retornar, sincronizar ожидания на стороне клиента.
- Após a implementação do GA, você pode executar o lançamento do `status.md` e atualizá-lo no runbook em um arquivo confiável.