//! Hardware Abstraction Layer (HAL)
//!
//! Provides capability-gated access to GPIO, I2C, and SPI hardware
//! interfaces for edge/IoT deployments. All operations are deny-by-default
//! and require explicit capability grants.
//!
//! # Capability Model
//!
//! Hardware access is gated by [`HardwareCapability`]:
//!
//! - `Gpio` — General-purpose I/O pin access
//! - `I2c` — I2C bus read/write
//! - `Spi` — SPI bus transfer
//!
//! # Usage
//!
//! ```ignore
//! use aether_core::wasi::hardware::{HardwareCapability, HardwareAbstraction, PinMode, PullMode, GpioPin};
//! use aether_core::capability::CapabilitySet;
//!
//! let caps = CapabilitySet::empty();
//! // GPIO access denied by default
//! ```

#![allow(missing_docs)]

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinMode {
    Input,
    Output,
    Analog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullMode {
    None,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioPin {
    pub pin_number: u8,
    pub mode: PinMode,
    pub pull: PullMode,
}

impl GpioPin {
    pub fn new(pin_number: u8, mode: PinMode, pull: PullMode) -> Self {
        Self {
            pin_number,
            mode,
            pull,
        }
    }

    pub fn input(pin_number: u8) -> Self {
        Self {
            pin_number,
            mode: PinMode::Input,
            pull: PullMode::None,
        }
    }

    pub fn output(pin_number: u8) -> Self {
        Self {
            pin_number,
            mode: PinMode::Output,
            pull: PullMode::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct I2cDevice {
    pub address: u8,
    pub bus: u8,
}

impl I2cDevice {
    pub fn new(address: u8, bus: u8) -> Self {
        Self { address, bus }
    }

    pub fn read(&self, _capabilities: &CapabilitySet) -> Result<Vec<u8>> {
        if !has_hardware_capability(_capabilities, HardwareCapability::I2c) {
            return Err(Error::capability_denied(
                "hardware:i2c",
                format!(
                    "I2C read on bus {} address 0x{:02X}",
                    self.bus, self.address
                ),
            ));
        }
        Ok(Vec::new())
    }

    pub fn write(&self, _data: &[u8], _capabilities: &CapabilitySet) -> Result<()> {
        if !has_hardware_capability(_capabilities, HardwareCapability::I2c) {
            return Err(Error::capability_denied(
                "hardware:i2c",
                format!(
                    "I2C write on bus {} address 0x{:02X}",
                    self.bus, self.address
                ),
            ));
        }
        Ok(())
    }

    pub fn write_read(
        &self,
        _write_data: &[u8],
        _read_len: usize,
        _capabilities: &CapabilitySet,
    ) -> Result<Vec<u8>> {
        if !has_hardware_capability(_capabilities, HardwareCapability::I2c) {
            return Err(Error::capability_denied(
                "hardware:i2c",
                format!(
                    "I2C write_read on bus {} address 0x{:02X}",
                    self.bus, self.address
                ),
            ));
        }
        Ok(vec![0u8; _read_len])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiDevice {
    pub chip_select: u8,
    pub frequency: u32,
}

impl SpiDevice {
    pub fn new(chip_select: u8, frequency: u32) -> Self {
        Self {
            chip_select,
            frequency,
        }
    }

    pub fn transfer(&self, _data: &[u8], _capabilities: &CapabilitySet) -> Result<Vec<u8>> {
        if !has_hardware_capability(_capabilities, HardwareCapability::Spi) {
            return Err(Error::capability_denied(
                "hardware:spi",
                format!("SPI transfer on CS {}", self.chip_select),
            ));
        }
        Ok(vec![0u8; _data.len()])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareCapability {
    Gpio,
    I2c,
    Spi,
}

fn has_hardware_capability(_caps: &CapabilitySet, _hw_cap: HardwareCapability) -> bool {
    match _hw_cap {
        HardwareCapability::Gpio => false,
        HardwareCapability::I2c => false,
        HardwareCapability::Spi => false,
    }
}

pub trait HardwareAbstraction {
    fn gpio_read(&self, pin: &GpioPin, capabilities: &CapabilitySet) -> Result<bool>;
    fn gpio_write(
        &mut self,
        pin: &GpioPin,
        value: bool,
        capabilities: &CapabilitySet,
    ) -> Result<()>;
    fn i2c_read(&self, device: &I2cDevice, capabilities: &CapabilitySet) -> Result<Vec<u8>>;
    fn i2c_write(
        &self,
        device: &I2cDevice,
        data: &[u8],
        capabilities: &CapabilitySet,
    ) -> Result<()>;
    fn spi_transfer(
        &self,
        device: &SpiDevice,
        data: &[u8],
        capabilities: &CapabilitySet,
    ) -> Result<Vec<u8>>;
    fn available_capabilities(&self) -> HashSet<HardwareCapability>;
}

pub struct MockHardware {
    gpio_states: std::collections::HashMap<u8, bool>,
    capabilities: HashSet<HardwareCapability>,
}

impl MockHardware {
    pub fn new(capabilities: HashSet<HardwareCapability>) -> Self {
        Self {
            gpio_states: std::collections::HashMap::new(),
            capabilities,
        }
    }
}

impl HardwareAbstraction for MockHardware {
    fn gpio_read(&self, pin: &GpioPin, capabilities: &CapabilitySet) -> Result<bool> {
        if !self.capabilities.contains(&HardwareCapability::Gpio) {
            return Err(Error::capability_denied(
                "hardware:gpio",
                format!("GPIO read on pin {}", pin.pin_number),
            ));
        }
        if !has_hardware_capability(capabilities, HardwareCapability::Gpio) {
            return Err(Error::capability_denied(
                "hardware:gpio",
                format!("GPIO read on pin {} denied by actor caps", pin.pin_number),
            ));
        }
        Ok(self
            .gpio_states
            .get(&pin.pin_number)
            .copied()
            .unwrap_or(false))
    }

    fn gpio_write(
        &mut self,
        pin: &GpioPin,
        value: bool,
        capabilities: &CapabilitySet,
    ) -> Result<()> {
        if !self.capabilities.contains(&HardwareCapability::Gpio) {
            return Err(Error::capability_denied(
                "hardware:gpio",
                format!("GPIO write on pin {}", pin.pin_number),
            ));
        }
        if !has_hardware_capability(capabilities, HardwareCapability::Gpio) {
            return Err(Error::capability_denied(
                "hardware:gpio",
                format!("GPIO write on pin {} denied by actor caps", pin.pin_number),
            ));
        }
        self.gpio_states.insert(pin.pin_number, value);
        Ok(())
    }

    fn i2c_read(&self, device: &I2cDevice, capabilities: &CapabilitySet) -> Result<Vec<u8>> {
        if !self.capabilities.contains(&HardwareCapability::I2c) {
            return Err(Error::capability_denied(
                "hardware:i2c",
                format!(
                    "I2C read on bus {} address 0x{:02X}",
                    device.bus, device.address
                ),
            ));
        }
        device.read(capabilities)
    }

    fn i2c_write(
        &self,
        device: &I2cDevice,
        data: &[u8],
        capabilities: &CapabilitySet,
    ) -> Result<()> {
        if !self.capabilities.contains(&HardwareCapability::I2c) {
            return Err(Error::capability_denied(
                "hardware:i2c",
                format!(
                    "I2C write on bus {} address 0x{:02X}",
                    device.bus, device.address
                ),
            ));
        }
        device.write(data, capabilities)
    }

    fn spi_transfer(
        &self,
        device: &SpiDevice,
        data: &[u8],
        capabilities: &CapabilitySet,
    ) -> Result<Vec<u8>> {
        if !self.capabilities.contains(&HardwareCapability::Spi) {
            return Err(Error::capability_denied(
                "hardware:spi",
                format!("SPI transfer on CS {}", device.chip_select),
            ));
        }
        device.transfer(data, capabilities)
    }

    fn available_capabilities(&self) -> HashSet<HardwareCapability> {
        self.capabilities.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_caps() -> CapabilitySet {
        CapabilitySet::empty()
    }

    #[test]
    fn test_gpio_pin_constructors() {
        let input = GpioPin::input(5);
        assert_eq!(input.pin_number, 5);
        assert_eq!(input.mode, PinMode::Input);

        let output = GpioPin::output(12);
        assert_eq!(output.pin_number, 12);
        assert_eq!(output.mode, PinMode::Output);

        let custom = GpioPin::new(7, PinMode::Analog, PullMode::Up);
        assert_eq!(custom.pin_number, 7);
        assert_eq!(custom.mode, PinMode::Analog);
        assert_eq!(custom.pull, PullMode::Up);
    }

    #[test]
    fn test_i2c_device_construction() {
        let dev = I2cDevice::new(0x3C, 1);
        assert_eq!(dev.address, 0x3C);
        assert_eq!(dev.bus, 1);
    }

    #[test]
    fn test_spi_device_construction() {
        let dev = SpiDevice::new(0, 1_000_000);
        assert_eq!(dev.chip_select, 0);
        assert_eq!(dev.frequency, 1_000_000);
    }

    #[test]
    fn test_i2c_read_denied_without_capability() {
        let dev = I2cDevice::new(0x3C, 1);
        let result = dev.read(&empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_i2c_write_denied_without_capability() {
        let dev = I2cDevice::new(0x3C, 1);
        let result = dev.write(&[0x00, 0x01], &empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_i2c_write_read_denied_without_capability() {
        let dev = I2cDevice::new(0x3C, 1);
        let result = dev.write_read(&[0x00], 4, &empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_spi_transfer_denied_without_capability() {
        let dev = SpiDevice::new(0, 1_000_000);
        let result = dev.transfer(&[0xFF, 0x00], &empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_hardware_gpio_read_write() {
        let mut hw = MockHardware::new([HardwareCapability::Gpio].into_iter().collect());
        let pin = GpioPin::output(4);

        // Deny-by-default: actor has no hardware caps
        let read_result = hw.gpio_read(&pin, &empty_caps());
        assert!(read_result.is_err());

        let write_result = hw.gpio_write(&pin, true, &empty_caps());
        assert!(write_result.is_err());
    }

    #[test]
    fn test_mock_hardware_no_i2c_capability() {
        let hw = MockHardware::new(HashSet::new());
        let dev = I2cDevice::new(0x50, 0);

        let result = hw.i2c_read(&dev, &empty_caps());
        assert!(result.is_err());

        let result = hw.i2c_write(&dev, &[0x00], &empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_hardware_no_spi_capability() {
        let hw = MockHardware::new(HashSet::new());
        let dev = SpiDevice::new(0, 500_000);

        let result = hw.spi_transfer(&dev, &[0xAA], &empty_caps());
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_hardware_available_capabilities() {
        let hw = MockHardware::new(
            [HardwareCapability::Gpio, HardwareCapability::I2c]
                .into_iter()
                .collect(),
        );
        let caps = hw.available_capabilities();
        assert!(caps.contains(&HardwareCapability::Gpio));
        assert!(caps.contains(&HardwareCapability::I2c));
        assert!(!caps.contains(&HardwareCapability::Spi));
    }

    #[test]
    fn test_hardware_capability_serde_roundtrip() {
        let cap = HardwareCapability::Gpio;
        let json = serde_json::to_string(&cap).expect("serialize");
        let deserialized: HardwareCapability = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cap, deserialized);
    }

    #[test]
    fn test_gpio_pin_serde_roundtrip() {
        let pin = GpioPin::new(17, PinMode::Output, PullMode::Up);
        let json = serde_json::to_string(&pin).expect("serialize");
        let deserialized: GpioPin = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(pin, deserialized);
    }

    #[test]
    fn test_i2c_device_serde_roundtrip() {
        let dev = I2cDevice::new(0x68, 1);
        let json = serde_json::to_string(&dev).expect("serialize");
        let deserialized: I2cDevice = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dev, deserialized);
    }
}
