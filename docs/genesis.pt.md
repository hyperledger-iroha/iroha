---
lang: pt
direction: ltr
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/genesis.md (Genesis configuration) -->

# Configuração de Gênesis

Um arquivo `genesis.json` define as primeiras transações que são executadas
quando uma rede Iroha é iniciada. O arquivo é um objeto JSON com estes campos:

- `chain`: identificador único da chain.
- `executor` (opcional): caminho para o bytecode do executor (`.to`). Se
  presente, o gênesis inclui uma instrução `Upgrade` como primeira transação.
  Se omitido, nenhuma atualização é feita e o executor embutido é utilizado.
- `ivm_dir`: diretório contendo as bibliotecas de bytecode IVM. Se omitido,
  assume o valor padrão `"."`.
- `transactions`: lista de transações de gênesis executadas
  sequencialmente. Cada entrada pode conter:
  - `parameters`: parâmetros iniciais da rede.
  - `instructions`: instruções codificadas com Norito.
  - `ivm_triggers`: triggers com executáveis de bytecode IVM.
  - `topology`: topologia inicial de peers. Cada entrada pode incluir
    `pop_hex` (opcional) para o PoP, mas ele deve estar presente antes da assinatura.
- `crypto`: snapshot de criptografia espelhado de `iroha_config.crypto`
  (`default_hash`, `allowed_signing`, `allowed_curve_ids`,
  `sm2_distid_default`, `sm_openssl_preview`). `allowed_curve_ids` espelha
  `crypto.curves.allowed_curve_ids` para que manifests possam anunciar quais
  curvas de controlador o cluster aceita. As ferramentas impõem combinações
  SM válidas: manifests que listam `sm2` também precisam alternar o hash para
  `sm3-256`, enquanto builds compiladas sem o feature `sm` rejeitam `sm2`
  integralmente.

Exemplo (saída de `kagami genesis generate default`, com instruções
encurtadas):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### Inicializar o bloco `crypto` para SM2/SM3

Use o helper `xtask` para produzir, em um único passo, o inventário de chaves
e o snippet de configuração pronto para colar:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

O arquivo `client-sm2.toml` passa a conter:

```toml
# Account key material
public_key = "sm2:86264104..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # remova "sm2" para permanecer em modo somente verificação
allowed_curve_ids = [1]               # adicione novos curve ids (ex.: 15 para SM2) quando controladores forem permitidos
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # opcional: apenas ao implantar o caminho OpenSSL/Tongsuo
```

Copie os valores de `public_key`/`private_key` para a configuração da
conta/cliente e atualize o bloco `crypto` de `genesis.json` para que ele
corresponda ao snippet (por exemplo, ajustando `default_hash` para `sm3-256`,
adicionando `"sm2"` a `allowed_signing` e incluindo os `allowed_curve_ids`
corretos). Kagami irá recusar manifests em que as configurações de
hash/curvas e a lista de algoritmos permitidos sejam inconsistentes.

> **Dica:** envie o snippet para stdout com `--snippet-out -` quando quiser
> apenas inspecionar a saída. Use `--json-out -` para emitir também o
> inventário de chaves no stdout.

Se preferir dirigir manualmente os comandos de baixo nível do CLI, o fluxo
equivalente é:

