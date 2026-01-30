---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Processo de release
summary: Execute o gate de release do CLI/SDK, aplique a politica de versionamento compartilhada e publique notas de release canonicas.
---

# Processo de release

Os binarios da SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) e os crates de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sao entregues juntos. O pipeline
de release mantem o CLI e as bibliotecas alinhados, garante cobertura de lint/test e
captura artefatos para consumidores downstream. Execute a checklist abaixo para cada
tag candidato.

## 0. Confirmar aprovacao da revisao de seguranca

Antes de executar o gate tecnico de release, capture os artefatos mais recentes da
revisao de seguranca:

- Baixe o memo de revisao de seguranca SF-6 mais recente ([reports/sf6-security-review](./reports/sf6-security-review.md))
  e registre seu hash SHA256 no ticket de release.
- Anexe o link do ticket de remediacao (por exemplo, `governance/tickets/SF6-SR-2026.md`) e anote
  os aprovadores de Security Engineering e do Tooling Working Group.
- Verifique que a checklist de remediacao no memo esta fechada; itens pendentes bloqueiam o release.
- Prepare o upload dos logs do harness de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto com o bundle de manifest.
- Confirme que o comando de assinatura que voce vai executar inclui `--identity-token-provider` e
  um `--identity-token-audience=<aud>` explicito para capturar o escopo do Fulcio na evidencia do release.

Inclua esses artefatos ao notificar a governanca e publicar o release.

## 1. Executar o gate de release/testes

O helper `ci/check_sorafs_cli_release.sh` roda formatacao, Clippy e testes nos crates
CLI e SDK com um diretorio target local ao workspace (`.target`) para evitar conflitos
de permissao ao executar dentro de containers CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script faz as seguintes verificacoes:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com a feature `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` para esses mesmos crates

Se algum passo falhar, corrija a regressao antes de taguear. Builds de release devem
ser continuos com main; nao faca cherry-pick de correcoes em branches de release. O
gate tambem verifica se os flags de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) foram fornecidos quando aplicavel; argumentos faltando
fazem a execucao falhar.

## 2. Aplicar a politica de versionamento

Todos os crates CLI/SDK da SoraFS usam SemVer:

- `MAJOR`: Introduzido para o primeiro release 1.0. Antes de 1.0 o aumento menor
  `0.y` **indica mudancas quebradoras** na superficie do CLI ou nos esquemas Norito.
- `MINOR`: Trabalho de features compativel para tras (novos comandos/flags, novos
  campos Norito protegidos por politica opcional, adicoes de telemetria).
- `PATCH`: Correcoes de bugs, releases somente de documentacao e atualizacoes de
  dependencias que nao mudam o comportamento observavel.

Mantenha sempre `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` na mesma versao
para que os consumidores de SDK downstream possam depender de uma unica string de
versao alinhada. Ao atualizar versoes:

1. Atualize os campos `version =` em cada `Cargo.toml`.
2. Regenere o `Cargo.lock` via `cargo update -p <crate>@<new-version>` (o workspace
   exige versoes explicitas).
3. Rode novamente o gate de release para garantir que nao restem artefatos desatualizados.

## 3. Preparar notas de release

Cada release deve publicar um changelog em markdown que destaque mudancas de CLI, SDK e
impacto de governanca. Use o template em `docs/examples/sorafs_release_notes.md`
(copie-o para seu diretorio de artefatos de release e preencha as secoes com detalhes
concretos).

Conteudo minimo:

- **Highlights**: manchetes de features para consumidores de CLI e SDK.
- **Compatibilidade**: mudancas quebradoras, upgrades de politica, requisitos minimos
  de gateway/nodo.
- **Passos de upgrade**: comandos TL;DR para atualizar dependencias cargo e refazer
  fixtures deterministicas.
- **Verificacao**: hashes de saida ou envelopes e a revisao exata de
  `ci/check_sorafs_cli_release.sh` executada.

Anexe as notas de release preenchidas ao tag (por exemplo, corpo de release do GitHub) e
guarde junto com os artefatos gerados de forma deterministica.

