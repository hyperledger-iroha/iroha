// Fixed SoraFS Node24 TestsStream relation. This is not execution authority.
// The fixed child must supply actual events and retain original input/native
// owners through stream EOF; signed producer/index joins remain separate.
import { createHash } from "node:crypto";
import { isAbsolute, join, normalize } from "node:path";

const CONTRACT_SHA256 = "dcd87081a7f214eb6c144a6a72b95627220dd2447f83aa35d0979f76bf6fa7bf";
const MAX_EVENTS = 1024;
const MAX_TEXT_BYTES = 1024 * 1024;
const BUNDLE_SUITE = "sorafsFixtureBundleValidation";
const BUNDLE_PARENT = "fixture-bundle wrapper matches all nine release-wide outcome goldens byte-for-byte";
const COUNTS = Object.freeze({ tests: 55, failed: 0, passed: 55, cancelled: 0,
  skipped: 0, todo: 0, topLevel: 46, suites: 0 });
const DIAGNOSTICS = Object.freeze(["tests 55", "suites 0", "pass 55", "fail 0",
  "cancelled 0", "skipped 0", "todo 0"]);
const COMMON = ["nesting", "name", "testId", "parentId", "tags", "line", "column", "file"];

function demand(condition, message) {
  if (!condition) throw new Error(`SoraFS JavaScript test events: ${message}`);
}
function own(value) {
  demand(value !== null && typeof value === "object" && !Array.isArray(value)
    && [Object.prototype, null].includes(Object.getPrototypeOf(value)), "expected a plain event object");
  const keys = Reflect.ownKeys(value);
  demand(keys.length <= 24 && keys.every((key) => typeof key === "string" && key.length <= 64), "event field bound");
  const result = Object.create(null);
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    demand(Object.hasOwn(descriptor, "value"), "event accessors are not observations");
    result[key] = descriptor.value;
  }
  return result;
}
function fields(value, required, optional = []) {
  const row = own(value);
  demand(required.every((key) => Object.hasOwn(row, key))
    && Object.keys(row).every((key) => required.includes(key) || optional.includes(key)), "event fields differ");
  return row;
}
function integer(value, minimum = 0) {
  demand(Number.isSafeInteger(value) && !Object.is(value, -0) && value >= minimum, "event integer differs");
}
function duration(value) {
  demand(typeof value === "number" && Number.isFinite(value) && !Object.is(value, -0) && value >= 0, "event duration differs");
}
function expectedCases(raw, suiteRoot) {
  demand(Buffer.isBuffer(raw) && raw.length > 0 && raw.length <= 128 * 1024,
    "case contract byte bound");
  raw = Buffer.from(raw);
  demand(createHash("sha256").update(raw).digest("hex") === CONTRACT_SHA256,
    "case contract differs from the reviewed original fixture");
  demand(typeof suiteRoot === "string" && suiteRoot.length <= 4096 && !suiteRoot.includes("\0")
    && isAbsolute(suiteRoot) && normalize(suiteRoot) === suiteRoot, "suite root is not canonical");
  // The pinned fixture owns every name and callback identity. Its one nested
  // list has no explicit parent field; this fixed join binds it to the existing
  // canonical bundle case, not to a second caller-provided case inventory.
  const contract = JSON.parse(raw.toString("utf8"));
  const cases = [], top = [];
  for (const suite of contract.suites) {
    for (const item of suite.cases) {
      const row = { suite: suite.name, path: [item.name], name: item.name,
        file: join(suiteRoot, suite.name + ".js"), nesting: 0, number: top.length + 1,
        stages: new Set() };
      cases.push(row); top.push(row);
      if (suite.name === BUNDLE_SUITE && item.name === BUNDLE_PARENT) {
        row.children = suite.nested_case_names.map((name, index) => ({
          suite: suite.name, path: [item.name, name], name, file: row.file,
          nesting: 1, number: index + 1, parent: row, stages: new Set(),
        }));
        cases.push(...row.children);
      }
    }
  }
  demand(contract.schema === "sorafs.javascript.shared_assertion_contract.v1"
    && cases.length === 55 && top.length === 46, "fixed fixture inventory differs");
  return { cases, top, parent: top.find((row) => row.children) };
}

/** One poisoned-on-refusal owner for the fixed Node24 structured event stream.
 * No test, file, native addon or runtime is executed/authenticated by this class.
 * finish must be called only after the actual original TestsStream reaches EOF.
 */
