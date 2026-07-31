# @swaggerexpert/cookie

[![npmversion](https://badge.fury.io/js/@swaggerexpert%2Fcookie.svg)](https://www.npmjs.com/package/@swaggerexpert/cookie)
[![npm](https://img.shields.io/npm/dm/@swaggerexpert/cookie)](https://www.npmjs.com/package/@swaggerexpert/cookie)
[![Test workflow](https://github.com/swaggerexpert/cookie/actions/workflows/test.yml/badge.svg)](https://github.com/swaggerexpert/cookie/actions)
[![Dependabot enabled](https://img.shields.io/badge/Dependabot-enabled-blue.svg)](https://dependabot.com/)
[![try on RunKit](https://img.shields.io/badge/try%20on-RunKit-brightgreen.svg?style=flat)](https://npm.runkit.com/@swaggerexpert/cookie)
[![Tidelift](https://tidelift.com/badges/package/npm/@swaggerexpert%2Fcookie)](https://tidelift.com/subscription/pkg/npm-.swaggerexpert-cookie?utm_source=npm-swaggerexpert-cookie&utm_medium=referral&utm_campaign=readme)

`@swaggerexpert/cookie` is [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265) compliant cookie `parser`, `validator` and `serializer`.

<table>
  <tr>
    <td align="right" valign="middle">
        <img src="https://cdn2.hubspot.net/hubfs/4008838/website/logos/logos_for_download/Tidelift_primary-shorthand-logo.png" alt="Tidelift" width="60" />
      </td>
      <td valign="middle">
        <a href="https://tidelift.com/subscription/pkg/npm-.swaggerexpert-cookie?utm_source=npm-swaggerexpert-cookie&utm_medium=referral&utm_campaign=readme">
            Get professionally supported @swaggerexpert/cookie with Tidelift Subscription.
        </a>
      </td>
  </tr>
</table>

## Table of Contents

- [Getting started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
    - [Cookie](#cookie)
      - [Parsing cookie](#parsing-cookie)
      - [Serializing cookie](#serializing-cookie)
    - [Encoders](#encoders)
      - [Cookie Name encoders](#cookie-name-encoders)
      - [Cookie Value encoders](#cookie-value-encoders)
    - [Validators](#validators)
      - [Cookie Name validators](#cookie-name-validators)
      - [Cookie Value validators](#cookie-value-validators)
    - [Grammar](#grammar)
- [More about RFC 6265](#more-about-rfc-6265)
- [License](#license)

## Getting started

### Installation

You can install `@swaggerexpert/cookie` using `npm`:

```sh
 $ npm install @swaggerexpert/cookie
```

### Usage

`@swaggerexpert/cookie` currently supports **parsing**, **validation** and **serialization**.
Parser is based on a superset of [ABNF](https://www.rfc-editor.org/rfc/rfc5234) ([SABNF](https://cs.github.com/ldthomas/apg-js2/blob/master/SABNF.md))
and uses [apg-lite](https://github.com/ldthomas/apg-lite) parser generator.

#### Cookie

The `Cookie` header is an HTTP header used to send cookies from the client (e.g., a browser) to the server.
The `Cookie` header is sent in HTTP requests and contains one or more cookies that the server previously set
via the `Set-Cookie` header. Cookies are formatted as key-value pairs, separated by semicolons (`;`),
and do not include metadata like `Path`, `Domain`, or `HttpOnly`.

##### Parsing Cookie

Parsing a cookie is as simple as importing the **parseCookie** function and calling it.

```js
import { parseCookie } from '@swaggerexpert/cookie';

const parseResult = parseCookie('foo=bar');
parseResult.result.success; // => true
```

The **lenient** mode for cookie parsing is designed to handle and extract valid
cookie-pairs from potentially malformed or non-standard cookie strings.
It focuses on maintaining compatibility with real-world scenarios where cookie
headers may deviate from strict compliance with RFC 6265.

```js
import { parseCookie } from '@swaggerexpert/cookie';

/**
 * All of the following parse successfully.
 */

parseCookie('foo1=bar;  foo2=baz', { strict: false });
parseCookie('foo1=bar;foo2=baz', { strict: false });
parseCookie('FOO    = bar;   baz  =   raz', { strict: false });
parseCookie('foo="bar=123456789&name=Magic+Mouse"', { strict: false });
parseCookie('foo  =  "bar"', { strict: false });
parseCookie('foo  =  bar  ;  fizz  =  buzz', { strict: false });
parseCookie('foo =', { strict: false });
parseCookie('\tfoo\t=\tbar\t', { strict: false });
parseCookie('foo1=bar;foo2=baz', { strict: false });
parseCookie('foo1=bar;  foo2=baz', { strict: false });
parseCookie('foo=bar; fizz; buzz', { strict: false });
parseCookie('fizz; buzz; foo=bar', { strict: false });
parseCookie('name=name=3', { strict: false });
parseCookie('name:name=3', { strict: false });
```

**ParseResult** returned by the parser has the following shape:

```
{
  result: {
    success: true,
    state: 101,
    stateName: 'MATCH',
    length: 7,
    matched: 7,
    maxMatched: 7,
    maxTreeDepth: 9,
    nodeHits: 71
  },
  ast: fnast {
    callbacks: [
      'cookie-string': [Function: cookieString],
      'cookie-pair': [Function: cookiePair],
      'cookie-name': [Function: cookieName],
      'cookie-value': [Function: cookieValue]
    ],
    init: [Function (anonymous)],
    ruleDefined: [Function (anonymous)],
    udtDefined: [Function (anonymous)],
    down: [Function (anonymous)],
    up: [Function (anonymous)],
    translate: [Function (anonymous)],
    setLength: [Function (anonymous)],
    getLength: [Function (anonymous)],
    toXml: [Function (anonymous)]
  }
}
```

###### Interpreting AST as list of entries

```js
import { parseCookie } from '@swaggerexpert/cookie';

const parseResult = parse('foo=bar');
const parts = [];

parseResult.ast.translate(parts);
```

After running the above code, **parts** variable has the following shape:

```js
[
  ['cookie-string', 'foo=bar'],
  ['cookie-pair', 'foo=bar'],
  ['cookie-name', 'foo'],
  ['cookie-value', 'bar'],
]
```

###### Interpreting AST as XML

```js
import { parseCookie } from '@swaggerexpert/cookie';

const parseResult = parseCookie('foo=bar');
const xml = parseResult.ast.toXml();
```

After running the above code, **xml** variable has the following content:

```xml
<?xml version="1.0" encoding="utf-8"?>
<root nodes="4" characters="7">
  <!-- input string -->
  foo=bar
  <node name="cookie-string" index="0" length="7">
    foo=bar
    <node name="cookie-pair" index="0" length="7">
      foo=bar
      <node name="cookie-name" index="0" length="3">
        foo
      </node><!-- name="cookie-name" -->
      <node name="cookie-value" index="4" length="3">
        bar
      </node><!-- name="cookie-value" -->
    </node><!-- name="cookie-pair" -->
  </node><!-- name="cookie-string" -->
</root>
```

> NOTE: AST can also be traversed in classical way using [depth first traversal](https://www.tutorialspoint.com/data_structures_algorithms/depth_first_traversal.htm). For more information about this option please refer to [apg-js](https://github.com/ldthomas/apg-js) and [apg-js-examples](https://github.com/ldthomas/apg-js-examples).

##### Serializing Cookie

Serializing a cookie is as simple as importing the **serializeCookie** function and calling it.

**Serializing object**:

```js
import { serializeCookie } from '@swaggerexpert/cookie';

serializeCookie({ foo: 'bar' }); // => 'foo=bar'
serializeCookie({ foo: 'bar', baz: 'raz' }); // => 'foo=bar; baz=raz'
```

**Serializing entries**:

By using entries (list of key-value pairs), this function supports creating cookies
with duplicate names—a common scenario when sending multiple values for the same cookie name.

```js
import { serializeCookie } from '@swaggerexpert/cookie';

serializeCookie([['foo',  'bar']]); // => 'foo=bar'
serializeCookie([[ 'foo', 'bar'], ['baz', 'raz' ]]); // => 'foo=bar; baz=raz'
serializeCookie([[ 'foo', 'bar'], ['baz', 'raz' ], ['foo', 'boo']]); // => 'foo=bar; baz=raz; foo=boo'
```

**Options**

The `serializeCookie` function provides a powerful and flexible API for serializing cookies,
enabling the implementation of various scenarios. By default, it uses a set of sensible options
to ensure compliance with cookie standards. However, its behavior can be customized by overriding these defaults.

**Default Options**

[Encoders](#encoders) transform cookie names and values during serialization to ensure they meet encoding requirements.
[Validators](#validators) ensure that cookie names and values conform to the required standards. If validation fails, an error is thrown.
Validators are executed **after** encoders, ensuring that values are first transformed before being validated.

The default options for `serializeCookie` ensure strict compliance with cookie standards by applying
no encoding for cookie names, RFC compliant encoding for values, and robust validation for
both names and values according to [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265).

```js
{
  encoders: {
    name: identity,
    value: cookieValueStrictBase64urlEncoder
  },
  validators: {
    name: cookieNameStrictValidator,
    value: cookieValueStrictValidator,
  }
}
```

**Customizing serialization**

You can customize `serializeCookie` behavior by providing your own [encoders](#encoders) or [validators](#validators):

```js
import { serializeCookie } from '@swaggerexpert/cookie';

serializeCookie({ foo: 'bar' }, {
  encoders: {
    name: (name) => name.toUpperCase(), // custom name encoder
    value: (value) => encodeURIComponent(value), // custom value encoder
  },
  validators: {
    name: (name) => {
      if (!/^[a-zA-Z]+$/.test(name)) {
        throw new TypeError('Custom validation failed for cookie name');
      }
    },
    value: (value) => {
      if (value.includes(';')) {
        throw new TypeError('Custom validation failed for cookie value');
      }
    },
  },
});
```

Completely **bypassing** encoding and validation is possible as well:

```js
import { serializeCookie, identity, noop } from '@swaggerexpert/cookie';

serializeCookie({ foo: ';' }, {
  encoders: {  name: identity, value: identity },
  validators: { name: noop, value: noop },
}); // => "foo=;"
```

#### Encoders

The `@swaggerexpert/cookie` library provides a suite of encoders designed to handle cookie names and values during **serialization**.
Each encoder adheres to specific rules for encoding characters, ensuring compatibility with cookie-related standards.
Below is a detailed overview of the available encoders.

##### Generic Encoders

Generic encoders can be used both for cookie names and values.

**base64**

Encodes a string using the [Base64](https://datatracker.ietf.org/doc/html/rfc4648) algorithm. Base64 encoding
is explicitly mentioned in [RFC 6265, section 4.1.1](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1) as an example of appropriate encoding for cookie values.

```js
import { base64Encoder } from '@swaggerexpert/cookie';

base64Encoder('foo<'); // => 'Zm9vPA=='
```

**base64url**

Encodes a string using the [base64url](https://datatracker.ietf.org/doc/html/rfc4648#section-5) algorithm.
This encoding consists of Base64 Encoding with URL and Filename Safe Alphabet.

```js
import { base64urlEncoder } from '@swaggerexpert/cookie';

base64urlEncoder('foo<'); // => 'Zm9vPA'
```

##### Cookie Name Encoders

**cookieNameStrictPercentEncoder**

Percent-encodes characters that fall outside the allowable set defined by the `cookie-name` rule in [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

- **Use Case**: Ensures that the cookie name strictly adheres to the `cookie-name` non-terminal rule.

```js
import { cookieNameStrictPercentEncoder } from '@swaggerexpert/cookie';

cookieNameStrictPercentEncoder('foo<'); // => 'foo%3C'
```

**cookieNameLenientPercentEncoder**

Percent-encodes characters that fall outside the allowable set defined by the `lenient-cookie-name` rule.
This allows for a more lenient interpretation of cookie names.

- **Use Case**: Useful in scenarios where broader compatibility is required, or when leniency in cookie names is acceptable.

```js
import { cookieNameLenientPercentEncoder } from '@swaggerexpert/cookie';

cookieNameLenientPercentEncoder('foo<'); // => 'foo<'
```

##### Cookie Value Encoders

**cookieValueStrictBase64Encoder**

Applies [Base64](https://datatracker.ietf.org/doc/html/rfc4648) encoding to a cookie value if any of its characters fall outside the allowable set defined by the `cookie-value` rule in  [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

- **Use Case**: To achieve the best compatibility, Base64 encoding is mentioned by RFC 6265 as an example of appropriate encoding to use. Ensures strict compliance with the `cookie-value` non-terminal rule.

```js
import { cookieValueStrictBase64Encoder } from '@swaggerexpert/cookie';

cookieValueStrictBase64Encoder(';'); // => Ow==
cookieValueStrictBase64Encoder('";"'); // => "Ow=="
```

**cookieValueStrictBase64urlEncoder**

Applies [base64url](https://datatracker.ietf.org/doc/html/rfc4648#section-5) encoding to a cookie value if any of its characters fall outside the allowable set defined by the `cookie-value` rule in  [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

- **Use Case**: More appropriate for modern tooling, it ensures strict compliance with the `cookie-value` non-terminal rule by producing encoded value with URL and Filename Safe Alphabet.

```js
import { cookieValueStrictBase64Encoder } from '@swaggerexpert/cookie';

cookieValueStrictBase64Encoder(';'); // => Ow
cookieValueStrictBase64Encoder('";"'); // => "Ow"
```

**cookieValueStrictPercentEncoder**

Percent-encodes characters that fall outside the allowable set defined by the `cookie-value` rule in [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

- **Use Case**: Ensures strict compliance with the `cookie-value` non-terminal rule, encoding characters such as `;` and `,`.

```js
import { cookieValueStrictPercentEncoder } from '@swaggerexpert/cookie';

cookieValueStrictPercentEncoder(';'); // => '%3B'
```

**cookieValueLenientBase64Encoder**

Applies [Base64](https://datatracker.ietf.org/doc/html/rfc4648) encoding to a cookie value if any of its characters fall outside the allowable set defined by the `lenient-cookie-value`.

- **Use Case**: To achieve the best compatibility, Base64 encoding is mentioned by RFC 6265 as an example of appropriate encoding to use. Useful when broader compatibility is needed, and when leniency in cookie values is acceptable.

```js
import { cookieValueLenientBase64Encoder } from '@swaggerexpert/cookie';

cookieValueLenientBase64Encoder(';'); // => Ow==
cookieValueLenientBase64Encoder('";"'); // => "Ow=="
```

**cookieValueLenientBase64urlEncoder**

Applies [base64url](https://datatracker.ietf.org/doc/html/rfc4648#section-5) encoding to a cookie value if any of its characters fall outside the allowable set defined by the `lenient-cookie-value`.

- **Use Case**: More appropriate for modern tooling. Useful when producing encoded value with URL and Filename Safe Alphabet, and leniency in cookie values is acceptable.
-

```js
import { cookieValueLenientBase64urlEncoder } from '@swaggerexpert/cookie';

cookieValueLenientBase64urlEncoder(';'); // => Ow
cookieValueLenientBase64urlEncoder('";"'); // => "Ow"
```

**cookieValueLenientPercentEncoder**

Percent-encodes characters that fall outside the allowable set defined by the `lenient-cookie-value` rule.
This allows for a more permissive interpretation of cookie values.

- **Use Case**: Useful when broader compatibility is needed, and when leniency in cookie values is acceptable.

```js
import { cookieValueLenientPercentEncoder } from '@swaggerexpert/cookie';

cookieValueLenientPercentEncoder('"'); // => '"'
```

#### Validators

The `@swaggerexpert/cookie` library provides a suite of validators designed to ensure that cookie names and values
comply with the necessary standards during **serialization**. These validators are strict about adherence to the rules
while offering flexibility with both strict and lenient validation modes. Validators focus on validating input and
throwing descriptive errors for invalid values.

##### Cookie Name Validators

**cookieNameStrictValidator**

Validates cookie names based on the `cookie-name` rule defined in [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

```js
import { cookieNameStrictValidator } from '@swaggerexpert/cookie';

cookieNameStrictValidator('ValidName'); // passes
cookieNameStrictValidator('InvalidName<'); // throws: Invalid cookie name
```


**cookieNameLenientValidator**

Validates cookie names based on the `lenient-cookie-name` rules Allows a broader range of characters than the strict validator.

```js
import { cookieNameLenientValidator } from '@swaggerexpert/cookie';

cookieNameLenientValidator('ValidLenientName'); // passes
cookieNameLenientValidator('\tInvalidLenientName'); // throws: Invalid cookie name
```

##### Cookie Value Validators

**cookieValueStrictValidator**

Validates cookie values according to the `cookie-value` rules in [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1).

```js
import { cookieValueStrictValidator } from '@swaggerexpert/cookie';

cookieValueStrictValidator('"validValue"'); // passes
cookieValueStrictValidator('invalid\\value'); // throws: Invalid cookie value
```

**cookieValueLenientValidator**

Validates cookie values according to `lenient-cookie-value` rules. Accepts a broader range of characters,
including some that are disallowed in strict mode.

```js
import { cookieValueLenientValidator } from '@swaggerexpert/cookie';

cookieValueLenientValidator('"lenient;value"'); // passes
cookieValueLenientValidator('invalid value'); // throws: Invalid cookie value
```

#### Grammar

New grammar instance can be created in following way:

```js
import { Grammar } from '@swaggerexpert/cookie';

const grammar = new Grammar();
```

To obtain original ABNF (SABNF) grammar as a string:

```js
import { Grammar } from '@swaggerexpert/cookie';

const grammar = new Grammar();

grammar.toString();
// or
String(grammar);
```

## More about RFC 6265

The cookie is defined by the following [ABNF](https://tools.ietf.org/html/rfc5234) syntax

```abnf
; Lenient version of https://datatracker.ietf.org/doc/html/rfc6265#section-4.2.1
lenient-cookie-string        = lenient-cookie-entry *( ";" OWS lenient-cookie-entry )
lenient-cookie-entry         = lenient-cookie-pair / lenient-cookie-pair-invalid
lenient-cookie-pair          = OWS lenient-cookie-name OWS "=" OWS lenient-cookie-value OWS
lenient-cookie-pair-invalid  = OWS 1*tchar OWS ; Allow for standalone entries like "fizz" to be ignored
lenient-cookie-name          = 1*( %x21-3A / %x3C / %x3E-7E ) ; Allow all printable US-ASCII except "="
lenient-cookie-value         = lenient-quoted-value [ *lenient-cookie-octet ] / *lenient-cookie-octet
lenient-quoted-value         = DQUOTE *( lenient-quoted-char ) DQUOTE
lenient-quoted-char          = %x20-21 / %x23-7E ; Allow all printable US-ASCII except DQUOTE
lenient-cookie-octet         = %x21-2B / %x2D-3A / %x3C-7E
                             ; Allow all printable characters except CTLs, semicolon and SP

; https://datatracker.ietf.org/doc/html/rfc6265#section-4.2.1
cookie-string     = cookie-pair *( ";" SP cookie-pair )

; https://datatracker.ietf.org/doc/html/rfc6265#section-4.1.1
; https://www.rfc-editor.org/errata/eid5518
cookie-pair       = cookie-name "=" cookie-value
cookie-name       = token
cookie-value      = ( DQUOTE *cookie-octet DQUOTE ) / *cookie-octet
                  ; https://www.rfc-editor.org/errata/eid8242
cookie-octet      = %x21 / %x23-2B / %x2D-3A / %x3C-5B / %x5D-7E
                       ; US-ASCII characters excluding CTLs,
                       ; whitespace, DQUOTE, comma, semicolon,
                       ; and backslash

; https://datatracker.ietf.org/doc/html/rfc6265#section-2.2
OWS            = *( [ CRLF ] WSP ) ; "optional" whitespace

; https://datatracker.ietf.org/doc/html/rfc9110#section-5.6.2
token          = 1*(tchar)
tchar          = "!" / "#" / "$" / "%" / "&" / "'" / "*"
                 / "+" / "-" / "." / "^" / "_" / "`" / "|" / "~"
                 / DIGIT / ALPHA
                 ; any VCHAR, except delimiters

; https://datatracker.ietf.org/doc/html/rfc2616#section-2.2
CHAR           = %x01-7F ; any US-ASCII character (octets 0 - 127)
CTL            = %x00-1F / %x7F ; any US-ASCII control character
separators     = "(" / ")" / "<" / ">" / "@" / "," / ";" / ":" / "\" / %x22 / "/" / "[" / "]" / "?" / "=" / "{" / "}" / SP / HT
SP             = %x20 ; US-ASCII SP, space (32)
HT             = %x09 ; US-ASCII HT, horizontal-tab (9)

; https://datatracker.ietf.org/doc/html/rfc5234#appendix-B.1
ALPHA          =  %x41-5A / %x61-7A ; A-Z / a-z
DIGIT          =  %x30-39 ; 0-9
DQUOTE         =  %x22 ; " (Double Quote)
WSP            =  SP / HTAB ; white space
HTAB           =  %x09 ; horizontal tab
CRLF           =  CR LF ; Internet standard newline
CR             =  %x0D ; carriage return
LF             =  %x0A ; linefeed
```

## License

`@swaggerexpert/cookie` is licensed under [Apache 2.0 license](https://github.com/swaggerexpert/cookie/blob/main/LICENSE).
`@swaggerexpert/cookie` comes with an explicit [NOTICE](https://github.com/swaggerexpert/cookie/blob/main/NOTICE) file
containing additional legal notices and information.
