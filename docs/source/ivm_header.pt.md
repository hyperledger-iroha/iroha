---
lang: pt
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T10:30:30.084677+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Cabeçalho de bytecode IVM


Magia
- 4 bytes: ASCII `IVM\0` no deslocamento 0.

Layout (atual)
- Deslocamentos e tamanhos (17 bytes no total):
  - 0..4: magia `IVM\0`
  -4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (bits de recurso; veja abaixo)
  -7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (little endian)
  -16: `abi_version: u8`

Bits de modo
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (reservado/com recursos limitados).

Campos (significado)
- `abi_version`: tabela syscall e versão do esquema pointer-ABI.
- `mode`: bits de recurso para rastreamento ZK/VECTOR/HTM.
- `vector_length`: comprimento lógico do vetor para operações vetoriais (0 → não definido).
- `max_cycles`: limite de preenchimento de execução utilizado no modo ZK e admissão.

Notas
- Endianness e layout são definidos pela implementação e vinculados a `version`. O layout on-wire acima reflete a implementação atual em `crates/ivm_abi/src/metadata.rs`.
- Um leitor mínimo pode confiar neste layout para artefatos atuais e deve lidar com alterações futuras por meio do gate `version`.
- A aceleração de hardware (SIMD/Metal/CUDA) é opcional por host. O tempo de execução lê valores `AccelerationConfig` de `iroha_config`: `enable_simd` força substitutos escalares quando falsos, enquanto `enable_metal` e `enable_cuda` bloqueiam seus respectivos back-ends mesmo quando compilados. Criação de VM.
- SDKs móveis (Android/Swift) apresentam os mesmos botões; `IrohaSwift.AccelerationSettings`
  chama `connect_norito_set_acceleration_config` para que as compilações do macOS/iOS possam optar pelo Metal /
  NEON, mantendo alternativas determinísticas.
- Os operadores também podem forçar a desativação de back-ends específicos para diagnóstico exportando `IVM_DISABLE_METAL=1` ou `IVM_DISABLE_CUDA=1`. Essas substituições de ambiente têm precedência sobre a configuração e mantêm a VM no caminho determinístico da CPU.

Ajudantes de estado duráveis e superfície ABI
- As syscalls auxiliares de estado duráveis (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* e codificação/decodificação JSON/SCHEMA) fazem parte da ABI V1 e estão incluídas na computação `abi_hash`.
- CoreHost conecta STATE_{GET,SET,DEL} ao estado de contrato inteligente durável apoiado por WSV; hosts dev/test podem usar sobreposições ou persistência local, mas devem preservar o mesmo comportamento observável.

Validação
- A admissão de nós aceita apenas os cabeçalhos `version_major = 1` e `version_minor = 0`.
- `mode` deve conter apenas bits conhecidos: `ZK`, `VECTOR`, `HTM` (bits desconhecidos são rejeitados).
- `vector_length` é consultivo e pode ser diferente de zero mesmo se o bit `VECTOR` não estiver definido; a admissão impõe apenas um limite superior.
- Valores `abi_version` suportados: a primeira versão aceita apenas `1` (V1); outros valores são rejeitados na admissão.

### Política (gerada)
O seguinte resumo de política é gerado a partir da implementação e não deve ser editado manualmente.<!-- BEGIN GENERATED HEADER POLICY -->
| Campo | Política |
|---|---|
| versão_major | 1 |
| versão_menor | 0 |
| modo (bits conhecidos) | 0x07 (ZK=0x01, VETOR=0x02, HTM=0x04) |
| abi_versão | 1 |
| comprimento_vetor | 0 ou 1..=64 (aconselhamento; independente do bit VECTOR) |
<!-- END GENERATED HEADER POLICY -->

### Hashes ABI (gerados)
A tabela a seguir é gerada a partir da implementação e lista valores canônicos `abi_hash` para políticas suportadas.

<!-- BEGIN GENERATED ABI HASHES -->
| Política | abi_hash (hexadecimal) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Pequenas atualizações podem adicionar instruções atrás de `feature_bits` e espaço reservado de opcode; atualizações principais podem alterar codificações ou remover/reaproveitar apenas junto com uma atualização de protocolo.
- As faixas do Syscall são estáveis; desconhecido para o `abi_version` ativo produz `E_SCALL_UNKNOWN`.
- Os horários de gás estão vinculados ao `version` e exigem vetores dourados na mudança.

Inspecionando artefatos
- Use `ivm_tool inspect <file.to>` para uma visualização estável dos campos de cabeçalho.
- Para desenvolvimento, exemplos/incluem um pequeno alvo Makefile `examples-inspect` que executa inspeção em artefatos construídos.

Exemplo (Rust): magia mínima + verificação de tamanho

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Observação: o layout exato do cabeçalho além da mágica é versionado e definido pela implementação; prefira `ivm_tool inspect` para nomes e valores de campos estáveis.