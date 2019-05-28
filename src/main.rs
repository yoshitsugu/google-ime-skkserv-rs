use daemonize::Daemonize;
use docopt::Docopt;
use encoding::all::EUC_JP;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use env_logger;
use log::{debug, error};
use rustc_serialize::json::Json;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

mod cache;
mod error;

use cache::{new_cache, LockedCache};
use error::SearchError;

enum RequestCode {
    Disconnect,
    Convert,
    Version,
    Name,
    Invalid(u8)
}

impl From<u8> for RequestCode {
    fn from(code: u8) -> Self {
        use RequestCode::*;
        match code {
            b'0' => Disconnect,
            b'1' => Convert,
            b'2' => Version,
            b'3' => Name,
            code => Invalid(code),
        }
    }
} 

const SERVER_VERSION: &str = "Google IME SKK Server in Rust.0.1";
const PID_DIR: &str = "/tmp/gskkserv.pid";
const WORK_DIR: &str = "/tmp";
#[cfg(not(test))]
const GOOGLE_IME_URL: &str = "http://www.google.com/transliterate?langpair=ja-Hira%7Cja&text=";

const JSON_ERROR_MSG: &str = "Cannot find expected json structure";

#[cfg(not(test))]
fn search_with_api(kana: &str) -> Result<String, SearchError> {
    let url = format!("{}{}", GOOGLE_IME_URL, kana);
    Ok(reqwest::get(&url)?.text()?)
}

fn search(buf: &[u8]) -> Result<Vec<u8>, SearchError> {
    let kana = EUC_JP.decode(&buf, DecoderTrap::Ignore).unwrap();
    let s = search_with_api(&kana)?;
    let json = Json::from_str(s.as_str())?;
    let array = json.as_array().ok_or(JSON_ERROR_MSG)?;
    let kanji = array[0].as_array().ok_or(JSON_ERROR_MSG)?;
    let _kanjis = kanji[1].as_array().ok_or(JSON_ERROR_MSG)?;
    let mut kanjis = "1".to_string();
    for _k in _kanjis {
        let _ks = _k.as_string().ok_or(JSON_ERROR_MSG)?;
        kanjis = format!("{}/{}", kanjis, _ks);
    }
    kanjis = format!("{}/\n", kanjis);
    let r = EUC_JP.encode(&kanjis, EncoderTrap::Ignore).unwrap();
    Ok(r)
}

fn create_response<'a>(
    buf: &[u8],
    n: usize,
    cache: &'a mut LockedCache,
    host_and_port: &'a str,
) -> &'a [u8] {
    debug!("CODE: {}", buf[0]);
    match RequestCode::from(buf[0]) {
        RequestCode::Disconnect => b"0",
        RequestCode::Convert => {
            let cache_key = buf[1..(n - 1)].to_vec();
            debug!("cache_key: {:?}", &cache_key);
            if cache.contains_key(&cache_key) {
                cache.get(&cache_key).unwrap().as_slice()
            } else {
                match search(&buf[1..(n - 1)]) {
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
        RequestCode::Version => SERVER_VERSION.as_bytes(),
        RequestCode::Name => host_and_port.as_bytes(),
        RequestCode::Invalid(code) => {
            error!("INVALID CODE: {}", code);
            b"0"
        }
    }

}

fn handle_client(mut stream: TcpStream, mut cache: LockedCache, host_and_port: &str) {
    loop {
        let mut buf = [0; 512];
        match stream.read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    break;
                }
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
    println!("listen on {}", &host_and_port);
    let listener = TcpListener::bind(&host_and_port.clone()).unwrap();
    let cache = new_cache();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cache = cache.clone();
                let host_and_port = host_and_port.clone();
                thread::spawn(move || {
                    let cache = cache.lock().unwrap();
                    handle_client(stream, cache, &host_and_port);
                });
            }
            Err(_) => {
                error!("Error!");
            }
        }
    }
}

static USAGE: &'static str = "
Usage:
  gskkserv [--host=<host>] [--port=<port>] [-d]

Options:
  -h --host=<host>     Server Host [default: 0.0.0.0]
  -p --port=<port>     Server Port [default: 55100]
  -d                   Deamonize
";

fn main() {
    env_logger::init();
    let args = Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());
    if args.get_bool("-d") {
        let daemonize = Daemonize::new()
            .pid_file(PID_DIR)
            .working_directory(WORK_DIR);
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
    Ok(
        r#"[["ともえごぜん",["巴御前","ともえごぜん","トモエゴゼン"]]]"#.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use encoding::all::{EUC_JP, UTF_8};
    use encoding::{DecoderTrap, EncoderTrap, Encoding};

    use super::{
        search, create_response, SERVER_VERSION,
    };

    use super::cache::new_cache;

    #[test]
    fn test_search() {
        let utf8_bytes = EUC_JP
            .encode("ともえごぜん", EncoderTrap::Ignore)
            .unwrap();
        let result = search(utf8_bytes.as_slice());
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
            create_response(&[b'0'], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            b"0"
        );
        assert_eq!(
            create_response(&[b'2'], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            SERVER_VERSION.as_bytes()
        );
        assert_eq!(
            create_response(&[b'3'], 1, &mut cache.lock().unwrap(), "0.0.0.0:5555"),
            b"0.0.0.0:5555"
        );
    }
}