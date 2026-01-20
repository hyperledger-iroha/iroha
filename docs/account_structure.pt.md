---
lang: pt
direction: ltr
source: docs/account_structure.md
status: complete
translator: manual
source_hash: 7e6a1321c6f8d71ac4b576a55146767fbc488b29c7e21d82bc2e1c55db89769c
source_last_modified: "2025-11-12T00:36:40.117854+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/account_structure.md (Account Structure RFC) -->

# RFC de Estrutura de Contas

**Status:** Aceito (ADDR‑1)  
**Público:** Equipes de modelo de dados, Torii, Nexus, Wallet e Governança  
**Issues relacionadas:** A definir

## Resumo

Este documento propõe uma revisão completa do esquema de endereçamento de
contas para fornecer:

- Um endereço **Iroha Bech32m (IH‑B32)** legível para humanos e com checksum,
  que vincula um discriminante de cadeia ao signatário da conta e oferece
  formas textuais determinísticas, adequadas à interoperabilidade.
- Identificadores de domínio globalmente exclusivos, respaldados por um
  registro que possa ser consultado via Nexus para roteamento entre cadeias.
- Camadas de transicao que mantenham os aliases de roteamento
  `alias@domain` funcionando enquanto migramos wallets, APIs e contratos para
  o novo formato.

## Motivação

Hoje, wallets e ferramentas off‑chain dependem de aliases de roteamento brutos
`alias@domain`. Isso traz dois problemas principais:

1. **Sem vínculo de rede.** A string não possui checksum nem prefixo de
   cadeia, o que permite que usuários colem um endereço de outra rede sem
   feedback imediato. A transação acabará rejeitada (desajuste de
   `chain_id`) ou, pior, poderá ser aplicada em uma conta inesperada se o
   destino existir localmente.
2. **Colisão de domínios.** Domínios são apenas namespaces e podem ser
   reutilizados em cada cadeia. A federação de serviços (custódias, bridges,
   fluxos cross‑chain) torna‑se frágil porque `finance` na cadeia A não guarda
   relação com `finance` na cadeia B.

Precisamos de um formato de endereço amigável para humanos que proteja contra
erros de copiar/colar e de um mapeamento determinístico do nome de domínio para
uma cadeia autorizada.

## Objetivos

- Projetar um envelope de endereço Bech32m inspirado em IH58 para contas de
  Iroha, publicando aliases textuais canônicos CAIP‑10.
- Codificar o discriminante de cadeia configurado diretamente em cada endereço
  e definir seu processo de governança/registro.
- Descrever como introduzir um registro global de domínios sem quebrar
  instalações existentes e especificar regras de normalização/anti‑spoofing.
- Documentar expectativas operacionais, etapas de migração e questões em
  aberto.

## Não‑objetivos

- Implementar transferências de ativos cross‑chain. A camada de roteamento
  apenas retorna a cadeia de destino.
- Alterar a estrutura interna de `AccountId` (permanece
  `DomainId + PublicKey`).
- Finalizar a governança para emissão de domínios globais. Este RFC foca no
  modelo de dados e nas primitivas de transporte.

## Contexto

### Alias de roteamento atual

```text
AccountId {
    domain: DomainId,   // wrapper sobre Name (string em estilo ASCII)
    signatory: PublicKey // string multihash (por exemplo, ed0120...)
}

Display / Parse: "<signatory multihash>@<domain name>"

Esta forma textual passa a ser tratada como um **alias de conta**: uma
conveniência de roteamento que aponta para o
[`AccountAddress`](#2-canonical-address-codecs) canônico. Continua útil para
leitura humana e governança limitada ao domínio, mas deixa de ser o
identificador autoritativo da conta on‑chain.
```

`ChainId` vive fora de `AccountId`. Os nós comparam o `ChainId` de cada
transação com a configuração durante a admissão
(`AcceptTransactionFail::ChainIdMismatch`) e rejeitam transações de outras
redes, mas a string de conta em si não carrega nenhuma dica de rede.

