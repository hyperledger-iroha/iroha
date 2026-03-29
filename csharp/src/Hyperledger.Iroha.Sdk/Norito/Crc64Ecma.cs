namespace Hyperledger.Iroha.Norito;

public static class Crc64Ecma
{
    private static readonly ulong[] Table = BuildTable();

    public static ulong Compute(ReadOnlySpan<byte> data)
    {
        var crc = ulong.MaxValue;
        foreach (var value in data)
        {
            var index = (int)((crc ^ value) & 0xFF);
            crc = Table[index] ^ (crc >> 8);
        }

        return crc ^ ulong.MaxValue;
    }

    private static ulong[] BuildTable()
    {
        const ulong polynomial = 0xC96C5795D7870F42UL;
        var table = new ulong[256];
        for (var index = 0; index < table.Length; index++)
        {
            ulong crc = (uint)index;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 1) != 0
                    ? (crc >> 1) ^ polynomial
                    : crc >> 1;
            }

            table[index] = crc;
        }

        return table;
    }
}
