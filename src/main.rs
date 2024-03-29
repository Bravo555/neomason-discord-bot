use dotenvy::dotenv;

use fancy_regex::Regex;
use log::{error, info, warn};
use rand::prelude::*;
use rusqlite::{self as db, params};
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::interaction::Interaction;
use serenity::prelude::*;
use serenity::{
    async_trait,
    client::Client,
    framework::StandardFramework,
    http::{CacheHttp, GuildPagination},
    model::{
        channel::Message,
        gateway::Ready,
        id::{GuildId, UserId},
    },
};
use std::{convert::TryInto, env, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task, time};

use chrono::prelude::*;

type SimpleResponse = (Regex, String, u64);

struct DbContainer;

impl TypeMapKey for DbContainer {
    type Value = Arc<Mutex<db::Connection>>;
}

struct SimpleResponses;

impl TypeMapKey for SimpleResponses {
    type Value = Vec<SimpleResponse>;
}

mod migrations {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // TODO: have it ignore all the bots, not only itself
        // probably should use the serenity framework thingy
        if msg.author.id == ctx.cache.current_user_id() {
            return;
        }

        info!("got message {:?}", msg);

        {
            let data = ctx.data.read().await;
            let simple_responses = data.get::<SimpleResponses>().unwrap();
            info!("{simple_responses:#?}");
            let message = msg.content.to_lowercase();
            info!("{message}");
            for (keyword, response, guildid) in simple_responses {
                if keyword.is_match(&message).unwrap() && msg.guild_id.unwrap().as_u64() == guildid
                {
                    println!("sending response: {response}");
                    send_msg(response, &msg, &ctx).await;
                }
            }
        }

        match msg.content.as_str() {
            "gank" => {
                gank(&ctx, msg).await;
            }
            "based" => {
                based(&ctx, msg).await;
            }
            "!basedstats" => {
                let response = basedstats(&ctx, msg.guild_id.unwrap()).await;
                send_msg(&response, &msg, &ctx).await;
            }
            "!list" => {
                let response = list_responses(&ctx, msg.guild_id.unwrap()).await;
                msg.channel_id.say(&ctx, response).await.unwrap();
            }
            _ if msg.content.starts_with("!set ") => {
                let body = &msg.content["!set ".len()..];
                let guildid = msg.guild_id.unwrap();
                set(&ctx, &msg, body, guildid).await;
            }
            _ => (),
        };
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let data = ctx.data.read().await;
        let simple_responses = data.get::<SimpleResponses>().unwrap();

        let ctx = ctx.clone();
        let guilds = ctx
            .http()
            .get_guilds(Some(&GuildPagination::After(GuildId(0))), Some(10))
            .await
            .unwrap();

        // set up application commands
        for guild in &guilds {
            guild
                .id
                .set_application_commands(&ctx.http, |commands| {
                    commands
                        .create_application_command(|command| {
                            command
                                .name("basedstats")
                                .description("Wypisz najbardziej zbazowanych użytkowników")
                        })
                        .create_application_command(|command| {
                            command
                                .name("listresponses")
                                .description("Wypisz odpowiedzi")
                        }).create_application_command(|command| {
                            command
                                .name("setresp")
                                .description("Dodaj nową odpowiedź")
                                .create_option(|option| {
                                    option
                                        .required(true)
                                        .kind(CommandOptionType::String)
                                        .name("fraza")
                                        .description("Bot odpowie jeżeli zobaczy tą frazę w wiadomości od użytkownika")
                                })
                                .create_option(|option| {
                                    option
                                        .required(true)
                                        .kind(CommandOptionType::String)
                                        .name("odpowiedz")
                                        .description("Bot odpowie tym tekstem")
                                })
                        }).create_application_command(|command| {
                            command
                                .name("delresp")
                                .description("Usuń istniejącą odpowiedź")
                                .create_option(|option| {
                                    let option = option
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                        .name("fraza")
                                        .description("Usuń odpowiedź na tą frazę");

                                    for (regex, _, _) in simple_responses {
                                        let keyword = regex.as_str().replace("\\b", "");
                                        option.add_string_choice(&keyword, regex);
                                    }

                                    option
                                })
                        })
                })
                .await
                .unwrap();
        }

