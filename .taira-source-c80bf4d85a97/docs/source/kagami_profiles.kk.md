---
lang: kk
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 профильдері

Kagami Iroha 3 желілері үшін алдын ала орнатуларды жібереді, осылайша операторлар детерминистикалық штамп қоя алады.
генезис әр желі тұтқаларын жонглерліксіз көрсетеді.

- Профильдер: `iroha3-dev` (тізбек `iroha3-dev.local`, коллекторлар k=1 r=1, NPoS таңдалған кезде тізбек идентификаторынан алынған VRF тұқымы), `iroha3-taira` (тізбек `iroha3-taira`, коллекторлар k=3r қажет), NPoS таңдалған кезде `--vrf-seed-hex`), `iroha3-nexus` (`iroha3-nexus` тізбегі, коллекторлар k=5 r=3, NPoS таңдалған кезде `--vrf-seed-hex` қажет).
- Консенсус: Sora профильдік желілері (Nexus + деректер кеңістігі) NPoS талап етеді және кезеңдік кесулерге рұқсат бермейді; рұқсат етілген Iroha3 орналастырулары Sora профилінсіз іске қосылуы керек.
- Буын: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Nexus үшін `--consensus-mode npos` пайдаланыңыз; `--vrf-seed-hex` тек NPoS үшін жарамды (taira/nexus үшін қажет). Kagami Iroha3 сызығына DA/RBC бекітеді және қорытынды шығарады (тізбек, коллекторлар, DA/RBC, VRF тұқымы, саусақ ізі).
- Тексеру: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` профиль күтулерін қайталайды (тізбек идентификаторы, DA/RBC, коллекторлар, PoP қамтуы, консенсус саусақ ізі). Taira/nexus үшін NPoS манифестін тексергенде ғана `--vrf-seed-hex` жеткізіңіз.
- Үлгі жинақтары: алдын ала жасалған бумалар `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README) астында тұрады. `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` көмегімен қалпына келтіріңіз.
- Mochi: `mochi`/`mochi-genesis` `--genesis-profile <profile>` және `--vrf-seed-hex <hex>` (тек NPoS) қабылдайды, оларды Kagami мекенжайына жіберіңіз және пайдаланылған кездегі Kagami профилін шығарыңыз.

Бумалар топология жазбаларымен қатар BLS PoPs ендірілген, сондықтан `kagami verify` сәтті болады
қораптан тыс; конфигурациялардағы сенімді әріптестерді/порттарды жергілікті үшін қажетінше реттеңіз
түтін шығады.