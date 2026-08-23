Correct return type of `T` in a function generated via [`ffi_import`]

The associated type acts as a type-level rewrite; relying solely on generics would
introduce additional type parameters on every call site and break existing blanket
implementations. Keeping it as an associated type keeps downstream use ergonomic.
