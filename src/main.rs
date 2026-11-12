#[cfg(unix)]
use daemonize::Daemonize;
use docopt::Docopt;
use encoding::all::EUC_JP;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use log::{debug, error};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

mod cache;
mod error;

use crate::cache::{Cache, LockedCache, new_cache};
use crate::error::SearchError;

enum RequestCode {
    Disconnect,
    Convert,
    Version,
    Name,
    Completion,
    Invalid(u8),
}

impl From<u8> for RequestCode {
    fn from(code: u8) -> Self {
        use RequestCode::*;
        match code {
            b'0' => Disconnect,
            b'1' => Convert,
            b'2' => Version,
            b'3' => Name,
            b'4' => Completion,
            code => Invalid(code),
        }
    }
}

const SERVER_VERSION: &str = concat!("Google IME SKK Server in Rust.", env!("CARGO_PKG_VERSION"));
#[cfg(unix)]
const PID_DIR: &str = "/tmp/gskkserv.pid";
#[cfg(unix)]
const WORK_DIR: &str = "/tmp";
const GOOGLE_IME_URL: &str = "https://www.google.com/transliterate";

const JSON_ERROR_MSG: &str = "Cannot find expected json structure";

fn request_api(kana: &str, endpoint: &str) -> Result<String, SearchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    Ok(client
        .get(endpoint)
        .query(&[("langpair", "ja-Hira|ja"), ("text", kana)])
        .send()?
        .error_for_status()?
        .text()?)
}

#[cfg(not(test))]
fn search_with_api(kana: &str) -> Result<String, SearchError> {
    request_api(kana, GOOGLE_IME_URL)
}

fn format_response(json: &str, delimiter: &str) -> Result<Vec<u8>, SearchError> {
    let segments: Vec<(String, Vec<String>)> = serde_json::from_str(json)?;
    let (_, candidates) = segments.first().ok_or(JSON_ERROR_MSG)?;
    if candidates.is_empty() {
        return Err(JSON_ERROR_MSG.into());
    }
    let response = format!("1{delimiter}{}{delimiter}\n", candidates.join(delimiter));
    Ok(EUC_JP.encode(&response, EncoderTrap::Ignore).unwrap())
}

fn search(buf: &[u8], delimiter: &str) -> Result<Vec<u8>, SearchError> {
    let kana = EUC_JP.decode(buf, DecoderTrap::Ignore).unwrap();
    format_response(&search_with_api(&kana)?, delimiter)
}

fn load_from_cache_or_search<'a>(
    cache_key: Vec<u8>,
    cache: &'a mut LockedCache,
    delimiter: &str,
) -> &'a [u8] {
    if cache.contains_key(&cache_key) {
        cache.get(&cache_key).unwrap().as_slice()
    } else {
        match search(&cache_key, delimiter) {
            Ok(result) => {
                cache.insert(cache_key.clone(), result.clone());
                cache.get(&cache_key).unwrap().as_slice()
            }
            Err(err) => {
                error!("{:?}", err);
                b"0"
            }
        }
    }
}

fn create_response<'a>(
    buf: &[u8; 512],
    n: usize,
    cache: &'a mut LockedCache,
    host_and_port: &'a str,
) -> &'a [u8] {
    debug!("CODE: {}", buf[0]);
    match RequestCode::from(buf[0]) {
        RequestCode::Disconnect => b"0",
        RequestCode::Convert => load_from_cache_or_search(buf[1..(n - 1)].to_vec(), cache, "/"),
        RequestCode::Version => SERVER_VERSION.as_bytes(),
        RequestCode::Name => host_and_port.as_bytes(),
        RequestCode::Completion => load_from_cache_or_search(buf[1..(n - 1)].to_vec(), cache, " "),
        RequestCode::Invalid(code) => {
            error!("INVALID CODE: {}", code);
            b"0"
        }
    }
}

fn handle_client(mut stream: TcpStream, cache: Cache, host_and_port: &str) {
    loop {
        let mut buf = [0; 512];
        match stream.read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    break;
                }
                let mut cache = cache.lock().unwrap();
                let result = create_response(&buf, n, &mut cache, host_and_port);
                stream.write_all(result).unwrap();
            }
            Err(err) => {
                error!("{:?}", err);
                stream.write_all(b"0").unwrap();
            }
        }
    }
}

fn listen(args: &docopt::ArgvMap) {
    let host_and_port = format!("{}:{}", args.get_str("--host"), args.get_str("--port"));
    println!("listen on {}", host_and_port);
    let listener = TcpListener::bind(&host_and_port).unwrap();
    let cache = new_cache();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cache = cache.clone();
                let host_and_port = host_and_port.clone();
                thread::spawn(move || {
                    handle_client(stream, cache, &host_and_port);
                });
            }
            Err(_) => {
                error!("Error!");
            }
        }
    }
}

