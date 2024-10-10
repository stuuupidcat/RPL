use rustc_index::Idx;

/// Conceptually, this is like a `Vec<Vec<RWState>>`. But the number of
/// RWU's can get very large, so it uses a more compact representation.
pub(super) struct RWStates<Local: Idx> {
    /// Total number of statements.
    statements: usize,
    /// Total number of locals.
    locals: usize,

    /// A compressed representation of `RWState`s.
    ///
    /// Each word represents 4 different `RWState`s packed together. Each packed
    /// RWState is stored in 2 bits: a read bit, and a write bit.
    ///
    /// The data for each statement is contiguous and starts at a word boundary,
    /// so there might be an unused space left.
    words: Vec<u8>,
    /// Number of words per each statement.
    statement_words: usize,
    _marker: std::marker::PhantomData<Local>,
}

impl<Local: Idx> RWStates<Local> {
    const RW_READ: u8 = 0b01;
    const RW_WRITE: u8 = 0b10;
    // const RW_MASK: u8 = 0b11;

    /// Size of packed RWU in bits.
    const RW_BITS: usize = 2;
    /// Size of a word in bits.
    const WORD_BITS: usize = std::mem::size_of::<u8>() * 8;
    /// Number of packed RWStates that fit into a single word.
    const WORD_RWU_COUNT: usize = Self::WORD_BITS / Self::RW_BITS;

    pub(super) fn new(statements: usize, locals: usize) -> Self {
        let statement_words = locals.div_ceil(Self::WORD_RWU_COUNT);
        Self {
            statements,
            locals,
            statement_words,
            words: vec![0u8; statement_words * statements],
            _marker: std::marker::PhantomData,
        }
    }

    fn word_and_shift(&self, statement: usize, local: Local) -> (usize, u32) {
        assert!(statement < self.statements);
        assert!(local.index() < self.locals);

        let var = local.index();
        let word = var / Self::WORD_RWU_COUNT;
        let shift = Self::RW_BITS * (var % Self::WORD_RWU_COUNT);
        (statement * self.statement_words + word, shift as u32)
    }

    pub(super) fn get_read(&self, statement: usize, local: Local) -> bool {
        let (word, shift) = self.word_and_shift(statement, local);
        (self.words[word] >> shift) & Self::RW_READ != 0
    }

    pub(super) fn get_write(&self, statement: usize, local: Local) -> bool {
        let (word, shift) = self.word_and_shift(statement, local);
        (self.words[word] >> shift) & Self::RW_WRITE != 0
    }

    pub(super) fn get_reads(&self, statement: usize) -> impl Iterator<Item = Local> + '_ {
        (0..self.locals)
            .map(Local::new)
            .filter(move |&local| self.get_read(statement, local))
    }

    pub(super) fn get_writes(&self, statement: usize) -> impl Iterator<Item = Local> + '_ {
        (0..self.locals)
            .map(Local::new)
            .filter(move |&local| self.get_write(statement, local))
    }

    pub(super) fn set_read(&mut self, statement: usize, local: Local) {
        let (word, shift) = self.word_and_shift(statement, local);
        let word = &mut self.words[word];
        *word |= Self::RW_READ << shift;
    }

    pub(super) fn set_write(&mut self, statement: usize, local: Local) {
        let (word, shift) = self.word_and_shift(statement, local);
        let word = &mut self.words[word];
        *word |= Self::RW_WRITE << shift;
    }

    pub(super) fn set_read_write(&mut self, statement: usize, local: Local) {
        let (word, shift) = self.word_and_shift(statement, local);
        let word = &mut self.words[word];
        *word |= (Self::RW_READ | Self::RW_WRITE) << shift;
    }
}
