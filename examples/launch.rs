use dotenvy::dotenv;
use env_logger::Builder;
use roblox_rs::account::{Account, LaunchData};
use std::env;

pub fn init_colored() {
    Builder::from_default_env()
        .format(|buf, record| {
            use std::io::Write;

            let level = record.level();
            let level_style = buf.default_level_style(level);

            writeln!(
                buf,
                "[{} {}{}{:?}] {}",
                chrono::Local::now().format("%H:%M:%S%.3f"),
                level_style,
                level,
                ansi_term::Style::default(),
                record.args()
            )
        })
        .init();
}

#[tokio::main]
async fn main() {
    dotenv().unwrap();

    init_colored();

    let token = env::var("ROBLOX_TOKEN").unwrap();

    let mut acc = Account::new(token).await.unwrap();

    acc.launch(
        LaunchData::builder()
            .place_id(109983668079237)
            .private_code(
                "https://www.roblox.com/share?code=8f4cf5034023684c858b38e72dc8a153&type=Server"
                    .to_string(),
            )
            .build(),
    )
    .await
    .unwrap();

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