static USAGE: &str = "
Usage:
  gskkserv [--host=<host>] [--port=<port>] [-d]
  gskkserv --help

Options:
  -h --host=<host>     Server Host [default: 0.0.0.0]
  -p --port=<port>     Server Port [default: 55100]
  -d                   Daemonize (Unix only)
  --help               Show this help
";

fn main() {
    env_logger::init();
    let args = Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());
    if args.get_bool("-d") {
        #[cfg(not(unix))]
        {
            eprintln!("Daemon mode is only supported on Unix");
            std::process::exit(1);
        }
        #[cfg(unix)]
        let daemonize = Daemonize::new()
            .pid_file(PID_DIR)
            .working_directory(WORK_DIR);
        #[cfg(unix)]
        match daemonize.start() {
            Ok(_) => {
                listen(&args);
            }
            Err(e) => eprintln!("{}", e),
        }
    } else {
        listen(&args);
    }
}

#[cfg(test)]
fn search_with_api(_kana: &str) -> Result<String, SearchError> {
    Ok(r#"[["ともえごぜん",["巴御前","ともえごぜん","トモエゴゼン"]]]"#.to_string())
}

#[cfg(test)]
mod tests {
    use encoding::all::{EUC_JP, UTF_8};
    use encoding::{DecoderTrap, EncoderTrap, Encoding};

    use super::{SERVER_VERSION, create_response, search};

    use super::cache::new_cache;

    #[test]
    fn rejects_invalid_api_responses() {
        for json in [
            "[]",
            "null",
            "not json",
            "[[]]",
            r#"[["かな",[]]]"#,
            r#"[["かな",[1]]]"#,
        ] {
            assert!(super::format_response(json, "/").is_err(), "{json}");
        }
    }

    #[test]
    fn formats_completion_and_half_width_kana() {
        let response = super::format_response(r#"[["かな",["仮名","ｶﾅ"]]]"#, " ").unwrap();
        assert_eq!(
            EUC_JP.decode(&response, DecoderTrap::Strict).unwrap(),
            "1 仮名 ｶﾅ \n"
        );
    }

    #[test]
    fn api_encodes_query_and_checks_http_status() {
        for status in ["200 OK", "503 Service Unavailable"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    request.push_str(&line);
                }
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]"
                );
                stream.write_all(response.as_bytes()).unwrap();
                request
            });
            let result = super::request_api("かな&?#", &endpoint);
            let request = server.join().unwrap();
            assert!(request.starts_with(
                "GET /?langpair=ja-Hira%7Cja&text=%E3%81%8B%E3%81%AA%26%3F%23 HTTP/1.1\r\n"
            ));
            if status.starts_with("200") {
                assert_eq!(result.unwrap(), "[]");
            } else {
                assert!(
                    matches!(result, Err(super::SearchError::Request(e)) if e.status() == Some(reqwest::StatusCode::SERVICE_UNAVAILABLE))
                );
            }
        }
    }

    #[test]
    #[ignore = "requires access to the live Google CGI API"]
    fn live_google_api() {
        let json = super::request_api("ともえごぜん", super::GOOGLE_IME_URL).unwrap();
        let response = super::format_response(&json, "/").unwrap();
        let response = EUC_JP.decode(&response, DecoderTrap::Strict).unwrap();
        assert!(response.starts_with("1/"));
        assert!(response.contains("/巴御前/"));
        assert!(response.ends_with("/\n"));
    }

    #[test]
    fn test_search() {
        let utf8_bytes = EUC_JP.encode("ともえごぜん", EncoderTrap::Ignore).unwrap();
        let result = search(utf8_bytes.as_slice(), "/");
        assert!(result.is_ok());
        let euc_jp_str = EUC_JP
            .decode(result.unwrap().as_slice(), DecoderTrap::Ignore)
            .unwrap();
        let utf8_encoded_arr = UTF_8.encode(&euc_jp_str, EncoderTrap::Ignore).unwrap();
        assert_eq!(
            UTF_8
                .decode(utf8_encoded_arr.as_slice(), DecoderTrap::Ignore)
                .unwrap(),
            "1/巴御前/ともえごぜん/トモエゴゼン/\n"
        )
    }

    #[test]
    fn test_create_response() {
        let cache = new_cache();
        assert_eq!(
            create_response(&[b'0'; 512], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            b"0"
        );
        assert_eq!(
            create_response(&[b'2'; 512], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            SERVER_VERSION.as_bytes()
        );
        assert_eq!(
            create_response(&[b'3'; 512], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            b"0.0.0.0:5555"
        );
    }
}
