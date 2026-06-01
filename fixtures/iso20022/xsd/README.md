# ISO 20022 XSD Fixtures

This directory stores offline XSD fixtures used by the ISO 20022 parser and
rail-profile tests.

The files under `iso/` are Standards Editor generated ISO 20022 schema files
mirrored from the Apache-2.0 licensed `moov-io/fedwire20022` repository:

- `pacs.008.001.08.xsd`
- `pacs.009.001.08.xsd`

Tests use these full XSDs as stable MDR-derived fixtures for namespace,
`Document` root, and live rail profile admission coverage. Runtime validation
does not fetch schemas over the network.
