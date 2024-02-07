#!/usr/bin/env bash

set -eu

rustfmt src/*.rs
cargo clippy --all --all-targets -- \
    -W clippy::all \
    -W clippy::complexity \
    -W clippy::correctness \
    -W clippy::nursery \
    -W clippy::pedantic \
    -W clippy::perf \
    -W clippy::suspicious \
    -A clippy::cast_possible_truncation \
    -A clippy::derive_partial_eq_without_eq \
    -A dead_code \
    -D warnings
cargo "test"
