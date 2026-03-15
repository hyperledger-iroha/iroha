---
lang: pt
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T18:50:10.586502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Resumo da arquitetura de ativação SM2/SM3/SM4 para Hyperledger Iroha v2.

# Resumo da arquitetura do programa SM

## Objetivo
Defina o plano técnico, a postura da cadeia de suprimentos e os limites de risco para a introdução da criptografia nacional chinesa (SM2/SM3/SM4) em toda a pilha Iroha v2, preservando a execução determinística e a auditabilidade.

## Escopo
- **Caminhos críticos de consenso:** `iroha_crypto`, `iroha`, `irohad`, host IVM, intrínsecos Kotodama.
- **SDKs e ferramentas do cliente:** Rust CLI, Kagami, SDKs Python/JS/Swift, utilitários genesis.
- **Configuração e serialização:** Botões `iroha_config`, tags de modelo de dados Norito, manipulação de manifesto, atualizações de multicodec.
- **Testes e conformidade:** Conjuntos de unidade/propriedade/interoperabilidade, chicotes à prova de Wyche, perfil de desempenho, orientação de exportação/regulatória. *(Status: pilha SM apoiada por RustCrypto mesclada; conjunto fuzz `sm_proptest` opcional e chicote de paridade OpenSSL disponível para CI estendido.)*

Fora do escopo: algoritmos PQ, aceleração de host não determinística em caminhos de consenso; As compilações wasm/`no_std` foram descontinuadas.

## Entradas e resultados do algoritmo
| Artefato | Proprietário | Vencimento | Notas |
|----------|-------|-----|-------|
| Design de recurso de algoritmo SM (`SM-P0`) | GT de criptografia | 2025-02 | Controle de recursos, auditoria de dependências, registro de riscos. |
| Integração Core Rust (`SM-P1`) | Crypto WG / Modelo de Dados | 2025-03 | Auxiliares de verificação/hash/AEAD baseados em RustCrypto, extensões Norito, acessórios. |
| Assinatura + syscalls VM (`SM-P2`) | Programa Núcleo/SDK IVM | 2025-04 | Wrappers de assinatura determinística, syscalls, cobertura Kotodama. |
| Ativação opcional de provedor e operações (`SM-P3`) | GT de operações/desempenho de plataforma | 2025-06 | Backend OpenSSL/Tongsuo, intrínsecos ARM, telemetria, documentação. |

## Bibliotecas selecionadas
- **Primário:** Caixas RustCrypto (`sm2`, `sm3`, `sm4`) com recurso `rfc6979` habilitado e SM3 vinculado a nonces determinísticos.
- **FFI opcional:** API do provedor OpenSSL 3.x ou Tongsuo para implantações que exigem pilhas certificadas ou mecanismos de hardware; limitado por recursos e desabilitado por padrão em binários de consenso.### Status de integração da biblioteca principal
- `iroha_crypto::sm` expõe hashing SM3, verificação SM2 e auxiliares SM4 GCM/CCM sob o recurso `sm` unificado, com caminhos de assinatura RFC6979 determinísticos disponíveis para SDKs via `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Tags Norito/Norito-JSON e auxiliares multicodec cobrem chaves/assinaturas públicas SM2 e cargas úteis SM3/SM4 para que as instruções sejam serializadas deterministicamente entre hosts.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- Conjuntos de respostas conhecidas validam a integração RustCrypto (`sm3_sm4_vectors.rs`, `sm2_negative_vectors.rs`) e são executados como parte dos trabalhos de recurso `sm` do CI, mantendo a verificação determinística enquanto os nós continuam assinando com Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- Validação de compilação do recurso `sm` opcional: `cargo check -p iroha_crypto --features sm --locked` (frio 7,9s/quente 0,23s) e `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1,0s) ambos bem-sucedidos; ativar o recurso adiciona 11 caixas (`base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4`, `sm4-gcm`). Descobertas registradas em `docs/source/crypto/sm_rustcrypto_spike.md`.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- Dispositivos de verificação negativa BouncyCastle/GmSSL vivem sob `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json`, garantindo que casos de falha canônica (r = 0, s = 0, incompatibilidade de ID distinto, chave pública adulterada) permaneçam alinhados com amplamente implantados provedores.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` agora compila o conjunto de ferramentas OpenSSL 3.x vendido (recurso `openssl` crate `vendored`), portanto, compilações e testes de visualização sempre visam um provedor moderno compatível com SM, mesmo quando o sistema LibreSSL/OpenSSL não possui algoritmos SM.【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` agora detecta AArch64 NEON em tempo de execução e encadeia os ganchos SM3/SM4 por meio do despacho x86_64/RISC-V enquanto respeita o botão de configuração `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`). Quando os back-ends vetoriais estão ausentes, o despachante ainda roteia através do caminho escalar RustCrypto para que bancos e alternâncias de políticas se comportem de forma consistente entre os hosts.【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Esquema Norito e superfícies de modelo de dados| Tipo Norito/consumidor | Representação | Restrições e notas |
|------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | Bare: blob de 32 bytes · JSON: string hexadecimal maiúscula (`"4F4D..."`) | Envolvimento de tupla canônica Norito `[u8; 32]`. A decodificação JSON/Bare rejeita comprimentos ≠ 32. Viagens de ida e volta cobertas por `sm_norito_roundtrip::sm3_digest_norito_roundtrip`. |
| `Sm2PublicKey` / `Sm2Signature` | Blobs prefixados multicodec (`0x1306` provisório) | As chaves públicas codificam pontos SEC1 não compactados; as assinaturas são `(r∥s)` (32 bytes cada) com proteções de análise DER. |
| `Sm4Key` | Nu: blob de 16 bytes | Zerando o wrapper exposto a Kotodama/CLI. A serialização JSON é omitida deliberadamente; os operadores devem passar chaves por meio de blobs (contratos) ou hexadecimal CLI (`--key-hex`). |
| Operandos `sm4_gcm_seal/open` | Tupla de 4 blobs: `(key, nonce, aad, payload)` | Chave = 16 bytes; nonce = 12 bytes; comprimento da tag fixado em 16 bytes. Retorna `(ciphertext, tag)`; Kotodama/CLI emite auxiliares hexadecimais e base64.【crates/ivm/tests/sm_syscalls.rs:728】 |
| Operandos `sm4_ccm_seal/open` | Tupla de 4 blobs (chave, nonce, aad, carga útil) + comprimento da tag em `r14` | Nonce 7–13 bytes; comprimento da etiqueta ∈ {4,6,8,10,12,14,16}. O recurso `sm` expõe o CCM por trás do sinalizador `sm-ccm`. |
| Intrínsecos Kotodama (`sm::hash`, `sm::seal_gcm`, `sm::open_gcm`,…) | Mapa para SCALLs acima | A validação de entrada reflete as regras do host; tamanhos malformados aumentam `ExecutionError::Type`. |

