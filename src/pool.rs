use crate::runner::{ControlExecuteMessage, Runner};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{Receiver, RecvError, SendError, Sender};
use std::usize;

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
        match self.1.execute() {
            std::ops::ControlFlow::Break(()) => std::ops::ControlFlow::Break(()),
            std::ops::ControlFlow::Continue(c) => {
                std::ops::ControlFlow::Continue(Pooled::wrap(self.0, c))
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

impl<Req, const CCOUNT: usize> Default for Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
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

pub struct Pool<Req: ControlExecuteMessage, const CCOUNT: usize>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    user_request_channel: Chan<Req>,
    user_response_channel: Chan<Ret<Req>>,
    pooled_request_channel: [PoolConDef<Req>; CCOUNT],
    pooled_response_channel: Chan<Pooled<Ret<Req>>>,
}

pub struct PoolConDef<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pooled_chan: Chan<Pooled<Req>>,
}

pub struct PoolCon<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    _thread: std::thread::JoinHandle<()>,
    send_pooled_req: Sender<Pooled<Req>>,
}

impl<Req> PoolConDef<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
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

impl<Req> PoolCon<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    fn send(&self, req: Pooled<Req>) -> Result<(), SendError<Pooled<Req>>> {
        self.send_pooled_req.send(req)
    }
}

impl<Req, const CCOUNT: usize> Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pub fn new() -> Self {
        Self::default()
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

        let runners = self
            .pooled_request_channel
            .map(|con_def| con_def.run(send_pooled_response.clone()));
        let balancer_start_req = Arc::new(PoolBalancer::<CCOUNT>::new());
        let balancer_end_req = Arc::clone(&balancer_start_req);

        std::thread::spawn::<_, Result<(), RecvError>>(move || {
            let pb = balancer_start_req;
            loop {
                let req = recv_user_req.recv()?;
                let runner_ref = PoolBalancer::send(&pb);
                let pooled_req = Pooled::wrap(runner_ref.id, req);
                runners[runner_ref.id].send(pooled_req).unwrap();
            }
        });

        std::thread::spawn(move || {
            let pb = balancer_end_req;
            loop {
                let pooled_response = recv_pooled_response.recv().unwrap();
                let (runner_id, response) = pooled_response.unwrap();
                PoolBalancer::done(&pb, runner_id);
                user_send_response.send(response).unwrap();
            }
        });

        (user_send_req, user_recv_response)
    }
}

#[derive(Default)]
struct PoolAnaliticRunner {
    running: AtomicUsize,
}

struct PoolRunnerRef {
    id: usize,
    running: usize,
}

struct PoolBalancer<const N: usize> {
    runners: [PoolAnaliticRunner; N],
}

impl<const N: usize> PoolBalancer<N> {
    fn get_by_id(&self, id: usize) -> &PoolAnaliticRunner {
        &self.runners[id]
    }
    fn new() -> Self {
        PoolBalancer {
            runners: [(); N].map(|_| PoolAnaliticRunner::default()),
        }
    }
    fn send(pb: &Arc<Self>) -> PoolRunnerRef {
        let mut min = PoolRunnerRef {
            id: 0,
            running: usize::MAX,
        };
        for id in 0..N {
            let running = pb
                .get_by_id(id)
                .running
                .load(std::sync::atomic::Ordering::SeqCst);
            if running == 0 {
                eprintln!("[ACQ] Found idle runner #{id} (0 -> 1)");
                pb.get_by_id(id)
                    .running
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return PoolRunnerRef { id, running };
            } else if running < min.running {
                min = PoolRunnerRef { id, running };
            }
        }
        let old = pb
            .get_by_id(min.id)
            .running
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        eprintln!("[ACQ] Best runner #{} ({} -> {})", min.id, old, old + 1);
        min
    }
    fn done(pb: &Arc<Self>, id: usize) {
        let old = pb
            .get_by_id(id)
            .running
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        eprintln!("[REL] runner #{} ({} -> {})", id, old, old - 1);
    }
}
