UPDATE workflow
SET
    image = $2,
    loaded = $3
WHERE target = $1;
