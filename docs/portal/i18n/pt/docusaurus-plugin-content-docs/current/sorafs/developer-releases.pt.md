---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Processo de lançamento
resumo: Execute o portão de lançamento do CLI/SDK, aplique uma política de versionamento compartilhado e publique notas de lançamento canônicas.
---

# Processo de lançamento

Os binários de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) e as caixas de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) são entregues juntos. Ó gasoduto
de release mantem o CLI e as bibliotecas alinhadas, garantem cobertura de lint/test e
captura de artistas para consumidores a jusante. Execute um checklist abaixo para cada
marque candidato.

## 0. Confirmar aprovação da revisão de segurança

Antes de executar o portão técnico de lançamento, capture os artistas mais recentes da
revisão de segurança:

- Baixe o memorando de revisão de segurança SF-6 mais recente ([reports/sf6-security-review](./reports/sf6-security-review.md))
  e registre seu hash SHA256 no ticket de lançamento.
- Anexo o link do ticket de remediação (por exemplo, `governance/tickets/SF6-SR-2026.md`) e anota
  os aprovadores de Engenharia de Segurança e do Tooling Working Group.
- Verifique se o checklist de remediação no memorando está fechado; itens pendentes bloqueados ou liberados.
- Preparar o upload dos logs do chicote de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto com o pacote de manifesto.
- Confirme que o comando de assinatura que você vai executar inclui `--identity-token-provider` e
  um `--identity-token-audience=<aud>` explicito para capturar o escopo do Fulcio na evidência do release.

Inclui esses artistas para notificar a governança e publicar o release.

## 1. Executar o portão de lançamento/testes

O helper `ci/check_sorafs_cli_release.sh` roda formatação, Clippy e testes nos caixotes
CLI e SDK com um diretório target local ao workspace (`.target`) para evitar conflitos
de permissão para executar dentro de containers CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script faz as seguintes verificações:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com recurso `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` para essas mesmas caixas

Se algum passo falhar, corrija a regressão antes de taguear. Builds de lançamento devem
ser contínuo com main; nao faca cherry-pick de correcoes em ramos de release. Ó
gate também verifica se as bandeiras de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) foram fornecidos quando aplicável; argumentos faltando
fazer a execução falhar.

## 2. Aplicar uma política de versionamento

Todos os crates CLI/SDK da SoraFS usam SemVer:

- `MAJOR`: Introdução para o primeiro release 1.0. Antes de 1.0 ou aumento menor
  `0.y` **indica mudancas quebradas** na superfície do CLI ou nos esquemas Norito.
- `MINOR`: Trabalho de funcionalidades compatíveis para tras (novos comandos/flags, novos
  campos Norito protegidos por política opcional, adicoes de telemetria).
- `PATCH`: Correções de bugs, releases somente de documentação e atualizações de
  dependências que não mudam o comportamento observavel.Mantenha sempre `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` na mesma versão
para que os consumidores de SDK downstream possam depender de uma string única de
versão homologada. Ao atualizar versões:

1. Atualize os campos `version =` em cada `Cargo.toml`.
2. Regenere o `Cargo.lock` via `cargo update -p <crate>@<new-version>` (o espaço de trabalho
   exige versoes explicitas).
3. Rodei novamente o portão de liberação para garantir que não restassem artistas desatualizados.

## 3. Preparar notas de lançamento

Cada release deve publicar um changelog em markdown que destaca mudanças de CLI, SDK e
impacto de governança. Use o modelo em `docs/examples/sorafs_release_notes.md`
(cópia para seu diretorio de artistas de liberação e preenchimento das secas com detalhes
concretos).

Conteudo mínimo:

- **Destaques**: manchetes de funcionalidades para consumidores de CLI e SDK.
- **Compatibilidade**: mudanças quebradas, atualizações de política, requisitos mínimos
  de gateway/nodo.
- **Passos de atualização**: comandos TL;DR para atualizar dependências carga e refazer
  luminárias determinísticas.
- **Verificação**: hashes de saida ou envelopes e a revisão exata de
  `ci/check_sorafs_cli_release.sh` realizado.

Anexo as notas de release preenchidas ao tag (por exemplo, corpo de release do GitHub) e
guarde junto com os artistas gerados de forma determinística.

