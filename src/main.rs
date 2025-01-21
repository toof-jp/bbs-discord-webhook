use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use std::{env, fmt};

use anyhow::Result;
use chrono::NaiveDateTime;
use dotenvy::dotenv;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let webhook_url = env::var("WEBHOOK_URL").expect("WEBHOOK_URL must be set");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let image_url_prefix = env::var("IMAGE_URL_PREFIX").expect("IMAGE_URL_PREFIX must be set");

    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    let mut max_no = if Path::new("res_no").exists() {
        read_to_string("res_no")
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
    } else {
        get_max_res_no(&pool).await?
    };

    loop {
        let vec = get_res(&pool, max_no).await?;

        for res in vec {
            post(&webhook_url, &res, &image_url_prefix).await?;
            eprintln!("posted: {}", res.no);
            max_no = res.no;
            std::fs::write("res_no", format!("{}\n", max_no)).unwrap();
            sleep(Duration::from_millis(500));
        }

        sleep(Duration::from_secs(1));
    }

    Ok(())
}

async fn post(webhook_url: &str, res: &Res, image_url_prefix: &str) -> Result<()> {
    #[derive(Serialize)]
    struct DiscordEmbed {
        image: HashMap<String, String>,
    }

    #[derive(Serialize)]
    struct DiscordWebhook {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        embeds: Option<Vec<DiscordEmbed>>,
    }

    let webhook_data = DiscordWebhook {
        content: res.to_string(),
        embeds: res.oekaki_id.map(|oekaki_id| {
            vec![DiscordEmbed {
                image: HashMap::from([
                    ("url".to_string(), format!("{}{}.png", image_url_prefix, oekaki_id))
                ]),
            }]
        }),
    };

    reqwest::Client::new()
        .post(webhook_url)
        .json(&webhook_data)
        .send()
        .await?;

    Ok(())
}

async fn get_max_res_no(pool: &PgPool) -> Result<i32> {
    let result = sqlx::query!("SELECT MAX(no) FROM res")
        .fetch_one(pool)
        .await?;

    Ok(result.max.unwrap_or(0))
}

#[derive(Debug, Default, Serialize)]
pub struct Res {
    pub no: i32,
    pub name_and_trip: String,
    pub datetime: NaiveDateTime,
    pub datetime_text: String,
    pub id: String,
    pub main_text: String,
    pub main_text_html: String,
    pub oekaki_id: Option<i32>,
}

impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "### __{} {} {} ID: {}__\n{}\n",
            self.no, self.name_and_trip, self.datetime_text, self.id, self.main_text
        )
    }
}

async fn get_res(pool: &PgPool, cursor: i32) -> Result<Vec<Res>> {
    sqlx::query_as!(
        Res,
        r#"
            SELECT *
            FROM res
            WHERE no > $1
            ORDER BY no ASC
        "#,
        cursor
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
