package sorafsfixtures

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type chunkerFixture struct {
	Profile            string   `json:"profile"`
	InputSeed          string   `json:"input_seed"`
	InputLength        int      `json:"input_length"`
	ChunkCount         int      `json:"chunk_count"`
	ChunkLengths       []int    `json:"chunk_lengths"`
	ChunkOffsets       []int    `json:"chunk_offsets"`
	ChunkDigestSHA3    string   `json:"chunk_digest_sha3_256"`
	ChunkDigestsBLAKE3 []string `json:"chunk_digests_blake3"`
}

type backpressureScenario struct {
	Name                  string `json:"name"`
	FeedSizes             []int  `json:"feed_sizes"`
	ExpectedChunkLengths  []int  `json:"expected_chunk_lengths"`
	ChunkCount            int    `json:"chunk_count"`
	MaxFeedSize           int    `json:"max_feed_size"`
	MinFeedSize           int    `json:"min_feed_size"`
}

type backpressureCorpus struct {
	Profile    string                `json:"profile"`
	InputLength int                  `json:"input_length"`
	ChunkDigest string               `json:"chunk_digest_sha3_256"`
	Scenarios  []backpressureScenario `json:"scenarios"`
}

func fixtureDir(t *testing.T) string {
	t.Helper()
	dir, err := filepath.Abs(".")
	if err != nil {
		t.Fatalf("resolve fixture dir: %v", err)
	}
	return dir
}

func loadFixture(t *testing.T) chunkerFixture {
	t.Helper()
	path := filepath.Join(fixtureDir(t), "sf1_profile_v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture json: %v", err)
	}
	var fixture chunkerFixture
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatalf("decode fixture json: %v", err)
	}
	return fixture
}

func TestChunkerFixtureConsistency(t *testing.T) {
	fixture := loadFixture(t)

	if fixture.Profile != "sorafs.sf1@1.0.0" {
		t.Fatalf("unexpected profile: %s", fixture.Profile)
	}
	if fixture.ChunkCount != len(fixture.ChunkLengths) {
		t.Fatalf("chunk count mismatch: %d vs %d", fixture.ChunkCount, len(fixture.ChunkLengths))
	}
	if fixture.ChunkCount != len(fixture.ChunkOffsets) {
		t.Fatalf("chunk offsets mismatch: %d vs %d", fixture.ChunkCount, len(fixture.ChunkOffsets))
	}
	if fixture.ChunkCount != len(fixture.ChunkDigestsBLAKE3) {
		t.Fatalf("chunk digest count mismatch: %d vs %d", fixture.ChunkCount, len(fixture.ChunkDigestsBLAKE3))
	}

	total := 0
	for idx, length := range fixture.ChunkLengths {
		if length <= 0 {
			t.Fatalf("chunk length at %d non-positive: %d", idx, length)
		}
		total += length
	}
	if total != fixture.InputLength {
		t.Fatalf("chunk lengths sum mismatch: %d vs %d", total, fixture.InputLength)
	}

	for idx := range fixture.ChunkOffsets {
		offset := fixture.ChunkOffsets[idx]
		if offset < 0 {
			t.Fatalf("chunk offset negative at %d: %d", idx, offset)
		}
		if idx == 0 && offset != 0 {
			t.Fatalf("first chunk offset should be zero, got %d", offset)
		}
		if idx > 0 {
			prevEnd := fixture.ChunkOffsets[idx-1] + fixture.ChunkLengths[idx-1]
			if offset != prevEnd {
				t.Fatalf("chunk %d offset mismatch: expected %d got %d", idx, prevEnd, offset)
			}
		}
	}

	inputPath := filepath.Join(fixtureDir(t), "..", "..", "fuzz", "sorafs_chunker", "sf1_profile_v1_input.bin")
	info, err := os.Stat(inputPath)
	if err != nil {
		t.Fatalf("stat input corpus: %v", err)
	}
	if int(info.Size()) != fixture.InputLength {
		t.Fatalf("input corpus size mismatch: %d vs %d", info.Size(), fixture.InputLength)
	}
}

func TestBackpressureCorpusAlignment(t *testing.T) {
	fixture := loadFixture(t)
	path := filepath.Join(fixtureDir(t), "..", "..", "fuzz", "sorafs_chunker", "sf1_profile_v1_backpressure.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read backpressure corpus: %v", err)
	}
	var corpus backpressureCorpus
	if err := json.Unmarshal(raw, &corpus); err != nil {
		t.Fatalf("decode backpressure corpus: %v", err)
	}

	if corpus.Profile != fixture.Profile {
		t.Fatalf("profile mismatch: %s vs %s", corpus.Profile, fixture.Profile)
	}
	if corpus.InputLength != fixture.InputLength {
		t.Fatalf("input length mismatch: %d vs %d", corpus.InputLength, fixture.InputLength)
	}
	if corpus.ChunkDigest != fixture.ChunkDigestSHA3 {
		t.Fatalf("chunk digest mismatch: %s vs %s", corpus.ChunkDigest, fixture.ChunkDigestSHA3)
	}

	for _, scenario := range corpus.Scenarios {
		if scenario.ChunkCount != len(scenario.ExpectedChunkLengths) {
			t.Fatalf("scenario %s chunk count mismatch", scenario.Name)
		}
		if scenario.ChunkCount != fixture.ChunkCount {
			t.Fatalf("scenario %s chunk count mismatch vs fixture", scenario.Name)
		}
		if len(scenario.FeedSizes) == 0 {
			t.Fatalf("scenario %s has no feed sizes", scenario.Name)
		}

		sumFeed := 0
		for _, feed := range scenario.FeedSizes {
			if feed <= 0 {
				t.Fatalf("scenario %s has non-positive feed size %d", scenario.Name, feed)
			}
			sumFeed += feed
		}
		if sumFeed != corpus.InputLength {
			t.Fatalf("scenario %s feed sizes sum mismatch: %d vs %d", scenario.Name, sumFeed, corpus.InputLength)
		}
		if len(scenario.ExpectedChunkLengths) != len(fixture.ChunkLengths) {
			t.Fatalf("scenario %s expected chunk length count mismatch", scenario.Name)
		}
		for idx, length := range scenario.ExpectedChunkLengths {
			if length != fixture.ChunkLengths[idx] {
				t.Fatalf("scenario %s chunk length mismatch at %d: %d vs %d", scenario.Name, idx, length, fixture.ChunkLengths[idx])
			}
		}
		maxFeed := scenario.FeedSizes[0]
		minFeed := scenario.FeedSizes[0]
		for _, feed := range scenario.FeedSizes[1:] {
			if feed > maxFeed {
				maxFeed = feed
			}
			if feed < minFeed {
				minFeed = feed
			}
		}
		if scenario.MaxFeedSize != maxFeed {
			t.Fatalf("scenario %s max feed mismatch: %d vs %d", scenario.Name, scenario.MaxFeedSize, maxFeed)
		}
		if scenario.MinFeedSize != minFeed {
			t.Fatalf("scenario %s min feed mismatch: %d vs %d", scenario.Name, scenario.MinFeedSize, minFeed)
		}
	}
}
