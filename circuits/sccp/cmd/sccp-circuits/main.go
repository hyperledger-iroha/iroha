// Command sccp-circuits exposes the closed catalogue and deterministic test vectors.
package main

import (
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/frontend/cs/r1cs"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/circuit"
	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

func main() {
	if len(os.Args) < 2 {
		fatal("usage: sccp-circuits <catalogue|emit-kat|constraint-count>")
	}
	switch os.Args[1] {
	case "catalogue":
		catalogue()
	case "emit-kat":
		emitKAT(os.Args[2:])
	case "constraint-count":
		constraintCount(os.Args[2:])
	default:
		fatal("unknown command %q", os.Args[1])
	}
}

func constraintCount(arguments []string) {
	flags := flag.NewFlagSet("constraint-count", flag.ExitOnError)
	profileID := flags.String("profile", "", "exact final-V1 profile id")
	if err := flags.Parse(arguments); err != nil {
		fatal("parse arguments: %v", err)
	}
	if flags.NArg() != 0 || *profileID == "" {
		fatal("constraint-count requires exactly --profile")
	}
	cfg, err := profile.ByID(*profileID)
	if err != nil {
		fatal("resolve profile: %v", err)
	}
	var definition frontend.Circuit
	if cfg.Role == profile.Message {
		definition, err = circuit.NewMessage(cfg)
	} else {
		definition, err = circuit.NewEpochAnchor(cfg)
	}
	if err != nil {
		fatal("construct circuit: %v", err)
	}
	field := ecc.BN254.ScalarField()
	if cfg.Curve == profile.BLS12381 {
		field = ecc.BLS12_381.ScalarField()
	}
	constraints, err := frontend.Compile(field, r1cs.NewBuilder, definition)
	if err != nil {
		fatal("compile circuit: %v", err)
	}
	result := struct {
		Profile     string        `json:"profile"`
		Role        profile.Role  `json:"role"`
		OuterCurve  profile.Curve `json:"outer_curve"`
		BLSMode     string        `json:"bls_mode"`
		Constraints int           `json:"constraints"`
	}{
		Profile:     cfg.ID,
		Role:        cfg.Role,
		OuterCurve:  cfg.Curve,
		BLSMode:     "emulated-bls12-381-g1-g2-pairing",
		Constraints: constraints.GetNbConstraints(),
	}
	if err := json.NewEncoder(os.Stdout).Encode(result); err != nil {
		fatal("encode constraint count: %v", err)
	}
}

func emitKAT(arguments []string) {
	flags := flag.NewFlagSet("emit-kat", flag.ExitOnError)
	profileID := flags.String("profile", "", "exact final-V1 profile id")
	outputPath := flags.String("output", "-", "exclusive output file or - for stdout")
	if err := flags.Parse(arguments); err != nil {
		fatal("parse arguments: %v", err)
	}
	if flags.NArg() != 0 || *profileID == "" {
		fatal("emit-kat requires exactly --profile and optional --output")
	}
	cfg, err := profile.ByID(*profileID)
	if err != nil {
		fatal("resolve profile: %v", err)
	}
	vector, err := circuit.PublicKAT(cfg)
	if err != nil {
		fatal("construct KAT: %v", err)
	}
	var output io.Writer = os.Stdout
	var file *os.File
	if *outputPath != "-" {
		file, err = os.OpenFile(*outputPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			fatal("create KAT output: %v", err)
		}
		defer file.Close()
		output = file
	}
	encoder := json.NewEncoder(output)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(vector); err != nil {
		fatal("encode KAT: %v", err)
	}
}

func catalogue() {
	type entry struct {
		ID               string        `json:"id"`
		Lane             string        `json:"lane"`
		Role             profile.Role  `json:"role"`
		Curve            profile.Curve `json:"curve"`
		IndependentKeyID string        `json:"independent_key_id"`
	}
	entries := make([]entry, 0, 8)
	for _, cfg := range profile.All() {
		entries = append(entries, entry{
			ID:               cfg.ID,
			Lane:             cfg.Lane,
			Role:             cfg.Role,
			Curve:            cfg.Curve,
			IndependentKeyID: hex.EncodeToString(cfg.IndependentKeyID[:]),
		})
	}
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(entries); err != nil {
		fatal("encode catalogue: %v", err)
	}
}

func fatal(format string, arguments ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", arguments...)
	os.Exit(1)
}