### Identificadores de domínio

`DomainId` encapsula um `Name` (string normalizada) e é restrito à cadeia
local. Cada cadeia pode registrar `wonderland`, `finance` etc. de maneira
independente.

### Contexto Nexus

O Nexus é responsável pela coordenação entre componentes (lanes/data‑spaces).
No momento ele não possui nenhum conceito de roteamento de domínios entre
cadeias.

## Design proposto

### 1. Discriminante de cadeia determinístico

Amplia‑se `iroha_config::parameters::actual::Common` com:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // novo, coordenado globalmente
    // ... campos existentes
}
```

- **Restrições:**
  - Único por rede ativa; gerenciado por meio de um registro público assinado
    com faixas reservadas explícitas (por exemplo `0x0000–0x0FFF` para
    test/dev, `0x1000–0x7FFF` para alocações da comunidade,
    `0x8000–0xFFEF` para entradas aprovadas pela governança,
    `0xFFF0–0xFFFF` reservado).
  - Imutável para uma cadeia em execução. Alterá‑lo requer hard fork e
    atualização do registro.
- **Governança e registro:** um conjunto de governança multi‑assinatura mantém
  um registro JSON assinado (e fixado em IPFS) que mapeia discriminantes para
  aliases humanos e identificadores CAIP‑2. Clientes obtêm e armazenam em
  cache esse registro para validar e exibir metadados de cadeia.
- **Uso:** o valor é propagado pela admissão de estado, Torii, SDKs e APIs de
  wallet, de forma que cada componente possa incorporá‑lo ou validá‑lo.
  Expomos o discriminante como parte de CAIP‑2 (por exemplo `iroha:0x1234`).

<a id="2-canonical-address-codecs"></a>

### 2. Codecs de endereço canônico

O modelo de dados em Rust expõe uma única representação binária canônica
(`AccountAddress`) que pode ser emitida em vários formatos voltados ao
usuário:

- **IH58 (Iroha Base58)** – envelope Base58 que inclui o discriminante de
  cadeia. Decoders validam o prefixo antes de promover a payload à forma
  canônica.
- **Vista comprimida Sora:** alfabeto exclusivo de Sora com **105 símbolos**,
  formado ao anexar o poema イロハ em half‑width (incluindo ヰ و ヱ) ao conjunto
  IH58 de 58 caracteres. Strings começam com o sentinel `snx1`, incorporam um
  checksum derivado de Bech32m e omitem o prefixo de rede (Sora Nexus é
  inferido pelo sentinel).

  ```text
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Hex canônico:** representação `0x…` amigável para depuração da envelope
  binária canônica.

`AccountAddress::parse_any` auto‑detecta entradas comprimidas, IH58 ou hex
canônico e retorna tanto a payload decodificada quanto o
`AccountAddressFormat` detectado. Torii chama `parse_any` para endereços
suplementares ISO 20022 e armazena a forma hex canônica, garantindo que os
metadados permaneçam determinísticos independentemente da representação
original.

#### 2.1 Layout do byte de cabeçalho (ADDR‑1a)

Cada payload canônica é organizada como `header · domain selector · controller`.
O `header` é um único byte que indica quais regras de parse são aplicadas aos
bytes seguintes:

