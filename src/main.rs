use aio_rs::{ActionRequest, ActionResult};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (runner, send_aio, recv_aio) = aio_rs::runner::Runner::new();
    let (runner2, send_aio2, recv_aio2) = aio_rs::runner::Runner::new();
    runner.run_thread();
    runner2.run_thread();

    queue_wait_cycle(&send_aio, &recv_aio, &send_aio2, &recv_aio2)?;
    //queue_all_wait_all(&send_aio, &recv_aio, &send_aio2, &recv_aio2)?;

    send_aio.send(ActionRequest::StopRunner)?;
    send_aio2.send(ActionRequest::StopRunner)?;
    Ok(())
}

fn queue_wait_cycle(
    send_aio: &Sender<ActionRequest>,
    recv_aio: &Receiver<Result<ActionResult, std::io::Error>>,
    send_aio2: &Sender<ActionRequest>,
    recv_aio2: &Receiver<Result<ActionResult, std::io::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..512 {
        let opt = OpenOptions::new().read(true).to_owned();
        let ci_sh = ActionRequest::Open(PathBuf::from("ci.sh"), opt);
        let opt = OpenOptions::new().read(true).to_owned();
        let txt = ActionRequest::Open(PathBuf::from("txt"), opt);

        send_aio.send(ci_sh)?;
        send_aio2.send(txt)?;
    }

    for _ in 0..512 {
        recv_aio2.recv()??;
        recv_aio.recv()??;
    }
    Ok(())
}

fn queue_all_wait_all(
    send_aio: &Sender<ActionRequest>,
    recv_aio: &Receiver<Result<ActionResult, std::io::Error>>,
    send_aio2: &Sender<ActionRequest>,
    recv_aio2: &Receiver<Result<ActionResult, std::io::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..512 {
        let opt = OpenOptions::new().read(true).to_owned();
        let ci_sh = ActionRequest::Open(PathBuf::from("ci.sh"), opt);
        let opt = OpenOptions::new().read(true).to_owned();
        let txt = ActionRequest::Open(PathBuf::from("txt"), opt);

        send_aio.send(ci_sh)?;
        send_aio2.send(txt)?;
    }
    for _ in 0..512 {
        recv_aio2.recv()??;
        recv_aio.recv()??;
    }
    Ok(())
}
