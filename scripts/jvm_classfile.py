"""Read JVM declaration metadata without loading classes or using reflection.

This standard-library-only reader is used by the Kotlin JNI release check.
It accepts bounded class files and rejects malformed or ambiguous declarations;
it does not execute bytecode or replace the JVM verifier or JDK API linting.
"""

from __future__ import annotations

from dataclasses import dataclass


MAX_CLASS_BYTES = 8 * 1024 * 1024
ACC_PUBLIC = 0x0001
ACC_STATIC = 0x0008
ACC_NATIVE = 0x0100
ACC_ABSTRACT = 0x0400


class ClassFileError(ValueError):
    """The input is not a bounded, unambiguous JVM declaration inventory."""


@dataclass(frozen=True)
class Method:
    """A declared method, including private and synthetic methods."""

    name: str
    descriptor: str
    flags: int

    @property
    def native(self) -> bool:
        """Whether the method has the JVM ACC_NATIVE flag."""
        return bool(self.flags & ACC_NATIVE)

    @property
    def static(self) -> bool:
        """Whether JNI receives a class rather than an instance receiver."""
        return bool(self.flags & ACC_STATIC)


@dataclass(frozen=True)
class ClassFile:
    """Class identity and declarations, independent of any runtime loader."""

    name: str
    major: int
    source_file: str | None
    methods: tuple[Method, ...]
    super_name: str | None = None
    interfaces: tuple[str, ...] = ()
    lambda_metafactory: bool = False


def _modified_utf8(raw: bytes) -> str:
    # JVM UTF8 constants encode UTF-16 code units: NUL is C0 80, and a
    # supplementary character is two three-byte surrogate encodings.
    if b"\0" in raw or any(value >= 0xF0 for value in raw):
        raise ClassFileError("invalid modified UTF-8 constant")
    try:
        units = raw.replace(b"\xc0\x80", b"\0").decode("utf-8", "surrogatepass")
        return units.encode("utf-16-be", "surrogatepass").decode("utf-16-be")
    except UnicodeError as error:
        raise ClassFileError("invalid declaration name in modified UTF-8") from error


def method_descriptor(descriptor: str) -> tuple[tuple[str, ...], str]:
    """Parse a JVM method descriptor, rejecting void arguments and bad arrays."""
    if not descriptor.startswith("("):
        raise ClassFileError(f"invalid method descriptor: {descriptor!r}")

    def field(position: int, *, result: bool = False) -> int:
        start = position
        while position < len(descriptor) and descriptor[position] == "[":
            position += 1
        dimensions = position - start
        if position == len(descriptor) or dimensions > 255:
            raise ClassFileError("invalid array descriptor")
        kind = descriptor[position]
        if kind == "L":
            end = descriptor.find(";", position + 1)
            name = descriptor[position + 1:end]
            if end < 0 or not name or any(c in name for c in ".[;\0"):
                raise ClassFileError("invalid object descriptor")
            if any(not part for part in name.split("/")):
                raise ClassFileError("invalid object class name")
            return end + 1
        if kind in "BCDFIJSZ" or (kind == "V" and result and not dimensions):
            return position + 1
        raise ClassFileError("invalid field descriptor")

    position, slots = 1, 0
    arguments = []
    while position < len(descriptor) and descriptor[position] != ")":
        end = field(position)
        argument = descriptor[position:end]
        arguments.append(argument)
        slots += 2 if argument in ("J", "D") else 1
        position = end
    if position == len(descriptor) or slots > 255:
        raise ClassFileError("invalid method parameter list")
    result_start = position + 1
    if field(result_start, result=True) != len(descriptor):
        raise ClassFileError("trailing method descriptor data")
    return tuple(arguments), descriptor[result_start:]


