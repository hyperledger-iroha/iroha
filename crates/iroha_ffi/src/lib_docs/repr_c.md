Robust type that conforms to C ABI and can be safely shared across FFI boundaries. This does
not guarantee the ABI compatibility of the referent for pointers. These pointers are opaque

# Safety

Type implementing the trait must be a robust type with a guaranteed C ABI. Care must be taken
not to dereference pointers whose referents don't implement `ReprC`; they are considered opaque
