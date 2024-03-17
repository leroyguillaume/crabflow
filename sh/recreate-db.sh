#!/bin/bash

set -e

export DATABASE_URL=postgres://crabflow:crabflow@127.0.0.1/crabflow

sqlx database drop -yD $DATABASE_URL
sqlx database create -D $DATABASE_URL
sqlx migrate run -D $DATABASE_URL --source common/resources/main/db/migrations
