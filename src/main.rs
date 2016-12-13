#[macro_use]
extern crate clap;
extern crate rouille;
extern crate filetime;
extern crate time;

use std::path::Path;
use clap::{App, Arg, AppSettings};
use rouille::*;

fn main() {
    let m = App::new("servent")
        .author(crate_authors!())
        .version(crate_version!())
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("addr")
                .help("Address the server will bind to (e.g. 127.0.0.1:3000)")
                .validator(is_valid)
                .index(1)
                .required(true)
        )
        .arg(
            Arg::with_name("root-path")
                .short("p")
                .long("path")
                .takes_value(true)
                .value_name("PATH")
                .default_value(".")
                .help("Path of the root directory of the server")
        )
        .get_matches();

    let addr = m.value_of("addr").unwrap();
    // Take ownership to make it usable in the closure
    let path = m.value_of("root-path").unwrap().to_owned();

    rouille::start_server(addr, move |req| {
        handle(&path, req)
    });
}

use std::net::ToSocketAddrs;
fn is_valid(url: String) -> Result<(), String> {
    url.to_socket_addrs()
        .map(|_| ())
        .map_err(|_| "Not a valid address".to_string())
}

use std::fs;

// Code below is a modified version of `rouille::match_assets`

fn handle<P: AsRef<Path>>(path: P, req: &Request) -> Response{
    let path = path.as_ref();
    let path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            println!("Root path doesn't exist!");
            return Response::empty_404()
        },
    };

    let potential_file = path.join(&req.url()[1..]);

    let mut potential_file = match potential_file.canonicalize() {
        Ok(f) => f,
        Err(_) => return Response::empty_404(),
    };

    if !potential_file.starts_with(path) {
        return Response::empty_404();
    }

    match fs::metadata(&potential_file) {
        Ok(ref m) if m.is_file() => (),
        Ok(ref m) if m.is_dir() => potential_file = potential_file.join("index.html"),
        _ => return Response::empty_404(),
    };

    let extension = potential_file.extension().and_then(|s| s.to_str());

    let file = match fs::File::open(&potential_file) {
        Ok(f) => f,
        Err(_) => return Response::empty_404(),
    };

    let etag: String = (fs::metadata(&potential_file)
        .map(|meta| filetime::FileTime::from_last_modification_time(&meta).seconds_relative_to_1970())
        .unwrap_or(time::now().tm_nsec as u64)
        ^ 0xd3f40305c9f8e911u64).to_string();

    let not_modified: bool = req.header("If-None-Match")
        .map(|req_etag| req_etag == etag)
        .unwrap_or(false);

    if not_modified {
        return Response {
            status_code: 304,
            headers: vec![
                ("Cache-Control".to_owned(), "public, max-age=3600".to_owned()),
                ("ETag".to_owned(), etag.to_string())
            ],
            data: ResponseBody::empty()
        };
    }

    Response {
        status_code: 200,
        headers: vec![
            ("Cache-Control".to_owned(), "public, max-age=3600".to_owned()),
            ("Content-Type".to_owned(), extension_to_mime(extension).to_owned()),
            ("ETag".to_owned(), etag.to_string())
        ],
        data: ResponseBody::from_file(file),
    }
}

include!("extension_to_mime.rs");
