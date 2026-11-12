# Google IME skkserv in Rust

[![Test](https://github.com/yoshitsugu/google-ime-skkserv-rs/actions/workflows/test.yml/badge.svg)](https://github.com/yoshitsugu/google-ime-skkserv-rs/actions/workflows/test.yml)

SKK server for Google IME in Rust.

## Build and run

Install the latest stable Rust with [rustup](https://rustup.rs/), then build this checkout:

```sh
cargo install --path . --locked
gskkserv
```

The server listens on `0.0.0.0:55100` by default. To listen locally on a different port:

```sh
gskkserv --host=127.0.0.1 --port=55101
```

Use `-d` to daemonize on Unix. Windows supports foreground mode only.
The project uses Rust edition 2024; the updated dependencies and tests were verified
with stable Rust 1.99.1. CI follows the latest stable Rust on Linux, macOS, and Windows.

`cargo install gskkserv --locked` installs the published crates.io release,
which may be older than this checkout.

## Google Japanese Input API

The server uses [Google CGI API for Japanese Input](https://www.google.co.jp/ime/cgiapi.html)
at `https://www.google.com/transliterate`, with `langpair=ja-Hira|ja` and a URL-encoded
UTF-8 `text` parameter. The SKK client connection uses EUC-JP.

On 2026-11-12, both the original HTTP endpoint and HTTPS endpoint returned HTTP 200
and conversion candidates for `ともえごぜん`, including `巴御前`. The documented
JSON response format still works. Requests now use HTTPS, check HTTP error statuses,
and time out after 10 seconds. This check confirms current operation; it does not
establish future availability of the external service.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Normal tests do not contact Google. To explicitly check the live API:

```sh
cargo test --locked live_google_api -- --ignored
```

Release builds use Rustls for HTTPS without requiring OpenSSL. The release workflow
builds Linux GNU/musl, Intel/Apple Silicon macOS, and Windows MSVC binaries.
