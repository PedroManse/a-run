use crate::runner::Runner;

use super::ControlExecuteMessage;
use std::sync::mpsc::{Receiver, Sender};
//use std::sync::{Arc, Mutex};

type Ret<T> = <T as ControlExecuteMessage>::Res;
#[derive(Debug)]
pub struct Pooled<T>(usize, T);

impl<T> Pooled<T> {
    fn wrap(id: usize, v: T) -> Self {
        Self(id, v)
    }
    fn unwrap(self) -> (usize, T) {
        (self.0, self.1)
    }
}

impl<T> ControlExecuteMessage for Pooled<T>
where
    T: ControlExecuteMessage,
{
    type Res = Pooled<Ret<T>>;
    fn execute(self) -> std::ops::ControlFlow<(), Self::Res> {
        println!("[POOLED RUNNER #{}] START", self.0);
        let ret = match self.1.execute() {
            std::ops::ControlFlow::Break(()) => std::ops::ControlFlow::Break(()),
            std::ops::ControlFlow::Continue(c) => {
                std::ops::ControlFlow::Continue(Pooled::wrap(self.0, c))
            }
        };
        println!("[POOLED RUNNER #{}] END", self.0);
        ret
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

pub struct Pool<Req: ControlExecuteMessage, const CCOUNT: usize> {
    user_request_channel: Chan<Req>,
    user_response_channel: Chan<Ret<Req>>,
    pooled_request_channel: [Chan<Pooled<Req>>; CCOUNT],
    pooled_response_channel: Chan<Pooled<Ret<Req>>>,
}

impl<Req, const CCOUNT: usize> Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pub fn new() -> Self {
        Self {
            user_request_channel: Chan::new(),
            user_response_channel: Chan::new(),
            pooled_request_channel: [(); CCOUNT].map(|_| Chan::new()),
            pooled_response_channel: Chan::new(),
        }
    }

    pub fn start(self) -> (Sender<Req>, Receiver<Ret<Req>>) {
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

        let runners = self.pooled_request_channel.map(|Chan { send, recv }| {
            Runner::make_runner(recv, send_pooled_response.clone());
            send
        });

        // analytics
        //let a = Arc::new(Mutex::new(0));
        //let a1 = Arc::clone(&a);
        //let a2 = Arc::clone(&a);

        std::thread::spawn(move || {
            let mut runner_id = 0;
            loop {
                //*a1.lock().unwrap() += 1;
                //println!("{a1:?}");
                let req = recv_user_req.recv().unwrap();
                let pooled_req = Pooled::wrap(runner_id, req);
                runners[runner_id].send(pooled_req).unwrap();
                runner_id = (runner_id + 1) % CCOUNT;
            }
        });

        std::thread::spawn(move || {
            loop {
                //*a2.lock().unwrap() -= 1;
                //println!("{a2:?}");
                let pooled_response = recv_pooled_response.recv().unwrap();
                let (_runner_id, response) = pooled_response.unwrap();
                user_send_response.send(response).unwrap();
            }
        });

        (user_send_req, user_recv_response)
    }
}
