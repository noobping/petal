#[derive(Clone, Copy, Debug)]
pub enum Station {
    Jpop,
    Kpop,
}

impl Station {
    pub fn stream_url(self) -> &'static str {
        match self {
            Station::Jpop => "https://listen.moe/stream",
            Station::Kpop => "https://listen.moe/kpop/stream",
        }
    }

    pub fn ws_url(self) -> &'static str {
        match self {
            Station::Jpop => "wss://listen.moe/gateway_v2",
            Station::Kpop => "wss://listen.moe/kpop/gateway_v2",
        }
    }
}
