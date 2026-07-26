#!/usr/bin/env swift
//
// dashboards/mobile_ci.swift
//
// Generates a summary of the Swift CI dashboard data. The script expects a JSON
// payload matching the schema described in docs/source/references/ios_metrics.md.
// Sample data is provided in dashboards/data/mobile_ci.sample.json.

import Foundation

// MARK: - Models

struct CIDashboard: Decodable {
    struct Lane: Decodable {
        let name: String
        let successRate14Runs: Double
        let lastFailure: Date?
        let flakeCount: Int
        let mttrHours: Double
        let deviceTag: String?

        private enum CodingKeys: String, CodingKey {
            case name
            case successRate14Runs = "success_rate_14_runs"
            case lastFailure = "last_failure"
            case flakeCount = "flake_count"
            case mttrHours = "mttr_hours"
            case deviceTag = "device_tag"
        }
    }

    struct Buildkite: Decodable {
        let lanes: [Lane]
        let queueDepth: Int
        let averageRuntimeMinutes: Double

        private enum CodingKeys: String, CodingKey {
            case lanes
            case queueDepth = "queue_depth"
            case averageRuntimeMinutes = "average_runtime_minutes"
        }
    }

    struct DeviceGroup: Decodable {
        let passes: Int
        let failures: Int
    }

    struct Devices: Decodable {
        let emulators: DeviceGroup
        let strongboxCapable: DeviceGroup

        private enum CodingKeys: String, CodingKey {
            case emulators
            case strongboxCapable = "strongbox_capable"
        }
    }

    struct AlertState: Decodable {
        let consecutiveFailures: Int
        let openIncidents: [String]

        private enum CodingKeys: String, CodingKey {
            case consecutiveFailures = "consecutive_failures"
            case openIncidents = "open_incidents"
        }
    }

    struct AccelerationBench: Decodable {
        struct MetalVSCPU: Decodable {
            let cpu: Double
            let metal: Double

            private enum CodingKeys: String, CodingKey {
                case cpu
                case metal
            }
        }

        let metalVsCpuMerkleMs: MetalVSCPU?
        let neonCrc64ThroughputMbS: Double?

        private enum CodingKeys: String, CodingKey {
            case metalVsCpuMerkleMs = "metal_vs_cpu_merkle_ms"
            case neonCrc64ThroughputMbS = "neon_crc64_throughput_mb_s"
        }
    }

    let generatedAt: Date
    let buildkite: Buildkite
    let devices: Devices
    let alertState: AlertState
    let accelerationBench: AccelerationBench?

    private enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case buildkite
        case devices
        case alertState = "alert_state"
        case accelerationBench = "acceleration_bench"
    }
}

// MARK: - Helpers

struct Options {
    let inputPath: String
    let outputPath: String?
}

func loadDashboard(path: String) throws -> CIDashboard {
    let url = URL(fileURLWithPath: path)
    let data = try Data(contentsOf: url)
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .iso8601
    return try decoder.decode(CIDashboard.self, from: data)
}

func format(date: Date?, relativeTo reference: Date) -> String {
    guard let date else { return "n/a" }
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .full
    return formatter.localizedString(for: date, relativeTo: reference)
}

func renderSummary(_ dashboard: CIDashboard) -> String {
    let now = dashboard.generatedAt
    let isoFormatter = ISO8601DateFormatter()
    var lines: [String] = []

    lines.append("Swift CI Dashboard")
    lines.append("Generated: \(isoFormatter.string(from: now))")
    lines.append("")

    lines.append("Buildkite")
    lines.append("  Queue depth: \(dashboard.buildkite.queueDepth)")
    lines.append("  Average runtime: \(String(format: "%.1f", dashboard.buildkite.averageRuntimeMinutes)) minutes")
    if dashboard.buildkite.lanes.isEmpty {
        lines.append("  Lanes: none")
    } else {
        for lane in dashboard.buildkite.lanes {
            let successRate = String(format: "%.1f", lane.successRate14Runs * 100)
            let lastFailure = format(date: lane.lastFailure, relativeTo: now)
            lines.append("  - \(lane.name)")
            if let tag = lane.deviceTag {
                lines.append("      Device tag: \(tag)")
            }
            lines.append("      Success rate (14 runs): \(successRate)%")
            lines.append("      Last failure: \(lastFailure)")
            lines.append("      Flake count: \(lane.flakeCount)")
            lines.append("      MTTR: \(String(format: "%.1f", lane.mttrHours)) hours")
        }
    }
    lines.append("")

    lines.append("Device Coverage (last window)")
    lines.append("  Emulators: \(dashboard.devices.emulators.passes) pass / \(dashboard.devices.emulators.failures) fail")
    lines.append("  StrongBox capable: \(dashboard.devices.strongboxCapable.passes) pass / \(dashboard.devices.strongboxCapable.failures) fail")
    lines.append("")

    lines.append("Alerts")
    lines.append("  Consecutive failures: \(dashboard.alertState.consecutiveFailures)")
    if dashboard.alertState.openIncidents.isEmpty {
        lines.append("  Open incidents: none")
    } else {
        dashboard.alertState.openIncidents.forEach { incident in
            lines.append("  - \(incident)")
        }
    }

    if let bench = dashboard.accelerationBench {
        lines.append("")
        lines.append("Acceleration Benchmarks")
        if let metal = bench.metalVsCpuMerkleMs {
            let ratio = metal.cpu / max(metal.metal, 0.0001)
            let cpu = String(format: "%.2f ms", metal.cpu)
            let metalMs = String(format: "%.2f ms", metal.metal)
            let speedup = String(format: "%.2fx", ratio)
            lines.append("  Merkle (Metal vs CPU): CPU=\(cpu) Metal=\(metalMs) speedup=\(speedup)")
        }
        if let throughput = bench.neonCrc64ThroughputMbS {
            let formatted = String(format: "%.1f MB/s", throughput)
            lines.append("  NEON CRC64 throughput: \(formatted)")
        }
    }

    return lines.joined(separator: "\n")
}

func usageAndExit(defaultPath: String) -> Never {
    let command = URL(fileURLWithPath: CommandLine.arguments.first ?? "mobile_ci.swift").lastPathComponent
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

let defaultPath = "dashboards/data/mobile_ci.sample.json"
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
