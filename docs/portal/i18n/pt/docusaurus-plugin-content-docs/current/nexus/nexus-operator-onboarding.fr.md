---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: Integração de operadores de espaço de dados Sora Nexus
descrição: Espelho de `docs/source/sora_nexus_operator_onboarding.md`, seguindo a lista de verificação de liberação de combate e combate para os operadores Nexus.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sora_nexus_operator_onboarding.md`. Gardez as duas cópias alinhadas até chegar às edições localizadas no portal.
:::

# Integração de operadores de espaço de dados Sora Nexus

Este guia captura o fluxo de combate em que os operadores de espaço de dados Sora Nexus devem seguir um após um lançamento anunciado. O runbook completo em voz dupla (`docs/source/release_dual_track_runbook.md`) e a nota de seleção de artefatos (`docs/source/release_artifact_selection.md`) em comentários descritivos alinham os pacotes/imagens carregados, os manifestos e os modelos de configuração com as atençãos globais das pistas antes de apresentar uma noeud on-line.

## Público e pré-requisitos
- Você foi aprovado pelo Programa Nexus e recuperou sua afetação de espaço de dados (índice de pista, ID/alias do espaço de dados e exigências de política de roteamento).
- Você pode acessar artefatos assinados por release publiques por Release Engineering (tarballs, imagens, manifestos, assinaturas, cles publiques).
- Você deve gerar ou coletar o material dos arquivos de produção para sua função de validador/observador (identidade do noeud Ed25519; código de consenso BLS + PoP para os validadores; além de toda a alternância de confidencialidade funcional).
- Você pode juntar os pares Sora Nexus existentes que inicializam seu noeud.

## Etapa 1 - Confirmação do perfil de lançamento
1. Identifique o alias da rede ou o ID da cadeia que você está usando.
2. Lance `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) em um checkout deste depósito. Consulte o ajudante `release/network_profiles.toml` e imprima o perfil do implantador. Para Sora Nexus a resposta doit etre `iroha3`. Para qualquer outro valor, pare e entre em contato com a Release Engineering.
3. Anote a etiqueta de referência da versão no anúncio do lançamento (por exemplo, `iroha3-v3.2.0`); você o utiliza para recuperar artefatos e manifestos.

## Etapa 2 - Recuperar e validar os artefatos
1. Descarregar o pacote `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos compatíveis (`.sha256`, opcionais `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` se você implantá-los conteneurs).
2. Validade da integridade antes do descompressor:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Substitua `openssl` pelo verificador aprovado pela organização se você utilizar um material KMS.
3. Inspecione `PROFILE.toml` no tarball e nos manifestos JSON para confirmar:
   -`profile = "iroha3"`
   - Os campeões `version`, `commit` e `built_at` correspondem ao anúncio do lançamento.
   - L'OS/architecture corresponde a votre cible de implantação.
4. Se você usar o conteúdo da imagem, repita o hash/assinatura de verificação para `<profile>-<version>-<os>-image.tar` e confirme o ID da imagem registrado em `<profile>-<version>-image.json`.

## Etapa 3 - Preparando a configuração a partir dos modelos
1. Extraia o pacote e copie `config/` do local ou a nova lira na configuração.
2. Digite os arquivos sob `config/` como os modelos:
   - Substitua `public_key`/`private_key` por seus arquivos Ed25519 de produção. Suprima os arquivos privados do disco se a fonte for removida de um HSM; atualize a configuração do ponteiro para o conector HSM.
   - Ajuste `trusted_peers`, `network.address` e `torii.address` para que eles reflitam suas interfaces acessíveis e os pares de atribuições de bootstrap.
   - Insira o atual `client.toml` com o terminal Torii cote operador (e inclui a configuração TLS se aplicável) e os identificadores fornecidos para a operação de inicialização.
3. Mantenha o ID da cadeia fornecido no pacote, exceto as instruções explícitas de Governança - a via global atende a um identificador de cadeia canônico exclusivo.
4. Planeje o descasque da noite com a bandeira do perfil Sora: `irohad --sora --config <path>`. O carregador de configuração rejeitou os parâmetros SoraFS ou multi-lane se o sinalizador estiver ausente.## Etapa 4 - Alinhar os metadados do espaço de dados e a rota
1. Edite `config/config.toml` para que a seção `[nexus]` corresponda ao catálogo de espaço de dados fornecido pelo Conselho Nexus:
   - `lane_count` iguale o total de pistas ativas na época atual.
   - Cada entrada em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` contém um `index`/`id` exclusivo e o alias convenus. Não suprima as entradas globais existentes; adicione seu alias delegados se você aconselhar um atributo de espaços de dados adicionais.
   - Certifique-se de que cada entrada do espaço de dados inclua `fault_tolerance (f)`; Os comitês lane-relay são dimensionados para `3f+1`.
