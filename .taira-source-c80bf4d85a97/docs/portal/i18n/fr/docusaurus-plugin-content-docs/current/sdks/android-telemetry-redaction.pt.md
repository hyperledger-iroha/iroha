---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f22e82241a07052e62531058ef612cbd99bfe52a0bb0cf53ec6d2e28bd6bf389
source_last_modified: "2025-11-18T05:53:43.294424+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Plano de redação de telemetria Android
sidebar_label: Telemetria Android
slug: /sdks/android-telemetry
---

:::note Fonte canonica
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Plano de redação de telemetria Android (AND7)

## Escopo

Este documento registra a política proposta de redação de telemetria e os artefatos de habilitação para o SDK Android conforme exigido pelo item de roadmap **AND7**. Ele alinha a instrumentação mobile com a baseline dos nós Rust enquanto considera garantias de privacidade específicas do dispositivo. O resultado serve como pré‑leitura para a revisão de governança de SRE de fevereiro de 2026.

Objetivos:

- Catalogar cada sinal emitido pelo Android que chega aos backends de observabilidade compartilhados (traces OpenTelemetry, logs codificados em Norito, exportações de métricas).
- Classificar os campos que diferem da baseline Rust e documentar controles de redação ou retenção.
- Descrever o trabalho de habilitação e testes para que os times de suporte respondam de forma determinística a alertas relacionados à redação.

## Inventário de sinais (rascunho)

Instrumentação planejada agrupada por canal. Todos os nomes de campo seguem o esquema de telemetria do SDK Android (`org.hyperledger.iroha.android.telemetry.*`). Campos opcionais são marcados com `?`.

| ID do sinal | Canal | Campos‑chave | Classificação PII/PHI | Redação / Retenção | Notas |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Trace span | `authority_hash`, `route`, `status_code`, `latency_ms` | Autoridade é pública; a rota não contém segredos | Emitir autoridade hashada (`blake2b_256`) antes do export; reter por 7 dias | Espelha `torii.http.request` do Rust; o hashing garante privacidade do alias móvel. |
| `android.torii.http.retry` | Evento | `route`, `retry_count`, `error_code`, `backoff_ms` | Nenhuma | Sem redação; reter por 30 dias | Usado para auditorias determinísticas de retry; campos idênticos aos do Rust. |
| `android.pending_queue.depth` | Métrica gauge | `queue_type`, `depth` | Nenhuma | Sem redação; reter por 90 dias | Corresponde a `pipeline.pending_queue_depth` do Rust. |
| `android.keystore.attestation.result` | Evento | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (derivado), metadados do dispositivo | Substituir o alias por rótulo determinístico, redigir a marca para bucket enum | Necessário para prontidão AND2; nós Rust não emitem metadados do dispositivo. |
| `android.keystore.attestation.failure` | Contador | `alias_label`, `failure_reason` | Nenhuma após redação do alias | Sem redação; reter por 90 dias | Suporta drills de chaos; `alias_label` derivado do alias hashado. |
| `android.telemetry.redaction.override` | Evento | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Papel do ator qualifica como PII operacional | O campo exporta a categoria de papel mascarada; reter por 365 dias com log de auditoria | Não existe no Rust; operadores devem abrir override via suporte. |
| `android.telemetry.export.status` | Contador | `backend`, `status` | Nenhuma | Sem redação; reter por 30 dias | Paridade com contadores de status do exportador Rust. |
| `android.telemetry.redaction.failure` | Contador | `signal_id`, `reason` | Nenhuma | Sem redação; reter por 30 dias | Necessário para espelhar `streaming_privacy_redaction_fail_total` do Rust. |
| `android.telemetry.device_profile` | Métrica gauge | `profile_id`, `sdk_level`, `hardware_tier` | Metadados do dispositivo | Emitir buckets grosseiros (SDK major, hardware tier); reter por 30 dias | Habilita dashboards de paridade sem expor detalhes do OEM. |
| `android.telemetry.network_context` | Evento | `network_type`, `roaming` | Operadora pode ser PII | Remover `carrier_name` totalmente; reter outros campos por 7 dias | `ClientConfig.networkContextProvider` fornece o snapshot sanitizado para apps emitirem tipo de rede + roaming sem expor dados do assinante; dashboards tratam o sinal como análogo móvel de `peer_host` do Rust. |
| `android.telemetry.config.reload` | Evento | `source`, `result`, `duration_ms` | Nenhuma | Sem redação; reter por 30 dias | Espelha spans de reload de config do Rust. |
| `android.telemetry.chaos.scenario` | Evento | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Perfil de dispositivo em buckets | Igual a `device_profile`; reter por 30 dias | Registrado durante ensaios de chaos exigidos para AND7. |
| `android.telemetry.redaction.salt_version` | Métrica gauge | `salt_epoch`, `rotation_id` | Nenhuma | Sem redação; reter por 365 dias | Rastreia rotação do salt Blake2b; alerta de paridade quando o epoch Android diverge do Rust. |
| `android.crash.report.capture` | Evento | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Impressão de crash + metadados de processo | Hash de `crash_id` com o salt de redação, bucketizar estado do watchdog, remover stack frames antes do export; reter por 30 dias | Ativado automaticamente quando `ClientConfig.Builder.enableCrashTelemetryHandler()` é chamado; alimenta dashboards sem expor traços identificáveis. |
| `android.crash.report.upload` | Contador | `crash_id`, `backend`, `status`, `retry_count` | Impressão de crash | Reutilizar `crash_id` hashado, emitir apenas status; reter por 30 dias | Emitir via `ClientConfig.crashTelemetryReporter()` ou `CrashTelemetryHandler.recordUpload` para que os uploads compartilhem garantias Sigstore/OLTP. |

