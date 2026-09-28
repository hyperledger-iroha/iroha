import Foundation

public extension AccelerationSettings {
    /// Decode a direct acceleration policy record. Counts are unsigned; zero is explicit.
    static func fromJSON(_ data: Data, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        guard let policy = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              policy.keys.allSatisfy(accelerationKeys.contains) else {
            throw invalidConfiguration("expected canonical acceleration policy fields")
        }
        if let raw = policy["resource_limits"] {
            guard let resources = raw as? [String: Any], resources.keys.allSatisfy(resourceKeys.contains) else {
                throw invalidConfiguration("resource_limits must contain canonical resource fields")
            }
        }
        decoder.keyDecodingStrategy = .useDefaultKeys
        return try decoder.decode(Self.self, from: data)
    }

    /// Load a direct acceleration policy record from a file.
    static func fromJSONFile(at url: URL, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        try fromJSON(Data(contentsOf: url), decoder: decoder)
    }

    /// Read the canonical root `accel` section of an `iroha_config` document.
    /// TOML resource ceilings use `[accel.resource_limits]`. An absent section uses defaults.
    static func fromIrohaConfig(_ data: Data, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        if let value = try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed) {
            guard let document = value as? [String: Any] else {
                throw invalidConfiguration("configuration must be an object")
            }
            guard document["acceleration"] == nil else {
                throw invalidConfiguration("use the canonical accel section")
            }
            guard let section = document["accel"] else {
                guard !document.keys.contains(where: accelerationKeys.contains) else {
                    throw invalidConfiguration("acceleration policy belongs under accel")
                }
                return Self()
            }
            guard let policy = section as? [String: Any] else {
                throw invalidConfiguration("accel must be a configuration object")
            }
            return try decodePolicy(policy, decoder: decoder)
        }
        guard let text = String(data: data, encoding: .utf8) else {
            throw invalidConfiguration("configuration must use UTF-8")
        }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("{") else {
            throw invalidConfiguration("malformed JSON configuration")
        }
        return try decodePolicy(parseToml(text), decoder: decoder)
    }

    /// Read a canonical node configuration file supplied by the caller.
    static func fromIrohaConfigFile(at url: URL, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        try fromIrohaConfig(Data(contentsOf: url), decoder: decoder)
    }
}

private extension AccelerationSettings {
    static let accelerationKeys: Set<String> = [
        "enable_simd", "enable_metal", "enable_cuda", "max_gpus",
        "merkle_min_leaves_gpu", "merkle_min_leaves_metal", "merkle_min_leaves_cuda",
        "prefer_cpu_sha2_max_leaves_aarch64", "prefer_cpu_sha2_max_leaves_x86", "resource_limits"
    ]
    static let resourceKeys: Set<String> = [
        "host_bytes", "pinned_bytes", "device_bytes", "in_flight", "metadata_bytes",
        "observed_devices", "discovery_ordinals", "modules", "streams", "artifact_bytes"
    ]

    static func invalidConfiguration(_ description: String) -> DecodingError {
        .dataCorrupted(.init(codingPath: [], debugDescription: description))
    }

    static func decodePolicy(_ policy: [String: Any], decoder: JSONDecoder) throws -> AccelerationSettings {
        guard policy.keys.allSatisfy(accelerationKeys.contains) else {
            throw invalidConfiguration("unknown accel configuration field")
        }
        if let raw = policy["resource_limits"] {
            guard let resources = raw as? [String: Any], resources.keys.allSatisfy(resourceKeys.contains) else {
                throw invalidConfiguration("resource_limits must contain canonical resource fields")
            }
        }
        return try fromJSON(JSONSerialization.data(withJSONObject: policy), decoder: decoder)
    }

