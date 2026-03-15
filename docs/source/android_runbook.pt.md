---
lang: pt
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook de operações do Android SDK

Este runbook oferece suporte a operadores e engenheiros de suporte que gerenciam o Android SDK
implantações para AND7 e além. Combine com o manual de suporte do Android para SLA
definições e caminhos de escalonamento.

> **Observação:** Ao atualizar procedimentos de incidentes, atualize também o arquivo compartilhado
> matriz de solução de problemas (`docs/source/sdk/android/troubleshooting.md`) para que o
> tabelas de cenários, SLAs e referências de telemetria permanecem alinhadas com este runbook.

## 0. Início rápido (quando os pagers disparam)

Mantenha esta sequência à mão para alertas Sev1/Sev2 antes de mergulhar nos detalhes
seções abaixo:

1. **Confirme a configuração ativa:** Capture a soma de verificação do manifesto `ClientConfig`
   emitido na inicialização do aplicativo e compare-o com o manifesto fixado em
   `configs/android_client_manifest.json`. Se os hashes divergirem, interrompa os lançamentos e
   registre um ticket de desvio de configuração antes de tocar em telemetria/substituições (consulte §1).
2. **Execute o portão de comparação de esquema:** Execute a CLI `telemetry-schema-diff` em
   o instantâneo aceito
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   Trate qualquer saída `policy_violations` como Sev2 e bloqueie as exportações até que o
   a discrepância é compreendida (ver §2.6).
3. **Verifique os painéis + CLI de status:** Abra o Android Telemetry Redaction e
   Conselhos de saúde do exportador e, em seguida, execute
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. Se
   autoridades estão ocultas ou erro de exportação, capture capturas de tela e
   Saída CLI para o documento do incidente (ver §2.4–§2.5).
4. **Decidir sobre substituições:** Somente após as etapas acima e com incidente/proprietário
   gravado, emita uma substituição limitada via `scripts/android_override_tool.sh`
   e registre-o em `telemetry_override_log.md` (ver §3). Validade padrão: <24h.
5. **Escalar por lista de contatos:** Página do Android de plantão e TL de observabilidade
   (contatos em §8), depois siga a árvore de escalonamento em §4.1. Se atestado ou
   Os sinais do StrongBox estão envolvidos, puxe o pacote mais recente e execute o chicote
   verificações do §7 antes de reativar as exportações.

## 1. Configuração e implantação

