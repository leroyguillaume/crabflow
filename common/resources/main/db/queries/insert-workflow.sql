INSERT INTO workflow (target, tag)
VALUES ($1, $2)
RETURNING
    created_at,
    target,
    tag,
    state;