## 4. Executar ganchos de lançamento

Rode `scripts/release_sorafs_cli.sh` para gerar o pacote de assinatura e o resumo de
verificação que acompanham cada release. O wrapper compila a CLI quando necessário,
chama `sorafs_cli manifest sign` e imediatamente reexecuta `manifest verify-signature`
para que falhas aparecam antes da marcação. Exemplo:

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

Dicas:

- Cadastrar inputs de release (payload, planos, resumos, hash esperado do token)
  no repo ou na configuração de implantação para manter o script reproduzível. Ó
  bundle de fixtures em `fixtures/sorafs_manifest/ci_sample/` mostra o layout canônico.
- Baseie a automação de CI em `.github/workflows/sorafs-cli-release.yml`; ele roda o
  gate de release, chama o script acima e arquiva bundles/assinaturas como artefactos
  fazer fluxo de trabalho. Mantenha a mesma ordem de comandos (gate de release -> assinatura ->
  verificação) em outros sistemas CI para que os logs de auditoria batam com os hashes
  gerados.
- Mantenha `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` e
  `manifest.verify.summary.json` juntos; eles formam o pacote referenciado na
  notificação de governança.
- Quando o release atualizar fixtures canonicos, copie o manifesto atualizado, o
  chunk plan e os resumos para `fixtures/sorafs_manifest/ci_sample/` (e atualizar
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de taguear. Operadores
  downstream depende dos fixtures comprometidos para reproduzir o pacote de lançamento.
- Captura de log de execução da verificação de canais limitados de
  `sorafs_cli proof stream` e anexo ao pacote do release para demonstrar que as
  salvaguardas de prova de streaming continuam ativas.
- Cadastre o `--identity-token-audience` exato usado durante a assinatura nas
  notas de lançamento; a governanca cruza o auditório com a política de Fulcio antes de
  aprovar uma publicação.Use `scripts/sorafs_gateway_self_cert.sh` quando o release também incluir um rollout
de gateway. Aponte para o mesmo pacote de manifesto para provar que um atestado
corresponda ao candidato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag e publicação

Depois que os cheques passarem e os ganchos forem concluídos:

1. Rode `sorafs_cli --version` e `sorafs_fetch --version` para confirmar que os binários
   reportam a nova versão.
2. Prepare uma configuração de lançamento em um `sorafs_release.toml` versionado (preferido)
   ou outro arquivo de configuração rastreado pelo seu repositório de implantação. Evite depender
   de variáveis de ambiente ad-hoc; passe os caminhos para o CLI com `--config` (ou
   equivalente) para que as entradas do release sejam explícitas e reproduzíveis.
3. Crie uma tag assinada (preferida) ou anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Faca upload dos artistas (pacotes CAR, manifestos, resumos de provas, notas de lançamento,
   saídas de atestado) para o registro do projeto seguindo um checklist de governança
   não [guia de implantação](./developer-deployment.md). Se o lançamento gerou novos fixtures,
   envie-as para o repositório de fixtures compartilhado ou object store para que a automação
   de auditorias consiga comparar o pacote publicado com o controle de código.
5. Notifique o canal de governança com links para a tag aprovada, notas de lançamento, hashes
   do bundle/assinaturas do manifesto, resumos arquivados de `manifest.sign/verify` e
   quaisquer envelopes de atestado. Inclui uma URL do trabalho de CI (ou arquivo de logs)
   que rodou `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. Atualizar
   o ticket de governança para que os auditores possam rastrear as aprovações dos arquitetos;
   quando o job `.github/workflows/sorafs-cli-release.yml` publicar notificações, linke
   os hashes registrados em vez de colar resumos ad-hoc.

## 6. Pós-lançamento

- Garanta que a documentação aponte para a nova versão (quickstarts, templates de CI)
  Esteja atualizado ou confirme que não há mudança e necessidade.
- Registre entradas no roteiro se for necessário trabalho posterior (por exemplo, bandeiras
- Arquive os logs do gate de release para auditorias - guarde-os ao lado dos artistas
  assinados.

Siga este pipeline mantendo o CLI, os crates SDK e o material de governança alinhados
em cada ciclo de lançamento.