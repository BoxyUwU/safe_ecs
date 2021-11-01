use crate::{Commands, CommandsWithEntity, World};

pub trait Scope {
    fn scope(&self, f: impl FnOnce(&Self)) -> &Self {
        f(self);
        self
    }

    fn scope_mut(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        f(self);
        self
    }
}

impl Scope for Commands<'_> {}
impl Scope for CommandsWithEntity<'_, '_> {}
impl Scope for World {}
