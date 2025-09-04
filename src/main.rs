use a_run::aio::{AIOStop, ActionRequest};
use a_run::pool::Pool;
use std::fs::OpenOptions;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Pool::<_, 3>::new().start();
    let open_file_request = || {
        ActionRequest::Open(
            PathBuf::from("Cargo.toml"),
            OpenOptions::new().read(true).to_owned(),
        )
    };

    for _ in 0..8 {
        pool.send(open_file_request())?;
    }
    pool.stop_and_close()?.close_capture(&AIOStop);

    // take chan, await tasks
    //let p = pool.stop()?.0.close_await(&AIOStop);
    // take chan, capture tasks
    //let p = pool.stop()?.0.close_capture(&AIOStop);
    // drop chan, await tasks (compile time error)
    //let p = pool.stop_and_close()?.close_await(&AIOStop);
    // drop chan, capture tasks
    //let p = pool.stop_and_close()?.close_capture(&AIOStop);

    //let (send_aio, recv_aio) = a_run::runner::Runner::new();

    //send_aio.send(ActionRequest::Open(
    //    PathBuf::from("Cargo.toml"),
    //    OpenOptions::new().read(true).to_owned(),
    //))?;

    //let in_file = match recv_aio.recv()?? {
    //    ActionResult::Open(f) => f,
    //    _ => panic!(),
    //};

    //send_aio.send(ActionRequest::Read(in_file))?;
    //send_aio.send(ActionRequest::Open(
    //    PathBuf::from("txt"),
    //    OpenOptions::new()
    //        .write(true)
    //        .create(true)
    //        .truncate(true)
    //        .to_owned(),
    //))?;

    //let (file, mut content) = match recv_aio.recv()?? {
    //    ActionResult::Read(f, c) => (f, c),
    //    _ => panic!(),
    //};

    //content.reverse();

    //let out_file = match recv_aio.recv()?? {
    //    ActionResult::Open(f) => f,
    //    _ => panic!(),
    //};

    //send_aio.send(ActionRequest::WriteAll(out_file, content))?;
    //recv_aio.recv()??;

    //send_aio.send(ActionRequest::StopRunner)?;
    Ok(())
}
