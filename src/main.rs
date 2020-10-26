use anyhow::{anyhow, ensure, Context, Error, Result};
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use tiny_http::Server;

#[derive(Deserialize, Debug)]
struct Config {
    download_dir: String,
    port: u16,
}

fn main() -> Result<()> {
    let file_reader = BufReader::new(File::open("config.json")?);
    let config: Config = serde_json::from_reader(file_reader).context("Can't read config file")?;
    ensure!(
        Path::new(&config.download_dir).exists(),
        "Download dir does not exist"
    );
    let mut netrc_file = dirs::home_dir().ok_or(anyhow!("No home dir"))?;
    netrc_file.push(".netrc");
    let netrc_file_reader = BufReader::new(File::open(netrc_file.as_path())?);
    let netrc = netrc::Netrc::parse(netrc_file_reader)
        .map_err(|err| match err {
            netrc::Error::Io(e) => anyhow!("{}", e),
            netrc::Error::Parse(s, _) => anyhow!("{}", s),
        })
        .context("Failed to read .netrc")?;
    std::env::set_current_dir(&config.download_dir)?;
    let server = Server::http(format!("0.0.0.0:{}", config.port)).unwrap();
    loop {
        match download_loop(&config, &server, &netrc) {
            Ok(_) => {}
            Err(err) => println!("{:?}", err),
        };
    }
}

fn download_loop(config: &Config, server: &Server, netrc: &netrc::Netrc) -> Result<()> {
    clear_dir()?;
    println!("Listen for incoming urls on {}", config.port);
    let request = match server.recv() {
        Ok(rq) => rq,
        Err(e) => {
            return Err(anyhow!("{}", e));
        }
    };
    let url: Vec<&str> = request.url().split("=").collect();
    let url = percent_decode_str(url[1])
        .decode_utf8()
        .context("Failed percent decode str")?;
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
    upload_ftp(netrc).context("Upload failed")
}

fn download(url: &str) -> Result<()> {
    println!("Downloading");
    let output = Command::new("svtplay-dl")
        .arg("-q")
        .arg("2200")
        .arg("-Q")
        .arg("600")
        .arg("--remux")
        .arg("--silent-semi")
        .arg(url)
        .output()?;

    if output.status.success() {
        println!("Download complete");
        Ok(())
    } else {
        Err(anyhow!(
            "Svtplay-dl exited with {}\nStdout: {}\nStderr: {}",
            output.status,
            std::str::from_utf8(&output.stdout)?,
            std::str::from_utf8(&output.stderr)?
        ))
    }
}

fn upload_ftp(netrc: &netrc::Netrc) -> Result<()> {
    let (ref host, _) = netrc.hosts[0];
    println!("Uploading to ftp");
    Command::new("lftp")
        .arg(format!("{}:21", host.to_string()))
        .arg("-e")
        .arg(format!("cd TvFromPi; put {}; exit 0", get_file_name()?))
        .output()?;
    println!("Upload complete");

    Ok(())
}

#[inline]
fn list_folder() -> Result<std::fs::ReadDir> {
    std::fs::read_dir(".").map_err(Error::msg)
}

fn clear_dir() -> Result<()> {
    for entry in list_folder()? {
        let path = entry?.path();
        std::fs::remove_file(path).map_err(Error::msg)?;
    }
    Ok(())
}

// The download directory should never contain more than one file at a time.
// Thus take(1)
fn get_file_name() -> Result<String> {
    for entry in list_folder()? {
        return Ok(entry?.path().to_string_lossy().into_owned());
    }
    return Err(anyhow!("No file was found"));
}
