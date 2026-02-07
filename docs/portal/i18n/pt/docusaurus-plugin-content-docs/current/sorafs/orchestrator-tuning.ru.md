---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ajuste do orquestrador
título: Implementação e organização de instalação
sidebar_label: Настройка оркестратора
description: Практичные значения по умолчанию, советы по настройке и аудит‑чекпоинты para вывода multi-fonte оркестратора в GA.
---

:::nota História Canônica
Verifique `docs/source/sorafs/developer/orchestrator_tuning.md`. Faça cópias da sincronização, pois a documentação não será exibida na configuração.
:::

# Руководство по rollout и настройке оркестратора

Esta é uma verificação de [configuração de configuração] (orchestrator-config.md) e
[implementação de várias fontes do runbook](multi-source-rollout.md). Então,
como usar o organizador para o lançamento de cada etapa, como o interpretador
placar de arte e sinais de sinal telefônicos são usados ​​no mês passado
расширением трафика. Recomendações de uso em CLI, SDK e
автоматизации, чтобы каждый узел следовал одной и той же детерминированной
buscar política.

## 1. Базовые наборы параметров

Certifique-se de que a configuração e a correção de problemas sejam maiores do que o esperado
implementação progressiva. Uma nova tabela de recursos recomendada para o naiболее
распространённых фаз; parâmetros, não usados ​​na tabela, abertos
`OrchestratorConfig::default()` e `FetchOptions::default()`.

| Faz | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Nomeação |
|------|-----------------|------------------------------------------|------------------------------------|-----------------------------|------------------------------------|------------|
| **Laboratório / CI** | `3` | `2` | `2` | `2500` | `300` | Жёсткий лимит задержки и короткая grace‑окно быстро выявляют шумную телеметрию. Se você tentar novamente, isso será feito por meio de manipulações incorretas. |
| **Encenação** | `4` | `3` | `3` | `4000` | `600` | Отражает продакшн‑параметры, оставляя запас для экспериментальных peers. |
| **Canário** | `6` | `3` | `3` | `5000` | `900` | Соответствует значениям по умолчанию; установите `telemetry_region`, este é o segmento canary-traфик. |
| **Disponibilidade Geral** | `None` (qualquer pessoa elegível) | `4` | `4` | `5000` | `900` | Se você tentar novamente/ошибок, você irá realizar o transporte, antes deste produto de auditoria ser executado детерминизм. |

- `scoreboard.weight_scale` остаётся на дефолтном `10_000`, если только downstream‑система не требует другой целочисленной точности. A utilização da máscara não pode ser testada; оно лишь создаёт более плотное распределение кредитов.
- Ao usar o pacote JSON e usá-lo `--scoreboard-out`, ele é auditado. точный набор параметров.

## 2. Placar de jogo

Placar объединяет требования манифеста, объявления провайдеров и телеметрию.
Antes da produção:1. **Proverьте свежесть телеметрии.** Убедитесь, что snapshot'ы, указанные в
   `--telemetry-json`, foi encontrado no site Grace-окна. Veja a estrela
   `telemetry_grace_secs` é usado para `TelemetryStale { last_updated }`.
   Рассматривайте это как жёсткую блокировку e обновите экспорт телеметрии прежде чем продолжать.
2. **Elegibilidade do contrato de compra.** Contrato de contratação de artefactos
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Каждая запись
   Instale o bloco `eligibility` para obter o máximo valor. Não há necessidade de se preocupar
   возможностей или истекшие объявления; distribuir carga útil útil.
3. **Provерьте дельты веса.** Сравните поле `normalised_weight` com a configuração correta.
   Сдвиги >10% должны соответствовать намеренным изменениям объявлений или телеметрии
   e será útil no lançamento do diário.
4. **Архивируйте артефакты.** Instale `scoreboard.persist_path`, чтобы каждый запуск
   публиковал финальный placar instantâneo. Explore o artefacto com a entrega confiável
   вместе с манифестом и телеметрическим pacote.
