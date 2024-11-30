use alloc::collections::BTreeMap;

const HISTORY_SIZE: usize = 256; // In number of frames

pub struct SystemStats {

    by_app: BTreeMap<&'static str, [AppDataPoint; HISTORY_SIZE]>,
    system: [SystemDataPoint; HISTORY_SIZE],
    pub heap_total: usize,

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

    pub fn get_system_history<T, F>(&self, selector: F) -> [T; HISTORY_SIZE]

        where F: Fn(&SystemDataPoint) -> T
    {
        core::array::from_fn(|t| {
            let dp = get_history_point(&self.system, self.ring_index, t);
            selector(dp)
        })

    }

    pub fn get_app_history<T, F>(&self,  app_name: &str, selector: F) -> [T; HISTORY_SIZE]
    
        where F: Fn(&AppDataPoint) -> T
    {

        let app_history = self.by_app.get(app_name).expect("Unknown app");

        core::array::from_fn(|t| {
            let dp = get_history_point(app_history, self.ring_index, t);
            selector(dp)
        })

    }

}

fn get_history_point<T>(ring_buffer: &[T], ring_index: usize, t: usize) -> &T {

    let ring_size = ring_buffer.len();

    let i = match t <= ring_index {
        true => ring_index - t,
        false => ring_size - (t - ring_index),
    };

    &ring_buffer[i]

}

fn get_ring_iterator<T>(ring_buffer: &[T], ring_index: usize) -> impl Iterator<Item = &T> {

    let ring_size = ring_buffer.len();

    (0..ring_size).map(move |t| {

        let i = match t <= ring_index {
            true => ring_index - t,
            false => ring_size - (t - ring_index),
        };

        &ring_buffer[i]
    })

}