```text
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Esse primeiro byte empacota a metadata de esquema para os decoders:

| Bits | Campo         | Valores permitidos | Erro em caso de violação                                  |
|------|---------------|--------------------|------------------------------------------------------------|
| 7‑5  | `addr_version` | `0` (v1). Valores `1‑7` reservados para revisões futuras. | Valores fora de `0‑7` disparam `AccountAddressError::InvalidHeaderVersion`; implementações DEVEM tratar valores não zero como não suportados hoje. |
| 4‑3  | `addr_class`  | `0` = chave única, `1` = multisig. | Outros valores geram `AccountAddressError::UnknownAddressClass`. |
| 2‑1  | `norm_version` | `1` (Norm v1). Valores `0`, `2`, `3` reservados. | Valores fora de `0‑3` geram `AccountAddressError::InvalidNormVersion`. |
| 0    | `ext_flag`    | DEVE ser `0`.      | Bit ligado → `AccountAddressError::UnexpectedExtensionFlag`. |

O encoder em Rust escreve sempre `0x02` (versão 0, classe single‑key, versão
de normalização 1, bit de extensão limpo).

#### 2.2 Codificação do selector de domínio (ADDR‑1a)

O selector de domínio vem logo após o cabeçalho e é uma união etiquetada:

| Tag   | Significado              | Payload   | Notas |
|-------|--------------------------|-----------|-------|
| `0x00` | Domínio default implícito | nenhuma  | Corresponde ao `default_domain_name()` configurado. |
| `0x01` | Digest de domínio local | 12 bytes  | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Entrada de registro global | 4 bytes | `registry_id` em big‑endian; reservado até o registro global estar disponível. |

Rótulos de domínio são canonizados (UTS‑46 + STD3 + NFC) antes do hash. Tags
desconhecidas resultam em `AccountAddressError::UnknownDomainTag`. Ao validar
um endereço contra um domínio, selectors incompatíveis geram
`AccountAddressError::DomainMismatch`.

```text
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

O selector é adjacente ao payload do controlador, de forma que o decoder pode
percorrer o formato on‑wire em ordem: lê o byte de tag, lê a payload
específica desse tag e então avança para os bytes do controlador.

**Exemplos de selector**

- *Default implícito* (`tag = 0x00`). Sem payload. Exemplo de hex canônico
  para o domínio default usando a chave de teste determinista:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Digest local* (`tag = 0x01`). A payload é o digest de 12 bytes. Exemplo
  (`treasury`, seed `0x01`):
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *Registro global* (`tag = 0x02`). A payload é um `registry_id:u32`
  big‑endian. Os bytes subsequentes são idênticos ao caso de domínio default
  implícito; o selector apenas substitui o rótulo normalizado de domínio por
  um ponteiro de registro. Exemplo com `registry_id = 0x0000_002A` (42 em
  decimal) e o controlador determinístico default:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Codificação do payload de controlador (ADDR‑1a)

O payload do controlador é outra união etiquetada acrescentada após o selector
de domínio:

