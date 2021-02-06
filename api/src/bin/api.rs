use dotenv::dotenv;
use std::{env, thread};

fn main() {
    dotenv().ok();

    let port = env::var("API_PORT");
    println!("port: {:?}", port);
    for i in 0..60 {
        thread::sleep(std::time::Duration::from_secs(1));
        println!("{} from api", i);
    }
}
