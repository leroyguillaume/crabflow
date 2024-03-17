UPDATE workflow
SET
    tag = $2,
    state = $3,
    descriptor = $4
WHERE
    target = $1
    AND state != 'loading';