- **Fornecimento de ClientConfig:** Garanta que os clientes Android carreguem o endpoint Torii, TLS
  políticas e botões de nova tentativa de manifestos derivados de `iroha_config`. Validar
  valores durante a inicialização do aplicativo e soma de verificação de log do manifesto ativo.
  Referência de implementação: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  roscas `TelemetryOptions` de `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (mais o `TelemetryObserver` gerado) para que as autoridades com hash sejam emitidas automaticamente.
- **Recarga a quente:** Use o observador de configuração para obter `iroha_config`
  atualizações sem reinicializações do aplicativo. Recarregamentos com falha devem emitir o
  Evento `android.telemetry.config.reload` e nova tentativa de gatilho com exponencial
  espera (máximo de 5 tentativas).
- **Comportamento de fallback:** Quando a configuração estiver ausente ou for inválida, volte para
  padrões seguros (modo somente leitura, sem envio de fila pendente) e exibir um usuário
  alerta. Registre o incidente para acompanhamento.

### 1.1 Diagnóstico de recarga de configuração- O observador de configuração emite sinais `android.telemetry.config.reload` com
  `source`, `result`, `duration_ms` e campos opcionais `digest`/`error` (consulte
  `configs/android_telemetry.json` e
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  Espere um único evento `result:"success"` por manifesto aplicado; repetido
  Os registros `result:"error"` indicam que o observador esgotou suas 5 tentativas de espera
  começando aos 50ms.
- Durante um incidente, capture o último sinal de recarga do coletor
  (armazenamento OTLP/span ou o endpoint de status de redação) e registre o `digest` +
  `source` no documento do incidente. Compare o resumo com
  `configs/android_client_manifest.json` e o manifesto de lançamento distribuído para
  operadores.
- Se o observador continuar a emitir erros, execute o chicote direcionado para reproduzir
  a falha de análise com o manifesto suspeito:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  Anexe a saída do teste e o manifesto com falha ao pacote de incidentes para que o SRE
  pode compará-lo com o esquema de configuração preparado.
- Quando a telemetria de recarga estiver faltando, confirme se o `ClientConfig` ativo carrega um
  coletor de telemetria e que o coletor OTLP ainda aceita o
  ID `android.telemetry.config.reload`; caso contrário, trate-o como uma telemetria Sev2
  regressão (mesmo caminho de §2.4) e liberações de pausa até que o sinal retorne.

### 1.2 Pacotes de exportação de chave determinística
- As exportações de software agora emitem pacotes v3 com salt + nonce por exportação, `kdf_kind` e `kdf_work_factor`.
  O exportador prefere Argon2id (64 MiB, 3 iterações, paralelismo = 2) e volta para
  PBKDF2-HMAC-SHA256 com piso de iteração de 350 k quando Argon2id não está disponível no dispositivo. Pacote
  O AAD ainda está vinculado ao alias; as senhas devem ter pelo menos 12 caracteres para exportações v3 e o
  o importador rejeita sementes totalmente sem sal/nonce.
  `KeyExportBundle.decode(Base64|bytes)`, importe com a senha original e reexporte para v3 para
  mude para o formato de memória rígida. O importador rejeita pares sal/nonce totalmente zero ou reutilizados; sempre
  gire pacotes em vez de reutilizar exportações antigas entre dispositivos.
- Testes de caminho negativo em `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`
  rejeição. Limpe as matrizes de caracteres da senha após o uso e capture a versão do pacote e `kdf_kind`
  nas notas de incidente quando a recuperação falha.

## 2. Telemetria e redação

> Referência rápida: consulte
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> para a lista de verificação condensada de comando/limite usada durante a ativação
> sessões e pontes de incidentes.- **Inventário de sinais:** Consulte `docs/source/sdk/android/telemetry_redaction.md`
  para obter a lista completa de intervalos/métricas/eventos emitidos e
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  para detalhes do proprietário/validação e lacunas pendentes.
- **Diferença de esquema canônico:** O instantâneo AND7 aprovado é
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  Cada nova execução da CLI deve ser comparada com este artefato para que os revisores possam ver
  que o `intentional_differences` e `android_only_signals` aceitos ainda
  corresponder às tabelas de políticas documentadas em
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. A CLI agora adiciona
  `policy_violations` quando qualquer diferença intencional está faltando um
  `status:"accepted"`/`"policy_allowlisted"` (ou quando os registros somente do Android são perdidos
  seu status aceito), então trate as violações não vazias como Sev2 e pare
  exportações. Os trechos `jq` abaixo permanecem como uma verificação manual de integridade em arquivos
  artefatos:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  Trate qualquer saída desses comandos como uma regressão de esquema que precisa de uma
  Bug de prontidão AND7 antes que as exportações de telemetria continuem; `field_mismatches`
  deve permanecer vazio conforme `telemetry_schema_diff.md` §5. O ajudante agora escreve
  `artifacts/android/telemetry/schema_diff.prom` automaticamente; passar
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (ou conjunto
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) ao executar em hosts de teste/produção
  então o medidor `telemetry_schema_diff_run_status` muda para `policy_violation`
  automaticamente se a CLI detectar desvio.
- **Ajudante CLI:** `scripts/telemetry/check_redaction_status.py` inspeciona
  `artifacts/android/telemetry/status.json` por padrão; passe `--status-url` para
  teste de consulta e `--write-cache` para atualizar a cópia local para offline
  exercícios. Use `--min-hashed 214` (ou defina
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) para impor a governança
  piso em autoridades com hash durante cada pesquisa de status.
- **Hash de autoridade:** Todas as autoridades são hash usando Blake2b-256 com o
  sal de rotação trimestral armazenado no cofre de segredos seguros. As rotações ocorrem em
  na primeira segunda-feira de cada trimestre às 00:00 UTC. Verifique se o exportador coleta
  o novo salt verificando a métrica `android.telemetry.redaction.salt_version`.
- **Buckets de perfil de dispositivo:** Somente `emulator`, `consumer` e `enterprise`
  camadas são exportadas (juntamente com a versão principal do SDK). Os painéis comparam estes
  conta em relação às linhas de base do Rust; variação >10% gera alertas.
- **Metadados de rede:** o Android exporta apenas os sinalizadores `network_type` e `roaming`.
  Os nomes das operadoras nunca são emitidos; as operadoras não devem solicitar ao assinante
  informações em registros de incidentes. O instantâneo higienizado é emitido como o
  Evento `android.telemetry.network_context`, portanto, certifique-se de que os aplicativos registrem um
  `NetworkContextProvider` (via
  `ClientConfig.Builder.setNetworkContextProvider(...)` ou a conveniência
  auxiliar `enableAndroidNetworkContext(...)`) antes que as chamadas Torii sejam emitidas.
- **Ponteiro Grafana:** O painel `Android Telemetry Redaction` é o
  verificação visual canônica para a saída CLI acima - confirme o
  O painel `android.telemetry.redaction.salt_version` corresponde à atual época do sal
  e o widget `android_telemetry_override_tokens_active` permanece em zero
  sempre que não houver exercícios ou incidentes em andamento. Escalar se algum painel se desviar
  antes que os scripts CLI relatem uma regressão.

### 2.1 Exportar fluxo de trabalho do pipeline1. **Distribuição de configuração.** `ClientConfig.telemetry.redaction` é encadeado de
   `iroha_config` e recarregado a quente por `ConfigWatcher`. Cada recarga registra o
   resumo do manifesto mais época do sal - capture essa linha em incidentes e durante
   ensaios.
2. **Instrumentação.** Os componentes do SDK emitem intervalos/métricas/eventos no
   `TelemetryBuffer`. O buffer marca cada carga útil com o perfil do dispositivo e
   época atual do sal para que o exportador possa verificar as entradas de hash de forma determinística.
3. **Filtro de redação.** `RedactionFilter` hashes `authority`, `alias` e
   identificadores do dispositivo antes de saírem do dispositivo. Falhas emitem
   `android.telemetry.redaction.failure` e bloquear a tentativa de exportação.
4. **Exportador + coletor.** Cargas higienizadas são enviadas por meio do Android
   Exportador OpenTelemetry para a implantação `android-otel-collector`. O
   saídas dos ventiladores do coletor para rastreamentos (Tempo), métricas (Prometheus) e Norito
   coletores de log.
5. **Ganchos de observabilidade.** Leituras `scripts/telemetry/check_redaction_status.py`
   contadores coletores (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) e produz o pacote de status
   referenciado ao longo deste runbook.

### 2.2 Portas de validação

- **Diferença de esquema:** Executar
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  sempre que manifesta mudança. Após cada execução, confirme cada
  A entrada `intentional_differences[*]` e `android_only_signals[*]` está carimbada
  `status:"accepted"` (ou `status:"policy_allowlisted"` para hash/compartimento
  campos) conforme recomendado em `telemetry_schema_diff.md` §3 antes de anexar o
  artefato até incidentes e relatórios de laboratório de caos. Use o instantâneo aprovado
  (`android_vs_rust-20260305.json`) como guarda-corpo e fiapos do recém-emitido
  JSON antes de ser arquivado:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  Compare `$LATEST` com
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  para provar que a lista de permissões permaneceu inalterada. `status` ausente ou em branco
  entradas (por exemplo em `android.telemetry.redaction.failure` ou
  `android.telemetry.redaction.salt_version`) agora são tratados como regressões e
  deve ser resolvido antes que a revisão possa ser encerrada; a CLI apresenta o aceito
  diretamente, portanto a referência cruzada do manual §3.4 só se aplica quando
  explicando por que um status não `accepted` aparece.

  **Sinais AND7 canônicos (instantâneo de 05/03/2026)**| Sinal | Canal | Estado | Nota sobre governança | Gancho de validação |
  |----|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` | Evento | `accepted` | Os espelhos substituem os manifestos e devem corresponder às entradas `telemetry_override_log.md`. | Assista `android_telemetry_override_tokens_active` e arquive os manifestos conforme §3. |
  | `android.telemetry.network_context` | Evento | `accepted` | O Android omite intencionalmente os nomes das operadoras; apenas `network_type` e `roaming` são exportados. | Certifique-se de que os aplicativos registrem um `NetworkContextProvider` e confirme se o volume do evento segue o tráfego Torii em `Android Telemetry Overview`. |
  | `android.telemetry.redaction.failure` | Contador | `accepted` | Emite sempre que o hash falha; a governança agora requer metadados de status explícitos no artefato de comparação de esquema. | O painel do painel `Redaction Compliance` e a saída CLI do `check_redaction_status.py` devem permanecer em zero, exceto durante exercícios. |
  | `android.telemetry.redaction.salt_version` | Medidor | `accepted` | Prova que o exportador está a utilizar a actual época trimestral do sal. | Compare o widget salt de Grafana com a época do secrets-vault e certifique-se de que as execuções de comparação de esquema mantenham a anotação `status:"accepted"`. |

  Se alguma entrada na tabela acima descartar um `status`, o artefato diff deverá ser
  regenerado **e** `telemetry_schema_diff.md` atualizado antes do AND7
  pacote de governança é distribuído. Incluir o JSON atualizado em
  `docs/source/sdk/android/readiness/schema_diffs/` e vincule-o a partir do
  incidente, laboratório de caos ou relatório de ativação que acionou a repetição.
- **Cobertura CI/unidade:** `ci/run_android_tests.sh` deve passar antes
  publicar compilações; o pacote impõe comportamento de hashing/substituição exercendo
  os exportadores de telemetria com cargas úteis de amostra.
