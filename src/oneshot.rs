use crate::runner::{ControlExecuteMessage, StopRunner};
use std::fmt::Display;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
type Ret<T> = <T as ControlExecuteMessage>::Res;

#[derive(Debug)]
pub struct OneShotSendErr<T>(T);
impl<T: std::fmt::Debug> Display for OneShotSendErr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to send {:?}", self.0)
    }
}
impl<T: std::fmt::Debug> std::error::Error for OneShotSendErr<T> {}

pub struct OneShot<Req>
where
    Req: ControlExecuteMessage,
{
    req: Req,
    chan: oneshot::Sender<Ret<Req>>,
}

impl<Req> OneShot<Req>
where
    Req: ControlExecuteMessage,
{
    fn unpack(self) -> (Req, oneshot::Sender<Ret<Req>>) {
        (self.req, self.chan)
    }
}

pub struct RunnerInternals<Req>
where
    Req: ControlExecuteMessage,
{
    reqs: Receiver<OneShot<Req>>,
}

pub struct RunnerApi<Req>
where
    Req: ControlExecuteMessage,
{
    send_one_shot_req: Sender<OneShot<Req>>,
    thread: JoinHandle<RunnerInternals<Req>>,
}

impl<Req> RunnerApi<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    fn _new() -> Self {
        let (send, reqs) = mpsc::channel();
        let internal: RunnerInternals<Req> = RunnerInternals { reqs };
        let thread = std::thread::spawn(move || {
            loop {
                let (req, chan) = internal.reqs.recv().unwrap().unpack();
                match req.execute() {
                    std::ops::ControlFlow::Continue(v) => {
                        chan.send(v).unwrap();
                    }
                    std::ops::ControlFlow::Break(()) => return internal,
                };
            }
        });
        Self {
            send_one_shot_req: send,
            thread,
        }
    }
    fn _send(&self, req: Req) -> Result<oneshot::Receiver<Ret<Req>>, OneShotSendErr<Req>> {
        let (chan, user_recv) = oneshot::channel();
        let msg = OneShot { req, chan };
        self.send_one_shot_req.send(msg).map_err(|e| OneShotSendErr(e.0.req))?;
        Ok(user_recv)
    }
}

impl<Req> crate::runner::RunnerApi for RunnerApi<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    type Req = Req;
    type SendAck = Result<oneshot::Receiver<Ret<Req>>, OneShotSendErr<Req>>;
    type CloseResult = Result<RunnerInternals<Req>, OneShotSendErr<Req>>;
    fn send(&self, req: Self::Req) -> Self::SendAck {
        self._send(req)
    }
    fn new() -> Self {
        Self::_new()
    }
    // TODO better error
    fn close(self, s: impl StopRunner<Req>) -> Self::CloseResult {
        self.send(s.get())?.recv().unwrap();
        Ok(self.thread.join().unwrap())
    }
}
