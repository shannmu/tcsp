use std::{fs::{File, OpenOptions}, sync::Mutex};
use std::io::Write;
use log::{Level, Metadata, Record, SetLoggerError};

struct FileLogger {
    file: Mutex<File>,
    level: Level,
}

impl FileLogger {
    fn new(file_path: &str, level: Level) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(file_path)
            .unwrap();
        
        FileLogger {
            file: Mutex::new(file),
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
            let mut file = self.file.lock().unwrap();
            writeln!(file, "{} - {}", record.level(), record.args()).unwrap();
        }
    }

    fn flush(&self) {}
}

pub fn init_logger(file_path: &str, level: Level) -> Result<(), SetLoggerError> {
    let logger = FileLogger::new(file_path, level);
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(level.to_level_filter());
    Ok(())
}
