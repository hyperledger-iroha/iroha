#[derive(Clone)]
pub struct BranchPredictor {
    table: Vec<u8>,
}

impl BranchPredictor {
    pub fn new(size: usize) -> Self {
        // Initialize counters to weakly not taken (1)
        let size = size.next_power_of_two();
        Self {
            table: vec![1; size],
        }
    }

    fn index(&self, pc: u64) -> usize {
        (pc as usize / 2) & (self.table.len() - 1)
    }

    pub fn predict(&self, pc: u64) -> bool {
        self.table[self.index(pc)] >= 2
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }

    pub fn update(&mut self, pc: u64, taken: bool) {
        let idx = self.index(pc);
        let counter = &mut self.table[idx];
        if taken {
            if *counter < 3 {
                *counter += 1;
            }
        } else if *counter > 0 {
            *counter -= 1;
        }
    }
}
