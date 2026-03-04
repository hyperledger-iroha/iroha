use std::error::Error;

#[derive(Debug, Default)]
pub struct Instructions;

#[derive(Debug, Default)]
pub struct Git2Builder;

impl Git2Builder {
    pub fn default() -> Self {
        Self
    }

    pub fn sha(self, _include: bool) -> Self {
        self
    }

    pub fn build(self) -> Result<Instructions, Box<dyn Error + Send + Sync>> {
        Ok(Instructions::default())
    }
}

#[derive(Debug, Default)]
pub struct CargoBuilder;

impl CargoBuilder {
    pub fn default() -> Self {
        Self
    }

    pub fn target_triple(self, _include: bool) -> Self {
        self
    }

    pub fn features(self, _include: bool) -> Self {
        self
    }

    pub fn build(self) -> Result<Instructions, Box<dyn Error + Send + Sync>> {
        Ok(Instructions::default())
    }
}

#[derive(Debug, Default)]
pub struct Emitter;

impl Emitter {
    pub fn default() -> Self {
        Self
    }

    pub fn add_instructions(
        self,
        _instructions: &Instructions,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(self)
    }

    pub fn emit(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
