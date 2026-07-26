# This file generates the montogomary form integers for x in [0, 2^16) \intersect
# BN::ScalarField

verbose = False

modulus = 0x30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001 
R = 2**256  % modulus
table_size = 1<<16 

# @input:  field element a
# @output: 4 u64 a0, a1, a2, a3 s.t.
#          a = a3 * 2^192 + a2 * 2^128 + a1 * 2^64 + a0
def decompose_field_element(a):
    a0 = a % 2**64
    a = a // 2**64
    a1 = a % 2**64
    a = a // 2**64
    a2 = a % 2**64
    a = a // 2**64
    a3 = a
    return [a0, a1, a2, a3]


# @input:  field element a
# @output: a rust format string that encodes
#          4 u64 a0, a1, a2, a3 s.t.
#          a = a3 * 2^192 + a2 * 2^128 + a1 * 2^64 + a0
def format_field_element(a):
    [a0, a1, a2, a3] = decompose_field_element(a);
    return "Fr([" + hex(a0) + "," + hex(a1) + "," + hex(a2) + "," + hex(a3) + "]),\n"


f = open("src/bn256/fr/table.rs", "w")
f.write("//! auto generated file from scripts/bn256.sage, do not modify\n")
f.write("//! see src/bn256/fr.rs for more details\n")
f.write("use super::Fr;\n")
f.write("pub const FR_TABLE: &[Fr] = &[\n")

for i in range(table_size):
    a = (i * R) % modulus
    if verbose:
        print (i, a, format_field_element(a))
    f.write(format_field_element(a))

f.write("\n];")

