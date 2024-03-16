INSERT INTO workflow (target, image)
VALUES ($1, $2)
RETURNING
    created_at,
    target,
    image,
    state AS "state: WorkflowState";