Referência rápida de uso:
- **Hashing SM3 em contratos/testes:** `Sm3Digest::hash(b"...")` (Rust) ou Kotodama `sm::hash(input_blob)`. JSON espera 64 caracteres hexadecimais.
- **SM4 AEAD via CLI:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` produz pares de texto cifrado + tag hex/base64. Descriptografe com `gcm-open` correspondente.
- **Sequências de caracteres multicodec:** Chaves/assinaturas públicas SM2 analisam de/para a sequência multibase aceita por `PublicKey::from_str`/`Signature::from_bytes`, permitindo que manifestos e IDs de conta Norito transportem signatários SM.

Os consumidores de modelos de dados devem tratar as chaves e tags SM4 como blobs transitórios; nunca persista chaves brutas na cadeia. Os contratos devem armazenar apenas saídas de texto cifrado/tag ou resumos derivados (por exemplo, SM3 da chave) quando a auditoria for necessária.

### Cadeia de suprimentos e licenciamento
| Componente | Licença | Mitigação |
|-----------|---------|-----------|
| `sm2`, `sm3`, `sm4` | Apache-2.0/MIT | Rastreie commits upstream, fornecedor se forem necessárias versões lockstep, agende auditoria de terceiros antes de o validador assinar o GA. |
| `rfc6979` | Apache-2.0/MIT | Já utilizado em outros algoritmos; confirme a ligação determinística `k` com o resumo SM3. |
| OpenSSL/Tongsuo opcional | Apache-2.0 / estilo BSD | Manter o recurso `sm-ffi-openssl`, exigir aceitação explícita do operador e lista de verificação de empacotamento. |### Sinalizadores de recursos e propriedade
| Superfície | Padrão | Mantenedor | Notas |
|--------|---------|------------|-------|
| `iroha_crypto/sm-core`, `sm-ccm`, `sm` | Desativado | GT de criptografia | Ativa primitivas RustCrypto SM; `sm` agrupa auxiliares CCM para clientes que exigem criptografia autenticada. |
| `ivm/sm` | Desativado | Equipe principal IVM | Constrói syscalls SM (`sm3_hash`, `sm2_verify`, `sm4_gcm_*`, `sm4_ccm_*`). O controle de host deriva de `crypto.allowed_signing` (presença de `sm2`). |
| `iroha_crypto/sm_proptest` | Desativado | GT de controle de qualidade / criptografia | Equipamento de teste de propriedade que cobre assinaturas/tags malformadas. Habilitado apenas em CI estendido. |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | GT Configuração / GT Operadoras | A presença do hash `sm2` mais `sm3-256` permite syscalls/assinaturas SM; remover `sm2` retorna ao modo somente verificação. |
| Opcional `sm-ffi-openssl` (visualização) | Desativado | Operações de plataforma | Recurso de espaço reservado para integração de provedor OpenSSL/Tongsuo; permanece desativado até que os POPs de certificação e embalagem cheguem. |

A política de rede agora expõe `network.require_sm_handshake_match` e
`network.require_sm_openssl_preview_match` (ambos padrão para `true`). Limpar qualquer sinalizador permite
implantações mistas onde observadores somente Ed25519 se conectam a validadores habilitados para SM; incompatibilidades são
registrado em `WARN`, mas os nós de consenso devem manter os padrões habilitados para evitar acidentes
divergência entre pares com conhecimento de SM e com deficiência de SM.
A CLI apresenta essas alternâncias por meio da atualização de handshake do `iroha_cli app sorafs
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
sinalizadores para restaurar a aplicação estrita.#### Visualização do OpenSSL/Tongsuo (`sm-ffi-openssl`)
- **Escopo.** Cria um shim de provedor somente para visualização (`OpenSslProvider`) que valida a disponibilidade do tempo de execução OpenSSL e expõe hashing SM3 apoiado por OpenSSL, verificação SM2 e criptografia/descriptografia SM4-GCM, permanecendo ativo. Os binários de consenso devem continuar usando o caminho RustCrypto; o back-end da FFI é estritamente opcional para pilotos de verificação/assinatura de borda.
- **Pré-requisitos de compilação.** Compile com `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` e garanta os links do conjunto de ferramentas com OpenSSL/Tongsuo 3.0+ (`libcrypto` com suporte SM2/SM3/SM4). A vinculação estática é desencorajada; prefira bibliotecas dinâmicas gerenciadas pelo operador.
- **Teste de fumaça do desenvolvedor.** Execute `scripts/sm_openssl_smoke.sh` para executar `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` seguido por `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`; o auxiliar pula automaticamente quando os cabeçalhos de desenvolvimento OpenSSL ≥3 estão indisponíveis (ou `pkg-config` está faltando) e exibe a saída de fumaça para que os desenvolvedores possam ver se a verificação SM2 foi executada ou voltou para a implementação Rust.
- **Rust scaffolding.** O módulo `openssl_sm` agora roteia hashing SM3, verificação SM2 (pré-hash ZA + SM2 ECDSA) e criptografia/descriptografia SM4 GCM por meio de OpenSSL com erros estruturados cobrindo alternâncias de visualização e comprimentos de chave/nonce/tag inválidos; SM4 CCM permanece puro-Rust apenas até que calços FFI adicionais aterrem.
- **Comportamento de pular.** Quando cabeçalhos ou bibliotecas OpenSSL ≥3.0 estão ausentes, o teste de fumaça imprime um banner de pular (via `-- --nocapture`), mas ainda sai com êxito para que o CI possa distinguir lacunas de ambiente de regressões genuínas.
- **Proteções de tempo de execução.** A visualização do OpenSSL está desabilitada por padrão; habilite-o por meio da configuração (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) antes de tentar usar o caminho FFI. Mantenha os clusters de produção no modo somente verificação (omita `sm2` de `allowed_signing`) até que o provedor se forme, conte com o substituto determinístico do RustCrypto e confine os pilotos de assinatura a ambientes isolados.
- **Lista de verificação de pacotes.** Documente a versão do provedor, o caminho de instalação e os hashes de integridade nos manifestos de implantação. Os operadores devem fornecer scripts de instalação que instalem a compilação OpenSSL/Tongsuo aprovada, registre-a no armazenamento confiável do sistema operacional (se necessário) e fixe as atualizações atrás das janelas de manutenção.
- **Próximas etapas.** Marcos futuros adicionam ligações determinísticas SM4 CCM FFI, trabalhos de fumaça CI (consulte `ci/check_sm_openssl_stub.sh`) e telemetria. Acompanhe o progresso em SM-P3.1.x em `roadmap.md`.

