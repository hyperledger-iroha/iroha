---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: Pacote de programação SoraFS (SNNet-5a)
sidebar_label: Pacote de programação
description: Configure a configuração, verifique a configuração e a configuração do SoraFS no projeto Selecione Torii/QUIC na versão atual do SNNet-5a.
---

:::nota História Canônica
:::

O conversor SoraNet fornece transporte para uso para SoraFS, mas não há nenhum cartão **SNNet-5a** требует регулируемого fallback, чтобы операторы могли сохранять детерминированный доступ на чтение, пока развертывание анонимности завершается. Este pacote de parâmetros de configuração CLI/SDK, configuração de perfil, configuração de configuração e lista de verificação, Não é necessário para o trabalho SoraFS no programa Torii/QUIC sem necessidade de transporte privado.

Fallback é usado para staging e é um produto regular, pois SNNet-5–SNNet-9 não produz seus portões. Храните артефакты ниже вместе собычными материалами развертывания SoraFS, чтобы операторы могли переключаться между анонимным и прямым режимами по требованию.

## 1. Bandeiras CLI e SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` отключает планирование реле и принудительно использует транспорты Torii/QUIC. A interface CLI é exibida `direct-only` como uma opção de transferência.
- O SDK é instalado `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` como uma opção que permite que você execute a configuração do programa. Сгенерированные биндинги в `iroha::ClientOptions` e `iroha_android` прокидывают тот же enum.
- Gateway-харнесс (`sorafs_fetch`, Python-биндинги) pode ser usado para executar a operação somente direta Norito JSON-хелперы, чтобы автоматизация получала такое же поведение.

Verifique o sinalizador no runbook para parceiros e verifique a configuração do `iroha_config`, sem necessidade operação temporária.

## 2. Gateway de perfil político

Use Norito JSON para determinar a configuração do operador. Exemplo de perfil no código `docs/examples/sorafs_direct_mode_policy.json`:

- `transport_policy: "direct_only"` — fornece um certificado que permite que você reclame o tráfego de transporte da SoraNet.
- `max_providers: 2` — ограничивает прямых пиров наиболее надежными Torii/QUIC эндпойнтами. Mantenha a conformidade com a conformidade regional.
- `telemetry_region: "regulated-eu"` — маркирует метрики, чтобы дашборды e аудиты различали fallback-запуски.
- Консервативные бюджеты повторов (`retry_budget: 2`, `provider_failure_threshold: 3`) para isso, чтобы не маскировать неправильно gateway instalado.

Gere JSON como `sorafs_cli fetch --config` (automático) ou SDK-binding (`config_from_json`) para políticas públicas operador. Coloque seu placar (`persist_path`) para os auditores.Os parâmetros de configuração do gateway no gateway são especificados em `docs/examples/sorafs_gateway_direct_mode.toml`. Você pode usar `iroha app sorafs gateway direct-mode enable`, verificar envelope/admissão, definir padrões para limite de taxa e definir tabela `direct_mode` contém um arquivo do plano e um resumo do manifesto do arquivo. Adicione placeholder-значения на данные вашего rollout antes de configurar a configuração do sistema.

## 3. Набор проверок соответствия

Para definir a configuração do programa, clique no botão CLI:

