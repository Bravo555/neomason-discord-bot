ALTER TABLE users
    RENAME TO users_;
ALTER TABLE responses
    RENAME TO responses_;
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    userid INTEGER NOT NULL,
    guildid INTEGER NOT NULL,
    based INTEGER NOT NULL,
    CONSTRAINT user_guild UNIQUE(userid, guildid)
);
CREATE TABLE responses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guildid INTEGER NOT NULL,
    keyword TEXT NOT NULL,
    response TEXT NOT NULL,
    CONSTRAINT keyword_response UNIQUE(keyword, response)
);
INSERT INTO users(userid, guildid, based)
SELECT id,
    223923153691344897,
    based
FROM users_;
INSERT INTO responses(guildid, keyword, response)
SELECT 223923153691344897,
    keyword,
    response
FROM responses_;
DROP TABLE users_;
DROP TABLE responses_;