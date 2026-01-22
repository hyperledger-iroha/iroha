---
lang: pt
direction: ltr
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7d9bce40727d178bcc7d780c608d82bcd14b0814a7b537cbe9c39a539a200c8
source_last_modified: "2025-12-19T22:31:17.718007+00:00"
translation_last_reviewed: 2026-01-01
---

# Guia de testes e solucao de problemas

Este guia explica como reproduzir cenarios de integracao, quais infraestruturas precisam estar online e como coletar logs acionaveis. Consulte o [relatorio de status](../../status.md) do projeto antes de comecar para saber quais componentes estao atualmente verdes.

## Passos de reproducao

### Testes de integracao (`integration_tests` crate)

1. Garanta que as dependencias do workspace estejam compiladas: `cargo build --workspace`.
2. Execute a suite de testes de integracao com logs completos: `cargo test -p integration_tests -- --nocapture`.
3. Se precisar reexecutar um cenario especifico, use o caminho do modulo, por exemplo `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. Capture fixtures serializadas com Norito para garantir entradas consistentes entre nos:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "ih58...",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   Armazene o Norito JSON resultante junto aos artefatos de teste para que os peers possam reproduzir o mesmo estado.

### Testes do cliente Python (`pytests` directory)

1. Instale os requisitos de Python com `pip install -r pytests/requirements.txt` em um ambiente virtual.
2. Exporte as fixtures compativeis com Norito geradas acima via um caminho compartilhado ou variavel de ambiente.
3. Execute a suite com saida verbosa: `pytest -vv pytests`.
4. Para depuracao direcionada, execute `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`.

## Portas e servicos necessarios

Os seguintes servicos devem estar acessiveis antes de executar qualquer suite:

- **API HTTP Torii**: padrao `127.0.0.1:1337`. Substitua via `torii.address` na sua config (veja `docs/source/references/peer.template.toml`).
- **Notificacoes WebSocket Torii**: padrao `127.0.0.1:8080` para assinaturas de clientes usadas pelo `pytests`.
- **Exportador de telemetria**: padrao `127.0.0.1:8180`. Os testes de integracao esperam que as metricas cheguem aqui para as verificacoes de saude.
- **PostgreSQL** (quando habilitado): padrao `127.0.0.1:5432`. Garanta que as credenciais estejam alinhadas ao perfil do compose em [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

Consulte o [guia de solucao de problemas de telemetria](telemetry.md) se algum endpoint nao estiver disponivel.

### Estabilidade de peers embutidos

`NetworkBuilder::start()` agora aplica uma janela de liveness de cinco segundos apos o genesis para cada peer embutido. Se um processo sair durante esse periodo de guarda, o builder aborta com um erro detalhado que aponta para os logs stdout/stderr em cache. Em maquinas com recursos limitados, voce pode estender a janela (em milissegundos) configurando `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; reduzir para `0` desativa o guard por completo. Garanta que seu ambiente deixe folga de CPU suficiente durante os primeiros segundos de cada suite de integracao para que os peers alcancem o bloco 1 sem acionar o watchdog.

## Coleta e analise de logs

Comece a partir de um diretorio de execucao limpo para que artefatos anteriores nao escondam novos problemas. Os scripts abaixo coletam logs em formatos que as ferramentas Norito downstream conseguem consumir.

- Use [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) apos a execucao dos testes para agregar metricas de nos em snapshots Norito JSON com timestamp.
- Ao investigar problemas de rede, execute [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) para transmitir eventos Torii para `monitor_output.norito.json`.
- Os logs de testes de integracao ficam em `integration_tests/target/`; comprima-os com [`scripts/profile_build.sh`](../../scripts/profile_build.sh) para compartilhar com outras equipes.
- Os logs do cliente Python sao gravados em `pytests/.pytest_cache`. Exporte-os junto com a telemetria capturada com:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

Recolha um pacote completo (integracao, Python, telemetria) antes de abrir um issue para que os maintainers possam reproduzir as traces Norito.

## Proximos passos

Para checklists especificas de release, veja [pipeline](pipeline.md). Se voce encontrar regressos ou falhas, documente-os no [status tracker](../../status.md) compartilhado e referencie entradas relevantes de [solucao de problemas do sumeragi](sumeragi.md).
