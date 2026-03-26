# RFC da estrutura da conta

**Status:** Aceito (ADDR-1)  
**Público:** Modelo de dados, Torii, Nexus, Wallet, equipes de governança  
**Problemas relacionados:** A definir

## Resumo

Este documento descreve a pilha de endereçamento de contas de remessa implementada em
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) e o
ferramentas complementares. Ele fornece:

- Um endereço **I105** com soma de verificação e voltado para humanos, produzido por
  `AccountAddress::to_i105` que vincula um discriminante de cadeia à conta
  controlador e oferece formas textuais determinísticas e compatíveis com interoperabilidade.
- Seletores de domínio para domínios padrão implícitos e resumos locais, com um
  tag seletora de registro global reservada para futuro roteamento apoiado por Nexus (o
  a pesquisa de registro **ainda não foi enviada**).

## Motivação

Atualmente, carteiras e ferramentas fora da cadeia dependem de aliases de roteamento `alias@domain` (rejected legacy form) brutos. Isto
tem duas desvantagens principais:

1. **Sem ligação de rede.** A string não tem soma de verificação ou prefixo de cadeia, então os usuários
   pode colar um endereço da rede errada sem feedback imediato. O
   a transação acabará sendo rejeitada (incompatibilidade de cadeia) ou, pior, bem-sucedida
   contra uma conta não intencional se o destino existir localmente.
2. **Colisão de domínios.** Os domínios são apenas de namespace e podem ser reutilizados em cada
   cadeia. Federação de serviços (custodiantes, pontes, fluxos de trabalho entre cadeias)
   torna-se frágil porque `finance` na cadeia A não está relacionado a `finance` na
   cadeia B.

Precisamos de um formato de endereço amigável que proteja contra erros de copiar/colar
e um mapeamento determinístico do nome de domínio para a cadeia autoritativa.

## Metas

- Descrever o envelope I105 implementado no modelo de dados e o
  regras canônicas de análise/alias que `AccountId` e `AccountAddress` seguem.
- Codifique o discriminante de cadeia configurado diretamente em cada endereço e
  definir seu processo de governança/registro.
- Descrever como introduzir um registro de domínio global sem quebrar a corrente
  implantações e especificar regras de normalização/anti-spoofing.

## Não-metas

- Implementação de transferências de ativos entre cadeias. A camada de roteamento retorna apenas o
  cadeia alvo.
- Finalizar a governação para a emissão de domínios globais. Esta RFC se concentra nos dados
  modelo e primitivas de transporte.

## Plano de fundo

### Alias de roteamento atual

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` mora fora de `AccountId`. Os nós verificam o `ChainId` da transação
contra configuração durante a admissão (`AcceptTransactionFail::ChainIdMismatch`)
e rejeitar transações estrangeiras, mas a sequência da conta em si não carrega
dica de rede.

### Identificadores de domínio

`DomainId` envolve um `Name` (string normalizada) e tem como escopo a cadeia local.
Cada rede pode registrar `wonderland`, `finance`, etc.

### Contexto do Nexus

Nexus é responsável pela coordenação entre componentes (pistas/espaços de dados). Isso
atualmente não tem conceito de roteamento de domínio entre cadeias.

## Projeto proposto

### 1. Discriminante de cadeia determinística

`iroha_config::parameters::actual::Common` agora expõe:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Restrições:**
  - Único por rede ativa; gerenciado através de um registro público assinado com
    intervalos reservados explícitos (por exemplo, `0x0000–0x0FFF` test/dev, `0x1000–0x7FFF`
    alocações comunitárias, `0x8000–0xFFEF` aprovadas pela governança, `0xFFF0–0xFFFF`
    reservado).
  - Imutável para uma corrente em execução. Mudá-lo requer um hard fork e um
    atualização de registro.
- **Governança e registro (planejado):** Um conjunto de governança com múltiplas assinaturas
  manter um registro JSON assinado mapeando discriminantes para aliases humanos e
  Identificadores CAIP-2. Este registro ainda não faz parte do tempo de execução enviado.
- **Uso:** Encadeado por meio de admissão estadual, Torii, SDKs e APIs de carteira para que
  cada componente pode incorporá-lo ou validá-lo. A exposição ao CAIP-2 continua a ser um futuro
  tarefa de interoperabilidade.

### 2. Codecs de endereço canônicos

O modelo de dados Rust expõe uma única representação canônica de carga útil
(`AccountAddress`) que pode ser emitido em vários formatos voltados para humanos. I105 é
o formato de conta preferido para compartilhamento e produção canônica; o comprimido
O formulário `sora` é a segunda melhor opção, somente Sora, para UX, onde o alfabeto kana
agrega valor. O hexadecimal canônico continua sendo um auxílio à depuração.

- **I105** – um envelope I105 que incorpora a corrente
  discriminante. Os decodificadores validam o prefixo antes de promover a carga útil para
  a forma canônica.
- **Visualização compactada Sora** – um alfabeto somente Sora de **105 símbolos** construído por
  anexando o poema イロハ de meia largura (incluindo ヰ e ヱ) aos 58 caracteres
  Conjunto I105. Strings começam com o sentinela `sora`, incorporam um derivado de Bech32m
  soma de verificação e omitir o prefixo da rede (Sora Nexus está implícito no sentinela).

```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Hexagonal canônico** – uma codificação `0x…` amigável para depuração do byte canônico
  envelope.

`AccountAddress::parse_encoded` detecta automaticamente I105 (preferencial), compactado (`sora`, segundo melhor) ou hexadecimal canônico
(`0x...` somente; hexadecimal simples é rejeitado) insere e retorna a carga útil decodificada e a carga detectada
`AccountAddress`. Torii agora chama `parse_encoded` para ISO 20022 suplementar
endereça e armazena a forma hexadecimal canônica para que os metadados permaneçam determinísticos
independentemente da representação original.

#### 2.1 Layout de bytes de cabeçalho (ADDR-1a)

Cada carga útil canônica é apresentada como `header · controller`. O
`header` é um único byte que comunica quais regras do analisador se aplicam aos bytes que
siga:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

O primeiro byte, portanto, empacota os metadados do esquema para decodificadores downstream:

