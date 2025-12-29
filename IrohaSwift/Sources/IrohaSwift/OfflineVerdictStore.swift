import Foundation

/// Compatibility shim retained so integrators that reference the renamed
/// `OfflineVerdictStore.swift` file continue to build until they regenerate
/// their Pods workspace.
@available(*, deprecated, renamed: "OfflineVerdictJournal")
public typealias OfflineVerdictStore = OfflineVerdictJournal
