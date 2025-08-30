use crate::{ActionRequest, ActionResult};
use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender};

pub struct Runner {
    incoming: Receiver<ActionRequest>,
    outgoing: Sender<Result<ActionResult, std::io::Error>>,
}

#[derive(Debug)]
pub enum RunnerError {
    Recv(RecvError),
    Send(SendError<Result<ActionResult, std::io::Error>>),
}

impl Runner {
    pub fn new() -> (
        Runner,
        Sender<ActionRequest>,
        Receiver<Result<ActionResult, std::io::Error>>,
    ) {
        let (req_send, req_recv) = mpsc::channel();
        let (res_send, res_recv) = mpsc::channel();
        (
            Runner {
                incoming: req_recv,
                outgoing: res_send,
            },
            req_send,
            res_recv,
        )
    }
    pub fn run_thread(mut self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(
            move || {
                while self.execute_one().unwrap() != ControlFlow::Break(()) {}
            },
        )
    }
    fn execute_one(&mut self) -> Result<ControlFlow<()>, RunnerError> {
        let msg = self.incoming.recv().map_err(RunnerError::Recv)?;
        Ok(if let ActionRequest::StopRunner = msg {
            ControlFlow::Break(())
        } else {
            let res = msg.execute();
            self.outgoing.send(res).map_err(RunnerError::Send)?;
            ControlFlow::Continue(())
        })
    }
}