| Tag   | Controlador | Layout | Notas |
|-------|-------------|--------|-------|
| `0x00` | Chave única | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id = 0x01` mapeia para Ed25519 hoje. `key_len` é limitado a `u8`; valores maiores geram `AccountAddressError::KeyPayloadTooLong`. |
| `0x01` | Multisig    | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Suporta até 255 membros (`CONTROLLER_MULTISIG_MEMBER_MAX`). Curvas desconhecidas geram `AccountAddressError::UnknownCurve`; políticas mal formadas sobem como `AccountAddressError::InvalidMultisigPolicy`. |

Políticas multisig também expõem um mapa CBOR ao estilo CTAP2 e um digest
canônico, permitindo que hosts e SDKs validem o controlador de forma
determinística. Veja
`docs/source/references/address_norm_v1.md` (ADDR‑1c) para o esquema,
procedimento de hashing e fixtures de referência.

Todos os bytes de chave são codificados exatamente como retornados por
`PublicKey::to_bytes`; os decoders reconstroem instâncias `PublicKey` e
retornam `AccountAddressError::InvalidPublicKey` se os bytes não baterem com a
curva declarada.

> **Aplicação canônica Ed25519 (ADDR‑3a):** chaves com `curve_id = 0x01` devem
> decodificar exatamente para a sequência de bytes emitida pelo signatário e
> não podem cair no subgrupo de ordem pequena. Os nós rejeitam codificações
> não canônicas (por exemplo, valores reduzidos módulo `2^255‑19`) e pontos
> fracos como o elemento identidade; SDKs devem refletir esses erros de
> validação antes de submeter endereços.

##### 2.3.1 Registro de identificadores de curva (ADDR‑1d)

| ID (`curve_id`) | Algoritmo | Feature gate | Notas |
|-----------------|-----------|--------------|-------|
| `0x00` | Reservado | — | NÃO DEVE ser emitido; decoders retornam `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519   | Sempre ativo | Algoritmo canônico v1 (`Algorithm::Ed25519`); único id habilitado hoje em builds de produção. |
| `0x02` | ML‑DSA (preview) | `ml-dsa` | Reservado para o rollout de ADDR‑3; desativado por padrão até que o caminho de assinatura ML‑DSA esteja disponível. |
| `0x03` | Marcador GOST | `gost` | Reservado para provisionamento/negociação; payloads com esse id DEVEM ser rejeitados até que a governança aprove um perfil TC26 concreto. |
| `0x0A` | GOST R 34.10‑2012 (256, conjunto A) | `gost` | Reservado; habilitado em conjunto com o feature criptográfico `gost`. |
| `0x0B` | GOST R 34.10‑2012 (256, conjunto B) | `gost` | Reservado para aprovação futura de governança. |
| `0x0C` | GOST R 34.10‑2012 (256, conjunto C) | `gost` | Reservado para aprovação futura de governança. |
| `0x0D` | GOST R 34.10‑2012 (512, conjunto A) | `gost` | Reservado para aprovação futura de governança. |
| `0x0E` | GOST R 34.10‑2012 (512, conjunto B) | `gost` | Reservado para aprovação futura de governança. |
| `0x0F` | SM2       | `sm`   | Reservado; estará disponível quando o feature de criptografia SM for estabilizado. |

Os slots `0x04–0x09` permanecem livres para curvas adicionais; introduzir um
novo algoritmo exige atualização do roadmap e cobertura correspondente em
SDK/host. Encoders DEVEM rejeitar algoritmos não suportados com
`ERR_UNSUPPORTED_ALGORITHM`, e decoders DEVEM falhar rapidamente em ids
desconhecidos com `ERR_UNKNOWN_CURVE` para manter comportamento fail‑closed.

O registro canônico (incluindo um export JSON legível por máquinas) está em
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Ferramentas DEVEM consumi‑lo diretamente para manter identificadores de curva
consistentes entre SDKs e fluxos operacionais.

