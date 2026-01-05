---
lang: pt
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8990e19977e7f3fb370b9c8f66064542135fa171
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# Atualizacoes de runtime (IVM + Host) - Sem downtime, sem hardfork

Este documento especifica um mecanismo determinista, controlado por governanca, para introduzir
novas capacidades de IVM/host (por exemplo, novos syscalls e tipos pointer-ABI) sem parar a rede
ou fazer hardfork. Os nos implantam binarios com antecedencia; a ativacao e coordenada on-chain
em uma janela de altura limitada. Contratos antigos continuam a rodar sem mudanca; novas
capacidades sao controladas por versao ABI e politica.

Nota (primeira versao): Apenas ABI v1 e suportado. Manifests de runtime upgrade para outras versoes de ABI sao rejeitados ate que uma versao futura introduza uma nova ABI.

Objetivos
- Ativacao determinista em uma janela de altura agendada com aplicacao idempotente.
- Coexistencia de multiplas versoes ABI; nunca quebrar binarios existentes.
- Guardas de admissao e execucao para que payloads pre-ativacao nao habilitem novo comportamento.
- Rollout amigavel para operadores com visibilidade de capacidades e modos de falha claros.

Nao objetivos
- Alterar numeros de syscalls existentes ou IDs de tipos de ponteiro (proibido).
- Fazer patch de nos ao vivo sem implantar binarios atualizados.

Definicoes
- Versao ABI: inteiro pequeno declarado em `ProgramMetadata.abi_version` que seleciona um `SyscallPolicy` e uma allowlist de tipos de ponteiro.
- Hash ABI: digest determinista da superficie ABI para uma versao dada: lista de syscalls (numeros+formatos), IDs/allowlist de tipos de ponteiro e flags de politica; calculado por `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: mapeamento do host que decide se um numero de syscall e permitido para uma versao ABI e politica do host.
- Activation Window: intervalo semiaberto de altura de bloco `[start, end)` no qual a ativacao e valida exatamente uma vez em `start`.

Objetos de estado (Modelo de dados)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 dos bytes Norito canonicos de um manifest.
- Campos de `RuntimeUpgradeManifest`:
  - `name: String` - rotulo legivel.
  - `description: String` - descricao curta para operadores.
  - `abi_version: u16` - versao ABI alvo a ativar.
  - `abi_hash: [u8; 32]` - hash ABI canonico para a politica alvo.
  - `added_syscalls: Vec<u16>` - numeros de syscalls que se tornam validos com esta versao.
  - `added_pointer_types: Vec<u16>` - identificadores de tipos de ponteiro adicionados pela atualizacao.
  - `start_height: u64` - primeira altura de bloco em que a ativacao e permitida.
  - `end_height: u64` - limite superior exclusivo da janela de ativacao.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` - digests SBOM para artefatos de atualizacao.
  - `slsa_attestation: Vec<u8>` - bytes brutos de atestacao SLSA (base64 em JSON).
  - `provenance: Vec<ManifestProvenance>` - assinaturas sobre o payload canonico.
- Campos de `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` - payload canonico da proposta.
  - `status: RuntimeUpgradeStatus` - estado do ciclo de vida da proposta.
  - `proposer: AccountId` - autoridade que submeteu a proposta.
  - `created_height: u64` - altura de bloco em que a proposta entrou no ledger.
