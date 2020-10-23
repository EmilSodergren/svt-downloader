use percent_encoding::percent_decode_str;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::process::Command;
use tiny_http::Server;

type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize, Debug)]
struct Config {
    download_dir: String,
    port: u16,
}

fn main() -> Result<()> {
    let file_reader = BufReader::new(File::open("config.json")?);
    let config: Config = serde_json::from_reader(file_reader)?;
    if !std::path::Path::new(&config.download_dir).exists() {
        return Err(Error::new(ErrorKind::Other, "Download dir does not exist"));
    }
    let mut netrc_file = dirs::home_dir().ok_or(Error::new(ErrorKind::Other, "No home dir"))?;
    netrc_file.push(".netrc");
    let netrc_file_reader = BufReader::new(File::open(netrc_file.as_path())?);
    let netrc = netrc::Netrc::parse(netrc_file_reader).map_err(|e| match e {
        netrc::Error::Io(e) => e,
        netrc::Error::Parse(s, _) => Error::new(ErrorKind::Other, s),
    })?;
    std::env::set_current_dir(&config.download_dir)?;
    let server = Server::http(format!("0.0.0.0:{}", config.port)).unwrap();
    loop {
        clear_dir();
        println!("Listen for incoming urls on {}", config.port);
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, e));
            }
        };
        let url: Vec<&str> = request.url().split("=").collect();
        let url = percent_decode_str(url[1])
            .decode_utf8()
            .map_err(|e| Error::new(ErrorKind::Other, e))?
            .into_owned();
        println!("Received request for downloading: {}", &url);
        match download(&url) {
            Ok(_) => {
                request.respond(tiny_http::Response::empty(200))?;
            }
            Err(e) => {
                request.respond(tiny_http::Response::empty(500))?;
                return Err(e);
            }
        };
        upload_ftp(&netrc).map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))?;
    }
}

fn download(url: &str) -> Result<()> {
    let output = Command::new("svtplay-dl").arg(url).output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Other,
            format!("Svtplay-dl exited with status: {}", output.status),
        ))
    }
}

fn upload_ftp(netrc: &netrc::Netrc) -> Result<()> {
    let (ref host, _) = netrc.hosts[0];
    Command::new("lftp")
        .arg(format!("{}:21", host.to_string()))
        .arg("-e")
        .arg(format!("cd TvFromPi; put {}; exit 0", get_file_name()?))
        .output()?;

    Ok(())
}

fn clear_dir() {
    std::fs::read_dir(".")
        .unwrap()
        .map(|res| res.map(|e| e.path()).unwrap())
        .for_each(|f| std::fs::remove_file(f).unwrap());
}

fn get_file_name() -> Result<String> {
    Ok(std::fs::read_dir(".")?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>>>()?[0]
        .to_owned()
        .to_str()
        .unwrap()
        .to_owned())
}
