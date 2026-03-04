---
lang: pt
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2026-01-03T18:07:57.069144+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Modelo de arquivamento de produto % SM2/SM3/SM4 (开发备案)
% Hyperledger Iroha Grupo de Trabalho de Conformidade
% 2026-05-06

# Instruções

Use este modelo ao enviar um *arquivo de desenvolvimento de produto* para um departamento provincial
ou escritório municipal da Administração de Criptografia do Estado (SCA) antes de distribuir
Binários habilitados para SM ou artefatos de origem da China continental. Substitua o
espaços reservados com detalhes específicos do projeto, exporte o formulário preenchido como PDF se
necessário e anexe os artefatos mencionados na lista de verificação.

Nº 1. Resumo do candidato e do produto

| Campo | Valor |
|-------|-------|
| Nome da organização | {{ ORGANIZAÇÃO }} |
| Endereço registrado | {{ ENDEREÇO ​​}} |
| Representante legal | {{ LEGAL_REP }} |
| Contato principal (nome/cargo/e-mail/telefone) | {{ CONTATO }} |
| Nome do produto | Hyperledger Iroha {{ RELEASE_NAME }} |
| Versão do produto/ID de compilação | {{VERSÃO}} |
| Tipo de arquivamento | Desenvolvimento de produto (开发备案) |
| Data de depósito | {{AAAA-MM-DD }} |

# 2. Visão geral do uso de criptografia

- Algoritmos suportados: `SM2`, `SM3`, `SM4` (forneça a matriz de uso abaixo).
- Contexto de uso:
  | Algoritmo | Componente | Finalidade | Salvaguardas determinísticas |
  |-----------|-----------|---------|--------------------------|
  | SM2 | {{ COMPONENTE }} | {{ OBJETIVO }} | RFC6979 + aplicação canônica de r∥s |
  | SM3 | {{ COMPONENTE }} | {{ OBJETIVO }} | Hash determinístico via `Sm3Digest` |
  | SM4 | {{ COMPONENTE }} | {{ OBJETIVO }} | AEAD (GCM/CCM) com política de nonce aplicada |
- Algoritmos não SM na construção: {{ OTHER_ALGORITHMS }} (para integridade).

# 3. Controles de desenvolvimento e cadeia de suprimentos

- Repositório de código-fonte: {{ REPOSITORY_URL }}
- Instruções de construção determinísticas:
  1.`git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"` (ajuste conforme necessário).
  3. SBOM gerado via `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`).
- Resumo do ambiente de integração contínua:
  | Artigo | Valor |
  |------|-------|
  | Construir SO/versão | {{BUILD_OS}} |
  | Conjunto de ferramentas do compilador | {{ CADEIA DE FERRAMENTAS }} |
  | Fonte OpenSSL / Tongsuo | {{ OPENSSL_SOURCE }} |
  | Soma de verificação de reprodutibilidade | {{ CHECKSUM }} |

# 4. Gerenciamento e segurança de chaves

- Recursos SM habilitados por padrão: {{ DEFAULTS }} (por exemplo, somente verificação).
- Sinalizadores de configuração necessários para assinatura: {{ CONFIG_FLAGS }}.
- Abordagem de custódia de chaves:
  | Artigo | Detalhes |
  |------|---------|
  | Ferramenta de geração de chaves | {{KEY_TOOL}} |
  | Meio de armazenamento | {{STORAGE_MEDIUM}} |
  | Política de backup | {{ BACKUP_POLICY }} |
  | Controles de acesso | {{ ACCESS_CONTROLS }} |
- Contatos de resposta a incidentes (24 horas por dia, 7 dias por semana):
  | Função | Nome | Telefone | E-mail |
  |------|------|-------|-------|
  | Liderança criptográfica | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} |
  | Operações de plataforma | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} |
  | Contato jurídico | {{ NOME }} | {{ TELEFONE }} | {{ E-MAIL }} |

# 5. Lista de verificação de anexos- [] Instantâneo do código-fonte (`{{ SOURCE_ARCHIVE }}`) e hash.
- [] Script de construção determinístico/notas de reprodutibilidade.
- [ ] SBOM (`{{ SBOM_PATH }}`) e manifesto de dependência (impressão digital `Cargo.lock`).
- [ ] Transcrições de testes determinísticos (`scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm`).
- [] Exportação do painel de telemetria demonstrando a observabilidade do SM.
- [ ] Instrução de controle de exportação (consulte o modelo separado).
- [ ] Relatórios de auditoria ou avaliações de terceiros (se já concluídos).

# 6. Declaração do Requerente

> Confirmo que as informações acima são precisas, que as informações divulgadas
> a funcionalidade criptográfica está em conformidade com as leis e regulamentos aplicáveis da RPC,
> e que a organização manterá os artefactos submetidos durante pelo menos
> três anos.

- Assinatura (representante legal): ________________________
- Data: ________________________