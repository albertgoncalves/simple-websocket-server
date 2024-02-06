#!/usr/bin/env bash

# NOTE: See `https://autobahn.readthedocs.io/en/latest/contents.html`.

set -eu

(
    mkdir -p report/
    cd ../
    docker run -it --rm -v "$PWD"/autobahn:/autobahn --network host \
        crossbario/autobahn-testsuite wstest -m fuzzingclient \
        -s autobahn/config.json
)
