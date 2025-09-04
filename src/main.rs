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

    let mode = 1;
    if mode == 1 {
        // take chan, await tasks
        let (p, r) = pool.stop()?;
        p.close_await(&AIOStop);
        while let Ok(x) = r.recv() {
            println!("{x:?}");
        }
    } else if mode == 2 {
        // take chan, capture tasks
        let (p, r) = pool.stop()?;
        let xs = p.close_capture(&AIOStop, r);
        println!("{xs:?}");
    } else if mode == 3 {
        // drop chan, await tasks (compile time error)
        //let p = pool.stop_and_close()?.close_await(&AIOStop);
    } else if mode == 4{
        // drop chan, capture tasks
        let xs = pool.stop_and_close()?.close_capture(&AIOStop);
        println!("{xs:?}");
    }
    Ok(())
}
