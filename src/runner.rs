use std::ops::ControlFlow;
pub trait ControlExecuteMessage: Send + Sync + 'static {
    type Res;
    fn execute(self) -> ControlFlow<(), Self::Res>;
}

/// Makes a request that a runner's [`ControlExecuteMessage`] can identify and return a [`ControlFlow::Break`]
pub trait StopRunner<Req> {
    fn get(&self) -> Req;
}

pub trait RunnerApi {
    type Req: ControlExecuteMessage;
    type SendAck;
    type CloseResult;
    fn send(&self, req: Self::Req) -> Self::SendAck;
    fn close(self, s: impl StopRunner<Self::Req>) -> Self::CloseResult;
    fn new() -> Self;
}

pub type Req<T> = <T as RunnerApi>::Req;
pub type SendAck<T> = <T as RunnerApi>::Req;
pub type CloseRes<T> = <T as RunnerApi>::Req;
pub type Ret<T> = <T as ControlExecuteMessage>::Res;