#### Instantâneo de propriedade do código
- **Crypto WG:** `iroha_crypto`, acessórios SM, documentação de conformidade.
- **IVM Core:** implementações de syscall, intrínsecos Kotodama, controle de host.
- **Config WG:** קונפיגורציית `crypto.allowed_signing`/`default_hash`, ולידציית מניפסט, חיווט קבלה.
- **Programa SDK:** ferramentas com reconhecimento de SM em CLI/Kagami/SDKs, equipamentos compartilhados.
- **GT de operações e desempenho de plataforma:** ganchos de aceleração, telemetria, capacitação do operador.

## Manual de migração de configuraçãoAs operadoras que mudam de redes somente Ed25519 para implantações habilitadas para SM devem
siga o processo encenado em
[`sm_config_migration.md`](sm_config_migration.md). O guia cobre a construção
validação, camadas `iroha_config` (`defaults` → `user` → `actual`), gênese
regeneração por meio de substituições `kagami` (por exemplo, `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`), validação pré-voo e reversão
planejamento para que os instantâneos de configuração e manifestos permaneçam consistentes em todo o
frota.

## Política Determinística
- Aplicar nonces derivados de RFC6979 para todos os caminhos de assinatura SM2 em SDKs e assinatura de host opcional; os verificadores aceitam apenas codificações canônicas r∥s.
- A comunicação do plano de controle (streaming) permanece Ed25519; SM2 limitado a assinaturas no plano de dados, a menos que a governança aprove a expansão.
- Intrínsecos (ARM SM3/SM4) restritos a operações de verificação/hash determinísticas com detecção de recursos em tempo de execução e fallback de software.

## Norito e plano de codificação
1. Estenda enums de algoritmo em `iroha_data_model` com `Sm2PublicKey`, `Sm2Signature`, `Sm3Digest`, `Sm4Key`.
2. Serialize assinaturas SM2 como arrays `r∥s` de largura fixa big-endian (32+32 bytes) para evitar ambiguidades de DER; conversões tratadas em adaptadores. *(Concluído: implementado em auxiliares `Sm2Signature`; viagens de ida e volta Norito/JSON em vigor.)*
3. Registre identificadores multicodec (`sm3-256`, `sm2-pub`, `sm4-key`) se estiver usando multiformatos, atualize dispositivos e documentos. *(Progresso: código provisório `sm2-pub` `0x1306` agora validado com chaves derivadas; códigos SM3/SM4 pendentes de atribuição final, rastreados via `sm_known_answers.toml`.)*
4. Atualize os testes dourados Norito cobrindo viagens de ida e volta e rejeição de codificações malformadas (r ou s curto/longo, parâmetros de curva inválidos).## Plano de integração de host e VM (SM-2)
1. Implementar o syscall `sm3_hash` do lado do host espelhando o hash shim GOST existente; reutilize `Sm3Digest::hash` e exponha caminhos de erros determinísticos. *(Landed: host retorna Blob TLV; consulte implementação `DefaultHost` e regressão `sm_syscalls.rs`.)*
2. Estenda a tabela syscall da VM com `sm2_verify` que aceita assinaturas r∥s canônicas, valida IDs distintos e mapeia falhas para códigos de retorno determinísticos. *(Concluído: host + Kotodama retorno intrínseco `1/0`; o conjunto de regressão agora cobre assinaturas truncadas, chaves públicas malformadas, TLVs não blob e cargas úteis `distid` UTF-8/vazias/incompatíveis.)*
3. Forneça syscalls `sm4_gcm_seal`/`sm4_gcm_open` (e opcionalmente CCM) com dimensionamento explícito de nonce/tag (RFC 8998). *(Concluído: GCM usa nonces fixos de 12 bytes + tags de 16 bytes; CCM suporta nonces de 7 a 13 bytes com comprimentos de tag {4,6,8,10,12,14,16} controlados via `r14`; Kotodama os expõe como `sm::seal_gcm/open_gcm` e `sm::seal_ccm/open_ccm`.) Documente a política de reutilização de uso único no manual do desenvolvedor.*
4. Conecte contratos de fumaça Kotodama e testes de integração IVM cobrindo casos positivos e negativos (tags alteradas, assinaturas malformadas, algoritmos não suportados). *(Feito via `crates/ivm/tests/kotodama_sm_syscalls.rs` espelhando regressões de host para SM3/SM2/SM4.)*
5. Atualize listas de permissões de syscall, políticas e documentos ABI (`crates/ivm/docs/syscalls.md`) e atualize manifestos com hash após adicionar as novas entradas.

