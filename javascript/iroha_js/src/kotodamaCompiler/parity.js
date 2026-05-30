export function normalizeKotodamaParitySource(source) {
    return source.replace(/\r\n/g, '\n');
}
export function normalizeKotodamaParitySourcePath(sourcePath) {
    if (!sourcePath)
        return null;
    return sourcePath.replace(/\\/g, '/').replace(/^.*\/(?:iroha)\//, '');
}
export function normalizeKotodamaParityCompilerStderr(stderr) {
    return stderr
        .replace(/\r\n/g, '\n')
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean);
}
export function normalizeKotodamaParityDiagnostic(diagnostic) {
    const normalizedMessage = diagnostic.message.trim().replace(/`([^`]+)`/g, '\'$1\'');
    const locationMatch = normalizedMessage.match(/^(.*?)(?:\s+at\s+(\d+):(\d+))?\.?$/);
    const message = locationMatch?.[1]
        ?.replace(/^(?:compile error:\s*)?(?:parser error:\s*)?/i, '')
        .trim()
        .replace(/\.$/, '') ?? normalizedMessage;
    const line = diagnostic.line ?? (locationMatch?.[2] ? Number.parseInt(locationMatch[2], 10) : undefined);
    const column = diagnostic.column ?? (locationMatch?.[3] ? Number.parseInt(locationMatch[3], 10) : undefined);
    return {
        severity: diagnostic.severity,
        message,
        line,
        column,
        code: diagnostic.code ?? null,
        phase: diagnostic.phase ?? null,
    };
}
export function createKotodamaParityDiagnostic(input) {
    return {
        severity: input.severity ?? 'error',
        message: input.message,
        line: input.line,
        column: input.column,
        code: input.code ?? null,
        phase: input.phase ?? null,
    };
}
