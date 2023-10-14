use log::{Log, Metadata, Record};
use crate::serial_println;

pub struct SerialLogger;


impl Log for SerialLogger {

    // TODO
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {

        if !self.enabled(record.metadata()) {
            return;
        }
 
        serial_println!(
            "{}: {} -- {}",
            record.level(),
            record.module_path().unwrap(), // Not sure why this can fail?
            record.args(),
        );
    }

    fn flush(&self) {}
}