- Campos de `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` - identificador do algoritmo de digest.
  - `digest: Vec<u8>` - bytes brutos do digest (base64 em JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - Invariantes: `end_height > start_height`; `abi_version` e estritamente maior que qualquer versao ativa; `abi_hash` deve ser igual a `ivm::syscalls::compute_abi_hash(policy_for(abi_version))`; `added_*` deve listar exatamente o delta aditivo entre a nova politica ABI e a anterior; numeros/IDs existentes NAO devem ser removidos ou renumerados.

Layout de armazenamento
- `world.runtime_upgrades`: mapa MVCC chaveado por `RuntimeUpgradeId.0` (hash cru de 32 bytes) com valores codificados como payloads Norito canonicos `RuntimeUpgradeRecord`. As entradas persistem entre blocos; commits sao idempotentes e seguros contra replay.

Instrucoes (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Efeitos: insere `RuntimeUpgradeRecord { status: Proposed }` com chave `RuntimeUpgradeId` se nao existir.
  - Rejeita se a janela se sobrepoe a outro registro Proposed/Activated ou se as invariantes falham.
  - Idempotente: reenviar os mesmos bytes canonicos do manifest nao faz nada.
  - Codificacao canonica: bytes do manifest devem corresponder a `RuntimeUpgradeManifest::canonical_bytes()`; codificacoes nao canonicas sao rejeitadas.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Precondicoes: existe um registro Proposed correspondente; `current_height` deve ser igual a `manifest.start_height`; `current_height < manifest.end_height`.
  - Efeitos: muda o registro para `ActivatedAt(current_height)`; adiciona `abi_version` ao conjunto ABI ativo.
  - Idempotente: replays na mesma altura sao no-ops; outras alturas sao rejeitadas de forma determinista.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Precondicoes: status e Proposed e `current_height < manifest.start_height`.
  - Efeitos: muda para `Canceled`.

Eventos (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Regras de admissao
- Admissao de contratos: na primeira versao, apenas `ProgramMetadata.abi_version = 1` e aceito; outros valores sao rejeitados com `IvmAdmissionError::UnsupportedAbiVersion`.
  - Para ABI v1, recompute `abi_hash(1)` e exija igualdade com payload/manifest quando fornecido; caso contrario, rejeitar com `IvmAdmissionError::ManifestAbiHashMismatch`.
- Admissao de transacoes: as instrucoes `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` exigem permissoes apropriadas (root/sudo); devem satisfazer as restricoes de sobreposicao de janela.

Aplicacao de provenance
- Manifests de runtime upgrade podem carregar digests SBOM (`sbom_digests`), bytes de atestacao SLSA (`slsa_attestation`) e metadados de signatarios (assinaturas `provenance`). As assinaturas cobrem o `RuntimeUpgradeManifestSignaturePayload` canonico (todos os campos do manifest exceto a lista de assinaturas `provenance`).
- A configuracao de governanca controla a aplicacao em `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (aceita provenance ausente, verifica se presente) ou `required` (rejeita se provenance estiver ausente).
  - `require_sbom`: quando `true`, pelo menos um digest SBOM e requerido.
  - `require_slsa`: quando `true`, uma atestacao SLSA nao vazia e requerida.
  - `trusted_signers`: lista de chaves publicas de signatarios aprovados.
  - `signature_threshold`: numero minimo de assinaturas confiaveis exigido.
- Rejeicoes de provenance expoem codigos de erro estaveis em falhas de instrucao (prefixo `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetria: `runtime_upgrade_provenance_rejections_total{reason}` conta as razoes de rejeicao de provenance.

Regras de execucao
- Politica de host VM: durante a execucao do programa, derive `SyscallPolicy` de `ProgramMetadata.abi_version`. Syscalls desconhecidos para essa versao mapeiam para `VMError::UnknownSyscall`.
- Pointer-ABI: allowlist derivada de `ProgramMetadata.abi_version`; tipos fora da allowlist para essa versao sao rejeitados durante decode/validacao.
- Troca de host: cada bloco recomputa o conjunto ABI ativo; uma vez que uma transacao de ativacao e confirmada, transacoes subsequentes no mesmo bloco observam a nova politica (validado por `runtime_upgrade_admission::activation_allows_new_abi_in_same_block`).
  - Binding de politica de syscalls: `CoreHost` le a versao ABI declarada pela transacao e aplica `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` contra o `SyscallPolicy` por bloco. O host reutiliza a instancia VM no escopo da transacao, portanto ativacoes no meio do bloco sao seguras - transacoes posteriores observam a politica atualizada enquanto as anteriores continuam com sua versao original.

Invariantes de determinismo e seguranca
- A ativacao ocorre apenas em `start_height` e e idempotente; reorgs abaixo de `start_height` reaplicam deterministamente quando o bloco volta.
- Versoes ABI existentes permanecem ativas indefinidamente; novas versoes apenas estendem o conjunto ativo.
- Nenhuma negociacao dinamica influencia o consenso ou a ordem de execucao; o gossip de capacidades e apenas informativo.

Rollout do operador (sem downtime)
1) Implantar um binario de no que suporte a nova versao ABI (`v+1`) mas nao a ative.
2) Observar a capacidade da frota via telemetria (percentual de nos anunciando suporte a `v+1`).
3) Submeter `ProposeRuntimeUpgrade` com uma janela suficientemente a frente (por exemplo, `H+N`).
4) Em `start_height`, `ActivateRuntimeUpgrade` executa automaticamente como parte do bloco incluido e muda o conjunto ativo do host; nos que nao foram atualizados continuam funcionando para contratos antigos, mas rejeitam admissao/execucao de programas `v+1`.
5) Apos a ativacao, recompilar/implantar contratos visando `v+1`.

