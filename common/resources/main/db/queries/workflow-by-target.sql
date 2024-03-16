SELECT
    created_at,
    target,
    tag,
    state AS "state: WorkflowState"
FROM workflow
WHERE target = $1;