### Status de integração de host e VM
- DefaultHost, CoreHost e WsvHost expõem os syscalls SM3/SM2/SM4 e os bloqueiam em `sm_enabled`, retornando `PermissionDenied` quando o sinalizador de tempo de execução é falso.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- O gate `crypto.allowed_signing` é encadeado por meio de pipeline/executor/estado para que os nós de produção optem por participar de forma determinística por meio da configuração; adicionar `sm2` alterna a disponibilidade do auxiliar SM.`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- A cobertura de regressão exercita caminhos habilitados e desabilitados (DefaultHost/CoreHost/WsvHost) para hashing SM3, verificação SM2 e selo/aberto SM4 GCM/CCM fluxos.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## Tópicos de configuração
- Adicione `crypto.allowed_signing`, `crypto.default_hash`, `crypto.sm2_distid_default` e o `crypto.enable_sm_openssl_preview` opcional a `iroha_config`. Certifique-se de que o encanamento de recursos do modelo de dados espelhe a caixa criptográfica (`iroha_data_model` expõe `sm` → `iroha_crypto/sm`).
- Configuração de conexão com políticas de admissão para que arquivos de manifestos/gênese definam algoritmos permitidos; o plano de controle permanece Ed25519 por padrão.### Trabalho CLI e SDK (SM-3)
1. **CLI Torii** (`crates/iroha_cli`): adicione keygen/importação/exportação SM2 (com reconhecimento de distid), auxiliares de hash SM3 e comandos de criptografia/descriptografia SM4 AEAD. Atualize prompts e documentos interativos.
2. **Ferramentas Genesis** (`xtask`, `scripts/`): permite que os manifestos declarem algoritmos de assinatura permitidos e hashes padrão, falham rapidamente se o SM estiver habilitado sem os botões de configuração correspondentes. *(Concluído: `RawGenesisTransaction` agora carrega um bloco `crypto` com `default_hash`/`allowed_signing`/`sm2_distid_default`; `ManifestCrypto::validate` e `kagami genesis validate` rejeitam configurações de SM inconsistentes e anúncios de manifesto de padrões/genesis o instantâneo.)*
3. **Superfícies SDK**:
   - Rust (`iroha_client`): expõe auxiliares de assinatura/verificação SM2, hashing SM3, wrappers SM4 AEAD com padrões determinísticos.
   - Python/JS/Swift: espelha a API Rust; reutilize equipamentos preparados em `sm_known_answers.toml` para testes entre idiomas.
4. Documente o fluxo de trabalho do operador para ativar o SM em inícios rápidos CLI/SDK e garanta que as configurações JSON/YAML aceitem as novas tags de algoritmo.

#### Progresso da CLI
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` agora emite uma carga JSON que descreve o par de chaves SM2 junto com um snippet `client.toml` (`public_key_config`, `private_key_hex`, `distid`). O comando aceita `--seed-hex` para geração determinística e espelha a derivação RFC 6979 usada pelos hosts.
- `cargo xtask sm-operator-snippet --distid CN12345678901234` agrupa o fluxo de keygen/exportação, gravando as mesmas saídas `sm2-key.json`/`client-sm2.toml` em uma única etapa. Use `--json-out <path|->`/`--snippet-out <path|->` para redirecionar arquivos ou transmiti-los para stdout, removendo a dependência `jq` para automação.
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` deriva os mesmos metadados de material existente para que os operadores possam validar IDs distintivos antes da admissão.
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` imprime o snippet de configuração (incluindo orientação `allowed_signing`/`sm2_distid_default`) e, opcionalmente, reemite o inventário de chave JSON para scripts.
- `iroha_cli tools crypto sm3 hash --data <string>` faz hash de cargas arbitrárias; `--data-hex` / `--file` cobrem entradas binárias e o comando relata resumos hexadecimais e base64 para ferramentas de manifesto.
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (e `gcm-open`) envolve os auxiliares SM4-GCM do host e superfície `ciphertext_hex`/`tag_hex` ou cargas úteis de texto simples. `sm4 ccm-seal` / `sm4 ccm-open` fornecem o mesmo UX para CCM com validação de comprimento de nonce (7–13 bytes) e comprimento de tag (4,6,8,10,12,14,16); ambos os comandos emitem opcionalmente bytes brutos para o disco.## Estratégia de teste
### Testes de Unidade/Respostas Conhecidas
- Vetores GM/T 0004 e GB/T 32905 para SM3 (por exemplo, `"abc"`).
- Vetores GM/T 0002 e RFC 8998 para SM4 (bloco + GCM/CCM).
- Exemplos GM/T 0003/GB/T 32918 para SM2 (valor Z, verificação de assinatura), incluindo Anexo Exemplo 1 com ID `ALICE123@YAHOO.COM`.
- Arquivo temporário de fixação provisória: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- O conjunto de regressão SM2 derivado de Wycheproof (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) agora carrega um corpus de 52 casos que sobrepõe acessórios determinísticos (Anexo D, sementes SDK) com bits-flip, adulteração de mensagens e negativos de assinatura truncada. O JSON higienizado reside em `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` e `sm2_fuzz.rs` o consome diretamente para que os cenários de caminho feliz e de violação permaneçam alinhados nas execuções de fuzz/propriedade. 벡터들은 표준 곡선뿐만 아니라 Anexo 영역도 다루며, 필요 시 내장 `Sm2PublicKey` 검증 이후 BigInt 백업 루틴이 추적을 완료합니다.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (ou `--input-url <https://…>`) corta deterministicamente qualquer queda upstream (tag do gerador opcional) e reescreve `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`. Até que o C2SP publique o corpus oficial, baixe os forks manualmente e alimente-os através do helper; ele normaliza chaves, contagens e sinalizadores para que os revisores possam raciocinar sobre as diferenças.
- Ida e volta SM2/SM3 Norito validadas em `crates/iroha_data_model/tests/sm_norito_roundtrip.rs`.
- Regressão de syscall do host SM3 em `crates/ivm/tests/sm_syscalls.rs` (recurso SM).
- SM2 verifica regressão syscall em `crates/ivm/tests/sm_syscalls.rs` (casos de sucesso + falha).

### Testes de propriedade e regressão
- Proptest para SM2 rejeitando curvas inválidas, r/s não canônicos e reutilização de nonces. *(Disponível em `crates/iroha_crypto/tests/sm2_fuzz.rs`, bloqueado atrás de `sm_proptest`; habilitar via `cargo test -p iroha_crypto --features "sm sm_proptest"`.)*
- Vetores Wycheproof SM4 (modo bloco/AES) adaptados para modos variados; rastreie upstream para adições SM2. `sm3_sm4_vectors.rs` agora exerce inversões de bits de tags, tags truncadas e adulteração de texto cifrado para GCM e CCM.