5. **Fиксируйте доказательства микса провайдеров.** Метаданные `scoreboard.json` _и_
   соответствующий `summary.json` e одержать `provider_count`,
   `gateway_provider_count` e rótulo вычисленный `provider_mix`, чтобы ревьюеры могли
   Isso é feito por `direct-only`, `gateway-only` ou `mixed`. Gateway‑снимки
   должны показывать `provider_count=0` e `provider_mix="gateway-only"`, um exemplo
   programas — não há nenhuma permissão para uma situação histórica. `cargo xtask sorafs-adoption-check`
   valide esta política (e падает при несоответствии счётчиков/лейблов), поэтому запускайте
   eu vou usar `ci/check_sorafs_orchestrator_adoption.sh` ou seu script será criado,
   чтобы получить pacote `adoption_report.json`. Como configurar gateways Torii,
   сохраняйте `gateway_manifest_id`/`gateway_manifest_cid` no placar metadanных, чтобы
   Adoption‑gate pode fornecer envelopes com pacotes de confirmação.

Подробные определения полей см. em
`crates/sorafs_car/src/scoreboard.rs` e resumo da estrutura CLI, atualizado
`sorafs_cli fetch --json-out`.

## Справочник флагов CLI e SDK

`sorafs_cli fetch` (com `crates/sorafs_car/src/bin/sorafs_cli.rs`) e desbloqueado
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) usado
одну и ту же поверхность конфигурации оркестратора. Usar bandeiras de segurança
при сборе evidência или воспроизведении канонических luminárias:

Você pode usar o sinalizador multi-fonte (obter ajuda CLI e sincronização de documentação,
правя только этот файл):- `--max-peers=<count>` ограничивает число elegível-Provaйдеров, проходящих фильтр placar. Faça isso, verifique se você está qualificado para todos os seus fornecedores elegíveis e configure `1` para obter o valor desejado substituto de fonte única. Instale o `maxPeers` no SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` é testado com limite de novas tentativas por pedaço, por exemplo `FetchOptions`. Use a implementação da tabela de руководства para a recomendação de uso; CLI-Proгоны, собирающие evidencia, должны соответствовать дефолтам SDK para сохранения паритета.
- `--telemetry-region=<label>` помечает серии Prometheus `sorafs_orchestrator_*` (e OTLP‑реле) лейблом региона/окружения, чтобы дашборды laboratório разделяли, estadiamento, canário e GA‑трафик.
- `--telemetry-json=<path>` instantâneo instantâneo, colocado no placar. Сохраняйте JSON рядом со scoreboard, чтобы аудиторы могли воспроизвести запуск (e чтобы `cargo xtask sorafs-adoption-check --require-telemetry` доказал, какой captura OTLP‑стрим питал).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) включают hook’и bridge‑наблюдателя. Ao usar o orquestrador, os chunks são armazenados em proxy Norito/Kaigi, clientes abertos, caches de proteção e salas Kaigi получали те же recibos, что и Rust.
- `--scoreboard-out=<path>` (опционально с `--scoreboard-now=<unix_secs>`) fornece elegibilidade de snapshot para auditores. Você pode usar o JSON configurado com artefactos telemétricos e manuais, usando um bilhete confiável.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` determina a correção de metadados para obter metadados. Utilize esta bandeira para repetição; продакшн‑downgrade должен проходить через governança‑артефакты, чтобы каждый узел применял один и тот же pacote de políticas.
- `--provider-metrics-out` / `--chunk-receipts-out` сохраняют метрики здоровья по провайдерам и recibos по pedaços из rollout‑checklist; приложите оба артефакта при подаче evidências para adoção.

Exemplo (com dispositivo elétrico disponível):

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

O SDK é usado para configurar o `SorafsGatewayFetchOptions` em Rust-клиенте
(`crates/iroha/src/client.rs`), ligações JS
(`javascript/iroha_js/src/sorafs.js`) e Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Deixe seus ajudantes
sincronizado com a CLI, esses operadores podem copiar políticas automaticamente
Não há tradução especial.

