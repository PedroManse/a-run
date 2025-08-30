use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
pub mod runner;

#[derive(Debug)]
pub struct AFile(std::fs::File);

#[derive(Debug)]
pub enum ActionRequest {
    Open(PathBuf, OpenOptions),
    Read(AFile),
    WriteAll(AFile, Vec<u8>),
    Close(AFile),
    StopRunner,
}

#[derive(Debug)]
pub enum ActionResult {
    Open(AFile),
    Read(AFile, Vec<u8>),
    WriteAll(AFile),
    Close,
}

impl ActionRequest {
    pub fn execute(self) -> Result<ActionResult, std::io::Error> {
        match self {
            ActionRequest::Open(path, opt) => opt.open(path).map(AFile).map(ActionResult::Open),
            ActionRequest::Read(mut file) => {
                let mut buf = vec![];
                file.0.read_to_end(&mut buf)?;
                Ok(ActionResult::Read(file, buf))
            }
            ActionRequest::WriteAll(mut file, buf) => {
                file.0.write_all(&buf)?;
                Ok(ActionResult::WriteAll(file))
            }
            ActionRequest::Close(_) => Ok(ActionResult::Close),
            ActionRequest::StopRunner => {
                panic!("Stop Runner Action")
            }
        }
    }
}
