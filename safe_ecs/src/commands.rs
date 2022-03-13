use std::marker::PhantomData;

use crate::{Component, Entity, StaticColumns, World};

pub enum Insert<T> {
    Data(Entity, T),
    Swap,
}

pub enum Remove {
    Data(Entity),
    Swap,
}

pub struct AddRemover<'world, T: Component> {
    world: &'world World,
    inserts: Vec<Insert<T>>,
    removes: Vec<Remove>,
}

pub struct EntityBoundAddRemover<'world, 'a, T: Component> {
    entity: Entity,
    add_remover: &'a mut AddRemover<'world, T>,
}

pub trait Commands {
    #[doc(hidden)]
    type Inserts;
    #[doc(hidden)]
    type Removes;
    type StorageBorrow<'a>;
    #[doc(hidden)]
    type AddRemover<'b>;
    #[doc(hidden)]
    type BorrowedAddRemover<'a, 'b: 'a>;
    type Passed<'a, 'b: 'a>;

    #[doc(hidden)]
    fn make_add_remover(world: &World) -> Self::AddRemover<'_>;
    #[doc(hidden)]
    fn make_passed<'a, 'b: 'a>(
        storage: Self::StorageBorrow<'a>,
        add_remover: &'a mut Self::AddRemover<'b>,
    ) -> Self::Passed<'a, 'b>;
    #[doc(hidden)]
    fn flush(
        inserts: Self::Inserts,
        removes: Self::Removes,
        storages: Self::StorageBorrow<'_>,
        world: &mut World,
    );
    #[doc(hidden)]
    fn reborrow_storage<'a: 'b, 'b>(
        storage: &'b mut Self::StorageBorrow<'a>,
    ) -> Self::StorageBorrow<'b>;
    #[doc(hidden)]
    fn make_inserts_removes(add_removers: Self::AddRemover<'_>) -> (Self::Inserts, Self::Removes);
}
impl<T: Component> Commands for T {
    type Inserts = Vec<Insert<T>>;
    type Removes = Vec<Remove>;
    type StorageBorrow<'a> = &'a mut StaticColumns<T>;
    type AddRemover<'b> = AddRemover<'b, T>;
    type BorrowedAddRemover<'a, 'b: 'a> = &'a mut AddRemover<'b, T>;
    type Passed<'a, 'b: 'a> = (&'a mut StaticColumns<T>, &'a mut AddRemover<'b, T>);
    fn make_add_remover(world: &World) -> Self::AddRemover<'_> {
        AddRemover {
            world,
            inserts: vec![],
            removes: vec![Remove::Swap],
        }
    }
    fn make_passed<'a, 'b: 'a>(
        storage: Self::StorageBorrow<'a>,
        add_remover: &'a mut Self::AddRemover<'b>,
    ) -> Self::Passed<'a, 'b> {
        (storage, add_remover)
    }
    fn flush(
        inserts: Vec<Insert<T>>,
        removes: Vec<Remove>,
        storages: Self::StorageBorrow<'_>,
        world: &mut World,
    ) {
        AddRemover::flush(inserts, removes, world, storages);
    }
    fn reborrow_storage<'a: 'b, 'b>(
        storage: &'b mut Self::StorageBorrow<'a>,
    ) -> Self::StorageBorrow<'b> {
        storage
    }
    fn make_inserts_removes(add_removers: Self::AddRemover<'_>) -> (Self::Inserts, Self::Removes) {
        (add_removers.inserts, add_removers.removes)
    }
}

macro_rules! foo_tuple_impl {
    ($( ($T:ident $U:ident $V:ident) )*) => {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        impl<$($T: Commands,)*> Commands for ($($T,)*) {
            type Inserts = ($($T::Inserts,)*);
            type Removes = ($($T::Removes,)*);
            type StorageBorrow<'a> = ($($T::StorageBorrow<'a>,)*);
            type AddRemover<'b> = ($($T::AddRemover<'b>,)*);
            type BorrowedAddRemover<'a, 'b: 'a> = ($($T::BorrowedAddRemover<'a, 'b>,)*);
            type Passed<'a, 'b: 'a> = ($($T::Passed<'a, 'b>,)*);
            fn make_add_remover(world: &World) -> Self::AddRemover<'_> {
                ($($T::make_add_remover(world),)*)
            }
            fn make_passed<'a, 'b: 'a>(
                storage: Self::StorageBorrow<'a>,
                add_remover: &'a mut Self::AddRemover<'b>,
            ) -> Self::Passed<'a, 'b> {
                let ($($U,)*) = storage;
                let ($($T,)*) = add_remover;
                ($($T::make_passed($U, $T),)*)
            }
            fn flush(inserts: Self::Inserts, removes: Self::Removes, storages: Self::StorageBorrow<'_>, world: &mut World) {
                let ($($T,)*) = inserts;
                let ($($U,)*) = removes;
                let ($($V,)*) = storages;
                $(
                    $T::flush($T, $U, $V, world);
                )*
            }
            fn reborrow_storage<'a: 'b, 'b>(
                storage: &'b mut Self::StorageBorrow<'a>,
            ) -> Self::StorageBorrow<'b> {
                let ($($T,)*) = storage;
                ($($T::reborrow_storage($T),)*)
            }
            fn make_inserts_removes(add_removers: Self::AddRemover<'_>) -> (Self::Inserts, Self::Removes) {
                let ($($T,)*) = add_removers;
                $(
                    let ($U, $V) = $T::make_inserts_removes($T);
                )*
                (($($U,)*), ($($V,)*))
            }
        }
    };
}

foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3) (D D2 D3) (E E2 E3) (F F2 F3) (G G2 G3) (H H2 H3));
foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3) (D D2 D3) (E E2 E3) (F F2 F3) (G G2 G3));
foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3) (D D2 D3) (E E2 E3) (F F2 F3));
foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3) (D D2 D3) (E E2 E3));
foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3) (D D2 D3));
foo_tuple_impl!((A A2 A3) (B B2 B3) (C C2 C3));
foo_tuple_impl!((A A2 A3) (B B2 B3));
foo_tuple_impl!((A A2 A3));

pub fn command_scope<Cmds: Commands, R /*F: FnOnce(Cmds::Passed<'_, '_>, &World) -> R*/>(
    world: &mut World,
    mut storage: Cmds::StorageBorrow<'_>,
    //func: F,
) -> R {
    // let mut add_removers = Cmds::make_add_remover(world);
    // let r = {
    //     let storage = Cmds::reborrow_storage(&mut storage);
    //     let passed = Cmds::make_passed(storage, &mut add_removers);
    //     func(passed, world)
    // };
    // let (inserts, removes) = Cmds::make_inserts_removes(add_removers);
    // Cmds::flush(inserts, removes, storage, world);
    // r
    todo!()
}

impl<'world, T: Component> AddRemover<'world, T> {
    fn flush(
        inserts: Vec<Insert<T>>,
        removes: Vec<Remove>,
        world: &mut World,
        storage: &mut StaticColumns<T>,
    ) {
        world
            .entities
            .fix_reserved_entities(|reserved| world.archetypes[0].entities.push(reserved));

        let mut inserts = inserts.into_iter();
        let mut removes = removes.into_iter();

        let mut next_remove = true;
        loop {
            match next_remove {
                true => match removes.next() {
                    None => break,
                    Some(Remove::Swap) => {
                        next_remove = false;
                        continue;
                    }
                    Some(Remove::Data(entity)) => {
                        storage.remove_component(world, entity);
                    }
                },
                false => match inserts.next() {
                    None => break,
                    Some(Insert::Swap) => {
                        next_remove = true;
                        continue;
                    }
                    Some(Insert::Data(entity, component)) => {
                        storage.insert_component(world, entity, component);
                    }
                },
            }
        }
    }

    pub fn spawn(&mut self) -> EntityBoundAddRemover<'_, 'world, T> {
        let entity = self.world.entities.reserve_entity();
        EntityBoundAddRemover {
            entity,
            add_remover: self,
        }
    }
    pub fn entity(&mut self, entity: Entity) -> EntityBoundAddRemover<'_, 'world, T> {
        EntityBoundAddRemover {
            entity,
            add_remover: self,
        }
    }
}

impl<'a, 'world, T: Component> EntityBoundAddRemover<'a, 'world, T> {
    pub fn insert(&mut self, component: T) -> &mut Self {
        if let Some(Insert::Swap) = self.add_remover.inserts.last() {
            self.add_remover.removes.push(Remove::Swap);
        }
        self.add_remover
            .inserts
            .push(Insert::Data(self.entity, component));
        self
    }
    pub fn remove(&mut self) -> &mut Self {
        if let Some(Remove::Swap) = self.add_remover.removes.last() {
            self.add_remover.inserts.push(Insert::Swap);
        }
        self.add_remover.removes.push(Remove::Data(self.entity));
        self
    }
}

mod tests {
    use crate::*;

    fn basic_insert() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let mut u64s = world.new_static_column::<u64>();
        let e = world.spawn().id();
        let (a, b) = (&mut u32s, &mut u64s);
        command_scope::<(u32, u64), ()>(&mut world, (a, b));
        // world.command_scope(|mut cmds| {
        //     cmds.entity(e).insert(10_u32).insert(12_u64).remove::<u32>();
        // });
        // let q = &*world.borrow::<u32>().unwrap();
        // let mut iter = ColumnIterator::new(q, &world);
        // assert_eq!(iter.next(), None);
        // let q = &*world.borrow::<u64>().unwrap();
        // let mut iter = ColumnIterator::new(q, &world);
        // assert_eq!(iter.next(), Some(&12));
        // assert_eq!(iter.next(), None);
    }

    #[test]
    fn spawn() {
        // let mut world = World::new();
        // let e1 = world.command_scope(|mut cmds| {
        //     cmds.spawn()
        //         .insert(10_u32)
        //         .insert(12_u64)
        //         .remove::<u32>()
        //         .id()
        // });

        // let q = &*world.borrow::<u32>().unwrap();
        // let mut iter = ColumnIterator::new((WithEntities, q), &world);
        // assert_eq!(iter.next(), None);
        // let q = &*world.borrow::<u64>().unwrap();
        // let mut iter = ColumnIterator::new((WithEntities, q), &world);
        // assert_eq!(iter.next(), Some((e1, &12)));
        // assert_eq!(iter.next(), None);
    }
}