- **Verificações de integridade do injetor:** Use
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` antes dos ensaios
  para confirmar que a injeção de falha funciona e que alerta o fogo quando os guardas são hash
  estão desarmados. Sempre limpe o injetor com `--clear` após a validação
  completa.

### 2.3 Mobile ↔ Lista de verificação de paridade de telemetria Rust

Mantenha os exportadores Android e os serviços do nó Rust alinhados, respeitando o
diferentes requisitos de redação documentados em
`docs/source/sdk/android/telemetry_redaction.md`. A tabela abaixo serve como
lista de permissões dupla referenciada na entrada do roteiro AND7 – atualize-a sempre que o
esquema diff introduz ou remove campos.| Categoria | Exportadores Android | Serviços de ferrugem | Gancho de validação |
|----------|------------------|---------------|-----------------|
| Contexto de autoridade/rota | Hash `authority`/`alias` via Blake2b-256 e descarte nomes de host Torii brutos antes da exportação; emita `android.telemetry.redaction.salt_version` para provar a rotação do sal. | Emita nomes de host Torii completos e IDs de pares para correlação. | Compare as entradas `android.torii.http.request` vs `torii.http.request` na comparação de esquema mais recente em `readiness/schema_diffs/` e confirme se `android.telemetry.redaction.salt_version` corresponde ao salt do cluster executando `scripts/telemetry/check_redaction_status.py`. |
| Identidade do dispositivo e do assinante | Balde `hardware_tier`/`device_profile`, aliases de controlador de hash e nunca exporte números de série. | Nenhum metadado do dispositivo; os nós emitem o validador `peer_id` e o controlador `public_key` literalmente. | Espelhe os mapeamentos em `docs/source/sdk/mobile_device_profile_alignment.md`, audite as saídas de `PendingQueueInspector` durante os laboratórios e garanta que os testes de hash de alias dentro de `ci/run_android_tests.sh` permaneçam verdes. |
| Metadados de rede | Exportar apenas booleanos `network_type` + `roaming`; `carrier_name` foi descartado. | Rust retém nomes de host de pares, além de metadados completos de endpoint TLS. | Armazene o JSON diff mais recente em `readiness/schema_diffs/` e confirme se o lado Android ainda omite `carrier_name`. Alerte se o widget “Network Context” do Grafana mostrar alguma string de operadora. |
| Evidência de substituição/caos | Emita eventos `android.telemetry.redaction.override` e `android.telemetry.chaos.scenario` com papéis de ator mascarados. | Os serviços Rust emitem aprovações de substituição sem mascaramento de função e sem extensões específicas de caos. | Verifique `docs/source/sdk/android/readiness/and7_operator_enablement.md` após cada exercício para garantir que tokens de substituição e artefatos de caos sejam arquivados junto com os eventos Rust desmascarados. |

Fluxo de trabalho de paridade:

1. Após cada alteração no manifesto ou no exportador, execute
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   portanto, o artefato JSON e as métricas espelhadas chegam ao pacote de evidências
   (o auxiliar ainda escreve `artifacts/android/telemetry/schema_diff.prom` por padrão).
2. Revise a diferença em relação à tabela acima; se o Android agora emitir um campo que é
   permitido apenas no Rust (ou vice-versa), registre um bug de prontidão AND7 e atualize
   o plano de redação.
3. Durante as verificações semanais, execute
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   para confirmar que as épocas do sal correspondem ao widget Grafana e anote a época no
   diário de plantão.
4. Registre quaisquer deltas em
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` então
   a governação pode auditar decisões de paridade.

### 2.4 Painéis de observabilidade e limites de alerta

Mantenha os painéis e alertas alinhados com as aprovações de diferenças de esquema AND7 quando
revisando a saída `scripts/telemetry/check_redaction_status.py`:

- `Android Telemetry Redaction` — Widget da época do sal, substitui o medidor de token.
- `Redaction Compliance` — Contador `android.telemetry.redaction.failure` e
  painéis de tendências de injetores.
- `Exporter Health` — Detalhamento da taxa `android.telemetry.export.status`.
- `Android Telemetry Overview` — buckets de perfil de dispositivo e volume de contexto de rede.

Os seguintes limites refletem o cartão de referência rápida e devem ser aplicados
durante a resposta a incidentes e ensaios:| Métrica / painel | Limite | Ação |
|----------------|-----------|--------|
| `android.telemetry.redaction.failure` (placa `Redaction Compliance`) | >0 em uma janela contínua de 15 minutos | Investigue o sinal de falha, execute a limpeza do injetor, registre a saída CLI + captura de tela Grafana. |
| `android.telemetry.redaction.salt_version` (placa `Android Telemetry Redaction`) | Difere da época do sal do cofre dos segredos | Interromper lançamentos, coordenar com rotação de segredos, arquivar nota AND7. |
| `android.telemetry.export.status{status="error"}` (placa `Exporter Health`) | >1% das exportações | Inspecione a integridade do coletor, capture diagnósticos CLI e escale para SRE. |
| Paridade `android.telemetry.device_profile{tier="enterprise"}` vs Rust (`Android Telemetry Overview`) | Variação> 10% da linha de base do Rust | Acompanhamento de governança de arquivos, verificação de pools de equipamentos, anotação de artefatos de diferenças de esquema. |
| Volume `android.telemetry.network_context` (`Android Telemetry Overview`) | Cai para zero enquanto existe tráfego Torii | Confirme o registro `NetworkContextProvider` e execute novamente a comparação do esquema para garantir que os campos não sejam alterados. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | Janela de cancelamento/perfuração aprovada externamente diferente de zero | Vincule o token a um incidente, gere novamente o resumo e revogue por meio do fluxo de trabalho no §3. |

### 2.5 Preparação do operador e trilha de capacitação

O item AND7 do roteiro apresenta um currículo de operador dedicado para suporte, SRE e
liberar as partes interessadas para entender as tabelas de paridade acima antes que o runbook seja publicado
GA. Use o esboço em
`docs/source/sdk/android/telemetry_readiness_outline.md` para logística canônica
(agenda, apresentadores, cronograma) e `docs/source/sdk/android/readiness/and7_operator_enablement.md`
para obter a lista de verificação detalhada, links de evidências e registro de ações. Mantenha o seguinte
fases em sincronia sempre que o plano de telemetria muda:| Fase | Descrição | Pacote de evidências | Proprietário principal |
|-------|------------|-----------------|---------------|
| Distribuição pré-leitura | Envie a pré-leitura da apólice, `telemetry_redaction.md`, e o cartão de referência rápida pelo menos cinco dias úteis antes do briefing. Rastreie as confirmações no registro de comunicações do esboço. | `docs/source/sdk/android/telemetry_readiness_outline.md` (Logística de Sessão + Log de Comunicações) e o e-mail arquivado em `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`. | Gerente de documentos/suporte |
| Sessão de preparação ao vivo | Ofereça o treinamento de 60 minutos (aprofundamento da política, instruções do runbook, painéis, demonstração do laboratório de caos) e mantenha a gravação em execução para visualizadores assíncronos. | Gravação + slides armazenados sob `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` com referências capturadas no §2 do esboço. | LLM (proprietário interino do AND7) |
| Execução do laboratório do caos | Execute pelo menos C2 (substituição) + C6 (reprodução de fila) de `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` imediatamente após a sessão ao vivo e anexe logs/capturas de tela ao kit de ativação. | Relatórios de cenário e capturas de tela dentro de `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` e `/screenshots/<YYYY-MM>/`. | Observabilidade Android TL + SRE de plantão |
| Verificação de conhecimento e presença | Colete envios de questionários, corrija qualquer pontuação <90% e registre estatísticas de participação/questionários. Mantenha as perguntas de referência rápida alinhadas com a lista de verificação de paridade. | Exportações de questionário em `docs/source/sdk/android/readiness/forms/responses/`, resumo Markdown/JSON produzido via `scripts/telemetry/generate_and7_quiz_summary.py` e a tabela de presença dentro de `and7_operator_enablement.md`. | Engenharia de suporte |
| Arquivo e acompanhamentos | Atualize o log de ação do kit de ativação, carregue os artefatos no arquivo e anote a conclusão em `status.md`. Quaisquer tokens de correção ou substituição emitidos durante a sessão devem ser copiados para `telemetry_override_log.md`. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (log de ação), `.../archive/<YYYY-MM>/checklist.md` e o log de substituição mencionado em §3. | LLM (proprietário interino do AND7) |

Quando o currículo for executado novamente (trimestralmente ou antes de grandes alterações no esquema), atualize
o esboço com a nova data da sessão, manter a lista de participantes atualizada e
regenerar os artefatos JSON/Markdown do resumo do questionário para que os pacotes de governança possam
referenciar evidências consistentes. A entrada `status.md` para AND7 deve estar vinculada ao
pasta de arquivo mais recente assim que cada sprint de ativação for encerrado.

### 2.6 Listas de permissões de diferenças de esquema e verificações de políticas

