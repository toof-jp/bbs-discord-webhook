use std::{collections::HashMap, env, fmt, thread::sleep, time::Duration};

use anyhow::Result;
use chrono::NaiveDateTime;
use dotenv::dotenv;
use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let webhook_url = env::var("WEBHOOK_URL").expect("WEBHOOK_URL must be set");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    let mut max_no = if std::path::Path::new("res_no").exists() {
        std::fs::read_to_string("res_no")
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
            post(&webhook_url, &res.to_string()).await?;
            max_no = res.no;
            std::fs::write("res_no", max_no.to_string()).unwrap();
            sleep(Duration::from_millis(500));
        }

        sleep(Duration::from_secs(1));
    }

    Ok(())
}

async fn post(webhook_url: &str, content: &str) -> Result<()> {
    let mut map = HashMap::new();
    map.insert("content", content);

    reqwest::Client::new()
        .post(webhook_url)
        .json(&map)
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
