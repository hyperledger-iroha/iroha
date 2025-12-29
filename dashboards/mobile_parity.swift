#!/usr/bin/env swift
//
// dashboards/mobile_parity.swift
//
// Generates a concise summary of the Swift Norito parity dashboard data. The
// script expects a JSON payload that matches the schema documented in
// docs/source/references/ios_metrics.md (see the sample data under
// dashboards/data/mobile_parity.sample.json).

import Foundation

// MARK: - Models

struct ParityDashboard: Decodable {
    struct FixtureDetail: Decodable {
        let instruction: String
        let ageHours: Double
        let owner: String

        private enum CodingKeys: String, CodingKey {
            case instruction
            case ageHours = "age_hours"
            case owner
        }
    }

    struct Fixtures: Decodable {
        let outstandingDiffs: Int
        let oldestDiffHours: Double
        let details: [FixtureDetail]

        private enum CodingKeys: String, CodingKey {
            case outstandingDiffs = "outstanding_diffs"
            case oldestDiffHours = "oldest_diff_hours"
            case details
        }
    }

    struct Pipeline: Decodable {
        let lastRun: Date?
        let status: String
        let failedTests: [String]
        let metadata: Metadata?
        let metadataSource: String?

        private enum CodingKeys: String, CodingKey {
            case lastRun = "last_run"
            case status
            case failedTests = "failed_tests"
            case metadata
            case metadataSource = "metadata_source"
        }

        struct Metadata: Decodable {
            struct TestSummary: Decodable {
                let name: String
                let durationSeconds: Double

                private enum CodingKeys: String, CodingKey {
                    case name
                    case durationSeconds = "duration_seconds"
                }
            }

            let jobName: String?
            let durationSeconds: Double?
            let tests: [TestSummary]?

            private enum CodingKeys: String, CodingKey {
                case jobName = "job_name"
                case durationSeconds = "duration_seconds"
                case tests
            }
        }
    }

    struct RegenSLA: Decodable {
        let lastSuccess: Date?
        let hoursSinceSuccess: Double
        let breach: Bool

        private enum CodingKeys: String, CodingKey {
            case lastSuccess = "last_success"
            case hoursSinceSuccess = "hours_since_success"
            case breach
        }
    }

    struct Alert: Decodable {
        let message: String
        let severity: String?
    }

    struct Telemetry: Decodable {
        let saltEpoch: String?
        let saltRotationAgeHours: Double?
        let overridesOpen: Int?
        let deviceProfileAlignment: String?
        let schemaVersion: String?
        let schemaPolicyViolationCount: Int?
        let schemaPolicyViolations: [String]
        let notes: [String]

        private enum CodingKeys: String, CodingKey {
            case saltEpoch = "salt_epoch"
            case saltRotationAgeHours = "salt_rotation_age_hours"
            case overridesOpen = "overrides_open"
            case deviceProfileAlignment = "device_profile_alignment"
            case schemaVersion = "schema_version"
            case schemaPolicyViolationCount = "schema_policy_violation_count"
            case schemaPolicyViolations = "schema_policy_violations"
            case notes
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            saltEpoch = try container.decodeIfPresent(String.self, forKey: .saltEpoch)
            saltRotationAgeHours = try container.decodeIfPresent(Double.self, forKey: .saltRotationAgeHours)
            overridesOpen = try container.decodeIfPresent(Int.self, forKey: .overridesOpen)
            deviceProfileAlignment = try container.decodeIfPresent(String.self, forKey: .deviceProfileAlignment)
            schemaVersion = try container.decodeIfPresent(String.self, forKey: .schemaVersion)
            schemaPolicyViolationCount = try container.decodeIfPresent(Int.self, forKey: .schemaPolicyViolationCount)
            schemaPolicyViolations = try container.decodeIfPresent([String].self, forKey: .schemaPolicyViolations) ?? []
            notes = try container.decodeIfPresent([String].self, forKey: .notes) ?? []
        }
    }

