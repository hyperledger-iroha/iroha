---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptação de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de transferência e identificação de fornecedores SoraFS (rede SF-2b)

Esta é a configuração exata dos resultados práticos para **SF-2b**: определить и
принудительно применять процесс допуска, требования к идентичности и
Carga útil de certificação para a configuração SoraFS. Ona расширяет
Prossiga seu negócio, consulte o arquiteto RFC SoraFS e разбивает
оставшуюся работу на отслеживаемые инженерные задачи.

## Цели политики

- Certifique-se de que o operador autorizado possa publicar a descrição `ProviderAdvertV1`, que é a primeira opção.
- Привязать каждый ключ объявления к документу идентичности, утвержденному governança, аттестованным эндпоинтам и aposta mínima.
- Verifique a determinação do instrumento, como Torii, шлюзы e `sorafs-node` preliminares одинаковые проверки.
- Você pode obter a confirmação e a anulação de qualquer instrumento que possa ser usado.

## Código de identificação e participação

| Treino | Descrição | Resultado |
|------------|----------|-----------|
| Происхождение ключа объявления | Провайдеры должны зарегистрировать пару ключей Ed25519, которой подписывается каждое anúncio. Бандл допуска хранит публичный ключ вместе с подписью governança. | Registre o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) e salve-o em uma nova restauração (`sorafs_manifest::provider_admission`). |
| Estaca Указатель | Para obter o novo `StakePointer`, você pode usar o pool de piquetagem ativo. | Valide-o em `sorafs_manifest::provider_advert::StakePointer::validate()` e execute-o em CLI/teste. |
| Теги юрисдикции | O provedor possui contato direto + contato direto. | Selecione o esquema padrão `jurisdiction_code` (ISO 3166-1 alpha-2) e o opcional `contact_uri`. |
| Local de certificação | O ponto de acesso de segurança do servidor pode ser usado para obter a certificação mTLS ou QUIC. | Selecione a carga útil Norito `EndpointAttestationV1` e instale-a no local onde está a transferência da banda. |

##Process допуска

1. **Создание предложения**
   - CLI: download `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`,
     formulário `ProviderAdmissionProposalV1` + certificado de banda.
   - Validação: убедиться в наличии обязательных полей, aposta> 0, канонического chunker handle em `profile_id`.
2. **Governança de Governança**
   - Совет подписывает `blake3("sorafs-provider-admission-v1" || canonical_bytes)` используя существующие
     envelope de instrumento (modelo `sorafs_manifest::governance`).
   - Envelope enviado em `governance/providers/<provider_id>/admission.json`.
3. **Configurar na restauração**
   - Реализовать общий валидатор (`sorafs_manifest::provider_admission::validate_envelope`), который
     verifique Torii/шлюзы/CLI.
   - Обновить путь допуска Torii, чтобы отклонять anúncios, у которых digest ou срок действия отличается от envelope.
4. **Remoção e remoção**
   - Добавить `ProviderAdmissionRenewalV1` com suporte opcional/stake.
   - Открыть путь CLI `--revoke`, который фиксирует причину отзыва e отправляет событие governança.

