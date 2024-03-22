CREATE TYPE workflow_state AS ENUM (
    'created',
    'loaded',
    'loading'
);

CREATE TABLE workflow (
    target VARCHAR (255) NOT NULL UNIQUE PRIMARY KEY,
    tag VARCHAR (255) NOT NULL UNIQUE,
    state workflow_state NOT NULL DEFAULT 'created',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    descriptor JSONB DEFAULT NULL
);
