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
  # Symlink created by prepare_command points to ../dist/NoritoBridge.xcframework
  s.vendored_frameworks = 'NoritoBridge.xcframework'
  s.preserve_paths   = [
    'NoritoBridge.xcframework'
  ]
  s.static_framework = true
  s.pod_target_xcconfig = {
    'OTHER_LDFLAGS' => '-all_load'
  }
  s.user_target_xcconfig = {
    'OTHER_LDFLAGS' => '-all_load'
  }
  s.prepare_command  = <<-CMD
    if [ ! -e "NoritoBridge.xcframework" ]; then
      if [ -d "../dist/NoritoBridge.xcframework" ]; then
        ln -s ../dist/NoritoBridge.xcframework NoritoBridge.xcframework
      else
        echo "error: missing NoritoBridge.xcframework. Build via scripts/build_norito_xcframework.sh first." >&2
        exit 1
      fi
    fi
  CMD
end
