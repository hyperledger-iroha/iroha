---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Proцесс релиза
resumo: Запустите релизный гейт CLI/SDK, definindo a política de versão e abrindo notas canônicas релиза.
---

#Process релиза

Binários SoraFS (`sorafs_cli`, `sorafs_fetch`, ajudantes) e caixas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) verifique isso. Релизный
pipeline cria CLI e bibliotecas organizadas, обеспечивает покрытие lint/test
e фиксирует артефакты para downstream потребителей. Verifique a lista de verificação agora para
tag de candidato каждого.

## 0. Faça a assinatura da assinatura

Antes de abrir o portão de liberação, verifique a revisão de segurança dos artefatos:

- Скачайте самый свежий меморандум SF-6 по безопасности ([reports/sf6-security-review](./reports/sf6-security-review.md))
  e зафиксируйте его hash SHA256 no ticket de liberação.
- Acesse a consulta de ticket de remediação (por exemplo, `governance/tickets/SF6-SR-2026.md`) e obtenha
  aprovação do Grupo de Trabalho de Engenharia de Segurança e Ferramentas.
- Verifique esta lista de verificação de remediação no memo закрыт; Não há pontos bloqueados.
- Подготовьте загрузку логов chicote de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  вместе com manifesto do pacote.
- Убедитесь, что команда подписи, которую вы планируете выполнить, включает и `--identity-token-provider`, и
  явный `--identity-token-audience=<aud>`, seu escopo Fulcio é uma evidência confiável.

Use esses artefatos para governança corporativa e religiosidade pública.

## 1. Liberar portão de lançamento/teste

Ajuda `ci/check_sorafs_cli_release.sh` para formatar formatos, Clippy e testes
nas caixas CLI e SDK no diretório de destino local do espaço de trabalho (`.target`) чтобы избежать
Os conectores estão disponíveis para abrir os contatos do CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Скрипт выполняет следующие проверки:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com recurso `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` para caixas de papelão

Se você não tiver uma etiqueta, registre-se para marcar. Релизные сборки должны
идти непрерывно от main; Não há escolha de cereja em filmes de lançamento. Portão também
fornecer sinalizadores de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; отсутствующие аргументы валят запуск.

## 2. Versão Política de Privacidade

As caixas SoraFS CLI/SDK usam SemVer:

- `MAJOR`: Verificado com versão atual 1.0. Versão 1.0 menor `0.y`
  **Isto de quebrar a imagem** na superfície CLI ou no quadro Norito.
  para política opcional, uso de telefonia).
- `PATCH`: Исправления багов, релизы и обновления зависимостей, somente documentação,
  não é possível fazer isso.

Use `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` na versão anterior, aqui
O SDK downstream pode ser baseado em uma string de versão específica. Por exemplo:

1. Abra o `version =` no lugar `Cargo.toml`.
2. Altere `Cargo.lock` para `cargo update -p <crate>@<new-version>` (área de trabalho
   требует явных версий).
3. Se você fechar a porta de liberação, não deverá usar nenhum artefato.## 3. Notas de versão do Подготовить

A opção de registro de alterações de markdown publica o changelog com uma conta de CLI, SDK
e governança. Utilize o cabo `docs/examples/sorafs_release_notes.md` (configurado
его в директорию релизных артефактов и заполните секции конкретикой).

Valor mínimo:

- **Destaques**: Crie arquivos para usar CLI e SDK.
- **Etapas de atualização**: Comandos TL;DR para entrega e entrega de carga
  детерминированных luminárias.
- **Verificação**: хэши или envelopes вывода команд и точная ревизия
  `ci/check_sorafs_cli_release.sh`, a caixa está danificada.

Leia as notas de lançamento aqui (por exemplo, no lançamento do GitHub) e
храните рядом с детерминированно сгенерированными артефактами.

## 4. Ganchos de liberação Выполнить

Abra `scripts/release_sorafs_cli.sh`, este pacote de assinatura será gerado e
resumo de verificação, которые отгружаются с каждым релизом. Invólucro de embalagem nova
use CLI, use `sorafs_cli manifest sign` e muito mais
`manifest verify-signature`, este é o método de marcação. Exemplo:

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

Sugestões:

- Отслеживайте релизные entradas (carga útil, planos, resumos, hash de token esperado)
  Em repositórios ou configurações de implantação, este script será criado. IC
  pacote pod `fixtures/sorafs_manifest/ci_sample/` показывает канонический layout.
- Construir automação CI em `.github/workflows/sorafs-cli-release.yml`; em você
  release gate, você abre o script e arquiva pacotes/assinaturas como artefatos de fluxo de trabalho.
  Повторяйте тот же порядок команд (portão de liberação → assinar → verificar) no sistema CI других,
  Os logs de auditoria são gerenciados por hashes.
- Verifique `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` e
  `manifest.verify.summary.json` é um pacote que contém notificação de governança.
- Если релиз обновляет luminárias canônicas, скопируйте обновленный manifesto, plano de bloco e
  resumos em `fixtures/sorafs_manifest/ci_sample/` (e obter
  `docs/examples/sorafs_ci_sample/manifest.template.json`) para marcação. Operadores downstream
  Use os fixtures disponíveis para o pacote de lançamento do воспроизводимости.
- Verifique o log de um canal limitado para `sorafs_cli proof stream` e
  приложите его к релизному pacету, чтобы показать, что protege as atividades de streaming à prova.
- Verifique o `--identity-token-audience`, disponível nas notas de lançamento;
  governança сверяет audiência с политикой Fulcio перед одобрением публикации.

Use `scripts/sorafs_gateway_self_cert.sh`, sem usar o gateway de implementação.
Selecione este pacote de manifesto, aqui está o atestado fornecido pelo artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Segurança e publicidade

Verifique verificações de segurança e ganchos de segurança:1. Abra `sorafs_cli --version` e `sorafs_fetch --version`, isso é feito, aqui
   бинарники показывают новую версию.
2. Verifique a configuração de liberação em `sorafs_release.toml` na versão de controle (explicado)
   ou em outro arquivo de configuração, abra seu repositório de implantação. Избегайте
   ad-hoc переменных окружения; transferir o arquivo CLI para `--config` (ou analógico)
   чтобы liberar entradas были явными и воспроизводимыми.
3. Verifique o texto do texto (recomendado) ou o texto anotado:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (pacotes CAR, manifestos, resumos de provas, notas de lançamento,
   resultados de atestado) no registro do projeto, lista de verificação de governança из
   [guia de implantação](./developer-deployment.md). Если релиз создал новые luminárias,
   отправьте их общий fixture repo ou object store, чтобы auditoria automação moгла
   сравнить опубликованный pacote com versão de controle.
5. Verifique o canal de governança com base em tags, notas de lançamento, hashes
   pacote/manifesto de arquivo, resumos `manifest.sign/verify` e любые
   envelopes de atestado. Приложите URL CI job (ou arquivos de log), que são acessados
   `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. Governança Обновите
   ticket, чтобы аудиторы могли связать aprovações с артефактами; cogda
   `.github/workflows/sorafs-cli-release.yml` é um serviço público, disponível
   Hashes criados em resumos ad-hoc.

## 6. Destino pós-religioso

- Убедитесь, что документация, указывающая на новую версию (guias de início rápido, modelos de CI),
  обновлена, либо подтвердите отсутствие изменений.
- Заведите roteiro записи, если нужна последующая работа (por exemplo, migração флаги,
- Архивируйте логи вывода release gate для аудиторов - храните их рядом с подписанными
  artefatos.

Este pipeline contém CLI, caixas SDK e materiais de governança sincronizados
no ciclo padrão.