"""Token fingerprints for local ZK source guards; requires only Python's stdlib.

Comments and formatting do not change these contracts. Literal contents, token
boundaries, attributes, imports, and executable assertions do. This is a source
regression guard, not a Rust parser or a substitute for running the Rust tests.
"""

from __future__ import annotations

import hashlib
import re


RAW_STRING = re.compile(r'(?:br|cr|r)(#*)"')
CHARACTER = re.compile(r"b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
WORD = re.compile(r"[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9]*")
PUNCTUATION = re.compile(r"<<=|>>=|\.\.=|\.\.\.|::|->|=>|==|!=|<=|>=|&&|\|\||<<|>>|\+=|-=|\*=|/=|%=|\^=|&=|\|=|\.\.")


def rust_tokens(source: str) -> tuple[str, ...]:
    """Preserve Rust code tokens and literals while skipping comments/space."""
    result = []
    cursor = 0
    while cursor < len(source):
        if source[cursor].isspace():
            cursor += 1
            continue
        if source.startswith("//", cursor):
            end = source.find("\n", cursor)
            cursor = len(source) if end < 0 else end + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise AssertionError("unterminated Rust block comment")
            continue
        raw = RAW_STRING.match(source, cursor)
        if raw:
            delimiter = '"' + raw.group(1)
            end = source.find(delimiter, raw.end())
            if end < 0:
                raise AssertionError("unterminated Rust raw string")
            end += len(delimiter)
        else:
            quote = cursor + (source[cursor] in "bc")
            if quote < len(source) and source[quote] == '"':
                end = quote + 1
                while end < len(source) and source[end] != '"':
                    end += 2 if source[end] == "\\" else 1
                if end >= len(source):
                    raise AssertionError("unterminated Rust string")
                end += 1
            else:
                match = (
                    CHARACTER.match(source, cursor)
                    or WORD.match(source, cursor)
                    or PUNCTUATION.match(source, cursor)
                )
                end = match.end() if match else cursor + 1
        result.append(source[cursor:end])
        cursor = end
    return tuple(result)


def token_hash(source: str) -> str:
    """Hash length-delimited code tokens without conflating token boundaries."""
    digest = hashlib.sha256()
    for token in rust_tokens(source):
        encoded = token.encode()
        digest.update(len(encoded).to_bytes(8, "little"))
        digest.update(encoded)
    return digest.hexdigest()
