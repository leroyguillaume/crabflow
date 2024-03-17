#!/bin/bash

cargo run -p crabflow-$1 -- ${@:2}
