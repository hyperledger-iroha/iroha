<!-- Japanese translation of docs/profile_build.md -->

---
lang: ja
direction: ltr
source: docs/profile_build.md
status: complete
translator: manual
---

# `iroha_data_model` ビルドのプロファイル取得

`iroha_data_model` のビルド工程で時間がかかるステップを特定するには、次のヘルパースクリプトを実行します。

```sh
./scripts/profile_build.sh
```

スクリプトは `cargo build -p iroha_data_model --timings` を実行し、結果を `target/cargo-timings/` に出力します。生成される `cargo-timing.html` をブラウザで開き、タスクを所要時間順に並べ替えると、どのクレートやビルドステップが最も時間を要しているか確認できます。

タイミング情報を参考に、特に時間の長い処理から最適化を検討してください。
