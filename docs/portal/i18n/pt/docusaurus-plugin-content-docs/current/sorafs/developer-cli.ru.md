---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-cli
título: Книга рецептов CLI SoraFS
sidebar_label: Книга рецептов CLI
description: Практический разбор по задачам для консолидированной поверхности `sorafs_cli`.
---

:::nota História Canônica
:::

Консолидированная поверхность `sorafs_cli` (engradado de substituição `sorafs_car` com recurso de recurso `cli`) раскрывает Este é o caso, não é necessário para o artefato SoraFS. Use este livro de receitas, чтобы сразу перейти к типовым fluxos de trabalho; Configure-o para o gerenciamento de pipeline e runbooks para o contato operacional.

## Cargas úteis de expansão

Use `car pack` para determinar o pedaço de arquivos de carro e planos. O comando automático usa o chunker SF-1, sem nenhuma alça usada.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Pedaço de alça padrão: `sorafs.sf1@1.0.0`.
- Входные директории обходятся в лексикографическом порядке, чтобы checksums оставались стабильными между plataforma.
- JSON-резюме включает resumos de carga útil, метаданные по chunk e корневой CID, распознаваемый реестром и оркестратором.

## Сборка se manifesta

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- A opção `--pin-*` é fornecida com a lâmpada `PinPolicy` em `sorafs_manifest::ManifestBuilder`.
- Укажите `--chunk-plan`, если хотите, чтобы CLI пересчитал SHA3 digest para chunk перед отправкой; иначе он использует resumo do resumo.
- JSON é usado para carregar a carga útil Norito para comparar diffs antes da atualização.

## Подписание manifestos без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Crie tokens in-line, переменные окружения ou файловые источники.
- Добавляет provenance метаданные (`token_source`, `token_hash_hex`, digest chunk) sem JWT bruto, exceto `--include-token=true`.
- Adicionado para CI: acesse GitHub Actions OIDC, instalado `--identity-token-provider=github-actions`.

## Manifestos de manifestação em Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Выполняет Norito-декодирование provas de alias e проверяет соответствие resumo манифеста до POST em Torii.
- Пересчитывает SHA3 digest chunk из plan, чтобы предотвращать атаки на несоответствие.
- A configuração de status HTTP, cabeçalhos e cargas úteis é restaurada para auditoria posterior.

## Проверка содержимого CAR e provas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Transfira o PoR anterior e crie resumos de carga útil na configuração do manual.
- Фиксирует количества и идентификаторы, нужные при отправке provas репликации в governança.

## Provas de Потоковая телеметрия

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Генерирует NDJSON элементы для каждого переданного prova (отключите replay через `--emit-events=false`).
- Агрегирует количества успехов/ошибок, гистограммы латентности и выборочные сбои в sumário JSON, чтобы dashboards moгли строить графики без чтения логов.
- Abra o trabalho com um código desconhecido, o gateway de segurança é compatível com o local ou com a prova de PoR local (definido `--por-root-hex`) provas отклоняет. Use o código `--max-failures` e `--max-verification-failures` para repetição.
- Сегодня поддерживается PoR; PDP e PoTR переиспользуют тот же envelope после выхода SF-13/SF-14.
- `--governance-evidence-dir` fornece configuração de renderização, metadannые (carimbo de data e hora, CLI de versão, gateway de URL, manual de resumo) e arquivo de cópia em указанную директорию, чтобы pacotes de governança podem ser arхивировать доказательства prova-stream без повторного запуска.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — transfere a documentação para a bandeira.
- `docs/source/sorafs_proof_streaming.md` — provas de tela e painel Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — permite agrupar chunking, manifesto de arquivo e executar CAR.