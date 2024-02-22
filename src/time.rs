use std::time::SystemTime;

pub fn get_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| panic!("Time before unix epoch; Time travelers have better things to do then play Bi games"))
        .as_secs()
}