# xmip-core-contract-fixed-width

The fixed-width content contract, a technology of
[xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

Two claims. **Well-formedness is a given**: the Stream is text.
**Conformance is a given once the contract is named**: a Receive or Send
Location that refers to this contract with a layout bound has every record held
to it, by length and by field, each departure naming the record and the field.

The claims are this crate's when it is called, and the System Tests call it.
A node holding a Location's Streams to it is
[decided, not built](../../../../../doc/architecture/estate-map.md#arrival-validation): a node refuses to start a
Location that names a contract until it does.

The layout language is the COBOL copybook, because that is what fixed-width
files come with. `src/copybook.rs` lists the subset read; a copybook outside it,
binary usage above all, is refused when bound, by name.

## The declaration

What a Location naming this contract is configured with is two `const`s in
its `declaration` module, `src/declaration.rs`, beside the code, which its
`ContractFactory` reads (ADR-0064, amendment 2026-11-26):
`declaration::SETTINGS`, `reference`, the path of the copybook records are
held to, and `NAME`, `fixed-width`, the name a Validate step's `format`
names it by. Built with its `declaration` feature alone
(`--no-default-features --features declaration`), the crate is those and
nothing more: its code and every crate the code is built on are behind the
default feature. So the library the surfaces load can carry it without the
code, for the language server and the desktop to suggest and check a
Location's contract settings though no node in their process loaded the
technology, once the gathering crate of open problem 48 links it.
`test/Declaration.Test.ps1` in the estate holds that the declaration builds
nothing the contract capability does not.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
