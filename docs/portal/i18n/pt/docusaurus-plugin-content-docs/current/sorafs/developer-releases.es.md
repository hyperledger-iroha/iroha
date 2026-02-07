---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Processo de lançamento
resumo: executa o portão de lançamento do CLI/SDK, aplica a política de versão compartilhada e publica notas de lançamento canônicas.
---

# Processo de lançamento

Os binários de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) e as caixas de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) são publicados juntos. O gasoduto
de lançamento, mantenha o CLI e as bibliotecas alinhadas, garanta a cobertura de
lint/test e captura de artefatos para consumidores a jusante. Ejetar a lista de
verificação de baixo para cada candidato de tag.

## 0. Confirmar a aprovação da revisão de segurança

Antes de executar o portão técnico de lançamento, capture os artefatos mais
recentes da revisão de segurança:

- Baixe o memorando mais recente de revisão de segurança SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  e registre seu hash SHA256 no ticket de lançamento.
- Adicione o link do ticket de remediação (por exemplo, `governance/tickets/SF6-SR-2026.md`) e anota
  os aprovadores da Engenharia de Segurança e do Grupo de Trabalho de Ferramentas.
- Verifique se a lista de remediação do memorando está fechada; os itens sem resolução bloqueiam o lançamento.
- Preparar para subir os logs do arnês de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto com o pacote do manifesto.
- Confirme que o comando de firma que os aviões executam inclui tanto `--identity-token-provider` como
  um `--identity-token-audience=<aud>` explícito para que o alcance de Fulcio seja capturado na evidência de liberação.

Inclua esses artefatos para notificar o governo e publicar o lançamento.

## 1. Executar o portão de lançamento/verificar

El helper `ci/check_sorafs_cli_release.sh` executa formato, Clippy e testes
sobre as caixas de CLI e SDK com um diretório de destino local no espaço de trabalho (`.target`)
para evitar conflitos de permissões ao executar dentro dos contêineres CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script realiza as seguintes comparações:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com o recurso `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` para caixas esos mismos

Se algum passo falhar, corrija a regressão antes de marcar. Los builds de lançamento
deve ser contínuo com principal; não há necessidade de escolher as correções e as etapas de lançamento.
El gate também verificou que as bandeiras de firma sem chaves (`--identity-token-issuer`,
`--identity-token-audience`) é proporcional ao local correspondente; os argumentos
faltantes fizeram a execução falhar.

## 2. Aplicar a política de versionamento

Todas as caixas de CLI/SDK de SoraFS usam SemVer:- `MAJOR`: Se introduzir para o primer release 1.0. Antes de 1.0 o incremento
  menor `0.y` **indica mudanças de ruptura** na superfície do CLI ou nos
  esquemas Norito.
- `MINOR`: Trabalho de função sem interrupção (novos comandos/flags, novos
  campos Norito protegidos por política opcional, acréscimos de telemetria).
- `PATCH`: Correções de bugs, lançamentos individuais de documentação e atualizações de
  dependências que não alteram o comportamento observável.

Sempre mantenha `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` na mesma versão
para que os consumidores de SDK downstream possam depender de uma única cadeia
alineada. Todas as versões incrementais:

1. Atualize os campos `version =` em cada `Cargo.toml`.
2. Regenera o `Cargo.lock` através do `cargo update -p <crate>@<new-version>` (o espaço de trabalho
   exige versões explícitas).
3. Execute o portão de lançamento outra vez para garantir que não haja artefatos
   obsoletos.

## 3. Preparar notas de lançamento

Cada lançamento deve publicar um changelog em markdown que resalta mudanças que
afeta CLI, SDK e governança. Usa la plantilla en
`docs/examples/sorafs_release_notes.md` (cópia do seu diretório de artefatos de
liberar e completar as seções com detalhes concretos).

Conteúdo mínimo:

- **Destacados**: títulos de funções para consumidores de CLI e SDK.
- **Impacto**: mudanças de ruptura, atualizações de políticas, requisitos
  mínimos de gateway/nodo.
- **Pasos de atualização**: comandos TL;DR para atualizar dependências de carga e
  reejecutar luminárias deterministas.
- **Verificação**: hashes ou envelopes de saída de comandos e revisão exata
  de `ci/check_sorafs_cli_release.sh` ejetado.