def parse_class(data: bytes) -> ClassFile:
    """Read class metadata with checked lengths and constant-pool references."""
    if len(data) > MAX_CLASS_BYTES:
        raise ClassFileError("class file exceeds the byte limit")
    position = 0

    def take(size: int) -> bytes:
        nonlocal position
        end = position + size
        if end > len(data):
            raise ClassFileError("truncated class file")
        value = data[position:end]
        position = end
        return value

    def integer(size: int) -> int:
        return int.from_bytes(take(size), "big")

    if integer(4) != 0xCAFEBABE:
        raise ClassFileError("invalid class file magic")
    integer(2)  # minor version
    major = integer(2)
    count = integer(2)
    if count < 1:
        raise ClassFileError("empty constant pool")
    pool: list[tuple[int, bytes] | None] = [None] * count
    index = 1
    widths = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4,
              11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}
    while index < count:
        tag = integer(1)
        if tag == 1:
            payload = take(integer(2))
        elif tag in widths:
            payload = take(widths[tag])
        else:
            raise ClassFileError(f"unknown constant-pool tag {tag}")
        pool[index] = (tag, payload)
        if tag in (5, 6):
            index += 1
            if index >= count:
                raise ClassFileError("wide constant has no reserved slot")
        index += 1

    def entry(index: int, tag: int) -> bytes:
        if not 0 < index < count or pool[index] is None or pool[index][0] != tag:
            raise ClassFileError("invalid constant-pool reference")
        return pool[index][1]

    def utf8(index: int) -> str:
        return _modified_utf8(entry(index, 1))

    def attributes() -> dict[str, bytes]:
        found = {}
        for _ in range(integer(2)):
            name = utf8(integer(2))
            if name in found and name in ("Code", "SourceFile", "BootstrapMethods"):
                raise ClassFileError(f"duplicate {name} attribute")
            found[name] = take(integer(4))
        return found

    integer(2)  # class access flags
    name = utf8(int.from_bytes(entry(integer(2), 7), "big"))
    if not name or any(c in name for c in ".[;\0") or any(not p for p in name.split("/")):
        raise ClassFileError("invalid class name")
    superclass_index = integer(2)
    super_name = (
        utf8(int.from_bytes(entry(superclass_index, 7), "big"))
        if superclass_index else None
    )
    interfaces = tuple(
        utf8(int.from_bytes(entry(integer(2), 7), "big"))
        for _ in range(integer(2))
    )
    for parent_name in ((super_name,) if super_name is not None else ()) + interfaces:
        if not parent_name or any(c in parent_name for c in ".[;\0") or any(not part for part in parent_name.split("/")):
            raise ClassFileError("invalid parent class name")
    for _ in range(integer(2)):
        take(6)  # fields: access, name, descriptor
        attributes()
    methods = []
    seen = set()
    for _ in range(integer(2)):
        flags = integer(2)
        method = Method(utf8(integer(2)), utf8(integer(2)), flags)
        method_descriptor(method.descriptor)
        key = (method.name, method.descriptor)
        if key in seen:
            raise ClassFileError("duplicate method declaration")
        seen.add(key)
        attrs = attributes()
        if method.native and (flags & ACC_ABSTRACT or "Code" in attrs or method.name.startswith("<")):
            raise ClassFileError("native method has incompatible attributes")
        methods.append(method)
    attrs = attributes()
    source_file = None
    if "SourceFile" in attrs:
        source = attrs["SourceFile"]
        if len(source) != 2:
            raise ClassFileError("invalid SourceFile attribute")
        source_file = utf8(int.from_bytes(source, "big"))
    if position != len(data):
        raise ClassFileError("trailing class file bytes")
    lambda_metafactory = False
    bootstrap = attrs.get("BootstrapMethods")
    if bootstrap is not None:
        offset = 0

        def bootstrap_u2() -> int:
            nonlocal offset
            if offset + 2 > len(bootstrap):
                raise ClassFileError("truncated BootstrapMethods attribute")
            value = int.from_bytes(bootstrap[offset:offset + 2], "big")
            offset += 2
            return value

        for _ in range(bootstrap_u2()):
            handle = entry(bootstrap_u2(), 15)
            if handle[0] == 6:  # REF_invokeStatic: the two standard lambda bootstraps.
                reference = int.from_bytes(handle[1:], "big")
                if not 0 < reference < count or pool[reference] is None or pool[reference][0] not in (10, 11):
                    raise ClassFileError("invalid static bootstrap method reference")
                target = pool[reference][1]
                owner = utf8(int.from_bytes(entry(int.from_bytes(target[:2], "big"), 7), "big"))
                signature = entry(int.from_bytes(target[2:], "big"), 12)
                method = utf8(int.from_bytes(signature[:2], "big"))
                if owner == "java/lang/invoke/LambdaMetafactory" and method in ("metafactory", "altMetafactory"):
                    lambda_metafactory = True
            for _ in range(bootstrap_u2()):
                argument = bootstrap_u2()
                if not 0 < argument < count or pool[argument] is None:
                    raise ClassFileError("invalid bootstrap argument reference")
        if offset != len(bootstrap):
            raise ClassFileError("trailing BootstrapMethods attribute data")
    return ClassFile(name, major, source_file, tuple(methods), super_name, interfaces, lambda_metafactory)
