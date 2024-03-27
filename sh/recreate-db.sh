#!/bin/bash

set -e

if [ -z "$DATABASE_URL" ]; then
    export DATABASE_URL=postgres://crabflow:crabflow@127.0.0.1/crabflow
fi

sqlx database drop -yD $DATABASE_URL
sqlx database create -D $DATABASE_URL
./sh/run.sh migrator