| Pedaços | Campo | Valores permitidos | Erro por violação |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). Os valores `1-7` estão reservados para futuras revisões. | Valores fora de `0-7` trigger `AccountAddressError::InvalidHeaderVersion`; as implementações DEVEM tratar versões diferentes de zero como sem suporte hoje. |
| 4-3 | `addr_class` | `0` = chave única, `1` = multisig. | Outros valores aumentam `AccountAddressError::UnknownAddressClass`. |
| 2-1 | `norm_version` | `1` (Norma v1). Os valores `0`, `2`, `3` são reservados. | Valores fora de `0-3` aumentam `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | DEVE ser `0`. | Definir aumentos de bits `AccountAddressError::UnexpectedExtensionFlag`. |

O codificador Rust escreve `0x02` para controladores de chave única (versão 0, classe 0,
norma v1, sinalizador de extensão desmarcado) e `0x0A` para controladores multisig (versão 0,
classe 1, norma v1, sinalizador de extensão desmarcado).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Codificações de carga útil do controlador (ADDR-1a)

A carga útil do controlador é outra união marcada anexada após o seletor de domínio:

| Etiqueta | Controlador | Disposição | Notas |
|-----|-----------|--------|-------|
| `0x00` | Chave única | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` mapeia para Ed25519 hoje. `key_len` é limitado a `u8`; valores maiores geram `AccountAddressError::KeyPayloadTooLong` (portanto, chaves públicas ML‑DSA de chave única, que têm >255 bytes, não podem ser codificadas e devem usar multisig). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Suporta até 255 membros (`CONTROLLER_MULTISIG_MEMBER_MAX`). Curvas desconhecidas aumentam `AccountAddressError::UnknownCurve`; políticas malformadas aparecem como `AccountAddressError::InvalidMultisigPolicy`. |

As políticas Multisig também expõem um mapa CBOR estilo CTAP2 e um resumo canônico para
hosts e SDKs podem verificar o controlador de forma determinística. Veja
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) para o esquema,
regras de validação, procedimento de hash e acessórios de ouro.

Todos os bytes de chave são codificados exatamente como retornados por `PublicKey::to_bytes`; decodificadores reconstroem instâncias `PublicKey` e aumentam `AccountAddressError::InvalidPublicKey` se os bytes não corresponderem à curva declarada.

> **Aplicação canônica Ed25519 (ADDR-3a):** chaves de curva `0x01` devem ser decodificadas para a sequência de bytes exata emitida pelo signatário e não devem estar no subgrupo de ordem pequena. Os nós agora rejeitam codificações não canônicas (por exemplo, valores reduzidos módulo `2^255-19`) e pontos fracos, como o elemento de identidade, portanto, os SDKs devem revelar erros de validação de correspondência antes de enviar endereços.

##### 2.3.1 Registro de identificador de curva (ADDR-1d)

| ID (`curve_id`) | Algoritmo | Portão de recurso | Notas |
|-----------------|-----------|-------------|-------|
| `0x00` | Reservado | — | NÃO DEVE ser emitido; superfície dos decodificadores `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Algoritmo canônico v1 (`Algorithm::Ed25519`); habilitado na configuração padrão. |
| `0x02` | ML-DSA (Dilithium3) | — | Usa os bytes da chave pública Dilithium3 (1952 bytes). Endereços de chave única não podem codificar ML‑DSA porque `key_len` é `u8`; multisig usa comprimentos `u16`. |
| `0x03` | BLS12‑381 (normal) | `bls` | Chaves públicas em G1 (48 bytes), assinaturas em G2 (96 bytes). |
| `0x04` | secp256k1 | — | ECDSA determinístico sobre SHA‑256; as chaves públicas usam o formato compactado SEC1 de 33 bytes e as assinaturas usam o layout canônico `r∥s` de 64 bytes. |
| `0x05` | BLS12‑381 (pequeno) | `bls` | Chaves públicas em G2 (96 bytes), assinaturas em G1 (48 bytes). |
| `0x0A` | GOST R 34.10‑2012 (256, conjunto A) | `gost` | Disponível apenas quando o recurso `gost` está ativado. |
| `0x0B` | GOST R 34.10‑2012 (256, conjunto B) | `gost` | Disponível somente quando o recurso `gost` está ativado. |
| `0x0C` | GOST R 34.10‑2012 (256, conjunto C) | `gost` | Disponível somente quando o recurso `gost` está ativado. |
| `0x0D` | GOST R 34.10‑2012 (512, conjunto A) | `gost` | Disponível somente quando o recurso `gost` está habilitado. |
| `0x0E` | GOST R 34.10‑2012 (512, conjunto B) | `gost` | Disponível somente quando o recurso `gost` está ativado. |
| `0x0F` | SM2 | `sm` | Comprimento DistID (u16 BE) + bytes DistID + chave SM2 descompactada SEC1 de 65 bytes; disponível apenas quando `sm` está habilitado. |

Os slots `0x06–0x09` permanecem não atribuídos para curvas adicionais; introduzindo um novo
O algoritmo requer uma atualização do roteiro e cobertura correspondente do SDK/host. Codificadores
DEVE rejeitar qualquer algoritmo não suportado com `ERR_UNSUPPORTED_ALGORITHM`, e
os decodificadores DEVEM falhar rapidamente em ids desconhecidos com `ERR_UNKNOWN_CURVE` para preservar
comportamento de falha fechada.

O registro canônico (incluindo uma exportação JSON legível por máquina) reside em
[`docs/source/references/address_curve_registry.md`](fonte/referências/address_curve_registry.md).
As ferramentas DEVEM consumir esse conjunto de dados diretamente para que os identificadores de curva permaneçam
consistente em SDKs e fluxos de trabalho do operador.

- **Gating do SDK:** os SDKs são padronizados para validação/codificação somente Ed25519. Swift expõe
  sinalizadores de tempo de compilação (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); o SDK Java/Android requer
  `AccountAddress.configureCurveSupport(...)`; o SDK JavaScript usa
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  O suporte secp256k1 está disponível, mas não habilitado por padrão no JS/Android
  SDKs; os chamadores devem aceitar explicitamente ao emitir controladores não Ed25519.
- **Host gate:** `Register<Account>` rejeita controladores cujos signatários usam algoritmos
  ausentes na lista `crypto.allowed_signing` do nó **ou** identificadores de curva ausentes
  `crypto.curves.allowed_curve_ids`, portanto os clusters devem anunciar suporte (configuração +
  genesis) antes que os controladores ML-DSA/GOST/SM possam ser registrados. Controlador BLS
  algoritmos são sempre permitidos quando compilados (as chaves de consenso dependem deles),
  e a configuração padrão habilita Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Orientação do controlador Multisig

`AccountController::Multisig` serializa políticas via
`crates/iroha_data_model/src/account/controller.rs` e impõe o esquema
documentado em [`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md).
Principais detalhes de implementação:

