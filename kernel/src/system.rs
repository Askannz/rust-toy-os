use rand::rngs::SmallRng;
use crate::{network::TcpStack, time::SystemClock};

pub struct System {
    pub clock: SystemClock,
    pub tcp_stack: TcpStack,
    pub rng: SmallRng,
}
