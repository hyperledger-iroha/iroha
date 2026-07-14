---- MODULE TmpParamLocal ----
EXTENDS Naturals, TLAPS
VARIABLE f

LOCAL I(index) == INSTANCE TmpParamBase WITH x <- f[index]

THEOREM Smoke == \A index \in 0..1: I(index)!Op = f[index] + 1
BY DEF I!Op
=============================================================================
