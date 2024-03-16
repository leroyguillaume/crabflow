UPDATE workflow
SET
    image = $2,
    state = $3
WHERE
    target = $1
    AND state != 'loading';
