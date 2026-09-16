use std::env;

/// Konfigurasi runtime aplikasi UptimePulse
/// Mendukung argumen baris perintah (CLI flags) dan Environment Variables
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub retention_days: u32,
    pub admin_password: Option<String>,
}

impl AppConfig {
    pub fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();
        let mut host = env::var("UPTIME_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let mut port: u16 = env::var("UPTIME_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3001);
        let mut db_path = env::var("UPTIME_DB_PATH").unwrap_or_else(|_| "uptime.db".to_string());
        let mut retention_days: u32 = env::var("UPTIME_RETENTION_DAYS")
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(90);
        let mut admin_password = env::var("ADMIN_PASSWORD").ok();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--host" => {
                    if i + 1 < args.len() {
                        host = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Flag --host requires an argument".to_string());
                    }
                }
                "-p" | "--port" => {
                    if i + 1 < args.len() {
                        port = args[i + 1]
                            .parse()
                            .map_err(|_| "Port must be a valid number (1-65535)")?;
                        i += 2;
                    } else {
                        return Err("Flag --port requires an argument".to_string());
                    }
                }
                "-d" | "--db" => {
                    if i + 1 < args.len() {
                        db_path = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Flag --db requires an argument".to_string());
                    }
                }
                "-r" | "--retention" => {
                    if i + 1 < args.len() {
                        retention_days = args[i + 1]
                            .parse()
                            .map_err(|_| "Retention days must be a positive integer")?;
                        i += 2;
                    } else {
                        return Err("Flag --retention requires an argument".to_string());
                    }
                }
                "--password" => {
                    if i + 1 < args.len() {
                        admin_password = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err("Flag --password requires an argument".to_string());
                    }
                }
                "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    return Err(format!(
                        "Unknown argument: '{}'\nRun with --help to see available options.",
                        other
                    ));
                }
            }
        }

        Ok(Self {
            host,
            port,
            db_path,
            retention_days,
            admin_password,
        })
    }
}

fn print_help() {
    println!(
        r#"UptimePulse v0.1.0 - Ultra-lightweight self-hosted monitoring system

USAGE:
  uptime-pulse [OPTIONS]

OPTIONS:
  -h, --host <HOST>         Listen host address [env: UPTIME_HOST] (default: 0.0.0.0)
  -p, --port <PORT>         Listen port [env: UPTIME_PORT] (default: 3001)
  -d, --db <PATH>           SQLite database path [env: UPTIME_DB_PATH] (default: uptime.db)
  -r, --retention <DAYS>    Log retention days before auto-pruning [env: UPTIME_RETENTION_DAYS] (default: 90)
      --password <PASS>     Default admin password if not set [env: ADMIN_PASSWORD]
      --help                Show this help message
"#
    );
}
