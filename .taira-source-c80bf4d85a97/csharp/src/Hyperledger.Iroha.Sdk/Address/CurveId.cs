namespace Hyperledger.Iroha.Address;

public enum CurveId : byte
{
    Ed25519 = 1,
    MlDsa = 2,
    Gost256A = 10,
    Gost256B = 11,
    Gost256C = 12,
    Gost512A = 13,
    Gost512B = 14,
    Sm2 = 15,
}
