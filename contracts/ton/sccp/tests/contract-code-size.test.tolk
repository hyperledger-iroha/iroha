//! Hard bounds and an exact development report for compiled SCCP code cells.

import "@acton/build"
import "@acton/io"
import "@acton/testing/expect"

fun assertCodeTreeBounded(name: string, code: cell): void {
    val (cells, bits, refs) = code.calculateSizeStrict(1024);
    val depth = code.depth();
    println(
        "{} code: {} cells, {} bits, {} refs, depth {}",
        name,
        cells,
        bits,
        refs,
        depth,
    );
    expect(cells <= 1024).toEqual(true);
    expect(bits <= cells * 1023).toEqual(true);
    expect(depth < 512).toEqual(true);
}

get fun `test compiled contract code trees stay within TON structural bounds`() {
    assertCodeTreeBounded("TairaXorJettonWallet", build("TairaXorJettonWallet"));
    assertCodeTreeBounded("TairaXorJettonMaster", build("TairaXorJettonMaster"));
    assertCodeTreeBounded("TairaXorSccpBridge", build("TairaXorSccpBridge"));
}