### Interoperabilidade e desempenho
- RustCrypto ↔ Conjunto de paridade OpenSSL/Tongsuo para sinalização/verificação SM2, resumos SM3 e SM4 ECB/GCM vive em `crates/iroha_crypto/tests/sm_cli_matrix.rs`; invoque-o com `scripts/sm_interop_matrix.sh`. Os vetores de paridade CCM agora são executados em `sm3_sm4_vectors.rs`; O suporte à matriz CLI seguirá assim que as CLIs upstream exporem os auxiliares CCM.
- O auxiliar SM3 NEON agora executa o caminho de compactação/preenchimento Armv8 de ponta a ponta com controle de tempo de execução por meio de `sm_accel::is_sm3_enabled` (recurso + substituições de env espelhadas em SM3/SM4). Golden digests (zero/`"abc"`/bloco longo + comprimentos aleatórios) e testes de desativação forçada mantêm a paridade com o back-end escalar RustCrypto, e o micro-banco Criterion (`crates/sm3_neon/benches/digest.rs`) captura a taxa de transferência escalar vs NEON em hosts AArch64.
- Espelhamento de chicote de desempenho `scripts/gost_bench.sh` para comparar Ed25519/SHA-2 vs SM2/SM3/SM4 e validar limites de tolerância.#### Arm64 Baseline (Apple Silicon local; Critério `sm_perf`, atualizado em 05/12/2025)
- `scripts/sm_perf.sh` agora executa o banco Criterion e impõe medianas contra `crates/iroha_crypto/benches/sm_perf_baseline.json` (gravado no macOS aarch64; tolerância de 25% por padrão, os metadados de linha de base capturam o triplo do host). O novo sinalizador `--mode` permite que os engenheiros capturem pontos de dados escalares vs NEON vs `sm-neon-force` sem editar o script; o pacote de captura atual (JSON bruto + resumo agregado) reside em `artifacts/sm_perf/2026-03-lab/m3pro_native/` e marca cada carga útil com `cpu_label="m3-pro-native"`.
- Os modos de aceleração agora selecionam automaticamente a linha de base escalar como alvo de comparação. `scripts/sm_perf.sh` encadeia `--compare-baseline/--compare-tolerance/--compare-label` a `sm_perf_check`, emitindo deltas por benchmark em relação à referência escalar e falhando quando a desaceleração excede o limite configurado. As tolerâncias por benchmark da linha de base orientam a proteção de comparação (o SM3 é limitado a 12% na linha de base escalar da Apple, enquanto o delta de comparação do SM3 agora permite até 70% em relação à referência escalar para evitar oscilações); as linhas de base do Linux reutilizam o mesmo mapa de comparação porque são exportadas da captura `neoverse-proxy-macos`, e iremos restringi-las após uma execução bare-metal do Neoverse se as medianas diferirem. Passe `--compare-tolerance` explicitamente ao capturar limites mais rígidos (por exemplo, `--compare-tolerance 0.20`) e use `--compare-label` para anotar hosts de referência alternativos.
- As linhas de base registradas na máquina de referência CI agora estão em `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` e `sm_perf_baseline_aarch64_macos_neon_force.json`. Atualize-os com `scripts/sm_perf.sh --mode scalar --write-baseline`, `--mode auto --write-baseline` ou `--mode neon-force --write-baseline` (defina `SM_PERF_CPU_LABEL` antes de capturar) e arquive o JSON gerado junto com os logs de execução. Mantenha a saída auxiliar agregada (`artifacts/.../aggregated.json`) com o PR para que os revisores possam auditar cada amostra. As linhas de base do Linux/Neoverse agora são fornecidas em `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json`, promovidas a partir de `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` (rótulo de CPU `neoverse-proxy-macos`, tolerância de comparação SM3 0,70 para aarch64 macOS/Linux); execute novamente em hosts Neoverse bare-metal quando disponível para aumentar as tolerâncias.
- Os arquivos JSON de linha de base agora podem conter um objeto `tolerances` opcional para reforçar as proteções por benchmark. Exemplo:
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` aplica esses limites fracionários (8% e 12% no exemplo) ao usar a tolerância CLI global para quaisquer benchmarks não listados.
- Os protetores de comparação também podem respeitar `compare_tolerances` na linha de base de comparação. Use isso para permitir um delta mais flexível em relação à referência escalar (por exemplo, `\"sm3_vs_sha256_hash/sm3_hash\": 0.70` na linha de base escalar) enquanto mantém o `tolerances` primário estrito para verificações diretas da linha de base.- As linhas de base do Apple Silicon verificadas agora são fornecidas com proteções de concreto: as operações SM2/SM4 permitem desvio de 12–20% dependendo da variação, enquanto as comparações SM3/ChaCha ficam em 8–12%. A tolerância `sm3` da linha de base escalar agora foi reduzida para 0,12; os arquivos `unknown_linux_gnu` espelham a exportação `neoverse-proxy-macos` com o mesmo mapa de tolerância (comparação SM3 0,70) e notas de metadados indicando que eles são enviados para o portão Linux até que uma nova execução do Neoverse bare-metal esteja disponível.
- Assinatura SM2: 298µs por operação (Ed25519: 32µs) ⇒ ~9,2× mais lenta; verificação: 267µs (Ed25519: 41µs) ⇒ ~6,5× mais lento.
- Hashing SM3 (carga útil de 4KiB): 11,2µs, efetivamente paridade com SHA-256 a 11,3µs (≈356MiB/s vs 353MiB/s).
- Selo/abertura SM4-GCM (carga útil de 1KiB, nonce de 12 bytes): 15,5µs vs ChaCha20-Poly1305 a 1,78µs (≈64MiB/s vs 525MiB/s).
- Artefactos de referência (`target/criterion/sm_perf*`) capturados para reprodutibilidade; as linhas de base do Linux são provenientes de `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (rótulo de CPU `neoverse-proxy-macos`, tolerância de comparação SM3 0,70) e podem ser atualizadas em hosts Neoverse bare-metal (`SM-4c.1`) assim que o tempo de laboratório for aberto para aumentar as tolerâncias.

#### Lista de verificação de captura entre arquiteturas
- Execute `scripts/sm_perf_capture_helper.sh` **na máquina de destino** (estação de trabalho x86_64, servidor Neoverse ARM, etc.). Passe `--cpu-label <host>` para carimbar as capturas e (ao executar no modo matriz) para pré-preencher o plano/comandos gerados para agendamento de laboratório. O auxiliar imprime comandos específicos do modo que:
  1. executar o conjunto de critérios com o conjunto de recursos correto e
  2. escreva medianas em `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`.
- Capture primeiro a linha de base escalar e, em seguida, execute novamente o auxiliar para `auto` (e `neon-force` em plataformas AArch64). Use um `SM_PERF_CPU_LABEL` significativo para que os revisores possam rastrear detalhes do host nos metadados JSON.
- Após cada execução, arquive o diretório `target/criterion/sm_perf*` bruto e inclua-o no PR junto com as linhas de base geradas. Aumente as tolerâncias por referência assim que duas execuções consecutivas se estabilizarem (consulte `sm_perf_baseline_aarch64_macos_*.json` para formatação de referência).
- Registre as medianas + tolerâncias nesta seção e atualize `status.md`/`roadmap.md` quando uma nova arquitetura for abordada. As linhas de base do Linux agora são verificadas a partir da captura `neoverse-proxy-macos` (os metadados indicam a exportação para o portão aarch64-unknown-linux-gnu); execute novamente em hosts Neoverse/x86_64 bare-metal como acompanhamento quando esses slots de laboratório estiverem disponíveis.

#### ARMv8 SM3/SM4 intrínsecos vs caminhos escalares
`sm_accel` (consulte `crates/iroha_crypto/src/sm.rs:739`) fornece a camada de despacho de tempo de execução para auxiliares SM3/SM4 apoiados por NEON. O recurso é protegido em três níveis:| Camada | Controle | Notas |
|-------|---------|-------|
| Tempo de compilação | `--features sm` (agora extrai `sm-neon` automaticamente em `aarch64`) ou `sm-neon-force` (testes/benchmarks) | Constrói os módulos NEON e vincula `sm3-neon`/`sm4-neon`. |
| Detecção automática em tempo de execução | `sm4_neon::is_supported()` | Verdadeiro apenas em CPUs que expõem equivalentes AES/PMULL (por exemplo, Apple série M, Neoverse V1/N2). VMs que mascaram NEON ou FEAT_SM4 recorrem ao código escalar. |
| Substituição do operador | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | Despacho orientado por configuração aplicado na inicialização; use `force-enable` apenas para criação de perfil em ambientes confiáveis ​​e prefira `force-disable` ao validar substitutos escalares. |

**Envelope de desempenho (Apple M3 Pro; medianas registradas em `sm_perf_baseline_aarch64_macos_{mode}.json`):**

| Modo | Resumo SM3 (4KiB) | Selo SM4-GCM (1KiB) | Notas |
|------|-------------------|-----------|-------|
| Escalar | 11,6µs | 15,9µs | Caminho RustCrypto determinístico; usado em todos os lugares onde o recurso `sm` é compilado, mas NEON não está disponível. |
| NÉON automático | ~2,7× mais rápido que escalar | ~2,3× mais rápido que escalar | Os kernels NEON atuais (SM-5a.2c) ampliam a programação em quatro palavras por vez e usam distribuição de fila dupla; as medianas exatas variam por host, portanto consulte os metadados JSON de linha de base. |
| Força NÉON | Espelha NEON auto, mas desativa totalmente o substituto | O mesmo que NEON automático | Exercido via `scripts/sm_perf.sh --mode neon-force`; mantém o CI honesto mesmo em hosts que usariam o modo escalar como padrão. |

**Determinismo e orientação de implantação**
- Os intrínsecos nunca alteram os resultados observáveis — `sm_accel` retorna `None` quando o caminho acelerado está indisponível, então o auxiliar escalar é executado. Os caminhos do código de consenso, portanto, permanecem determinísticos desde que a implementação escalar esteja correta.
- **Não** bloqueie a lógica de negócios sobre se o caminho NEON foi usado. Trate a aceleração apenas como uma dica de desempenho e exponha o status apenas por telemetria (por exemplo, medidor `sm_intrinsics_enabled`).
- Sempre execute `ci/check_sm_perf.sh` (ou `make check-sm-perf`) depois de tocar no código SM para que o chicote do Criterion valide os caminhos escalares e acelerados usando as tolerâncias incorporadas em cada JSON de linha de base.
- Ao fazer benchmarking ou depuração, prefira o botão de configuração `crypto.sm_intrinsics` em vez de sinalizadores de tempo de compilação; recompilar com `sm-neon-force` desativa totalmente o fallback escalar, enquanto `force-enable` simplesmente estimula a detecção de tempo de execução.
- Documente a política escolhida nas notas de lançamento: as compilações de produção devem deixar a política em `Auto`, permitindo que cada validador descubra recursos de hardware de forma independente, enquanto ainda compartilha os mesmos artefatos binários.
- Evite enviar binários que misturem intrínsecos de fornecedores vinculados estaticamente (por exemplo, bibliotecas SM4 de terceiros), a menos que respeitem o mesmo fluxo de envio e teste - caso contrário, as regressões de desempenho não serão capturadas por nossas ferramentas de linha de base.#### x86_64 Linha de base Rosetta (Apple M3 Pro; capturado em 01/12/2025)
- As linhas de base residem em `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`), com capturas brutas + agregadas em `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/`.
- As tolerâncias por benchmark em x86_64 são definidas como 20% para SM2, 15% para Ed25519/SHA-256 e 12% para SM4/ChaCha. `scripts/sm_perf.sh` agora padroniza a tolerância de comparação de aceleração para 25% em hosts não AArch64, de modo que escalar-vs-auto permanece firme enquanto deixa a folga de 5,25 em AArch64 para a linha de base `m3-pro-native` compartilhada até que uma nova execução do Neoverse chegue.

| Referência | Escalar | Automóvel | Força Neon | Automático vs Escalar | Néon vs Escalar | Néon vs Automóvel |
|-----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57,43 |  57.12 |      55,77 |          -0,53% |         -2,88% |        -2,36% |
| sm2_vs_ed25519_sign/sm2_sign |   572,76 | 568,71 |     557,83 |          -0,71% |         -2,61% |        -1,91% |
| sm2_vs_ed25519_verificar/verificar/ed25519 |    69,03 |  68,42 |      66,28 |          -0,88% |         -3,97% |        -3,12% |
| sm2_vs_ed25519_verificar/verificar/sm2 |   521,73 | 514,50 |     502.17 |          -1,38% |         -3,75% |        -2,40% |
| sm3_vs_sha256_hash/sha256_hash |    16,78 |  16,58 |      16.16 |          -1,19% |         -3,69% |        -2,52% |
| sm3_vs_sha256_hash/sm3_hash |    15,78 |  15,51 |      15.04 |          -1,71% |         -4,69% |        -3,03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1,96 |   1,97 |       1,97 |           0,39% |          0,16% |        -0,23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16,38 |      16.26 |           0,72% |         -0,01% |        -0,72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1,96 |   2h00 |       1,93 |           2,23% |         -1,14% |        -3,30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16,60 |  16,58 |      16h15 |          -0,10% |         -2,66% |        -2,57% |

#### x86_64 / outros alvos não-aarch64
- As compilações atuais ainda enviam apenas o caminho escalar RustCrypto determinístico em x86_64; mantenha `sm` ativado, mas **não** injete kernels AVX2/VAES externos até que SM-4c.1b chegue. A política de tempo de execução espelha o ARM: o padrão é `Auto`, respeita `crypto.sm_intrinsics` e exibe os mesmos medidores de telemetria.
- As capturas Linux/x86_64 ainda precisam ser gravadas; reutilize o auxiliar nesse hardware e coloque as medianas em `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` junto com as linhas de base do Rosetta e o mapa de tolerância acima.**Armadilhas comuns**
1. **Instâncias ARM virtualizadas:** Muitas nuvens expõem NEON, mas ocultam as extensões SM4/AES que `sm4_neon::is_supported()` verifica. Espere o caminho escalar nesses ambientes e capture as linhas de base de desempenho de acordo.
2. **Substituições parciais:** A mistura de valores `crypto.sm_intrinsics` persistentes entre execuções leva a leituras de desempenho inconsistentes. Documente a substituição pretendida no tíquete do experimento e redefina a configuração antes de capturar novas linhas de base.
3. **Paridade CI:** alguns executores do macOS não permitem amostragem de desempenho baseada em contador enquanto o NEON está ativo. Mantenha as saídas `scripts/sm_perf_capture_helper.sh` anexadas aos PRs para que os revisores possam confirmar que o caminho acelerado foi exercido mesmo que o executor oculte esses contadores.
4. **Variantes ISA futuras (SVE/SVE2):** Os kernels atuais assumem formatos de pista NEON. Antes de migrar para SVE/SVE2, estenda `sm_accel::NeonPolicy` com uma variante dedicada para que possamos manter CI, telemetria e botões do operador alinhados.

Os itens de ação rastreados em SM-5a/SM-4c.1 garantem que o CI capture provas de paridade para cada nova arquitetura, e o roteiro permaneça em 🈺 até que as linhas de base Neoverse/x86 e as tolerâncias NEON vs escalar convirjam.

## Conformidade e Notas Regulatórias

### Normas e Referências Normativas
- **GM/T 0002-2012** (SM4), **GM/T 0003-2012** + **GB/T 32918 series** (SM2), **GM/T 0004-2012** + **GB/T 32905/32907** (SM3) e **RFC 8998** regem as definições de algoritmo, vetores de teste e Ligações KDF que nossos fixtures consomem.【docs/source/crypto/sm_vectors.md#L79】
- O resumo de conformidade em `docs/source/crypto/sm_compliance_brief.md` interliga esses padrões com as responsabilidades de arquivamento/exportação para equipes de engenharia, SRE e jurídicas; mantenha esse resumo atualizado sempre que o catálogo da GM/T for revisado.

### Fluxo de trabalho regulatório da China continental
1. **Arquivamento do produto (开发备案):** Antes de enviar binários habilitados para SM da China continental, envie o manifesto do artefato, as etapas determinísticas de construção e a lista de dependências à administração provincial de criptografia. Os modelos de arquivamento e a lista de verificação de conformidade ficam em `docs/source/crypto/sm_compliance_brief.md` e no diretório de anexos (`sm_product_filing_template.md`, `sm_sales_usage_filing_template.md`, `sm_export_statement_template.md`).
2. **Arquivo de vendas/uso (销售/使用备案):** Os operadores que executam nós habilitados para SM em terra devem registrar seu escopo de implantação, postura de gerenciamento de chaves e plano de telemetria. Anexe manifestos assinados e instantâneos de métricas `iroha_sm_*` ao arquivar.
3. **Testes credenciados:** Operadores de infraestrutura crítica podem exigir relatórios de laboratório certificados. Forneça scripts de construção reproduzíveis, exportações SBOM e artefatos Wycheproof/interop (veja abaixo) para que os auditores downstream possam reproduzir os vetores sem alterar o código.
4. **Acompanhamento de status:** Registrar os arquivamentos concluídos no ticket de liberação e `status.md`; registros ausentes bloqueiam a promoção de apenas verificação para pilotos de assinatura.### Postura de Exportação e Distribuição
- Trate os binários compatíveis com SM como itens controlados de acordo com **US EAR Categoria 5 Parte 2** e **Regulamento UE 2021/821 Anexo 1 (5D002)**. A publicação da fonte continua a qualificar-se para as exclusões de código aberto/ENC, mas a redistribuição para destinos embargados ainda requer revisão legal.
- Os manifestos de lançamento devem agrupar uma declaração de exportação referenciando a base ENC/TSU e listar os identificadores de construção OpenSSL/Tongsuo se a visualização FFI estiver empacotada.
- Prefira embalagens regionais (por exemplo, espelhos do continente) quando os operadores precisarem de distribuição onshore para evitar problemas de transferência transfronteiriça.

### Documentação e evidências do operador
- Combine este resumo de arquitetura com a lista de verificação de implementação em `docs/source/crypto/sm_operator_rollout.md` e o guia de arquivamento de conformidade em `docs/source/crypto/sm_compliance_brief.md`.
- Mantenha o início rápido do genesis/operador sincronizado em `docs/genesis.md`, `docs/genesis.he.md` e `docs/genesis.ja.md`; No fluxo de trabalho SM2/SM3 CLI, existe a fonte de verdade voltada para o operador para semear manifestos `crypto`.
- Arquive registros de origem OpenSSL/Tongsuo, saída `scripts/sm_openssl_smoke.sh` e paridade `scripts/sm_interop_matrix.sh` com cada pacote de lançamento para que os parceiros de conformidade e auditoria tenham artefatos determinísticos.
- Atualize `status.md` sempre que o escopo de conformidade mudar (novas jurisdições, conclusões de arquivamento ou decisões de exportação) para manter o estado do programa detectável.
- Siga as revisões de preparação preparadas (`SM-RR1` – `SM-RR3`) capturadas em `docs/source/release_dual_track_runbook.md`; a promoção entre as fases somente de verificação, piloto e assinatura do GA requer os artefatos ali enumerados.

## Receitas de interoperabilidade

### RustCrypto ↔ Matriz OpenSSL/Tongsuo
1. Certifique-se de que as CLIs OpenSSL/Tongsuo estejam disponíveis (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` permite a seleção explícita de ferramentas).
2. Execute `scripts/sm_interop_matrix.sh`; ele invoca `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` e exerce sinalização/verificação SM2, resumos SM3 e fluxos SM4 ECB/GCM em cada provedor, ignorando qualquer CLI que esteja ausente.【scripts/sm_interop_matrix.sh#L1】
3. Arquive os arquivos `target/debug/deps/sm_cli_matrix*.log` resultantes com os artefatos de lançamento.

