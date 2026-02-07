---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Processus de release
resumo: execute o portão de lançamento CLI/SDK, aplique a política de controle de versão compartilhada e publique as notas de lançamento canônicas.
---

# Processo de lançamento

Os binários SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) e as caixas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) estão livres em conjunto. O pipeline
de release garde le CLI e as bibliotecas alinhadas, garanta a cobertura lint/test
e capturar artefatos para os consumidores a jusante. Execute a lista de verificação
ci-dessous para cada candidato a tag.

## 0. Confirmar a validação da revista de segurança

Antes de executar a técnica de liberação do portão, capture os últimos artefatos de
revista de segurança:

- Baixe o memorando de segurança SF-6 mais recente ([reports/sf6-security-review](./reports/sf6-security-review.md))
  e registre seu hash SHA256 no ticket de liberação.
- Joignez le lien du ticket de remédiation (par ex. `governance/tickets/SF6-SR-2026.md`) et notez
  os aprovadores da Engenharia de Segurança e do Grupo de Trabalho de Ferramentas.
- Verifique se a lista de verificação de recuperação do memorando está encerrada; os elementos não resolvidos bloqueiam a liberação.
- Prepare o upload dos registros do chicote de fios (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  com o pacote de manifesto.
- Confirme que o comando de assinatura que você compõe executado inclui `--identity-token-provider` et
  um `--identity-token-audience=<aud>` explícito para capturar o escopo Fulcio nas previsões de lançamento.

Inclua esses artefatos na notificação ao governo e na publicação.

## 1. Execute o portão de lançamento/testes

O auxiliar `ci/check_sorafs_cli_release.sh` executa a formatação, Clippy e os testes
nas caixas CLI e SDK com um repertório de destino local no espaço de trabalho (`.target`)
para evitar conflitos de permissões durante a execução nos contêineres CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script efetua as seguintes afirmações:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com recurso `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` para essas mesmas caixas

Se uma etapa for reproduzida, corrija a regressão antes do marcador. As compilações de lançamento
doivent être continus com main ; ne cherry-pickez pas de correctifs dans des branch
lançamento. O portão também verifica se as bandeiras de assinatura keyless (`--identity-token-issuer`,
`--identity-token-audience`) são necessários quando necessários; fonte les arguments manquants
ouça a execução.

## 2. Aplicando a política de versionamento

Todas as caixas CLI/SDK SoraFS usando SemVer :- `MAJOR`: Introdução para a versão de estreia 1.0. Avant 1.0, le bump mineur `0.y`
  **indique as alterações em andamento** na superfície do CLI ou nos esquemas Norito.
- `MINOR`: Novas funcionalidades (novos comandos/sinalizadores, novos campeões Norito
  derrière une politique optionnelle, ajouts de télémétrie).
- `PATCH`: Correções de bugs, libera documentação exclusiva e atualizações atuais
  dependências que não modificam o comportamento observável.

Gardez sempre `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` na mesma versão
para que os consumidores SDK downstream possam depender de uma única cadeia de versão
alinhado. Lors des bumps de version :

1. Coloque agora os campos `version =` em cada `Cargo.toml`.
2. Gerencie o `Cargo.lock` via `cargo update -p <crate>@<new-version>` (o espaço de trabalho
   impor versões explícitas).
3. Relance a porta de liberação para evitar artefatos perdidos.

## 3. Preparar as notas de lançamento

Cada lançamento publica um changelog e markdown antes das alterações
impacta a CLI, o SDK e a governança. Use o modelo em
`docs/examples/sorafs_release_notes.md` (copie-o em seu repertório de artefatos
libere e complete as seções com detalhes concretos).

Conteúdo mínimo:

- **Destaques**: títulos de funcionalidades para os consumidores CLI e SDK.
- **Compatibilité**: changements cassants, upgrades de politiques, exigences minimales
  gateway/nœud.
- **Etapas de atualização**: comandos TL;DR para manter no dia as dependências de carga e
  relance os fixtures determinados.
- **Verificação**: hashes de saída ou envelopes e revisão exata de
  `ci/check_sorafs_cli_release.sh` executado.

Leia as notas de lançamento respondidas na tag (por exemplo, corpo de lançamento do GitHub) e
armazene-os no local de artefatos gerados de forma determinada.

## 4. Execute os ganchos de liberação

Execute `scripts/release_sorafs_cli.sh` para gerar o pacote de assinaturas e o
currículo de verificação publicado com cada lançamento. O wrapper constrói a CLI si
necessário, ligue para `sorafs_cli manifest sign` e retorne imediatamente
`manifest verify-signature` para restaurar as verificações antes da etiqueta. Exemplo:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Dicas:- Acompanhe as entradas de lançamento (carga útil, planos, resumos, hash de token atendido)
  em seu repositório ou configuração de implantação para manter o script reproduzível.
  O pacote CI sob `fixtures/sorafs_manifest/ci_sample/` mostra o layout canônico.
- Baseie a automação CI em `.github/workflows/sorafs-cli-release.yml`; ela executa
  o portão de lançamento, invoque o script ci-dessus e arquive pacotes/assinaturas como
  artefatos de fluxo de trabalho. Reproduza a mesma ordem de comandos (portão → assinatura →
  verificação) em outros sistemas CI para alinhar os logs de auditoria com hashes.
- Gardez `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` e outros
  Conjunto `manifest.verify.summary.json`: eles formam o pacote referenciado na
  notificação de governo.
- Quando o lançamento for realizado no dia dos jogos canônicos, copie o manifesto rafraîchi,
  o plano de partes e os resumos em `fixtures/sorafs_manifest/ci_sample/` (e mettez
  no dia `docs/examples/sorafs_ci_sample/manifest.template.json`) antes da etiqueta. Les
  operadores a jusante dependem de luminárias comprometidas para reproduzir o pacote.
- Capture o log de execução da verificação dos canais limitados de
  `sorafs_cli proof stream` e coloque-o no pacote de liberação para desmontar os
  garde-fous de proof streaming restent actifs.
- Anote l'`--identity-token-audience` exatamente utilizado durante a assinatura nas notas
  de liberação; o governo recuperou o público com a política Fulcio antes da aprovação.

Use `scripts/sorafs_gateway_self_cert.sh` quando o lançamento inclui também um lançamento
portal. Aponte para o mesmo pacote de manifesto para provar que o atestado
corresponde ao artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Marcador e editor

Depois da passagem dos cheques e do fim dos ganchos:1. Execute `sorafs_cli --version` e `sorafs_fetch --version` para confirmar que os binários
   reporte a nova versão.
2. Prepare a configuração de lançamento em uma versão `sorafs_release.toml` (preferida)
   ou outro arquivo de configuração que acompanha seu repositório de implantação. Evite a dependência
   variáveis de ambiente ad-hoc; passe os caminhos para CLI com `--config` (ou
   equivalente) para que as entradas sejam explícitas e reproduzíveis.
3. Crie uma etiqueta assinada (préféré) ou uma etiqueta anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Carregue os artefatos (pacotes CAR, manifestos, currículos de provas, notas de lançamento,
   saídas de atestado) para o registro do projeto de acordo com a lista de verificação de governo
   no [guia de implementação](./developer-deployment.md). Si la lançar um produto
   de novos fixtures, você pode ver o repositório de fixtures compartilhado ou o objeto store
   depois que a automação de auditoria pode comparar o pacote publicado no controle de origem.
5. Notifique o canal de governo com as garantias da etiqueta assinada, as notas de lançamento,
   os hashes do pacote/assinaturas do manifesto, os currículos arquivados `manifest.sign/verify`
   e todo o envelope de atestado. Inclua o URL do trabalho CI (ou o arquivo de logs) aqui
   executado `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. Mettezà
   Jour le ticket de gouvernance para que os auditores possam confiar nas aprovações
   artefatos auxiliares; quando `.github/workflows/sorafs-cli-release.yml` enviar notificações,
   coloque os hashes registrados em vez de coletar currículos ad-hoc.

## 6. Pós-lançamento do Suivi

- Certifique-se de que a documentação aponta para a nova versão (guias de início rápido, modelos CI)
  está no momento ou confirme o que alguma alteração não é necessária.
- Criar entradas de roteiro se um trabalho de acompanhamento for necessário (por exemplo, bandeiras de migração,
- Arquivar os registros de saída do portão de liberação para os auditores: armazená-los no local
  artefatos assinados.

Siga este pipeline mantendo a CLI, as caixas SDK e os elementos de governança
alinhados a cada ciclo de liberação.