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
        handle(&req, &path)
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
fn handle<P: AsRef<Path>>(req: &Request, path: &P) -> Response{
    let path = path.as_ref();
    let path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Response::empty_404(),
    };

    let potential_file = {
        let mut path = path.to_path_buf();
        for component in req.url().split('/') {
            path.push(component);
        }
        path
    };

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

    let extension = potential_file.extension().and_then(std::ffi::OsStr::to_str);

    let file = match fs::File::open(&potential_file) {
        Ok(f) => f,
        Err(_) => return Response::empty_404(),
    };

    let etag = (fs::metadata(&potential_file)
        .map(|meta| filetime::FileTime::from_last_modification_time(&meta).seconds_relative_to_1970())
        .unwrap_or(time::now().tm_nsec as u64)
        ^ 0xd3f40305c9f8e911u64).to_string();

    Response::from_file(extension_to_mime(extension), file)
        .with_etag(req, etag)
        .with_public_cache(3600)
}

include!("extension_to_mime.rs");
