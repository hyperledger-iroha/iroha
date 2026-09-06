# Canonical schema identity qualification

`identity::NoritoSchema` declares a nominal identity and its single root-frame
projection independently of Rust module and crate locations. The dedicated
`NoritoSchema` derive requires `#[norito_schema(name = "...")]`; no source path
is inferred. Type and const arguments compose in declaration order, using each
type argument's nominal identity. Lifetime parameter slots use the fixed erased marker `'_`; concrete lifetime
values do not participate.

A non-generic declaration may specify `frame = "..."` when its root frame
advertises a different name. This distinction preserves the existing string
projections: `&str`, `Cow<str>` and `Box<str>` advertise the `String` frame, while
containers holding those values retain their nominal element names. It also
preserves explicitly named DTO roots without substituting that root name into
the identity of `Vec<Dto>` or another generic envelope. A fixed projection on a
generic derive is rejected because it would erase the type or const arguments.

The contract has no payload codec bound. `PhantomData<T>`, `HashOf<T>` and
`SignatureOf<T>` can use marker identities without making the marker implement
serialization or deserialization. Manually implemented identities must follow
the same composition rules and must not inspect `type_name`, `module_path` or
an alias registry.

The kernel retains the existing domain-separated SHA-256 name digest truncated
to 16 bytes. Its result does not depend on `schema-structural` or hardware.
There is one declared root-frame identity per type, with no alternate decode
names or compatibility dispatch.

## Current qualification boundary

The active `NoritoSerialize` and `NoritoDeserialize` frame selection has **not**
changed. This module is a preparation step for relocating model types while
preserving named and generic frame identities. Implementing `NoritoSchema`
alone does not alter existing frames. The executable tests explicitly expose
that a moved generic payload still receives a different header from the current
codec, even when its canonical identity and payload bytes match.

The Norito and crypto `schema_identity` tests record full frames, nominal names
and both codec-direction hashes from the active codec before its cutover. They
cover the standard codec constructors, nested explicitly named DTOs, root string
projections, primitive const arguments and typed cryptographic markers. Derive
unit and UI tests cover declaration errors and marker-only generic bounds.

TODO: Declare and qualify every production payload identity, including manual
codec implementations and generic envelopes, before connecting the kernel to
active encode/decode headers. That cutover must also cover nested payload
contexts that consult schema hashes. Structural schema generation remains a
separate inspection concern; changing active framing is outside this patch.

`Vec<&str>` and `Vec<Cow<str>>` currently have only an encoding fixture: the
existing vector decoder requires a higher-ranked element decoder that those
borrowed types do not implement. Their recorded decode hash is explicitly null.
