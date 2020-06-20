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

struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        // TODO: have it ignore all the bots, not only itself
        // probably should use the serenity framework thingy
        if msg.author.id == 722854871174479892 {
            return;
        }

        if msg.content != "!prev" && msg.content != "based" {
            let mut data = ctx.data.write();
            let last_msg = data.get_mut::<LastMessage>().unwrap();
            last_msg.replace(msg.clone());
        }

        println!("got message {}", msg.content);

        let simple_responses = vec![
            ("karakan", "jebany kaczyński"),
            ("kraj z gówna", "ta kurwa polska"),
        ];
        for resp in simple_responses {
            if msg.content.contains(resp.0) {
                send_msg(resp.1, &msg, &ctx);
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

                let mut stmt = db.prepare("SELECT id, based FROM users").unwrap();
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

    let mut client = Client::new(&token, Handler).expect("error creating client");
    println!("{:?}", client.cache_and_http.cache.read().users);

    {
        let mut data = client.data.write();
        data.insert::<DbContainer>(Arc::new(Mutex::new(db)));
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
