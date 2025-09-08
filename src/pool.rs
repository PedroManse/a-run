use crate::runner::{ControlExecuteMessage, StopRunner};
use crate::queue::Runner;
use std::marker::PhantomData;
use std::ops::ControlFlow;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{Receiver, RecvError, SendError, Sender, TryRecvError};
use std::thread::JoinHandle;

mod api;
mod balancer;
mod close;
pub use api::*;
use balancer::*;
pub use close::*;

type Ret<T> = <T as ControlExecuteMessage>::Res;

#[derive(Debug)]
pub struct Pooled<T>(usize, T);

impl<T> Pooled<T> {
    fn pack(id: usize, v: T) -> Self {
        Self(id, v)
    }
    fn unpack(self) -> (usize, T) {
        (self.0, self.1)
    }
}

impl<T> ControlExecuteMessage for Pooled<T>
where
    T: ControlExecuteMessage,
{
    type Res = Pooled<Ret<T>>;
    fn execute(self) -> std::ops::ControlFlow<(), Self::Res> {
        match self.1.execute() {
            std::ops::ControlFlow::Break(()) => std::ops::ControlFlow::Break(()),
            std::ops::ControlFlow::Continue(c) => {
                std::ops::ControlFlow::Continue(Pooled::pack(self.0, c))
            }
        }
    }
}

pub struct Chan<T> {
    send: Sender<T>,
    recv: Receiver<T>,
}

impl<T> Chan<T> {
    fn new() -> Self {
        let (send, recv) = std::sync::mpsc::channel();
        Self { send, recv }
    }
}

impl<T> Default for Chan<T> {
    fn default() -> Self {
        Chan::new()
    }
}

pub struct PoolConDef<Req> {
    pooled_chan: Chan<Pooled<Req>>,
}

#[derive(Debug)]
pub struct PoolCon<Req> {
    _thread: std::thread::JoinHandle<()>,
    send_pooled_req: Sender<Pooled<Req>>,
}

impl<Req> PoolConDef<Req>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    fn new() -> Self {
        Self {
            pooled_chan: Chan::new(),
        }
    }
    fn run(self, send_pooled_res: Sender<Pooled<Ret<Req>>>) -> PoolCon<Req> {
        let thread = Runner::make_bound(self.pooled_chan.recv, send_pooled_res);
        PoolCon {
            _thread: thread,
            send_pooled_req: self.pooled_chan.send,
        }
    }
}

impl<Req> PoolCon<Req> {
    fn send(&self, req: Pooled<Req>) -> Result<(), SendError<Pooled<Req>>> {
        self.send_pooled_req.send(req)
    }
}

pub struct Pool<Req, const CCOUNT: usize>
where
    Req: ControlExecuteMessage,
{
    user_request_channel: Chan<ControlFlow<(), Req>>,
    user_response_channel: Chan<Ret<Req>>,
    pooled_request_channel: [PoolConDef<Req>; CCOUNT],
    pooled_response_channel: Chan<Pooled<Ret<Req>>>,
}

impl<Req, const CCOUNT: usize> Default for Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    fn default() -> Self {
        Self {
            user_request_channel: Chan::new(),
            user_response_channel: Chan::new(),
            pooled_request_channel: [(); CCOUNT].map(|_| PoolConDef::new()),
            pooled_response_channel: Chan::new(),
        }
    }
}

impl<Req, const CCOUNT: usize> Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(self) -> PoolApi<Req, CCOUNT> {
        let Chan {
            send: send_pooled_response,
            recv: recv_pooled_response,
        } = self.pooled_response_channel;
        let Chan {
            send: user_send_req,
            recv: recv_user_req,
        } = self.user_request_channel;
        let Chan {
            send: user_send_response,
            recv: user_recv_response,
        } = self.user_response_channel;

        let runners = self
            .pooled_request_channel
            .map(|con_def| con_def.run(send_pooled_response.clone()));

        let manager_thread = std::thread::spawn(move || {
            let pb = PoolBalancer::<CCOUNT>::new();
            loop {
                match recv_user_req.try_recv() {
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        panic!("Channel closed")
                    }
                    Ok(ControlFlow::Continue(req)) => {
                        let runner_ref = pb.send();
                        let pooled_req = Pooled::pack(runner_ref.id, req);
                        runners[runner_ref.id].send(pooled_req).unwrap();
                        continue;
                    }
                    Ok(ControlFlow::Break(())) => {
                        return close::PoolCloserDef {
                            balancer: pb,
                            recv_pooled_response,
                            runners,
                            user_send_response,
                        };
                    }
                };
                match recv_pooled_response.try_recv() {
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        panic!("Channel closed")
                    }
                    Ok(pooled_response) => {
                        let (runner_id, response) = pooled_response.unpack();
                        pb.done(runner_id);
                        user_send_response.send(response).unwrap();
                    }
                }
                std::thread::yield_now();
            }
        });

        PoolApi {
            send_req: user_send_req,
            recv_res: user_recv_response,
            manager_thread,
        }
    }
}
