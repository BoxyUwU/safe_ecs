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
    fn from_world(world: &World) -> Self::SelfCtor<'_>;
    fn get_access() -> Result<Access, ()>;
}

impl<'a, Q: QueryParam> SystemParam for Query<'a, Q> {
    type SelfCtor<'b> = Query<'b, Q>;

    fn from_world(world: &World) -> Self::SelfCtor<'_> {
        world.query::<Q>()
    }

    fn get_access() -> Result<Access, ()> {
        Q::get_access()
    }
}

macro_rules! system_param_tuple_impl {
    ($($T:ident)+) => {
        impl<$($T: SystemParam),+> SystemParam for ($($T,)+) {
            type SelfCtor<'b> = ($($T::SelfCtor<'b>,)+);

            fn from_world(world: &World) -> Self::SelfCtor<'_> {
                ($($T::from_world(world),)+)
            }

            fn get_access() -> Result<Access, ()> {
                Access::from_array([$($T::get_access()),+])
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
    fn run(&mut self, world: &World);
    fn get_access(&self) -> Result<Access, ()>;
}

struct FunctionSystem<In, Func>(Func, PhantomData<fn(In)>)
where
    Self: System;

pub trait ToSystem<In> {
    fn system(self) -> Box<dyn System>;
}

macro_rules! system_impl {
    ($($T:ident)+) => {
        impl<Func, $($T: SystemParam,)+> System for FunctionSystem<($($T,)+), Func>
        where
            for<'a> &'a mut Func: FnMut($($T,)+),
            for<'a> &'a mut Func: FnMut($($T::SelfCtor<'_>,)+), {
                fn run(&mut self, world: &World) {
                    let this = self;
                    (&mut &mut this.0)($($T::from_world(world),)+)
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
                Box::new(FunctionSystem(self, PhantomData))
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

#[test]
fn foo() {
    fn takes_sys<In, T: ToSystem<In>>(s: T, world: &World) {
        let mut sys = T::system(s);
        sys.run(world);
    }
    fn query(mut q: Query<&u64>) {
        for int in &mut q {
            dbg!(int);
        }
    }
    let mut world = World::new();
    world.spawn().insert(10_u64);
    takes_sys(query, &world);
    takes_sys(|_: Query<&u32>, _: Query<&u64>| todo!(), &world);
}
