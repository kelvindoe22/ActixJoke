pub mod database {
    use postgres::Client;
    use rand::{self, Rng};

    pub fn query(client: &mut Client) -> Option<(String, String)> {
        let random_num = rand::thread_rng().gen_range(1..=get_last_num(client).expect("Empty db"));

        let mut joke = String::new();
        let mut author = String::new();

        for row in client
            .query(
                &*format!(
                    "SELECT joke,author FROM bad_jokes WHERE id = {}",
                    random_num
                ),
                &[],
            )
            .unwrap()
        {
            joke = String::from(row.get::<_, &str>(0));
            author = String::from(row.get::<_, &str>(1));
        }

        if joke.is_empty() || author.is_empty() {
            None
        } else {
            Some((joke, author))
        }
    }

    fn get_last_num(cl: &mut Client) -> Option<i32> {
        let mut val = None;
        for row in cl
            .query("SELECT * FROM bad_jokes ORDER BY id DESC LIMIT 1;", &[])
            .unwrap()
        {
            val = Some(row.get::<_, i32>(0));
        }
        val
    }
}

pub mod datamodels {
    use serde::{Deserialize, Serialize};
    use tera::Tera;

    #[derive(Deserialize, Serialize)]
    pub struct Joke {
        pub author: String,
        pub joke: String,
    }

    impl Joke {
        pub fn sqli(&mut self) {
            self.author = Self::new_string(self.author.chars());
            self.joke = Self::new_string(self.joke.chars());
        }

        fn new_string(chars: std::str::Chars) -> String {
            let mut new_string = String::new();
            chars.for_each(|c| {
                if c as u8 == 39 {
                    new_string.push_str("''");
                } else {
                    new_string.push(c);
                }
            });

            new_string
        }
    }

    #[derive(Deserialize, Serialize)]
    pub struct Status {
        pub status: String,
        pub message: String,
    }

    #[derive(Deserialize, Serialize)]
    pub struct Login {
        pub pass: String,
    }

    pub struct Appdata {
        pub tera: Tera,
    }
}

#[test]
fn de() {
    dotenv::dotenv().ok();

    let k = std::env::var("creds").unwrap();
    println!("{k}");
}