## Задачи реализации| Opção | Bem | Proprietário(s) | Status |
|--------|--------|----------|--------|
| Esquema | Selecione `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) em `crates/sorafs_manifest/src/provider_admission.rs`. Realizado em `sorafs_manifest::provider_admission` com validação.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Armazenamento / Governança | ✅ Melhor |
| CLI de instrumentos | A versão `sorafs_manifest_stub` é composta por: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | ✅ Melhor |

CLI pode usar a certificação de banda larga (`--endpoint-attestation-intermediate`),
Você pode usar canonicamente um envelope/envelope e testar uma cópia do envelope no local `sign`/`verify`. Operador poderoso
передавать тела advert напрямую или переиспользовать подписанные anúncios, а файлы подписей можно
Por favor, instale `--council-signature-public-key` com `--council-signature-file` para instalação automática.

### Справочник CLI

Execute o comando `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Bandeiras de identificação: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e como mínimo `--endpoint=<kind:host>`.
  - Para a certificação do ponto de venda `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, certificado de segurança
    `--endpoint-attestation-leaf=<path>` (opcional `--endpoint-attestation-intermediate=<path>`
    para o elemento элемента цепочки) e любые согласованные ALPN ID
    (`--endpoint-attestation-alpn=<token>`). Para o QUIC-эндпоинтов можно передать транспортные отчеты через
    `--endpoint-attestation-report[-hex]=...`.
  - Выход: канонические байты предложения Norito (`--proposal-out`) e JSON-сводка
    (stdout para умолчанию ou `--json-out`).
-`sign`
  - Входные данные: предложение (`--proposal`), подписанный anúncio (`--advert`), опциональное тело anúncio
    (`--advert-body`), época de retenção e quanto tempo é mínimo. Você pode fazer isso
    inline (`--council-signature=<signer_hex:signature_hex>`) ou uma linha de código, compatível
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Envelope de validação de formulário (`--envelope-out`) e JSON-отчет com resumo privado,
    числом подписантов и входными путями.
-`verify`
  - Prover o envelope completo (`--envelope`) e fornecer uma solução opcional,
    anúncio ou anúncio. JSON-отчет подсвечивает значения digest, статус проверки подписей
    e quais artefatos opcionais são fornecidos.
-`renewal`
  - Связывает новый утвержденный envelope com ранее утвержденным resumo. Требуются
    `--previous-envelope=<path>` e possivelmente `--envelope=<path>` (ou carga útil Norito).
    Provedor CLI, quais aliases de perfil, recursos e chave de anúncio são necessários, por este documento
    participação de обновления, эндпоинтов и metadados. Usando uma bateria canônica `ProviderAdmissionRenewalV1`
    (`--renewal-out`) mais JSON.
-`revoke`
  - Выпускает аварийный бандл `ProviderAdmissionRevocationV1` para prova, чей envelope нужно отозвать.
    Obrigado `--envelope=<path>`, `--reason=<text>`, como mínimo de `--council-signature` e opcional
    `--revoked-at`/`--notes`. CLI pode fornecer e fornecer o resumo do arquivo, verificando a carga útil Norito
    `--revocation-out` e contém JSON-отчет com digest e числом подписей.
| Prova | Verifique o valor do validador, usando Torii, шлюзами e `sorafs-node`. Предоставить unit + CLI testes de integração.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Rede TL / Armazenamento | ✅ Melhor |
| Integração Torii | Você pode validar anúncios de anúncios em Torii, exibir anúncios de maneira política e telefônica pública. | Rede TL | ✅ Melhor | Torii contém envelopes de governança (`torii.sorafs.admission_envelopes_dir`), fornecer resumo de compilação/processamento de conteúdo e telefone público допуска.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || Avaliação | Faça o download de esquemas/opções + CLI, abra o ciclo no documento (como o runbook não e comandos CLI em `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Armazenamento / Governança | ✅ Melhor |
| Telemetria | Определить dashboards/alertas `provider_admission` (пропущенное обновление, срок действия envelope). | Observabilidade | 🟠 Em processo | A chave `torii_sorafs_admission_total{result,reason}` está correta; painéis/alertas em trabalho.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook atualizado e atualizado

#### Плановое обновление (обновления estaca/топологии)
1. Соберите пару последующего предложения/anúncio через `provider-admission proposal` e `provider-admission sign`,
   увеличив `--retention-epoch` e обновив estaca/эндпоинты по необходимости.
2. Выполните
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда проверяет неизменность полей capacidade/perfil через
   `AdmissionRecord::apply_renewal`, выпускает `ProviderAdmissionRenewalV1` e печатает resumos para
   журнала governança.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Crie um envelope pré-definido em `torii.sorafs.admission_envelopes_dir`, crie um envelope Norito/JSON
   na governança do repositório e na criação de hash обновления + época de retenção em `docs/source/sorafs/migration_ledger.md`.
4. Selecione o operador, qual novo envelope está ativado e desativado
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para a substituição do produto.
5. Ajuste e ajuste os acessórios canônicos do número `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) fornece o padrão Norito.

#### Аварийный отзыв
1. Abra o envelope e verifique-o:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI подписывает `ProviderAdmissionRevocationV1`, prove набор подписей через
   `verify_revocation_signatures` e um resumo do arquivo.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Abra o envelope de `torii.sorafs.admission_envelopes_dir`, transfira Norito/JSON para o cartão de admissão
   e proteger os princípios de hash no protocolo de governança.
3. Selecione `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, isso pode ser feito,
   что кеши отбросили отозванный anúncio; храните артефакты отзыва в ретроспективах инцидента.

## Teste e telemetria- Adicione luminárias douradas para entrega e entrega de envelopes
  `fixtures/sorafs_manifest/provider_admission/`.
- Расширить CI (`ci/check_sorafs_fixtures.sh`) para a transferência de envelopes e envelopes.
- Dispositivos elétricos Сгенерированные включают `metadata.json` com resumos canônicos; downstream-тесты утверждают
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Testes de integração adicionais:
  - Torii отклоняет anúncios с отсутствующими или просроченными envelopes de admissão.
  - CLI проходит ida e volta предложения → envelope → verificação.
  - A governança de segurança depende do atestado de endpoint sem o ID do provedor de identificação.
- Treinar por telemetria:
  - Emite contadores `provider_admission_envelope_{accepted,rejected}` em Torii. ✅ `torii_sorafs_admission_total{result,reason}` теперь показывает aceito/rejeitado.
  - Faça o download da experiência de trabalho no dia seguinte (será concluído em 7 dias).

## Следующие шаги

1. ✅ Selecione o esquema de configuração Norito e verifique a validação em `sorafs_manifest::provider_admission`. Sinalizadores de recursos não são úteis.
2. ✅ CLI потоки (`proposal`, `sign`, `verify`, `renewal`, `revoke`) задокументированы и проверены testes de integração; você pode fornecer scripts de governança em sincronização com runbook.
3. ✅ Torii admissão/descoberta принимает envelopes и публикует телеметрические счетчики принятия/отклонения.
4. Foco na configuração: abrir painéis/alertas para admissão, opções de inicialização, tarefas no final do mês, data de início предупреждения (`torii_sorafs_admission_total`, medidores de validade).