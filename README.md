# Google IME skkserv in Rust
[![Build Status](https://travis-ci.org/yoshitsugu/google-ime-skkserv-rs.svg)](https://travis-ci.org/yoshitsugu/google-ime-skkserv-rs)
[![Build status](https://ci.appveyor.com/api/projects/status/9tb7evyxth0hnl6o?svg=true)](https://ci.appveyor.com/project/yoshitsugu/google-ime-skkserv-rs)  
SKK server for Google IME in Rust.  

## Usage
0. Prepare Rust environment.
1. Clone this repository.
2. Run `cargo install --path .`
3. Run `gskkserv`, then skkserv will listen port 55100
   - You can use `-d` option to daemonize the process
