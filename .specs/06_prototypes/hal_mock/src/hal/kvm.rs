pub struct KvmMock {
    fd: i32,
    vm_fd: i32,
    vcpu_fd: i32,
}

impl KvmMock {
    pub fn new() -> Self {
        Self {
            fd: -1,
            vm_fd: -1,
            vcpu_fd: -1,
        }
    }

    pub fn create_vm(&mut self) -> Result<(), &'static str> {
        self.vm_fd = 1;
        Ok(())
    }

    pub fn create_vcpu(&mut self, id: u32) -> Result<(), &'static str> {
        self.vcpu_fd = id as i32;
        Ok(())
    }

    pub fn set_memory(&self, _gpa: u64, _size: usize) -> Result<(), &'static str> {
        Ok(())
    }

    pub fn run_vcpu(&self) -> Result<VcpuExit, &'static str> {
        Ok(VcpuExit::Hlt)
    }
}

#[derive(Debug, Clone)]
pub enum VcpuExit {
    Hlt,
    IoOut(u16, Vec<u8>),
    IoIn(u16, usize),
    Exception(u32),
}

impl Default for KvmMock {
    fn default() -> Self {
        Self::new()
    }
}