- **Gating em SDKs:** SDKs vêm por padrão apenas com Ed25519. Swift expõe
  flags de compilação (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); o SDK Java exige
  `AccountAddress.configureCurveSupport(...)`, e o SDK JavaScript usa
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`,
  obrigando integradores a fazer opt‑in explícito antes de emitirem endereços
  com curvas adicionais.
- **Gating no host:** `Register<Account>` rejeita controladores cujos
  signatários usam algoritmos ausentes da lista
  `crypto.allowed_signing` **ou** cujos `curve_id` não aparecem em
  `crypto.curves.allowed_curve_ids`; clusters precisam anunciar suporte
  (configuração + gênese) antes de aceitar controladores ML‑DSA/GOST/SM.

#### 2.4 Regras de falha (ADDR‑1a)

- Payloads menores que o cabeçalho + selector obrigatórios, ou com bytes
  remanescentes, emitem `AccountAddressError::InvalidLength` ou
  `AccountAddressError::UnexpectedTrailingBytes`.
- Headers que ligam o `ext_flag` reservado ou anunciam versões/classes
  não suportadas DEVEM ser rejeitados via `UnexpectedExtensionFlag`,
  `InvalidHeaderVersion` ou `UnknownAddressClass`.
- Tags de selector/controlador desconhecidas resultam em
  `UnknownDomainTag` ou `UnknownControllerTag`.
- Material de chave grande demais ou malformado gera
  `KeyPayloadTooLong` ou `InvalidPublicKey`.
- Controladores multisig com mais de 255 membros produzem
  `MultisigMemberOverflow`.
- Conversões IME/NFKC: kana Sora em half‑width podem ser normalizadas para
  full‑width sem quebrar o decode, mas o sentinel ASCII `snx1` e os dígitos/
  letras IH58 DEVEM permanecer ASCII. Sentinels em full‑width ou com
  case‑folding resultam em `ERR_MISSING_COMPRESSED_SENTINEL`; payloads ASCII
  em full‑width geram `ERR_INVALID_COMPRESSED_CHAR`, e mismatches de checksum
  se propagam como `ERR_CHECKSUM_MISMATCH`. Property tests em
  `crates/iroha_data_model/src/account/address.rs` cobrem esses caminhos, de
  modo que SDKs e wallets possam contar com falhas determinísticas.
- O parse de aliases `address@domain` em Torii e nos SDKs agora retorna os
  mesmos códigos `ERR_*` quando entradas IH58/comprimidas falham antes do
  fallback para o alias (por exemplo, checksum inválido ou digest de domínio
  conflitante), permitindo que os clientes repassem motivos estruturados em
  vez de deduzir mensagens textuais.

#### 2.5 Vetores binários normativos

- **Domínio default implícito (`default`, byte seed `0x00`)**  
  Hex canônico:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  Decomposição: cabeçalho `0x02`, selector `0x00` (default implícito), tag de
  controlador `0x00`, id de curva `0x01` (Ed25519), comprimento `0x20` e, em
  seguida, os 32 bytes da chave.
- **Digest de domínio local (`treasury`, byte seed `0x01`)**  
  Hex canônico:
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  Decomposição: cabeçalho `0x02`, tag de selector `0x01` mais o digest
  `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, seguido do payload de chave única
  (`0x00` como tag de controlador, `0x01` como id de curva, `0x20` como
  comprimento e os 32 bytes da chave Ed25519).

Os testes de unidade (`account::address::tests::parse_any_accepts_all_formats`)
verificam esses vetores V1 via `AccountAddress::parse_any`, garantindo que o
tooling possa confiar no mesmo payload canônico em representações hex, IH58 e
comprimidas. O conjunto estendido de fixtures pode ser regenerado com:

```text
cargo xtask address-vectors --out fixtures/account/address_vectors.json
```

### 3. Domínios globalmente únicos e normalização

Veja também
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
para a pipeline Norm v1 canônica usada em Torii, no modelo de dados e nos
SDKs.

`DomainId` passa a ser uma tupla etiquetada:

```text
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // novo enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // padrão para a cadeia local
    External { chain_discriminant: u16 },
}
```

`LocalChain` encapsula o `Name` existente para domínios gerenciados pela
cadeia atual. Quando um domínio é registrado via registro global, persistimos
o discriminante da cadeia proprietária. Exibição/parse permanecem inalterados
por enquanto, mas a estrutura expandida permite decisões de roteamento.

#### 3.1 Normalização e defesas contra spoofing

Norm v1 define a pipeline canônica que todo componente deve seguir antes de
persistir um nome de domínio ou incorporá‑lo em `AccountAddress`. O walkthrough
completo está em `docs/source/references/address_norm_v1.md`; o resumo abaixo
captura os passos que wallets, Torii, SDKs e ferramentas de governança devem
implementar.

1. **Validação de entrada.** Rejeitar strings vazias, whitespace e delimitadores
   reservados `@`, `#`, `$`. Isso corresponde às invariantes de
   `Name::validate_str`.
2. **Composição Unicode NFC.** Aplicar normalização NFC (via ICU) para colapsar
   de forma determinística sequências canonicamente equivalentes
   (por exemplo `e\u{0301}` → `é`).
3. **Normalização UTS‑46.** Rodar a saída NFC através de UTS‑46 com
   `use_std3_ascii_rules = true`, `transitional_processing = false` e
   verificação de limites DNS ativada. O resultado é uma sequência de
   A‑labels em minúsculas; entradas que violem STD3 falham aqui.
