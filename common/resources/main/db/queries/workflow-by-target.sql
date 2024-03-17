SELECT
    created_at,
    target,
    tag,
    state
FROM workflow
WHERE target = $1;