    let generatedAt: Date
    let fixtures: Fixtures
    let pipeline: Pipeline
    let regenSLA: RegenSLA
    let alerts: [Alert]
    let acceleration: AccelerationSummary?
    let telemetry: Telemetry?

    private enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case fixtures
        case pipeline
        case regenSLA = "regen_sla"
        case acceleration
        case telemetry
        case alerts
    }
}

struct AccelerationSummary: Decodable {
    struct Backend: Decodable {
        let enabled: Bool
        let parity: String?
        let perfDeltaPct: Double?

        private enum CodingKeys: String, CodingKey {
            case enabled
            case parity
            case perfDeltaPct = "perf_delta_pct"
        }
    }

    let metal: Backend?
    let neon: Backend?
    let strongbox: Backend?
}

// MARK: - Helpers

struct Options {
    let inputPath: String
    let outputPath: String?
}

func loadDashboard(path: String) throws -> ParityDashboard {
    let url = URL(fileURLWithPath: path)
    let data = try Data(contentsOf: url)
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .iso8601
    return try decoder.decode(ParityDashboard.self, from: data)
}

func format(date: Date?, relativeTo reference: Date) -> String {
    guard let date else { return "n/a" }
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .full
    return formatter.localizedString(for: date, relativeTo: reference)
}

func renderSummary(_ dashboard: ParityDashboard) -> String {
    let now = dashboard.generatedAt
    var lines: [String] = []
    lines.append("Swift Norito Parity Dashboard")
    lines.append("Generated: \(ISO8601DateFormatter().string(from: now))")
    lines.append("")

    lines.append("Fixtures")
    lines.append("  Outstanding diffs: \(dashboard.fixtures.outstandingDiffs)")
    lines.append("  Oldest diff age: \(String(format: "%.1f", dashboard.fixtures.oldestDiffHours))h")
    if dashboard.fixtures.details.isEmpty {
        lines.append("  Details: none")
    } else {
        for detail in dashboard.fixtures.details {
            lines.append("  - \(detail.instruction) | \(String(format: "%.1f", detail.ageHours))h | owner: \(detail.owner)")
        }
    }
    lines.append("")

    lines.append("Pipeline")
    let lastRunString = format(date: dashboard.pipeline.lastRun, relativeTo: now)
    lines.append("  Status: \(dashboard.pipeline.status)")
    lines.append("  Last run: \(lastRunString)")
    if dashboard.pipeline.failedTests.isEmpty {
        lines.append("  Failed tests: none")
    } else {
        lines.append("  Failed tests:")
        dashboard.pipeline.failedTests.forEach { lines.append("    - \($0)") }
    }
    if let metadata = dashboard.pipeline.metadata {
        lines.append("  Metadata:")
        if let jobName = metadata.jobName {
            lines.append("    Job: \(jobName)")
        }
        if let duration = metadata.durationSeconds {
            let formatted = String(format: "%.1f", duration)
            lines.append("    Duration: \(formatted)s")
        }
        if let tests = metadata.tests, !tests.isEmpty {
            lines.append("    Tests:")
            tests.forEach { test in
                let formatted = String(format: "%.1f", test.durationSeconds)
                lines.append("      - \(test.name): \(formatted)s")
            }
        }
    }
    if let source = dashboard.pipeline.metadataSource {
        lines.append("  Metadata source: \(source)")
    }
    lines.append("")

    lines.append("Regen SLA")
    let lastSuccessString = format(date: dashboard.regenSLA.lastSuccess, relativeTo: now)
    lines.append("  Last success: \(lastSuccessString)")
    lines.append("  Hours since success: \(String(format: "%.1f", dashboard.regenSLA.hoursSinceSuccess))h")
    let breachText = dashboard.regenSLA.breach ? "yes" : "no"
    lines.append("  SLA breach: \(breachText)")
    lines.append("")

    lines.append("Alerts")
    if dashboard.alerts.isEmpty {
        lines.append("  (none)")
    } else {
        dashboard.alerts.forEach { alert in
            if let severity = alert.severity {
                lines.append("  - [\(severity.uppercased())] \(alert.message)")
            } else {
                lines.append("  - \(alert.message)")
            }
        }
    }

    if let accel = dashboard.acceleration {
        lines.append("")
        lines.append("Acceleration")
        func describe(_ label: String, backend: AccelerationSummary.Backend?) {
            guard let backend else {
                lines.append("  \(label): n/a")
                return
            }
            let parity = backend.parity ?? "unknown"
            if let delta = backend.perfDeltaPct {
                let formatted = String(format: "%.1f%%", delta)
                lines.append("  \(label): enabled=\(backend.enabled) parity=\(parity) perf_delta=\(formatted)")
            } else {
                lines.append("  \(label): enabled=\(backend.enabled) parity=\(parity)")
            }
        }
        describe("Metal", backend: accel.metal)
        describe("NEON", backend: accel.neon)
        describe("StrongBox", backend: accel.strongbox)
    }

    if let telemetry = dashboard.telemetry {
        lines.append("")
        lines.append("Telemetry")
        let saltEpoch = telemetry.saltEpoch ?? "n/a"
        let rotation = telemetry.saltRotationAgeHours.map { String(format: "%.1f", $0) + "h" } ?? "n/a"
        let overrides = telemetry.overridesOpen.map(String.init) ?? "n/a"
        let alignment = telemetry.deviceProfileAlignment ?? "n/a"
        let schema = telemetry.schemaVersion ?? "n/a"
        let violations = telemetry.schemaPolicyViolationCount ?? telemetry.schemaPolicyViolations.count
        lines.append("  Salt epoch: \(saltEpoch)")
        lines.append("  Rotation age: \(rotation)")
        lines.append("  Overrides open: \(overrides)")
        lines.append("  Device profile alignment: \(alignment)")
        lines.append("  Schema version: \(schema)")
        lines.append("  Schema policy violations: \(violations)")
        if !telemetry.schemaPolicyViolations.isEmpty {
            lines.append("    Violations:")
            telemetry.schemaPolicyViolations.forEach { lines.append("      - \($0)") }
        }
        if !telemetry.notes.isEmpty {
            lines.append("  Notes:")
            telemetry.notes.forEach { lines.append("    - \($0)") }
        }
    }

    return lines.joined(separator: "\n")
}