2. Insira um dia `[[nexus.routing_policy.rules]]` para capturar a política que você atribuiu. O modelo por padrão encaminha as instruções de governo para a via `1` e as implementações de contratos para a via `2`; adicione ou modifique as regras para que o tráfego destinado ao seu espaço de dados seja direcionado para a faixa e o alias correto. Coordenação com Engenharia de Liberação antes de alterar a ordem das regras.
3. Reviva os arquivos `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Os operadores são censos e conservam os valores aprovados pelo conselho; ajuste-os exclusivamente se uma política for aprovada e ratificada.
4. Registre a configuração final em seu rastreador de operações. O runbook de lançamento tem uma voz dupla exigindo anexar o `config.toml` efetivo (segredos redigidos) no ticket de embarque.

## Etape 5 - Validação pré-voo
1. Execute o validador de configuração integral antes de reingressar na rede:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela imprimi la configuração resolue et echoue tot se as entradas do catálogo / roteamento forem incoerentes ou se a gênese e a configuração forem divergentes.
2. Se você implantar o conteúdo, execute o comando meme na imagem após ser carregado com `docker load -i <profile>-<version>-<os>-image.tar` (pense em incluir `--sora`).
3. Verifique os registros para anúncios nos identificadores de espaço reservado de pista/espaço de dados. Se necessário, retorne à etapa 4 - as implantações de produção não dependem dos IDs de espaço reservado livres com os modelos.
4. Execute seu procedimento smoke locale (p. ex., receba uma solicitação `FindNetworkStatus` com `iroha_cli`, confirme se os pontos de extremidade expostos por telemetria `nexus_lane_state_total` e verifique se as partes de streaming são alteradas ou importadas de acordo com as exigências).

## Etapa 6 - Corte e transferência
1. Armazene a verificação `manifest.json` e os artefatos de assinatura no bilhete de liberação para que os auditores possam reproduzir suas verificações.
2. Informe as operações Nexus que o nó está pronto para ser introduzido; inclui:
   - Identidade do nó (ID de peer, nomes de host, endpoint Torii).
   - Valores efetivos do catálogo/espaço de dados e política de rota.
   - Hashes de binários/imagens verificados.
3. Coordonnez l'admission finale des pairs (fofoca sementes e afetação de pista) com `@nexus-core`. Não se alegre com a reserva antes de receber a aprovação; Sora Nexus aplica uma ocupação determinada de pistas e requer um manifesto de admissão neste dia.
4. Após a versão online do novo, coloque seus runbooks no dia seguinte com as introduções de substituições e anote a etiqueta de lançamento para que a iteração prochaine faça parte desta linha de base.

## Checklist de referência
- [ ] Perfil de lançamento válido como `iroha3`.
- [] Hashes e assinaturas do pacote/imagem verifica.
- [] Cles, endereços de pares e endpoints Torii são um dia de produção em valor.
- [ ] Catálogo de faixas/espaço de dados e política de roteamento Nexus correspondente à afetação do conselho.
- [ ] Validador de configuração (`irohad --sora --config ... --trace-config`) sem avisos.
- [] Arquivos de manifestos/assinaturas no ticket de embarque e notificação de operações.

Para um contexto mais amplo nas fases de migração Nexus e nas tentativas de telemetria, consulte [Nexus notas de transição](./nexus-transition-notes).