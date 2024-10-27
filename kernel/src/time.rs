use uefi::prelude::RuntimeServices;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub struct SystemClock {
    period_s: f64,
    epoch_offset: f64,
}

impl SystemClock {
    pub fn new(runtime_services: &RuntimeServices) -> Self {
        let period_s = Self::estimate_cpu_period(runtime_services);
        let secs_since_epoch = Self::get_epoch_time(runtime_services);

        let n = unsafe { core::arch::x86_64::_rdtsc() };
        let epoch_offset: f64 = secs_since_epoch - (n as f64 * period_s);

        SystemClock {
            period_s,
            epoch_offset,
        }
    }

    pub fn time(&self) -> f64 {
        // in milliseconds
        let n = unsafe { core::arch::x86_64::_rdtsc() };
        1000f64 * (n as f64) * self.period_s + self.epoch_offset
    }

    pub fn spin_delay(&self, duration: f64) {
        let t0 = self.time();
        while self.time() - t0 < duration {}
    }

    pub fn utc_datetime(runtime_services: &RuntimeServices) -> DateTime<Utc> {

        let t_uefi = runtime_services.get_time().unwrap();

        // We're assuming the UEFI RTC clock returns UTC
        Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(
                    t_uefi.year().into(),
                    t_uefi.month().into(),
                    t_uefi.day().into(),
                )
                .expect("Invalid RTC clock")
                .and_hms_opt(
                    t_uefi.hour().into(),
                    t_uefi.minute().into(),
                    t_uefi.second().into(),
                )
                .expect("Invalid RTC clock"),
            )
            .unwrap()
    }

    fn estimate_cpu_period(runtime_services: &RuntimeServices) -> f64 {
        // Waiting for the "seconds" value to change
        let s1 = runtime_services.get_time().unwrap().second();
        let s2 = loop {
            let s2 = runtime_services.get_time().unwrap().second();
            if s1 != s2 {
                break s2;
            }
        };

        // Waiting approximately one second and measuring the change in rdtsc
        let n1 = unsafe { core::arch::x86_64::_rdtsc() };
        loop {
            let s3 = runtime_services.get_time().unwrap().second();
            if s2 != s3 {
                break;
            }
        }
        let n2 = unsafe { core::arch::x86_64::_rdtsc() };

        let period_s = 1.0 / ((n2 - n1) as f64);
        let freq_ghz = 1.0 / period_s / 1E9;
        log::debug!(
            "CPU frequency estimated to {:.3}Ghz ({} cycles in approx 1s)",
            freq_ghz,
            n2 - n1
        );

        period_s
    }

    fn get_epoch_time(runtime_services: &RuntimeServices) -> f64 {

        let t_chrono = Self::utc_datetime(runtime_services);

        let secs_since_epoch: u64 = (t_chrono - DateTime::UNIX_EPOCH)
            .num_seconds()
            .try_into()
            .expect("Current time before UNIX epoch");

        log::debug!("UNIX time: {}", secs_since_epoch);

        secs_since_epoch as f64
    }
}