## 4. Executar hooks de release

Rode `scripts/release_sorafs_cli.sh` para gerar o bundle de assinatura e o resumo de
verificacao que acompanham cada release. O wrapper compila o CLI quando necessario,
chama `sorafs_cli manifest sign` e imediatamente reexecuta `manifest verify-signature`
para que falhas aparecam antes do tagging. Exemplo:

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

- Registre inputs de release (payload, plans, summaries, hash esperado do token)
  no repo ou na configuracao de deployment para manter o script reproduzivel. O
  bundle de fixtures em `fixtures/sorafs_manifest/ci_sample/` mostra o layout canonico.
- Baseie a automacao de CI em `.github/workflows/sorafs-cli-release.yml`; ele roda o
  gate de release, chama o script acima e arquiva bundles/assinaturas como artefatos
  do workflow. Mantenha a mesma ordem de comandos (gate de release -> assinatura ->
  verificacao) em outros sistemas CI para que os logs de auditoria batam com os hashes
  gerados.
- Mantenha `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` e
  `manifest.verify.summary.json` juntos; eles formam o pacote referenciado na
  notificacao de governanca.
- Quando o release atualizar fixtures canonicos, copie o manifest atualizado, o
  chunk plan e os summaries para `fixtures/sorafs_manifest/ci_sample/` (e atualize
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de taguear. Operadores
  downstream dependem dos fixtures commitados para reproduzir o bundle de release.
- Capture o log de execucao da verificacao de bounded-channels de
  `sorafs_cli proof stream` e anexe ao pacote do release para demonstrar que as
  salvaguardas de proof streaming continuam ativas.
- Registre o `--identity-token-audience` exato usado durante a assinatura nas
  notas de release; a governanca cruza o audience com a politica de Fulcio antes de
  aprovar a publicacao.

Use `scripts/sorafs_gateway_self_cert.sh` quando o release tambem incluir um rollout
de gateway. Aponte para o mesmo bundle de manifest para provar que a attestation
corresponde ao artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag e publicacao

Depois que os checks passarem e os hooks forem concluidos:

1. Rode `sorafs_cli --version` e `sorafs_fetch --version` para confirmar que os binarios
   reportam a nova versao.
2. Prepare a configuracao de release em um `sorafs_release.toml` versionado (preferido)
   ou outro arquivo de configuracao rastreado pelo seu repo de deployment. Evite depender
   de variaveis de ambiente ad-hoc; passe os caminhos para o CLI com `--config` (ou
   equivalente) para que os inputs do release sejam explicitos e reproduziveis.
3. Crie um tag assinado (preferido) ou anotado:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Faca upload dos artefatos (bundles CAR, manifests, resumos de proofs, notas de release,
   outputs de attestation) para o registry do projeto seguindo a checklist de governanca
   no [guia de deployment](./developer-deployment.md). Se o release gerou novas fixtures,
   envie-as para o repo de fixtures compartilhado ou object store para que a automacao
   de auditoria consiga comparar o bundle publicado com o controle de codigo.
5. Notifique o canal de governanca com links para o tag assinado, notas de release, hashes
   do bundle/assinaturas do manifest, resumos arquivados de `manifest.sign/verify` e
   quaisquer envelopes de attestation. Inclua a URL do job de CI (ou arquivo de logs)
   que rodou `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. Atualize
   o ticket de governanca para que os auditores possam rastrear aprovacoes ate os artefatos;
   quando o job `.github/workflows/sorafs-cli-release.yml` publicar notificacoes, linke
   os hashes registrados em vez de colar resumos ad-hoc.

## 6. Pos-release

- Garanta que a documentacao apontando para a nova versao (quickstarts, templates de CI)
  esteja atualizada ou confirme que nenhuma mudanca e necessaria.
- Registre entradas no roadmap se for necessario trabalho posterior (por exemplo, flags
- Arquive os logs do gate de release para auditoria - guarde-os ao lado dos artefatos
  assinados.

Seguir este pipeline mantem o CLI, os crates SDK e o material de governanca alinhados
em cada ciclo de release.
