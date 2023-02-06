pub trait Process {
    fn process(&self) -> Result<(), String>;
}