### Fumaça de visualização do OpenSSL (Packaging Gate)
1. Instale os cabeçalhos de desenvolvimento OpenSSL ≥3.0 e certifique-se de que `pkg-config` possa localizá-los.
2. Execute `scripts/sm_openssl_smoke.sh`; o auxiliar executa `cargo check`/`cargo test --test sm_openssl_smoke`, exercitando hashing SM3, verificação SM2 e viagens de ida e volta SM4-GCM por meio do back-end FFI (o equipamento de teste permite a visualização explicitamente).【scripts/sm_openssl_smoke.sh#L1】
3. Trate qualquer falha não ignorada como um bloqueador de liberação; capture a saída do console para evidência de auditoria.

### Atualização Determinística de Fixtures
- Gere novamente os equipamentos SM (`sm_vectors.md`, `fixtures/sm/…`) antes de cada registro de conformidade e, em seguida, execute novamente a matriz de paridade e o chicote de fumaça para que os auditores recebam novas transcrições determinísticas junto com os registros.## Preparação para Auditoria Externa
- `docs/source/crypto/sm_audit_brief.md` empacota o contexto, o escopo, o cronograma e os contatos para a revisão externa.
- Os artefatos de auditoria estão sob `docs/source/crypto/attachments/` (registro de fumaça OpenSSL, snapshot da árvore de carga, exportação de metadados de carga, proveniência do kit de ferramentas) e `fuzz/sm_corpus_manifest.json` (sementes de fuzz SM determinísticas provenientes de vetores de regressão existentes). No macOS, o log de fumaça registra atualmente uma execução ignorada porque o ciclo de dependência do espaço de trabalho impede `cargo check`; As compilações do Linux sem o ciclo exercitarão totalmente o back-end de visualização.
- Circulado para Crypto WG, Platform Ops, Security e Docs/DevRel leads em 30/01/2026 para alinhamento antes do envio da RFQ.

