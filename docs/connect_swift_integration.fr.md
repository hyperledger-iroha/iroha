---
lang: fr
direction: ltr
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/connect_swift_integration.md (Integrating NoritoBridgeKit) -->

## Intégrer NoritoBridgeKit dans un Projet iOS (Xcode)

Ce guide montre comment intégrer le bridge Norito en Rust (XCFramework) et les
wrappers Swift dans une application iOS, puis échanger des frames Iroha
Connect via WebSocket en réutilisant les mêmes codecs Norito que l’hôte Rust.

Prérequis
- Une archive zip `NoritoBridge.xcframework` (générée par le workflow de CI) et
  le helper Swift `NoritoBridgeKit.swift` (copiez la version située dans
  `examples/ios/NoritoDemo/Sources` si vous n’utilisez pas directement le
  projet de démonstration).
- Xcode 15+ et cible iOS 13+.

Option A : Swift Package Manager (recommandée)
1) Publiez un SPM binaire à partir de `Package.swift.template` dans
   `crates/connect_norito_bridge/` (remplissez l’URL et le checksum depuis la
   CI).
2) Dans Xcode : File → Add Packages… → saisissez l’URL du dépôt SPM → ajoutez
   le produit `NoritoBridge` à votre target.
3) Ajoutez `NoritoBridgeKit.swift` à la cible de votre application (glisser‑déposer
   dans le projet, en coch
