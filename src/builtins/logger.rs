use std::env;

use flexi_logger::{
    Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming, WriteMode,
    opt_format,
};

/*
  Application logging.

  Three things were wrong with the previous six-line setup, and each one only
  shows up once the server has been running for a while:

    - No rotation. `status.log` grew without bound until the disk filled.
    - File only. Under systemd or Docker `journalctl -u <unit>` / `docker logs`
      showed nothing at all, because nothing was written to a standard stream.
    - A hardcoded "info" level, so raising verbosity to diagnose a live problem
      meant editing the source and redeploying.
*/

/// Rotate the active file once it reaches this many megabytes.
fn rotate_megabytes() -> u64 {
    positive_env("LOG_ROTATE_MB", 10)
}

/// How many rotated files to keep before the oldest is discarded.
fn keep_files() -> usize {
    positive_env("LOG_KEEP_FILES", 7) as usize
}

fn positive_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

pub fn init() -> LoggerHandle {
    // Keeps log files out of the working directory, and lets a service unit put
    // them somewhere like /var/log. Created automatically if missing.
    let directory = env::var("LOG_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "./logs".to_string());

    /*
      The level, from RUST_LOG, falling back to "info" so it is a restart away
      rather than a rebuild.

      Read and normalised here rather than via `try_with_env_or_str`, because
      that helper only falls back when RUST_LOG is *absent*. A `.env` file
      carrying `RUST_LOG=` - which is how this template ships, meaning "use the
      default" - sets the variable to the empty string, which is present but
      parses to an empty spec: no targets, no output. The result was a server
      that logged nothing at all, to neither the file nor the journal, until
      someone happened to set a level explicitly.
    */
    let specification = env::var("RUST_LOG")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "info".to_string());

    Logger::try_with_str(&specification)
        .expect("invalid log specification (check RUST_LOG)")
        .log_to_file(
            FileSpec::default()
                .directory(directory)
                .suppress_timestamp()
                .basename("status")
                .suffix("log"),
        )
        /*
          Rotate on age OR size, whichever comes first: a busy day is bounded by
          the size limit, and a quiet week still starts a fresh file rather than
          appending to a months-old one.
        */
        .rotate(
            Criterion::AgeOrSize(Age::Day, rotate_megabytes() * 1024 * 1024),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(keep_files()),
        )
        /*
          Also write to stderr, which is what journald and `docker logs` capture.
          Without this the service looks silent to every standard tool.
          `Duplicate::All` rather than `Duplicate::Info` on purpose: the level is
          already decided by RUST_LOG, so capping this sink separately would mean
          `RUST_LOG=debug` changed the log file but not `journalctl` output - the
          one place an operator is most likely to be looking.
          This also softens the one downside of `WriteMode::Async`: buffered file
          writes can be lost if the process is killed outright, but the same lines
          have already reached the journal.
        */
        .duplicate_to_stderr(Duplicate::All)
        .format_for_files(opt_format)
        .write_mode(WriteMode::Async)
        .append()
        .start()
        .expect("could not start logger")
}