export class SorafsJavascriptTestEvents {
  #cases; #top; #parent; #ids = new Map(); #nextTop = 0; #nextNested = 0;
  #events = 0; #textBytes = 0; #diagnostics = 0; #diagnosticDuration;
  #nextTopExecution = 0; #nextNestedExecution = 0;
  #nestedPlan = false; #rootPlan = false; #summary; #failed = false; #finished = false; #active = false;

  constructor(contractBytes, { suiteRoot }) {
    const expected = expectedCases(contractBytes, suiteRoot);
    this.#cases = expected.cases; this.#top = expected.top; this.#parent = expected.parent;
  }

  #operation(action) {
    if (this.#active) this.#failed = true;
    demand(!this.#failed && !this.#finished, "event owner is already refused or consumed");
    this.#active = true;
    try {
      const result = action();
      demand(!this.#failed, "event owner was refused during observation");
      return result;
    } catch (error) {
      this.#failed = true;
      throw error;
    } finally {
      this.#active = false;
    }
  }

  #snapshot(value, depth = 0) {
    demand(depth <= 4, "event nesting bound");
    if (typeof value === "string") {
      demand(value.length <= 4096, "event text bound");
      this.#textBytes += Buffer.byteLength(value);
      demand(this.#textBytes <= MAX_TEXT_BYTES, "aggregate event text bound");
    } else if (Array.isArray(value)) {
      demand(value.length === 0 && Reflect.ownKeys(value).length === 1, "unexpected event array");
    } else if (value !== null && typeof value === "object") {
      const result = Object.create(null);
      for (const [key, child] of Object.entries(own(value))) {
        this.#snapshot(key, depth + 1); result[key] = this.#snapshot(child, depth + 1);
      }
      return result;
    } else demand(value === null || ["number", "boolean"].includes(typeof value), "event value type");
    return Array.isArray(value) ? [] : value;
  }

  /** Consume one actual {type,data} item without changing test runner behavior. */
  accept(event) {
    return this.#operation(() => {
      demand(++this.#events <= MAX_EVENTS && !this.#summary, "event count or event after summary");
      const item = fields(event, ["type", "data"]);
      const type = item.type;
      demand(typeof type === "string" && type.length <= 64, "event type bound");
      demand(type !== "test:fail", "test, registration or final hook failed");
      const data = this.#snapshot(item.data);
      if (type === "test:plan") return this.#plan(data);
      if (type === "test:diagnostic") return this.#diagnostic(data);
      if (type === "test:summary") return this.#end(data);
      demand(["test:enqueue", "test:dequeue", "test:start", "test:complete", "test:pass"].includes(type), "unknown event type");
      demand(!this.#rootPlan && this.#diagnostics === 0, "case event after final plan");
      this.#case(type, data);
    });
  }

  #case(type, value) {
    const terminal = ["test:complete", "test:pass"].includes(type);
    const queue = ["test:enqueue", "test:dequeue"].includes(type);
    const row = fields(value, [...COMMON, ...(queue ? ["type"] : []),
      ...(terminal ? ["testNumber", "details"] : [])], type === "test:pass" ? ["classname"] : []);
    demand(row.tags.length === 0 && Array.isArray(row.tags), "tagged test is not the fixed case");
    for (const key of ["testId", "line", "column"]) integer(row[key], 1);
    integer(row.parentId); integer(row.nesting);
    demand(!queue || row.type === "test", "suite substitution");
    const test = this.#cases.find((item) => item.file === row.file && item.name === row.name && item.nesting === row.nesting);
    demand(test && !test.stages.has(type), "missing, extra, renamed or duplicate case event");
    if (type === "test:enqueue") {
      demand(!this.#ids.has(row.testId), "testId reused by another original case");
      if (test.parent) {
        demand(test === test.parent.children[this.#nextNested]
          && (this.#nextNested === 0 || test.parent.children[this.#nextNested - 1].stages.has("test:pass"))
          && test.parent.stages.has("test:dequeue") && !test.parent.stages.has("test:complete"), "nested registration order/parent lifecycle");
        this.#nextNested += 1;
      } else demand(test === this.#top[this.#nextTop++], "top-level registration order");
      test.id = row.testId; test.location = [row.line, row.column]; this.#ids.set(row.testId, test);
    }
    demand((this.#ids.get(row.testId) === test && test.stages.has("test:enqueue")) || type === "test:enqueue", "case was not originally enqueued");
    demand(row.parentId === (test.parent?.id ?? 0), "original test parent differs");
    demand(test.location[0] === row.line && test.location[1] === row.column, "case source coordinate changed");
    if (type !== "test:enqueue") demand(test.stages.has("test:enqueue"), "case precedes enqueue");
    if (["test:complete", "test:start", "test:pass"].includes(type)) demand(test.stages.has("test:dequeue"), "case precedes execution");
    if (type === "test:dequeue") {
      const siblings = test.parent ? test.parent.children : this.#top;
      const index = test.parent ? this.#nextNestedExecution : this.#nextTopExecution;
      demand(test === siblings[index] && (index === 0 || siblings[index - 1].stages.has("test:pass")),
        "serial execution order or preceding sibling outcome differs");
      if (test.parent) this.#nextNestedExecution += 1;
      else this.#nextTopExecution += 1;
    }
    if (terminal) {
      integer(row.testNumber, 1); demand(row.testNumber === test.number, "case ordinal differs");
      const parentName = test.parent?.name;
      const details = fields(row.details, ["duration_ms", "type", ...(type === "test:complete" ? ["passed"] : [])], ["classname"]);
      duration(details.duration_ms); demand(details.type === "test", "terminal suite substitution");
      demand(details.classname === parentName && row.classname === (type === "test:pass" ? parentName : undefined), "terminal parent label differs");
      if (type === "test:complete") {
        demand(details.passed === true, "test did not complete successfully"); test.duration = details.duration_ms;
        demand(!test.children || test.children.every((child) => child.stages.has("test:complete")), "parent completed before children");
      } else {
        demand(test.stages.has("test:complete") && test.stages.has("test:start")
          && details.duration_ms === test.duration, "pass lacks original completion/start");
        demand(!test.children || this.#nestedPlan && test.children.every((child) => child.stages.has("test:pass")), "parent passed without nested plan/outcomes");
      }
    }
    test.stages.add(type);
  }

  #plan(value) {
    const row = own(value);
    integer(row.nesting); integer(row.count);
    if (row.nesting === 1) {
      fields(row, ["nesting", "count", "file", "line", "column"]);
      demand(!this.#nestedPlan && !this.#rootPlan && row.count === 9
        && row.file === this.#parent.file && row.line === this.#parent.location?.[0]
        && row.column === this.#parent.location?.[1] && this.#parent.stages.has("test:complete")
        && this.#parent.children.every((child) => child.stages.has("test:pass")), "nested plan differs");
      this.#nestedPlan = true;
    } else {
      fields(row, ["nesting", "count"]);
      demand(row.nesting === 0 && row.count === 46 && !this.#rootPlan && this.#nestedPlan
        && this.#cases.every((test) => test.stages.has("test:pass")), "root plan differs");
      this.#rootPlan = true;
    }
  }

  #diagnostic(value) {
    const row = fields(value, ["nesting", "message", "level"]);
    integer(row.nesting);
    demand(this.#rootPlan && row.nesting === 0 && row.level === "info" && this.#diagnostics < 8, "unexpected diagnostic");
    if (this.#diagnostics < 7) demand(row.message === DIAGNOSTICS[this.#diagnostics], "test diagnostic differs");
    else {
      demand(/^duration_ms (?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u.test(row.message), "duration diagnostic differs");
      this.#diagnosticDuration = Number(row.message.slice(12)); duration(this.#diagnosticDuration);
    }
    this.#diagnostics += 1;
  }

  #end(value) {
    const row = fields(value, ["success", "counts", "duration_ms"]);
    const counts = fields(row.counts, Object.keys(COUNTS));
    for (const value of Object.values(counts)) integer(value);
    demand(row.success === true && this.#rootPlan && this.#diagnostics === 8
      && Object.keys(COUNTS).every((key) => counts[key] === COUNTS[key]), "summary is incomplete or unsuccessful");
    duration(row.duration_ms); demand(row.duration_ms === this.#diagnosticDuration, "summary duration differs");
    this.#summary = row;
  }

  /** Consume completed observations after actual stream EOF; grants no authority. */
  finish() {
    return this.#operation(() => {
      demand(this.#summary && this.#cases.every((test) => test.stages.size === 5)
        && this.#nextTop === 46 && this.#nextNested === 9
        && this.#nextTopExecution === 46 && this.#nextNestedExecution === 9, "event stream ended before all original cases");
      const cases = this.#cases.map((test) => Object.freeze({ suite: test.suite,
        path: Object.freeze([...test.path]), testId: test.id, parentId: test.parent?.id ?? 0,
        duration_ms: test.duration }));
      this.#finished = true;
      return Object.freeze({ contract_sha256: CONTRACT_SHA256, cases: Object.freeze(cases),
        counts: COUNTS, duration_ms: this.#summary.duration_ms });
    });
  }
}
