use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ElodonError{
    #[error("There were no results found in {search} where {id}. Either those {search} don't exist or elodon hasn't unlocked them")]
    NoResults {
        search: String,
        id: String
    },
    #[error("Level_ids are from 1 - 5. Level id given was {0} which doesn't correspond to a level")]
    WrongLevelId(u32),
    #[error("could not extract level id from {0}")]
    ParseError(String),
    #[error("Genre_ids are from 1 - 9. Genre id given was {0} which doesn't correspond to a level")]
    WrongGenreId(u32),
    #[error(transparent)]
    List(#[from] ElodonErrorList),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Command that required server was not run in server")]
    NoGuild,
    #[error("Irrecoverable error (likely bad state or manual shutdown). More info: {0}")]
    Shutdown(#[from] std::io::Error),
}

impl From<ElodonError> for String {
    fn from(value: ElodonError) -> Self {
        format!("{}", value) //NOT AN ERROR. rustrover just dumb
    }
}

#[derive(Error, Debug)]
pub struct ElodonErrorList(Vec<ElodonError>);

impl Display for ElodonErrorList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ \n")?;
        for error in &self.0 {
            write!(f, "{error}\n")?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

impl ElodonErrorList{
    pub fn new() -> ElodonErrorList {
        ElodonErrorList(vec![])
    }
}

impl Deref for ElodonErrorList {
    type Target = Vec<ElodonError>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ElodonErrorList{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}