func usageAndExit(defaultPath: String) -> Never {
    let command = URL(fileURLWithPath: CommandLine.arguments.first ?? "mobile_parity.swift").lastPathComponent
    fputs("usage: \(command) [--output <path>] [dashboard.json]\n", stderr)
    fputs("       defaults to \(defaultPath) when no input is provided\n", stderr)
    exit(1)
}

func parseOptions(defaultPath: String) -> Options {
    var args = Array(CommandLine.arguments.dropFirst())
    var outputPath: String?
    var inputPath: String?

    while !args.isEmpty {
        let arg = args.removeFirst()
        switch arg {
        case "--output", "-o":
            guard let next = args.first else {
                usageAndExit(defaultPath: defaultPath)
            }
            outputPath = next
            args.removeFirst()
        case "--help", "-h":
            usageAndExit(defaultPath: defaultPath)
        default:
            inputPath = arg
        }
    }

    return Options(inputPath: inputPath ?? defaultPath, outputPath: outputPath)
}

// MARK: - Main

let defaultPath = "dashboards/data/mobile_parity.sample.json"
let options = parseOptions(defaultPath: defaultPath)

do {
    let dashboard = try loadDashboard(path: options.inputPath)
    let summary = renderSummary(dashboard)
    if let output = options.outputPath {
        try (summary + "\n").write(toFile: output, atomically: true, encoding: .utf8)
    } else {
        print(summary)
    }
} catch {
    fputs("Failed to load dashboard data: \(error)\n", stderr)
    exit(1)
}
