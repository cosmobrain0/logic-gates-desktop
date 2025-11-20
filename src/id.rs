#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(usize);

#[derive(Debug, Clone)]
pub struct IdGenerator {
    inner: usize,
}
impl IdGenerator {
    pub fn new() -> Self {
        Self { inner: 0 }
    }

    pub fn generate(&mut self) -> Id {
        self.inner += 1;
        Id(self.inner - 1)
    }
}
