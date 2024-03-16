CREATE TYPE workflow_state AS ENUM (
    'created',
    'loaded',
    'loading'
);

CREATE TABLE workflow (
    id VARCHAR (255) NOT NULL PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    target VARCHAR (255) NOT NULL UNIQUE,
    image VARCHAR (255) NOT NULL UNIQUE,
    state workflow_state NOT NULL DEFAULT 'created'
);
