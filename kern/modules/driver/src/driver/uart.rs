pub trait Uart: Driver {
    fn init(&self) -> Result<()>;
    fn read(&self) -> Result<u8>;
    fn write(&self, data: u8) -> Result<()>;
    fn write_str(&self, data: &str) -> Result<()>;
}
