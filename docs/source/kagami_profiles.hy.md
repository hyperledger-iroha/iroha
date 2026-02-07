---
lang: hy
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 պրոֆիլներ

Kagami-ը տրամադրում է նախադրյալներ Iroha 3 ցանցերի համար, որպեսզի օպերատորները կարողանան սահմանել որոշիչ
genesis-ը դրսևորվում է առանց ցանցային կոճակները ձեռնամուխ լինելու:

- Պրոֆիլներ. երբ ընտրված է NPoS), `iroha3-nexus` (շղթա `iroha3-nexus`, կոլեկտորներ k=5 r=3, պահանջում է `--vrf-seed-hex`, երբ ընտրված է NPoS):
- Կոնսենսուս. Sora պրոֆիլային ցանցերը (Nexus + տվյալների տարածություններ) պահանջում են NPoS և թույլ չեն տալիս փուլային կտրվածքներ; թույլատրված Iroha3 տեղակայումները պետք է աշխատեն առանց Sora պրոֆիլի:
- Սերունդ՝ `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`: Օգտագործեք `--consensus-mode npos` Nexus-ի համար; `--vrf-seed-hex`-ը վավեր է միայն NPoS-ի համար (պահանջվում է թեստի/նեքուսի համար): Kagami-ը ամրացնում է DA/RBC-ն Iroha3 գծի վրա և թողարկում ամփոփում (շղթա, հավաքիչներ, DA/RBC, VRF սերմ, մատնահետք):
- Ստուգում. `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]`-ը կրկնում է պրոֆիլի ակնկալիքները (շղթայի id, DA/RBC, կոլեկցիոներներ, PoP ծածկույթ, համաձայնության մատնահետք): Տրամադրեք `--vrf-seed-hex` միայն այն ժամանակ, երբ ստուգեք NPoS մանիֆեստը թեստուսի/նեքուսի համար:
- Նմուշային փաթեթներ. նախապես ստեղծված փաթեթները գործում են `defaults/kagami/iroha3-{dev,testus,nexus}/`-ի ներքո (genesis.json, config.toml, docker-compose.yml, verify.txt, README): Վերականգնել `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`-ով:
- Mochi. `mochi`/`mochi-genesis` ընդունեք `--genesis-profile <profile>` և `--vrf-seed-hex <hex>` (միայն NPoS), փոխանցեք դրանք Kagami և տպեք նույն Kagami-ին, երբ օգտագործվում է նույն Kagami-ի համար:

Փաթեթները տեղադրում են BLS PoP-ները տոպոլոգիայի մուտքերի կողքին, որպեսզի `kagami verify` հաջողվի
տուփից դուրս; հարմարեցնել վստահելի հասակակիցները/պորտերը կոնֆիգուրացիաներում, ըստ անհրաժեշտության տեղական համար
ծուխը հոսում է.