### Pontos de integração

- `ClientConfig` agora encadeia dados de telemetria derivados do manifest via `setTelemetryOptions(...)`/`setTelemetrySink(...)`, registrando `TelemetryObserver` automaticamente para que autoridades hashadas e métricas de salt fluam sem observers específicos. Ver `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` e as classes em `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Aplicativos podem chamar `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` para registrar o `AndroidNetworkContextProvider` baseado em reflexão, que consulta `ConnectivityManager` em runtime e emite o evento `android.telemetry.network_context` sem dependências Android de compile time.
- Os testes unitários `TelemetryOptionsTests` e `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) protegem os helpers de hashing e o hook de integração do ClientConfig para que regressões de manifest apareçam imediatamente.
- O kit/labs de habilitação agora cita APIs concretas em vez de pseudocódigo, mantendo este documento e o runbook alinhados com o SDK publicado.

> **Nota de operações:** a planilha de owners/status fica em `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` e deve ser atualizada junto com esta tabela em cada checkpoint AND7.

## Allowlists de paridade e fluxo de diff de schema

A governança exige uma dupla allowlist para que os exports Android nunca vazem identificadores que serviços Rust expõem intencionalmente. Esta seção espelha a entrada do runbook (`docs/source/android_runbook.md` §2.3), mas mantém o plano AND7 autocontido.

| Categoria | Exportadores Android | Serviços Rust | Validação |
|----------|-------------------|---------------|-----------------|
| Contexto de autoridade/rota | Hash de `authority`/`alias` via Blake2b-256 e remoção de hostnames Torii antes do export; emitir `android.telemetry.redaction.salt_version` para provar rotação do salt. | Emitir hostnames Torii completos e peer IDs para correlação. | Comparar `android.torii.http.request` vs `torii.http.request` no diff mais recente em `docs/source/sdk/android/readiness/schema_diffs/`, depois rodar `scripts/telemetry/check_redaction_status.py` para confirmar epochs do salt. |
| Identidade de dispositivo e signer | Bucketizar `hardware_tier`/`device_profile`, hashear aliases do controlador e nunca exportar números de série. | Emitir `peer_id` do validador, `public_key` do controlador e hashes de fila sem alterações. | Alinhar com `docs/source/sdk/mobile_device_profile_alignment.md`, rodar testes de hashing de alias em `java/iroha_android/run_tests.sh` e arquivar outputs do queue inspector durante os labs. |
| Metadados de rede | Exportar apenas `network_type` + `roaming`; remover `carrier_name`. | Manter metadados de hostname/TLS de peers. | Guardar cada diff em `readiness/schema_diffs/` e alertar se o widget “Network Context” do Grafana mostrar strings de carrier. |
| Evidência de override/chaos | Emitir `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` com papéis mascarados. | Emitir aprovações de override sem máscara; sem spans de chaos. | Verificar `docs/source/sdk/android/readiness/and7_operator_enablement.md` após drills para garantir tokens de override e artefatos de chaos junto aos eventos Rust não mascarados. |

