use crate::runner::{ControlExecuteMessage, Ret};
use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender};
use std::thread::JoinHandle;

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

pub struct RunnerApi<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send,
{
    send_req: Sender<Req>,
    recv_ret: Receiver<Ret<Req>>,
    thread: JoinHandle<()>,
}

impl<Req> RunnerApi<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    pub fn recv(&self) -> Result<Ret<Req>, RecvError> {
        self.recv_ret.recv()
    }
}

impl<Req> crate::runner::RunnerApi for RunnerApi<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    type Req = Req;
    type SendAck = Result<(), SendError<Req>>;
    type CloseResult = Result<(), SendError<Req>>;
    fn new() -> Self {
        let (res_send, res_recv) = mpsc::channel();
        let (req_send, req_recv) = mpsc::channel();
        let thread = Runner::make_bound(req_recv, res_send);
        Self {
            send_req: req_send,
            recv_ret: res_recv,
            thread,
        }
    }
    fn send(&self, req: Self::Req) -> Self::SendAck {
        self.send_req.send(req)
    }
    // TODO better error
    fn close(self, s: impl crate::runner::StopRunner<Req>) -> Self::CloseResult {
        self.send_req.send(s.get())?;
        self.thread.join().unwrap();
        Ok(())
    }
}
