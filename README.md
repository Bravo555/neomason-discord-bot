# neomason-discord-bot
A bot for a friend's Discord server (Neo-Masonic Internationale)

## What does it do?
Currently, the limited functionality the bot offers is:
- responding with a predefined response when a specified keyword is detected in a message sent by a user (can add responses by using `!set` command, eg. `!set keyword A response.`)
- keeping track of "based" score for users. A user's "based" score is increased when in response to a message sent by the user, another user responds with `based`

## How to run?
A configuration of the bot is kept in environment variables, as per [12 factor app's recommendations](https://12factor.net/).

- **DISCORD_TOKEN**: a discord token for the bot
- **DB_NAME**: a path to the SQLite3 database (will be created if it doesn't exist)

Additionally, user has to have SQLite dev libraries installed.
