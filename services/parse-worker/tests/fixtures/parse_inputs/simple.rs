pub struct Config {
    pub host: String,
    pub port: u16,
}

pub fn connect(cfg: Config) -> String {
    format!("{}:{}", cfg.host, cfg.port)
}

pub const DEFAULT_PORT: u16 = 8080;

pub enum Transport {
    Tcp,
    Udp,
}