O roteiro chama explicitamente uma política de lista de permissões dupla (redações móveis vs.
Retenção de ferrugem) que é aplicada pela CLI `telemetry-schema-diff` alojada em
`tools/telemetry-schema-diff`. Cada artefato diff registrado em
`docs/source/sdk/android/readiness/schema_diffs/` deve documentar quais campos são
hash/com bucket no Android, quais campos permanecem sem hash no Rust e se
qualquer sinal não listado na lista de permissões entrou na compilação. Capture essas decisões
diretamente no JSON executando:

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```O `jq` final é avaliado como não operacional quando o relatório está limpo. Trate qualquer saída
desse comando como um bug de prontidão Sev2: um `policy_violations` preenchido
array significa que a CLI descobriu um sinal que não está na lista somente para Android
nem na lista de isenção somente para ferrugem documentada em
`docs/source/sdk/android/telemetry_schema_diff.md`. Quando isso ocorrer, pare
exportações, registre um ticket AND7 e execute novamente a comparação somente após o módulo de política
e os instantâneos do manifesto foram corrigidos. Armazene o JSON resultante em
`docs/source/sdk/android/readiness/schema_diffs/` com sufixo de data e nota
o caminho dentro do incidente ou relatório de laboratório para que a governança possa reproduzir as verificações.

**Matriz de hash e retenção**

| Campo.sinal | Manipulação do Android | Tratamento de ferrugem | Marca de lista de permissões |
|--------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Hash Blake2b-256 (`representation: "blake2b_256"`) | Armazenado literalmente para rastreabilidade | `policy_allowlisted` (hash móvel) |
| `attestation.result.alias` | Blake2b-256 com hash | Alias ​​de texto simples (arquivos de atestado) | `policy_allowlisted` |
| `attestation.result.device_tier` | Em balde (`representation: "bucketed"`) | String de camada simples | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | Ausente – Os exportadores de Android abandonam totalmente o campo | Apresentar sem redação | `rust_only` (documentado em §3 de `telemetry_schema_diff.md`) |
| `android.telemetry.redaction.override.*` | Sinal apenas para Android com papéis de atores mascarados | Nenhum sinal equivalente emitido | `android_only` (deve ficar `status:"accepted"`) |

Quando novos sinais aparecerem, adicione-os ao módulo de política de comparação de esquema **e** ao
tabela acima para que o runbook espelhe a lógica de imposição enviada na CLI.
As execuções do esquema agora falharão se qualquer sinal somente Android omitir um `status` explícito ou se
a matriz `policy_violations` não está vazia, portanto, mantenha esta lista de verificação sincronizada com
`telemetry_schema_diff.md` §3 e os snapshots JSON mais recentes referenciados em
`telemetry_redaction_minutes_*.md`.

## 3. Substituir fluxo de trabalho

As substituições são a opção de “quebrar o vidro” ao fazer hash de regressões ou privacidade
alertas bloqueiam clientes. Aplique-os somente após registrar toda a trilha de decisão
no documento do incidente.1. **Confirme o desvio e o escopo.** Aguarde o alerta do PagerDuty ou a diferença de esquema
   portão para disparar, então corra
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` para
   provar autoridades incompatíveis. Anexe a saída CLI e as capturas de tela do Grafana
   ao registro do incidente.
2. **Prepare uma solicitação assinada.** Preencha
   `docs/examples/android_override_request.json` com o ID do ticket, solicitante,
   vencimento e justificativa. Armazene o arquivo próximo aos artefatos do incidente para
   conformidade pode auditar as entradas.
3. **Emitir a substituição.** Invocar
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   O auxiliar imprime o token de substituição, grava o manifesto e anexa uma linha
   para o log de auditoria do Markdown. Nunca poste o token no chat; entregá-lo diretamente
   aos operadores Torii que aplicam a substituição.
4. **Monitore o efeito.** Dentro de cinco minutos, verifique um único
   O evento `android.telemetry.redaction.override` foi emitido, o coletor
   o endpoint de status mostra `override_active=true` e o documento do incidente lista o
   expiração. Assista ao painel Visão geral da telemetria do Android “Substituir tokens
   painel ativo” (`android_telemetry_override_tokens_active`) para o mesmo
   contagem de tokens e continue executando a CLI de status a cada 10 minutos até
   hash estabiliza.
5. **Revogar e arquivar.** Assim que a mitigação chegar, execute
  `scripts/android_override_tool.sh revoke --token <token>` para que o log de auditoria
  captura o tempo de revogação e executa
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  para atualizar o instantâneo limpo que a governança espera. Anexe o
  manifesto, resumo JSON, transcrições CLI, instantâneos Grafana e o log NDJSON
  produzido via `--event-log` para
  `docs/source/sdk/android/readiness/screenshots/<date>/` e faça a ligação cruzada do
  entrada de `docs/source/sdk/android/telemetry_override_log.md`.

Substituições que excedam 24 horas exigem aprovação do Diretor de SRE e de Compliance e
deve ser destacado na próxima revisão semanal do AND7.

### 3.1 Substituir matriz de escalonamento

| Situação | Duração máxima | Aprovadores | Notificações necessárias |
|-----------|--------------|-----------|-------------|
| Investigação de inquilino único (incompatibilidade de autoridade com hash, cliente Sev2) | 4 horas | Engenheiro de suporte + SRE de plantão | Ticket `SUP-OVR-<id>`, evento `android.telemetry.redaction.override`, registro de incidentes |
| Interrupção de telemetria em toda a frota ou reprodução solicitada por SRE | 24 horas | SRE de plantão + líder do programa | Nota PagerDuty, substituir entrada de log, atualizar em `status.md` |
| Solicitação de conformidade/perícia ou qualquer caso superior a 24 horas | Até ser explicitamente revogado | Diretor SRE + Líder de Compliance | Lista de discussão de governança, registro de substituição, status semanal AND7 |

