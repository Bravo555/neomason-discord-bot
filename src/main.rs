use dotenv::dotenv;
use rusqlite as db;
use rusqlite::{params, NO_PARAMS};
use serenity::client::Client;
use serenity::model::{channel::Message, gateway::Ready, id::UserId};
use serenity::prelude::*;
use std::{
    env,
    sync::{Arc, Mutex},
};

struct LastMessage;

impl TypeMapKey for LastMessage {
    type Value = Option<Message>;
}

struct DbContainer;

impl TypeMapKey for DbContainer {
    type Value = Arc<Mutex<db::Connection>>;
}

struct SimpleResponses;

impl TypeMapKey for SimpleResponses {
    type Value = Vec<(String, String)>;
}

struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        // TODO: have it ignore all the bots, not only itself
        // probably should use the serenity framework thingy
        if msg.author.id == ctx.cache.read().user.id {
            return;
        }

        println!("got message {}", msg.content);

        {
            let data = ctx.data.read();
            let simple_responses = data.get::<SimpleResponses>().unwrap();
            let message = msg.content.to_lowercase();
            for (keyword, response) in simple_responses {
                if message.contains(keyword) {
                    println!("sending response: {}", response);
                    send_msg(response, &msg, &ctx);
                }
            }
        }

        match msg.content.as_str() {
            "based" => {
                let mut data = ctx.data.write();
                let prev_id = data
                    .get::<LastMessage>()
                    .unwrap()
                    .as_ref()
                    .map(|msg| msg.author.id.clone())
                    .unwrap();

                if prev_id == msg.author.id {
                    send_msg("You can't increase your own based score!", &msg, &ctx);
                    return;
                }

                {
                    let db = data.get_mut::<DbContainer>().unwrap().lock().unwrap();
                    db.execute(
                        "INSERT OR IGNORE INTO users (id, based) VALUES (?1, ?2)",
                        params![*prev_id.as_u64() as i64, 0],
                    )
                    .unwrap();
                    db.execute(
                        "UPDATE users SET based = based + 1 WHERE id = (?1)",
                        params![*prev_id.as_u64() as i64],
                    )
                    .unwrap();
                }

                let prev = data.get::<LastMessage>().unwrap();
                if let Some(prev) = prev {
                    let m = format!(
                        "{} is now more based",
                        prev.author_nick(&ctx).unwrap_or(prev.author.name.clone())
                    );
                    send_msg(&m, &msg, &ctx);
                }
            }
            "!basedstats" => {
                let data = ctx.data.read();
                let db = data.get::<DbContainer>().unwrap().lock().unwrap();

                let mut stmt = db
                    .prepare("SELECT id, based FROM users ORDER BY based DESC")
                    .unwrap();
                let users = stmt
                    .query_map(NO_PARAMS, |row| {
                        let id: i64 = row.get(0).unwrap();
                        let based: i64 = row.get(1).unwrap();

                        Ok((id, based))
                    })
                    .unwrap();

                let mut text = String::from("Based stats:\n");
                for user in users {
                    let user = user.unwrap();
                    let based = user.1;

                    let userid: UserId = (user.0 as u64).into();
                    let user = userid.to_user(&ctx).unwrap();
                    let username = user
                        .nick_in(&ctx, &msg.guild_id.unwrap())
                        .unwrap_or(user.name);
                    let user_txt = format!("{}: {}\n", username, based.to_string());
                    text.push_str(&user_txt);
                }
                send_msg(&text, &msg, &ctx);
            }
            "!list" => {
                let data = ctx.data.read();
                let responses = data.get::<SimpleResponses>().unwrap();

                msg.channel_id
                    .send_message(&ctx, |f| {
                        f.embed(|e| {
                            let description = responses
                                .iter()
                                .map(|(key, resp)| format!("{} => {}", key, resp))
                                .collect::<Vec<String>>()
                                .join("\n");
                            e.description(description)
                        })
                    })
                    .unwrap();
            }
            _ if msg.content.starts_with("!set ") => {
                let body = &msg.content["!set ".len()..];
                set(&ctx, &msg, body);
            }
            _ if !msg.content.starts_with("!") => {
                let mut data = ctx.data.write();
                let last_msg = data.get_mut::<LastMessage>().unwrap();
                last_msg.replace(msg.clone());
            }
            _ => (),
        };
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("discord token missing");
    let db_name = env::var("DB_NAME").expect("db name missing");

    let db = db::Connection::open(db_name).unwrap();
    db.execute(
        "CREATE TABLE IF NOT EXISTS users (
                  id              INTEGER PRIMARY KEY,
                  based           INTEGER NOT NULL
                  )",
        params![],
    )
    .unwrap();

    db.execute(
        "CREATE TABLE IF NOT EXISTS responses (
            keyword     TEXT PRIMARY KEY NOT NULL,
            response    TEXT NOT NULL
        )",
        NO_PARAMS,
    )
    .unwrap();

    let responses: Vec<(String, String)> = {
        let mut responses_query = db.prepare("SELECT * FROM responses").unwrap();
        responses_query
            .query_map(NO_PARAMS, |row| {
                Ok((row.get(0).unwrap(), row.get(1).unwrap()))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    println!("{:?}", responses);

    let mut client = Client::new(&token, Handler).expect("error creating client");

    {
        let mut data = client.data.write();
        data.insert::<DbContainer>(Arc::new(Mutex::new(db)));
        data.insert::<SimpleResponses>(responses);
        data.insert::<LastMessage>(None);
    }

    client
        .start()
        .expect("error occurred while starting the client");
}

fn send_msg(text: &str, msg: &Message, ctx: &Context) {
    if let Err(e) = msg.channel_id.say(&ctx, text) {
        println!("cant send message: {}", e);
    }
}

fn set(ctx: &Context, msg: &Message, body: &str) {
    let send_msg = |text| send_msg(text, &msg, &ctx);

    if body.starts_with("\"") {
        // keyword enclosed with quotation marks
        send_msg("Error: multiple word keywords not yet supported");
        return;
    } else {
        // just split by whitespace
        let mut words = body.split_whitespace();
        let keyword = words.next();
        if let None = keyword {
            send_msg("Error: no keyword\nSyntax: `!set \"some keywords\" a response");
            return;
        };

        let keyword = keyword.unwrap();
        {
            let data = ctx.data.read();
            let responses = data.get::<SimpleResponses>().unwrap();
            if responses.iter().any(|(key, _r)| key == keyword) {
                send_msg("Error: already exists");
                return;
            }
        }
        let response = words.collect::<Vec<&str>>().join(" ");
        if response.len() < 1 {
            send_msg("Error: no response\nSyntax: `!set \"some keywords\" a response");
            return;
        }

        println!("before db\nkeyword: {}\nresponse: {}", keyword, response);
        add_response(&ctx, keyword, &response);

        send_msg(&format!("{} => {} - successfully set", &keyword, &response))
    }
}

fn add_response(ctx: &Context, keyword: &str, response: &str) {
    let mut data = ctx.data.write();
    {
        let db = data.get_mut::<DbContainer>().unwrap().lock().unwrap();
        db.execute(
            "INSERT INTO responses (keyword, response) VALUES (?1, ?2)",
            params![keyword, response],
        )
        .unwrap();
    }

    let responses = data.get_mut::<SimpleResponses>().unwrap();
    responses.push((keyword.to_string(), response.to_string()));
}
