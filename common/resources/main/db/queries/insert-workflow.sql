INSERT INTO workflow (target, image)
VALUES ($1, $2)
RETURNING
    created_at AS workflow_created_at,
    target AS workflow_target,
    image AS workflow_image,
    loaded AS workflow_loaded;
