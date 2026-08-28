FROM --platform=linux/amd64 golang:1.25.7-bookworm@sha256:58259daf0a27c150118663ef7452aa94d66a86d55e73b3443386146623f5364d AS build

ENV CGO_ENABLED=0 \
    GOARCH=amd64 \
    GOAMD64=v1 \
    GOENV=off \
    GOFLAGS=-mod=vendor \
    GOOS=linux \
    GOPROXY=off \
    GOSUMDB=off \
    GOTOOLCHAIN=local \
    GOWORK=off \
    LANG=C \
    LC_ALL=C \
    SOURCE_DATE_EPOCH=1787875200 \
    TZ=UTC

WORKDIR /src
COPY go.mod go.sum ./
COPY vendor ./vendor
COPY cmd ./cmd
COPY internal ./internal
COPY tools ./tools
COPY manifests ./manifests
COPY sbom.spdx.json vendor-inventory-final-v1.json ./

RUN --network=none test "$(go version)" = "go version go1.25.7 linux/amd64" \
 && go test -count=1 -timeout=2h ./... \
 && go run ./tools/metadata inventory --root vendor --output /tmp/vendor-inventory.json \
 && cmp /tmp/vendor-inventory.json vendor-inventory-final-v1.json \
 && go run ./tools/metadata sbom --output /tmp/sbom.spdx.json \
 && cmp /tmp/sbom.spdx.json sbom.spdx.json \
 && mkdir -p /out \
 && go build -trimpath -buildvcs=false -ldflags="-buildid=" -o /out/sccp-circuits ./cmd/sccp-circuits \
 && (cd /out && sha256sum sccp-circuits > sccp-circuits.sha256)

FROM scratch
COPY --from=build /out/ /
COPY --from=build /src/manifests/ /manifests/
COPY --from=build /src/sbom.spdx.json /metadata/sbom.spdx.json
COPY --from=build /src/vendor-inventory-final-v1.json /metadata/vendor-inventory-final-v1.json
COPY builder/policy-final-v1.json /metadata/builder-policy-final-v1.json
ENTRYPOINT ["/sccp-circuits"]
