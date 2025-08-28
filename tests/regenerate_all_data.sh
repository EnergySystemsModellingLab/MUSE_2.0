#!/bin/sh

mydir=$(dirname "$0")
cd "$mydir"

echo Building MUSE 2.0
examples=$(cargo run example list 2> /dev/null)

for example in $examples; do
    echo Generating data for example: $example

    # We only need debug files for the simple model
    unset extra_args
    if [ $example = simple ]; then
        extra_args=--debug-model
    fi

    env MUSE2_LOG_LEVEL=off cargo run example run $extra_args -o "data/$example" "$example" 2> /dev/null
done
