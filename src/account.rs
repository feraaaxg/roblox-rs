use reqwest::Client;

struct Account {
    token: String,
    client: Client,
}

impl Account {
    pub fn new<T>(token: T) -> Self
    where
        T: Into<String>,
    {
        todo!()
    }
}
