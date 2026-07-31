class JSONPointerError extends Error {
  constructor(message, options = undefined) {
    super(message, options);
    this.name = this.constructor.name;
    if (typeof message === 'string') {
      this.message = message;
    }
    if (typeof Error.captureStackTrace === 'function') {
      Error.captureStackTrace(this, this.constructor);
    } else {
      this.stack = new Error(message).stack;
    }

    /**
     * This needs to stay here until our minimum supported version of Node.js is >= 16.9.0.
     * Node.js is >= 16.9.0 supports error causes natively.
     */
    if (options != null && typeof options === 'object' && Object.prototype.hasOwnProperty.call(options, 'cause') && !('cause' in this)) {
      const {
        cause
      } = options;
      this.cause = cause;
      if (cause instanceof Error && 'stack' in cause) {
        this.stack = `${this.stack}\nCAUSE: ${cause.stack}`;
      }
    }

    /**
     * Allows to assign arbitrary properties to the error object.
     */
    if (options != null && typeof options === 'object') {
      const {
        cause,
        ...causelessOptions
      } = options;
      Object.assign(this, causelessOptions);
    }
  }
}
export default JSONPointerError;