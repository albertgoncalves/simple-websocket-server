#!/usr/bin/env bash

set -eu

rustfmt src/*.rs
cargo clippy
cargo build
cargo "test"
