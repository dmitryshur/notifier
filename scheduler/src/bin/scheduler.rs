use std::thread;

fn main() {
    for i in 0..60 {
        thread::sleep(std::time::Duration::from_secs(2));
        println!("{} from scheduler", i);
    }
}
