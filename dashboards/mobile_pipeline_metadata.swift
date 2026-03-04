#!/usr/bin/env swift
//
// dashboards/mobile_pipeline_metadata.swift
//
// Renders a concise summary of the shared Swift pipeline metadata feed.
// The feed captures job/test duration data that parity dashboards share
// with other SDKs. See docs/source/references/ios_metrics.md for schema
// notes and dashboards/data/mobile_pipeline_metadata.sample.json for a sample.

import Foundation

struct Options {
    let inputPath: String
    let outputPath: String?
}

struct PipelineMetadata: Decodable {
    struct Test: Decodable {
        let name: String
        let durationSeconds: Double

        private enum CodingKeys: String, CodingKey {
            case name
            case durationSeconds = "duration_seconds"
        }
    }

    let jobName: String
    let durationSeconds: Double
    let tests: [Test]
    let buildkiteURL: String?
    let notes: String?

    private enum CodingKeys: String, CodingKey {
        case jobName = "job_name"
        case durationSeconds = "duration_seconds"
        case tests
        case buildkiteURL = "buildkite_url"
        case notes
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        jobName = try container.decode(String.self, forKey: .jobName)
        durationSeconds = try container.decode(Double.self, forKey: .durationSeconds)
        tests = try container.decodeIfPresent([Test].self, forKey: .tests) ?? []
        buildkiteURL = try container.decodeIfPresent(String.self, forKey: .buildkiteURL)
        notes = try container.decodeIfPresent(String.self, forKey: .notes)
    }
}

func loadMetadata(from path: String) throws -> PipelineMetadata {
    let url = URL(fileURLWithPath: path)
    let data = try Data(contentsOf: url)
    let decoder = JSONDecoder()
    return try decoder.decode(PipelineMetadata.self, from: data)
}

func formatDuration(seconds: Double) -> String {
    let minutes = seconds / 60.0
    return String(format: "%.1fs (%.1f min)", seconds, minutes)
}

func render(metadata: PipelineMetadata) -> String {
    var lines: [String] = []
    lines.append("Swift Pipeline Metadata Summary")
    lines.append("Job: \(metadata.jobName)")
    lines.append("Total duration: \(formatDuration(seconds: metadata.durationSeconds))")
    if let url = metadata.buildkiteURL, !url.isEmpty {
        lines.append("Buildkite: \(url)")
    }
    if let notes = metadata.notes, !notes.isEmpty {
        lines.append("Notes: \(notes)")
    }
    lines.append("")
    if metadata.tests.isEmpty {
        lines.append("Tests: (none reported)")
        return lines.joined(separator: "\n")
    }
    lines.append("Tests:")
    for test in metadata.tests {
        lines.append("  - \(test.name): \(formatDuration(seconds: test.durationSeconds))")
    }
    return lines.joined(separator: "\n")
}

func usageAndExit(defaultPath: String) -> Never {
    let command = URL(fileURLWithPath: CommandLine.arguments.first ?? "mobile_pipeline_metadata.swift").lastPathComponent
    fputs("usage: \(command) [--output <path>] [pipeline_metadata.json]\n", stderr)
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

let defaultPath = "dashboards/data/mobile_pipeline_metadata.sample.json"
let options = parseOptions(defaultPath: defaultPath)

do {
    let metadata = try loadMetadata(from: options.inputPath)
    let summary = render(metadata: metadata)
    if let output = options.outputPath {
        try (summary + "\n").write(toFile: output, atomically: true, encoding: .utf8)
    } else {
        print(summary)
    }
} catch {
    fputs("error: \(error)\n", stderr)
    exit(2)
}