    static func parseUnsignedInteger(_ raw: String) -> UInt64? {
        let syntax: (String, Int, Int)
        if raw.hasPrefix("0x") { syntax = (#"^0x[0-9a-fA-F]+(?:_[0-9a-fA-F]+)*$"#, 16, 2) }
        else if raw.hasPrefix("0o") { syntax = (#"^0o[0-7]+(?:_[0-7]+)*$"#, 8, 2) }
        else if raw.hasPrefix("0b") { syntax = (#"^0b[01]+(?:_[01]+)*$"#, 2, 2) }
        else { syntax = (#"^\+?(?:0|[1-9][0-9]*(?:_[0-9]+)*)$"#, 10, 0) }
        guard raw.range(of: syntax.0, options: .regularExpression) != nil else { return nil }
        return UInt64(raw.dropFirst(syntax.2).replacingOccurrences(of: "_", with: ""), radix: syntax.1)
    }

    // This projection reads scalar policy tables, rather than implementing a
    // second full TOML decoder. Known unsupported policy spellings must fail.
    static func parseToml(_ text: String) throws -> [String: Any] {
        enum Section { case root, unrelated, acceleration, resources }
        var section = Section.root
        var policy: [String: Any] = [:]
        var resources: [String: Any] = [:]
        var seenTables: Set<String> = []
        var multilineQuote: Character?
        for rawLine in text.split(separator: "\n", omittingEmptySubsequences: false) {
            guard let content = try tomlLineContent(String(rawLine), multilineQuote: &multilineQuote) else { continue }
            let line = content.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty { continue }
            if line.hasPrefix("[") {
                let arrayTable = line.hasPrefix("[[")
                let delimiterLength = arrayTable ? 2 : 1
                let suffix = arrayTable ? "]]" : "]"
                guard line.hasSuffix(suffix), line.count >= delimiterLength * 2 else {
                    throw invalidConfiguration("malformed TOML table header")
                }
                let path = try tomlKeyPath(String(line.dropFirst(delimiterLength).dropLast(delimiterLength)))
                guard path.first != "acceleration" else {
                    throw invalidConfiguration("use the canonical accel section")
                }
                guard path.first == "accel" else {
                    section = .unrelated
                    continue
                }
                guard !arrayTable, path == ["accel"] || path == ["accel", "resource_limits"] else {
                    throw invalidConfiguration("malformed or unknown accel table")
                }
                guard seenTables.insert(path.joined(separator: ".")).inserted else {
                    throw invalidConfiguration("duplicate acceleration table")
                }
                section = path.count == 1 ? .acceleration : .resources
                continue
            }
            guard let equals = tomlAssignmentSeparator(line) else {
                if section == .acceleration || section == .resources {
                    throw invalidConfiguration("acceleration fields require key = value")
                }
                continue
            }
            let path = try tomlKeyPath(String(line[..<equals]))
            if section == .root {
                guard path.first != "accel", path.first != "acceleration" else {
                    throw invalidConfiguration("root dotted or inline acceleration policy is unsupported; use [accel] and [accel.resource_limits]")
                }
                guard !(path.count == 1 && accelerationKeys.contains(path[0])) else {
                    throw invalidConfiguration("acceleration policy belongs under accel")
                }
                continue
            }
            guard section != .unrelated else { continue }
            guard path.count == 1 else {
                throw invalidConfiguration("dotted acceleration fields are unsupported; use [accel.resource_limits]")
            }
            let key = path[0]
            let raw = line[line.index(after: equals)...].trimmingCharacters(in: .whitespacesAndNewlines)
            let validKeys = section == .acceleration ? accelerationKeys : resourceKeys
            guard validKeys.contains(key), key != "resource_limits" else {
                throw invalidConfiguration("unknown field or inline resource table; use [accel.resource_limits]")
            }
            let value: Any
            if raw == "true" { value = true }
            else if raw == "false" { value = false }
            else if let number = parseUnsignedInteger(raw) { value = number }
            else { throw invalidConfiguration("\(key) requires an unsigned integer or boolean") }
            if section == .resources {
                guard resources.updateValue(value, forKey: key) == nil else {
                    throw invalidConfiguration("duplicate resource limit \(key)")
                }
            } else {
                guard policy.updateValue(value, forKey: key) == nil else {
                    throw invalidConfiguration("duplicate acceleration field \(key)")
                }
            }
        }
        guard multilineQuote == nil else { throw invalidConfiguration("unterminated TOML multiline string") }
        if seenTables.contains("accel.resource_limits") { policy["resource_limits"] = resources }
        return policy
    }

    // Strip comments only outside strings. Skip unrelated multiline string bodies
    // so a policy-looking line inside a string cannot become active configuration.
    static func tomlLineContent(_ line: String, multilineQuote: inout Character?) throws -> String? {
        let characters = Array(line)
        let continuedString = multilineQuote != nil
        var result = ""
        var quote: Character?
        var cursor = 0
        while cursor < characters.count {
            let character = characters[cursor]
            if let delimiter = multilineQuote {
                if delimiter == "\"", character == "\\" {
                    cursor += min(2, characters.count - cursor)
                    continue
                }
                if cursor + 2 < characters.count,
                   characters[cursor] == delimiter,
                   characters[cursor + 1] == delimiter,
                   characters[cursor + 2] == delimiter {
                    multilineQuote = nil
                    cursor += 3
                } else {
                    cursor += 1
                }
                continue
            }
            if let delimiter = quote {
                result.append(character)
                if delimiter == "\"", character == "\\", cursor + 1 < characters.count {
                    cursor += 1
                    result.append(characters[cursor])
                } else if character == delimiter {
                    quote = nil
                }
            } else if character == "#" {
                break
            } else if character == "\"" || character == "'" {
                if cursor + 2 < characters.count,
                   characters[cursor + 1] == character,
                   characters[cursor + 2] == character {
                    result.append(contentsOf: [character, character, character])
                    multilineQuote = character
                    cursor += 3
                    continue
                }
                quote = character
                result.append(character)
            } else {
                result.append(character)
            }
            cursor += 1
        }
        guard quote == nil else { throw invalidConfiguration("unterminated TOML string") }
        return continuedString ? nil : result
    }

    static func tomlAssignmentSeparator(_ line: String) -> String.Index? {
        var quote: Character?
        var escaped = false
        for index in line.indices {
            let character = line[index]
            if escaped { escaped = false; continue }
            if let delimiter = quote {
                if delimiter == "\"", character == "\\" { escaped = true }
                else if character == delimiter { quote = nil }
            } else if character == "\"" || character == "'" {
                quote = character
            } else if character == "=" {
                return index
            }
        }
        return nil
    }

    // Decode bare, literal and basic quoted key components. A dot is a separator
    // only outside quotes; Unicode escapes in basic keys have TOML scalar rules.
    static func tomlKeyPath(_ text: String) throws -> [String] {
        let characters = Array(text)
        var cursor = 0
        var result: [String] = []
        func space(_ character: Character) -> Bool { character == " " || character == "\t" }
        func bare(_ character: Character) -> Bool {
            guard character.unicodeScalars.count == 1, let value = character.unicodeScalars.first?.value else { return false }
            return (48...57).contains(value) || (65...90).contains(value)
                || (97...122).contains(value) || value == 45 || value == 95
        }
        while cursor < characters.count {
            while cursor < characters.count, space(characters[cursor]) { cursor += 1 }
            guard cursor < characters.count else { throw invalidConfiguration("empty TOML key component") }
            var component = ""
            if characters[cursor] == "\"" || characters[cursor] == "'" {
                let delimiter = characters[cursor]
                cursor += 1
                var closed = false
                while cursor < characters.count {
                    let character = characters[cursor]
                    cursor += 1
                    if character == delimiter { closed = true; break }
                    if delimiter == "\"", character == "\\" {
                        guard cursor < characters.count else { throw invalidConfiguration("incomplete TOML key escape") }
                        let escape = characters[cursor]
                        cursor += 1
                        switch escape {
                        case "b": component.append("\u{8}")
                        case "t": component.append("\t")
                        case "n": component.append("\n")
                        case "f": component.append("\u{c}")
                        case "r": component.append("\r")
                        case "\"", "\\": component.append(escape)
                        case "u", "U":
                            let count = escape == "u" ? 4 : 8
                            guard cursor + count <= characters.count else { throw invalidConfiguration("incomplete TOML Unicode escape") }
                            let digits = String(characters[cursor..<(cursor + count)])
                            guard digits.range(of: "^[0-9a-fA-F]+$", options: .regularExpression) != nil,
                                  let value = UInt32(digits, radix: 16), let scalar = UnicodeScalar(value) else {
                                throw invalidConfiguration("invalid TOML Unicode scalar")
                            }
                            component.unicodeScalars.append(scalar)
                            cursor += count
                        default: throw invalidConfiguration("invalid TOML key escape")
                        }
                    } else {
                        guard character.unicodeScalars.allSatisfy({ $0.value >= 32 && $0.value != 127 }) else {
                            throw invalidConfiguration("invalid TOML quoted key character")
                        }
                        component.append(character)
                    }
                }
                guard closed else { throw invalidConfiguration("unterminated TOML key") }
            } else {
                while cursor < characters.count, bare(characters[cursor]) {
                    component.append(characters[cursor])
                    cursor += 1
                }
                guard !component.isEmpty else { throw invalidConfiguration("invalid TOML bare key") }
            }
            result.append(component)
            while cursor < characters.count, space(characters[cursor]) { cursor += 1 }
            if cursor == characters.count { return result }
            guard characters[cursor] == "." else { throw invalidConfiguration("TOML key components require dot separators") }
            cursor += 1
            guard cursor < characters.count else { throw invalidConfiguration("empty TOML key component") }
        }
        guard !result.isEmpty else { throw invalidConfiguration("empty TOML key path") }
        return result
    }
}