Adicione as notas de lançamento completas à tag (por exemplo, o corpo do lançamento
no GitHub) e proteja-os junto com os artefatos gerados de forma determinista.

## 4. Executar os ganchos de liberação

Execute `scripts/release_sorafs_cli.sh` para gerar o pacote de firmas e o
resumo da verificação que você envia com cada liberação. O wrapper compila a CLI
quando for necessário, ligue para `sorafs_cli manifest sign` e reproduza imediatamente
`manifest verify-signature` para que as falhas apareçam antes de etiquetar.
Exemplo:

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

Conselhos:- Registrar as entradas do lançamento (payload, planos, resumos, hash do token esperado)
  em seu repositório ou configuração de despliegue para que o script seja reproduzível. O pacote
  de fixtures en `fixtures/sorafs_manifest/ci_sample/` mostra o layout canônico.
- Baseie a automatização de CI em `.github/workflows/sorafs-cli-release.yml`; ejetado
  el gate de release, invoque o script anterior e arquive bundles/firmas como
  artefatos do fluxo de trabalho. Reflita a mesma ordem de comandos (gate de release → firmar
  → verificar) em outros sistemas CI para que os registros de auditoria coincidam com os
  hashes gerados.
- Manter juntos `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  e `manifest.verify.summary.json`; forma o pacote referenciado na notificação
  de governança.
- Quando o lançamento de luminárias canônicas for atualizado, copie o manifesto atualizado,
  o plano de bloco e os resumos em `fixtures/sorafs_manifest/ci_sample/` (e atualiza
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de etiquetar.
  Os operadores downstream dependem dos fixtures versionados para reprodução
  o pacote de lançamento.
- Captura o registro de execução da verificação de canais acotados de
  `sorafs_cli proof stream` e adicionado ao pacote de lançamento para demonstrar que
  as seguranças da prova de streaming continuam ativadas.
- Registre o `--identity-token-audience` exato usado durante a firma nas
  notas de lançamento; governo verifica a audiência contra a política de Fulcio
  antes de aprovar a publicação.

Usa `scripts/sorafs_gateway_self_cert.sh` quando o lançamento também inclui um
implementação do gateway. Apunta al mismo pacote de manifesto para provar que la
atestado coincide com o artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetar e publicar

Depois que as verificações forem feitas e os ganchos forem concluídos:1. Execute `sorafs_cli --version` e `sorafs_fetch --version` para confirmar os binários
   relatar a nova versão.
2. Prepare a configuração do lançamento em um `sorafs_release.toml` versionado (preferido)
   ou em outro arquivo de configuração rastreado pelo seu repositório despliegue. Evita depende de
   variáveis de ambiente ad-hoc; passos para CLI com `--config` (ou equivalente) para que
   As entradas do release são explícitas e reproduzíveis.
3. Crie uma tag firmada (preferida) ou uma tag anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Sube los artefatos (pacotes CAR, manifestos, currículos de provas, notas de liberação,
   saídas de atestado) no registro do projeto seguindo a lista de verificação de governança
   no [guia de despliegue](./developer-deployment.md). Se o lançamento gerar novos
   fixtures, submetê-los ao repositório de fixtures compartilhado ou ao armazenamento de objetos para que
   A automatização de auditoria pode comparar o pacote publicado com o controle de
   versões.
5. Notificação ao canal de governo com links para tags firmadas, notas de lançamento, hashes
   del bundle/firmas del manifest, resumos arquivados de `manifest.sign/verify` e
   qualquer envelope de atestado. Inclui a URL do CI do trabalho (ou arquivo de logs) que
   executado `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. Atualizar
   o bilhete de governo para que os auditores possam trazer as aprovações para os
   artefatos; quando o trabalho `.github/workflows/sorafs-cli-release.yml` publico
   notificações, coloque os hashes registrados em vez de coletar currículos ad-hoc.

## 6. Seguimento posterior ao lançamento

- Certifique-se de que a documentação apresentada na nova versão (inícios rápidos, guias de CI)
  está atualizado ou confirma que não são necessárias mudanças.
- Registre entradas de roteiro se precisar de trabalho posterior (por exemplo, bandeiras de
- Arquivar os registros de saída do portão de liberação para auditoria: guardá-los junto com os
  artefatos firmados.

A seguir este pipeline mantém a CLI, as caixas do SDK e o material de governança
alinhados para cada ciclo de lançamento.