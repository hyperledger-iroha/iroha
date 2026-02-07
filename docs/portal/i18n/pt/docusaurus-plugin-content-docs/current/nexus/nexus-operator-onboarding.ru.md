---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: Operador de espaço de dados Sora Nexus
descrição: Зеркало `docs/source/sora_nexus_operator_onboarding.md`, отслеживающее релиза чеклист отслеживающее ponta a ponta para o operador Nexus.
---

:::nota História Canônica
Esta página contém `docs/source/sora_nexus_operator_onboarding.md`. Selecione uma cópia sincronizada que não seja exibida no portal.
:::

# Integração do espaço de dados do operador Sora Nexus

Este é um tipo de análise de ponta a ponta que permite que o operador de espaço de dados Sora Nexus possa ser usado. Para obter runbook de trilha dupla (`docs/source/release_dual_track_runbook.md`) e criar seus próprios artefatos (`docs/source/release_artifact_selection.md`), descrição, como fazer isso скачанные pacotes/imagens, manifestos e шаблоны конфигурации с глобальными ожиданиями по lanes до вывода узла on-line.

## Аудитория и предпосылки
- Você criou o programa Nexus e instalou o espaço de dados (índice de faixa, ID/alias do espaço de dados e política de roteamento de definição).
- Você está fornecendo artefatos de lançamento da Engenharia de Liberação (tarballs, imagens, manifestos, assinaturas, chaves públicas).
- Você é um engenheiro ou um profissional que produz material de classe para o papel de validador/observador (Ed25519 идентичность узла; BLS консенсусный ключ + PoP para validadores;
- Você pode enviar para pares Sora Nexus, mas você pode usar o bootstrap.

## Passo 1 - Подтвердить perfil de confiança
1. Selecione o alias ou o ID da cadeia que você deseja usar.
2. Selecione `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) na finalização da compra deste repositório. O Helper usa `release/network_profiles.toml` e exibe o perfil. Para Sora Nexus foi instalado um `iroha3`. Ele foi contratado pela Engenharia de Liberação e pela Engenharia de Liberação.
3. Verifique a versão da tag da versão anônima (exemplo `iroha3-v3.2.0`); он понадобится для загрузки artefatos e manifestos.

## Passo 2 - Artefatos de proteção e fornecimento
1. Compre o pacote `iroha3` (`<profile>-<version>-<os>.tar.zst`) e você mesmo forneça os arquivos (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json`, exceto você que está expandindo o conteúdo).
2. Verifique a solução antes da expansão:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Instale `openssl` no verificador, organize a organização e não use o dispositivo KMS.
3. Selecione `PROFILE.toml` usando tarball e manifestos JSON, que são atualizados:
   -`profile = "iroha3"`
   - Os polos `version`, `commit`, `built_at` são solicitados com autorização prévia.
   - OS/архитектура соответствуют цели деплоя.
4. Exclua a imagem do contêiner, insira o hash/assinatura para `<profile>-<version>-<os>-image.tar` e forneça o ID da imagem para `<profile>-<version>-image.json`.

## Passo 3 - Configurações de configuração dos painéis
1. Распакуйте bundle e скопируйте `config/` tudo, você pode usar esta configuração.
2. Definindo o modelo do `config/` como a configuração:
   - Instale `public_key`/`private_key` no fabricante Ed25519. Use as chaves privadas do disco, exceto as do HSM; Para obter a configuração, você está conectado ao conector HSM.
   - Instale `trusted_peers`, `network.address` e `torii.address` também, isso é feito on-line em sua interface de entrega e crie pares de bootstrap.
   - Use `client.toml` com endpoint Torii voltado para o operador (usando configuração TLS antes de qualquer necessidade) e seus dados para ferramentas operacionais.
3. Сохраняйте ID da cadeia do pacote, если только Governança явно не указала иначе - глобальная lane ожидает единый канонический identificador de cadeia.
4. Planeje uma senha com o perfil Sora: `irohad --sora --config <path>`. O carregador de configuração está configurado para SoraFS ou multi-lane, exceto a bandeira aberta.## Passo 4 - Definindo metadanos de espaço de dados e roteamento
1. Отредактируйте `config/config.toml`, чтобы раздел `[nexus]` соответствовал каталогу espaço de dados, выданному Nexus Conselho:
   - `lane_count` должен равняться общему числу pistas, включенных в текущей эпохе.
   - A mensagem em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` é compatível com o único `index`/`id` e apelidos conhecidos. Não altere as descrições globais; добавьте делегированные aliases, если совет назначил дополнительные espaços de dados.
   - Убедитесь, что каждая запись dataspace включает `fault_tolerance (f)`; комитеты lane-relay имеют размер `3f+1`.
2. Abra `[[nexus.routing_policy.rules]]`, verifique a política de segurança. Шаблон по умолчанию направляет governança инструкции на lane `1` e деплой контрактов на lane `2`; Adicione ou configure o endereço também, esse tráfego para seu espaço de dados está localizado na pista e alias. Consulte a Engenharia de Liberação antes de definir o procedimento.
3. Verifique as configurações `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Ожидается, что операторы сохраняют значения, одобренные советом; изменяйте их только при ратификации обновленной политики.
4. Verifique a configuração final da operação. O runbook de trilha dupla contém o arquivo `config.toml` (com segredo de edição) e o ticket onboard.

## Capítulo 5 - Validação da validação
1. Abra a configuração do validador de configuração antes de configurar a configuração:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   O comando permite a configuração de configuração e configuração padrão, selecionando o catálogo/marcando conexões ou genesis e config definidos.
2. Se você estiver executando o contêiner, use o comando `docker load -i <profile>-<version>-<os>-image.tar` (não забудьте `--sora`).
3. Prove os logs para a localização do espaço reservado para identificação de pista/espaço de dados. Este é o caso, veja a Etapa 4 - o produto não contém IDs de espaço reservado no painel.
4. Verifique o cenário de fumaça local (por exemplo, use `FindNetworkStatus` para definir o `iroha_cli`, verifique quais pontos de extremidade de telemetria publique `nexus_lane_state_total` e убедитесь, quais chaves de streaming podem ser roteadas ou importadas).

## Passo 6 - Corte e transferência
1. Solicite o certificado `manifest.json` e os artefactos de assinatura no bilhete de confiança, os auditores podem ser solicitados prova.
2. Verifique as operações Nexus que você usou; включите:
   - Идентичность узла (ID de peer, nomes de host, endpoint Torii).
   - Эффективные значения каталога via/espaço de dados e política de roteamento.
   - Хэши проверенных бинарников/образов.
3. Скоординируйте финальный прием pares (sementes de fofoca e pista de назначение) com `@nexus-core`. Não deixe isso acontecer, pois não há necessidade de uso; Sora Nexus требует детерминированной faixas de ocupação e manifesto de admissão regular.
4. Você pode usar runbooks na estrutura com suas substituições e atualizar a tag релиза, esta iteração de iteração стартовала с этой linha de base.

## Справочный чеклист
- [ ] O perfil pode ser usado como `iroha3`.
- [ ] Хэши и подписи bundle/image проверены.
- [ ] Ключи, адреса peers e endpoints Torii обновлены до продакшн значений.
- [ ] Catálogo de pistas/espaço de dados e política de roteamento Nexus соответствуют назначению совета.
- [ ] Configurações de validação (`irohad --sora --config ... --trace-config`) são executadas sem problemas.
- [ ] Manifestos/assinaturas criados em тикете онбординга e Ops уведомлен.

Para maior conexão do giroscópio da migração Nexus e conexão por telefone. [Notas de transição Nexus](./nexus-transition-notes).