- As políticas são normalizadas e validadas por `MultisigPolicy::validate()` antes
  sendo incorporado. Os limites devem ter peso ≥1 e ≤Σ; membros duplicados são
  removido deterministicamente após classificação por `(algorithm || 0x00 || key_bytes)`.
- A carga útil do controlador binário (`ControllerPayload::Multisig`) codifica
  `version:u8`, `threshold:u16`, `member_count:u8`, depois o de cada membro
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Isso é exatamente o que
  `AccountAddress::canonical_bytes()` grava em cargas úteis I105 (preferencial)/sora (segunda melhor).
- Hashing (`MultisigPolicy::digest_blake2b256()`) usa Blake2b-256 com o
  `iroha-ms-policy` string de personalização para que os manifestos de governança possam ser vinculados a um
  ID de política determinística que corresponde aos bytes do controlador incorporados no I105.
- A cobertura do equipamento reside em `fixtures/account/address_vectors.json` (casos
  `addr-multisig-*`). Carteiras e SDKs devem declarar as strings canônicas I105
  abaixo para confirmar se seus codificadores correspondem à implementação do Rust.

| ID do caso | Limite/membros | Literal I105 (prefixo `0x02F1`) | Sora compactado (`sora`) literal | Notas |
|---------|---------------------|--------------------------------|------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` peso, membros `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Quórum de governança no domínio do conselho. |
| `addr-multisig-wonderland-threshold2` | `≥2`, membros `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Exemplo do país das maravilhas com assinatura dupla (peso 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, membros `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Quorum de domínio padrão implícito usado para governança básica.

#### 2.4 Regras de falha (ADDR-1a)

- Cargas menores que o cabeçalho + seletor necessário ou com bytes restantes emitem `AccountAddressError::InvalidLength` ou `AccountAddressError::UnexpectedTrailingBytes`.
- Cabeçalhos que definem o `ext_flag` reservado ou anunciam versões/classes não suportadas DEVEM ser rejeitados usando `UnexpectedExtensionFlag`, `InvalidHeaderVersion` ou `UnknownAddressClass`.
- Tags de seletor/controlador desconhecidos geram `UnknownDomainTag` ou `UnknownControllerTag`.
- O material da chave superdimensionado ou malformado gera `KeyPayloadTooLong` ou `InvalidPublicKey`.
- Controladores Multisig com mais de 255 membros arrecadam `MultisigMemberOverflow`.
- Conversões IME/NFKC: Sora kana de meia largura pode ser normalizado para seus formatos de largura total sem quebrar a decodificação, mas o ASCII `sora` sentinela e os dígitos/letras I105 DEVEM permanecer ASCII. Superfícies sentinelas de largura total ou dobradas em caixa `ERR_MISSING_COMPRESSED_SENTINEL`, cargas úteis ASCII de largura total aumentam `ERR_INVALID_COMPRESSED_CHAR` e incompatibilidades de soma de verificação surgem como `ERR_CHECKSUM_MISMATCH`. Os testes de propriedade em `crates/iroha_data_model/src/account/address.rs` cobrem esses caminhos para que SDKs e carteiras possam contar com falhas determinísticas.
- A análise Torii e SDK de aliases `address@domain` (rejected legacy form) agora emite os mesmos códigos `ERR_*` quando as entradas I105 (preferencial)/sora (segunda melhor) falham antes do substituto do alias (por exemplo, incompatibilidade de soma de verificação, incompatibilidade de resumo de domínio), para que os clientes possam retransmitir razões estruturadas sem adivinhar a partir de strings em prosa.
- Cargas úteis do seletor local com menos de 12 bytes aparecem em `ERR_LOCAL8_DEPRECATED`, preservando uma transição rígida de resumos locais-8 legados.
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Vetores binários normativos

- **Domínio padrão implícito (`default`, byte inicial `0x00`)**  
  Hex canônico: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Divisão: `0x02` cabeçalho, `0x00` seletor (padrão implícito), `0x00` tag do controlador, `0x01` ID da curva (Ed25519), `0x20` comprimento da chave, seguido pela carga útil da chave de 32 bytes.
- **Resumo do domínio local (`treasury`, byte inicial `0x01`)**  
  Hex canônico: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Divisão: cabeçalho `0x02`, tag seletora `0x01` mais resumo `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, seguido pela carga útil de chave única (`0x00` tag, `0x01` ID da curva, `0x20` comprimento, Ed25519 de 32 bytes chave).

Os testes de unidade (`account::address::tests::parse_encoded_accepts_all_formats`) afirmam os vetores V1 abaixo por meio de `AccountAddress::parse_encoded`, garantindo que as ferramentas possam contar com a carga canônica em formatos hexadecimais, I105 (preferencial) e compactados (`sora`, o segundo melhor). Regenere o conjunto de acessórios estendido com `cargo run -p iroha_data_model --example address_vectors`.

| Domínio | Byte de semente | Hexágono canônico | Compactado (`sora`) |
|-------------|-----------|------------------------------------------------------------------------------------------|------------|
| padrão | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| tesouraria | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| país das maravilhas | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| irha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| alfa | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| ômega | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| governação | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| validadores | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| explorador | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Revisado por: Data Model WG, Cryptography WG — escopo aprovado para ADDR-1a.

##### Aliases de referência do Sora Nexus

As redes Sora Nexus são padrão para `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). O
Os ajudantes `AccountAddress::to_i105` e `to_i105` emitem, portanto,
formas textuais consistentes para cada carga canônica. Jogos selecionados de
`fixtures/account/address_vectors.json` (gerado via
`cargo xtask address-vectors`) são mostrados abaixo para referência rápida:

