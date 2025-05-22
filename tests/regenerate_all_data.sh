#!/bin/sh

mydir=$(dirname "$0")
cd "$mydir"

echo Building MUSE 2.0
examples=$(cargo run example list 2> /dev/null)

for example in $examples; do
    echo Generating data for example: $example
    env MUSE2_LOG_LEVEL=off cargo run example run --debug-model -o "data/$example" "$example" 2> /dev/null
done
