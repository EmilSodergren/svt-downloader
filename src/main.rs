use percent_encoding::percent_decode_str;
use std::process::Command;
use tiny_http::Server;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let server = Server::http("0.0.0.0:3211").unwrap();
    loop {
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {
                println!("error: {}", e);
                break;
            }
        };
        let url: Vec<&str> = request.url().split("=").collect();
        let url = percent_decode_str(url[1]).decode_utf8()?.into_owned();
        download(&url);
        println!("URL: {:?}", url);
        println!("Remote Addr: {:?}", request.remote_addr());
        request.respond(tiny_http::Response::empty(200))?;
    }
    Ok(())
}

fn download(url: &str) {
    Command::new("sh")
        .arg("-c")
        .arg("svtplay-dl")
        .arg(url)
        .spawn()?;
}

fn upload_ftp() {}
