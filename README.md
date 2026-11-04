# Chainlink Data Streams Solana

This repo contains the on-chain programs + utilities for Chainlink Data Streams on Solana.

## Programs
### Verifier
The on-chain program used to verify data streams DON reports. For more information see the [verifier program documentation](programs/verifier/README.md).


## Developing
This project uses the below tools:
```text
solana-cli 1.18.26
anchor 0.29.0
rustc 1.82.0
```

`anchor build` is for development and tests. Release binaries are built with `solana-verify`, see below.

## Building a release
Programs deployed to devnet or mainnet are built with [solana-verify](https://github.com/Ellipsis-Labs/solana-verifiable-build),
which compiles inside a pinned Docker image so anyone can rebuild a commit and get the same binary.

Requires Docker and `solana-verify` (tested with 0.5.2). Installing `solana-verify` from source needs Rust 1.85 or
newer, newer than this repo's toolchain. That only affects building the tool: the program itself is compiled by the
Solana platform tools inside the image.
```bash
cargo +1.91 install solana-verify --locked
```

Build from a tagged commit:
```bash
git checkout <release tag>
solana-verify build --library-name verifier
```
The binary is `target/deploy/verifier.so` and the last line of output is its hash; publish that hash with the
release. `solana-verify` hashes the program without its trailing zero bytes, so the hash differs from `sha256sum` of
the file. Rebuilding the same commit gives the same hash.

The program ID is compiled in (`declare_id!` in `programs/verifier/src/lib.rs`), so a build for another deployment,
such as staging, needs that line changed first. Those builds are reproducible with the same change, but can't be
verified against a public commit.

## Generate Go bindings
Generate the bindings if contract interface is changed

```bash          
go install github.com/gagliardetto/anchor-go@v0.3.1
./client/scripts/anchor-go-gen.sh``
```