### Status do compromisso de auditoria

- **Trail of Bits (prática de criptografia CN)** — Declaração de trabalho executada em **2026-02-21**, início **2026-02-24**, janela de trabalho de campo **2026-02-24–2026-03-22**, relatório final previsto para **2026-04-15**. Ponto de verificação de status semanal todas as quartas-feiras às 09:00 UTC com o líder do Crypto WG e contato de engenharia de segurança. Consulte [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) para contatos, resultados finais e anexos de evidências.
- **NCC Group APAC (slot de contingência)** — Reservada a janela de maio de 2026 como uma revisão de acompanhamento/paralela caso descobertas adicionais ou solicitações de reguladores exijam uma segunda opinião. Detalhes de envolvimento e ganchos de escalonamento são registrados junto com a entrada Trilha de Bits em `sm_audit_brief.md`.

## Riscos e Mitigações

Registro completo: consulte [`sm_risk_register.md`](sm_risk_register.md) para detalhes
pontuação de probabilidade/impacto, gatilhos de monitoramento e histórico de aprovação. O
O resumo abaixo rastreia os itens principais apresentados para a engenharia de lançamento.
| Risco | Gravidade | Proprietário | Mitigação |
|------|----------|-------|-----------|
| Falta de auditoria externa para caixas RustCrypto SM | Alto | GT de criptografia | Trilha de contrato do Grupo Bits/NCC, manter somente verificação até que o relatório de auditoria seja aceito. |
| Regressões nonce determinísticas entre SDKs | Alto | Líderes do programa SDK | Compartilhe equipamentos no SDK CI; impor codificação canônica r∥s; adicionar testes de integração entre SDKs (rastreados no SM-3c). |
| Bugs específicos do ISA em intrínsecos | Médio | GT Desempenho | Intrínsecos do recurso, exigem cobertura de CI no ARM, mantêm substituto de software. Matriz de validação de hardware mantida em `sm_perf.md`. |
| Ambiguidade de conformidade atrasa adoção | Médio | Documentos e contato jurídico | Publicar resumo de conformidade e lista de verificação do operador (SM-6a/SM-6b) antes da GA; reunir informações jurídicas. Lista de verificação de arquivamento enviada em `sm_compliance_brief.md`. |
| Desvio de back-end FFI com atualizações do provedor | Médio | Operações de plataforma | Fixe versões do provedor, adicione testes de paridade, mantenha a aceitação do backend FFI até que o pacote se estabilize (SM-P3). |## Perguntas abertas/acompanhamentos
1. Selecione parceiros de auditoria independentes com experiência em algoritmos SM em Rust.
   - **Resposta (24/02/2026):** A prática de criptografia CN da Trail of Bits assinou o SOW de auditoria primária (início em 24/02/2026, entrega em 15/04/2026) e o NCC Group APAC mantém um período de contingência em maio para que os reguladores possam solicitar uma segunda revisão sem reabrir as compras. O escopo de envolvimento, os contatos e as listas de verificação residem em [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) e são espelhados em `sm_audit_vendor_landscape.md`.
