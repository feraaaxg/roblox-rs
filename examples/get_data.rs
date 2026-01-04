use std::env;

use roblox_rs::account::Account;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();

    let token = env::var("ROBLOX_TOKEN").unwrap();

    let acc = Account::new(token).await.unwrap();

    let private_links = acc.get_my_private_servers().await.unwrap();

    println!("{:#?}", &private_links);

    let private_server_details = acc
        .get_private_server_details(private_links.get(1).unwrap().private_server_id)
        .await
        .unwrap();

    println!("{:#?}", private_server_details);
}
