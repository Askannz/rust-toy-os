use alloc::collections::BTreeMap;

const HISTORY_SIZE: usize = 256; // In number of frames

pub struct SystemStats {

    by_app: BTreeMap<&'static str, [AppDataPoint; HISTORY_SIZE]>,
    system: [SystemDataPoint; HISTORY_SIZE],
    heap_total: usize,

    ring_index: usize,
}

#[derive(Debug, Clone)]
pub struct SystemDataPoint {
    pub net_recv: usize,
    pub net_sent: usize,
    pub heap_usage: usize,
    pub frametime_used: f64,
}

#[derive(Debug, Clone)]
pub struct AppDataPoint {
    pub net_recv: usize,
    pub net_sent: usize,
    pub mem_used: usize,
    pub frametime_used: f64,
}

impl SystemStats {

    pub fn new(heap_total: usize, app_names: &[&'static str]) -> Self {

        let by_app = app_names.iter().map(|app_name| {

            let app_history: [AppDataPoint; HISTORY_SIZE] = core::array::from_fn(|_| AppDataPoint {
                net_recv: 0,
                net_sent: 0,
                mem_used: 0,
                frametime_used: 0.0
            });

            (*app_name, app_history)
        })
        .collect();

        let system_history: [SystemDataPoint; HISTORY_SIZE] = core::array::from_fn(|_| SystemDataPoint {
            net_recv: 0,
            net_sent: 0,
            heap_usage: 0,
            frametime_used: 0.0,
        });

        SystemStats {
            by_app,
            system: system_history,
            heap_total,
            ring_index: 0,
        }
    }

    pub fn next_frame(&mut self) {
        self.ring_index = (self.ring_index + 1) % HISTORY_SIZE;
    }

    pub fn get_system_point_mut(&mut self) -> &mut SystemDataPoint{
        self.system.get_mut(self.ring_index).unwrap()
    }

    pub fn get_app_point_mut(&mut self, app_name: &str) -> &mut AppDataPoint {
        let app_history = self.by_app.get_mut(app_name).expect("Unknown app");
        app_history.get_mut(self.ring_index).unwrap()
    }

    pub fn get_system_history(&self) -> impl Iterator<Item = &SystemDataPoint> {
        get_ring_iterator(&self.system, self.ring_index)
    }

    pub fn get_app_history(&self,  app_name: &str) -> impl Iterator<Item = &AppDataPoint> {
        let app_history = self.by_app.get(app_name).expect("Unknown app");
        get_ring_iterator(app_history, self.ring_index)
    }

}

fn get_ring_iterator<T>(ring_buffer: &[T], ring_index: usize) -> impl Iterator<Item = &T> {

    let ring_size = ring_buffer.len();

    (0..ring_size).map(move |t| {

        let i = match t > ring_index {
            false => ring_index - t,
            true => ring_size - 1 - (t - ring_index),
        };

        &ring_buffer[i]
    })

}

