extern crate daemonize;
extern crate docopt;
extern crate encoding;
extern crate gskkserv;
extern crate reqwest;
extern crate rustc_serialize;

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io;
use std::io::Read;
use std::io::Write;
use std::str;
use rustc_serialize::json;
use rustc_serialize::json::Json;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use encoding::all::EUC_JP;
use docopt::Docopt;
use daemonize::Daemonize;
use gskkserv::cache::{new_cache, LockedCache};

const CLIENT_END: u8 = b'0';
const CLIENT_REQUEST: u8 = b'1';
const CLIENT_VERSION: u8 = b'2';
const CLIENT_HOST: u8 = b'3';
//const CLIENT_COMP: u8      = 4;

const SERVER_VERSION: &'static str = "Google IME SKK Server in Rust.0.0";
const PID_DIR: &'static str = "/tmp/gskkserv.pid";
const WORK_DIR: &'static str = "/tmp";
const GOOGLE_IME_URL: &'static str =
    "http://www.google.com/transliterate?langpair=ja-Hira%7Cja&text=";

#[derive(Debug)]
enum SearchError {
    Io(io::Error),
    Json(json::BuilderError),
    Msg(String),
    Request(reqwest::Error)
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

impl From<String> for SearchError {
    fn from(err: String) -> SearchError {
        SearchError::Msg(err)
    }
}

impl From<reqwest::Error> for SearchError {
    fn from(err: reqwest::Error) -> SearchError {
        SearchError::Request(err)
    }
}

fn search(read: &[u8]) -> Result<Vec<u8>, SearchError> {
    let kana = EUC_JP.decode(&read, DecoderTrap::Ignore).unwrap();
    let url = format!("{}{}", GOOGLE_IME_URL, &kana);
    let s = reqwest::get(&url)?.text()?;
    let json = Json::from_str(s.as_str())?;
    let json_error_msg = "cannot found expected json structure".to_string();
    let array = json.as_array().ok_or(json_error_msg.clone())?;
    let kanji = array[0].as_array().ok_or(json_error_msg.clone())?;
    let _kanjis = kanji[1].as_array().ok_or(json_error_msg.clone())?;
    let mut kanjis = "1".to_string();
    for _k in _kanjis {
        let _ks = _k.as_string().ok_or(json_error_msg.clone())?;
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
                        stream.write(&[b'0', b'\n']).unwrap();
                    }
                    CLIENT_REQUEST => {
                        let cache_key = read[1..(n - 1)].to_vec();
                        // let stdout = io::stdout();
                        if cache.contains_key(&cache_key) {
                            // writeln!(&mut stdout.lock(), "hit!").unwrap();
                            stream.write(cache.get(&cache_key).unwrap()).unwrap();
                        } else {
                            // writeln!(&mut stdout.lock(), "not hit...").unwrap();
                            match search(&read[1..(n - 1)]) {
                                Ok(result) => {
                                    cache.insert(read[1..(n - 1)].to_vec(), result.clone());
                                    stream.write(result.as_slice()).unwrap();
                                }
                                Err(err) => {
                                    println!("{:?}", err);
                                    stream.write(&[b'0', b'\n']).unwrap();
                                }
                            }
                        }
                    }
                    CLIENT_VERSION => {
                        stream.write(SERVER_VERSION.as_bytes()).unwrap();
                    }
                    CLIENT_HOST => {
                        stream.write("0.0.0.0".as_bytes()).unwrap();
                    }
                    _ => {
                        stream.write(&[b'0', b'\n']).unwrap();
                    }
                }
            }
            Err(err) => {
                println!("{:?}", err);
                stream.write(&[b'0', b'\n']).unwrap();
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
                println!("Error!");
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
            Err(e) => println!("{}", e),
        }
    } else {
        listen(&args);
    }
}
