type PathTemplate = string;
type ResolvedPath = string;

/**
 * Parsing
 */
export function parse(pathTemplate: PathTemplate): ParseResult;

interface ParseResult {
  readonly result: {
    readonly success: boolean;
  }
  readonly ast: {
    readonly translate: (parts: any[]) => Array<[string, string]>;
    readonly toXml: () => string;
  };
}

/**
 * Validation
 */
export function test(pathTemplate: PathTemplate, options?: TestOptions): boolean;

interface TestOptions {
  readonly strict?: boolean;
}

/**
 * Resolving
 */
export function resolve(pathTemplate: PathTemplate, parameters: Parameters, options?: ResolveOptions): ResolvedPath;
export function encodePathComponent(parameterValue: string): string;

interface Parameters {
  [key: string]: any;
}

interface ResolveOptions {
  encoder: (paramterValue: string, parameterName?: string) => string;
}

/**
 * Equivalence
 */
export function isIdentical(pathTemplate1: PathTemplate, pathTemplate2: PathTemplate, options?: EquivalenceOptions): boolean;

export interface EquivalenceOptions {
  normalizer?: Normalizer;
}

/**
 * Normalization
 */
export function normalize(pathTemplate: PathTemplate): PathTemplate;
export const identityNormalizer: Normalizer;
export const caseNormalizer: Normalizer;
export const percentEncodingNormalizer: Normalizer;
export const pathSegmentNormalizer: Normalizer;

export interface Normalizer {
  (pathTemplate: PathTemplate): PathTemplate;
}

/**
 * Grammar
 */

export function Grammar(): Grammar;

export interface Grammar {
  grammarObject: string; // Internal identifier
  rules: Rule[]; // List of grammar rules
  udts: UDT[]; // User-defined terminals (empty in this grammar)
  toString(): string; // Method to return the grammar in ABNF format
}

export interface Rule {
  name: string; // Rule name
  lower: string; // Lowercased rule name
  index: number; // Rule index
  isBkr: boolean; // Is this a back-reference?
  opcodes?: Opcode[]; // List of opcodes for the rule
}

export type Opcode =
  | { type: 1; children: number[] } // ALT (alternation)
  | { type: 2; children: number[] } // CAT (concatenation)
  | { type: 3; min: number; max: number } // REP (repetition)
  | { type: 4; index: number } // RNM (rule reference)
  | { type: 5; min: number; max: number } // TRG (terminal range)
  | { type: 6 | 7; string: number[] }; // TBS or TLS (byte sequence or literal string)

export type UDT = {}; // User-defined terminals (empty in this grammar)
