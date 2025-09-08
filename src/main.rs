use std::path::PathBuf;
use a_run::aio::ActionRequest;
use a_run::runner::RunnerApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let aio = a_run::oneshot::RunnerApi::<a_run::aio::ActionRequest>::new();
    let req = ActionRequest::Open(PathBuf::from("ci.sh"), std::fs::OpenOptions::new().read(true).to_owned());
    let ret = aio.send(req)?;
    let file = ret.recv()??;
    println!("{file:?}");
    Ok(())
}
