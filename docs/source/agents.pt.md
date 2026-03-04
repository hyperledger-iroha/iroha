---
lang: pt
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2026-01-03T18:08:01.361022+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guia de execução do agente de automação

Esta página resume as proteções operacionais para qualquer agente de automação
trabalhando dentro do espaço de trabalho Hyperledger Iroha. Ele reflete o canônico
A orientação `AGENTS.md` e as referências do roteiro para construção, documentação e
todas as mudanças de telemetria parecem iguais, quer tenham sido produzidas por um ser humano ou
um contribuidor automatizado.

Espera-se que cada tarefa forneça código determinístico, além de documentos, testes,
e evidências operacionais. Trate as seções abaixo como uma referência pronta antes
tocando em itens `roadmap.md` ou respondendo a perguntas de comportamento.

## Comandos de início rápido

| Ação | Comando |
|--------|---------|
| Construa o espaço de trabalho | `cargo build --workspace` |
| Execute o conjunto de testes completo | `cargo test --workspace` *(normalmente leva várias horas)* |
| Execute o clippy com avisos de negação por padrão | `cargo clippy --workspace --all-targets -- -D warnings` |
| Formatar código Rust | `cargo fmt --all` *(edição 2024)* |
| Teste uma única caixa | `cargo test -p <crate>` |
| Execute um teste | `cargo test -p <crate> <test_name> -- --nocapture` |
| Testes Swift SDK | Em `IrohaSwift/`, execute `swift test` |

## Fundamentos do fluxo de trabalho

- Leia os caminhos de código relevantes antes de responder perguntas ou alterar a lógica.
- Divida grandes itens do roadmap em commits tratáveis; nunca rejeite o trabalho imediatamente.
- Permaneça dentro da associação existente ao espaço de trabalho, reutilize caixas internas e faça
  **não** altere `Cargo.lock`, a menos que seja explicitamente instruído.
- Use sinalizadores de recursos e alternadores de capacidade somente quando exigido pelo hardware
  aceleradores; mantenha substitutos determinísticos disponíveis em todas as plataformas.
- Atualizar documentação e referências de Markdown juntamente com qualquer alteração funcional
  então os documentos sempre descrevem o comportamento atual.
- Adicione pelo menos um teste unitário para cada função nova ou modificada. Prefira in-line
  Módulos `#[cfg(test)]` ou a pasta `tests/` da caixa dependendo do escopo.
- Após terminar o trabalho, atualize `status.md` com um breve resumo e referência
  arquivos relevantes; mantenha `roadmap.md` focado em itens que ainda precisam de reparos.

## Protetores de implementação

### Serialização e modelos de dados
- Use o codec Norito em qualquer lugar (binário via `norito::{Encode, Decode}`,
  JSON via `norito::json::*`). Não adicione uso direto de serde/`serde_json`.
- As cargas úteis Norito devem anunciar seu layout (byte de versão ou sinalizadores de cabeçalho),
  e novos formatos exigem atualizações de documentação correspondentes (por exemplo,
  `norito.md`, `docs/source/da/*.md`).
- Os dados, manifestos e cargas de rede do Genesis devem permanecer determinísticos
  então dois pares com as mesmas entradas produzem hashes idênticos.

### Configuração e comportamento em tempo de execução
- Prefira botões que residem em `crates/iroha_config` em vez de novas variáveis de ambiente.
  Encadeie valores explicitamente por meio de construtores ou injeção de dependência.
- Nunca bloqueie syscalls IVM ou comportamento de código de operação - o ABI v1 é enviado para todos os lugares.
- Quando novas opções de configuração forem adicionadas, atualize padrões, documentos e qualquer coisa relacionada
  modelos (`peer.template.toml`, `docs/source/configuration*.md`, etc.).### ABI, Syscalls e tipos de ponteiro
- Trate a política da ABI como incondicional. Adicionando/removendo syscalls ou tipos de ponteiro
  requer atualização:
  -`ivm::syscalls::abi_syscall_list` e `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` mais os testes de ouro
  - `crates/ivm/tests/abi_hash_versions.rs` sempre que o hash ABI muda
- Syscalls desconhecidos devem ser mapeados para `VMError::UnknownSyscall` e os manifestos devem
  reter verificações de igualdade `abi_hash` assinadas em testes de admissão.

### Aceleração e Determinismo de Hardware
- Novas primitivas criptográficas ou matemática pesada devem ser fornecidas aceleradas por hardware
  caminhos (METAL/NEON/SIMD/CUDA) enquanto mantém substitutos determinísticos.
- Evitar reduções paralelas não determinísticas; a prioridade são resultados idênticos em
  cada peer mesmo quando o hardware for diferente.
- Mantenha os equipamentos Norito e FASTPQ reproduzíveis para que o SRE possa auditar toda a frota
  telemetria.

### Documentação e evidências
- Espelhe qualquer alteração de documento público no portal (`docs/portal/...`) quando
  aplicável para que o site de documentos permaneça atualizado com as fontes do Markdown.
- Quando novos fluxos de trabalho forem introduzidos, adicione runbooks, notas de governança ou
  listas de verificação explicando como ensaiar, reverter e capturar evidências.
- Ao traduzir conteúdo para acadiano, forneça representações semânticas escritas
  em transliterações cuneiformes em vez de fonéticas.

### Expectativas de testes e ferramentas
- Execute os conjuntos de testes relevantes localmente (`cargo test`, `swift test`,
  chicotes de integração) e documente os comandos na seção de testes PR.
- Mantenha os scripts de proteção de CI (`ci/*.sh`) e os painéis sincronizados com a nova telemetria.
- Para macros proc, emparelhe testes de unidade com testes de UI `trybuild` para bloquear diagnósticos.

## Lista de verificação pronta para envio

1. O código é compilado e `cargo fmt` não produziu diferenças.
2. Documentos atualizados (markdown do espaço de trabalho mais espelhos do portal) descrevem o novo
   comportamento, novos sinalizadores CLI ou botões de configuração.
3. Os testes cobrem cada novo caminho de código e falham deterministicamente quando as regressões
   aparecer.
4. Telemetria, painéis e definições de alerta fazem referência a quaisquer novas métricas ou
   códigos de erro.
5. `status.md` inclui um breve resumo referenciando os arquivos relevantes e
   seção do roteiro.

Seguir esta lista de verificação mantém a execução do roteiro auditável e garante que todos
agente contribui com evidências nas quais outras equipes podem confiar.