        task::spawn(async move {
            loop {
                let now = Local::now().time();
                if (now.hour(), now.minute()) == (21, 37) {
                    for guild in &guilds {
                        let channels = guild.id.channels(&ctx).await.unwrap();
                        // channel id for inner lodge text wall
                        if let Some(channel) =
                            channels.values().find(|&c| c.name() == "przegryw-nest")
                        {
                            info!("posting message");
                            channel
                                .say(&ctx, "JEST WOLNOŚĆ W PANOPTYKONIE")
                                .await
                                .unwrap();
                        }
                    }
                    time::sleep(Duration::from_secs(2 * 60)).await;
                }
                time::sleep(Duration::from_secs(10)).await;
            }
        });
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "basedstats" => basedstats(&ctx, command.guild_id.unwrap()).await,
                "listresponses" => list_responses(&ctx, command.guild_id.unwrap()).await,
                "setresp" => {
                    let options = &command.data.options;

                    let mut keyword = "";
                    let mut response = "";

                    for option in options {
                        match option.name.as_str() {
                            "fraza" => keyword = option.value.as_ref().unwrap().as_str().unwrap(),
                            "odpowiedz" => {
                                response = option.value.as_ref().unwrap().as_str().unwrap();
                            }
                            _ => (),
                        }
                    }

                    add_response(&ctx, keyword, response, command.guild_id.unwrap()).await;
                    format!("{keyword} => {response} - successfully set")
                }
                "delresp" => {
                    let options = &command.data.options;

                    let keyword = options[0].value.as_ref().unwrap();
                    let keyword = keyword.as_str().unwrap().replace("\\b", "");

                    if remove_response(&ctx, &keyword, command.guild_id.unwrap())
                        .await
                        .is_some()
                    {
                        format!("{keyword} - successfully removed")
                    } else {
                        format!("{keyword} - taka fraza nie istnieje")
                    }
                }
                command => unreachable!("Unknown command: {}", command),
            };

            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                error!("Cannot respond to slash command: {}", why);
            }
        }
    }
}

const DB_NAME_DEFAULT: &str = "neomason.db";

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN env variable missing");
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| {
        warn!("DB name not provided, using default: {}", DB_NAME_DEFAULT);
        DB_NAME_DEFAULT.into()
    });

    let mut db = db::Connection::open(db_name).unwrap();
    migrations::migrations::runner().run(&mut db).unwrap();

    let responses: Vec<SimpleResponse> = {
        let mut responses_query = db
            .prepare("SELECT keyword, response, guildid FROM responses")
            .unwrap();
        responses_query
            .query_map([], |row| {
                Ok((
                    row.get::<usize, String>(0).unwrap(),
                    row.get::<usize, String>(1).unwrap(),
                    row.get::<usize, i64>(2).unwrap().try_into().unwrap(),
                ))
            })
            .unwrap()
            .map(|response| {
                response.map(|(k, r, id)| (Regex::new(&format!(r"\b{}\b", &k)).unwrap(), r, id))
            })
            .filter_map(std::result::Result::ok)
            .collect()
    };
    info!("loaded responses: {:?}", responses);

    let framework = StandardFramework::new();

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<DbContainer>(Arc::new(Mutex::new(db)));
        data.insert::<SimpleResponses>(responses);
    }

    client
        .start()
        .await
        .expect("error occurred while starting the client");
}

async fn send_msg(text: &str, msg: &Message, ctx: &Context) {
    if let Err(e) = msg.reply(&ctx, text).await {
        println!("cant send message: {e}");
    }
}

