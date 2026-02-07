---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Руководство по соответствию chunker SoraFS
sidebar_label: Bloco de suporte
description: Treinamento e fluxo de trabalho para definir o perfil do chunker SF1 em fixtures e SDKs.
---

:::nota História Canônica
:::

Este documento é uma questão de ficção, que é muito importante para a realidade, os resultados
оставаться совместимой детерминированным perfil chunker SoraFS (SF1). Sim também
documentar a regeneração do fluxo de trabalho, a política de verificação e a verificação, os resultados
Os dispositivos elétricos instalados nos SDKs são sincronizados.

## Perfil Canônico

- Semente Входной (hex): `0000000000dec0ded`
- Tamanho do tamanho: 262144 bytes (256 KiB)
- Tamanho mínimo: 65536 bytes (64 KiB)
- Tamanho máximo: 524288 bytes (512 KiB)
- Polinômio rolante: `0x3DA3358B4DC173`
- Engrenagem de sementes: `sorafs-v1-gear`
Máscara de quebra: `0x0000FFFF`

Realização do modelo: `sorafs_chunker::chunk_bytes_with_digests_profile`.
O cartão SIMD é útil para identificar granitos e resumos.

## Набор jogos

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenerativo
fixtures e выпускает следующие файлы em `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — канонические границы чанков для
  use Rust, TypeScript e Go. Каждый файл объявляет канонический alça как
  `sorafs.sf1@1.0.0`, `sorafs.sf1@1.0.0`). Порядок фиксируется
  `ensure_charter_compliance` e NÃO ДОЛЖЕН изменяться.
- `manifest_blake3.json` — BLAKE3-верифицированный manifesto, покрывающий каждый файл fixtures.
- `manifest_signatures.json` — подписи совета (Ed25519) поверх resumo манифеста.
- `sf1_profile_v1_backpressure.json` e seus corpos em `fuzz/` —
  детерминированные streaming сценарии, usado no teste chunker de contrapressão.

### Política de política

Luminárias de regência **должна** включать валидную подпись совета. Gerador
отклоняет неподписанный вывод, если явно не передан `--allow-unsigned` (предназначен
только для локальных экспериментов). Converta apenas para anexar e
дедуплицируются по подписанту.

O que você precisa saber sobre isso:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificação

Auxiliar de CI `ci/check_sorafs_fixtures.sh` gerador de falha de energia
`--locked`. Если fixtures расходятся или отсутствуют подписи, job падает. Usar
Este script é exibido em fluxos de trabalho noturnos e durante a configuração de luminárias.

Шаги ручной provérки:

1. Insira `cargo test -p sorafs_chunker`.
2. Verifique `ci/check_sorafs_fixtures.sh` localmente.
3. Verifique se o `git status -- fixtures/sorafs_chunker` está pronto.

## Плейбук обновления

Para obter um novo chunker profissional ou criar SF1:

Sim. Veja também: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
требований к метаданным, шаблонов предложений и чеклистов валидации.1. Verifique `ChunkProfileUpgradeProposalV1` (como RFC SF-1) com novos parâmetros.
2. Registre os fixtures no `export_vectors` e atualize o novo manual de resumo.
3. Verifique o manifesto do volume de negócios. O que você pode fazer agora
   instalado em `manifest_signatures.json`.
4. Abra os fixtures do SDK (Rust/Go/TS) e configure o tempo de execução.
5. Registre corpora fuzz por parâmetros de configuração.
6. Обновите это руководство новым lidar com perfil, sementes e resumo.
7. Organize a configuração com testes atualizados e roteiro-обновлениями.

Изменения, которые затрагивают границы чанков или digests без соблюдения этого процесса,
недействительны не должны быть смержены.