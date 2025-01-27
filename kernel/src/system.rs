use crate::{network::TcpStack, time::SystemClock};
use rand::rngs::SmallRng;
use applib::StyleSheet;
use crate::stats::SystemStats;

pub struct System {
    pub clock: SystemClock,
    pub tcp_stack: TcpStack,
    pub rng: SmallRng,
    pub stylesheet: &'static StyleSheet,
    pub stats: SystemStats,
}