#### Responsabilidades da função| Função | Responsabilidades | SLA/Notas |
|------|------------------|------------|
| Telemetria Android de plantão (Incident Commander) | Impulsione a detecção, execute as ferramentas de substituição, registre as aprovações no documento do incidente e garanta que a revogação aconteça antes do vencimento. | Reconheça o PagerDuty em 5 minutos e registre o progresso a cada 15 minutos. |
| Observabilidade do Android TL (Haruka Yamamoto) | Valide o sinal de desvio, confirme o estado do exportador/coletor e assine o manifesto de substituição antes que ele seja entregue aos operadores. | Junte-se à ponte em 10 minutos; delegue ao proprietário do cluster de preparo se não estiver disponível. |
| Contato SRE (Liam O'Connor) | Aplique o manifesto aos coletores, monitore o backlog e coordene com a Engenharia de Liberação as mitigações do lado Torii. | Registre cada ação `kubectl` na solicitação de mudança e cole as transcrições dos comandos no documento do incidente. |
| Compliance (Sofia Martins / Daniel Park) | Aprovar substituições com duração superior a 30 minutos, verificar a linha do registro de auditoria e aconselhar sobre mensagens do regulador/cliente. | Pós-reconhecimento em `#compliance-alerts`; para eventos de produção, arquive uma nota de conformidade antes que a substituição seja emitida. |
| Gerente de Documentos/Suporte (Priya Deshpande) | Arquive manifestos/saída CLI em `docs/source/sdk/android/readiness/…`, mantenha o log de substituição organizado e agende laboratórios de acompanhamento se surgirem lacunas. | Confirma a retenção de evidências (13 meses) e arquiva AND7 acompanhamentos antes de encerrar o incidente. |

Escale imediatamente se algum token de substituição se aproximar de seu vencimento sem um
plano de revogação documentado.

## 4. Resposta a incidentes

- **Alertas:** serviço PagerDuty `android-telemetry-primary` cobre a redação
  falhas, interrupções do exportador e desvios de bucket. Reconhecer nas janelas do SLA
  (consulte o manual de suporte).
- **Diagnóstico:** Execute `scripts/telemetry/check_redaction_status.py` para coletar
  saúde atual do exportador, alertas recentes e métricas de autoridade com hash. Incluir
  saída na linha do tempo do incidente (`incident/YYYY-MM-DD-android-telemetry.md`).
- **Painéis:** Monitore a redação da telemetria do Android, telemetria do Android
  Painéis de Visão Geral, Conformidade de Redação e Integridade do Exportador. Capturar
  capturas de tela para registros de incidentes e anotar qualquer versão salt ou substituição
  desvios simbólicos antes de encerrar um incidente.
- **Coordenação:** Envolver a Engenharia de Liberação para questões do exportador, Compliance
  para perguntas de substituição/PII e líder do programa para incidentes de Sev 1.

### 4.1 Fluxo de escalonamento

Os incidentes do Android são triados usando os mesmos níveis de gravidade do Android
Manual de suporte (§2.1). A tabela abaixo resume quem deve ser paginado e como
espera-se que rapidamente cada respondente se junte à ponte.| Gravidade | Impacto | Respondente primário (≤5min) | Escalação secundária (≤10min) | Notificações adicionais | Notas |
|----------|--------|----------------------------|--------------------------------|--------------------------|-------|
| Sev1 | Interrupção do cliente, violação de privacidade ou vazamento de dados | Telemetria Android de plantão (`android-telemetry-primary`) | Torii plantão + Líder do Programa | Conformidade + Governança SRE (`#sre-governance`), proprietários de cluster de preparo (`#android-staging`) | Inicie a sala de guerra imediatamente e abra um documento compartilhado para registro de comandos. |
| Sev2 | Degradação da frota, uso indevido de substituição ou atraso de reprodução prolongado | Telemetria Android de plantão | Android Foundations TL + Documentos/Gerente de suporte | Líder do programa, contato de engenharia de liberação | Encaminhe para Compliance se as substituições excederem 24 horas. |
| Sev3 | Problema de inquilino único, ensaio de laboratório ou alerta consultivo | Engenheiro de suporte | Android de plantão (opcional) | Documentos/Suporte para conscientização | Converta para Sev2 se o escopo se expandir ou se vários locatários forem afetados. |

| Janela | Ação | Proprietário(s) | Evidências/Notas |
|----|--------|----------|----------------|
| 0–5min | Reconheça o PagerDuty, atribua um comandante de incidente (IC) e crie `incident/YYYY-MM-DD-android-telemetry.md`. Solte o link mais o status de uma linha em `#android-sdk-support`. | Engenheiro de suporte / SRE de plantão | Captura de tela do stub de ack + incidente do PagerDuty confirmado ao lado de outros logs de incidentes. |
| 5–15min | Execute `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` e cole o resumo no documento do incidente. Ping Android Observability TL (Haruka Yamamoto) e líder de suporte (Priya Deshpande) para transferência em tempo real. | IC + Android Observabilidade TL | Anexe o JSON de saída da CLI, anote os URLs do painel abertos e marque quem possui o diagnóstico. |
| 15–25min | Envolva os proprietários do cluster de teste (Haruka Yamamoto para observabilidade, Liam O'Connor para SRE) para reproduzir em `android-telemetry-stg`. Carregue sementes com `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` e capture dumps de fila do emulador Pixel + para confirmar a paridade dos sintomas. | Preparando proprietários de cluster | Carregue a saída `pending.queue` + `PendingQueueInspector` higienizada para a pasta de incidentes. |
| 25–40min | Decida sobre substituições, limitação Torii ou fallback do StrongBox. Se houver suspeita de exposição de PII ou hashing não determinístico, consulte Compliance (Sofia Martins, Daniel Park) via `#compliance-alerts` e notifique o Líder do Programa no mesmo tópico do incidente. | IC + Conformidade + Líder do Programa | Tokens de substituição de link, manifestos Norito e comentários de aprovação. |
| ≥40min | Fornece atualizações de status de 30 minutos (notas PagerDuty + `#android-sdk-support`). Agende a ponte da sala de guerra, se ainda não estiver ativa, documente o ETA de mitigação e garanta que a Engenharia de Liberação (Alexei Morozov) esteja em espera para rolar os artefatos do coletor/SDK. | CI | Atualizações com registro de data e hora mais registros de decisões armazenados no arquivo de incidentes e resumidos em `status.md` durante a próxima atualização semanal. |- Todos os escalonamentos devem permanecer refletidos no documento do incidente usando a tabela “Proprietário/Hora da próxima atualização” do Manual de suporte do Android.
- Se outro incidente já estiver aberto, entre na sala de guerra existente e anexe o contexto do Android em vez de criar um novo.
- Quando o incidente atingir lacunas no runbook, crie tarefas de acompanhamento no épico AND7 JIRA e marque `telemetry-runbook`.

## 5. Exercícios de caos e prontidão

- Executar os cenários detalhados em
  `docs/source/sdk/android/telemetry_chaos_checklist.md` trimestralmente e antes de
  principais lançamentos. Registre os resultados com o modelo de relatório de laboratório.
- Armazene evidências (capturas de tela, registros) em
  `docs/source/sdk/android/readiness/screenshots/`.
- Rastreie tickets de remediação no épico AND7 com rótulo `telemetry-lab`.
- Mapa de cenário: C1 (falha de redação), C2 (substituição), C3 (queda de energia do exportador), C4
  (esquema diff gate usando `run_schema_diff.sh` com uma configuração desviada), C5
  (distorção do perfil do dispositivo propagada via `generate_android_load.sh`), C6 (tempo limite Torii
  + repetição de fila), C7 (rejeição de atestado). Mantenha esta numeração alinhada com
  `telemetry_lab_01.md` e a lista de verificação do caos ao adicionar exercícios.

### 5.1 Desvio de redação e exercício de substituição (C1/C2)

1. Injete uma falha de hash via
   `scripts/telemetry/inject_redaction_failure.sh` e aguarde o PagerDuty
   alerta (`android.telemetry.redaction.failure`). Capture a saída CLI de
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` para
   o registro do incidente.
2. Limpe a falha com `--clear` e confirme se o alerta foi resolvido dentro
   10 minutos; anexe capturas de tela Grafana dos painéis salt/autoridade.
3. Crie uma solicitação de substituição assinada usando
   `docs/examples/android_override_request.json`, aplique com
   `scripts/android_override_tool.sh apply` e verifique a amostra sem hash por
   inspecionando a carga útil do exportador na preparação (procure
   `android.telemetry.redaction.override`).
4. Revogue a substituição com `scripts/android_override_tool.sh revoke --token <token>`,
   anexe o hash do token de substituição mais a referência do ticket a
   `docs/source/sdk/android/telemetry_override_log.md` e crie um resumo JSON
   sob `docs/source/sdk/android/readiness/override_logs/`. Isto fecha o
   Cenário C2 na lista de verificação do caos e mantém as evidências de governança atualizadas.

### 5.2 Queda de energia do exportador e exercício de repetição da fila (C3/C6)1. Reduza o coletor de teste (`escala kubectl
   deploy/android-otel-collector --replicas=0`) para simular um exportador
   escurecimento. Rastreie as métricas do buffer por meio da CLI de status e confirme o disparo dos alertas em
   a marca dos 15 minutos.
2. Restaure o coletor, confirme a drenagem do backlog e arquive o log do coletor
   trecho mostrando a conclusão da repetição.
3. No Pixel de teste e no emulador, siga o Cenário C6: instale
   `examples/android/operator-console`, alterne o modo avião, envie a demonstração
   transferências, desative o modo avião e observe as métricas de profundidade da fila.
4. Extraia cada fila pendente (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:classes >/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --file
   /tmp/.queue --json > queue-replay-.json`. Anexar decodificado
   envelopes mais hashes de repetição para o log do laboratório.
5. Atualize o relatório de caos com a duração da interrupção do exportador, profundidade da fila antes/depois,
   e confirmação de que `android_sdk_offline_replay_errors` permaneceu 0.

### 5.3 Preparando script de caos de cluster (android-telemetry-stg)

Proprietários de cluster de teste Haruka Yamamoto (Android Observability TL) e Liam O’Connor
(SRE) siga este roteiro sempre que um ensaio for agendado. A sequência mantém
participantes alinhados com a lista de verificação do caos da telemetria, garantindo ao mesmo tempo que
artefatos são capturados para governança.

**Participantes**

| Função | Responsabilidades | Contato |
|------|------------------|--------|
| IC de plantão Android | Conduz a broca, coordena notas do PagerDuty, possui registro de comando | PagerDuty `android-telemetry-primary`, `#android-sdk-support` |
| Preparando proprietários de cluster (Haruka, Liam) | Janelas de mudança de portão, execução de ações `kubectl`, telemetria de cluster de instantâneo | `#android-staging` |
| Gerente de documentos/suporte (Priya) | Registre evidências, rastreie a lista de verificação do laboratório e publique tickets de acompanhamento | `#docs-support` |

**Coordenação pré-voo**

- 48 horas antes do exercício, registre uma solicitação de mudança que liste o planejado
  cenários (C1–C7) e cole o link em `#android-staging` para que os proprietários do cluster
  pode bloquear implantações conflitantes.
- Colete o hash `ClientConfig` mais recente e `kubectl --context staging get pods
  -n saída android-telemetry-stg` para estabelecer o estado de linha de base e, em seguida, armazenar
  ambos sob `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- Confirme a cobertura do dispositivo (Pixel + emulador) e garanta
  `ci/run_android_tests.sh` compilou as ferramentas usadas durante o laboratório
  (`PendingQueueInspector`, injetores de telemetria).

**Pontos de verificação de execução**

- Anuncie “chaos start” em `#android-sdk-support`, inicie a gravação da ponte,
  e mantenha `docs/source/sdk/android/telemetry_chaos_checklist.md` visível para
  cada comando é narrado para o escriba.
- Faça com que um proprietário de teste espelhe cada ação do injetor (`kubectl scale`, exportador
  reinicializações, geradores de carga) para que tanto a Observabilidade quanto o SRE confirmem a etapa.
- Capture a saída de `scripts/telemetry/check_redaction_status.py
  --status-url https://android-telemetry-stg/api/redaction/status` após cada
  cenário e cole-o no documento do incidente.

**Recuperação**- Não saia da ponte até que todos os injetores sejam liberados (`inject_redaction_failure.sh --clear`,
  Os painéis `kubectl scale ... --replicas=1`) e Grafana mostram status verde.
- Docs/Support arquiva despejos de fila, logs CLI e capturas de tela em
  `docs/source/sdk/android/readiness/screenshots/<date>/` e marca o arquivo
  lista de verificação antes do fechamento da solicitação de mudança.
- Registrar tickets de acompanhamento com o rótulo `telemetry-chaos` para qualquer cenário que
  falharam ou produziram métricas inesperadas e referenciá-las em `status.md`
  durante a próxima revisão semanal.

| Tempo | Ação | Proprietário(s) | Artefato |
|------|--------|----------|----------|
| T−30min | Verifique a integridade do `android-telemetry-stg`: `kubectl --context staging get pods -n android-telemetry-stg`, confirme se não há atualizações pendentes e anote as versões do coletor. | Haruka | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20min | Carga de linha de base de sementes (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) e captura stdout. | Liam | `readiness/labs/reports/<date>/load-generator.log` |
| T−15min | Copie `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` para `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`, liste os cenários a serem executados (C1–C7) e atribua escribas. | Priya Deshpande (Suporte) | Remarcação de incidente cometida antes do início do ensaio. |
| T−10min | Confirme o emulador Pixel + online, o SDK mais recente instalado e `ci/run_android_tests.sh` compilou o `PendingQueueInspector`. | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T−5min | Inicie o Zoom Bridge, inicie a gravação da tela e anuncie o “início do caos” em `#android-sdk-support`. | IC / Documentos/Suporte | Gravação salva em `readiness/archive/<month>/`. |
| +0min | Execute o cenário selecionado de `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (normalmente C2 + C6). Mantenha o guia do laboratório visível e evoque invocações de comando à medida que elas acontecem. | Haruka dirige, Liam espelha resultados | Logs anexados ao arquivo do incidente em tempo real. |
| +15min | Faça uma pausa para coletar métricas (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) e capturar capturas de tela do Grafana. | Haruka | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25min | Restaure quaisquer falhas injetadas (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), repita filas e confirme o fechamento de alertas. | Liam | `readiness/labs/reports/<date>/recovery.log` |
| +35min | Análise: atualize o documento do incidente com aprovação/reprovação por cenário, liste acompanhamentos e envie artefatos para o git. Notifique o Docs/Support de que a lista de verificação de arquivo pode ser concluída. | CI | Documento do incidente atualizado, `readiness/archive/<month>/checklist.md` marcado. |

- Mantenha os proprietários de teste na ponte até que os exportadores estejam saudáveis ​​e todos os alertas tenham sido apagados.
- Armazene dumps de fila bruta em `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` e faça referência a seus hashes no log de incidentes.
- Se um cenário falhar, crie imediatamente um ticket JIRA denominado `telemetry-chaos` e faça link cruzado dele a partir de `status.md`.
- Auxiliar de automação: `ci/run_android_telemetry_chaos_prep.sh` agrupa o gerador de carga, os instantâneos de status e o encanamento de exportação de fila. Configure `ANDROID_TELEMETRY_DRY_RUN=false` quando o acesso temporário estiver disponível e `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (etc.) para que o script copie cada arquivo de fila, emita `<label>.sha256` e execute `PendingQueueInspector` para produzir `<label>.json`. Use `ANDROID_PENDING_QUEUE_INSPECTOR=false` somente quando a emissão JSON precisar ser ignorada (por exemplo, nenhum JDK disponível). **Sempre exporte os identificadores salt esperados antes de executar o auxiliar** definindo `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` e `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` para que as chamadas `check_redaction_status.py` incorporadas falhem rapidamente se a telemetria capturada divergir da linha de base do Rust.

## 6. Documentação e habilitação**Kit de ativação do operador:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  vincula o runbook, a política de telemetria, o guia de laboratório, a lista de verificação de arquivo e o conhecimento
  faz check-in em um único pacote pronto para AND7. Faça referência ao preparar o SRE
  pré-leituras de governança ou agendamento da atualização trimestral.
- **Sessões de capacitação:** uma gravação de capacitação de 60 minutos será executada em 18/02/2026
  com atualizações trimestrais. Os materiais vivem sob
  `docs/source/sdk/android/readiness/`.
- **Verificações de conhecimento:** A equipe deve pontuar ≥90% por meio do formulário de prontidão. Loja
  resulta em `docs/source/sdk/android/readiness/forms/responses/`.
- **Atualizações:** sempre que esquemas de telemetria, painéis ou políticas de substituição
  alterar, atualize este runbook, o playbook de suporte e `status.md` no mesmo
  RP.
- **Revisão semanal:** Após cada release candidate do Rust (ou pelo menos semanalmente), verifique
  `java/iroha_android/README.md` e este runbook ainda refletem a automação atual,
  procedimentos de rotação de acessórios e expectativas de governança. Capture a revisão em
  `status.md` para que a auditoria de marco do Foundations possa rastrear a atualização da documentação.

## 7. Arnês de atestado StrongBox- **Objetivo:** validar pacotes de atestados baseados em hardware antes de promover dispositivos no
  Piscina StrongBox (AND2/AND6). O chicote consome cadeias de certificados capturadas e as verifica
  contra raízes confiáveis usando a mesma política que o código de produção executa.
- **Referência:** Consulte `docs/source/sdk/android/strongbox_attestation_harness_plan.md` para obter a versão completa
  API de captura, ciclo de vida do alias, fiação CI/Buildkite e matriz de propriedade. Trate esse plano como o
  fonte de verdade ao integrar novos técnicos de laboratório ou atualizar artefatos financeiros/conformidade.
- **Fluxo de trabalho:**
  1. Colete um pacote de atestado no dispositivo (alias, `challenge.hex` e `chain.pem` com o
     ordem folha→raiz) e copie-o para a estação de trabalho.
  2. Execute `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` usando o apropriado
     Raiz do Google/Samsung (os diretórios permitem carregar pacotes inteiros de fornecedores).
  3. Arquive o resumo JSON junto com o material bruto de atestado em
     `artifacts/android/attestation/<device-tag>/`.
- **Formato do pacote:** Siga `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  para o layout de arquivo necessário (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **Raízes confiáveis:** Obtenha PEMs fornecidos pelo fornecedor no armazenamento de segredos do laboratório de dispositivos; passar vários
  Argumentos `--trust-root` ou aponte `--trust-root-dir` para o diretório que contém as âncoras quando
  a cadeia termina em uma âncora que não é do Google.
- **Armazenamento de CI:** Use `scripts/android_strongbox_attestation_ci.sh` para verificar pacotes arquivados em lote
  em máquinas de laboratório ou executores de CI. O script verifica `artifacts/android/attestation/**` e invoca o
  aproveitamento para cada diretório que contém os arquivos documentados, escrevendo `result.json` atualizado
  resumos em vigor.
- **CI lane:** Depois de sincronizar novos pacotes, execute a etapa Buildkite definida em
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  A tarefa executa `scripts/android_strongbox_attestation_ci.sh`, gera um resumo com
  `scripts/android_strongbox_attestation_report.py`, carrega o relatório para `artifacts/android_strongbox_attestation_report.txt`,
  e anota a construção como `android-strongbox/report`. Investigue quaisquer falhas imediatamente e
  vincule o URL de construção da matriz do dispositivo.
- **Relatórios:** anexe a saída JSON às revisões de governança e atualize a entrada da matriz do dispositivo em
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` com a data do atestado.
- **Ensaio simulado:** Quando o hardware não estiver disponível, execute `scripts/android_generate_mock_attestation_bundles.sh`
  (que usa `scripts/android_mock_attestation_der.py`) para criar pacotes de testes determinísticos mais uma raiz simulada compartilhada para que CI e documentos possam exercitar o aproveitamento de ponta a ponta.
- **Proteções no código:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` cobre vazio vs desafiado
  regeneração de atestado (metadados StrongBox/TEE) e emite `android.keystore.attestation.failure`
  na incompatibilidade de desafio, para que as regressões de cache/telemetria sejam capturadas antes do envio de novos pacotes.

## 8. Contatos

- **Engenharia de suporte de plantão:** `#android-sdk-support`
- **Governança SRE:** `#sre-governance`
- **Documentos/Suporte:** `#docs-support`
- **Árvore de escalonamento:** Consulte o manual de suporte do Android §2.1

## 9. Cenários de solução de problemasO item do roteiro AND7-P2 destaca três classes de incidentes que paginam repetidamente o
Android de plantão: Torii/tempo limite de rede, falhas de atestado do StrongBox e
Desvio de manifesto `iroha_config`. Analise a lista de verificação relevante antes de arquivar
Sev1/2 acompanha e arquiva as evidências em `incident/<date>-android-*.md`.

### 9.1 Torii e tempos limite de rede

**Sinais**

- Alertas em `android_sdk_submission_latency`, `android_sdk_pending_queue_depth`,
  `android_sdk_offline_replay_errors` e a taxa de erro Torii `/v2/pipeline`.
- Widgets `operator-console` (exemplos/android) mostrando drenagem de fila paralisada ou
  novas tentativas travadas em espera exponencial.

**Resposta imediata**

1. Confirme o PagerDuty (`android-networking`) e inicie um log de incidentes.
2. Capture instantâneos Grafana (latência de envio + profundidade da fila) cobrindo o
   últimos 30 minutos.
3. Registre o hash `ClientConfig` ativo dos logs do dispositivo (`ConfigWatcher`
   imprime o resumo do manifesto sempre que uma recarga é bem-sucedida ou falha).

**Diagnóstico**

- **Saúde da fila:** Extraia o arquivo de fila configurado de um dispositivo de armazenamento temporário ou do
  emulador (`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`). Decodifique os envelopes com
  `OfflineSigningEnvelopeCodec` conforme descrito em
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` para confirmar o
  o backlog corresponde às expectativas do operador. Anexe os hashes decodificados ao
  incidente.
- **Inventário de hash:** Depois de baixar o arquivo da fila, execute o auxiliar do inspetor
  para capturar hashes/aliases canônicos para os artefatos do incidente:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  Anexe `queue-inspector.json` e o stdout bem impresso ao incidente
  e vincule-o ao relatório do laboratório AND7 para o Cenário D.
- **Conectividade Torii:** Execute o chicote de transporte HTTP localmente para descartar o SDK
  regressões: exercícios `ci/run_android_tests.sh`
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests` e
  `ToriiMockServerTests`. Falhas aqui indicam um bug do cliente em vez de um
  Interrupção Torii.
- **Ensaio de injeção de falha:** No teste Pixel (StrongBox) e no AOSP
  emulador, alterne a conectividade para reproduzir o crescimento da fila pendente:
  `adb shell cmd connectivity airplane-mode enable` → enviar duas demonstrações
  transações via console do operador → `adb shell cmd conectividade modo avião
  desativar` → verify the queue drains and `android_sdk_offline_replay_errors`
  permanece 0. Registra hashes das transações reproduzidas.
- **Paridade de alerta:** Ao ajustar limites ou após alterações de Torii, execute
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` para que as regras Prometheus permaneçam
  alinhado com os painéis.

**Recuperação**

1. Se Torii estiver degradado, ative o Torii de plantão e continue reproduzindo o
   fila assim que `/v2/pipeline` aceitar tráfego.
2. Reconfigure os clientes afetados somente por meio de manifestos `iroha_config` assinados. O
   O observador de recarga a quente `ClientConfig` deve emitir um log de sucesso antes do incidente
   pode fechar.
3. Atualize o incidente com o tamanho da fila antes/depois da repetição mais hashes de
   quaisquer transações descartadas.

### 9.2 Falhas no StrongBox e no Atestado

**Sinais**- Alertas em `android_sdk_strongbox_success_rate` ou
  `android.keystore.attestation.failure`.
- A telemetria `android.keystore.keygen` agora registra o solicitado
  `KeySecurityPreference` e a rota utilizada (`strongbox`, `hardware`,
  `software`) com um sinalizador `fallback=true` quando uma preferência StrongBox chega
  TEE/software. As solicitações STRONGBOX_REQUIRED agora falham rapidamente em vez de silenciosamente
  retornando chaves TEE.
- Tickets de suporte referenciando dispositivos `KeySecurityPreference.STRONGBOX_ONLY`
  voltando às chaves de software.

**Resposta imediata**

1. Reconheça PagerDuty (`android-crypto`) e capture o rótulo de alias afetado
   (hash salgado) mais bucket de perfil do dispositivo.
2. Verifique a entrada da matriz de atestado do dispositivo em
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` e
   registrar a última data verificada.

**Diagnóstico**

- **Verificação do pacote:** Executar
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  no atestado arquivado para confirmar se a falha é devido ao dispositivo
  configuração incorreta ou uma mudança de política. Anexe o `result.json` gerado.
- **Regeneração de desafios:** Os desafios não são armazenados em cache. Cada solicitação de desafio regenera um novo
  atestado e caches por `(alias, challenge)`; chamadas sem desafio reutilizam o cache. Não compatível
- **Varredura CI:** Execute `scripts/android_strongbox_attestation_ci.sh` para que cada
  o pacote armazenado é revalidado; isso protege contra problemas sistêmicos introduzidos
  por novas âncoras de confiança.
- **Exercício do dispositivo:** Em hardware sem StrongBox (ou forçando o emulador),
  configure o SDK para exigir apenas o StrongBox, envie uma transação de demonstração e confirme
  o exportador de telemetria emite o evento `android.keystore.attestation.failure`
  com o motivo esperado. Repita em um Pixel compatível com StrongBox para garantir o
  o caminho feliz permanece verde.
- **Verificação de regressão do SDK:** Execute `ci/run_android_tests.sh` e pague
  atenção aos conjuntos focados em atestados (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` para separação de cache/desafio). Falhas aqui
  indicam uma regressão do lado do cliente.

**Recuperação**

1. Gere novamente pacotes de atestados se o fornecedor alternou certificados ou se o
   dispositivo recebeu recentemente um importante OTA.
2. Carregue o pacote atualizado para `artifacts/android/attestation/<device>/` e
   atualize a entrada da matriz com a nova data.
3. Se o StrongBox não estiver disponível na produção, siga o fluxo de trabalho de substituição em
   Seção3 e documente a duração do fallback; a mitigação a longo prazo requer
   substituição do dispositivo ou correção do fornecedor.

### 9.2a Recuperação Determinística de Exportação

- **Formatos:** As exportações atuais são v3 (sal/nonce por exportação + Argon2id, registradas como
- **Política de senha:** v3 impõe senhas com ≥12 caracteres. Se os usuários fornecerem prazos mais curtos
  senhas, instrua-os a reexportar com uma senha compatível; As importações v0/v1 são
  isento, mas deve ser reembalado como v3 imediatamente após a importação.
- **Proteções contra adulteração/reutilização:** Os decodificadores rejeitam comprimentos zero/sal curto ou nonce e são repetidos
  pares salt/nonce aparecem como erros `salt/nonce reuse`. Gere novamente a exportação para limpar
  o guarda; não tente forçar a reutilização.
  `SoftwareKeyProvider.importDeterministic(...)` para reidratar a chave, então
  `exportDeterministic(...)` para emitir um pacote v3 para que as ferramentas de desktop registrem o novo KDF
  parâmetros.### 9.3 Incompatibilidades de manifesto e configuração

**Sinais**

- Falhas de recarga `ClientConfig`, nomes de host Torii incompatíveis ou telemetria
  diferenças de esquema sinalizadas pela ferramenta de comparação AND7.
- Operadores relatando diferentes botões de nova tentativa/retirada em dispositivos no mesmo
  frota.

**Resposta imediata**

1. Capture o resumo `ClientConfig` impresso nos logs do Android e no
   resumo esperado do manifesto de lançamento.
2. Despeje a configuração do nó em execução para comparação:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**Diagnóstico**

- **Diferença de esquema:** Execute `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  para gerar um relatório de comparação Norito, atualize o arquivo de texto Prometheus e anexe o
  Artefato JSON mais evidências de métricas para o incidente e log de prontidão de telemetria AND7.
- **Validação de manifesto:** Use `iroha_cli runtime capabilities` (ou o tempo de execução
  comando audit) para recuperar os hashes criptográficos/ABI anunciados do nó e garantir
  eles correspondem ao manifesto móvel. Uma incompatibilidade confirma que o nó foi revertido
  sem reemitir o manifesto do Android.
- **Verificação de regressão do SDK:** `ci/run_android_tests.sh` abrange
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests` e
  `HttpClientTransportStatusTests`. Falhas sinalizam que o SDK enviado não pode
  analise o formato do manifesto atualmente implantado.

**Recuperação**

1. Gere novamente o manifesto por meio do pipeline autorizado (geralmente
   `iroha_cli runtime Capabilities` → manifesto Norito assinado → pacote de configuração) e
   reimplantá-lo através do canal do operador. Nunca edite `ClientConfig`
   substitui no dispositivo.
2. Assim que um manifesto corrigido chegar, observe o `ConfigWatcher` “recarregar ok”
   mensagem em cada nível da frota e encerrar o incidente somente após a telemetria
   esquema diff relata paridade.
3. Registre o hash do manifesto, o caminho do artefato de diferença de esquema e o link do incidente em
   `status.md` na seção Android para auditabilidade.

## 10. Currículo de capacitação do operador

O item do roteiro **AND7** requer um pacote de treinamento repetível para que os operadores,
engenheiros de suporte e o SRE podem adotar as atualizações de telemetria/edição sem
suposições. Combine esta seção com
`docs/source/sdk/android/readiness/and7_operator_enablement.md`, que contém
a lista de verificação detalhada e links de artefatos.

### 10.1 Módulos de sessão (briefing de 60 minutos)

1. **Arquitetura de telemetria (15min).** Percorra o buffer do exportador,
   filtro de redação e ferramentas de comparação de esquema. Demonstração
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` mais
   `scripts/telemetry/check_redaction_status.py` para que os participantes vejam como é a paridade
   aplicado.
2. **Runbook + laboratórios de caos (20min).** Destaque as Seções 2 a 9 deste runbook,
   ensaie um cenário de `readiness/labs/telemetry_lab_01.md` e mostre como
   para arquivar artefatos sob `readiness/labs/reports/<stamp>/`.
3. **Fluxo de trabalho de substituição + conformidade (10 minutos).** Revise as substituições da Seção 3,
   demonstrar `scripts/android_override_tool.sh` (aplicar/revogar/resumir) e
   atualizar `docs/source/sdk/android/telemetry_override_log.md` mais o mais recente
   digerir JSON.
4. **Perguntas e respostas/verificação de conhecimento (15min).** Use o cartão de referência rápida em
   `readiness/cards/telemetry_redaction_qrc.md` para ancorar perguntas, então
   capturar acompanhamentos em `readiness/and7_operator_enablement.md`.### 10.2 Cadência e proprietários de ativos

| Ativo | Cadência | Proprietário(s) | Localização do arquivo |
|-------|--------|----------|-------|
| Passo a passo gravado (Zoom/Equipes) | Trimestralmente ou antes de cada rotação de sal | Android Observability TL + Gerenciador de documentos/suporte | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (gravação + lista de verificação) |
| Apresentação de slides e cartão de referência rápida | Atualizar sempre que a política/runbook for alterada | Gerente de documentos/suporte | `docs/source/sdk/android/readiness/deck/` e `/cards/` (exportação PDF + Markdown) |
| Verificação de conhecimentos + ficha de presença | Após cada sessão ao vivo | Engenharia de suporte | Bloco de atendimento `docs/source/sdk/android/readiness/forms/responses/` e `and7_operator_enablement.md` |
| Lista de pendências de perguntas e respostas/registro de ações | Rolando; atualizado após cada sessão | LLM (atuando DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 Evidências e ciclo de feedback

- Armazene artefatos de sessão (capturas de tela, exercícios de incidentes, exportações de questionários) no
  mesmo diretório datado usado para ensaios de caos para que a governança possa auditar ambos
  faixas de prontidão juntas.
- Quando uma sessão for concluída, atualize `status.md` (seção Android) com links para
  o diretório de arquivo e anote todos os acompanhamentos abertos.
- As perguntas pendentes das perguntas e respostas ao vivo devem ser transformadas em problemas ou documentos
  pull solicitações dentro de uma semana; faça referência aos épicos do roteiro (AND7/AND8) no
  descrição do ticket para que os proprietários fiquem alinhados.
- As sincronizações SRE revisam a lista de verificação de arquivo mais o artefato de comparação de esquema listado em
  Seção 2.3 antes de declarar o currículo encerrado para o trimestre.