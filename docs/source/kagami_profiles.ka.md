---
lang: ka
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 პროფილები

Kagami აგზავნის წინასწარ პარამეტრებს Iroha 3 ქსელებისთვის, რათა ოპერატორებმა შეძლონ დეტერმინისტიკის დადგენა
გენეზისი ვლინდება ქსელის ღილაკების ჟონგლირების გარეშე.

- პროფილები: `iroha3-dev` (ჯაჭვი `iroha3-dev.local`, კოლექციონერები k=1 r=1, VRF თესლი მიღებული ჯაჭვის id-დან NPoS-ის არჩევისას), `iroha3-testus` (ჯაჭვი `iroha3-testus`, `iroha3-testus`, `iroha3-testus`, `iroha3-testus`, `iroha3-testus`, `iroha3-testus`, Kagami, Kagami, Kagami, Kagami, კოლექციონერები k=130X, კოლექციონერები. როდესაც არჩეულია NPoS), `iroha3-nexus` (ჯაჭვი `iroha3-nexus`, კოლექტორები k=5 r=3, საჭიროებს `--vrf-seed-hex`-ს NPoS არჩევისას).
- კონსენსუსი: Sora პროფილის ქსელები (Nexus + მონაცემთა სივრცეები) საჭიროებს NPoS-ს და აკრძალავს ეტაპობრივ ამოჭრას; ნებადართული Iroha3 განლაგება უნდა აწარმოოს Sora პროფილის გარეშე.
- თაობა: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. გამოიყენეთ `--consensus-mode npos` Nexus-ისთვის; `--vrf-seed-hex` მოქმედებს მხოლოდ NPoS-ისთვის (აუცილებელია ტესტუსისთვის/ნექსისთვის). Kagami ამაგრებს DA/RBC-ს Iroha3 ხაზზე და გამოსცემს შეჯამებას (ჯაჭვი, კოლექტორები, DA/RBC, VRF თესლი, თითის ანაბეჭდი).
- დადასტურება: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` იმეორებს პროფილის მოლოდინებს (ჯაჭვის ID, DA/RBC, კოლექციონერები, PoP დაფარვა, კონსენსუსის თითის ანაბეჭდი). მიაწოდეთ `--vrf-seed-hex` მხოლოდ NPoS მანიფესტის გადამოწმებისას ტესტუსისთვის/ნექსისთვის.
- პაკეტების ნიმუში: წინასწარ გენერირებული პაკეტები მოქმედებს `defaults/kagami/iroha3-{dev,testus,nexus}/`-ის ქვეშ (genesis.json, config.toml, docker-compose.yml, verify.txt, README). რეგენერაცია `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`-ით.
- მოჩი: `mochi`/`mochi-genesis` მიიღე `--genesis-profile <profile>` და `--vrf-seed-hex <hex>` (მხოლოდ NPoS), გადააგზავნე ისინი Kagami-ზე და დაბეჭდე იგივე Kagami-ზე, როდესაც გამოყენებულია იგივე Kagami-ზე.

პაკეტებში ჩაშენებულია BLS PoP-ები ტოპოლოგიის ჩანაწერებთან ერთად, ასე რომ `kagami verify` წარმატებული იქნება
ყუთიდან; დაარეგულირეთ სანდო თანატოლები/პორტები კონფიგურაციებში, როგორც საჭიროა ადგილობრივისთვის
კვამლი გადის.