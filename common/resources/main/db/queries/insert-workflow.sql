INSERT INTO workflow (target, tag)
VALUES ($1, $2)
RETURNING *;