| Conta/seletor | Literal I105 (prefixo `0x02F1`) | Sora compactado (`sora`) literal |
|--------------------|--------------------------------|-----------------------------------|
| `default` domínio (seletor implícito, semente `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (sufixo `@default` opcional ao fornecer dicas de roteamento explícitas) |
| `treasury` (seletor de resumo local, semente `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Ponteiro de registro global (`registry_id = 0x0000_002A`, equivalente a `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Essas strings correspondem às emitidas pelo CLI (`iroha tools address convert`), Torii
respostas (`canonical I105 literal rendering`) e auxiliares do SDK, portanto, copiar/colar UX
os fluxos podem confiar neles literalmente. Anexe `<address>@<domain>` (rejected legacy form) somente quando precisar de uma dica de roteamento explícita; o sufixo não faz parte da saída canônica.

#### 2.6 Aliases textuais para interoperabilidade (planejado)

- **Estilo de alias de cadeia:** `ih:<chain-alias>:<alias@domain>` para logs e humanos
  entrada. As carteiras devem analisar o prefixo, verificar a cadeia incorporada e bloquear
  incompatibilidades.
- **Formulário CAIP-10:** `iroha:<caip-2-id>:<i105-addr>` para cadeia agnóstica
  integrações. Este mapeamento **ainda não está implementado** no pacote enviado
  conjuntos de ferramentas.
- **Ajudantes de máquina:** Publicar codecs para Rust, TypeScript/JavaScript, Python,
  e Kotlin cobrindo I105 e formatos compactados (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` e seus equivalentes SDK). Os ajudantes do CAIP-10 são
  trabalho futuro.

#### 2.7 Alias ​​​​determinístico I105

- **Mapeamento de prefixo:** Reutilize `chain_discriminant` como o prefixo da rede I105.
  `encode_i105_prefix()` (consulte `crates/iroha_data_model/src/account/address.rs`)
  emite um prefixo de 6 bits (byte único) para valores `<64` e um prefixo de 14 bits e dois bytes
  forma para redes maiores. As atribuições autorizadas residem em
  [`address_prefix_registry.md`](fonte/referências/address_prefix_registry.md);
  Os SDKs DEVEM manter o registro JSON correspondente sincronizado para evitar colisões.
- **Material da conta:** I105 codifica a carga útil canônica construída por
  `AccountAddress::canonical_bytes()`—byte de cabeçalho, seletor de domínio e
  carga útil do controlador. Não há etapa de hash adicional; I105 incorpora o
  carga útil do controlador binário (chave única ou multisig) conforme produzido pelo Rust
  codificador, não o mapa CTAP2 usado para resumos de políticas multisig.
- **Codificação:** `encode_i105()` concatena os bytes do prefixo com o canônico
  carga útil e anexa uma soma de verificação de 16 bits derivada de Blake2b-512 com o fixo
  Prefix: `I105PRE` (`b"I105PRE"` || prefix || payload). The result is encoded via `bs58` using the I105 alphabet.
  Os auxiliares CLI/SDK expõem o mesmo procedimento e `AccountAddress::parse_encoded`
  reverte via `decode_i105`.

#### 2.8 Vetores de teste textuais normativos

`fixtures/account/address_vectors.json` contém I105 completo (preferencial) e compactado (`sora`, segundo melhor)
literais para cada carga canônica. Destaques:

- **`addr-single-default-ed25519` (Sora Nexus, prefixo `0x02F1`).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, compactado (`sora`)
  `sora2QG…U4N5E5`. Torii emite essas strings exatas de `AccountId`'s
  Implementação `Display` (I105 canônico) e `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (seletor de registro → tesouraria).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, compactado (`sora`)
  `sorakX…CM6AEP`. Demonstra que os seletores de registro ainda decodificam para
  a mesma carga canônica do resumo local correspondente.
- **Caso de falha (`i105-prefix-mismatch`).**  
  Analisando um literal I105 codificado com o prefixo `NETWORK_PREFIX + 1` em um nó
  esperando os rendimentos do prefixo padrão
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  antes que o roteamento de domínio seja tentado. O dispositivo `i105-checksum-mismatch`
  exercita a detecção de adulteração na soma de verificação Blake2b.

#### 2.9 Dispositivos de conformidade

ADDR-2 vem com um pacote de jogos repetível que cobre pontos positivos e negativos
cenários em hexadecimal canônico, I105 (preferencial), compactado (`sora`, meia/largura total), implícito
seletores padrão, aliases de registro globais e controladores de múltiplas assinaturas. O
JSON canônico reside em `fixtures/account/address_vectors.json` e pode ser
regenerado com:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Para experimentos ad-hoc (caminhos/formatos diferentes), o exemplo binário ainda é
disponível:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

Testes de unidade de ferrugem em `crates/iroha_data_model/tests/account_address_vectors.rs`
e `crates/iroha_torii/tests/account_address_vectors.rs`, juntamente com o JS,
Chicotes Swift e Android (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
consuma o mesmo acessório para garantir a paridade do codec entre SDKs e admissão Torii.

### 3. Domínios e normalização globalmente exclusivos

Veja também: [`docs/source/references/address_norm_v1.md`](fonte/referências/address_norm_v1.md)
para o pipeline canônico Norm v1 usado no Torii, no modelo de dados e nos SDKs.

Redefina `DomainId` como uma tupla marcada:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` agrupa o nome existente para domínios gerenciados pela cadeia atual.
Quando um domínio é registrado através do registro global, persistimos a propriedade
discriminante da cadeia. A exibição/análise permanece inalterada por enquanto, mas o
estrutura expandida permite decisões de roteamento.

#### 3.1 Normalização e defesas contra falsificação

A norma v1 define o pipeline canônico que cada componente deve usar antes de um domínio
o nome é persistido ou incorporado em um `AccountAddress`. O passo a passo completo
mora em [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md);
o resumo abaixo captura as etapas que carteiras, Torii, SDKs e governança
ferramentas devem ser implementadas.

1. **Validação de entrada.** Rejeite strings vazias, espaços em branco e reservados
   delimitadores `@`, `#`, `$`. Isso corresponde aos invariantes impostos por
   `Name::validate_str`.
2. **Composição NFC Unicode.** Aplicar a normalização NFC apoiada pela ICU de forma canônica
   sequências equivalentes entram em colapso deterministicamente (por exemplo, `e\u{0301}` → `é`).
3. **Normalização UTS-46.** Execute a saída NFC através de UTS‑46 com
   `use_std3_ascii_rules = true`, `transitional_processing = false` e
   Imposição de comprimento DNS habilitada. O resultado é uma sequência de rótulo A minúsculo;
   entradas que violam as regras STD3 falham aqui.
4. **Limites de comprimento.** Aplique os limites no estilo DNS: cada rótulo DEVE ser de 1 a 63
   bytes e o domínio completo NÃO DEVEM exceder 255 bytes após a etapa 3.
5. **Política confusa opcional.** As verificações de script UTS-39 são rastreadas para
   Norma v2; os operadores podem ativá-los antecipadamente, mas a falha na verificação deve abortar
   processamento.

Se cada estágio for bem-sucedido, a string minúscula do rótulo A será armazenada em cache e usada para
codificação de endereço, configuração, manifestos e pesquisas de registro. Resumo local
seletores derivam seu valor de 12 bytes como `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` usando a saída da etapa 3. Todas as outras tentativas (mistas
caixa, maiúscula, entrada Unicode bruta) são rejeitados com estrutura
`ParseError`s no limite onde o nome foi fornecido.

Acessórios canônicos demonstrando essas regras - incluindo viagens de ida e volta em punycode
e sequências STD3 inválidas – estão listadas em
`docs/source/references/address_norm_v1.md` e são espelhados no SDK CI
conjuntos de vetores rastreados sob ADDR-2.

### 4. Registro e roteamento de domínio Nexus

- **Esquema de registro:** Nexus mantém um mapa assinado `DomainName -> ChainRecord`
  onde `ChainRecord` inclui o discriminante de cadeia, metadados opcionais (RPC
  endpoints) e uma prova de autoridade (por exemplo, assinatura múltipla de governança).
- **Mecanismo de sincronização:**
  - As cadeias enviam reivindicações de domínio assinadas ao Nexus (durante a gênese ou via
    instrução de governança).
  - Nexus publica manifestos periódicos (JSON assinado mais raiz Merkle opcional)
    sobre HTTPS e armazenamento endereçado a conteúdo (por exemplo, IPFS). Os clientes fixam o
    último manifesto e verificação de assinaturas.
- **Fluxo de pesquisa:**
  - Torii recebe uma transação referenciando `DomainId`.
  - Se o domínio for desconhecido localmente, o Torii consulta o manifesto do Nexus em cache.
  - Se o manifesto indicar uma cadeia estrangeira, a transação é rejeitada com
    um erro determinístico `ForeignDomain` e as informações da cadeia remota.
  - Se o domínio estiver faltando no Nexus, o Torii retornará `UnknownDomain`.
- **Âncoras de confiança e rotação:** Chaves de governança assinam manifestos; rotação ou
  a revogação é publicada como uma nova entrada de manifesto. Os clientes impõem manifesto
  TTLs (por exemplo, 24h) e recusam-se a consultar dados obsoletos além dessa janela.
- **Modos de falha:** Se a recuperação do manifesto falhar, o Torii volta para o cache
  dados dentro do TTL; passado TTL emite `RegistryUnavailable` e recusa
  roteamento entre domínios para evitar estado inconsistente.

### 4.1 Imutabilidade, aliases e marcas de exclusão do registro (ADDR-7c)

O Nexus publica um **manifesto somente para acréscimos** para que cada atribuição de domínio ou alias
podem ser auditados e reproduzidos. Os operadores devem tratar o pacote descrito no
[runbook de manifesto de endereço](source/runbooks/address_manifest_ops.md) como o
única fonte da verdade: se um manifesto estiver faltando ou falhar na validação, Torii deve
recusar-se a resolver o domínio afetado.

Suporte de automação: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
reproduz a soma de verificação, o esquema e as verificações de resumo anterior explicadas no
livro de bordo. Inclua a saída do comando nos tickets de alteração para mostrar o `sequence`
e a ligação `previous_digest` foi validada antes da publicação do pacote.

#### Cabeçalho do manifesto e contrato de assinatura

| Campo | Requisito |
|-------|------------|
| `version` | Atualmente `1`. Bump apenas com uma atualização de especificação correspondente. |
| `sequence` | Aumente em **exatamente** um por publicação. Os caches Torii recusam revisões com lacunas ou regressões. |
| `generated_ms` + `ttl_hours` | Estabeleça a atualização do cache (padrão 24h). Se o TTL expirar antes da próxima publicação, Torii muda para `RegistryUnavailable`. |
| `previous_digest` | Resumo BLAKE3 (hex) do corpo do manifesto anterior. Os verificadores recomputam-no com `b3sum` para provar a imutabilidade. |
| `signatures` | Os manifestos são assinados via Sigstore (`cosign sign-blob`). As operações devem executar `cosign verify-blob --bundle manifest.sigstore manifest.json` e impor as restrições de identidade/emissor de governança antes da implementação. |

A automação de liberação emite `manifest.sigstore` e `checksums.sha256`
ao lado do corpo JSON. Mantenha os arquivos juntos ao espelhar para SoraFS ou
Endpoints HTTP para que os auditores possam repetir as etapas de verificação literalmente.

#### Tipos de entrada

| Tipo | Finalidade | Campos obrigatórios |
|------|---------|-----------------|
| `global_domain` | Declara que um domínio é registrado globalmente e deve ser mapeado para um discriminante de cadeia e um prefixo I105. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Retira um alias/seletor permanentemente. Obrigatório ao apagar resumos do Local-8 ou remover um domínio. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

As entradas `global_domain` podem opcionalmente incluir `manifest_url` ou `sorafs_cid`
para apontar carteiras para metadados de cadeia assinada, mas a tupla canônica permanece
`{domain, chain, discriminant/i105_prefix}`. `tombstone` registros **devem** citar
o seletor sendo retirado e o artefato de ticket/governança que autorizou
a mudança para que a trilha de auditoria possa ser reconstruída off-line.

#### Fluxo de trabalho e telemetria de alias/tombstone

1. **Detectar desvio.** Use `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}` e
   `torii_address_invalid_total{endpoint,reason}` (renderizado em
   `dashboards/grafana/address_ingest.json`) para confirmar envios locais e
   As colisões locais 12 permanecem em zero antes de propor uma lápide. O
   contadores por domínio permitem que os proprietários provem que apenas domínios de desenvolvimento/teste emitem Local-8
   tráfego (e que as colisões Local-12 sejam mapeadas para domínios de teste conhecidos) enquanto
   inclui o painel **Domain Kind Mix (5m)** para que os SREs possam representar graficamente quanto
   `domain_kind="local12"` o tráfego permanece e o `AddressLocal12Traffic`
   o alerta é acionado sempre que a produção ainda vê os seletores Local-12, apesar do
   portão de aposentadoria.
2. **Derivar resumos canônicos.** Executar
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (ou consuma `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) para capturar o `digest_hex` exato.
   A CLI aceita literais I105, `i105` e `0x…` canônicos; anexar
   `@<domain>` somente quando você precisar preservar um rótulo para manifestos.
   O resumo JSON mostra esse domínio por meio do campo `input_domain` e
   `legacy  suffix` reproduz a codificação convertida como `<address>@<domain>` (rejected legacy form) para
   diferenças de manifesto (este sufixo é metadados, não um ID de conta canônico).
   Para exportações orientadas para nova linha, use
   `iroha tools address normalize --input <file> legacy-selector input mode` para conversão local em massa
   seletores em formatos canônicos I105 (preferencial), compactado (`sora`, segundo melhor), hexadecimal ou JSON ao pular
   linhas não locais. Quando os auditores precisarem de evidências em planilhas, execute
   `iroha tools address audit --input <file> --format csv` para emitir um resumo CSV
   (`input,status,format,domain_kind,…`) que destaca os seletores locais,
   codificações canônicas e falhas de análise no mesmo arquivo.
3. **Anexar entradas do manifesto.** Rascunhe o registro `tombstone` (e o acompanhamento
   `global_domain` ao migrar para o registro global) e validar
   o manifesto com `cargo xtask address-vectors` antes de solicitar assinaturas.
4. **Verifique e publique.** Siga a lista de verificação do runbook (hashes, Sigstore,
   monotonicidade de sequência) antes de espelhar o pacote para SoraFS. Torii agora
   canoniza literais I105 (preferenciais)/sora (segundo melhor) imediatamente após o pacote chegar.
5. **Monitoramento e reversão.** Mantenha os painéis de colisão Local-8 e Local-12 em
   zero por 30 dias; se aparecerem regressões, republicar o manifesto anterior
   apenas no ambiente de não produção afetado até que a telemetria se estabilize.

Todas as etapas acima são evidências obrigatórias para ADDR-7c: manifestos sem
o pacote de assinatura `cosign` ou sem valores `previous_digest` correspondentes devem
ser rejeitado automaticamente, e os operadores devem anexar os registros de verificação ao
seus bilhetes de mudança.

### 5. Ergonomia de carteira e API

- **Padrões de exibição:** As carteiras mostram o endereço I105 (abreviado, soma de verificação)
  mais o domínio resolvido como um rótulo obtido do registro. Domínios são
  claramente marcados como metadados descritivos que podem mudar, enquanto I105 é o
  endereço estável.
- **Canonização de entrada:** Torii e SDKs aceitam I105 (preferencial)/sora (segundo melhor)/0x
  endereços mais `alias@domain` (rejected legacy form), `uaid:…` e
  `opaque:…` formulários e, em seguida, canonize para I105 para saída. Não há
  alternância de modo estrito; identificadores brutos de telefone/e-mail devem ser mantidos fora do registro
  via mapeamentos UAID/opacos.
- **Prevenção de erros:** Carteiras analisam prefixos I105 e impõem discriminação de cadeia
  expectativas. As incompatibilidades de cadeia desencadeiam falhas graves com diagnósticos acionáveis.
- **Bibliotecas de codecs:** Rust oficial, TypeScript/JavaScript, Python e Kotlin
  bibliotecas fornecem codificação/decodificação I105 mais suporte compactado (`sora`) para
  evite implementações fragmentadas. As conversões CAIP-10 ainda não foram enviadas.

#### Orientações sobre acessibilidade e compartilhamento seguro

- As orientações de implementação para superfícies de produtos são rastreadas ao vivo em
  `docs/portal/docs/reference/address-safety.md`; consulte essa lista de verificação quando
  adaptando esses requisitos à carteira ou ao explorer UX.
- **Fluxos de compartilhamento seguros:** superfícies que copiam ou exibem endereços usam como padrão o formulário I105 e expõem uma ação de “compartilhamento” adjacente que apresenta a string completa e um código QR derivado da mesma carga para que os usuários possam verificar a soma de verificação visualmente ou por digitalização. Quando o truncamento for inevitável (por exemplo, telas pequenas), retenha o início e o fim da string, adicione reticências claras e mantenha o endereço completo acessível por meio de cópia para a área de transferência para evitar recortes acidentais.
- **Proteções de IME:** As entradas de endereço DEVEM rejeitar artefatos de composição de teclados estilo IME/IME. Aplique entrada somente ASCII, apresente um aviso embutido quando caracteres de largura total ou Kana forem detectados e ofereça uma zona de colagem de texto simples que remove marcas de combinação antes da validação para que usuários japoneses e chineses possam desabilitar seu IME sem perder o progresso.
- **Suporte para leitor de tela:** Forneça rótulos visualmente ocultos (`aria-label`/`aria-describedby`) que descrevem os dígitos iniciais do prefixo I105 e dividem a carga útil do I105 em grupos de 4 ou 8 caracteres, para que a tecnologia assistiva leia caracteres agrupados em vez de uma string contínua. Anuncie o sucesso de cópia/compartilhamento por meio de regiões ao vivo educadas e garanta que as visualizações de QR incluam texto alternativo descritivo (“Endereço I105 para <alias> na cadeia 0x02F1”).
- **Uso compactado somente Sora:** Sempre rotule a visualização compactada `i105` como “somente Sora” e proteja-a por trás de uma confirmação explícita antes de copiar. SDKs e carteiras devem se recusar a exibir resultados compactados quando o discriminante da cadeia não for o valor Sora Nexus e devem direcionar os usuários de volta ao I105 para transferências entre redes para evitar o desvio de fundos.

## Lista de verificação de implementação

- **Envelope I105:** O prefixo codifica o `chain_discriminant` usando o compacto
  Esquema de 6/14 bits de `encode_i105_prefix()`, o corpo são os bytes canônicos
  (`AccountAddress::canonical_bytes()`), e a soma de verificação são os dois primeiros bytes
  de Blake2b-512(`b"I105PRE"` || prefixo || corpo). The full payload is encoded via `bs58` using the I105 alphabet.
- **Contrato de registro:** Publicação JSON assinada (e raiz Merkle opcional)
  `{discriminant, i105_prefix, chain_alias, endpoints}` com TTL 24h e
  chaves de rotação.
- **Política de domínio:** ASCII `Name` hoje; se estiver habilitando i18n, aplique UTS-46 para
  normalização e UTS-39 para verificações confusas. Aplicar rótulo máximo (63) e
  comprimentos totais (255).
- **Ajudantes textuais:** Envie codecs I105 ↔ compactados (`i105`) em Rust,
  TypeScript/JavaScript, Python e Kotlin com vetores de teste compartilhados (CAIP-10
  mapeamentos permanecem como trabalho futuro).
- **Ferramentas CLI:** fornece um fluxo de trabalho determinístico do operador via `iroha tools address convert`
  (consulte `crates/iroha_cli/src/address.rs`), que aceita literais I105/`0x…` e
  rótulos opcionais `<address>@<domain>` (rejected legacy form), o padrão é a saída I105 usando o prefixo Sora Nexus (`753`),
  e só emite o alfabeto compactado somente Sora quando os operadores o solicitam explicitamente com
  `--format i105` ou o modo de resumo JSON. O comando impõe expectativas de prefixo em
  analisar, registra o domínio fornecido (`input_domain` em JSON) e o sinalizador `legacy  suffix`
  reproduz a codificação convertida como `<address>@<domain>` (rejected legacy form) para que as diferenças do manifesto permaneçam ergonômicas.
- **Wallet/explorer UX:** siga as [diretrizes de exibição de endereço](source/sns/address_display_guidelines.md)
  fornecido com ADDR-6 - oferece botões de cópia dupla, mantém I105 como carga útil QR e avisa
  usuários que o formulário `i105` compactado é somente Sora e suscetível a reescritas de IME.
- **Integração Torii:** Cache Nexus manifesta respeitando TTL, emite
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` deterministicamente, e
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### Formatos de resposta Torii

- `GET /v1/accounts` aceita um parâmetro de consulta opcional `canonical I105 rendering` e
  `POST /v1/accounts/query` aceita o mesmo campo dentro do envelope JSON.
  Os valores suportados são:
  - `i105` (padrão) — as respostas emitem cargas úteis I105 canônicas (por exemplo,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `i105_default` — as respostas emitem a visualização compactada `i105` somente Sora enquanto
    mantendo filtros/parâmetros de caminho canônicos.
- Valores inválidos retornam `400` (`QueryExecutionFail::Conversion`). Isso permite
  carteiras e exploradores para solicitar strings compactadas para UX somente Sora enquanto
  mantendo I105 como padrão interoperável.
- Listagens de detentores de ativos (`GET /v1/assets/{definition_id}/holders`) e seu JSON
  contraparte do envelope (`POST …/holders/query`) também homenageia `canonical I105 rendering`.
  O campo `items[*].account_id` emite literais compactados sempre que o
  campo parâmetro/envelope está definido como `i105_default`, espelhando as contas
  endpoints para que os exploradores possam apresentar resultados consistentes em todos os diretórios.
- **Teste:** Adicionar testes de unidade para viagens de ida e volta do codificador/decodificador, cadeia errada
  falhas e pesquisas de manifesto; adicione cobertura de integração em Torii e SDKs
  para fluxos I105 ponta a ponta.

## Registro de código de erro

Codificadores e decodificadores de endereços expõem falhas por meio de
`AccountAddressError::code_str()`. As tabelas a seguir fornecem os códigos estáveis
que SDKs, carteiras e superfícies Torii devem aparecer junto com legíveis por humanos
mensagens, além de orientações de correção recomendadas.

### Construção Canônica

| Código | Falha | Correção recomendada |
|------|---------|-------------|
| `ERR_UNSUPPORTED_ALGORITHM` | O codificador recebeu um algoritmo de assinatura não suportado pelo registro ou pelos recursos de compilação. | Restrinja a construção de contas às curvas habilitadas no registro e na configuração. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | O comprimento da carga útil da chave de assinatura excede o limite suportado. | Os controladores de tecla única são limitados a comprimentos `u8`; use multisig para chaves públicas grandes (por exemplo, ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | A versão do cabeçalho do endereço está fora do intervalo compatível. | Emitir versão do cabeçalho `0` para endereços V1; atualize os codificadores antes de adotar novas versões. |
| `ERR_INVALID_NORM_VERSION` | O sinalizador de versão de normalização não é reconhecido. | Use a versão de normalização `1` e evite alternar bits reservados. |
| `ERR_INVALID_I105_PREFIX` | O prefixo de rede I105 solicitado não pode ser codificado. | Escolha um prefixo dentro do intervalo `0..=16383` inclusivo publicado no registro da cadeia. |
| `ERR_CANONICAL_HASH_FAILURE` | O hash de carga canônica falhou. | Tente novamente a operação; se o erro persistir, trate-o como um bug interno na pilha de hashing. |

### Decodificação de formato e detecção automática

| Código | Falha | Correção recomendada |
|------|---------|-------------|
| `ERR_INVALID_I105_ENCODING` | A string I105 contém caracteres fora do alfabeto. | Certifique-se de que o endereço use o alfabeto I105 publicado e não tenha sido truncado durante copiar/colar. |
| `ERR_INVALID_LENGTH` | O comprimento da carga útil não corresponde ao tamanho canônico esperado para o seletor/controlador. | Forneça a carga canônica completa para o seletor de domínio selecionado e o layout do controlador. |
| `ERR_CHECKSUM_MISMATCH` | Falha na validação da soma de verificação I105 (preferencial) ou compactada (`sora`, segundo melhor). | Gere novamente o endereço de uma fonte confiável; isso normalmente indica um erro de copiar/colar. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | Os bytes do prefixo I105 estão mal formados. | Codifique novamente o endereço com um codificador compatível; não altere os bytes I105 iniciais manualmente. |
| `ERR_INVALID_HEX_ADDRESS` | A forma hexadecimal canônica não foi decodificada. | Forneça uma string hexadecimal de comprimento par com prefixo `0x` produzida pelo codificador oficial. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | O formulário compactado não começa com `sora`. | Prefixe os endereços Sora compactados com o sentinela necessário antes de entregá-los aos decodificadores. |
| `ERR_COMPRESSED_TOO_SHORT` | A string compactada não possui dígitos suficientes para carga útil e soma de verificação. | Use a string compactada completa emitida pelo codificador em vez de trechos truncados. |
| `ERR_INVALID_COMPRESSED_CHAR` | Caractere fora do alfabeto compactado encontrado. | Substitua o caractere por um glifo Base-105 válido das tabelas publicadas de meia largura/largura total. |
| `ERR_INVALID_COMPRESSED_BASE` | O codificador tentou usar uma raiz não suportada. | Registre um bug no codificador; o alfabeto compactado é fixado na base 105 em V1. |
| `ERR_INVALID_COMPRESSED_DIGIT` | O valor do dígito excede o tamanho do alfabeto compactado. | Certifique-se de que cada dígito esteja dentro de `0..105)`, regenerando o endereço se necessário. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | A detecção automática não conseguiu reconhecer o formato de entrada. | Forneça strings hexadecimais I105 (preferencial), compactadas (`sora`) ou canônicas `0x` ao invocar analisadores. |

### Validação de domínio e rede

| Código | Falha | Correção recomendada |
|------|---------|-------------|
| `ERR_DOMAIN_MISMATCH` | O seletor de domínio não corresponde ao domínio esperado. | Utilize um endereço emitido para o domínio pretendido ou atualize a expectativa. |
| `ERR_INVALID_DOMAIN_LABEL` | O rótulo do domínio falhou nas verificações de normalização. | Canonize o domínio usando o processamento não transicional UTS-46 antes da codificação. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | O prefixo da rede I105 decodificado difere do valor configurado. | Mude para um endereço da cadeia de destino ou ajuste o discriminante/prefixo esperado. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Os bits da classe de endereço não são reconhecidos. | Atualize o decodificador para uma versão que compreenda a nova classe ou evite adulterar os bits do cabeçalho. |
| `ERR_UNKNOWN_DOMAIN_TAG` | A tag do seletor de domínio é desconhecida. | Atualize para uma versão que suporte o novo tipo de seletor ou evite usar cargas experimentais em nós V1. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | O bit de extensão reservado foi definido. | Limpar bits reservados; eles permanecem fechados até que uma futura ABI os apresente. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Tag de carga útil do controlador não reconhecida. | Atualize o decodificador para reconhecer novos tipos de controlador antes de analisá-los. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | A carga canônica continha bytes finais após a decodificação. | Regenerar a carga canônica; apenas o comprimento documentado deve estar presente. |

### Validação de carga útil do controlador

| Código | Falha | Correção recomendada |
|------|---------|-------------|
| `ERR_INVALID_PUBLIC_KEY` | Os bytes da chave não correspondem à curva declarada. | Certifique-se de que os bytes da chave sejam codificados exatamente conforme necessário para a curva selecionada (por exemplo, Ed25519 de 32 bytes). |
| `ERR_UNKNOWN_CURVE` | O identificador da curva não está registrado. | Use o ID da curva `1` (Ed25519) até que curvas adicionais sejam aprovadas e publicadas no registro. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | O controlador Multisig declara mais membros do que o suportado. | Reduza a associação multisig ao limite documentado antes da codificação. |
| `ERR_INVALID_MULTISIG_POLICY` | A carga útil da política multisig falhou na validação (limite/pesos/esquema). | Reconstrua a política para que ela satisfaça o esquema CTAP2, os limites de peso e as restrições de limite. |

## Alternativas consideradas

- **Pure checksum envelope (estilo Bitcoin).** Soma de verificação mais simples, mas detecção de erros mais fraca
  do que a soma de verificação I105 derivada de Blake2b (`encode_i105` trunca um hash de 512 bits)
  e carece de semântica de prefixo explícita para discriminantes de 16 bits.
- **Incorporação do nome da cadeia na string de domínio (por exemplo, `finance@chain`).** Quebras
- **Confie exclusivamente no roteamento Nexus sem alterar endereços.** Os usuários ainda assim
  copiar/colar strings ambíguas; queremos que o próprio endereço carregue contexto.
- **Envelope Bech32m.** Compatível com QR e oferece um prefixo legível por humanos, mas
  divergiria da implementação de remessa I105 (`AccountAddress::to_i105`)
  e exigir a recriação de todos os fixtures/SDKs. O roteiro atual mantém o I105+
  suporte compactado (@INLINE_CODE_490@@) enquanto continua a pesquisa sobre o futuro
  Camadas Bech32m/QR (o mapeamento CAIP-10 é adiado).

## Perguntas abertas

- Confirmar que `u16` discriminantes mais faixas reservadas cobrem a demanda de longo prazo;
  caso contrário, avalie `u32` com codificação varint.
- Finalizar o processo de governança de múltiplas assinaturas para atualizações de registro e como
  revogações/alocações expiradas são tratadas.
- Definir o esquema exato de assinatura do manifesto (por exemplo, Ed25519 multi-sig) e
  segurança de transporte (fixação HTTPS, formato hash IPFS) para distribuição Nexus.
- Determinar se será compatível com aliases/redirecionamentos de domínio para migrações e como
  para trazê-los à tona sem quebrar o determinismo.
- Especifique como os contratos Kotodama/IVM acessam os auxiliares I105 (`to_address()`,
  `parse_address()`) e se o armazenamento on-chain deveria expor o CAIP-10
  mapeamentos (hoje I105 é canônico).
- Explorar o registro de cadeias Iroha em registros externos (por exemplo, registro I105,
  Diretório de namespace CAIP) para um alinhamento mais amplo do ecossistema.

## Próximas etapas

1. A codificação I105 chegou a `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); continue portando fixtures/testes para cada SDK e limpe qualquer
   Espaços reservados Bech32m.
2. Estenda o esquema de configuração com `chain_discriminant` e obtenha sentido
  padrões para configurações de teste/desenvolvimento existentes. **(Concluído: `common.chain_discriminant`
  agora é enviado em `iroha_config`, padrão para `0x02F1` com por rede
  substituições.)**
3. Elabore o esquema de registro do Nexus e o editor do manifesto de prova de conceito.
4. Colete feedback de fornecedores de carteiras e custodiantes sobre aspectos de fatores humanos
   (Nomenclatura HRP, formatação de exibição).
5. Atualize a documentação (`docs/source/data_model.md`, documentos da API Torii) assim que o
   o caminho de implementação está comprometido.
6. Envie bibliotecas oficiais de codecs (Rust/TS/Python/Kotlin) com teste normativo
   vetores que abrangem casos de sucesso e fracasso.