4. **Limites de comprimento.** Aplicar os limites estilo DNS: cada label DEVE
   ter 1–63 bytes e o domínio completo NÃO deve exceder 255 bytes após o
   passo 3.
5. **Política opcional de confundíveis.** Checks de script UTS‑39 são
   acompanhados para Norm v2; operadores podem habilitá‑los antecipadamente,
   mas uma falha nesse check deve abortar o processamento.

Se todos os passos forem bem‑sucedidos, a string de A‑labels em minúsculas é
cacheada e usada para encoding de endereços, configuração, manifests e
consultas ao registro. Selectors de digest local derivam seu valor de
12 bytes como
`blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]` usando a
saída do passo 3. Outras formas de entrada (case misto, Unicode cru, etc.)
são rejeitadas com `ParseError`s estruturados na fronteira em que o nome foi
fornecido.

Fixtures canônicos que demonstram essas regras — incluindo round‑trips de
punycode e sequências inválidas sob STD3 — estão em
`docs/source/references/address_norm_v1.md` e são espelhados nos vetores de
testes de CI dos SDKs acompanhados sob ADDR‑2.

### 4. Registro de domínios no Nexus e roteamento

- **Esquema de registro:** o Nexus mantém um mapa assinado
  `DomainName -> ChainRecord`, em que `ChainRecord` inclui o discriminante de
  cadeia, metadados opcionais (endpoints RPC) e uma prova de autoridade
  (por exemplo, multi‑assinatura de governança).
- **Mecanismo de sincronização:**
  - Cadeias submetem ao Nexus reivindicações de domínio assinadas (no gênese
    ou via instruções de governança).
  - O Nexus publica manifests periódicos (JSON assinado + raiz Merkle
    opcional) sobre HTTPS e armazenamento endereçado por conteúdo
    (por exemplo, IPFS). Clientes fazem pin do manifest mais recente e
    verificam as assinaturas.
- **Fluxo de consulta:**
  - Torii recebe uma transação que referencia um `DomainId`.
  - Se o domínio for desconhecido localmente, Torii consulta o manifest
    cacheado do Nexus.
  - Se o manifest indicar que o domínio pertence a outra cadeia, a transação é
    rejeitada com erro determinístico `ForeignDomain` e informações sobre a
    cadeia remota.
  - Se o domínio não aparecer no Nexus, Torii retorna `UnknownDomain`.
- **Âncoras de confiança e rotação:** chaves de governança assinam os
  manifests; rotações ou revogações são publicadas como um novo manifest. Os
  clientes aplicam TTL (por exemplo, 24 h) e recusam dados expirados além
  dessa janela.
- **Modos de falha:** se a obtenção do manifest falhar, Torii recorre ao
  manifest em cache enquanto o TTL estiver válido; passado o TTL ele emite
  `RegistryUnavailable` e recusa roteamento entre domínios para evitar
  estados inconsistentes.

#### 4.1 Imutabilidade do registro, aliases e tombstones (ADDR‑7c)

O Nexus publica um **manifest append‑only**, de modo que cada atribuição de
domínio/alias possa ser auditada e reproduzida. Operadores devem tratar o
bundle descrito em
`docs/source/runbooks/address_manifest_ops.md` como fonte única da verdade:
se um manifest estiver ausente ou falhar validação, o Torii deve recusar‑se a
resolver o domínio afetado.

