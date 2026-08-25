use std::error::Error;

use crate::domain::ConversionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanOutput {
    pub conversion_id: ConversionId,
}

pub trait OutputCleaner {
    type Error: Error + Send + Sync + 'static;

    /// Cleans only disposable staging output for one conversion identity.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when staging cleanup fails.
    fn clean_staging(&self, id: &ConversionId) -> Result<(), Self::Error>;
}

/// Cleans disposable staging output for one conversion identity.
///
/// # Errors
///
/// Propagates the adapter error returned by [`OutputCleaner`].
pub fn handle<Cleaner>(command: CleanOutput, cleaner: &Cleaner) -> Result<(), Cleaner::Error>
where
    Cleaner: OutputCleaner,
{
    let CleanOutput { conversion_id } = command;
    cleaner.clean_staging(&conversion_id)
}
