#!/bin/bash

set -e

if [ -z "$DATABASE_URL" ]; then
    export DATABASE_URL=postgres://crabflow:crabflow@127.0.0.1/crabflow
fi

while getopts ":m" opt; do
    case $opt in
        m)
            migrate="true"
            ;;
    esac
done

sqlx database drop -yD $DATABASE_URL
sqlx database create -D $DATABASE_URL

if [ "$migrate" = "true" ]; then
    ./sh/run.sh migrator
fi
