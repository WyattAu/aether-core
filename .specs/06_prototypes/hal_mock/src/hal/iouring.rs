pub struct IoUringMock {
    sq_entries: u32,
    cq_entries: u32,
    pending: Vec<Sqe>,
}

#[derive(Debug, Clone)]
pub struct Sqe {
    pub opcode: u8,
    pub fd: i32,
    pub addr: u64,
    pub len: u32,
}

#[derive(Debug, Clone)]
pub struct Cqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

impl IoUringMock {
    pub fn new(entries: u32) -> Self {
        Self {
            sq_entries: entries,
            cq_entries: entries,
            pending: Vec::with_capacity(entries as usize),
        }
    }

    pub fn prep_read(&mut self, fd: i32, buf: &mut [u8], offset: u64) -> u64 {
        let sqe = Sqe {
            opcode: 0,
            fd,
            addr: buf.as_ptr() as u64,
            len: buf.len() as u32,
        };
        let id = self.pending.len() as u64;
        self.pending.push(sqe);
        id
    }

    pub fn prep_write(&mut self, fd: i32, buf: &[u8], offset: u64) -> u64 {
        let sqe = Sqe {
            opcode: 1,
            fd,
            addr: buf.as_ptr() as u64,
            len: buf.len() as u32,
        };
        let id = self.pending.len() as u64;
        self.pending.push(sqe);
        id
    }

    pub fn submit(&mut self) -> Result<u32, &'static str> {
        let count = self.pending.len() as u32;
        Ok(count)
    }

    pub fn wait_cqe(&mut self) -> Option<Cqe> {
        self.pending.pop().map(|sqe| Cqe {
            user_data: self.pending.len() as u64,
            res: sqe.len as i32,
            flags: 0,
        })
    }

    pub fn cq_ready(&self) -> u32 {
        0
    }
}