Suporte de automação:
`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
reexecuta as verificações de checksum, schema e `previous_digest` descritas
no runbook. Inclua a saída do comando nos tickets de mudança para mostrar que
o vínculo `sequence` + `previous_digest` foi validado antes de publicar o
bundle.

##### Cabeçalho de manifest e contrato de assinatura

| Campo               | Requisito |
|---------------------|-----------|
| `version`           | Atualmente `1`. Só deve ser incrementado junto a uma atualização do spec. |
| `sequence`          | Deve ser incrementado em **exatamente** 1 a cada publicação. Torii rejeita revisões com saltos ou regressões. |
| `generated_ms` + `ttl_hours` | Definem a frescura de cache (padrão 24 h). Se o TTL expira antes do próximo manifest, Torii passa a `RegistryUnavailable`. |
| `previous_digest`   | Digest BLAKE3 (hex) do corpo do manifest anterior. Verificadores o recalculam com `b3sum` para provar imutabilidade. |
| `signatures`        | Manifests são assinados via Sigstore (`cosign sign-blob`). Operações deve executar `cosign verify-blob --bundle manifest.sigstore manifest.json` e aplicar as restrições de identidade/emissor de governança antes do rollout. |

A automação de release gera `manifest.sigstore` e `checksums.sha256` ao lado
do corpo JSON. Esses arquivos devem ser mantidos juntos ao espelhar para
SoraFS ou endpoints HTTP, para que auditores possam reproduzir os passos de
verificação literalmente.

##### Tipos de entrada

| Tipo           | Propósito | Campos obrigatórios |
|----------------|-----------|---------------------|
| `global_domain` | Declara que um domínio está registrado globalmente e deve ser mapeado para um discriminante de cadeia e prefixo IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `local_alias`   | Acompanha selectors alternativos (`Local-12`) que ainda roteiam localmente. Adiciona o digest de 12 bytes e um `alias_label` opcional. | `{ "domain": "<label>", "selector": { "kind": "local", "digest_hex": "<12-byte-hex>" }, "alias_label": "<optional>" }` |
| `tombstone`     | Retira permanentemente um alias/selector. Obrigatório ao remover digests Local‑8 ou eliminar um domínio. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

Entradas `global_domain` podem opcionalmente incluir `manifest_url` ou
`sorafs_cid` para apontar para metadados de cadeia assinados, mas a tupla
canônica permanece `{domain, chain, discriminant/ih58_prefix}`. Registros
`tombstone` **devem** citar o selector que está sendo retirado e o
ticket/artefato de governança que autorizou a mudança, de modo que o trilho de
auditoria possa ser reconstruído offline.

##### Fluxo de alias/tombstone e telemetria

1. **Detectar drift.** Use
   `torii_address_local8_total{endpoint}` e
   `torii_address_invalid_total{endpoint,reason}`
   (renderizados em `dashboards/grafana/address_ingest.json`) para confirmar
   que strings Local‑8 já não são aceitas em produção antes de propor um
   tombstone.
2. **Derivar digests canônicos.** Rode
   `iroha address convert <address-or-account_id> --format json --expect-prefix 753`
   (ou consuma `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) para capturar exatamente o campo
   `digest_hex`. O CLI aceita entradas como `snx1...@wonderland`; o resumo
   JSON expõe o domínio via `input_domain` e `--append-domain` reproduz a
   codificação convertida como `<ih58>@wonderland` para atualizações de
   manifest. Para exportações orientadas a linha, use
   para converter em massa selectors Local em formas IH58 canônicas (ou
   comprimidas/hex/JSON), ignorando linhas não locais. Quando auditores
   precisarem de evidência amigável a planilhas, rode
   para emitir um CSV (`input,status,format,domain_kind,…`) que destaque
   selectors Local, codificações canônicas e falhas de parse em um único
   arquivo.
3. **Anexar entradas ao manifest.** Redija o registro `tombstone` (e o
   `global_domain` subsequente ao migrar para o registro global) e valide o
   manifest com `cargo xtask address-manifest verify` antes de solicitar
   assinaturas.
4. **Verificar e publicar.** Siga o checklist do runbook (hashes, Sigstore,
   monotonicidade de `sequence`) antes de espelhar o bundle no SoraFS. Torii
   de produção passem a exigir literais IH58/comprimidos canônicos assim que o
   bundle estiver disponível.
5. **Monitorar e, se necessário, reverter.** Mantenha os painéis Local‑8 em
   zero por 30 dias; se surgirem regressões, republicar o bundle anterior de
   manifests e, apenas em ambientes não‑produtivos, definir temporariamente
