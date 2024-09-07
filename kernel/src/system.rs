use rand::rngs::SmallRng;
use crate::{network::TcpStack, time::SystemClock, wasm::WasmEngine};

pub struct System {
    pub clock: SystemClock,
    pub tcp_stack: TcpStack,
    pub rng: SmallRng,
}
