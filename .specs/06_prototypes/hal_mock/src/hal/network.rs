use std::collections::VecDeque;

pub struct NetworkMock {
    tx_queue: VecDeque<Vec<u8>>,
    rx_queue: VecDeque<Vec<u8>>,
    mtu: usize,
}

impl NetworkMock {
    pub fn new(mtu: usize) -> Self {
        Self {
            tx_queue: VecDeque::new(),
            rx_queue: VecDeque::new(),
            mtu,
        }
    }

    pub fn send(&mut self, packet: Vec<u8>) -> Result<(), NetworkError> {
        if packet.len() > self.mtu {
            return Err(NetworkError::PacketTooLarge);
        }
        self.tx_queue.push_back(packet);
        Ok(())
    }

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx_queue.pop_front()
    }

    pub fn inject_rx(&mut self, packet: Vec<u8>) {
        self.rx_queue.push_back(packet);
    }

    pub fn drain_tx(&mut self) -> Vec<Vec<u8>> {
        self.tx_queue.drain(..).collect()
    }

    pub fn tx_pending(&self) -> usize {
        self.tx_queue.len()
    }

    pub fn rx_pending(&self) -> usize {
        self.rx_queue.len()
    }
}

#[derive(Debug, Clone)]
pub enum NetworkError {
    PacketTooLarge,
    QueueFull,
    Disconnected,
}

pub struct TapMock {
    name: String,
    network: NetworkMock,
}

impl TapMock {
    pub fn new(name: &str, mtu: usize) -> Self {
        Self {
            name: name.to_string(),
            network: NetworkMock::new(mtu),
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, NetworkError> {
        self.network.send(data.to_vec())?;
        Ok(data.len())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetworkError> {
        match self.network.recv() {
            Some(packet) => {
                let len = packet.len().min(buf.len());
                buf[..len].copy_from_slice(&packet[..len]);
                Ok(len)
            }
            None => Ok(0),
        }
    }
}
