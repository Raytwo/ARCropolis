use log::{LevelFilter, Metadata, Record, SetLoggerError};
use parking_lot::Mutex;
use std::{
    fs::File,
    io::{BufWriter, Write},
    ops::Deref,
    path::Path,
    time::SystemTime,
};

use crate::config;

/// Since we can't rely on most time based libraries, this is a seconds -> date/time string based on the `chrono` crates implementation
fn format_time_string(seconds: u64) -> String {
    let leapyear = |year| -> bool { year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) };

    static YEAR_TABLE: [[u64; 12]; 2] = [
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    ];

    let mut year = 1970;

    let seconds_in_day = seconds % 86400;
    let mut day_number = seconds / 86400;

    let sec = seconds_in_day % 60;
    let min = (seconds_in_day % 3600) / 60;
    let hours = seconds_in_day / 3600;
    loop {
        let year_length = if leapyear(year) { 366 } else { 365 };

        if day_number >= year_length {
            day_number -= year_length;
            year += 1;
        } else {
            break;
        }
    }
    let mut month = 0;
    while day_number >= YEAR_TABLE[if leapyear(year) { 1 } else { 0 }][month] {
        day_number -= YEAR_TABLE[if leapyear(year) { 1 } else { 0 }][month];
        month += 1;
    }
  
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        year,
        month + 1,
        day_number + 1,
        hours,
        min,
        sec
    )
}

static LOG_PATH: &'static str = "sd:/ultimate/arcropolis/logs";
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

lazy_static! {
    // Summon the file logger and create a file for it based on the current time (requires time to be initialized)
    static ref FILE_WRITER: FileLogger = {
        let seconds = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Clock may have gone backwards!");
        let path = Path::new(LOG_PATH).join(format!("{}.log", format_time_string(seconds.as_secs())));
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
                FileLogger(Some(Mutex::new(BufWriter::with_capacity(
                    FILE_LOG_BUFFER,
                    file,
                ))))
            },
        )
    };
}

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
            None => {
                return
            },
        };

        let skip_mod_path = record.target() == "no-mod-path";

        let message = if record.level() == LevelFilter::Debug && !skip_mod_path {
            let file = match record.file() {
                Some(file) => file,
                None => "???",
            };
            let number = match record.line() {
                Some(no) => format!("{}", no),
                None => "???".to_string(),
            };
            format!(
                "[{} | {}:{}] {}\n",
                module_path,
                file,
                number,
                record.args()
            )
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
            }
            "file" => {
                if config::file_logging_enabled() {
                    FILE_WRITER.write(strip_ansi_escapes::strip(message).unwrap_or(vec![]));
                }
            }
            _ => {
                print!("{}", message);
                if config::file_logging_enabled() {
                    FILE_WRITER.write(strip_ansi_escapes::strip(message).unwrap_or(vec![]));
                }
            }
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
