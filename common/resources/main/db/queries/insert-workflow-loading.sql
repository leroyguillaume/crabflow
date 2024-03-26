INSERT INTO workflow_loading (id, target)
VALUES ($1, $2)
RETURNING *;
