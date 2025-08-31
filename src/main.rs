use a_run::aio::{ActionRequest, ActionResult};
use std::fs::OpenOptions;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (send_aio, recv_aio) = a_run::runner::Runner::new();

    send_aio.send(ActionRequest::Open(
        PathBuf::from("Cargo.toml"),
        OpenOptions::new().read(true).to_owned(),
    ))?;

    let in_file = match recv_aio.recv()?? {
        ActionResult::Open(f) => f,
        _ => panic!(),
    };

    send_aio.send(ActionRequest::Read(in_file))?;
    send_aio.send(ActionRequest::Open(
        PathBuf::from("txt"),
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .to_owned(),
    ))?;

    let (file, mut content) = match recv_aio.recv()?? {
        ActionResult::Read(f, c) => (f, c),
        _ => panic!(),
    };

    content.reverse();

    let out_file = match recv_aio.recv()?? {
        ActionResult::Open(f) => f,
        _ => panic!(),
    };

    send_aio.send(ActionRequest::WriteAll(out_file, content))?;
    recv_aio.recv()??;

    send_aio.send(ActionRequest::StopRunner)?;
    Ok(())
}
