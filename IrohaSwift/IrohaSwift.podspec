Pod::Spec.new do |s|
  s.name             = 'IrohaSwift'
  s.version          = '0.1.0'
  s.summary          = 'Swift SDK for Hyperledger Iroha v2 / Sora Nexus Torii with bundled Norito bridge.'
  s.description      = <<-DESC
A Swift library for interacting with Hyperledger Iroha v2 and Sora Nexus:
- Torii HTTP(S) client (balances, transactions, ZK attachments, prover reports)
- Norito envelope encoder and Connect codec with bridge-backed signing
- Transaction/transfer builders and Ed25519 key management via CryptoKit
DESC
  s.homepage         = 'https://github.com/hyperledger/iroha/tree/main/IrohaSwift'
  s.license          = { :type => 'Apache-2.0', :file => 'LICENSE' }
  s.authors          = { 'Hyperledger Iroha Maintainers' => 'iroha@lists.hyperledger.org' }
  s.source           = {
    :git => 'https://github.com/hyperledger/iroha.git',
    :branch => 'main'
  }
  s.platform         = :ios, '15.0'
  s.osx.deployment_target = '12.0'
  s.swift_versions   = ['5.9']
  s.source_files     = 'Sources/IrohaSwift/**/*.{swift}'
  s.vendored_frameworks = '../dist/NoritoBridge.xcframework'
  s.preserve_paths   = [
    '../dist/NoritoBridge.artifacts.json',
    '../dist/NoritoBridge.xcframework'
  ]
  s.prepare_command  = <<-CMD
    if [ ! -d "../dist/NoritoBridge.xcframework" ]; then
      echo "error: missing dist/NoritoBridge.xcframework. Build via scripts/build_norito_xcframework.sh or place the release artifact under dist/ before linting/consuming the pod." >&2
      exit 1
    fi
    if [ ! -f "../dist/NoritoBridge.artifacts.json" ]; then
      echo "error: missing dist/NoritoBridge.artifacts.json. Keep the signed artifact manifest next to the xcframework for provenance checks." >&2
      exit 1
    fi
  CMD
end
