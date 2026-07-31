version_file = File.join(__dir__, 'VERSION')
unless File.file?(version_file) && !File.symlink?(version_file)
  raise 'IrohaSwift VERSION must be a regular non-symlink file'
end

version = File.binread(version_file).strip
canonical_semver = /\A(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\z/
raise 'IrohaSwift VERSION must be canonical SemVer' unless canonical_semver.match?(version)

Pod::Spec.new do |s|
  s.name             = 'IrohaSwift'
  s.version          = version
  s.summary          = 'Swift SDK for Hyperledger Iroha 3 and SORA Nexus Torii.'
  s.description      = <<-DESC
A Swift library for interacting with Hyperledger Iroha 3 and SORA Nexus:
- Torii HTTP(S) client (balances, transactions, ZK attachments, prover reports)
- Norito envelope encoder and Connect codec with required bridge-backed signing
- Transaction/transfer builders and Ed25519 key management via CryptoKit
DESC
  s.homepage         = 'https://github.com/hyperledger-iroha/iroha/tree/main/IrohaSwift'
  s.license          = { :type => 'Apache-2.0', :file => 'LICENSE' }
  s.authors          = { 'Hyperledger Iroha Maintainers' => 'iroha@lists.hyperledger.org' }
  s.source           = {
    :git => 'https://github.com/hyperledger-iroha/iroha.git',
    :tag => "iroha-swift-v#{version}"
  }
  s.platform         = :ios, '15.0'
  s.swift_versions   = ['5.9']
  s.source_files     = 'Sources/IrohaSwift/**/*.{swift}'
  s.pod_target_xcconfig = {
    'OTHER_LDFLAGS' => '-all_load'
  }
  s.user_target_xcconfig = {
    'OTHER_LDFLAGS' => '-all_load'
  }
end