- `direct_only_policy_rejects_soranet_only_providers` гарантирует, что `TransportPolicy::DirectOnly` быстро падает, когда каждый кандидат anúncio поддерживает только реле SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garante o uso de Torii/QUIC, fornecido on-line e sem registro de SoraNet сессии.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` парсит `docs/examples/sorafs_direct_mode_policy.json`, чтобы документация оставалась согласованной с утилитами-хелперами.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` запускает `sorafs_cli fetch --transport-policy=direct-only` против мокнутого Torii gateway, предоставляя fumaça-tester para регулируемых сред, фиксирующих прямые транспорты.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` оборачивает ту же команду JSON-politikой e сохранением placar para implementação automática.

Faça um teste de segurança antes da publicação:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

A área de trabalho do espaço de trabalho está localizada na configuração upstream, bloqueie o bloqueio de acesso em `status.md` e instale-o запуск после обновления зависимости.

## 4. Автоматизированные fumaça-produtores

A primeira versão da CLI não possui registro específico para operação (por exemplo, gateway de política externa ou несоответствие manifestos). O script de fumaça original é colocado em `scripts/sorafs_direct_mode_smoke.sh` e o `sorafs_cli fetch` é configurado para programar o programa de controle, placar сохранением e resumo сбором.

Exemplo de aplicação:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Script usado e CLI-флаги, e key=value configuração-файлы (см. `docs/examples/sorafs_direct_mode_smoke.conf`). Depois de inserir o manifesto de resumo e exibir o produto de teste do anúncio.
- `--policy` para usar o `docs/examples/sorafs_direct_mode_policy.json`, não posso transferir o orquestrador JSON, gerador `sorafs_orchestrator::bindings::config_to_json`. CLI definia a política de `--orchestrator-config=PATH`, обеспечивая воспроизводимые запуски без ручной подгонки флагов.
- Если `sorafs_cli` não em `PATH`, помощник собирает его из крейта `sorafs_orchestrator` (release профиль), чтобы smoke-прогоны проверяли поставляемую схему прямого режима.
- Выходные данные:
  - Carga útil útil (`--output`, por exemplo `artifacts/sorafs_direct_mode/payload.bin`).
  - Busca de resumo (`--summary`, por meio de um caminho com carga útil), содержащий регион телеметрии и отчеты провайдеров для implementação do доказательной базы.
  - Painel de avaliação do Снимок, compatível com a política JSON (por exemplo, `fetch_state/direct_mode_scoreboard.json`). Registre-o com um resumo no bilhete.
- Автоматизация portão de adoção: после завершения fetch скрипт вызывает `cargo xtask sorafs-adoption-check`, используя сохраненные пути placar e resumo. Требуемый кворум по умолчанию равен числу провайдеров в командной строке; Verifique `--min-providers=<n>` de acordo com a tabela acima. Adoção-отчеты пишутся рядом с summary (`--adoption-report=<path>` позволяет задать место), e um script para умолчанию добавляет `--require-direct-only` (em alternativa com fallback) e `--require-telemetry`, mas você está usando o sinalizador de segurança. Use `XTASK_SORAFS_ADOPTION_FLAGS` para executar o argumento xtask (por exemplo, `--allow-single-source` no momento downgrade одобренного, чтобы gate и терпел, и fallback forçado). Propor portão de adoção com `--skip-adoption-check` только при локальной диагностике; roteiro требует, чтобы каждый регулируемый запуск в прямом режиме включал pacote de adoção-отчета.

## 5. Verifique a configuração

1. **Configurações de configuração:** configure o perfil JSON para configurar a configuração no repositório `iroha_config` e registre-o na lista de ingressos.
2. **Gateway de áudio:** убедитесь, что Torii эндпойнты обеспечивают TLS, capacidade TLV e аудиторский лог до переключения no programa. Abra o gateway de política de perfil para operadores.
3. **Conformidade de conformidade:** você pode usar o playbook de conformidade com conformidade/regulamentos de implementação e implementação não será anonimizado.
4. **Teste de simulação:** você pode usar os testes de conformidade e a busca de preparação através dos testes Torii. Exibir saídas do placar e resumo CLI.
5. **Configurações no produto:** selecione a configuração correta, instale `transport_policy` em `direct_only` (usado `soranet-first`). O plano de documentação foi adquirido no SoraNet-first também, como SNNet-4/5/5a/5b/6a/7/8/12/13 transferido para `roadmap.md:532`.
6. **Exibição:** use o placar do снимки, busca de resumo e monitoração de resultados do tíquete. Verifique `status.md` com dados em condições normais e anormais.Selecione este arquivo de execução no runbook `sorafs_node_ops`, esses operadores podem executar o processo de execução perfeição. Quando o SNNet-5 foi instalado no GA, use o recurso substituto para participar da produção de telemetria.

## 6. Требования к доказательствам и portão de adoção

Захваты прямого режима по-прежнему должны проходить Portão de adoção SF-6c. Собирайте placar, resumo, envelope de manifesto e adoção-отчет para каждого запуска, чтобы `cargo xtask sorafs-adoption-check` мог проверить fallback-позицию. Ao abrir a porta de prova, você pode garantir a metadada no bilhete.

- **Transporte de transporte:** `scoreboard.json` должен указывать `transport_policy="direct_only"` (e переключать `transport_policy_override=true`, когда вы forçar o downgrade). Selecione uma política anônima para definir os padrões de configuração, os valores exibidos serão exibidos поэтапного plano anônimo.
- **Счетчики провайдеров:** Sessão somente gateway должны сохранять `provider_count=0` e заполнять `gateway_provider_count=<n>` числом Torii provador. Não use JSON como padrão: CLI/SDK é uma configuração segura, um portão de adoção que será aberto sem problemas.
- **Manifesto Доказательства:** Когда участвуют Torii gateways, передайте подписанный `--gateway-manifest-envelope <path>` (ou SDK-эквивалент), Os itens `gateway_manifest_provided` e `gateway_manifest_id`/`gateway_manifest_cid` foram registrados em `scoreboard.json`. Portanto, este `summary.json` é substituído por `manifest_id`/`manifest_cid`; проверка adoção провалится, если любой файл не содержит пару.
- **Ожидания телеметрии:** Когда телеметрия сопровождает захват, запускайте gate с `--require-telemetry`, чтобы adoção-отчет подтвердил отправку métrica. As repetições com isolamento de ar podem ser usadas para sinalizar, não CI e тикеты изменений должны документировать отсутствие.

Exemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Use `adoption_report.json` como placar, resumo, envelope de manifesto e pacote de fumaça. Este artefacto executa o trabalho de adoção no CI (`ci/check_sorafs_orchestrator_adoption.sh`) e realiza o downgrade da auditoria.