```bash
# 1. Produzir material de chave determinístico (grava JSON em disco)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hidratar o snippet que pode ser colado em arquivos de cliente/config
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Dica:** `jq` é usado acima para evitar um passo manual de copiar/colar.
> Se não estiver disponível, abra `sm2-key.json`, copie o campo
> `private_key_hex` e passe o valor diretamente para `crypto sm2 export`.

> **Guia de migração:** ao converter uma rede existente para SM2/SM3/SM4,
> siga [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> para aplicar overrides em `iroha_config`, regenerar manifests e planejar
> rollback.

## Gerar e validar

1. Gerar um template:

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - `--executor` (opcional) aponta para um arquivo `.to` de executor IVM;
     quando presente, Kagami emite uma instrução `Upgrade` como primeira
     transação.
   - `--genesis-public-key` define a chave pública que será usada para assinar
     o bloco de gênesis; deve ser um multihash reconhecido por
     `iroha_crypto::Algorithm` (incluindo variantes GOST TC26 quando
     compilado com o feature correspondente).

2. Validar enquanto edita:

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   Isto garante que `genesis.json` siga o schema, que os parâmetros sejam
   válidos, que os `Name` sejam corretos e que as instruções Norito possam ser
   decodificadas.

3. Assinar para deployment:

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   Quando o `irohad` é iniciado apenas com `--genesis-manifest-json` (sem um
   bloco de gênesis assinado), o nó passa a derivar automaticamente sua
   configuração criptográfica de runtime a partir do manifest; se um bloco de
   gênesis também for fornecido, manifest e config ainda precisam coincidir
   exatamente.

`kagami genesis sign` verifica que o JSON seja válido e produz um bloco
codificado com Norito pronto para ser usado via `genesis.file` na
configuração do nó. O arquivo `genesis.signed.nrt` resultante já está em
forma canônica “on wire”: um byte de versão seguido de um cabeçalho Norito
que descreve o layout da payload. Sempre distribua esta saída enquadrada.
Prefira o sufixo `.nrt` para payloads assinadas; se você não precisar
atualizar o executor no gênesis, pode omitir o campo `executor` e deixar de
fornecer o arquivo `.to`.

## O que o Gênesis pode fazer

O gênesis suporta as seguintes operações. Kagami as monta em transações em uma
ordem bem definida para que os peers executem, de forma determinista, a mesma
sequência:

- **Parâmetros**: definição de valores iniciais para Sumeragi (tempos de
  bloco/commit, drift), Block (máximo de transações), Transaction (máximo de
  instruções, tamanho de bytecode), limites do executor e de contratos
  inteligentes (combustível, memória, profundidade) e parâmetros
  personalizados. Kagami semeia `Sumeragi::NextMode` e a payload
  `sumeragi_npos_parameters` no bloco `parameters`, e o bloco assinado inclui
  as instruções `SetParameter` geradas para que o startup possa aplicar knobs
  de consenso a partir do estado on‑chain.
- **Instruções nativas**: registrar/cancelar registro de Domain, Account e
  Asset Definition; cunhar/queimar/transferir assets; transferir propriedade
  de domínios e definicões de ativos; modificar metadata; conceder permissões
  e roles.
- **Triggers IVM**: registrar triggers que executam bytecode IVM (ver
  `ivm_triggers`). Os executáveis dos triggers são resolvidos relativamente a
  `ivm_dir`.
- **Topologia**: fornecer o conjunto inicial de peers via array `topology`
  (lista de `PeerId`) dentro de qualquer transação (comumente a primeira ou
  a última).
- **Upgrade do executor (opcional)**: se `executor` estiver presente, o
  gênesis insere uma instrução `Upgrade` única como primeira transação; caso
  contrário, o gênesis começa diretamente com parâmetros/instruções.

### Ordenação das transações

Conceitualmente, as transações de gênesis são processadas nesta ordem:

1. (Opcional) Upgrade do executor  
2. Para cada transação em `transactions`:
   - Atualizações de parâmetros
   - Instruções nativas
   - Registro de triggers IVM
   - Entradas de topologia

Kagami e o código do nó garantem essa ordenação para que, por exemplo, os
parâmetros sejam aplicados antes das instruções subsequentes na mesma
transação.

## Fluxo de trabalho recomendado

- Partir de um template gerado com Kagami:
  - Apenas ISI embutidas:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> [--consensus-mode npos] > genesis.json`
  - Com upgrade opcional de executor: adicionar `--executor <path/to/executor.to>`
- `<PK>` é qualquer multihash reconhecido por `iroha_crypto::Algorithm`,
  incluindo variantes GOST TC26 quando o Kagami é compilado com
  `--features gost` (por exemplo `gost3410-2012-256-paramset-a:...`).
- Validar enquanto edita: `kagami genesis validate genesis.json`
- Assinar para deployment:
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Configurar os peers: definir `genesis.file` apontando para o arquivo
  Norito assinado (por exemplo `genesis.signed.nrt`) e `genesis.public_key`
  com o mesmo `<PK>` usado na assinatura.

Notas:
- O template “default” do Kagami registra um domínio e contas de exemplo,
  cunha alguns assets e concede permissões mínimas usando apenas ISIs
  embutidas – sem `.to`.
- Se você incluir um upgrade de executor, ele deve ser a primeira transação.
  Kagami faz cumprir essa regra ao gerar/assinar.
- Use `kagami genesis validate` para detectar valores `Name` inválidos
  (por exemplo, whitespace) e instruções malformadas antes da assinatura.

## Execução com Docker/Swarm

Os artefatos de Docker Compose e Swarm fornecidos lidam com ambos os casos:

- Sem executor: o comando compose remove um campo `executor` ausente/vazio e
  assina o arquivo.
- Com executor: resolve o caminho relativo do executor para um caminho
  absoluto dentro do container e assina o arquivo.

Isso mantém o desenvolvimento simples em máquinas sem amostras IVM
precompiladas, mas ainda permite upgrades de executor quando necessário.