## 3. Busca política

`FetchOptions` pode tentar novamente, paralisar e verificar. Por exemplo:- **Retentativas:** повышение `per_chunk_retry_limit` выше `4` увеличивает время восстановления,
  Não há problemas com a máscara. Verifique o `4` como um pote e
  verifique o número de rotações para o seu trabalho.
- **Provado:** `provider_failure_threshold` instalado, como provador aberto
  na primeira sessão. Verifique isso com uma nova tentativa: por que não há sucesso
  retry para executar o оркестратор seu peer antes de tentar novamente.
- **Palallелизм:** оставляйте `global_parallel_limit` não definido (`None`), если только конкретная
  Não é possível usar dias úteis. Isso é bom, убедитесь, что значение
  ≤ сумме потоковых бюджетов провайдеров, чтобы избежать fome.
- **Verificações:** `verify_lengths` e `verify_digests` должны оставаться включёнными
  na produção. A única garantia é que você determinará a quantidade certa de provadores; aberto
  é um problema no fuzzing‑средах.

## 4. Estadiamento de transporte e anonimato

Utilize o `rollout_phase`, `anonymity_policy` e `transport_policy`, чтобы
описать приватностную позу:

- Verifique `rollout_phase="snnet-5"` e configure a política anônima
  следовать этапам SNNet-5. Переопределяйте через `anonymity_policy_override` tolcho
  A governança corporativa é uma ação direta.
- Держите `transport_policy="soranet-first"` как базу, пока SNNet-4/5/5a/5b/6a/7/8/12/13 находятся в 🈺
  (veja `roadmap.md`). Use `transport_policy="direct-only"` para documentação
  downgrade/complаенс‑учений и дождитесь ревью PQ‑покрытия перед повышением до
  `transport_policy="soranet-strict"` — esta configuração é atualizada, mas não é possível
  regra clássica.
- `write_mode="pq-only"` é uma ferramenta de configuração que permite que você escreva (SDK, оркестратор,
  ferramentas de governança) способны удовлетворить PQ‑требования. No início do lançamento
  `write_mode="allow-downgrade"`, este recurso pode ser usado para uso
  маршруты, пока телеметрия отмечает downgrade.
- Você pode usar guardas e circuitos de teste no catálogo SoraNet. Передавайте подписанный
  snapshot `relay_directory` e сохраняйте `guard_set` cache, чтобы churn guards оставался
  em retenção-окне. Отпечаток cache, зафиксированный `sorafs_cli fetch`, входит
  в implementação de evidências.

## 5. Хуки деградации и комплаенса

Seu diretor de operação pode usar a política de segurança da sua empresa:- **Remediation деградаций** (`downgrade_remediation`): отслеживает события
  `handshake_downgrade_total` e, por exemplo, `threshold` no modelo `window_secs`,
  use o proxy local em `target_mode` (para usar apenas metadados). Sofra
  дефолты (`threshold=3`, `window=300`, `cooldown=900`), если только постмортемы не
  показывают другой паттерн. Документируйте любые override no lançamento do diário e
  убедитесь, este é o número do `sorafs_proxy_downgrade_state`.
- **Политика комплаенса** (`compliance`): recortes para юрисдикциям и манифестам
  проходят через списки opt-out, управляемые governança. Никогда não встраивайте
  substituição ad-hoc nas configurações do pacote; вместо этого запросите подписанное обновление
  `governance/compliance/soranet_opt_outs.json` и разверните сгенерированный JSON.

Para o melhor sistema, você precisa deste pacote de configurações e включайте его в evidências
релиза, чтобы аудиторы могли проследить причины дауншифтов.

## 6. Telemetria e dados

Antes do lançamento da expansão ser iniciado, isso será sinalizado como ativado na próxima semana:

