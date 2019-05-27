use daemonize::Daemonize;
use docopt::Docopt;
use encoding::all::EUC_JP;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use env_logger;
use failure::Fail;
use log::{debug, error};
use rustc_serialize::json;
use rustc_serialize::json::Json;
use std::io;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;


use gskkserv::cache::{new_cache, LockedCache};

const CLIENT_END: u8 = b'0';
const CLIENT_REQUEST: u8 = b'1';
const CLIENT_VERSION: u8 = b'2';
const CLIENT_HOST: u8 = b'3';
//const CLIENT_COMP: u8      = 4;

const SERVER_VERSION: &str = "Google IME SKK Server in Rust.0.0";
const PID_DIR: &str = "/tmp/gskkserv.pid";
const WORK_DIR: &str = "/tmp";
#[cfg(not(test))]
const GOOGLE_IME_URL: &str = "http://www.google.com/transliterate?langpair=ja-Hira%7Cja&text=";

#[derive(Debug, Fail)]
enum SearchError {
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] io::Error),
    #[fail(display = "{}", _0)]
    Json(#[fail(cause)] json::BuilderError),
    #[fail(display = "{}", _0)]
    Msg(&'static str),
    #[fail(display = "{}", _0)]
    Request(#[fail(cause)] reqwest::Error),
}

impl From<io::Error> for SearchError {
    fn from(err: io::Error) -> SearchError {
        SearchError::Io(err)
    }
}

impl From<json::BuilderError> for SearchError {
    fn from(err: json::BuilderError) -> SearchError {
        SearchError::Json(err)
    }
}

impl From<&'static str> for SearchError {
    fn from(err: &'static str) -> SearchError {
        SearchError::Msg(err)
    }
}

impl From<reqwest::Error> for SearchError {
    fn from(err: reqwest::Error) -> SearchError {
        SearchError::Request(err)
    }
}
const JSON_ERROR_MSG: &str = "Cannot find expected json structure";

#[cfg(not(test))]
fn search_with_api(kana: &str) -> Result<String, SearchError> {
    let url = format!("{}{}", GOOGLE_IME_URL, kana);
    Ok(reqwest::get(&url)?.text()?)
}

fn search(read: &[u8]) -> Result<Vec<u8>, SearchError> {
    let kana = EUC_JP.decode(&read, DecoderTrap::Ignore).unwrap();
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
    kanjis = format!("{}\n", kanjis);
    let r = EUC_JP.encode(&kanjis, EncoderTrap::Ignore).unwrap();
    Ok(r)
}

fn handle_client(mut stream: TcpStream, mut cache: LockedCache) {
    loop {
        let mut read = [0; 512];
        match stream.read(&mut read) {
            Ok(n) => {
                if n == 0 {
                    break;
                }
                match read[0] {
                    CLIENT_END => {
                        stream.write_all(&[b'0', b'\n']).unwrap();
                    }
                    CLIENT_REQUEST => {
                        let cache_key = read[1..(n - 1)].to_vec();
                        debug!("cache_key: {:?}", &cache_key);
                        if cache.contains_key(&cache_key) {
                            stream.write_all(cache.get(&cache_key).unwrap()).unwrap();
                        } else {
                            match search(&read[1..(n - 1)]) {
                                Ok(result) => {
                                    cache.insert(read[1..(n - 1)].to_vec(), result.clone());
                                    stream.write_all(result.as_slice()).unwrap();
                                }
                                Err(err) => {
                                    error!("{:?}", err);
                                    stream.write_all(&[b'0', b'\n']).unwrap();
                                }
                            }
                        }
                    }
                    CLIENT_VERSION => {
                        stream.write_all(SERVER_VERSION.as_bytes()).unwrap();
                    }
                    CLIENT_HOST => {
                        stream.write_all(b"0.0.0.0").unwrap();
                    }
                    _ => {
                        stream.write_all(&[b'0', b'\n']).unwrap();
                    }
                }
            }
            Err(err) => {
                println!("{:?}", err);
                stream.write_all(&[b'0', b'\n']).unwrap();
            }
        }
    }
}

fn listen(args: &docopt::ArgvMap) {
    let host_and_port = format!("{}:{}", args.get_str("--host"), args.get_str("--port"));
    println!("listen on {}", &host_and_port);
    let listener = TcpListener::bind(&host_and_port[..]).unwrap();
    let cache = new_cache();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cache = cache.clone();
                thread::spawn(move || {
                    let cache = cache.lock().unwrap();
                    handle_client(stream, cache);
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
    use super::search;

    use encoding::all::{EUC_JP, UTF_8};
    use encoding::{DecoderTrap, EncoderTrap, Encoding};
    #[test]
    fn test_search() {
        let utf8_bytes = EUC_JP
            .encode("ともえごぜん", EncoderTrap::Ignore)
            .unwrap();
        let result = search(utf8_bytes.as_slice());
        println!("{:?}", result);
        assert!(result.is_ok());
        let euc_jp_str = EUC_JP
            .decode(result.unwrap().as_slice(), DecoderTrap::Ignore)
            .unwrap();
        let utf8_encoded_arr = UTF_8
            .encode(&euc_jp_str, EncoderTrap::Ignore)
            .unwrap();
        assert_eq!(
            UTF_8.decode(utf8_encoded_arr.as_slice(), DecoderTrap::Ignore).unwrap(),
            "1/巴御前/ともえごぜん/トモエゴゼン\n"
        )
    }
}