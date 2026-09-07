package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
)

// countedWriter observes the actual bytes accepted by an identity sink, so a
// serializer cannot report a byte length inconsistent with the hashed stream.
type countedWriter struct {
	writer io.Writer
	bytes  int64
}

func (w *countedWriter) Write(value []byte) (int, error) {
	n, err := w.writer.Write(value)
	w.bytes += int64(n)
	return n, err
}

// r1csIdentity hashes exactly the pinned gnark ConstraintSystem.WriteTo stream.
// It creates no trusted setup, proving key, verification key, or output file.
func r1csIdentity(system io.WriterTo) (int64, string, error) {
	digest := sha256.New()
	sink := &countedWriter{writer: digest}
	reported, err := system.WriteTo(sink)
	if err != nil {
		return 0, "", fmt.Errorf("write canonical R1CS: %w", err)
	}
	if reported <= 0 || reported != sink.bytes {
		return 0, "", fmt.Errorf("canonical R1CS byte count mismatch: reported %d, observed %d", reported, sink.bytes)
	}
	return sink.bytes, hex.EncodeToString(digest.Sum(nil)), nil
}
