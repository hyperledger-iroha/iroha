---
lang: pt
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Manual de operações e auditoria de ativos confidenciais referenciado por `roadmap.md:M4`.

# Runbook de operações e auditoria de ativos confidenciais

Este guia consolida as superfícies de evidências nas quais auditores e operadores confiam
ao validar fluxos de ativos confidenciais. Complementa o manual de rotação
(`docs/source/confidential_assets_rotation.md`) e o livro de calibração
(`docs/source/confidential_assets_calibration.md`).

## 1. Divulgação seletiva e feeds de eventos

- Cada instrução confidencial emite uma carga útil estruturada `ConfidentialEvent`
  (`Shielded`, `Transferred`, `Unshielded`) capturado em
  `crates/iroha_data_model/src/events/data/events.rs:198` e serializado pelo
  executores (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699` – `4021`).
  O conjunto de regressão exercita as cargas concretas para que os auditores possam confiar
  layouts JSON determinísticos (`crates/iroha_core/tests/zk_confidential_events.rs:19` – `299`).
- Torii expõe esses eventos por meio do pipeline SSE/WebSocket padrão; auditores
  assinar usando `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  opcionalmente, delimitando o escopo para uma única definição de ativo. Exemplo de CLI:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Metadados de políticas e transições pendentes estão disponíveis através
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), espelhado pelo Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) e documentado em
  tanto o design de ativos confidenciais quanto os guias do SDK
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Telemetria, painéis e evidências de calibração

- Métricas de tempo de execução, profundidade da árvore de superfície, histórico de comprometimento/fronteira, remoção de raiz
  contadores e taxas de acerto do cache do verificador
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Painéis Grafana em
  `dashboards/grafana/confidential_assets.json` envia os painéis associados e
  alertas, com o fluxo de trabalho documentado em `docs/source/confidential_assets.md:401`.
- Execuções de calibração (NS/op, gas/op, ns/gas) com registros assinados ao vivo
  `docs/source/confidential_assets_calibration.md`. O mais recente silício da Apple
  A execução NEON está arquivada em
  `docs/source/confidential_assets_calibration_neon_20260428.log`, e o mesmo
  O razão registra as isenções temporárias para perfis SIMD-neutro e AVX2 até
  os hosts x86 ficam online.

## 3. Resposta a incidentes e tarefas do operador

- Os procedimentos de rotação/atualização residem em
  `docs/source/confidential_assets_rotation.md`, abordando como preparar novos
  pacotes de parâmetros, agendar atualizações de políticas e notificar carteiras/auditores. O
  listas de rastreadores (`docs/source/project_tracker/confidential_assets_phase_c.md`)
  proprietários de runbooks e expectativas de ensaio.
- Para ensaios de produção ou janelas de emergência, os operadores anexam evidências
  Entradas `status.md` (por exemplo, o registro de ensaio multi-lane) e incluem:
  Prova `curl` de transições de política, instantâneos Grafana e o evento relevante
  resumos para que os auditores possam reconstruir os cronogramas do mint→transfer→reveal.

## 4. Cadência de revisão externa

- Escopo da revisão de segurança: circuitos confidenciais, registros de parâmetros, política
  transições e telemetria. Este documento mais os formulários de registro de calibração
  o pacote de evidências enviado aos fornecedores; o agendamento da revisão é rastreado via
  M4 em `docs/source/project_tracker/confidential_assets_phase_c.md`.
- Os operadores devem manter `status.md` atualizado com quaisquer descobertas ou acompanhamento do fornecedor
  itens de ação. Até a conclusão da revisão externa, este runbook serve como base
  os auditores da linha de base operacional podem testar.