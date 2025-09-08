use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender};

pub struct Runner<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send,
{
    incoming: Receiver<Req>,
    outgoing: Sender<Ret<Req>>,
}

#[derive(Debug)]
pub enum RunnerError<Res> {
    Recv(RecvError),
    Send(SendError<Res>),
}

pub trait ControlExecuteMessage: Send + Sync + 'static {
    type Res;
    fn execute(self) -> ControlFlow<(), Self::Res>;
}

/// Makes a request that a runner's [`ControlExecuteMessage`] can identify and return a [`ControlFlow::Break`]
pub trait StopRunner<Req> {
    fn get(&self) -> Req;
}

type Ret<T> = <T as ControlExecuteMessage>::Res;

impl<Req> Runner<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    pub fn make_unbound() -> (Sender<Req>, Receiver<Ret<Req>>) {
        let (res_send, res_recv) = mpsc::channel();
        let (req_send, req_recv) = mpsc::channel();
        Self::make_bound(req_recv, res_send);
        (req_send, res_recv)
    }
    pub fn make_bound(
        req_recv: Receiver<Req>,
        res_send: Sender<Ret<Req>>,
    ) -> std::thread::JoinHandle<()> {
        Runner {
            incoming: req_recv,
            outgoing: res_send,
        }
        .run_thread()
    }
    pub fn run_thread(mut self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || while self.execute_one().unwrap().is_continue() {})
    }
    fn execute_one(&mut self) -> Result<ControlFlow<()>, RunnerError<Ret<Req>>> {
        let msg = self.incoming.recv().map_err(RunnerError::Recv)?;
        let res = msg.execute();
        Ok(match res {
            ControlFlow::Continue(m) => {
                self.outgoing.send(m).map_err(RunnerError::Send)?;
                ControlFlow::Continue(())
            }
            ControlFlow::Break(()) => ControlFlow::Break(()),
        })
    }
}
