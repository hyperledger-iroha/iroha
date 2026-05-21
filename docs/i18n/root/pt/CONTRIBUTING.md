---
lang: pt
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-08T10:55:43+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guia de contribuição

Obrigado por dedicar seu tempo para contribuir com Iroha 2!

Leia este guia para saber como você pode contribuir e quais diretrizes esperamos que você siga. Isso inclui as diretrizes sobre código e documentação, bem como nossas convenções relacionadas ao fluxo de trabalho git.

A leitura dessas diretrizes economizará seu tempo mais tarde.

## Como posso contribuir?

Existem muitas maneiras pelas quais você pode contribuir para o nosso projeto:

- Reportar [bugs](#reporting-bugs) e [vulnerabilidades](#reporting-vulnerabilities)
- [Sugerir melhorias](#suggesting-improvements) e implementá-las
- [Faça perguntas](#asking-questions) e interaja com a comunidade

Novo em nosso projeto? [Faça sua primeira contribuição](#your-first-code-contribution)!

###TL;DR

- Encontre [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240).
- Garfo [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Corrija o problema de sua escolha.
- Certifique-se de seguir nossos [guias de estilo](#style-guides) para código e documentação.
- Escreva [testes](https://doc.rust-lang.org/cargo/commands/cargo-test.html). Certifique-se de que todos sejam aprovados (`cargo test --workspace`). Se você tocar na pilha de criptografia SM, execute também `cargo test -p iroha_crypto --features "sm sm_proptest"` para executar o chicote opcional de fuzz/propriedade.
  - Nota: Os testes que exercitam o executor IVM sintetizarão automaticamente um bytecode mínimo e determinístico do executor se `defaults/executor.to` não estiver presente. Nenhuma pré-etapa é necessária para executar testes. Para gerar o bytecode canônico para paridade, você pode executar:
    -`cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    -`cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- Se você alterar as caixas de derivação/proc-macro, execute os conjuntos de UI trybuild via
  `make check-proc-macro-ui` (ou
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) e atualize
  `.stderr` é acionado quando o diagnóstico muda para manter as mensagens estáveis.
- Execute `make dev-workflow` (wrapper em torno de `scripts/dev_workflow.sh`) para executar fmt/clippy/build/test com `--locked` mais `swift test`; espere que `cargo test --workspace` leve horas e use `--skip-tests` apenas para loops locais rápidos. Consulte `docs/source/dev_workflow.md` para obter o runbook completo.
- Aplicar proteções com `make check-agents-guardrails` para bloquear edições `Cargo.lock` e novas caixas de espaço de trabalho, `make check-dependency-discipline` para falhar em novas dependências, a menos que explicitamente permitido, e `make check-missing-docs` para evitar novos calços `#[allow(missing_docs)]`, documentos ausentes em nível de caixa em caixas tocadas ou novos itens públicos sem comentários do documento (o guarda atualiza `docs/source/agents/missing_docs_inventory.{json,md}` via `scripts/inventory_missing_docs.py`). Adicione `make check-tests-guard` para que as funções alteradas falhem, a menos que os testes de unidade façam referência a elas (blocos `#[cfg(test)]`/`#[test]` embutidos ou caixa `tests/`; contagens de cobertura existentes) e `make check-docs-tests-metrics` para que as alterações do roteiro sejam combinadas com documentos, testes e métricas/painéis. Mantenha a aplicação do TODO via `make check-todo-guard` para que os marcadores TODO não sejam eliminados sem os documentos/testes que os acompanham. `make check-env-config-surface` regenera o inventário de alternância de ambiente e agora falha quando novos shims de ambiente de **produção** aparecem em relação a `AGENTS_BASE_REF`; defina `ENV_CONFIG_GUARD_ALLOW=1` somente após documentar adições intencionais no rastreador de migração. `make check-serde-guard` atualiza o inventário serde e falha em snapshots obsoletos ou em novas ocorrências de produção `serde`/`serde_json`; defina `SERDE_GUARD_ALLOW=1` apenas com um plano de migração aprovado. Mantenha grandes adiamentos visíveis por meio de trilhas de navegação TODO e tickets de acompanhamento, em vez de adiar silenciosamente. Execute `make check-std-only` para capturar `no_std`/`wasm32` cfgs e `make check-status-sync` para garantir que os itens abertos `roadmap.md` permaneçam somente abertos e que as alterações de roteiro/status ocorram juntas; defina `STATUS_SYNC_ALLOW_UNPAIRED=1` apenas para correções raras de erros de digitação somente de status após fixar `AGENTS_BASE_REF`. Para uma única invocação, use `make agents-preflight` para executar todas as proteções juntas.
- Execute guardas de serialização local antes de enviar: `make guards`.
  - Isso nega `serde_json` direto no código de produção, não permite novos deps serde diretos fora da lista de permissões e evita auxiliares AoS/NCB ad hoc fora de `crates/norito`.
- Opcionalmente, execute a matriz de recursos Norito localmente: `make norito-matrix` (usa um subconjunto rápido).
  - Para cobertura total, execute `scripts/run_norito_feature_matrix.sh` sem `--fast`.
  - Para incluir fumaça downstream por combo (caixa padrão `iroha_data_model`): `make norito-matrix-downstream` ou `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- Para caixas proc-macro, adicione um chicote de interface do usuário `trybuild` (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) e confirme o diagnóstico `.stderr` para os casos com falha. Mantenha o diagnóstico estável e sem pânico; atualize os equipamentos com `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` e proteja-os com `cfg(all(feature = "trybuild-tests", not(coverage)))`.
- Execute a rotina de pré-confirmação, como formatação e regeneração de artefatos (consulte [`pre-commit.sample`](./hooks/pre-commit.sample))
- Com o `upstream` definido para rastrear [repositório Hyperledger Iroha] (https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, `git commit -s`, `git push <your-fork>` e [criar um pull request](https://github.com/hyperledger-iroha/iroha/compare) para a ramificação `main`. Certifique-se de seguir as [diretrizes de solicitação pull](#pull-request-etiquette).

### Início rápido do fluxo de trabalho de AGENTES

- Execute `make dev-workflow` (wrapper em torno de `scripts/dev_workflow.sh`, documentado em `docs/source/dev_workflow.md`). Ele envolve `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (os testes podem levar várias horas) e `swift test`.
- Use `scripts/dev_workflow.sh --skip-tests` ou `--skip-swift` para iterações mais rápidas; execute novamente a sequência completa antes de abrir uma solicitação pull.
- Guardrails: evite tocar em `Cargo.lock`, adicionar novos membros do espaço de trabalho, introduzir novas dependências, adicionar novos calços `#[allow(missing_docs)]`, omitir documentos em nível de caixa, pular testes ao alterar funções, eliminar marcadores TODO sem documentos/testes ou reintroduzir cfgs `no_std`/`wasm32` sem aprovação. Execute `make check-agents-guardrails` (ou `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) mais `make check-dependency-discipline`, `make check-missing-docs` (atualiza `docs/source/agents/missing_docs_inventory.{json,md}`), `make check-tests-guard` (falha quando as funções de produção mudam sem evidência de teste de unidade - os testes mudam no diff ou os testes existentes devem fazer referência ao função), `make check-docs-tests-metrics` (falha quando as alterações do roteiro não possuem atualizações de documentos/testes/métricas), `make check-todo-guard`, `make check-env-config-surface` (falha em inventários obsoletos ou novas alternâncias de ambiente de produção; substituir por `ENV_CONFIG_GUARD_ALLOW=1` somente após atualizar os documentos) e `make check-serde-guard` (falha em estoques obsoletos de serde ou novos hits de serde de produção; substituir por `SERDE_GUARD_ALLOW=1` apenas com um plano de migração aprovado) localmente para sinal antecipado, `make check-std-only` para a proteção somente padrão e manter `roadmap.md`/`status.md` em sincronia com `make check-status-sync` (definido `STATUS_SYNC_ALLOW_UNPAIRED=1` apenas para raras correções de erros de digitação somente de status após fixar `AGENTS_BASE_REF`). Use `make agents-preflight` se desejar que um único comando execute todos os guardas antes de abrir um PR.

### Relatando Bugs

Um *bug* é um erro, falha de design, falha ou falha em Iroha que faz com que ele produza um resultado ou comportamento incorreto, inesperado ou não intencional.

Rastreamos bugs Iroha por meio de [Problemas do GitHub](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) rotulados com a tag `Bug`.

Quando você cria um novo problema, há um modelo para você preencher. Aqui está a lista de verificação do que você deve fazer ao relatar bugs:
- [] Adicione a tag `Bug`
- [ ] Explique o problema
- [] Forneça um exemplo mínimo de trabalho
- [] Anexe uma captura de tela

<details> <summary>Exemplo mínimo de trabalho</summary>

Para cada bug, você deve fornecer um [exemplo mínimo de trabalho](https://en.wikipedia.org/wiki/Minimal_working_example). Por exemplo:

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</detalhes>

---
**Observação:** Problemas como documentação desatualizada, documentação insuficiente ou solicitações de recursos devem usar os rótulos `Documentation` ou `Enhancement`. Eles não são insetos.

---

### Relatório de vulnerabilidades

Embora sejamos proativos na prevenção de problemas de segurança, é possível que você encontre uma vulnerabilidade de segurança antes de nós.

- Antes da primeira versão principal (2.0) todas as vulnerabilidades são consideradas bugs, então sinta-se à vontade para enviá-las como bugs [seguindo as instruções acima](#reporting-bugs).
- Após o primeiro lançamento principal, use nosso [programa de recompensas por bugs](https://hackerone.com/hyperledger) para enviar vulnerabilidades e receber sua recompensa.

:exclamação: Para minimizar os danos causados ​​por uma vulnerabilidade de segurança não corrigida, você deve divulgar a vulnerabilidade diretamente para Hyperledger o mais rápido possível e **evitar divulgar a mesma vulnerabilidade publicamente** por um período de tempo razoável.

Se você tiver alguma dúvida sobre como lidar com vulnerabilidades de segurança, sinta-se à vontade para entrar em contato com qualquer um dos mantenedores atualmente ativos nas mensagens privadas do Rocket.Chat.

### Sugerindo melhorias

Crie [um problema](https://github.com/hyperledger-iroha/iroha/issues/new) no GitHub com as tags apropriadas (`Optimization`, `Enhancement`) e descreva a melhoria que você está sugerindo. Você pode deixar essa ideia para nós ou para outra pessoa desenvolvermos, ou você mesmo pode implementá-la.

Se você pretende implementar a sugestão sozinho, faça o seguinte:

1. Atribua a si mesmo o problema que você criou **antes** de começar a trabalhar nele.
2. Trabalhe no recurso sugerido e siga nossas [diretrizes para código e documentação](#style-guides).
3. Quando estiver pronto para abrir uma solicitação pull, certifique-se de seguir as [diretrizes de solicitação pull](#pull-request-etiquette) e marque-a como implementando o problema criado anteriormente:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. Se sua alteração exigir uma alteração de API, use a tag `api-changes`.

   **Observação:** recursos que exigem alterações de API podem levar mais tempo para serem implementados e aprovados, pois exigem que os criadores da biblioteca Iroha atualizem seu código.### Fazendo perguntas

Uma pergunta é qualquer discussão que não seja um bug, nem um recurso ou solicitação de otimização.

<detalhes> <resumo> Como faço uma pergunta? </resumo>

Por favor, poste suas perguntas em [uma de nossas plataformas de mensagens instantâneas](#contact) para que a equipe e os membros da comunidade possam ajudá-lo em tempo hábil.

Você, como parte da comunidade mencionada, também deve considerar ajudar outras pessoas. Se você decidir ajudar, faça-o de maneira [respeitosa](CODE_OF_CONDUCT.md).

</detalhes>

## Sua primeira contribuição de código

1. Encontre um problema amigável para iniciantes entre os problemas com o rótulo [bom primeiro problema] (https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue).
2. Certifique-se de que ninguém mais esteja trabalhando nas questões que você escolheu, verificando se elas não foram atribuídas a ninguém.
3. Atribua o problema a si mesmo para que outras pessoas possam ver que alguém está trabalhando nele.
4. Leia nosso [Guia de estilo Rust](#rust-style-guide) antes de começar a escrever código.
5. Quando estiver pronto para confirmar suas alterações, leia as [diretrizes de pull request](#pull-request-etiquette).

## Etiqueta de solicitação pull

Por favor, [fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) o [repositório](https://github.com/hyperledger-iroha/iroha/tree/main) e [crie um feature branch](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) por suas contribuições. Ao trabalhar com **PRs de garfos**, consulte [este manual](https://help.github.com/articles/checking-out-pull-requests-locally).

#### Trabalhando na contribuição do código:
- Siga o [Guia de Estilo Rust](#rust-style-guide) e o [Guia de Estilo de Documentação](#documentation-style-guide).
- Certifique-se de que o código que você escreveu seja coberto por testes. Se você corrigiu um bug, transforme o exemplo mínimo funcional que reproduz o bug em um teste.
- Ao tocar em caixas de derivação/proc-macro, execute `make check-proc-macro-ui` (ou
  filtrar com `PROC_MACRO_UI_CRATES="crate1 crate2"`) então tente construir equipamentos de UI
  permaneçam sincronizados e os diagnósticos permaneçam estáveis.
- Documente novas APIs públicas (`//!` e `///` em nível de caixa em novos itens) e execute
  `make check-missing-docs` para verificar o guarda-corpo. Chame os documentos/testes que você
  adicionado na descrição da sua solicitação pull.

#### Comprometendo seu trabalho:
- Siga o [Guia de Estilo Git](#git-workflow).
- Esmague seus commits [antes](https://www.git-tower.com/learn/git/faq/git-squash/) ou [durante a mesclagem](https://rietta.com/blog/github-merge-types/).
- Se durante a preparação do seu pull request sua branch ficou desatualizada, faça rebase localmente com `git pull --rebase upstream main`. Alternativamente, você pode usar o menu suspenso do botão `Update branch` e escolher a opção `Update with rebase`.

  No interesse de tornar esse processo mais fácil para todos, tente não ter mais do que alguns commits para uma solicitação pull e evite reutilizar ramificações de recursos.

#### Criando uma solicitação pull:
- Use uma descrição apropriada da solicitação pull seguindo as orientações na seção [Etiqueta da solicitação pull](#pull-request-etiquette). Evite desviar-se dessas diretrizes, se possível.
- Adicione um [título da solicitação pull] formatado apropriadamente (#pull-request-titles).
- Se você acha que seu código não está pronto para mesclar, mas deseja que os mantenedores o examinem, crie um rascunho de pull request.

#### Mesclando seu trabalho:
- Uma solicitação pull deve passar por todas as verificações automatizadas antes de ser mesclada. No mínimo, o código deve ser formatado, passando em todos os testes, além de não ter lints `clippy` pendentes.
- Uma solicitação pull não pode ser mesclada sem duas revisões de aprovação dos mantenedores ativos.
- Cada solicitação pull notificará automaticamente os proprietários do código. Uma lista atualizada dos mantenedores atuais pode ser encontrada em [MAINTAINERS.md](MAINTAINERS.md).

#### Etiqueta de revisão:
- Não resolva uma conversa sozinho. Deixe o revisor tomar uma decisão.
- Reconhecer os comentários da revisão e interagir com o revisor (concordar, discordar, esclarecer, explicar, etc.). Não ignore os comentários.
- Para sugestões simples de alteração de código, se você aplicá-las diretamente, poderá resolver a conversa.
- Evite sobrescrever seus commits anteriores ao enviar novas alterações. Ofusca o que mudou desde a última revisão e força o revisor a começar do zero. Os commits são esmagados antes de serem mesclados automaticamente.

### Títulos de solicitação pull

Analisamos os títulos de todas as solicitações pull mescladas para gerar changelogs. Também verificamos se o título segue a convenção por meio da verificação *`check-PR-title`*.

Para passar na verificação *`check-PR-title`*, o título da solicitação pull deve seguir as seguintes diretrizes:

<details> <summary> Expanda para ler as diretrizes detalhadas do título</summary>

1. Siga o formato [commits convencionais](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers).

2. Se a solicitação pull tiver um único commit, o título do PR deverá ser igual à mensagem do commit.

</detalhes>

### Fluxo de trabalho Git

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) o [repositório](https://github.com/hyperledger-iroha/iroha/tree/main) e [criar um branch de recursos](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) por suas contribuições.
- [Configure o controle remoto](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) para sincronizar seu fork com o [repositório Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Use o [fluxo de trabalho de rebase do Git](https://git-rebase.io/). Evite usar `git pull`. Use `git pull --rebase`.
- Use os [git hooks](./hooks/) fornecidos para facilitar o processo de desenvolvimento.

Siga estas diretrizes de commit:

- **Assine cada commit**. Caso contrário, [DCO](https://github.com/apps/dco) não permitirá a mesclagem.

  Use `git commit -s` para adicionar automaticamente `Signed-off-by: $NAME <$EMAIL>` como a linha final da sua mensagem de commit. Seu nome e e-mail devem ser os mesmos especificados em sua conta GitHub.

  Também encorajamos você a assinar seus commits com a chave GPG usando `git commit -sS` ([saiba mais](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)).

  Você pode usar [o gancho `commit-msg`](./hooks/) para assinar automaticamente seus commits.

- As mensagens de commit devem seguir [commits convencionais](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) e o mesmo esquema de nomenclatura de [títulos de pull request](#pull-request-titles). Isso significa:
  - **Use o presente** ("Adicionar recurso", não "Recurso adicionado")
  - **Use o modo imperativo** ("Implantar no docker..." e não "Implantar no docker...")
- Escreva uma mensagem de commit significativa.
- Tente manter uma mensagem de commit curta.
- Se você precisar de uma mensagem de commit mais longa:
  - Limite a primeira linha da sua mensagem de commit a 50 caracteres ou menos.
  - A primeira linha da sua mensagem de commit deve conter o resumo do trabalho que você realizou. Se precisar de mais de uma linha, deixe uma linha em branco entre cada parágrafo e descreva as alterações no meio. A última linha deve ser a assinatura.
- Se você modificar o esquema (verifique gerando o esquema com `kagami schema` e diff), deverá fazer todas as alterações no esquema em um commit separado com a mensagem `[schema]`.
- Tente manter um commit por mudança significativa.
  - Se você corrigiu vários problemas em um PR, forneça commits separados.
  - Conforme mencionado anteriormente, as alterações no `schema` e na API devem ser feitas em commits apropriados, separados do resto do seu trabalho.
  - Adicione testes de funcionalidade no mesmo commit dessa funcionalidade.

## Testes e benchmarks

- Para executar os testes baseados em código-fonte, execute [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) na raiz Iroha. Observe que este é um processo longo.
- Para executar benchmarks, execute [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) a partir da raiz Iroha. Para ajudar a depurar saídas de benchmark, defina a variável de ambiente `debug_assertions` assim: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- Se você estiver trabalhando em um componente específico, lembre-se de que ao executar `cargo test` em um [espaço de trabalho](https://doc.rust-lang.org/cargo/reference/workspaces.html), ele executará apenas os testes para esse espaço de trabalho, que geralmente não inclui nenhum [teste de integração](https://www.testingxperts.com/blog/what-is-integration-testing).
- Se você quiser testar suas alterações em uma rede mínima, o [`docker-compose.yml`](defaults/docker-compose.yml) fornecido cria uma rede de 4 pares Iroha em contêineres docker que podem ser usados ​​para testar o consenso e a lógica relacionada à propagação de ativos. Recomendamos interagir com essa rede usando [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) ou a CLI do cliente Iroha incluída.
- Não remova testes com falha. Mesmo os testes ignorados serão executados em nosso pipeline eventualmente.
- Se possível, compare seu código antes e depois de fazer as alterações, pois uma regressão significativa de desempenho pode interromper as instalações dos usuários existentes.

### Verificações de proteção de serialização

Execute `make guards` para validar as políticas do repositório localmente:

- Lista de negação direta `serde_json` em fontes de produção (prefira `norito::json`).
- Proibir dependências/importações diretas `serde`/`serde_json` fora da lista de permissões.
- Impedir a reintrodução de auxiliares AoS/NCB ad hoc fora de `crates/norito`.

### Testes de depuração

<details> <summary> Expanda para saber como alterar o nível de log ou gravar logs em um JSON.</summary>

Se um dos seus testes estiver falhando, talvez você queira diminuir o nível máximo de log. Por padrão, Iroha registra apenas mensagens de nível `INFO`, mas mantém a capacidade de produzir logs de nível `DEBUG` e `TRACE`. Essa configuração pode ser alterada usando a variável de ambiente `LOG_LEVEL` para testes baseados em código ou usando o terminal `/configuration` em um dos peers em uma rede implementada.Embora os logs impressos no `stdout` sejam suficientes, você pode achar mais conveniente produzir logs formatados em `json` em um arquivo separado e analisá-los usando [node-bunyan](https://www.npmjs.com/package/bunyan) ou [rust-bunyan](https://crates.io/crates/bunyan).

Defina a variável de ambiente `LOG_FILE_PATH` em um local apropriado para armazenar os logs e analisá-los usando os pacotes acima.

</detalhes>

### Depuração usando o Tokio Console

<details> <summary> Expanda para aprender como compilar Iroha com suporte para console Tokio.</summary>

Às vezes pode ser útil para depuração analisar tarefas do Tokio usando [tokio-console](https://github.com/tokio-rs/console).

Neste caso você deve compilar Iroha com suporte ao console tokio assim:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

A porta para console tokio pode ser configurada através do parâmetro de configuração `LOG_TOKIO_CONSOLE_ADDR` (ou variável de ambiente).
O uso do Tokio Console exige que o nível de log seja `TRACE`, pode ser habilitado através do parâmetro de configuração ou variável de ambiente `LOG_LEVEL`.

Exemplo de execução de Iroha com suporte para Tokio Console usando `scripts/test_env.sh`:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</detalhes>

### Perfil

<detalhes> <resumo> Expanda para saber como criar o perfil de Iroha. </resumo>

Para otimizar o desempenho, é útil criar o perfil Iroha.

Atualmente, as compilações de criação de perfil exigem um conjunto de ferramentas noturno. Para preparar um, compile Iroha com o perfil `profiling` e recurso usando `cargo +nightly`:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

Em seguida, inicie Iroha e anexe o criador de perfil de sua escolha ao pid Iroha.

Alternativamente, é possível construir Iroha dentro do docker com suporte a profiler e criar o perfil Iroha desta forma.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

Por exemplo usando perf (disponível apenas no Linux):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

Para poder observar o perfil do executor durante a criação de perfil Iroha, o executor deve ser compilado sem remover símbolos.
Isso pode ser feito executando:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

Com o recurso de criação de perfil habilitado, Iroha expõe o endpoint a perfis pprof descartados:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</detalhes>

## Guias de estilo

Siga estas diretrizes ao fazer contribuições de código para nosso projeto:

### Guia de estilo Git

:book: [Leia as diretrizes do git](#git-workflow)

### Guia de estilo de ferrugem

<details> <summary> :book: Leia as diretrizes do código</summary>

- Use `cargo fmt --all` (edição 2024) para formatar o código.

Diretrizes do código:

- Salvo especificação em contrário, consulte [Práticas recomendadas para ferrugem](https://github.com/mre/idiomatic-rust).
- Use o estilo `mod.rs`. [Módulos autodenominados](https://rust-lang.github.io/rust-clippy/master/) não passarão na análise estática, exceto como testes [`trybuild`](https://crates.io/crates/trybuild).
- Use uma estrutura de módulos que prioriza o domínio.

  Exemplo: não faça `constants::logger`. Em vez disso, inverta a hierarquia, colocando primeiro o objeto para o qual ela é usada: `iroha_logger::constants`.
- Use [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) com uma mensagem de erro explícita ou prova de infalibilidade em vez de `unwrap`.
- Nunca ignore um erro. Se você não consegue `panic` e não consegue recuperar, pelo menos precisa ser registrado no log.
- Prefira devolver um `Result` em vez de `panic!`.
- Agrupar funcionalidades relacionadas espacialmente, de preferência dentro de módulos apropriados.

  Por exemplo, em vez de ter um bloco com definições `struct` e depois `impl`s para cada estrutura individual, é melhor ter os `impl`s relacionados a esse `struct` próximos a ele.
- Declarar antes da implementação: instruções e constantes `use` na parte superior, testes unitários na parte inferior.
- Tente evitar instruções `use` se o nome importado for usado apenas uma vez. Isso torna mais fácil mover seu código para um arquivo diferente.
- Não silencie os fiapos `clippy` indiscriminadamente. Se sim, explique seu raciocínio com um comentário (ou mensagem `expect`).
- Prefira `#[outer_attribute]` a `#![inner_attribute]` se algum deles estiver disponível.
- Se sua função não sofrer mutação em nenhuma de suas entradas (e não deveria alterar mais nada), marque-a como `#[must_use]`.
- Evite `Box<dyn Error>` se possível (preferimos digitação forte).
- Se sua função for getter/setter, marque-a como `#[inline]`.
- Se sua função for um construtor (ou seja, está criando um novo valor a partir dos parâmetros de entrada e chama `default()`), marque-a como `#[inline]`.
- Evite vincular seu código a estruturas de dados concretas; `rustc` é inteligente o suficiente para transformar um `Vec<InstructionExpr>` em `impl IntoIterator<Item = InstructionExpr>` e vice-versa quando necessário.

Diretrizes de nomenclatura:
- Use apenas palavras completas em nomes de estruturas, variáveis, métodos, características, constantes e módulos *públicos*. No entanto, abreviaturas são permitidas se:
  - O nome é local (por exemplo, argumentos de encerramento).
  - O nome é abreviado pela convenção Rust (por exemplo, `len`, `typ`).
  - O nome é uma abreviatura aceita (por exemplo, `tx`, `wsv` etc); consulte o [glossário do projeto](https://docs.iroha.tech/reference/glossary.html) para abreviações canônicas.
  - O nome completo teria sido sombreado por uma variável local (por exemplo, `msg <- message`).
  - O nome completo tornaria o código complicado com mais de 5 a 6 palavras (por exemplo, `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- Se você alterar as convenções de nomenclatura, certifique-se de que o novo nome escolhido seja _muito_ mais claro do que o que tínhamos antes.

Diretrizes para comentários:
- Ao escrever comentários não relacionados a documentos, em vez de descrever *o que* sua função faz, tente explicar *por que* ela faz algo de uma maneira específica. Isso economizará tempo para você e para o revisor.
- Você pode deixar marcadores `TODO` no código, desde que faça referência a um problema criado para ele. Não criar um problema significa que ele não será mesclado.

Usamos dependências fixadas. Siga estas diretrizes para controle de versão:

- Se o seu trabalho depende de um determinado crate, veja se ele já não foi instalado usando [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (use `bat` ou `grep`), e tente usar essa versão, em vez da versão mais recente.
- Use a versão completa "X.Y.Z" em `Cargo.toml`.
- Forneça versões atualizadas em um PR separado.

</detalhes>

### Guia de estilo de documentação

<details> <summary> :book: Leia as diretrizes da documentação</summary>


- Use o formato [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- Prefira a sintaxe de comentário de linha única. Use `///` acima dos módulos inline e `//!` para módulos baseados em arquivo.
- Se você puder vincular aos documentos de uma estrutura/módulo/função, faça-o.
- Se você puder dar um exemplo de uso, faça-o. Isto [também é um teste](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- Se uma função apresentar erro ou pânico, evite verbos modais. Exemplo: `Fails if disk IO fails` em vez de `Can possibly fail, if disk IO happens to fail`.
- Se uma função puder apresentar erro ou entrar em pânico por mais de um motivo, use uma lista com marcadores de condições de falha, com as variantes `Error` apropriadas (se houver).
- Funções *fazem* coisas. Use o modo imperativo.
- Estruturas *são* coisas. Vá direto ao ponto. Por exemplo, `Log level for reloading from the environment` é melhor que `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`.
- Estruturas possuem campos, que também *são* coisas.
- Módulos *contêm* coisas, e nós sabemos disso. Vá direto ao ponto. Exemplo: use `Logger-related traits.` em vez de `Module which contains logger-related logic`.


</detalhes>

## Contato

Os membros da nossa comunidade são ativos em:

| Serviço | Ligação |
|---------------|--------------------------------------------------------------------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| Lista de discussão | https://lists.lfdecentralizedtrust.org/g/iroha |
| Telegrama | https://t.me/hyperledgeriroha |
| Discórdia | https://discord.com/channels/905194001349627914/905205848547155968 |

---