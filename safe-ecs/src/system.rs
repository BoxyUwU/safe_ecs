use std::{any::TypeId, collections::HashSet, marker::PhantomData};

use crate::{query::QueryParam, Query, World};

pub struct Access {
    read: HashSet<TypeId>,
    write: HashSet<TypeId>,
}

impl Access {
    pub fn new() -> Self {
        Self {
            read: HashSet::new(),
            write: HashSet::new(),
        }
    }

    pub fn insert_write(mut self, id: TypeId) -> Result<Self, ()> {
        if self.write.contains(&id) || self.read.contains(&id) {
            return Err(());
        }
        self.write.insert(id);
        Ok(self)
    }

    pub fn insert_read(mut self, id: TypeId) -> Result<Self, ()> {
        if self.write.contains(&id) {
            return Err(());
        }
        self.read.insert(id);
        Ok(self)
    }

    pub fn join_with(mut self, other: Result<Access, ()>) -> Result<Self, ()> {
        let other = other?;
        self.read.extend(other.read.iter().copied());
        if self.write.intersection(&other.write).next().is_some() {
            return Err(());
        }
        self.write.extend(other.write.iter().copied());
        if self.read.intersection(&self.write).next().is_some() {
            return Err(());
        }
        Ok(self)
    }

    pub fn from_array<const N: usize>(accesses: [Result<Access, ()>; N]) -> Result<Self, ()> {
        let mut output = Access::new();
        for access in accesses.into_iter() {
            output = output.join_with(access)?;
        }
        Ok(output)
    }
}

pub trait SystemParam {
    type SelfCtor<'b>;
    type SystemParamState;
    fn from_world<'a>(
        world: &'a World,
        state: &'a mut Self::SystemParamState,
    ) -> Self::SelfCtor<'a>;
    fn get_access() -> Result<Access, ()>;
    fn new_state() -> Self::SystemParamState;
    fn system_finish_event(state: &mut Self::SystemParamState, world: &mut World);
}

impl<'a, Q: QueryParam> SystemParam for Query<'a, Q> {
    type SelfCtor<'b> = Query<'b, Q>;
    type SystemParamState = ();

    fn from_world<'b>(world: &'b World, _: &'b mut Self::SystemParamState) -> Self::SelfCtor<'b> {
        world.query::<Q>()
    }

    fn get_access() -> Result<Access, ()> {
        Q::get_access()
    }

    fn new_state() -> Self::SystemParamState {}

    fn system_finish_event(_: &mut Self::SystemParamState, _: &mut World) {}
}

impl<'a> SystemParam for &'a World {
    type SelfCtor<'b> = &'b World;
    type SystemParamState = ();

    fn from_world<'b>(world: &'b World, _: &'b mut Self::SystemParamState) -> Self::SelfCtor<'b> {
        world
    }

    fn get_access() -> Result<Access, ()> {
        Ok(Access::new())
    }

    fn new_state() -> Self::SystemParamState {}

    fn system_finish_event(_: &mut Self::SystemParamState, _: &mut World) {}
}

use crate::{CommandBuffer, Commands};
impl<'a> SystemParam for Commands<'a> {
    type SelfCtor<'b> = Commands<'b>;
    type SystemParamState = CommandBuffer;

    fn from_world<'b>(
        world: &'b World,
        state: &'b mut Self::SystemParamState,
    ) -> Self::SelfCtor<'b> {
        Commands(state, world)
    }

    fn get_access() -> Result<Access, ()> {
        Ok(Access::new())
    }

    fn new_state() -> Self::SystemParamState {
        CommandBuffer::new()
    }

    fn system_finish_event(state: &mut Self::SystemParamState, world: &mut World) {
        state.apply(world);
    }
}

macro_rules! system_param_tuple_impl {
    ($($T:ident)+) => {
        impl<$($T: SystemParam),+> SystemParam for ($($T,)+) {
            type SelfCtor<'b> = ($($T::SelfCtor<'b>,)+);
            type SystemParamState = ($($T::SystemParamState,)+);

            #[allow(non_snake_case)]
            fn from_world<'a>(world: &'a World, state: &'a mut Self::SystemParamState) -> Self::SelfCtor<'a> {
                let ($($T,)+) = state;
                ($($T::from_world(world, $T),)+)
            }

            fn get_access() -> Result<Access, ()> {
                Access::from_array([$($T::get_access()),+])
            }

            fn new_state() -> Self::SystemParamState {
                ($($T::new_state(),)+)
            }

            #[allow(non_snake_case)]
            fn system_finish_event(state: &mut Self::SystemParamState, world: &mut World) {
                let ($($T,)+) = state;
                $($T::system_finish_event($T, world);)+
            }
        }
    };
}

system_param_tuple_impl!(A B C D E F G H);
system_param_tuple_impl!(A B C D E F G);
system_param_tuple_impl!(A B C D E F);
system_param_tuple_impl!(A B C D E);
system_param_tuple_impl!(A B C D);
system_param_tuple_impl!(A B C);
system_param_tuple_impl!(A B);
system_param_tuple_impl!(A);

pub trait System {
    fn run(&mut self, world: &mut World);
    fn get_access(&self) -> Result<Access, ()>;
}

struct FunctionSystem<State, In, Func>(State, Func, PhantomData<fn(In)>)
where
    Self: System;

pub trait ToSystem<In> {
    fn system(self) -> Box<dyn System>;
}

macro_rules! system_impl {
    ($($T:ident)+) => {
        impl<Func, $($T: SystemParam,)+> System for FunctionSystem<($($T::SystemParamState,)+), ($($T,)+), Func>
        where
            for<'a> &'a mut Func: FnMut($($T,)+),
            for<'a> &'a mut Func: FnMut($($T::SelfCtor<'_>,)+), {
                #[allow(non_snake_case)]
                fn run(&mut self, world: &mut World) {
                    let this = self;
                    let ($($T,)+) = &mut this.0;
                    (&mut &mut this.1)($($T::from_world(world, $T),)+);
                    // FIXME move this to a separate fn so this one doesnt need `&mut World`
                    $($T::system_finish_event($T, world);)+
                }

                fn get_access(&self) -> Result<Access, ()> {
                    Access::from_array([$($T::get_access()),+])
                }
            }

        impl<Func: 'static, $($T: SystemParam + 'static,)+> ToSystem<($($T,)+)> for Func
        where
            for<'a> &'a mut Func: FnMut($($T,)+),
            for<'a> &'a mut Func: FnMut($($T::SelfCtor<'_>,)+), {
            fn system(self) -> Box<dyn System> {
                Box::new(FunctionSystem::<($($T::SystemParamState,)+), _, _>(($($T::new_state(),)+), self, PhantomData))
            }
        }
    };
}

system_impl!(A B C D E F G H);
system_impl!(A B C D E F G);
system_impl!(A B C D E F);
system_impl!(A B C D E);
system_impl!(A B C D);
system_impl!(A B C);
system_impl!(A B);
system_impl!(A);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_into_sys() {
        fn takes_sys<In, T: ToSystem<In>>(s: T, world: &mut World) {
            let mut sys = T::system(s);
            sys.run(world);
        }
        fn query(mut q: Query<&u64>, _c: Commands) {
            for _ in &mut q {}
        }
        let mut world = World::new();
        world.spawn().insert(10_u64);
        takes_sys(query, &mut world);
    }

    #[test]
    fn params() {
        // FIXME: panics
        fn query(_: Query<&u64>, _: Commands, _: &World) {}
        let mut world = World::new();
        world.access_scope(query);
    }
}
