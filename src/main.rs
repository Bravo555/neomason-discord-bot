use std::{env, collections::HashMap};
use serenity::client::Client;
use serenity::model::{channel::Message, gateway::Ready, id::UserId};
use serenity::prelude::{EventHandler, Context};
use serenity::prelude::*;

struct LastMessage;

impl TypeMapKey for LastMessage {
    type Value = Option<Message>;
}

/// Keep each user's "based" score (how based they are)
struct BasedStats;

impl TypeMapKey for BasedStats {
    type Value = HashMap<UserId, u32>;
}

struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
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
            ("karakan", "jebany kaczyński"), ("kraj z gówna", "ta kurwa polska")
        ];
        for resp in simple_responses {
            if msg.content.contains(resp.0) {
                send_msg(resp.1, &msg, &ctx);
            }
        }

        {
            let mut data = ctx.data.write();
            match msg.content.as_str() {
                "!prev" => {
                    let prev_content = data.get::<LastMessage>().unwrap().as_ref().unwrap().content.clone();
                    let m = format!("previous message was: {}", prev_content);
                    send_msg(&m, &msg, &ctx);
                },
                "based" => {
                    let based_map = data.get_mut::<BasedStats>().unwrap();
                    let entry = based_map.entry(msg.author.id).or_insert(0);
                    *entry += 1;
                    let prev = data.get::<LastMessage>().unwrap().as_ref().unwrap();
                    let m = format!("{} is now more based", prev.author.name);
                    send_msg(&m, &msg, &ctx);
                }
                _ => () 
            };
        }
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

fn main() {
    let mut client = Client::new(&env::var("DISCORD_TOKEN").expect("token"), Handler)
        .expect("error creating client");

    {
        let mut data = client.data.write();
        data.insert::<LastMessage>(None);
        data.insert::<BasedStats>(HashMap::default());
    }

    client.start().expect("error occurred while starting the client");
}

fn send_msg(text: &str, msg: &Message, ctx: &Context) {
    if let Err(e) = msg.channel_id.say(&ctx.http, text) {
        println!("cant send message: {}", e);
    }
}