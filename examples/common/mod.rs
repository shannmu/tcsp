use std::{fs::{File, OpenOptions}, sync::Mutex};
use log::{Level, Metadata, Record, SetLoggerError};

struct FileLogger {
    level: Level,
}

impl FileLogger {
    fn new(level: Level) -> Self {
        FileLogger {
            level,
        }
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init_logger(level: Level) -> Result<(), SetLoggerError> {
    let logger = FileLogger::new(level);
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(level.to_level_filter());
    Ok(())
}