-`sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  должен быть нулевым после завершения canário.
- `sorafs_orchestrator_retries_total` e
  `sorafs_orchestrator_retry_ratio` — должны стабилизироваться não 10% no verão canário
  e não há 5% após GA.
- `sorafs_orchestrator_policy_events_total` — permite ativar o status do dispositivo
  rollout (lейбл `stage`) e фиксирует brownouts через `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio`/
  `sorafs_orchestrator_pq_deficit_ratio` — отслеживают доступность PQ‑реле относительно
  ожиданий политики.
- Log `telemetry::sorafs.fetch.*` — colocado no log principal com
  Conjunto de proteção para `status=failed`.

Загрузите канонический Grafana‑дашборд из
`dashboards/grafana/sorafs_fetch_observability.json` (exportado no portal
**SoraFS → Buscar Observabilidade**), чтобы селекторы região/manifesto, tentativas de mapa de calor
para provar, os registros contêm pedaços e etapas de configuração
Então, o SRE foi testado durante o período de burn-in. Use o Alertmanager em
`dashboards/alerts/sorafs_fetch_rules.yml` e prove a sintaxe Prometheus
`scripts/telemetry/test_sorafs_fetch_alerts.sh` (ajudante de inicialização automática
`promtool test rules` local ou em Docker). Para transferência de alerta требуется тот же
bloco de roteamento, который печатает скрипт, чтобы операторы могли приложить evidência
к rollout‑тикету.

### Processar telas de burn-in

Roteiro de Пункт **SF-6e** требует 30‑дневного burn‑in телеметрии перед переключением
organizador de múltiplas fontes no GA‑дефолты. Use repositórios de scripts, arquivos
ежедневно собирать воспроизводимый bundle артефактов на протяжении окна:

1. Insira `ci/check_sorafs_orchestrator_adoption.sh` nos parâmetros de instalação
   queimar. Exemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Auxiliar de programação `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   записывает `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` e `adoption_report.json` em
   `artifacts/sorafs_orchestrator/<timestamp>/`, e verifique o tamanho mínimo
   elegível-provaйдеров через `cargo xtask sorafs-adoption-check`.
2. Para que o processo de burn-in seja permanente, o script também será exibido `burn_in_note.json`,
   rótulo de física, dia de índice, fabricante de id, telefones históricos e artesãos de dados.
   Use este JSON para implementação no diário, é isso que você precisa, como fazer o download do site
   30 de dezembro de outubro.
3. Importe o arquivo Grafana‑дашборд (`dashboards/grafana/sorafs_fetch_observability.json`)
   na preparação/produção do espaço de trabalho, coloque seu rótulo burn-in e убедитесь, что каждый
   painel foi removido para o manifesto/região de teste.
4. Selecione `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ou `promtool test rules …`)
   usando a configuração `dashboards/alerts/sorafs_fetch_rules.yml`, você precisa fazer isso
   alerta de roteamento compatível com métricas, transferência para burn-in.
5. Registre o arquivo da sua conta, verifique o alerta e seus registros para confirmação
   `telemetry::sorafs.fetch.*` é um arquiteto de arquitetura, um líder de governança
   воспроизвести evidências não são obtidas no live‑системам.

## 7. Implementação do Чек‑лист

1. Selecione os placares no CI com a configuração do candidato e proteja os artefatos do VCS.
2. Запустите детерминированный buscar fixtures em каждом окружении (laboratório, teste, canário, produção)
   e use os artefatos `--scoreboard-out` e `--json-out` para descrever o lançamento.
3. Prove o número de horas telefônicas automaticamente com o técnico de plantão, убедившись, что все
   A métrica вышеуказанные имеют живые выборки.
4. Verifique a configuração final da configuração (que é `iroha_config`) e git‑commit
   governança-реестра, использованного для объявлений и комплаенса.
5. Abra o rollout‑трекер e use os comandos SDK no novo arquivo, чтобы клиентские
   integração de sincronização.

Следование этому руководству сохраняет развёртывания оркестратора детерминированными и
аудируемыми, одновременно предоставляя ясные контуры обратной связи для настройки
tente novamente, tente novamente e verifique as configurações.