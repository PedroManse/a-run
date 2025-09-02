use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender};

pub struct Runner<Req>
where
    Req: ControlExecuteMessage,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug,
{
    incoming: Receiver<Req>,
    outgoing: Sender<<Req as ControlExecuteMessage>::Res>,
}

#[derive(Debug)]
pub enum RunnerError<Res>
where
    Res: std::fmt::Debug,
{
    Recv(RecvError),
    Send(SendError<Res>),
}

pub trait ExecuteMessage {
    type Res;
    fn execute(self) -> Self::Res;
}

/// Implement ControlExecuteMessage for requests that implement ExecuteMessage.
/// This should only be used for prototyping
impl<E: ExecuteMessage> ControlExecuteMessage for E {
    type Res = <E as ExecuteMessage>::Res;
    fn execute(self) -> ControlFlow<(), Self::Res> {
        ControlFlow::Continue(<Self as ExecuteMessage>::execute(self))
    }
}

pub trait ControlExecuteMessage {
    type Res;
    fn execute(self) -> ControlFlow<(), Self::Res>;
}

type Ret<T> = <T as ControlExecuteMessage>::Res;

impl<Req> Runner<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pub fn new() -> (Sender<Req>, Receiver<Ret<Req>>) {
        let (res_send, res_recv) = mpsc::channel();
        let (req_send, req_recv) = mpsc::channel();
        Self::make_runner(req_recv, res_send);
        (req_send, res_recv)
    }
    pub fn make_runner(
        req_recv: Receiver<Req>,
        res_send: Sender<Ret<Req>>,
    ) {
        Runner {
            incoming: req_recv,
            outgoing: res_send,
        }
        .run_thread();
    }
    pub fn run_thread(mut self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || while self.execute_one().unwrap().is_continue() {})
    }
    fn execute_one(&mut self) -> Result<ControlFlow<()>, RunnerError<Ret<Req>>> {
        let msg = self.incoming.recv().map_err(RunnerError::Recv)?;
        let res = msg.execute();
        Ok(match res {
            ControlFlow::Continue(m) => {
                let msg: Ret<Req> = m.into();
                self.outgoing.send(msg.into()).map_err(RunnerError::Send)?;
                ControlFlow::Continue(())
            }
            ControlFlow::Break(()) => ControlFlow::Break(()),
        })
    }
}
