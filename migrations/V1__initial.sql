CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, based NOT NULL);
CREATE TABLE IF NOT EXISTS responses (
    keyword TEXT PRIMARY KEY NOT NULL,
    response TEXT NOT NULL
)