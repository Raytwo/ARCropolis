use std::{
    fs::File,
    io::{BufWriter, Write},
    ops::Deref,
    path::Path,
    time::SystemTime,
};

use log::{LevelFilter, Metadata, Record, SetLoggerError};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use skyline::nn::time;

/// Since we can't rely on most time based libraries, this is a seconds -> date/time string based on the `chrono` crates implementation
fn get_time_string() -> String {
    let datetime: time::CalendarTime = time::get_calendar_time();

    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", datetime.year, datetime.month, datetime.day, datetime.hour, datetime.minute, datetime.second)
}

static LOG_PATH: &str = "sd:/ultimate/arcropolis/logs";
static FILE_LOG_BUFFER: usize = 0x2000; // Room for 0x2000 characters, might have performance issues if the logger level is "Info" or "Trace"
struct FileLogger(Option<Mutex<BufWriter<File>>>);

impl Deref for FileLogger {
    type Target = Option<Mutex<BufWriter<File>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileLogger {
    pub fn write<T: AsRef<[u8]>>(&self, message: T) {
        if let Some(writer) = &self.0 {
            let mut writer = writer.lock();
            let _ = writer.write(message.as_ref());
        }
    }
}

// Summon the file logger and create a file for it based on the current time (requires time to be initialized)
static FILE_WRITER: Lazy<FileLogger> = Lazy::new(|| {
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Clock may have gone backwards!");
    let path = Path::new(LOG_PATH).join(format!("{}.log", get_time_string()));
    let _ = std::fs::create_dir_all(LOG_PATH);
    std::fs::File::create(path).map_or_else(
        |_| {
            error!(target: "std", "Unable to initialize the file logger!");
            FileLogger(None)
        },
        |file| {
            // Spawn a log flusher, since we don't have the ability to flush the logger on application close, home button press,
            // or crash (crashing technically can be done but ARCropolis is not the place to implement)
            let _ = std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(2000));
                log::logger().flush();
            });
            FileLogger(Some(Mutex::new(BufWriter::with_capacity(FILE_LOG_BUFFER, file))))
        },
    )
});

struct ArcLogger;

static LOGGER: ArcLogger = ArcLogger;

pub fn init(filter: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(filter))
}

impl log::Log for ArcLogger {
    // Always log what we tell it to log
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let module_path = match record.module_path() {
            Some(path) => path,
            None => return,
        };

        let skip_mod_path = record.target() == "no-mod-path";

        let message = if record.level() == LevelFilter::Debug && !skip_mod_path {
            let file = record.file().unwrap_or("???");
            let number = match record.line() {
                Some(no) => format!("{}", no),
                None => "???".to_string(),
            };
            format!("[{} | {}:{}] {}\n", module_path, file, number, record.args())
        } else if !skip_mod_path {
            format!("[{}] {}\n", module_path, record.args())
        } else {
            format!("{}\n", record.args())
        };

        // We allow two different log targets, one for specifically logging to the skyline logger and the other for specifically
        // logging to a file. If no target is mentioned (or one that doesn't exist) we log to both.
        match record.target() {
            "std" => {
                print!("{}", message);
            },
            "file" => {
                if config::file_logging_enabled() {
                    FILE_WRITER.write(strip_ansi_escapes::strip(message).unwrap_or_default());
                }
            },
            _ => {
                print!("{}", message);
                if config::file_logging_enabled() {
                    FILE_WRITER.write(strip_ansi_escapes::strip(message).unwrap_or_default());
                }
            },
        }
    }

    // Only matters for writing to a file
    fn flush(&self) {
        if config::file_logging_enabled() {
            if let Some(writer) = &**FILE_WRITER {
                if let Some(mut writer) = writer.try_lock() {
                    if let Err(err) = writer.flush() {
                        error!(target: "std", "Failed to flush file logger! Reason: {:?}", err)
                    }
                }
            }
        }
    }
}
