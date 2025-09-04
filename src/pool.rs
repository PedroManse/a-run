use crate::runner::{ControlExecuteMessage, Runner, StopRunner};
use std::marker::PhantomData;
use std::ops::ControlFlow;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{Receiver, RecvError, SendError, Sender, TryRecvError};
use std::thread::JoinHandle;

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
    user_request_channel: Chan<ControlFlow<(), Req>>,
    user_response_channel: Chan<Ret<Req>>,
    pooled_request_channel: [PoolConDef<Req>; CCOUNT],
    pooled_response_channel: Chan<Pooled<Ret<Req>>>,
}

pub trait PoolCloserMarker {}
pub struct RecieverDropped;
pub struct RecieverReturned;
impl PoolCloserMarker for RecieverDropped {}
impl PoolCloserMarker for RecieverReturned {}

pub struct PoolConDef<Req>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pooled_chan: Chan<Pooled<Req>>,
}

#[derive(Debug)]
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

pub struct PoolApi<Req, const N: usize>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    send_req: Sender<ControlFlow<(), Req>>,
    recv_res: Receiver<Ret<Req>>,
    manager_thread: JoinHandle<PoolCloserDef<Req, N>>,
}

#[derive(Debug)]
pub struct PoolCloser<Req, const N: usize, R>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
    R: PoolCloserMarker,
{
    recv_pooled_response: Receiver<Pooled<Ret<Req>>>,
    runners: [PoolCon<Req>; N],
    balancer: PoolBalancer<N>,
    user_send_response: Sender<Ret<Req>>,
    _mark: PhantomData<R>,
}

impl<Req, const CCOUNT: usize> Pool<Req, CCOUNT>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
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
                        return PoolCloserDef {
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

impl<Req, const N: usize> PoolApi<Req, N>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    pub fn send(&self, req: Req) -> Result<(), SendError<ControlFlow<(), Req>>> {
        self.send_req.send(ControlFlow::Continue(req))
    }
    pub fn recv(&self) -> Result<Ret<Req>, RecvError> {
        self.recv_res.recv()
    }

    /// Stop execution of pool and take it's reciever for the remaining tasks
    pub fn stop(
        self,
    ) -> Result<
        (PoolCloser<Req, N, RecieverReturned>, Receiver<Ret<Req>>),
        SendError<ControlFlow<(), Req>>,
    > {
        self.send_req.send(ControlFlow::Break(()))?;
        let closer_def = self.manager_thread.join().unwrap();
        let closer = PoolCloser::<Req, N, RecieverReturned>::from(closer_def);
        Ok((closer, self.recv_res))
    }

    /// Stop execution of pool and drop the pool's response reciever
    pub fn stop_and_close(
        self,
    ) -> Result<PoolCloser<Req, N, RecieverDropped>, SendError<ControlFlow<(), Req>>> {
        self.send_req.send(ControlFlow::Break(()))?;
        let closer_def = self.manager_thread.join().unwrap();
        Ok(PoolCloser::<Req, N, RecieverDropped>::from(closer_def))
    }
}

pub struct PoolCloserDef<Req, const N: usize>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    recv_pooled_response: Receiver<Pooled<Ret<Req>>>,
    runners: [PoolCon<Req>; N],
    balancer: PoolBalancer<N>,
    user_send_response: Sender<Ret<Req>>,
}

impl<Req, const N: usize, R> From<PoolCloserDef<Req, N>> for PoolCloser<Req, N, R>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
    R: PoolCloserMarker,
{
    fn from(value: PoolCloserDef<Req, N>) -> Self {
        Self {
            recv_pooled_response: value.recv_pooled_response,
            runners: value.runners,
            balancer: value.balancer,
            user_send_response: value.user_send_response,
            _mark: PhantomData,
        }
    }
}

impl<Req, const N: usize, R> PoolCloser<Req, N, R>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
    R: PoolCloserMarker,
{
    fn kill<S>(self, closer: &S)
    where
        S: StopRunner<Req>,
    {
        for (runner_id, runner) in self.runners.into_iter().enumerate() {
            runner.send(Pooled::pack(runner_id, closer.get())).unwrap();
            runner._thread.join().unwrap();
        }
    }

    fn await_runners<F>(&self, mut f: F)
    where
        F: FnMut(Ret<Req>),
    {
        for _ in 0..self
            .balancer
            .total
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let pooled_response = self.recv_pooled_response.recv().unwrap();
            let (runner_id, response) = pooled_response.unpack();
            self.balancer.done(runner_id);
            f(response)
        }
    }

    /// Await every executor finish their tasks and capture their responses
    pub fn close_capture<S>(self, closer: &S) -> Vec<Ret<Req>>
    where
        S: StopRunner<Req>,
    {
        let mut late = Vec::with_capacity(
            self.balancer
                .total
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        self.await_runners(|response| {
            late.push(response);
        });
        self.kill(closer);
        late
    }
}

impl<Req, const N: usize> PoolCloser<Req, N, RecieverReturned>
where
    Req: ControlExecuteMessage + Send + Sync + 'static,
    <Req as ControlExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
{
    /// Await every executor finish their tasks and send their responses
    pub fn close_await<S>(self, closer: &S)
    where
        S: StopRunner<Req>,
    {
        self.await_runners(|response| {
            self.user_send_response.send(response).unwrap();
        });
        self.kill(closer);
    }
}

#[derive(Default, Debug)]
struct PoolAnaliticRunner {
    running: AtomicUsize,
}

struct PoolRunnerRef {
    id: usize,
    running: usize,
}

#[derive(Debug)]
struct PoolBalancer<const N: usize> {
    total: AtomicUsize,
    runners: [PoolAnaliticRunner; N],
}

impl<const N: usize> PoolBalancer<N> {
    fn get_by_id(&self, id: usize) -> &PoolAnaliticRunner {
        &self.runners[id]
    }
    fn new() -> Self {
        PoolBalancer {
            total: AtomicUsize::default(),
            runners: [(); N].map(|_| PoolAnaliticRunner::default()),
        }
    }
    #[must_use]
    fn send(&self) -> PoolRunnerRef {
        let mut min = PoolRunnerRef {
            id: 0,
            running: usize::MAX,
        };
        for id in 0..N {
            let running = self
                .get_by_id(id)
                .running
                .load(std::sync::atomic::Ordering::SeqCst);
            if running == 0 {
                //eprintln!("[ACQ] Found idle runner #{id} (0 -> 1)");
                self.get_by_id(id)
                    .running
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.total
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return PoolRunnerRef { id, running };
            } else if running < min.running {
                min = PoolRunnerRef { id, running };
            }
        }
        let _old = self
            .get_by_id(min.id)
            .running
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        //eprintln!("[ACQ] Best runner #{} ({} -> {})", min.id, _old, _old + 1);
        min
    }
    fn done(&self, id: usize) {
        let _old = self
            .get_by_id(id)
            .running
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        self.total
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        //eprintln!("[REL] runner #{} ({} -> {})", id, _old, _old - 1);
    }
}
