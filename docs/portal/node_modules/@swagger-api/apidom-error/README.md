# @swagger-api/apidom-error

`apidom-error` provides several error classes in support of [Joyent's Best Practices for Error Handling in Node.js](http://web.archive.org/web/20150221074228/http://www.joyent.com/developers/node/design/errors).
These error classes should be used as a superclass for all **operational errors**.
Operational errors represent run-time problems experienced by correctly-written programs.

Error classes from `apidom-error` handle complexities of extending native Error class
and support [error chaining](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/cause)
in backward and forward compatible way.

## Base errors

Base errors are meant to directly be thrown by ApiDOM code or to be extended
to form custom error hierarchies.

### ApiDOMError

Basic error class that can be easily extended and form error chains.

```js
import { ApiDOMError } from '@swagger-api/apidom-error';

const error1 = new ApiDOMError('error message'); // basic error
const error2 = new ApiDOMError('error message', { cause: new Error('cause') }); // error chain
```

### ApiDOMStructuredError

Error class that is based on `ApiDOMError` and allows to assign arbitrary data properties to its instances.

```js
import { ApiDOMStructuredError } from '@swagger-api/apidom-error';

const error1 = new ApiDOMStructuredError('error message', { cause: new Error('cause') }); // just like ApiDOMError

const error2 = new ApiDOMStructuredError('error message', {
  prop1: 'value1',
  prop2: 3,
})
error2.prop1; // => 'value1'
error2.prop2; // => 3
```

Although structured error class gives convenience by assigning arbitrary properties
it's highly recommended to extend `ApiDOMStructuredError` class and create public properties
explicitly.

```typescript
import { ApiDOMStructuredError, ApiDOMErrorOptions } from '@swagger-api/apidom-error';

interface StructuredErrorOptions extends ApiDOMErrorOptions {
  readonly prop1: string;
  readonly prop2: number;
}

class PropError extends ApiDOMStructuredError {
  public readonly  prop1: string;
  public readonly prop2: number;

  constructor(message?: string, options?: StructuredErrorOptions) {
    super(message, options)
    if (typeof options !== 'undefined') {
      this.prop1 = options.prop1;
      this.prop2 = options.prop2;
    }
  }
}
```

### ApiDOMAggregateError

`ApiDOMAggregateError` can be easily extended and represents instance of an error when
several errors need to be wrapped in a single error. It is thrown when multiple errors
need to be reported by an operation. Supports error chaining as well.

```js
import { ApiDOMAggregateError } from '@swagger-api/apidom-error';

const error1 = new Error('error1');
const error2 = new Error('error2');
const cause = new Error('optional cause');

const aggregateError = new ApiDOMAggregateError([error1, error2], 'aggregate error', {
  cause,
});
```

This class is based on native [AggregateError], and due to the fact that JavaScript
classes don't support multiple inheritance, it cannot also be based on `ApiDOMError`.
Nevertheless, for convenience, following will work:

```js
import { ApiDOMAggregateError, ApiDOMError } from '@swagger-api/apidom-error';

const aggregateError = new ApiDOMAggregateError([]);
aggregateError instanceof ApiDOMError; // => true
aggregateError instanceof AggregateError; // => true
```

## Generic custom errors

Generic custom errors represents custom errors that are generic enough
to be used across ApiDOM monorepo packages and are not specific to any
particular ApiDOM package.

### UnsupportedOperationError

This error class that is based on `ApiDOMError`. Thrown to indicate that the requested operation is not supported.

```js
import { UnsupportedOperationError } from '@swagger-api/apidom-error';

const error = new UnsupportedOperationError('error message');
```

### NotImplementedError

Error class that is based on `UnsupportedOperationError` and. It is thrown to indicate that a block
of code has not been implemented. This exception provides a more semantically rich description
of the problem than base `ApiDOMError`.

```js
import { NotImplementedError } from '@swagger-api/apidom-error';

const error = new NotImplementedError('error message');
```


