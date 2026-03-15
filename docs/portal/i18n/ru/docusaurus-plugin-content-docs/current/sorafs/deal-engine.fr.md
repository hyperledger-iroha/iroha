---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: механизм сделок
Название: Moteur d'accords SoraFS
Sidebar_label: Двигатель согласия
описание: Комплект приводов SF-8, интеграция Torii и поверхностей телеметрии.
---

:::note Источник канонический
:::

# Двигатель соглашений SoraFS

Дорожная карта SF-8 знакомит с двигателем соглашения SoraFS, Fournissant
определенная совместимость для договоренностей о складских запасах и рекуперации между ними
клиенты и повара. Согласование с помощью полезных данных Norito
definis dans `crates/sorafs_manifest/src/deal.rs`, couvrant les termes de l'accord,
le verrouillage de Bonds, les micropaiements вероятност и les enregistrements de reglement.

Рабочий SoraFS поставлен на место (`sorafs_node::NodeHandle`) в мгновение ока
`DealEngine` для каждого процесса. Мотор:

- подтвердить и зарегистрировать соглашения через `DealTermsV1` ;
- кумуля обвинений в неправомерных действиях в XOR или использование репликации est rapporté;
- оценить вероятностные отверстия микроплат через детерминированный механизм
  основа на BLAKE3 ; и др.
- создание снимков бухгалтерской книги и полезных данных правил, адаптированных к публикации
  де управление.

Единые тесты, необходимые для проверки, выбора микроплат и потоков
правила, согласно которым операторы могут наиболее эффективно использовать API в доверительном режиме. Правила
отключенные полезные нагрузки управления `DealSettlementV1`, интегрированное управление
в конвейере публикации SF-12 и в Jour La Série OpenTelemetry `sorafs.node.deal_*`
(И18НИ00000017Х, И18НИ00000018Х, И18НИ00000019Х,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) для панелей мониторинга Torii и др.
приложение SLO. Les travaux suivants ciblent l'automisation du slashing initiée par
аудиторы и координация семантики аннуляции с политикой управления.

Телеметрия для использования всего набора метрик `sorafs.node.micropayment_*`:
И18НИ00000024Х, И18НИ00000025Х,
И18НИ00000026Х, И18НИ00000027Х,
`micropayment_outstanding_nano` и контролеры билетов
(И18НИ00000029Х, И18НИ00000030Х,
`micropayment_tickets_duplicate_total`). Все это раскрывает поток лотереи
Вероятность того, что операторы могут эффективно коррелировать с прибылью от микроплатежей и т. д.
перенос кредита с результатами регулирования.

## Интеграция Torii

Torii раскрывает конечные точки, которые сигнализируют об использовании и пилотировании
Цикл жизни аккордов без специальной проводки:- `POST /v2/sorafs/deal/usage` принять телеметрию `DealUsageReport` и отправить
  детерминированные результаты совместимости (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` завершить курантский фенетр в потоковом режиме
  `DealSettlementRecord` — результат `DealSettlementV1`, закодированный в base64
  прет для публикации в DAG de gouvernance.
- Подача `/v2/events/sse` от Torii диффузное нарушение регистрации
  `SorafsGatewayEvent::DealUsage` резюме chaque soumission d'usage (эпоха, GiB-часы,
  контролеры билетов, детерминированные сборы), регистрация
  `SorafsGatewayEvent::DealSettlement`, который включает в себя снимок канонического реестра правил
  ainsi que le ignore/taille/base64 BLAKE3 de l'artefact de gouvernance sur disque, et des alertes
  `SorafsGatewayEvent::ProofHealth` из тех, которые PDP/PoTR исчезли (fournisseur, fenêtre,
  Этат удара/перезарядки, монтант де пеналите). Les consommateurs peuvent filtrer par Fournisseur
  для регулирования новых телеметрий, правил или предупреждений о санитарных доказательствах без опроса.

Две конечные точки, участвующие в рамках квот SoraFS через новое окно
`torii.sorafs.quota.deal_telemetry`, доступ к функциям регулировки тау-де-сумиссии
авторизовано при развертывании.