async fn set(ctx: &Context, msg: &Message, body: &str, guildid: GuildId) {
    let send_msg = |text| send_msg(text, msg, ctx);

    let (keyword, i) = if body.starts_with('\"') {
        // keyword enclosed with quotation marks
        let reg = Regex::new("^\"(.*)\"").unwrap();
        let captures = reg.captures(body).unwrap().unwrap();
        let keyword = captures.get(1).unwrap().as_str();

        (keyword, keyword.len() + 3)
    } else {
        let mut words = body.split_whitespace();
        // just split by whitespace
        let keyword = words.next();
        if keyword.is_none() {
            send_msg("Error: no keyword\nSyntax: `!set \"some keywords\" a response").await;
            return;
        };
        let keyword = keyword.unwrap();

        (keyword, keyword.len() + 1)
    };
    {
        let data = ctx.data.read().await;
        let responses = data.get::<SimpleResponses>().unwrap();
        if responses.iter().any(|(key, _r, current_guildid)| {
            key.as_str() == keyword && guildid.as_u64() == current_guildid
        }) {
            send_msg("Error: already exists").await;
            return;
        }
    }
    let response = &body[i..].to_string();

    add_response(ctx, keyword, response, guildid).await;
    send_msg(&format!("{} => {} - successfully set", &keyword, &response)).await;
}