Workflow:

1. Após cada mudança de manifest/exporter, execute `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` e salve o JSON em `docs/source/sdk/android/readiness/schema_diffs/`.
2. Revise o diff com a tabela acima. Se Android emitir campo exclusivo de Rust (ou vice‑versa), registre um bug AND7 e atualize este plano e o runbook.
3. Durante revisões semanais de ops, execute `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` e registre o epoch do salt e o timestamp do diff na planilha de readiness.
4. Registre quaisquer desvios em `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` para que os pacotes de governança capturem decisões de paridade.

> **Referência de schema:** os identificadores canônicos de campo vêm de `android_telemetry_redaction.proto` (materializado durante o build do SDK Android junto aos descritores Norito). O schema expõe `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket` e `actor_role_masked` usados agora no SDK e nos exportadores de telemetria.

`authority_hash` é um digest fixo de 32 bytes do valor de autoridade Torii registrado no proto. `attestation_digest` captura a impressão digital da declaração canônica de atestação, enquanto `device_brand_bucket` mapeia a string de marca Android crua para o enum aprovado (`generic`, `oem`, `enterprise`). `actor_role_masked` carrega a categoria do ator (`support`, `sre`, `audit`) em vez do identificador de usuário bruto.

### Alinhamento de exportação de telemetria de crash

A telemetria de crash agora compartilha os mesmos exportadores OpenTelemetry e a mesma cadeia de proveniência das sinais de rede Torii, encerrando o follow‑up de governança sobre exportadores duplicados. O crash handler alimenta o evento `android.crash.report.capture` com um `crash_id` hashado (Blake2b-256 usando o salt de redação já rastreado por `android.telemetry.redaction.salt_version`), buckets de estado de processo e metadados sanitizados do ANR watchdog. As stack traces permanecem no dispositivo e são apenas resumidas em `has_native_trace` e `anr_watchdog_bucket` antes do export, garantindo que nenhuma PII ou string OEM saia do dispositivo.

Enviar um crash cria o contador `android.crash.report.upload`, permitindo que SRE audite a confiabilidade do backend sem aprender nada sobre o usuário ou a stack. Como ambos os sinais reutilizam o exportador Torii, eles herdam as mesmas garantias Sigstore, políticas de retenção e hooks de alerta já definidos para AND7. Runbooks de suporte podem correlacionar um identificador de crash hashado entre bundles Android e Rust sem um pipeline dedicado.

Ative o handler via `ClientConfig.Builder.enableCrashTelemetryHandler()` quando as opções/sinks de telemetria estiverem configurados; bridges de upload podem reutilizar `ClientConfig.crashTelemetryReporter()` (ou `CrashTelemetryHandler.recordUpload`) para emitir resultados de backend na mesma pipeline assinada.

## Diferenças de política vs baseline Rust

Diferenças entre as políticas de telemetria Android e Rust com passos de mitigação.

