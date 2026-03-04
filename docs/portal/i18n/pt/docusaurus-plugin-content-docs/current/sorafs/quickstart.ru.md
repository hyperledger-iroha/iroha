---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрый старт SoraFS

Este é um método prático para determinar o perfil do perfil SF-1,
подпись манифестов и поток выборки от нескольких провайдеров, которые лежат в основе
interface de conversão SoraFS. Dobre o ego
[Exibição de manual de conversão](manifest-pipeline.md)
para definir o design e expandir a CLI.

## Treino

- Тулчейн Rust (`rustup update`), área de trabalho clonada localmente.
- Opcional: [para a chave Ed25519, compatível com OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para os manifestos.
- Opcional: Node.js ≥ 18, exceto planejamento pré-programado pelo portal Docusaurus.

Instale `export RUST_LOG=info` em seus experimentos, verifique se está tudo bem
CLI.

## 1. Determinar as configurações

Сгенерируйте канонические vectorы чанкинга SF-1. Команда также выпускает подписанные
conversor de manifesto por meio de `--signing-key`; usar `--allow-unsigned`
только в локальной разработке.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Resultados:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (não especificado)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Reduza a carga útil e use o plano

Use `sorafs_chunker` para obter o arquivo ou arquivo desejado:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Qual é o lugar:

- `profile` / `break_mask` – fornece parâmetros `sorafs.sf1@1.0.0`.
- `chunks[]` – configuração de configuração, длины e дайджесты BLAKE3 чанков.

Para uma maior física de criptografia, você precisa registrar a base do proptest, чтобы убедиться,
что потоковый и пакетный чанкинг остаются синхронными:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Соберите и подпишите манифест

Объедините plano чанков, алиасы e подписи управления в манифест с помощью
`sorafs-manifest-stub`. Команда ниже показывает payload одного файла; передайте путь
No diretório, isso é atualizado (CLI foi criado no site da linguagem).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Verifique `/tmp/docs.report.json` em:

- `chunking.chunk_digest_sha3_256` – SHA3-дайджест смещений/длин, совпадает с фикстурами
  чанкователя.
- `manifest.manifest_blake3` – BLAKE3-дайджест, подписанный no conversor манифеста.
- `chunk_fetch_specs[]` – instruções de uso para orquestradores.

Se você deseja obter itens reais, crie argumentos `--signing-key`
e `--signer`. O comando provеряет каждую подпись Ed25519 antes de converter.

## 4. Смоделируйте получение от нескольких провайдеров

Use dev-CLI para você, seu plano de programação é o mesmo ou
não há provadores. Este é o ideal para CI de fumaça e proteção
orquestrador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Provérbios:

- `payload_digest_hex` é fornecido pelo fabricante.
- `provider_reports[]` показывает количество успехов/ошибок по каждому провайдеру.
- Ненулевой `chunk_retry_total` подсвечивает настройки contrapressão.
- Передайте `--max-peers=<n>`, чтобы ограничить число провайдеров в запуске и
  сфокусировать CI-симуляции на основных кандидатах.
- `--retry-budget=<n>` fornece suporte padrão para a chave (3), itens
  Você pode usar o registro do orquestrador através da infecção.Adicione `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>`, este é o seu caso
завершаться с ошибкой, когда восстановленный отклоняется от манифеста.

## 5. Faça uma pausa

- **Integração com atualização** – передайте дайджест манифеста и
  `manifest_signatures.json` no processo atual, este Pin Registry pode ser usado
  entrega.
- **Redefinir configuração** – ознакомьтесь com [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией novo perfil. Автоматизация должна предпочитать канонические
  хэндлы (`namespace.name@semver`) ID do número.
- **Автоматизация CI** – добавьте команды выше в release-пайплайны, чтобы документация,
  física e artefactos públicos determinam a manutenção de instalações
  метаданными.