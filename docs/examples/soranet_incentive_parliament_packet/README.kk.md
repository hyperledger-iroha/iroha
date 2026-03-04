---
lang: kk
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay ынталандыру парламентінің пакеті

Бұл топтама Сора Парламентінің мақұлдауын талап ететін артефактілерді қамтиды
автоматты релелік төлемдер (SNNet-7):

- `reward_config.json` - Norito-серияланатын сыйақы қозғалтқышының конфигурациясы, дайын
  `iroha app sorafs incentives service init` арқылы жұтылуы керек. The
  `budget_approval_id` басқару хаттамасында көрсетілген хэшке сәйкес келеді.
- `shadow_daemon.json` - бенефициар және қайта ойнату арқылы тұтынылатын облигация картасы
  сым (`shadow-run`) және өндірістік демон.
- `economic_analysis.md` - 2025-10 -> 2025-11 үшін әділеттілік туралы қорытынды
  көлеңкелі модельдеу.
- `rollback_plan.md` - автоматты төлемдерді өшіруге арналған операциялық кітап.
- Қолдау артефактілері: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Тұтастықты тексеру

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Дайджесттерді Парламент хаттамасында жазылған мәндермен салыстырыңыз. Тексеру
тармағында сипатталғандай көлеңкелі қолтаңба
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Пакетті жаңарту

1. `reward_config.json` сыйақы салмағы, негізгі төлем немесе кез келген уақытта жаңартыңыз.
   мақұлдау хэшінің өзгеруі.
2. 60 күндік көлеңке модельдеуін қайта іске қосыңыз, `economic_analysis.md` жаңартыңыз.
   жаңа нәтижелер және JSON + бөлінген қолтаңба жұбын жасаңыз.
3. Жаңартылған топтаманы обсерваторияның бақылау тақтасымен бірге Парламентке ұсыныңыз
   қайта растауды сұраған кезде экспорттау.