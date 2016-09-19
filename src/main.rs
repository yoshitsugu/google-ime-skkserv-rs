extern crate hyper;
extern crate rustc_serialize;
extern crate encoding;
extern crate docopt;

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io;
use std::io::Read;
use std::io::Write;
use std::str;
use hyper::client::Client;
use rustc_serialize::json;
use rustc_serialize::json::Json;
use encoding::{Encoding, DecoderTrap, EncoderTrap};
use encoding::all::EUC_JP;
use docopt::Docopt;


const CLIENT_END: u8       = b'0';
const CLIENT_REQUEST: u8   = b'1';
const CLIENT_VERSION: u8   = b'2';
const CLIENT_HOST: u8      = b'3';
//const CLIENT_COMP: u8      = 4;

const SERVER_VERSION: &'static str = "Google IME SKK Server in Rust.0.0";

#[derive(Debug)]
enum SearchError {
    Io(io::Error),
    Json(json::BuilderError),
    Msg(String)
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

fn search(read: &[u8]) -> Result<Vec<u8>, SearchError> {
    let client = Client::new();
    let kana = EUC_JP.decode(&read, DecoderTrap::Ignore).unwrap();
    let url = format!("http://www.google.com/transliterate?langpair=ja-Hira%7Cja&text={},", &kana);
    let mut res = match client.get(&url).send() {
        Ok(r) => r,
        Err(_) => return Ok(EUC_JP.encode("4\n", EncoderTrap::Ignore).unwrap())
    };
    let mut s = String::new();
    try!(res.read_to_string(&mut s));
    let json = try!(Json::from_str(s.as_str()));
    let array = try!(json.as_array().ok_or("cannot found expected json structure".to_owned()));
    let kanji = try!(array[0].as_array().ok_or("cannot found expected json structure".to_owned()));
    let _kanjis = try!(kanji[1].as_array().ok_or("cannot found expected json structure".to_owned()));
    let mut kanjis = "1".to_string();
    for _k in _kanjis {
        let _ks = try!(_k.as_string().ok_or("cannot found expected json structure".to_owned()));
        kanjis = format!("{}/{}", kanjis, _ks);
    }
    kanjis = format!("{}\n", kanjis);
    let r = EUC_JP.encode(&kanjis, EncoderTrap::Ignore).unwrap();
    return Ok(r);
}

fn handle_client(mut stream: TcpStream) {
    loop {
        let mut read = [0; 512];
        match stream.read(&mut read) {
            Ok(n) => {
                if n == 0 {
                    break;
                }
                match read[0] {
                    CLIENT_END => {
                        stream.write(&[b'0',b'\n']).unwrap();
                    }
                    CLIENT_REQUEST => {
                        match search(&read[1..(n-1)]) {
                            Ok(result) => {
                                stream.write(result.as_slice()).unwrap();
                            }
                            Err(err) => {
                                println!("{:?}", err);
                                stream.write(&[b'0',b'\n']).unwrap();
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
                        stream.write(&[b'0',b'\n']).unwrap();
                    }
                }
                        
            }
            Err(err) => {
                println!("{:?}", err);
                stream.write(&[b'0',b'\n']).unwrap();
            }
        }
    }
}

static USAGE: &'static str = "
Usage:
  gskkserv [--host=<host>] [--port=<port>]

Options:
  -h --host=<host>     Server Host [default: 0.0.0.0]
  -p --port=<port>     Server Port [default: 55100]
";

fn main() {
    let args = Docopt::new(USAGE)
                      .and_then(|dopt| dopt.parse())
                      .unwrap_or_else(|e| e.exit());
    let host_and_port = format!("{}:{}", args.get_str("--host"),args.get_str("--port"));
    println!("listen on {}", &host_and_port);
    let listener = TcpListener::bind(&host_and_port[..]).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(_) => {
                println!("Error!");
            }
        }
    }
}
