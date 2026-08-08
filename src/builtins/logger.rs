use flexi_logger::{opt_format, Logger, FileSpec, WriteMode, LoggerHandle};

pub fn init() -> LoggerHandle {
  Logger::try_with_str("info").unwrap()
  .log_to_file(
    FileSpec::default()
    .suppress_timestamp()
    .basename("status")
    .suffix("log"),
  )
  .format_for_files(opt_format)
  .write_mode(WriteMode::Async)
  .append().start().unwrap()
}
