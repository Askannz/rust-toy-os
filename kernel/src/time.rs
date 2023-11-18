use uefi::prelude::RuntimeServices;

pub struct SystemClock {
    period_s: f64,
}

impl SystemClock {
    pub fn new(runtime_services: &RuntimeServices) -> Self {

        // Waiting for the "seconds" value to change
        let s1 = runtime_services.get_time().unwrap().second();
        let s2 = loop {
            let s2 = runtime_services.get_time().unwrap().second();
            if s1 != s2 { break s2 }
        };

        // Waiting approximately one second and measuring the change in rdtsc
        let n1 = unsafe { core::arch::x86_64::_rdtsc()};
        loop {
            let s3 = runtime_services.get_time().unwrap().second();
            if s2 != s3 { break; }
        };
        let n2 = unsafe { core::arch::x86_64::_rdtsc()};

        let period_s = 1.0 / ((n2 - n1) as f64);
        let freq_ghz = 1.0 / period_s / 1E9;
        log::debug!(
            "CPU frequency estimated to {:.3}Ghz ({} cycles in approx 1s)",
            freq_ghz, n2 - n1
        );

        SystemClock { period_s }
    }

    pub fn time(&self) -> f64 {  // in milliseconds
        let n = unsafe { core::arch::x86_64::_rdtsc()};
        1000f64 * (n as f64) * self.period_s
    }

    pub fn spin_delay(&self, duration: f64) {
        let t0 = self.time();
        while self.time() - t0 < duration {}
    }
}
