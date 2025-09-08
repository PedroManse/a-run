use super::*;

pub type PoolCloseRecvPair<Req, const N: usize> =
    (PoolCloser<Req, N, ReceiverReturned>, Receiver<Ret<Req>>);

pub trait PoolCloserMarker {}
pub struct ReceiverDropped;
pub struct ReceiverReturned;
impl PoolCloserMarker for ReceiverDropped {}
impl PoolCloserMarker for ReceiverReturned {}

#[derive(Debug)]
#[must_use]
pub struct PoolCloser<Req, const N: usize, R>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
    R: PoolCloserMarker,
{
    recv_pooled_response: Receiver<Pooled<Ret<Req>>>,
    runners: [PoolCon<Req>; N],
    balancer: PoolBalancer<N>,
    user_send_response: Sender<Ret<Req>>,
    _mark: PhantomData<R>,
}

pub struct PoolCloserDef<Req, const N: usize>
where
    Req: ControlExecuteMessage,
{
    pub recv_pooled_response: Receiver<Pooled<Ret<Req>>>,
    pub runners: [PoolCon<Req>; N],
    pub balancer: PoolBalancer<N>,
    pub user_send_response: Sender<Ret<Req>>,
}

impl<Req, const N: usize, R> From<PoolCloserDef<Req, N>> for PoolCloser<Req, N, R>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
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
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
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
    #[must_use]
    fn _close_capture<S>(self, closer: &S) -> Vec<Ret<Req>>
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

impl<Req, const N: usize> PoolCloser<Req, N, ReceiverDropped>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    /// Await every executor finish their tasks and capture their responses
    #[must_use]
    pub fn close_capture<S>(self, closer: &S) -> Vec<Ret<Req>>
    where
        S: StopRunner<Req>,
    {
        self._close_capture(closer)
    }
}

impl<Req, const N: usize> PoolCloser<Req, N, ReceiverReturned>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
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
    /// Await every executor finish their tasks and capture their responses
    #[must_use]
    pub fn close_capture<S>(self, closer: &S, _: Receiver<Ret<Req>>) -> Vec<Ret<Req>>
    where
        S: StopRunner<Req>,
    {
        self._close_capture(closer)
    }
}