2. Continue rastreando o upstream para um conjunto de dados oficial do Wycheproof SM2; o espaço de trabalho atualmente envia um conjunto de 52 casos com curadoria (aparelhos determinísticos + casos de violação sintetizados) e o alimenta em `sm2_wycheproof.rs`/`sm2_fuzz.rs`. Atualize o corpus via `cargo xtask sm-wycheproof-sync` assim que o JSON upstream chegar.
   - Rastrear suítes de vetores negativos Bouncy Castle e GmSSL; importe para `sm2_fuzz.rs` assim que o licenciamento for liberado para complementar o corpus existente.
3. Definir telemetria de linha de base (métricas, registro) para monitoramento de adoção de SM.
4. Decida se o padrão SM4 AEAD é GCM ou CCM para exposição Kotodama/VM.
5. Rastreie a paridade RustCrypto/OpenSSL para o Anexo Exemplo 1 (ID `ALICE123@YAHOO.COM`): confirme o suporte da biblioteca para a chave pública publicada e `(r, s)` para que os fixtures possam ser promovidos para testes de regressão.

## Itens de ação
- [x] Finalizar auditoria de dependência e captura no rastreador de segurança.
- [x] Confirme o envolvimento do parceiro de auditoria para as caixas RustCrypto SM (acompanhamento SM-P0). Trail of Bits (prática de criptografia CN) possui a revisão primária com datas de início/entrega registradas em `sm_audit_brief.md`, e o NCC Group APAC manteve um slot de contingência de maio de 2026 para satisfazer os acompanhamentos do regulador ou de governança.
- [x] Estender a cobertura Wycheproof para caixas de violação SM4 CCM (SM-4a).
- [x] Fixe dispositivos de assinatura SM2 canônicos em SDKs e conecte-os ao CI (SM-3c/SM-1b.1); protegido por `scripts/check_sm2_sdk_fixtures.py` (ver `ci/check_sm2_sdk_fixtures.sh`).

## Apêndice de conformidade (criptografia comercial estadual)

- **Classificação:** SM2/SM3/SM4 são enviados sob o regime de *criptografia comercial estatal* da China (Lei de Criptografia da RPC, Art.3). O envio desses algoritmos no software Iroha **não** coloca o projeto nas camadas principal/comum (secreto de estado), mas os operadores que os utilizam em implantações na RPC devem seguir o arquivamento de criptografia comercial e as obrigações de MLPS.【docs/source/crypto/sm_chinese_crypto_law_brief.md:14】
- **Linhagem de padrões:** Alinhe a documentação pública com as conversões GB/T oficiais das especificações GM/T:

| Algoritmo | Referência GB/T | Origem GM/T | Notas |
|-----------|----------------|-------------|-------|
| SM2 | GB/T32918 (todas as peças) | GM/T0003 | Assinatura digital ECC + troca de chaves; Iroha expõe verificação em nós principais e assinatura determinística para SDKs. |
| SM3 | GB/T32905 | GM/T0004 | Hash de 256 bits; hashing determinístico em caminhos escalares e acelerados ARMv8. |
| SM4 | GB/T32907 | GM/T0002 | Cifra de bloco de 128 bits; Iroha fornece auxiliares GCM/CCM e garante paridade big-endian entre implementações. |- **Manifesto de capacidade:** O endpoint Torii `/v2/node/capabilities` anuncia o seguinte formato JSON para que operadores e ferramentas possam consumir o manifesto SM programaticamente:

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

O subcomando CLI `iroha runtime capabilities` apresenta a mesma carga localmente, imprimindo um resumo de uma linha junto com o anúncio JSON para coleta de evidências de conformidade.

- **Produtos de documentação:** publicar notas de lançamento e SBOMs que identificam os algoritmos/padrões acima e manter o resumo completo de conformidade (`sm_chinese_crypto_law_brief.md`) junto com artefatos de lançamento para que os operadores possam anexá-lo aos registros provinciais.【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **Transferência do operador:** lembre aos implantadores que o MLPS2.0/GB/T39786-2021 exige avaliações de aplicativos criptográficos, SOPs de gerenciamento de chaves SM e retenção de evidências ≥6 anos; indique-lhes a lista de verificação do operador no resumo de conformidade.【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## Plano de Comunicação
- **Público:** Membros principais do Crypto WG, Engenharia de Liberação, Conselho de Revisão de Segurança, líderes do programa SDK.
- **Artefatos:** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, trecho do roteiro (SM-0 .. SM-7a).
- **Canal:** Agenda semanal de sincronização do Crypto WG + e-mail de acompanhamento resumindo os itens de ação e solicitando aprovação para atualização de bloqueio e entrada de dependências (versão preliminar circulada em 19/01/2025).
- **Proprietário:** Líder do Crypto WG (delegado aceitável).