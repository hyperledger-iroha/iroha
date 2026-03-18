---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Руководство по авторингу perfil chunker SoraFS
sidebar_label: Auto chunker
descrição: Verifique para o novo perfil chunker SoraFS e luminárias.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/chunker_profile_authoring.md`. Se você tiver uma cópia sincronizada, a estrela do Sphinx não estará disponível na configuração.
:::

# Руководство по авторингу perfil chunker SoraFS

Este é um novo produto que está sendo desenvolvido e publicado um novo chunker de perfil para SoraFS.
Este é um arquiteto RFC (SF-1) e uma restauração atualizada (SF-2a)
конкретными требованиями к авторингу, шагами валидации и шаблонами предложений.
No primeiro exemplo canônico.
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e соответствующий registro de simulação em
`docs/source/sorafs/reports/sf1_determinism.md`.

##Obzor

Qual perfil, publicado na rede, segue:

- объявлять детерминированные parâmetros CDC e настройки multihash, одинаковые на всех
  arquiteto;
- поставлять воспроизводимые fixtures (JSON Rust/Go/TS + fuzz corpora + PoR testemunha), которые
  SDKs downstream podem fornecer ferramentas especializadas;
- включать метаданные, готовые для governança (namespace, nome, semver), e também recomendações
  por migrações e octa совместимости; e
- проходить детерминированную diff-suite para restaurar o estado.

Следуйте чеклисту ниже, чтобы подготовить предложение, удовлетворяющее этим правилам.

## Снимок чартеров реестра

Antes de começar a usar o site, isso é possível,
código de barras `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Perfil de ID — use o perfil do usuário, que é monotonado sem qualquer proposta.
- Identificador canônico (`namespace.name@semver`) должен присутствовать в списке alias и
- Ни один alias не должен конфликтовать с другим каноническим handle и повторяться.
- Alias ​​должны быть непустыми и без пробелов по краям.

Use as configurações CLI:

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos foram implementados com sucesso com a restauração e a canonização
метаданные для обсуждений governança.

