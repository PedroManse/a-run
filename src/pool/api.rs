use super::*;

pub struct PoolApi<Req, const N: usize>
where
    Req: ControlExecuteMessage,
{
    pub(crate) send_req: Sender<ControlFlow<(), Req>>,
    pub(crate) recv_res: Receiver<Ret<Req>>,
    pub(crate) manager_thread: JoinHandle<PoolCloserDef<Req, N>>,
}

impl<Req, const N: usize> PoolApi<Req, N>
where
    Req: ControlExecuteMessage,
    Ret<Req>: std::fmt::Debug + Send + 'static,
{
    pub fn send(&self, req: Req) -> Result<(), SendError<ControlFlow<(), Req>>> {
        self.send_req.send(ControlFlow::Continue(req))
    }
    pub fn recv(&self) -> Result<Ret<Req>, RecvError> {
        self.recv_res.recv()
    }

    /// Stop execution of pool and take it's reciever for the remaining tasks
    pub fn stop(self) -> Result<PoolCloseRecvPair<Req, N>, SendError<ControlFlow<(), Req>>> {
        self.send_req.send(ControlFlow::Break(()))?;
        let closer_def = self.manager_thread.join().unwrap();
        let closer = PoolCloser::<Req, N, ReceiverReturned>::from(closer_def);
        Ok((closer, self.recv_res))
    }

    /// Stop execution of pool and drop the pool's response reciever
    pub fn stop_and_close(
        self,
    ) -> Result<PoolCloser<Req, N, ReceiverDropped>, SendError<ControlFlow<(), Req>>> {
        self.send_req.send(ControlFlow::Break(()))?;
        let closer_def = self.manager_thread.join().unwrap();
        Ok(PoolCloser::<Req, N, ReceiverDropped>::from(closer_def))
    }
}