async fn list_responses(ctx: &Context, guildid: GuildId) -> String {
    let data = ctx.data.read().await;
    let responses = data.get::<SimpleResponses>().unwrap();

    let text = responses
        .iter()
        .filter(|(_, _, g)| guildid == *g)
        .map(|(key, resp, _)| {
            format!(
                "{} => {}",
                key.as_str()
                    .strip_prefix("\\b")
                    .and_then(|k| k.strip_suffix("\\b"))
                    .unwrap_or(""),
                resp
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let text = if let "" = text.as_str() {
        "No responses yet! Add some using `!set` command".to_string()
    } else {
        text
    };

    text
}

async fn add_response(ctx: &Context, keyword: &str, response: &str, guildid: GuildId) {
    let mut data = ctx.data.write().await;
    {
        let db = data.get_mut::<DbContainer>().unwrap().lock().await;
        db.execute(
            "INSERT INTO responses (keyword, response, guildid) VALUES (?1, ?2, ?3)",
            params![keyword, response, i64::from(guildid)],
        )
        .unwrap();
    }

    let responses = data.get_mut::<SimpleResponses>().unwrap();
    responses.push((
        Regex::new(&format!(r"\b{keyword}\b")).unwrap(),
        response.to_string(),
        guildid.into(),
    ));
}

async fn remove_response(ctx: &Context, keyword: &str, guildid: GuildId) -> Option<()> {
    let mut data = ctx.data.write().await;
    {
        let db = data.get_mut::<DbContainer>().unwrap().lock().await;
        let res = db
            .execute(
                "DELETE FROM responses WHERE keyword = ?1 AND guildid = ?2",
                params![keyword, i64::from(guildid)],
            )
            .unwrap();
        info!("deleted {res} rows");
    }

    let responses = data.get_mut::<SimpleResponses>().unwrap();

    let keyword = format!("\\b{keyword}\\b");
    if let Some(pos) = responses
        .iter()
        .position(|(regex, _, _)| regex.as_str() == keyword)
    {
        responses.remove(pos);
        Some(())
    } else {
        None
    }
}

async fn based(ctx: &Context, msg: Message) {
    let mut data = ctx.data.write().await;
    let target = if msg.mentions.len() == 1 {
        msg.mentions.get(0).cloned()
    } else if let Some(ref ref_msg) = msg.message_reference {
        ref_msg
            .channel_id
            // should never return `None` because we're checking for it
            .message(&ctx, ref_msg.message_id.unwrap())
            .await
            .map(|m| m.author)
            .ok()
    } else {
        return;
    }
    .unwrap()
    .clone();

    if target.id == msg.author.id {
        send_msg("You can't increase your own based score!", &msg, ctx).await;
        return;
    }

    let mut rows = {
        let db = data.get_mut::<DbContainer>().unwrap().lock().await;
        let guildid = msg.guild_id.unwrap();
        // TODO: replace with `returning` clause once server has appropriate sqlite version
        let increase_score_result = db.execute(
                        "INSERT INTO users (userid, guildid, based) VALUES (?1, ?2, ?3) ON CONFLICT(userid, guildid) DO UPDATE SET based = based + 1 WHERE userid = (?1) AND guildid = (?2)",
                        params![i64::from(target.id), i64::from(guildid), 1],
                    );
        if let Err(err) = increase_score_result {
            send_msg(
                &format!("Error: can't increase based score\nReason:{err:?}"),
                &msg,
                ctx,
            )
            .await;
            return;
        }

        let mut stmt = db
            .prepare("SELECT based FROM users WHERE userid = (?1) AND guildid = (?2)")
            .unwrap();

        let rows = stmt
            .query_and_then(params![i64::from(target.id), i64::from(guildid)], |r| {
                r.get::<usize, i64>(0)
            })
            .unwrap();

        rows.collect::<Vec<_>>()
    };

    let based = if let Some(s) = rows.pop() {
        match s {
            Ok(res) => res,
            Err(err) => {
                send_msg(
                    &format!("Error: error retrieving based count! reason: {err:?}"),
                    &msg,
                    ctx,
                )
                .await;
                return;
            }
        }
    } else {
        send_msg("Error: user not found", &msg, ctx).await;
        return;
    };

    let nick = target
        .nick_in(&ctx, msg.guild_id.unwrap())
        .await
        .unwrap_or(target.name);

    let m = format!("{nick} is now more based. Their based score is now: {based}");
    send_msg(&m, &msg, ctx).await;
}

async fn basedstats(ctx: &Context, guildid: GuildId) -> String {
    let data = ctx.data.read().await;
    let db = data.get::<DbContainer>().unwrap().lock().await;
    let users = {
        let mut stmt = db
            .prepare("SELECT userid, based FROM users WHERE guildid = (?1) ORDER BY based DESC")
            .unwrap();
        let users = stmt
            .query_map(params![i64::from(guildid)], |row| {
                let id: i64 = row.get(0).unwrap();
                let based: i64 = row.get(1).unwrap();

                Ok((id, based))
            })
            .unwrap()
            .collect::<Vec<_>>();
        users
    };

    let mut text = String::from("Based stats:\n");
    for user in users {
        let user = user.unwrap();
        let based = user.1;

        let userid: UserId = (user.0 as u64).into();
        let user = userid.to_user(&ctx).await.unwrap();
        let username = user.nick_in(&ctx, guildid).await.unwrap_or(user.name);
        let user_txt = format!("{username}: {based}\n");
        text.push_str(&user_txt);
    }
    text
}

async fn gank(ctx: &Context, msg: Message) {
    let channels = msg.guild_id.unwrap().channels(&ctx).await.unwrap();
    let gank_channel = channels
        .values()
        // TODO: FIX THE GODDAMN HARDCODED CONFIG
        .find(|channel| channel.name() == "national-gank-bureau")
        .unwrap();

    let images = gank_channel
        .messages(&ctx, |retriever| retriever)
        .await
        .unwrap()
        .into_iter()
        .filter(|message| !message.attachments.is_empty())
        .choose(&mut thread_rng())
        .unwrap()
        .attachments;

    // need this contraption to work around "implementation of `FnOnce` is not general enough" bug.
    // this is a bug in lifetime inference of the compiler, probably it can't infer that str that comes out of the
    // closure has the same lifetime as the attachment, so the closure parameter
    // Hopefully this can be removed once the bug is fixed.
    use serenity::model::channel::Attachment;
    fn closure(a: &Attachment) -> &str {
        a.url.as_str()
    }

    let attachments = images.iter().map(closure);

    msg.channel_id
        .send_files(&ctx, attachments, |a| a)
        .await
        .unwrap();
}
