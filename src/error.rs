use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use thiserror::Error;
use crate::structs::User;

#[derive(Error, Debug)]
pub enum ElodonError{
    #[error("There was an issue with finding the entry for {search} = {id}. More info: {sql_error}")]
    WrongId{
        sql_error: sqlx::Error,
        search: String,
        id: i64
    },
    #[error(transparent)]
    List(#[from] ElodonErrorList),
    #[error("Failed to connect to database. More info: {0}")]
    DatabaseError(#[from] sqlx::Error)
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