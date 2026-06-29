namespace Hyperledger.Iroha.Sccp;

internal static class SccpObjectSnapshots
{
    internal static IReadOnlyDictionary<string, object?>? CopyDictionaryOrNull(
        IReadOnlyDictionary<string, object?>? dictionary)
        => dictionary is null ? null : CopyDictionary(dictionary);

    internal static IReadOnlyDictionary<string, object?> CopyDictionary(
        IReadOnlyDictionary<string, object?> dictionary)
    {
        var snapshot = new Dictionary<string, object?>(dictionary.Count, StringComparer.Ordinal);
        foreach (var item in dictionary)
        {
            snapshot[item.Key] = CopyValue(item.Value);
        }

        return snapshot;
    }

    internal static IReadOnlyList<IReadOnlyDictionary<string, object?>>? CopyDictionaryListOrNull(
        IReadOnlyList<IReadOnlyDictionary<string, object?>>? dictionaries)
    {
        if (dictionaries is null)
        {
            return null;
        }

        var snapshot = new IReadOnlyDictionary<string, object?>[dictionaries.Count];
        for (var index = 0; index < dictionaries.Count; index++)
        {
            var dictionary = dictionaries[index]
                ?? throw new ArgumentException("List item must not be null.", nameof(dictionaries));
            snapshot[index] = CopyDictionary(dictionary);
        }

        return Array.AsReadOnly(snapshot);
    }

    private static object? CopyValue(object? value)
    {
        return value switch
        {
            null => null,
            string text => text,
            byte[] bytes => bytes.ToArray(),
            IReadOnlyDictionary<string, object?> dictionary => CopyDictionary(dictionary),
            IReadOnlyList<string> list => list.ToArray(),
            IReadOnlyList<object?> list => list.Select(CopyValue).ToArray(),
            System.Collections.IDictionary dictionary => CopyDictionary(dictionary),
            System.Collections.IEnumerable enumerable => CopyEnumerable(enumerable),
            _ => value,
        };
    }

    private static object CopyDictionary(System.Collections.IDictionary dictionary)
    {
        var snapshot = new Dictionary<string, object?>(dictionary.Count, StringComparer.Ordinal);
        foreach (System.Collections.DictionaryEntry item in dictionary)
        {
            if (item.Key is not string key)
            {
                return CopyObjectDictionary(dictionary);
            }

            snapshot[key] = CopyValue(item.Value);
        }

        return snapshot;
    }

    private static IReadOnlyDictionary<object, object?> CopyObjectDictionary(
        System.Collections.IDictionary dictionary)
    {
        var snapshot = new Dictionary<object, object?>(dictionary.Count);
        foreach (System.Collections.DictionaryEntry item in dictionary)
        {
            if (item.Key is null)
            {
                throw new ArgumentException("Dictionary keys must not be null.", nameof(dictionary));
            }

            snapshot[item.Key] = CopyValue(item.Value);
        }

        return snapshot;
    }

    private static object?[] CopyEnumerable(System.Collections.IEnumerable enumerable)
    {
        var values = new List<object?>();
        foreach (var item in enumerable)
        {
            values.Add(CopyValue(item));
        }

        return values.ToArray();
    }
}