Torii e CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (implementado)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implementado)
  - `GET /v1/runtime/upgrades` -> lista de registros (implementado).
  - `POST /v1/runtime/upgrades/propose` -> encapsula `ProposeRuntimeUpgrade` (retorna esqueleto de instrucao; implementado).
  - `POST /v1/runtime/upgrades/activate/:id` -> encapsula `ActivateRuntimeUpgrade` (retorna esqueleto de instrucao; implementado).
  - `POST /v1/runtime/upgrades/cancel/:id` -> encapsula `CancelRuntimeUpgrade` (retorna esqueleto de instrucao; implementado).
- CLI
  - `iroha runtime abi active` (implementado)
  - `iroha runtime abi hash` (implementado)
  - `iroha runtime upgrade list` (implementado)
  - `iroha runtime upgrade propose --file <manifest.json>` (implementado)
  - `iroha runtime upgrade activate --id <id>` (implementado)
  - `iroha runtime upgrade cancel --id <id>` (implementado)

API de consulta core
- Consulta Norito singular (assinada):
  - `FindActiveAbiVersions` retorna uma struct Norito `{ active_versions: [u16], default_compile_target: u16 }`.
  - Ver exemplo: `docs/source/samples/find_active_abi_versions.md` (tipo/campos e exemplo JSON).

Mudancas de codigo requeridas (por crate)
- iroha_data_model
  - Adicionar `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums de instrucoes, eventos e codecs JSON/Norito com testes de roundtrip.
- iroha_core
  - WSV: adicionar registro `runtime_upgrades` com checagens de sobreposicao e getters.
  - Executors: implementar handlers de ISI; emitir eventos; aplicar regras de admissao.
  - Admission: gatilhar manifests de programa por atividade de `abi_version` e igualdade de `abi_hash`.
  - Mapeamento de politica de syscalls: passar o conjunto ABI ativo ao construtor do host VM; garantir determinismo usando a altura do bloco no inicio da execucao.
  - Tests: idempotencia da janela de ativacao, rejeicoes de sobreposicao, comportamento de admissao pre/post.
- ivm
  - Definir `ABI_V2` (exemplo) com politica: estender `abi_syscall_list()`; mapping `is_syscall_allowed(policy, number)`; extensao de politica de tipos de ponteiro.
  - Recomputar e fixar testes golden: `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`.
- iroha_cli / iroha_torii
  - Adicionar endpoints e comandos listados acima; helpers Norito JSON para manifests; testes de integracao basicos.
- Kotodama compiler
  - Permitir targeting `abi_version = v+1`; embutir `abi_hash` correto para a versao selecionada em manifests `.to`.

Telemetria
- Adicionar gauge `runtime.active_abi_versions` e counter `runtime.upgrade_events_total{kind}`.

Consideracoes de seguranca
- Apenas root/sudo pode propor/ativar/cancelar; manifests devem ser assinados apropriadamente.
- Janelas de ativacao evitam front-running e garantem aplicacao determinista.
- `abi_hash` fixa a superficie de interface para evitar drift silencioso entre binarios.

Criterios de aceitacao (Conformance)
- Pre-ativacao, nos rejeitam deterministamente codigo com `abi_version = v+1`.
- Post-ativacao em `start_height`, nos aceitam e executam `v+1`; programas antigos continuam rodando sem mudanca.
- Testes golden para hashes ABI e listas de syscalls passam em x86-64/ARM64.
- A ativacao e idempotente e segura sob reorgs.