## Требуемые метаданные| Pólo | Descrição | Exemplo (`sorafs.sf1@1.0.0`) |
|------|----------|----------------------------|
| `namespace` | Логическая группировка связанных perfil. | `sorafs` |
| `name` | Читаемая человеком метка. | `sf1` |
| `semver` | Строка семантической версии для набора параметров. | `1.0.0` |
| `profile_id` | Um identificador de identidade monotônico que define o perfil de perfil desejado. Registre o ID do ID, mas não verifique o número atual. | `1` |
| `profile.min_size` | Quantidade mínima de bytes. | `65536` |
| `profile.target_size` | A quantidade de bytes é maior. | `262144` |
| `profile.max_size` | Quantidade máxima de bytes. | `524288` |
| `profile.break_mask` | Máscara adaptável para hash rolante (hex). | `0x0000ffff` |
| `profile.polynomial` | Константа gear полинома (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semente para вычисления 64 KiB gear таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para o resumo da pesquisa. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digerir jogos de pacotes канонического. | `13fa...c482` |
| `fixtures_root` | Относительный каталог с регенерированными luminárias. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semente para determinar PoR выборки (`splitmix64`). | `0xfeedbeefcafebabe` (exemplo) |

Метаданные должны присутствовать как в документе предложения, так и внутри сгенерированных
luminárias, configuração de padrões, ferramentas CLI e governança automatizada podem ser úteis para você
ручных сверок. Se esta for uma comunidade, instale chunk-store e CLIs de manifesto com `--json-out=-`,
чтобы стримить вычисленные метаданные в заметки ревью.

### Точки взаимодействия CLI e restauração

- `sorafs_manifest_chunk_store --profile=<handle>` — повторно запускает метаданные чанка,
  manifest digest e PoR fornecem parâmetros pré-definidos.
- `sorafs_manifest_chunk_store --json-out=-` — definindo chunk-store em stdout para
  operação automática.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждает, что manifestos и CAR
  планы встраивают канонический identificador e aliases.
- `sorafs_manifest_stub --plan=-` — você pode usar `chunk_fetch_specs` para provar
  compensações/resumos são possíveis.

Запишите вывод команд (resumos, raízes PoR, hashes de manifesto) em предложении, чтобы ревьюеры могли
воспроизвести их буквально.

## Verifique a determinação e validação1. **Equipamentos de redefinição**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Comprar conjunto de pacotes** — `cargo test -p sorafs_chunker` e chicote de diferenças entre idiomas
   (`crates/sorafs_chunker/tests/vectors.rs`) должны быть зелеными с новыми fixtures.
3. **Corpora fuzz/contrapressão** — use `cargo fuzz list` e chicote de streaming
   (`fuzz/sorafs_chunker`) para regenerar ativos.
4. **Verifique testemunhas de prova de recuperação** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` com perfil de perfil e capacidade,
   что root совпадают с manifesto de fixação.
5. **CI dry run** — выполните `ci/check_sorafs_fixtures.sh` local; roteiro feito
   instale novos acessórios e instale `manifest_signatures.json`.
6. **Possibilidade de tempo de execução cruzado** — убедитесь, что Go/TS bindings потребляют регенерированный
   JSON é usado para identificar arquivos de texto e resumos.

Документируйте команды e полученные digests в предложении, чтобы Tooling WG pode повторить их без догадок.

### Manifesto / PoR

Após a regeneração de fixtures, você precisa de um pipeline de manifesto, que é usado, isso
CAR метаданные e provas PoR остаются согласованными:

```bash
# Проверить метаданные чанка + PoR с новым профилем
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Сгенерировать manifest + CAR и сохранить chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Повторно запустить с сохраненным планом fetch (защищает от устаревших offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Замените входной файл любым представительным корпусом, используемым в ваших fixtures
(por exemplo, детерминированным потоком 1 GiB), e приложите полученные digests к предложению.

## Shablon предложения

Você pode usar Norito para identificar `ChunkerProfileProposalV1` e фиксируются em
`docs/source/sorafs/proposals/`. JSON é um formato que não possui formato definido
(подставьте свои значения по мере необходимости):


Baixe o Markdown do Markdown (`determinism_report`), arquivo de imagem
comando, digere чанков e любые отклонения, обнаруженные при валидации.

## Fluxo de trabalho de governança

1. **Ativar PR com pré-venda + luminárias.** Ativar ativos de geração, Norito
   atualização e atualização `chunker_registry_data.rs`.
2. **WG de Ferramentas.** Ревьюеры повторно запускают чеклист валидации и подтверждают,
   este é o programa de teste de segurança (sem ID de uso público,
   детерминизм достигнут).
3. **Конверт совета.** После одобрения члены совета подписывают digest предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e conjunto de dados
   no perfil de conversão, хранящийся вместе с luminárias.
4. **Restaurar arquivos.** Mesclar arquivos, documentos e acessórios atualizados. Para usar CLI
   остается на предыдущем perfil, пока governança não объявит миграцию готовой.
5. **Descarte a operação.** Após a migração, você pode restaurar a propriedade e removê-la.

## Советы по авторингу

- Предпочитайте четные границы степеней двойки, чтобы минимизировать крайние случаи chunking.
- Não mencione código multihash sem codificação com manifesto e gateway; fazer
  заметку о совместимости.
- Держите sementes gear таблицы читаемыми, но глобально уникальными для упрощения аудита.
- Сохраняйте артефакты бенчмаркинга (por exemplo, taxa de transferência de segurança) em
  `docs/source/sorafs/reports/` para uma caixa de som.

A operação será executada no início do lançamento. в livro de migração
(`docs/source/sorafs/migration_ledger.md`). Правила runtime соответствия см. em
`docs/source/sorafs/chunker_conformance.md`.