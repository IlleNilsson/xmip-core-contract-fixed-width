# xmip-core-contract-fixed-width

The fixed-width content contract, a technology of
[xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

Two claims. **Well-formedness is a given**: the Stream is text.
**Conformance is a given once the contract is named**: a Receive or Send
Location that refers to this contract with a layout bound has every record held
to it, by length and by field, each departure naming the record and the field.

The claims are this crate's when it is called, and the Playground calls it.
A node holding a Location's Streams to it is
[decided, not built](../../../../../doc/architecture/estate-map.md#arrival-validation): a node refuses to start a
Location that names a contract until it does.

The layout language is the COBOL copybook, because that is what fixed-width
files come with. `src/copybook.rs` lists the subset read; a copybook outside it,
binary usage above all, is refused when bound, by name.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
