"use strict";

exports.__esModule = true;
exports.default = void 0;
class TraceBuilder {
  #trace;
  #path;
  #realm;
  constructor(trace, context = {}) {
    this.#trace = trace;
    this.#trace.steps = [];
    this.#trace.failed = false;
    this.#trace.failedAt = -1;
    this.#trace.message = `JSON Pointer "${context.jsonPointer}" was successfully evaluated against the provided value`;
    this.#trace.context = {
      ...context,
      realm: context.realm.name
    };
    this.#path = [];
    this.#realm = context.realm;
  }
  step({
    referenceToken,
    input,
    output,
    success = true,
    reason
  }) {
    const position = this.#path.length;
    this.#path.push(referenceToken);
    const step = {
      referenceToken,
      referenceTokenPosition: position,
      input,
      inputType: this.#realm.isObject(input) ? 'object' : this.#realm.isArray(input) ? 'array' : 'unrecognized',
      output,
      success
    };
    if (reason) {
      step.reason = reason;
    }
    this.#trace.steps.push(step);
    if (!success) {
      this.#trace.failed = true;
      this.#trace.failedAt = position;
      this.#trace.message = reason;
    }
  }
}
var _default = exports.default = TraceBuilder;