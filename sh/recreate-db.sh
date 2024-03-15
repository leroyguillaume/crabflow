#!/bin/bash

export DATABASE_URL=$(grep DATABASE_URL .cargo/config.toml | cut -d = -f 2 | cut -d '"' -f 2)

sqlx database drop -yD $DATABASE_URL
sqlx database create -D $DATABASE_URL
sqlx migrate run -D $DATABASE_URL --source common/resources/main/db/migrations
