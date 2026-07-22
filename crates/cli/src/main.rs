mod cli;
mod console_reporter;

use clap::Parser;
use cli::Cli;
use console_reporter::ConsoleReporter;
use happyjlc_core::run_with_reporter;
use std::process;
use std::sync::Arc;

fn exit_code_for_failures(failed: usize) -> i32 {
    if failed == 0 { 0 } else { 1 }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{} {} happyjlc] {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                record.level(),
                record.args()
            )
        })
        .init();

    let args = Cli::parse();
    if args.debug {
        log::set_max_level(log::LevelFilter::Debug);
    }

    let request = match args.into_request() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Error: {}", error);
            process::exit(1);
        }
    };
    let reporter = Arc::new(ConsoleReporter::new());
    match run_with_reporter(request, reporter.clone()).await {
        Ok(Some(summary)) => {
            let failed = summary.failed;
            reporter.report_summary(&summary);
            let exit_code = exit_code_for_failures(failed);
            if exit_code != 0 {
                process::exit(exit_code);
            }
        }
        Ok(None) => reporter.report_no_work(),
        Err(error) => {
            eprintln!("Error: {}", error);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::exit_code_for_failures;

    #[test]
    fn failed_components_produce_nonzero_exit_code() {
        assert_eq!(exit_code_for_failures(0), 0);
        assert_eq!(exit_code_for_failures(1), 1);
    }
}