| Categoria | Baseline Rust | Política Android | Mitigação / Validação |
|----------|---------------|----------------|-------------------------|
| Identificadores de autoridade/peer | Strings de autoridade em claro | `authority_hash` (Blake2b-256, salt rotativo) | Salt compartilhado publicado via `iroha_config.telemetry.redaction_salt`; teste de paridade garante mapeamento reversível para suporte. |
| Metadados de host/rede | Hostnames/IPs de nós exportados | Apenas tipo de rede + roaming | Dashboards de saúde de rede atualizados para usar categorias de disponibilidade em vez de hostnames. |
| Características do dispositivo | N/A (server-side) | Perfil bucketizado (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Rehearsals de chaos verificam o mapeamento; runbook de suporte documenta a escalada quando necessário. |
| Overrides de redação | Não suportado | Token de override manual armazenado no ledger Norito (`actor_role_masked`, `reason`) | Overrides exigem solicitação assinada; log de auditoria retido por 1 ano. |
| Traços de atestação | Atestação de servidor via SRE apenas | SDK emite resumo de atestação sanitizado | Cruzar hashes de atestação com o validador Rust; alias hashado evita vazamentos. |

Checklist de validação:

- Testes unitários de redação para cada sinal verificando campos hashados/mascarados antes do envio ao exportador.
- Ferramenta de diff de schema (compartilhada com nós Rust) executada nightly para confirmar paridade de campos.
- Script de rehearsal de chaos exercita o workflow de override e confirma o logging de auditoria.

## Tarefas de implementação (pré‑governança SRE)

1. **Confirmação do inventário** — Verificar a tabela acima contra hooks reais de instrumentação Android e definições de schema Norito. Owners: Android Observability TL, LLM.
2. **Diff de schema de telemetria** — Rodar a ferramenta de diff compartilhada contra métricas Rust para produzir artefatos de paridade para a revisão SRE. Owner: SRE privacy lead.
3. **Runbook draft (concluído 2026-02-03)** — `docs/source/android_runbook.md` agora documenta o fluxo end‑to‑end de override (Seção 3) e a matriz de escalonamento expandida mais responsabilidades de papel (Seção 3.1), conectando helpers de CLI, evidências de incidentes e scripts de chaos à política de governança. Owners: LLM com edição Docs/Support.
4. **Conteúdo de habilitação** — Preparar slides de briefing, instruções de lab e perguntas de conhecimento para a sessão de fevereiro de 2026. Owners: Docs/Support Manager, equipe de habilitação SRE.

## Workflow de habilitação e ganchos do runbook

### 1. Cobertura smoke local + CI

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` sobe o sandbox Torii, reproduz o fixture canônico multi‑source SoraFS (delegando para `ci/check_sorafs_orchestrator_adoption.sh`) e semeia telemetria Android sintética.
  - A geração de tráfego é feita por `scripts/telemetry/generate_android_load.py`, que grava um transcript request/response em `artifacts/android/telemetry/load-generator.log` e respeita headers, overrides de path ou modo dry‑run.
  - O helper copia o scoreboard e os resumos SoraFS para `${WORKDIR}/sorafs/` para que os rehearsals AND7 comprovem paridade multi‑source antes de tocar clientes móveis.
- A CI reutiliza as mesmas ferramentas: `ci/check_android_dashboard_parity.sh` roda `scripts/telemetry/compare_dashboards.py` contra `dashboards/grafana/android_telemetry_overview.json`, o dashboard de referência Rust e o arquivo de allowlist em `dashboards/data/android_rust_dashboard_allowances.json`, emitindo o snapshot assinado `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Os rehearsals de chaos seguem `docs/source/sdk/android/telemetry_chaos_checklist.md`; o script sample‑env e o check de paridade de dashboards formam o bundle de evidências “ready” que alimenta a auditoria burn‑in AND7.

### 2. Emissão de override e trilha de auditoria

- `scripts/android_override_tool.py` é a CLI canônica para emitir e revogar overrides de redação. `apply` ingere uma solicitação assinada, emite o bundle do manifest (`telemetry_redaction_override.to` por padrão) e adiciona uma linha de token hashado em `docs/source/sdk/android/telemetry_override_log.md`. `revoke` carimba o timestamp de revogação nessa mesma linha, e `digest` grava o snapshot JSON sanitizado exigido para governança.
- A CLI se recusa a modificar o log de auditoria se o cabeçalho da tabela Markdown estiver ausente, conforme a exigência de compliance registrada em `docs/source/android_support_playbook.md`. A cobertura unitária em `scripts/tests/test_android_override_tool_cli.py` protege o parser da tabela, os emissores de manifest e o tratamento de erros.
- Operadores anexarão o manifest gerado, o trecho atualizado do log **e** o digest JSON em `docs/source/sdk/android/readiness/override_logs/` sempre que um override for exercido; o log retém 365 dias de histórico conforme a decisão de governança deste plano.

### 3. Captura de evidências e retenção

- Cada rehearsal ou incidente produz um bundle estruturado em `artifacts/android/telemetry/` contendo:
  - O transcript do gerador de carga e contadores agregados de `generate_android_load.py`.
  - O diff de dashboards (`android_vs_rust-<stamp>.json`) e o hash da allowlist emitidos por `ci/check_android_dashboard_parity.sh`.
  - O delta do override log (se houve override), o manifest correspondente e o digest JSON atualizado.
- O relatório burn‑in SRE referencia esses artefatos mais o scoreboard SoraFS copiado por `android_sample_env.sh`, oferecendo à revisão AND7 uma cadeia determinística de hashes de telemetria → dashboards → status de overrides.

## Alinhamento de perfil de dispositivo entre SDKs

Os dashboards traduzem o `hardware_tier` Android para o `mobile_profile_class` canônico definido em `docs/source/sdk/mobile_device_profile_alignment.md` para que AND7 e IOS7 comparem as mesmas coortes:

- `lab` — emitido como `hardware_tier = emulator`, alinhado a `device_profile_bucket = simulator` no Swift.
- `consumer` — emitido como `hardware_tier = consumer` (com sufixo do SDK major) e agrupado com buckets `iphone_small`/`iphone_large`/`ipad` do Swift.
- `enterprise` — emitido como `hardware_tier = enterprise`, alinhado ao bucket `mac_catalyst` do Swift e a futuros runtimes iOS desktop gerenciados.

Qualquer novo tier deve ser adicionado ao documento de alinhamento e aos artefatos de diff antes que os dashboards o consumam.

## Governança e distribuição

- **Pacote de pré‑read** — Este documento e os artefatos de apêndice (diff de schema, diff do runbook, outline do deck de readiness) serão distribuídos à lista de governança SRE até **2026-02-05**.
- **Loop de feedback** — Comentários coletados durante a governança alimentarão o epic `AND7` no JIRA; bloqueios são expostos em `status.md` e nas notas do stand‑up semanal Android.
- **Publicação** — Após aprovação, o resumo da política será linkado em `docs/source/android_support_playbook.md` e referenciado pela FAQ de telemetria compartilhada em `docs/source/telemetry.md`.

## Notas de auditoria e compliance

- A política atende GDPR/CCPA removendo dados de assinante móvel antes do export; o salt de autoridade hashada gira trimestralmente e é armazenado no vault compartilhado de segredos.
- Artefatos de habilitação e atualizações do runbook são registrados no registro de compliance.
- Revisões trimestrais confirmam que overrides permanecem closed‑loop (sem acessos obsoletos).

## Resultado de governança (2026-02-12)

A sessão de governança SRE em **2026-02-12** aprovou a política de redação Android sem modificações. Decisões‑chave (ver `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Aceitação da política.** Autoridade hashada, bucketização do perfil de dispositivo e omissão de nomes de carriers foram ratificadas. O rastreamento de rotação de salt via `android.telemetry.redaction.salt_version` torna‑se item de auditoria trimestral.
- **Plano de validação.** Cobertura unitária/integrada, diffs de schema noturnos e rehearsals de chaos trimestrais foram endossados. Ação: publicar relatório de paridade de dashboards após cada rehearsal.
- **Governança de overrides.** Tokens de override registrados em Norito foram aprovados com janela de retenção de 365 dias. Support engineering será dono da revisão do digest do override log nas sincronizações mensais de operações.

## Status de follow‑up

1. **Alinhamento de perfil de dispositivo (prazo 2026-03-01).** ✅ Concluído — o mapeamento compartilhado em `docs/source/sdk/mobile_device_profile_alignment.md` define como valores `hardware_tier` do Android mapeiam para o `mobile_profile_class` canônico usado pelos dashboards de paridade e pelo tooling de diff.

## Próximo brief de governança SRE (T2 2026)

O item de roadmap **AND7** exige que a próxima sessão de governança SRE receba um pré‑read conciso sobre redação de telemetria Android. Use esta seção como brief vivo; mantenha‑a atualizada antes de cada reunião do conselho.

### Checklist de preparação

1. **Bundle de evidências** — exporte o último diff de schema, screenshots de dashboards e digest do override log (ver matriz abaixo) e coloque‑os em uma pasta datada (por exemplo `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) antes de enviar o convite.
2. **Resumo de drills** — anexe o log do rehearsal de chaos mais recente e o snapshot da métrica `android.telemetry.redaction.failure`; garanta que as anotações do Alertmanager referenciem o mesmo timestamp.
3. **Auditoria de override** — confirme que todos os overrides ativos estão registrados no registro Norito e resumidos no deck da reunião. Inclua datas de expiração e IDs de incidentes correspondentes.
4. **Nota de agenda** — avise o chair do SRE 48 horas antes da reunião com o link do brief, destacando decisões necessárias (novos sinais, mudanças de retenção ou atualizações de política de override).

### Matriz de evidências

| Artefato | Local | Owner | Notas |
|----------|----------|-------|-------|
| Diff de schema vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | Deve ser gerado <72 h antes da reunião. |
| Screenshots do diff de dashboards | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | Incluir `sorafs.fetch.*`, `android.telemetry.*` e snapshots do Alertmanager. |
| Digest de override | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Executar `scripts/android_override_tool.sh digest` (ver README no diretório) sobre o `telemetry_override_log.md` mais recente; tokens permanecem hashados antes de compartilhar. |
| Log de rehearsal de chaos | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Anexar resumo de KPI (stall count, retry ratio, uso de override). |

### Perguntas abertas para o conselho

- Precisamos reduzir a janela de retenção de override de 365 dias agora que o digest é automatizado?
- `android.telemetry.device_profile` deve adotar os novos rótulos compartilhados `mobile_profile_class` na próxima release, ou esperar os SDKs Swift/JS lançarem a mesma mudança?
- É necessária orientação adicional para residência regional de dados quando eventos Torii Norito-RPC chegarem ao Android (follow‑up NRPC-3)?

### Procedimento de diff de schema de telemetria

Execute a ferramenta de diff de schema pelo menos uma vez por release candidate (e sempre que a instrumentação Android mudar) para que o conselho SRE receba artefatos de paridade atualizados junto com o diff de dashboards:

1. Exporte os schemas de telemetria Android e Rust que deseja comparar. Para CI as configs ficam em `configs/android_telemetry.json` e `configs/rust_telemetry.json`.
2. Execute `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - Alternativamente, passe commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) para puxar as configs direto do git; o script fixa os hashes no artefato.
3. Anexe o JSON ao bundle de readiness e referencie‑o em `status.md` + `docs/source/telemetry.md`. O diff destaca campos adicionados/removidos e deltas de retenção para que auditores confirmem a paridade sem reexecutar a ferramenta.
4. Quando o diff revelar divergência permitida (por exemplo sinais de override só no Android), atualize o arquivo de allowlist referenciado por `ci/check_android_dashboard_parity.sh` e registre a justificativa no README do diretório de diff.

> **Regras de arquivo:** mantenha os cinco diffs mais recentes em `docs/source/sdk/android/readiness/schema_diffs/` e mova snapshots mais antigos para `artifacts/android/telemetry/schema_diffs/` para que revisores de governança sempre vejam os dados mais recentes.
