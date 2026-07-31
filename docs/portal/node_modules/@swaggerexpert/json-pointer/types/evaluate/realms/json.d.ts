import type { EvaluationRealm } from '../../index.ts';

export type JSONPrimitive = string | number | boolean | null;

export interface JSONObject {
  [key: string]: JSONValue;
}

export interface JSONArray extends Array<JSONValue> { }

export type JSONValue = JSONPrimitive | JSONObject | JSONArray;

declare class JSONEvaluationRealm extends EvaluationRealm {
  public readonly name: 'json';

  public isArray(node: unknown): node is JSONArray;
  public isObject(node: unknown): node is JSONObject;
  public sizeOf(node: unknown): number;
  public has(node: unknown, referenceToken: string): boolean;
  public evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

export default JSONEvaluationRealm;
