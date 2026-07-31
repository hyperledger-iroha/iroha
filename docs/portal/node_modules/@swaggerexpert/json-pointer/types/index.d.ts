export type JSONPointer = string;
export type URIFragmentJSONPointer = string;
export type StringifiedJSONPointer = string;
export type ReferenceToken = string;
export type EscapedReferenceToken = string;
export type UnescapedReferenceToken = string;

type Digit1To9 = "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9";
type Digit0To9 = "0" | Digit1To9;
type NumberString<T extends string = ""> = `${Digit0To9}${T}` | T;
type NonZeroNumber = `${Digit1To9}${NumberString}`;
export type ArrayIndex = "0" | NonZeroNumber;
export type ArrayDash = '-';
export type ArrayLocation = ArrayIndex | ArrayDash;

/**
 * Parsing
 */
export function parse(jsonPointer: JSONPointer, options?: ParseOptions): ParseResult;

export interface ParseOptions {
  readonly stats?: boolean;
  readonly trace?: boolean;
  readonly translator?: Translator | null;
}

export interface Translator<TTree = ASTTranslator> {
  getTree(): TTree;
}
export declare class CSTTranslator implements Translator<CSTTree> {
  getTree(): CSTTree;
}
export declare class ASTTranslator implements Translator<ASTTree> {
  getTree(): ASTTree;
}
export declare class XMLTranslator implements Translator<XMLTree> {
  getTree(): XMLTree;
}

export interface CSTNode {
  readonly type: 'json-pointer' | 'reference-token' | 'slash',
  readonly text: string,
  readonly start: number,
  readonly length: number,
  readonly children: CSTNode[],
}

export interface CSTTree {
  readonly root: CSTNode;
}

export type ASTTree = UnescapedReferenceToken[]

export type XMLTree = string;

export interface ParseResult<TTree = ASTTree> {
  readonly result: {
    readonly success: boolean;
    readonly state: number;
    readonly stateName: string;
    readonly length: number;
    readonly matched: number;
    readonly maxMatched: number;
    readonly maxTreeDepth: number
    readonly nodeHits: number;
  };
  readonly tree?: TTree;
  readonly stats?: Stats;
  readonly trace?: Trace;
}

export interface Stats {
  displayStats(): string;
  displayHits(): string;
}

export interface Trace {
  displayTrace(): string;
  inferExpectations(): string[];
}


/**
 * Testing
 */
export function testJSONPointer(jsonPointer: string): jsonPointer is JSONPointer;
export function testReferenceToken(referenceToken: string): referenceToken is ReferenceToken;
export function testArrayLocation(referenceToken: string): referenceToken is ArrayLocation;
export function testArrayIndex(referenceToken: string): referenceToken is ArrayIndex;
export function testArrayDash(referenceToken: string): referenceToken is ArrayDash;

/**
 * Escaping
 */
export function escape(referenceToken: UnescapedReferenceToken): EscapedReferenceToken;
export function unescape(referenceToken: EscapedReferenceToken): UnescapedReferenceToken;

/**
 * Compiling
 */
export function compile(referenceTokens: readonly UnescapedReferenceToken[]): JSONPointer;

/**
 * Evaluating
 */
export function evaluate<T = unknown>(value: unknown, jsonPointer: JSONPointer, options?: EvaluationOptions): T;
export function composeRealms(...realms: EvaluationRealm[]): EvaluationRealm;

export interface EvaluationOptions {
  strictArrays?: boolean;
  strictObjects?: boolean;
  realm?: EvaluationRealm;
  trace?: boolean | Partial<EvaluationTrace>;
}

export declare abstract class EvaluationRealm {
  public abstract readonly name: string;

  public abstract isArray(node: unknown): boolean;
  public abstract isObject(node: unknown): boolean;
  public abstract sizeOf(node: unknown): number;
  public abstract has(node: unknown, referenceToken: string): boolean;
  public abstract evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

/* Tracing start */
export interface EvaluationTrace {
  steps: EvaluationStep[];
  failed: boolean;
  failedAt: number;
  message?: string;
  context: TraceContext;
}
export interface EvaluationStep {
  referenceToken: string;
  referenceTokenPosition: number;
  input: unknown;
  inputType: 'object' | 'array' | 'unrecognized';
  output: unknown;
  success: boolean;
}
export interface TraceContext {
  jsonPointer: string;
  referenceTokens: string[];
  strictArrays: boolean;
  strictObjects: boolean;
  realm: string;
  value: unknown;
}
/* Tracking end */

/**
 * Representing
 */
export interface JSONString {
  to(jsonPointer: JSONPointer): StringifiedJSONPointer;
  from(jsonPointer: StringifiedJSONPointer): JSONPointer
}

export interface URIFragmentIdentifier {
  to(jsonPointer: JSONPointer): URIFragmentJSONPointer;
  from(jsonPointer: URIFragmentJSONPointer): JSONPointer
  fromURIReference(uriReference: string): JSONPointer;
}

export declare const JSONString: JSONString;
export declare const URIFragmentIdentifier: URIFragmentIdentifier;

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

/**
 * Errors
 */
export interface JSONPointerErrorOptions {
  readonly cause?: unknown;
  readonly [key: string]: unknown;
}

export declare class JSONPointerError extends Error {
  constructor(message?: string, options?: JSONPointerErrorOptions);
}
export declare class JSONPointerParseError extends JSONPointerError {
  jsonPointer?: JSONPointer;
}
export declare class JSONPointerCompileError extends JSONPointerError {
  referenceTokens: UnescapedReferenceToken[];
}
export declare class JSONPointerEvaluateError extends JSONPointerError {
  jsonPointer?: JSONPointer;
  referenceTokens?: UnescapedReferenceToken[];
  referenceToken?: UnescapedReferenceToken;
  referenceTokenPosition?: number;
  realm?: string;
  currentValue?: unknown;
}
export declare class JSONPointerTypeError extends JSONPointerEvaluateError { }
export declare class JSONPointerKeyError extends JSONPointerEvaluateError { }
export declare class JSONPointerIndexError extends JSONPointerEvaluateError { }
