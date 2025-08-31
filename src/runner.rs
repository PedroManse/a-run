use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender};

pub struct Runner<Req>
where
    Req: ExecuteMessage,
    <Req as ExecuteMessage>::Res: std::fmt::Debug,
{
    incoming: Receiver<Req>,
    outgoing: Sender<<Req as ExecuteMessage>::Res>,
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

type Ret<T> = <T as ExecuteMessage>::Res;

impl<Req> Runner<Req>
where
    Req: ExecuteMessage + Send + Sync + 'static,
    <Req as ExecuteMessage>::Res: std::fmt::Debug + Send + 'static,
    //Ret: Send + Sync + std::fmt::Debug + 'static + From<<Req as ExecuteMessage>::Res>,
{
    pub fn new() -> (Self, Sender<Req>, Receiver<Ret<Req>>) {
        let (req_send, req_recv) = mpsc::channel();
        let (res_send, res_recv) = mpsc::channel();
        (
            Runner {
                incoming: req_recv,
                outgoing: res_send,
            },
            req_send,
            res_recv,
        )
    }
    pub fn run_thread(mut self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                self.execute_one().unwrap();
            }
        })
    }
    fn execute_one(&mut self) -> Result<(), RunnerError<Ret<Req>>> {
        let msg = self.incoming.recv().map_err(RunnerError::Recv)?;
        let res: Ret<Req> = msg.execute().into();
        self.outgoing.send(res).map_err(RunnerError::Send)?;
        Ok(())
    }
}
