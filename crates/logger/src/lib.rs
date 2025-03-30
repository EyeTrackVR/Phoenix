use std::sync::{Mutex, OnceLock};
use colored::{ColoredString, Colorize};
use log::{Level, Log, Metadata, Record, SetLoggerError};

pub struct Logger {
    pub log_level: Mutex<Level>,
    pub crate_levels: Mutex<Vec<(String, Level)>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

impl Logger {
    pub fn new(level: Level) -> Logger {
        Logger {
            log_level: Mutex::new(level),
            crate_levels: Mutex::new(Vec::new()),
        }
    }

    pub fn set_level(&self, level: Level) {
        *self.log_level.lock().unwrap() = level;
    }

    pub fn colorize(&self, level: Level) -> ColoredString {
        match level {
            Level::Error => level.as_str().red(),
            Level::Warn => level.as_str().yellow(),
            Level::Info => level.as_str().green(),
            Level::Debug => level.as_str().blue(),
            Level::Trace => level.as_str().purple(),
        }
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let log_level = *self.log_level.lock().unwrap();
        let crate_levels = self.crate_levels.lock().unwrap();
        let crate_name = metadata.target().split("::").next().unwrap();
        // FIXME: depending on order added crate::module may inherit the level of crate
        for (name, level) in crate_levels.iter() {
            if crate_name == name {
                return metadata.level() <= *level;
            }
        }

        return metadata.level() <= log_level;
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata())  {
            #[cfg(not(target_family = "wasm"))]
            eprintln!("{}: {} - {}", self.colorize(record.level()), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init() -> Result<(), SetLoggerError> {
    let logger = LOGGER.get_or_init(|| {
        let env_level = std::env::var("RUST_LOG").unwrap_or("info".to_string());
        let level = env_level.parse().unwrap_or(Level::Info);
        return Logger::new(level);
    });

    #[cfg(feature = "panic-handler")]
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map_or("Unknown location".to_string(), |p| format!("{}:{}:{}", p.file(), p.line(), p.column()));
        let payload = info.payload().downcast_ref::<String>().map(|s| s.clone()).unwrap_or_else(|| {
            info.payload().downcast_ref::<&str>().unwrap_or(&"Unknown Payload").to_string()
        });
        // This treats newlines as a pseudo "stack trace" for the panic
        let payload = payload.lines().enumerate()
            .map(|(i, line)| match (i, line.trim().is_empty()) {
                (_, true) => String::new(),
                (0, _) => format!("{}", line),
                _ => format!("\t\t||  {}", line),
            })
            .filter(|line| !line.is_empty())
        .collect::<Vec<_>>().join("\n");

        let trace = match std::env::var("RUST_BACKTRACE") {
            Ok(_) => std::backtrace::Backtrace::force_capture().to_string(),
            Err(_) => "  Run with RUST_BACKTRACE=1 environment variable to display backtrace".to_string(),
        };
        let trace = trace.lines().map(|line| format!("\t\t|{}", line)).collect::<Vec<_>>().join("\n");
        log::error!("Panic occurred at: {}\n\t\t-----------------> {}\n{}", location.black(), payload.bright_red(), trace);
    }));

    return log::set_logger(logger).map(|()| log::set_max_level(log::LevelFilter::Trace));
}

pub fn set_level(level: Level) {
    LOGGER.get().unwrap().set_level(level);
}

pub fn set_crate_log(target: &str, level: Level) {
    LOGGER.get().unwrap().crate_levels.lock().unwrap().push((target.to_string(), level));
}

pub fn get_raw_logger() -> &'static Logger {
    return LOGGER.get().unwrap();
}
